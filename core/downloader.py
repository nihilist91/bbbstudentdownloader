"""
core/downloader.py - High-speed, resilient asset downloader for BigBlueButton recordings.
"""
import os
import time
import urllib.request
import urllib.error
from urllib.parse import urljoin
from typing import Callable, Optional, Dict, Any, List


USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"


def create_request(url: str, cookie: Optional[str] = None) -> urllib.request.Request:
    """Creates a configured urllib request with proper headers and cookies."""
    headers = {
        "User-Agent": USER_AGENT,
        "Accept": "*/*",
        "Accept-Encoding": "identity",
    }
    if cookie:
        headers["Cookie"] = cookie
    return urllib.request.Request(url, headers=headers)


def fetch_text(url: str, cookie: Optional[str] = None, timeout: int = 15) -> str:
    """Fetches text content (XML, JSON, SVG) from a URL. Returns empty string on 404 or error."""
    req = create_request(url, cookie)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            content = resp.read()
            return content.decode("utf-8", errors="ignore")
    except Exception:
        return ""


def check_asset_exists(url: str, cookie: Optional[str] = None, timeout: int = 8) -> bool:
    """Checks if a remote asset exists using a lightweight request."""
    req = create_request(url, cookie)
    try:
        # Some servers don't support HEAD properly, so we request range 0-1
        req.headers["Range"] = "bytes=0-1"
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status in (200, 206)
    except urllib.error.HTTPError as e:
        return e.code == 206 or e.code == 200
    except Exception:
        # Fallback to GET with minimal read
        try:
            req_get = create_request(url, cookie)
            with urllib.request.urlopen(req_get, timeout=timeout) as resp:
                resp.read(128)
                return True
        except Exception:
            return False


def fetch_file(url: str, dest_path: str, cookie: Optional[str] = None,
               progress_callback: Optional[Callable[[float, int, int], None]] = None,
               timeout: int = 60) -> bool:
    """
    Downloads a binary file with chunked streaming and granular progress updates.
    progress_callback(fraction_0_to_1, bytes_done, bytes_total)
    """
    req = create_request(url, cookie)
    os.makedirs(os.path.dirname(os.path.abspath(dest_path)), exist_ok=True)
    temp_path = dest_path + ".part"

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp, open(temp_path, "wb") as f:
            total_size = int(resp.headers.get("Content-Length") or 0)
            downloaded = 0
            chunk_size = 1 << 20  # 1 MB

            while True:
                chunk = resp.read(chunk_size)
                if not chunk:
                    break
                f.write(chunk)
                downloaded += len(chunk)

                if progress_callback:
                    frac = (downloaded / total_size) if total_size > 0 else 0.5
                    progress_callback(min(1.0, frac), downloaded, total_size)

        if os.path.exists(dest_path):
            try:
                os.remove(dest_path)
            except OSError:
                pass
        os.replace(temp_path, dest_path)
        return True

    except Exception as e:
        if os.path.exists(temp_path):
            try:
                os.remove(temp_path)
            except OSError:
                pass
        return False


def probe_recording_assets(base_url: str, cookie: Optional[str] = None) -> Dict[str, bool]:
    """
    Probes all candidate assets on a BigBlueButton recording server to determine available components.
    """
    assets = {
        "metadata": check_asset_exists(f"{base_url}/metadata.xml", cookie),
        "webcams_webm": check_asset_exists(f"{base_url}/video/webcams.webm", cookie),
        "webcams_mp4": check_asset_exists(f"{base_url}/video/webcams.mp4", cookie),
        "deskshare_webm": check_asset_exists(f"{base_url}/deskshare/deskshare.webm", cookie),
        "deskshare_mp4": check_asset_exists(f"{base_url}/deskshare/deskshare.mp4", cookie),
        "shapes_svg": check_asset_exists(f"{base_url}/shapes.svg", cookie),
        "chat_xml": check_asset_exists(f"{base_url}/chat.xml", cookie),
        "slides_new_xml": check_asset_exists(f"{base_url}/slides_new.xml", cookie),
        "presentation_text_json": check_asset_exists(f"{base_url}/presentation_text.json", cookie),
        "notes_html": check_asset_exists(f"{base_url}/notes.html", cookie) or check_asset_exists(f"{base_url}/notes/notes.html", cookie),
        "cursor_xml": check_asset_exists(f"{base_url}/cursor.xml", cookie),
        "panzooms_xml": check_asset_exists(f"{base_url}/panzooms.xml", cookie),
    }

    # Consolidated flags
    assets["has_audio"] = assets["webcams_webm"] or assets["webcams_mp4"]
    assets["has_deskshare"] = assets["deskshare_webm"] or assets["deskshare_mp4"]
    assets["has_slides"] = assets["shapes_svg"]
    assets["has_chat"] = assets["chat_xml"] or assets["slides_new_xml"]
    return assets


def download_all_slides(base_url: str, slides: List[dict], output_folder: str,
                        cookie: Optional[str] = None,
                        progress_callback: Optional[Callable[[float, str], None]] = None) -> List[dict]:
    """
    Downloads all slide image assets discovered in shapes.svg into output_folder/slides.
    Returns list of slides with local file paths.
    """
    slides_dir = os.path.join(output_folder, "slides")
    os.makedirs(slides_dir, exist_ok=True)
    total = len(slides)

    updated_slides = []
    for i, s in enumerate(slides, start=1):
        href = s["href"].lstrip("/")
        # URL could be relative e.g. "presentation/presId/slide-1.png"
        full_url = f"{base_url}/{href}" if not href.startswith("http") else href
        ext = os.path.splitext(href)[1] or ".png"
        local_name = f"slide_{s['index']:03d}{ext}"
        local_dest = os.path.join(slides_dir, local_name)

        if not os.path.exists(local_dest):
            ok = fetch_file(full_url, local_dest, cookie=cookie)
            if not ok and ext == ".png":
                # Try .svg counterpart
                alt_url = full_url[:-4] + ".svg"
                alt_dest = os.path.join(slides_dir, f"slide_{s['index']:03d}.svg")
                if fetch_file(alt_url, alt_dest, cookie=cookie):
                    local_dest = alt_dest
                    local_name = os.path.basename(alt_dest)
            elif not ok and ext == ".svg":
                # Try .png counterpart
                alt_url = full_url[:-4] + ".png"
                alt_dest = os.path.join(slides_dir, f"slide_{s['index']:03d}.png")
                if fetch_file(alt_url, alt_dest, cookie=cookie):
                    local_dest = alt_dest
                    local_name = os.path.basename(alt_dest)

        s_copy = dict(s)
        s_copy["local_path"] = local_dest
        s_copy["local_filename"] = f"slides/{local_name}"
        s_copy["exists"] = os.path.exists(local_dest)
        updated_slides.append(s_copy)

        if progress_callback:
            progress_callback(i / total, f"Downloaded slide {i}/{total}")

    return updated_slides
