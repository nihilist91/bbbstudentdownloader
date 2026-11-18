//! Port of core/parser.py: URL, metadata, shapes.svg, chat.xml and slide-text parsing.

use chrono::{Local, TimeZone};
use indexmap::IndexMap;
use serde_json::Value;
use url::Url;

use crate::html;
use crate::model::{ChatMessage, Metadata, Slide};
use crate::xml::{self, Element};
use crate::{py_round2, Result};

pub struct BbbUrl {
    pub meeting_id: String,
    pub base_url: String,
    pub server_origin: String,
}

/// Extracts the meeting ID and base URL from any BigBlueButton recording link.
pub fn parse_bbb_url(input: &str) -> Result<BbbUrl> {
    let raw = input.trim();
    let u = Url::parse(raw)
        .map_err(|_| "Invalid URL: Missing scheme (http/https) or domain name.")?;
    let netloc = u.authority().to_string();
    if netloc.is_empty() {
        return Err("Invalid URL: Missing scheme (http/https) or domain name.".into());
    }

    // 1. ?meetingId=...
    let meeting_id = if let Some(c) = crate::regex!(r"[?&]meetingId=([a-zA-Z0-9_\-]+)").captures(raw) {
        c[1].to_string()
    } else {
        let from_path = crate::regex!(r"/playback/presentation/(?:[\d.]+/)?(?:playback\.html/?)?([a-zA-Z0-9_\-]+)")
            .captures(u.path())
            .map(|c| c[1].to_string())
            .filter(|id| !id.ends_with(".html") && !id.ends_with(".htm") && !id.ends_with(".php"));
        // 2. /playback/presentation/.../<id>  3. /presentation/<id>
        match from_path {
            Some(id) => id,
            None => match crate::regex!(r"/presentation/([a-zA-Z0-9_\-]+)").captures(u.path()) {
                Some(c) => c[1].to_string(),
                None => {
                    return Err("Could not extract a BigBlueButton meeting ID from the URL. \
                                The link should contain /playback/presentation/ or ?meetingId="
                        .into())
                }
            },
        }
    };

    Ok(BbbUrl {
        base_url: format!("{}://{}/presentation/{}", u.scheme(), netloc, meeting_id),
        server_origin: format!("{}://{}", u.scheme(), netloc),
        meeting_id,
    })
}

/// Makes a string safe to use as a file or folder name on all major OSes.
pub fn safe_filename(name: &str, max_len: usize) -> String {
    let stripped = crate::regex!(r#"[\\/:*?"<>|\r\n\t]+"#).replace_all(name, " ");
    let collapsed = crate::regex!(r"\s+").replace_all(stripped.trim(), " ");
    let clipped: String = collapsed.chars().take(max_len).collect();
    let clean = clipped.trim();
    if clean.is_empty() { "Lecture".to_string() } else { clean.to_string() }
}

/// "1h 24m", "45m 12s" or "30s".
pub fn format_duration(seconds: f64) -> String {
    let sec = crate::py_round(seconds) as i64;
    let hrs = sec / 3600;
    let mins = (sec % 3600) / 60;
    let s = sec % 60;
    if hrs > 0 {
        format!("{hrs}h {mins:02}m")
    } else if mins > 0 {
        format!("{mins}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

/// "[HH:MM:SS]" style, without brackets: "HH:MM:SS" or "MM:SS".
pub fn format_timestamp(seconds: f64) -> String {
    let sec = crate::py_round(seconds).max(0.0) as i64;
    let hrs = sec / 3600;
    let mins = (sec % 3600) / 60;
    let s = sec % 60;
    if hrs > 0 {
        format!("{hrs:02}:{mins:02}:{s:02}")
    } else {
        format!("{mins:02}:{s:02}")
    }
}

/// Parses user input such as "01:45:00", "1h 30m", "90m" or "5400" into seconds.
pub fn parse_time_string(input: &str) -> Option<f64> {
    let s = input.trim().to_lowercase();
    if s.is_empty() {
        return None;
    }

    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        let nums: Option<Vec<f64>> = parts.iter().map(|p| p.trim().parse::<f64>().ok()).collect();
        if let Some(n) = nums {
            match n.len() {
                3 => return Some(n[0] * 3600.0 + n[1] * 60.0 + n[2]),
                2 => return Some(n[0] * 60.0 + n[1]),
                _ => {}
            }
        }
    }

    let cap = |re: &regex::Regex| -> Option<f64> { re.captures(&s)?.get(1)?.as_str().parse().ok() };
    let h = cap(crate::regex!(r"(\d+(?:\.\d+)?)\s*h"));
    let m = cap(crate::regex!(r"(\d+(?:\.\d+)?)\s*m"));
    let sec = cap(crate::regex!(r"(\d+(?:\.\d+)?)\s*s"));
    if h.is_some() || m.is_some() || sec.is_some() {
        return Some(h.unwrap_or(0.0) * 3600.0 + m.unwrap_or(0.0) * 60.0 + sec.unwrap_or(0.0));
    }

    s.parse::<f64>().ok()
}

/// Detects trailing dead air after class ended.
/// Returns (cut_seconds, has_dead_time, dead_time_seconds).
pub fn detect_lecture_end_time(
    total_duration: f64,
    slides: &[Slide],
    chat_messages: &[ChatMessage],
    buffer_seconds: f64,
) -> (f64, bool, f64) {
    if total_duration <= 0.0 {
        return (total_duration, false, 0.0);
    }
    let last_slide_flip = slides.iter().map(|s| s.start).fold(0.0_f64, f64::max);
    let last_chat_time = chat_messages.iter().map(|m| m.time_sec).fold(0.0_f64, f64::max);
    let last_activity = last_slide_flip.max(last_chat_time);
    if last_activity <= 0.0 {
        return (total_duration, false, 0.0);
    }
    let recommended_cut = last_activity + buffer_seconds;
    if recommended_cut >= total_duration {
        return (total_duration, false, 0.0);
    }
    let dead_time = total_duration - recommended_cut;
    if dead_time >= 900.0 {
        return (py_round2(recommended_cut), true, py_round2(dead_time));
    }
    (total_duration, false, 0.0)
}

/// Reads the first non-empty candidate, like Python's `a or b or c`.
fn first_nonempty(candidates: &[String]) -> String {
    candidates.iter().find(|s| !s.is_empty()).cloned().unwrap_or_default()
}

fn is_ascii_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

pub fn parse_metadata(xml_text: &str, meeting_id_fallback: &str) -> Metadata {
    let mut meta = Metadata { meeting_id: meeting_id_fallback.to_string(), ..Default::default() };
    if xml_text.is_empty() {
        return meta;
    }

    if let Some(root) = xml::parse(xml_text) {
        meta.meeting_id = first_nonempty(&[
            root.find("id").map(|e| e.text.clone()).unwrap_or_default(),
            meeting_with_id_text(&root),
            meeting_id_fallback.to_string(),
        ]);

        let name = first_nonempty(&[
            root.find_path_text("meeting", "name"),
            root.find_path_text("meeting", "title"),
            root.find("name").map(|e| e.text.clone()).unwrap_or_default(),
        ]);
        meta.name = html::unescape(&name).trim().to_string();

        if let Some(meta_node) = root.find("meta") {
            for child in &meta_node.children {
                let tag_l = child.tag.to_lowercase();
                let val = html::unescape(&child.text).trim().to_string();
                if val.is_empty() {
                    continue;
                }
                if ["context", "course", "subject", "module"].iter().any(|k| tag_l.contains(k)) {
                    if meta.course_name.is_empty() || tag_l.contains("name") {
                        meta.course_name = val;
                    }
                } else if ["creator", "presenter", "teacher", "author"].iter().any(|k| tag_l.contains(k)) {
                    meta.presenter = val;
                } else if tag_l.contains("description") {
                    meta.description = val;
                }
            }
        }

        let start_ts = first_nonempty(&[
            root.find("start_time").map(|e| e.text.clone()).unwrap_or_default(),
            root.find("startTime").map(|e| e.text.clone()).unwrap_or_default(),
        ]);
        let start_ts = start_ts.trim();
        if is_ascii_digits(start_ts) {
            let mut ts: f64 = start_ts.parse().unwrap_or(0.0);
            if ts > 10_000_000_000.0 {
                ts /= 1000.0;
            }
            let secs = ts.floor() as i64;
            let nanos = ((ts - ts.floor()) * 1e9) as u32;
            if let Some(dt) = Local.timestamp_opt(secs, nanos).single() {
                let naive = dt.naive_local();
                meta.date_str = naive.format("%Y-%m-%d %H:%M").to_string();
                let micros = naive.and_utc().timestamp_subsec_micros();
                let mut iso = naive.format("%Y-%m-%dT%H:%M:%S").to_string();
                if micros != 0 {
                    iso.push_str(&format!(".{micros:06}"));
                }
                meta.date_iso = iso;
            }
        }

        let dur_val = first_nonempty(&[
            root.find_path_text("playback", "duration"),
            root.find("duration").map(|e| e.text.clone()).unwrap_or_default(),
        ]);
        let dur_val = dur_val.trim();
        if is_ascii_digits(dur_val) {
            let n: f64 = dur_val.parse().unwrap_or(0.0);
            // Values above 10,000 are milliseconds.
            meta.duration_seconds = if n > 10_000.0 { n / 1000.0 } else { n };
            meta.duration_formatted = format_duration(meta.duration_seconds);
        }

        let parts = root.find("participants").map(|e| e.text.clone()).unwrap_or_default();
        if is_ascii_digits(&parts) {
            meta.participants = parts.parse().unwrap_or(0);
        }
    }

    if meta.course_name.is_empty() {
        if let Some(course) = course_from_raw_text(xml_text) {
            meta.course_name = course;
        }
    }
    if meta.course_name.is_empty() {
        meta.course_name = if meta.name.is_empty() { meta.meeting_id.clone() } else { meta.name.clone() };
    }
    meta
}

/// ElementTree's `.//meeting[@id]` findtext: the text of the first meeting element that has an id.
fn meeting_with_id_text(root: &Element) -> String {
    root.find_all("meeting")
        .into_iter()
        .find(|e| e.attr("id").is_some())
        .map(|e| e.text.clone())
        .unwrap_or_default()
}

/// Regex fallback used by the Python code when the XML is malformed.
fn course_from_raw_text(xml_text: &str) -> Option<String> {
    let opening = crate::regex!(r"(?i)<([\w.\-]*context[\w.\-]*)>");
    for c in opening.captures_iter(xml_text) {
        let tag = c.get(1)?.as_str();
        let body_start = c.get(0)?.end();
        let closing = regex::Regex::new(&format!(r"(?is)</{}>", regex::escape(tag))).ok()?;
        if let Some(m) = closing.find(&xml_text[body_start..]) {
            let val = html::unescape(&xml_text[body_start..body_start + m.start()]).trim().to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

/// Extracts slides (image tags with in/out timestamps) from shapes.svg.
pub fn parse_shapes_svg(svg_text: &str) -> Vec<Slide> {
    let mut slides: Vec<Slide> = Vec::new();
    if svg_text.is_empty() {
        return slides;
    }
    let num = |attrs: &str, re: &regex::Regex| -> Option<f64> {
        re.captures(attrs)?.get(1)?.as_str().parse().ok()
    };

    for m in crate::regex!(r"(?i)<image\b([^>]+)>").captures_iter(svg_text) {
        let attrs = &m[1];
        let Some(href_c) = crate::regex!(r#"(?i)(?:xlink:href|href)=["']([^"']+)["']"#).captures(attrs) else {
            continue;
        };
        let href = href_c[1].trim().to_string();
        if !(href.contains("slide-") || href.contains("presentation")
            || href.ends_with(".png") || href.ends_with(".svg")
            || href.ends_with(".jpg") || href.ends_with(".jpeg"))
        {
            continue;
        }
        let start = num(attrs, crate::regex!(r#"(?i)\bin=["']([\d.]+)["']"#)).unwrap_or(0.0);
        let end = num(attrs, crate::regex!(r#"(?i)\bout=["']([\d.]+)["']"#)).unwrap_or(start + 10.0);
        let width = num(attrs, crate::regex!(r#"(?i)\bwidth=["']([\d.]+)["']"#)).unwrap_or(1280.0);
        let height = num(attrs, crate::regex!(r#"(?i)\bheight=["']([\d.]+)["']"#)).unwrap_or(720.0);
        slides.push(Slide {
            href,
            start: py_round2(start),
            end: py_round2(end),
            duration: py_round2((end - start).max(1.0)),
            width: width as i64,
            height: height as i64,
            ..Default::default()
        });
    }

    slides.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
    for (i, s) in slides.iter_mut().enumerate() {
        s.index = i + 1;
    }
    slides
}

pub struct ChatParse {
    pub messages: Vec<ChatMessage>,
    pub links: Vec<String>,
    pub questions: Vec<ChatMessage>,
}

fn push_chat(out: &mut ChatParse, time_sec: f64, sender: String, message: String) {
    let urls: Vec<String> = crate::regex!(r#"https?://[^\s<>"]+"#)
        .find_iter(&message)
        .map(|m| m.as_str().to_string())
        .collect();
    for u in &urls {
        if !out.links.contains(u) {
            out.links.push(u.clone());
        }
    }
    let is_question = message.contains('?');
    let entry = ChatMessage {
        time_sec,
        time_str: format_timestamp(time_sec),
        sender,
        message,
        is_question,
        urls,
    };
    if is_question {
        out.questions.push(entry.clone());
    }
    out.messages.push(entry);
}

/// Parses public chat from chat.xml (two layouts seen in BBB recordings).
pub fn parse_chat_xml(xml_text: &str) -> ChatParse {
    let mut out = ChatParse { messages: Vec::new(), links: Vec::new(), questions: Vec::new() };
    if xml_text.is_empty() {
        return out;
    }
    let Some(root) = xml::parse(xml_text) else {
        return out;
    };

    // Format 1: <chattimeline in="14.5" name="Alice" message="..." />
    for item in root.find_all("chattimeline") {
        let in_t: f64 = item.attr("in").unwrap_or("0").trim().parse().unwrap_or(0.0);
        let name = html::unescape(item.attr("name").filter(|s| !s.is_empty()).unwrap_or("Participant"))
            .trim()
            .to_string();
        let raw = match item.attr("message") {
            Some(m) if !m.is_empty() => m.to_string(),
            _ => item.text.clone(),
        };
        let msg = html::unescape(&raw).trim().to_string();
        if msg.is_empty() {
            continue;
        }
        push_chat(&mut out, in_t, name, msg);
    }

    // Format 2: <event module="CHAT"><sender>..</sender><message>..</message></event>
    if out.messages.is_empty() {
        for ev in root.find_all("event").into_iter().filter(|e| e.attr("module") == Some("CHAT")) {
            let sender_raw = ev.child_text("sender");
            let sender = html::unescape(if sender_raw.is_empty() { "Participant" } else { &sender_raw })
                .trim()
                .to_string();
            let msg = html::unescape(&ev.child_text("message")).trim().to_string();
            let ts_str = ev.attr("timestamp").filter(|s| !s.is_empty()).unwrap_or("0");
            let ts: f64 = ts_str.trim().parse().unwrap_or(0.0);
            let in_t = if ts > 10_000_000_000.0 { ts / 1000.0 } else { ts };
            if msg.is_empty() {
                continue;
            }
            push_chat(&mut out, in_t, sender, msg);
        }
    }

    out.messages
        .sort_by(|a, b| a.time_sec.partial_cmp(&b.time_sec).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Python-style truthiness for a JSON value (used for `a or b` chains).
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Python's `str()` for the JSON scalars that appear in presentation_text.json.
fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        other => other.to_string(),
    }
}

/// Maps slide identifier (as string) to its extracted text.
pub fn parse_presentation_text(json_text: &str) -> IndexMap<String, String> {
    let mut result: IndexMap<String, String> = IndexMap::new();
    if json_text.is_empty() {
        return result;
    }
    let Ok(data) = serde_json::from_str::<Value>(json_text) else {
        return result;
    };
    let text_of = |v: &Value| v.get("text").and_then(Value::as_str).unwrap_or("").trim().to_string();

    match &data {
        Value::Object(_) => {
            if let Some(Value::Array(slides)) = data.get("slides") {
                for s in slides.iter().filter(|s| s.is_object()) {
                    let sid = if truthy(s.get("id")) {
                        py_str(s.get("id").unwrap())
                    } else {
                        s.get("num").map(py_str).unwrap_or_default()
                    };
                    if !sid.is_empty() {
                        result.insert(sid, text_of(s));
                    }
                }
            }
        }
        Value::Array(items) => {
            for item in items.iter().filter(|i| i.is_object()) {
                let sid = if truthy(item.get("slide")) {
                    py_str(item.get("slide").unwrap())
                } else if truthy(item.get("id")) {
                    py_str(item.get("id").unwrap())
                } else if truthy(item.get("num")) {
                    py_str(item.get("num").unwrap())
                } else {
                    (result.len() + 1).to_string()
                };
                result.insert(sid, text_of(item));
            }
        }
        _ => {}
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_variations() {
        let cases = [
            ("https://bbb.example.edu/playback/presentation/2.3/d49096f2a452556e34413acf9c48068280699fba-1716216469672",
             "d49096f2a452556e34413acf9c48068280699fba-1716216469672"),
            ("https://bbb.example.edu/playback/presentation/2.0/playback.html?meetingId=meeting-xyz-123", "meeting-xyz-123"),
            ("https://bbb.example.edu/presentation/meeting-abc-789", "meeting-abc-789"),
            ("https://moodle.univ.edu/playback/presentation/2.3/session-999?token=abc", "session-999"),
        ];
        for (u, id) in cases {
            let parsed = parse_bbb_url(u).unwrap();
            assert_eq!(parsed.meeting_id, id);
            assert!(parsed.base_url.ends_with(&format!("/presentation/{id}")));
        }
        assert!(parse_bbb_url("not a url").is_err());
    }

    #[test]
    fn metadata_parsing() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <recording>
            <id>rec-12345</id>
            <start_time>1716216469672</start_time>
            <meeting id="rec-12345"><name>Advanced Machine Learning: Transformers</name></meeting>
            <meta>
                <context>CS480 - Deep Learning</context>
                <creator>Dr. Yoshua Bengio</creator>
                <description>Attention mechanism and self-attention</description>
            </meta>
            <playback><duration>5400000</duration></playback>
        </recording>"#;
        let meta = parse_metadata(xml, "");
        assert_eq!(meta.name, "Advanced Machine Learning: Transformers");
        assert_eq!(meta.course_name, "CS480 - Deep Learning");
        assert_eq!(meta.presenter, "Dr. Yoshua Bengio");
        assert_eq!(meta.duration_formatted, "1h 30m");
        assert!(!meta.date_str.is_empty());
    }

    #[test]
    fn shapes_svg_parsing() {
        let svg = r#"<svg>
            <image id="im1" in="0" out="35.5" xlink:href="presentation/p1/slide-1.png" width="1280" height="720"/>
            <image id="im2" in="35.5" out="110.2" href="presentation/p1/slide-2.png" width="1280" height="720"/>
            <image id="im3" in="110.2" out="300" xlink:href="presentation/p1/slide-3.svg" width="1280" height="720"/>
        </svg>"#;
        let slides = parse_shapes_svg(svg);
        assert_eq!(slides.len(), 3);
        assert_eq!(slides[0].index, 1);
        assert_eq!(slides[0].start, 0.0);
        assert_eq!(slides[0].end, 35.5);
        assert_eq!(slides[2].href, "presentation/p1/slide-3.svg");
    }

    #[test]
    fn chat_parsing() {
        let chat = r#"<recording><chat>
            <chattimeline in="12.0" target="chat" name="Alice" message="Hello! Is homework 2 due Friday?" />
            <chattimeline in="25.4" target="chat" name="Professor" message="Yes Alice, see https://cs.edu/hw2" />
        </chat></recording>"#;
        let c = parse_chat_xml(chat);
        assert_eq!(c.messages.len(), 2);
        assert_eq!(c.links, vec!["https://cs.edu/hw2".to_string()]);
        assert_eq!(c.questions.len(), 1);
        assert_eq!(c.questions[0].sender, "Alice");
    }

    #[test]
    fn time_strings() {
        assert!((parse_time_string("01:30:00").unwrap() - 5400.0).abs() < 0.1);
        assert!((parse_time_string("1h 30m").unwrap() - 5400.0).abs() < 0.1);
        assert!((parse_time_string("90m").unwrap() - 5400.0).abs() < 0.1);
        assert!((parse_time_string("5400").unwrap() - 5400.0).abs() < 0.1);
        assert!((parse_time_string("45:00").unwrap() - 2700.0).abs() < 0.1);
        assert_eq!(parse_time_string(""), None);
    }

    #[test]
    fn dead_time_detection() {
        let slides = vec![
            Slide { start: 0.0, ..Default::default() },
            Slide { start: 1800.0, ..Default::default() },
            Slide { start: 6300.0, ..Default::default() },
        ];
        let chat = vec![ChatMessage { time_sec: 6500.0, ..Default::default() }];
        let (cut, has_dead, dead) = detect_lecture_end_time(18000.0, &slides, &chat, 600.0);
        assert!(has_dead);
        assert!((cut - 7100.0).abs() < 1.0);
        assert!(dead > 900.0);

        let (_, has_dead2, _) = detect_lecture_end_time(7800.0, &[
            Slide { start: 7000.0, ..Default::default() },
        ], &[ChatMessage { time_sec: 7100.0, ..Default::default() }], 600.0);
        assert!(!has_dead2);
    }

    #[test]
    fn safe_filename_strips_unsafe_chars() {
        assert_eq!(safe_filename("CS/101: Intro?", 100), "CS 101 Intro");
        assert_eq!(safe_filename("   ", 100), "Lecture");
    }
}
