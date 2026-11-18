//! Port of core/downloader.py: resilient HTTP helpers for BigBlueButton assets.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::model::{Assets, Slide};
use crate::Result;

pub const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// An asset key and the candidate URLs that prove it exists.
type AssetCheck = (&'static str, Vec<String>);

/// Progress callback: (fraction 0..1, bytes_done, bytes_total).
pub type FileProgress<'a> = &'a dyn Fn(f64, u64, u64);
/// Slide progress callback: (fraction 0..1, status message).
pub type SlideProgress<'a> = &'a dyn Fn(f64, &str);

/// `timeout_secs` limits connecting and each socket read, like Python's urllib timeout.
/// It is not a deadline for the whole download, so large lecture videos are not cut off.
fn request(url: &str, cookie: Option<&str>, timeout_secs: u64) -> ureq::Request {
    let t = Duration::from_secs(timeout_secs);
    let agent = ureq::AgentBuilder::new().timeout_connect(t).timeout_read(t).timeout_write(t).build();
    let mut req = agent
        .get(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "*/*")
        .set("Accept-Encoding", "identity");
    if let Some(c) = cookie.filter(|c| !c.is_empty()) {
        req = req.set("Cookie", c);
    }
    req
}

/// Decodes UTF-8 and silently drops invalid bytes (Python's `errors="ignore"`).
pub fn decode_text(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        out.push_str(chunk.valid());
    }
    out
}

/// Fetches text (XML, JSON, SVG). Returns "" on any error, like the Python version.
pub fn fetch_text(url: &str, cookie: Option<&str>, timeout_secs: u64) -> String {
    let Ok(resp) = request(url, cookie, timeout_secs).call() else {
        return String::new();
    };
    let mut buf = Vec::new();
    match resp.into_reader().read_to_end(&mut buf) {
        Ok(_) => decode_text(&buf),
        Err(_) => String::new(),
    }
}

/// Checks whether an asset exists, using a range request first and a short GET as fallback.
pub fn check_asset_exists(url: &str, cookie: Option<&str>) -> bool {
    match request(url, cookie, 8).set("Range", "bytes=0-1").call() {
        Ok(resp) => matches!(resp.status(), 200 | 206),
        Err(ureq::Error::Status(code, _)) => code == 200 || code == 206,
        Err(_) => match request(url, cookie, 8).call() {
            Ok(resp) => {
                let mut probe = [0u8; 128];
                resp.into_reader().read(&mut probe).is_ok()
            }
            Err(_) => false,
        },
    }
}

/// Fills `buf` completely unless the stream ends, so progress fires once per 1 MB chunk.
fn read_full(reader: &mut dyn Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = reader.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}

fn download_inner(url: &str, temp: &Path, cookie: Option<&str>, progress: Option<FileProgress>, timeout_secs: u64) -> Result<()> {
    let resp = request(url, cookie, timeout_secs).call()?;
    let total: u64 = resp
        .header("Content-Length")
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut file = File::create(temp)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done: u64 = 0;
    loop {
        let n = read_full(&mut *reader, &mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if let Some(cb) = progress {
            let frac = if total > 0 { done as f64 / total as f64 } else { 0.5 };
            cb(frac.min(1.0), done, total);
        }
    }
    file.flush()?;
    Ok(())
}

/// Streams a binary file to `dest` via a `.part` file. Returns false on any failure.
pub fn fetch_file(url: &str, dest: &Path, cookie: Option<&str>, progress: Option<FileProgress>, timeout_secs: u64) -> bool {
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let temp = PathBuf::from(format!("{}.part", dest.display()));
    let result = download_inner(url, &temp, cookie, progress, timeout_secs).and_then(|_| {
        if dest.exists() {
            fs::remove_file(dest)?;
        }
        fs::rename(&temp, dest)?;
        Ok(())
    });
    match result {
        Ok(()) => true,
        Err(_) => {
            let _ = fs::remove_file(&temp);
            false
        }
    }
}

/// Probes every candidate asset in parallel and returns which ones exist.
pub fn probe_recording_assets(base_url: &str, cookie: Option<&str>) -> Assets {
    let b = base_url;
    let checks: Vec<AssetCheck> = vec![
        ("metadata", vec![format!("{b}/metadata.xml")]),
        ("webcams_webm", vec![format!("{b}/video/webcams.webm")]),
        ("webcams_mp4", vec![format!("{b}/video/webcams.mp4")]),
        ("deskshare_webm", vec![format!("{b}/deskshare/deskshare.webm")]),
        ("deskshare_mp4", vec![format!("{b}/deskshare/deskshare.mp4")]),
        ("shapes_svg", vec![format!("{b}/shapes.svg")]),
        ("chat_xml", vec![format!("{b}/chat.xml")]),
        ("slides_new_xml", vec![format!("{b}/slides_new.xml")]),
        ("presentation_text_json", vec![format!("{b}/presentation_text.json")]),
        ("notes_html", vec![format!("{b}/notes.html"), format!("{b}/notes/notes.html")]),
        ("cursor_xml", vec![format!("{b}/cursor.xml")]),
        ("panzooms_xml", vec![format!("{b}/panzooms.xml")]),
    ];

    let results: HashMap<&str, bool> = std::thread::scope(|scope| {
        let handles: Vec<_> = checks
            .iter()
            .map(|(key, urls)| {
                (*key, scope.spawn(move || urls.iter().any(|u| check_asset_exists(u, cookie))))
            })
            .collect();
        handles
            .into_iter()
            .map(|(key, h)| (key, h.join().unwrap_or(false)))
            .collect()
    });
    let got = |k: &str| results.get(k).copied().unwrap_or(false);

    let mut a = Assets {
        metadata: got("metadata"),
        webcams_webm: got("webcams_webm"),
        webcams_mp4: got("webcams_mp4"),
        deskshare_webm: got("deskshare_webm"),
        deskshare_mp4: got("deskshare_mp4"),
        shapes_svg: got("shapes_svg"),
        chat_xml: got("chat_xml"),
        slides_new_xml: got("slides_new_xml"),
        presentation_text_json: got("presentation_text_json"),
        notes_html: got("notes_html"),
        cursor_xml: got("cursor_xml"),
        panzooms_xml: got("panzooms_xml"),
        ..Default::default()
    };
    a.has_audio = a.webcams_webm || a.webcams_mp4;
    a.has_deskshare = a.deskshare_webm || a.deskshare_mp4;
    a.has_slides = a.shapes_svg;
    a.has_chat = a.chat_xml || a.slides_new_xml;
    a
}

/// Extension of the last path component, or ".png" when there is none (Python's `splitext(href)[1] or ".png"`).
fn url_ext_or_png(href: &str) -> String {
    let last = href.rsplit('/').next().unwrap_or(href);
    match last.rfind('.') {
        Some(i) if i > 0 => last[i..].to_string(),
        _ => ".png".to_string(),
    }
}

/// Removes the last four characters (Python's `s[:-4]`).
fn chop4(s: &str) -> &str {
    let n = s.chars().count().saturating_sub(4);
    match s.char_indices().nth(n) {
        Some((i, _)) => &s[..i],
        None => "",
    }
}

/// Downloads every slide image into `out_dir/slides`, falling back to the png/svg counterpart.
pub fn download_all_slides(
    base_url: &str,
    slides: &[Slide],
    out_dir: &Path,
    cookie: Option<&str>,
    progress: Option<SlideProgress>,
) -> Vec<Slide> {
    let slides_dir = out_dir.join("slides");
    let _ = fs::create_dir_all(&slides_dir);
    let total = slides.len();
    let mut updated = Vec::with_capacity(total);

    for (i, s) in slides.iter().enumerate() {
        let href = s.href.trim_start_matches('/');
        let full_url = if href.starts_with("http") {
            href.to_string()
        } else {
            format!("{base_url}/{href}")
        };
        let ext = url_ext_or_png(href);
        let mut local_name = format!("slide_{:03}{}", s.index, ext);
        let mut dest = slides_dir.join(&local_name);

        if !dest.exists() && !fetch_file(&full_url, &dest, cookie, None, 60) {
            let (alt_ext, alt_url) = match ext.as_str() {
                ".png" => (".svg", format!("{}.svg", chop4(&full_url))),
                ".svg" => (".png", format!("{}.png", chop4(&full_url))),
                _ => ("", String::new()),
            };
            if !alt_ext.is_empty() {
                let alt_name = format!("slide_{:03}{}", s.index, alt_ext);
                let alt_dest = slides_dir.join(&alt_name);
                if fetch_file(&alt_url, &alt_dest, cookie, None, 60) {
                    dest = alt_dest;
                    local_name = alt_name;
                }
            }
        }

        let mut copy = s.clone();
        copy.local_path = Some(dest);
        copy.local_filename = Some(format!("slides/{local_name}"));
        updated.push(copy);

        if let Some(cb) = progress {
            cb((i + 1) as f64 / total as f64, &format!("Downloaded slide {}/{}", i + 1, total));
        }
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_and_chop_helpers() {
        assert_eq!(url_ext_or_png("presentation/p/slide-1.svg"), ".svg");
        assert_eq!(url_ext_or_png("presentation/p.v2/slide"), ".png");
        assert_eq!(chop4("https://x/slide-1.png"), "https://x/slide-1");
        assert_eq!(chop4("ab"), "");
    }

    #[test]
    fn decode_drops_invalid_bytes() {
        assert_eq!(decode_text(b"ok\xFFdone"), "okdone");
    }
}
