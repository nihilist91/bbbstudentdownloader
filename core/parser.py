"""
core/parser.py - Robust metadata, SVG timeline, and chat parser for BigBlueButton recordings.
"""
import re
import html
import json
import datetime
from typing import Optional, List, Dict, Tuple, Any
from urllib.parse import urlparse
import xml.etree.ElementTree as ET


def parse_bbb_url(url: str):
    """
    Parses any BigBlueButton recording URL and extracts meeting ID, base presentation URL,
    and base server origin.
    Handles:
      - /playback/presentation/2.3/<meetingId>
      - /playback/presentation/2.0/playback.html?meetingId=<meetingId>
      - /presentation/<meetingId>
    """
    url = url.strip()
    u = urlparse(url)
    if not u.scheme or not u.netloc:
        raise ValueError("Invalid URL: Missing scheme (http/https) or domain name.")

    # 1. Check query parameter ?meetingId=...
    m_query = re.search(r'[?&]meetingId=([a-zA-Z0-9_\-]+)', url)
    if m_query:
        meeting_id = m_query.group(1)
    else:
        # 2. Check path /playback/presentation/.../<meetingId>
        m_path = re.search(r'/playback/presentation/(?:[\d.]+/)?(?:playback\.html/?)?([a-zA-Z0-9_\-]+)', u.path)
        if m_path and not m_path.group(1).endswith(('.html', '.htm', '.php')):
            meeting_id = m_path.group(1)
        else:
            # 3. Check /presentation/<meetingId>
            m_alt = re.search(r'/presentation/([a-zA-Z0-9_\-]+)', u.path)
            if m_alt:
                meeting_id = m_alt.group(1)
            else:
                raise ValueError("Could not extract a BigBlueButton meeting ID from the URL. "
                                 "The link should contain /playback/presentation/ or ?meetingId=")

    base_url = f"{u.scheme}://{u.netloc}/presentation/{meeting_id}"
    server_origin = f"{u.scheme}://{u.netloc}"
    return meeting_id, base_url, server_origin


def safe_filename(name: str, max_len: int = 100) -> str:
    """Sanitizes strings for safe filenames across Windows, macOS, and Linux."""
    clean = re.sub(r'[\\/:*?"<>|\r\n\t]+', " ", str(name or "")).strip()
    clean = re.sub(r'\s+', " ", clean)
    return clean[:max_len].strip() or "Lecture"


def format_duration(seconds: float) -> str:
    """Formats seconds into readable string, e.g. '1h 24m' or '45m 12s'."""
    sec = int(round(seconds))
    hrs = sec // 3600
    mins = (sec % 3600) // 60
    s = sec % 60
    if hrs > 0:
        return f"{hrs}h {mins:02d}m"
    elif mins > 0:
        return f"{mins}m {s:02d}s"
    return f"{s}s"


def format_timestamp(seconds: float) -> str:
    """Formats seconds into [HH:MM:SS] or [MM:SS]."""
    sec = max(0, int(round(seconds)))
    hrs = sec // 3600
    mins = (sec % 3600) // 60
    s = sec % 60
    if hrs > 0:
        return f"{hrs:02d}:{mins:02d}:{s:02d}"
    return f"{mins:02d}:{s:02d}"


def parse_time_string(time_str: str) -> Optional[float]:
    """
    Parses a user-supplied time string (e.g. '01:45:00', '1h 30m', '90m', '5400')
    into seconds as a float. Returns None if invalid or blank.
    """
    if not time_str or not str(time_str).strip():
        return None
    s = str(time_str).strip().lower()

    # HH:MM:SS or MM:SS
    if ":" in s:
        parts = s.split(":")
        try:
            if len(parts) == 3:
                return float(parts[0]) * 3600 + float(parts[1]) * 60 + float(parts[2])
            elif len(parts) == 2:
                return float(parts[0]) * 60 + float(parts[1])
        except ValueError:
            pass

    # e.g. "1h 45m" or "2h" or "90m"
    m_h = re.search(r'(\d+(?:\.\d+)?)\s*h', s)
    m_m = re.search(r'(\d+(?:\.\d+)?)\s*m', s)
    m_s = re.search(r'(\d+(?:\.\d+)?)\s*s', s)
    if m_h or m_m or m_s:
        hrs = float(m_h.group(1)) if m_h else 0.0
        mins = float(m_m.group(1)) if m_m else 0.0
        secs = float(m_s.group(1)) if m_s else 0.0
        return hrs * 3600 + mins * 60 + secs

    # Pure number (seconds)
    try:
        return float(s)
    except ValueError:
        return None


def detect_lecture_end_time(total_duration: float, slides: list, chat_messages: list,
                            buffer_seconds: float = 600.0) -> tuple:
    """
    Detects if a lecture has excessive trailing dead time (e.g. professor forgot to
    stop recording and left an empty room for hours after class finished).

    Returns:
      (cut_seconds: float, has_dead_time: bool, dead_time_seconds: float)
    """
    if total_duration <= 0:
        return total_duration, False, 0.0

    last_slide_flip = 0.0
    if slides:
        last_slide_flip = max(s.get("in", 0.0) for s in slides)

    last_chat_time = 0.0
    if chat_messages:
        last_chat_time = max(m.get("time_sec", 0.0) for m in chat_messages)

    last_activity = max(last_slide_flip, last_chat_time)
    if last_activity <= 0:
        return total_duration, False, 0.0

    recommended_cut = last_activity + buffer_seconds

    if recommended_cut >= total_duration:
        return total_duration, False, 0.0

    dead_time = total_duration - recommended_cut
    if dead_time >= 900.0:  # > 15 minutes of dead air
        return round(recommended_cut, 2), True, round(dead_time, 2)

    return total_duration, False, 0.0


def parse_metadata(xml_text: str, meeting_id_fallback: str = "") -> dict:
    """
    Parses BigBlueButton metadata.xml to extract title, course name, presenter, date, duration, etc.
    """
    meta = {
        "meeting_id": meeting_id_fallback,
        "name": "",
        "course_name": "",
        "presenter": "",
        "date_str": "",
        "date_iso": "",
        "duration_seconds": 0.0,
        "duration_formatted": "Unknown duration",
        "participants": 0,
        "description": "",
    }

    if not xml_text:
        return meta

    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        # Fallback to regex if XML is slightly malformed
        pass
    else:
        # Meeting ID
        meta["meeting_id"] = root.findtext(".//id") or root.findtext(".//meeting[@id]") or meeting_id_fallback

        # Meeting Name / Title
        name = root.findtext(".//meeting/name") or root.findtext(".//meeting/title") or root.findtext(".//name") or ""
        meta["name"] = html.unescape(name).strip()

        # Meta section tags
        meta_node = root.find(".//meta")
        if meta_node is not None:
            for child in meta_node:
                tag_l = child.tag.lower()
                val = html.unescape(child.text or "").strip()
                if not val:
                    continue
                if any(k in tag_l for k in ("context", "course", "subject", "module")):
                    if not meta["course_name"] or "name" in tag_l:
                        meta["course_name"] = val
                elif any(k in tag_l for k in ("creator", "presenter", "teacher", "author")):
                    meta["presenter"] = val
                elif "description" in tag_l:
                    meta["description"] = val

        # Start Time
        start_ts = root.findtext(".//start_time") or root.findtext(".//startTime")
        if start_ts and str(start_ts).strip().isdigit():
            ts_num = int(start_ts.strip())
            # Convert milliseconds if needed
            if ts_num > 10_000_000_000:
                ts_num /= 1000.0
            try:
                dt = datetime.datetime.fromtimestamp(ts_num)
                meta["date_str"] = dt.strftime("%Y-%m-%d %H:%M")
                meta["date_iso"] = dt.isoformat()
            except Exception:
                pass

        # Duration
        dur_val = root.findtext(".//playback/duration") or root.findtext(".//duration")
        if dur_val and str(dur_val).strip().isdigit():
            dur_num = int(dur_val.strip())
            if dur_num > 10_000:  # milliseconds
                meta["duration_seconds"] = dur_num / 1000.0
            else:
                meta["duration_seconds"] = float(dur_num)
            meta["duration_formatted"] = format_duration(meta["duration_seconds"])

        # Participants
        parts = root.findtext(".//participants")
        if parts and parts.isdigit():
            meta["participants"] = int(parts)

    # Fallback course name if empty
    if not meta["course_name"]:
        # Try regex across whole text for context or course
        matches = re.findall(r'<([\w.\-]*context[\w.\-]*)>(.*?)</\1>', xml_text, re.I | re.S)
        for tag, val in matches:
            val_clean = html.unescape(val).strip()
            if val_clean:
                meta["course_name"] = val_clean
                break

    if not meta["course_name"]:
        meta["course_name"] = meta["name"] or meta["meeting_id"]

    return meta


def parse_shapes_svg(svg_text: str) -> list:
    """
    Parses shapes.svg from BigBlueButton recording to find all slides,
    their display times (in/out timestamps), and source image links.
    Returns list of dicts:
      [{"index": 1, "href": "presentation/.../slide-1.png", "in": 0.0, "out": 45.2, "width": 1280, "height": 720}]
    """
    slides = []
    if not svg_text:
        return slides

    # Match all <image ...> tags
    for match in re.finditer(r'<image\b([^>]+)>', svg_text, re.IGNORECASE):
        attrs = match.group(1)
        href_m = re.search(r'(?:xlink:href|href)=["\']([^"\']+)["\']', attrs, re.IGNORECASE)
        in_m = re.search(r'\bin=["\']([\d.]+)["\']', attrs, re.IGNORECASE)
        out_m = re.search(r'\bout=["\']([\d.]+)["\']', attrs, re.IGNORECASE)
        w_m = re.search(r'\bwidth=["\']([\d.]+)["\']', attrs, re.IGNORECASE)
        h_m = re.search(r'\bheight=["\']([\d.]+)["\']', attrs, re.IGNORECASE)

        if href_m:
            href = href_m.group(1).strip()
            # Only consider presentation slides (ignore small icons or cursors)
            if "slide-" in href or "presentation" in href or href.endswith(('.png', '.svg', '.jpg', '.jpeg')):
                start_t = float(in_m.group(1)) if in_m else 0.0
                end_t = float(out_m.group(1)) if out_m else start_t + 10.0
                w = float(w_m.group(1)) if w_m else 1280.0
                h = float(h_m.group(1)) if h_m else 720.0
                slides.append({
                    "href": href,
                    "in": round(start_t, 2),
                    "out": round(end_t, 2),
                    "duration": round(max(1.0, end_t - start_t), 2),
                    "width": int(w),
                    "height": int(h),
                })

    # Sort slides by start time
    slides.sort(key=lambda s: s["in"])
    for idx, s in enumerate(slides, start=1):
        s["index"] = idx

    return slides


def parse_chat_xml(xml_text: str) -> tuple:
    """
    Parses chat.xml to extract all public chat messages, questions, and shared links.
    Returns (messages, links, questions).
    """
    messages = []
    links = []
    questions = []

    if not xml_text:
        return messages, links, questions

    try:
        root = ET.fromstring(xml_text)
    except ET.ParseError:
        return messages, links, questions

    # Format 1: <chattimeline in="14.5" target="chat" name="Alice" message="..." />
    for item in root.findall(".//chattimeline"):
        in_t = float(item.get("in", 0) or 0)
        name = html.unescape(item.get("name", "Participant") or "Participant").strip()
        msg = html.unescape(item.get("message", "") or item.text or "").strip()
        if not msg:
            continue

        time_str = format_timestamp(in_t)
        found_urls = re.findall(r'https?://[^\s<>"]+', msg)
        for u in found_urls:
            if u not in links:
                links.append(u)

        is_question = "?" in msg
        entry = {
            "time_sec": in_t,
            "time_str": time_str,
            "sender": name,
            "message": msg,
            "is_question": is_question,
            "urls": found_urls
        }
        messages.append(entry)
        if is_question:
            questions.append(entry)

    # Format 2: <event module="CHAT"><sender>...</sender><message>...</message></event>
    if not messages:
        for ev in root.findall(".//event[@module='CHAT']"):
            sender = html.unescape(ev.findtext("sender") or "Participant").strip()
            msg = html.unescape(ev.findtext("message") or "").strip()
            ts_str = ev.get("timestamp") or "0"
            in_t = float(ts_str) / 1000.0 if float(ts_str) > 10_000_000_000 else float(ts_str)
            if not msg:
                continue

            time_str = format_timestamp(in_t)
            found_urls = re.findall(r'https?://[^\s<>"]+', msg)
            for u in found_urls:
                if u not in links:
                    links.append(u)

            is_question = "?" in msg
            entry = {
                "time_sec": in_t,
                "time_str": time_str,
                "sender": sender,
                "message": msg,
                "is_question": is_question,
                "urls": found_urls
            }
            messages.append(entry)
            if is_question:
                questions.append(entry)

    messages.sort(key=lambda m: m["time_sec"])
    return messages, links, questions


def parse_presentation_text(json_text: str) -> dict:
    """
    Parses presentation_text.json to extract slide text content.
    Returns dict mapping slide identifier or index to text string.
    """
    result = {}
    if not json_text:
        return result

    try:
        data = json.loads(json_text)
        if isinstance(data, dict):
            # Form 1: {"slides": [{"id": "slide-1", "num": 1, "text": "..."}]}
            slides = data.get("slides", [])
            for s in slides:
                if isinstance(s, dict):
                    sid = s.get("id") or str(s.get("num", ""))
                    txt = s.get("text", "").strip()
                    if sid:
                        result[str(sid)] = txt
        elif isinstance(data, list):
            # Form 2: [{"slide": 1, "text": "..."}]
            for item in data:
                if isinstance(item, dict):
                    sid = str(item.get("slide") or item.get("id") or item.get("num") or len(result)+1)
                    txt = item.get("text", "").strip()
                    result[sid] = txt
    except Exception:
        pass

    return result
