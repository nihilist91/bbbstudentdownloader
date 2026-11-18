//! Port of core/converter.py: FFMETADATA chapters, audio/video encoding, slide-sync video, PDF.

use std::fs;
use std::path::{Path, PathBuf};

use crate::ffmpeg::{find_ffmpeg, run_ffmpeg};
use crate::model::{Metadata, Slide};
use crate::pdf;
use crate::Result;

/// Escapes a value for FFMETADATA1 (`=`, `;`, `#`, `\` and newlines need a backslash).
/// The Python version did not escape these, which corrupted titles containing them.
fn ff_escape(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '=' | ';' | '#' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Writes an FFMETADATA1 file with global tags and one chapter per slide, capped at `cut`.
pub fn generate_ffmetadata_file(meta: &Metadata, slides: &[Slide], output: &Path, cut: Option<f64>) -> Result<()> {
    let date: String = meta.date_str.chars().take(10).collect();
    let mut lines: Vec<String> = vec![
        ";FFMETADATA1".into(),
        format!("title={}", ff_escape(&meta.name)),
        format!("artist={}", ff_escape(&meta.presenter)),
        format!("album={}", ff_escape(&meta.course_name)),
        format!("date={}", ff_escape(&date)),
        String::new(),
    ];

    for (i, s) in slides.iter().enumerate() {
        let start_t = s.start;
        if let Some(c) = cut.filter(|c| *c != 0.0) {
            if start_t >= c {
                break;
            }
        }
        let idx = if s.index > 0 { s.index } else { i + 1 };
        let start_ms = (start_t * 1000.0) as i64;
        let mut raw_end = s.end;
        if let Some(c) = cut.filter(|c| *c != 0.0) {
            raw_end = raw_end.min(c);
        }
        let mut end_ms = (raw_end * 1000.0) as i64;
        if end_ms <= start_ms {
            end_ms = start_ms + 5000;
        }
        lines.extend([
            "[CHAPTER]".to_string(),
            "TIMEBASE=1/1000".to_string(),
            format!("START={start_ms}"),
            format!("END={end_ms}"),
            format!("title=Slide {idx}"),
            String::new(),
        ]);
    }

    fs::write(output, lines.join("\n"))?;
    Ok(())
}

fn base_codec_args(args: &mut Vec<String>) {
    args.extend(["-c:a", "aac", "-b:a", "128k"].map(String::from));
}

fn push_cut(args: &mut Vec<String>, cut: Option<f64>) {
    if let Some(c) = cut.filter(|c| *c > 0.0) {
        args.push("-t".into());
        args.push(c.to_string());
    }
}

/// Extracts webcam audio to AAC M4A with ID3-style tags. Returns true if the output exists.
pub fn build_audio(webcams: &Path, output: &Path, meta: &Metadata, cut: Option<f64>) -> Result<bool> {
    if !webcams.is_file() {
        return Ok(false);
    }
    let title = if meta.name.is_empty() { "Lecture" } else { &meta.name };
    let artist = if meta.presenter.is_empty() { "Instructor" } else { &meta.presenter };
    let album = if meta.course_name.is_empty() { "Course" } else { &meta.course_name };
    let year: String = meta.date_str.chars().take(4).collect();

    let mut args: Vec<String> = vec!["-i".into(), webcams.display().to_string(), "-vn".into()];
    base_codec_args(&mut args);
    for (k, v) in [("title", title), ("artist", artist), ("album", album)] {
        args.push("-metadata".into());
        args.push(format!("{k}={v}"));
    }
    if !year.is_empty() && year.bytes().all(|b| b.is_ascii_digit()) {
        args.push("-metadata".into());
        args.push(format!("date={year}"));
    }
    push_cut(&mut args, cut);
    args.push(output.display().to_string());
    run_ffmpeg(&args)?;
    Ok(output.is_file())
}

/// Combines deskshare video with webcam audio and optional chapter metadata.
pub fn build_deskshare_video(
    deskshare: &Path,
    webcams: &Path,
    output: &Path,
    metadata_file: Option<&Path>,
    cut: Option<f64>,
) -> Result<bool> {
    if !deskshare.is_file() || !webcams.is_file() {
        return Ok(false);
    }
    let mut args: Vec<String> = vec![
        "-i".into(), deskshare.display().to_string(),
        "-i".into(), webcams.display().to_string(),
    ];
    if let Some(m) = metadata_file.filter(|p| p.is_file()) {
        args.extend(["-i".into(), m.display().to_string(), "-map_metadata".into(), "2".into()]);
    }
    args.extend(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "libx264", "-preset", "veryfast", "-crf", "22", "-pix_fmt", "yuv420p"].map(String::from));
    base_codec_args(&mut args);
    args.extend(["-shortest", "-movflags", "+faststart"].map(String::from));
    push_cut(&mut args, cut);
    args.push(output.display().to_string());
    run_ffmpeg(&args)?;
    Ok(output.is_file())
}

/// Single-quoted path for the ffconcat format (embedded quotes become `'\''`).
fn concat_quote(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
}

/// Builds a 1080p video from slide images synced to the webcam audio.
pub fn build_slide_sync_video(
    slides: &[Slide],
    webcams: &Path,
    output: &Path,
    metadata_file: Option<&Path>,
    cut: Option<f64>,
) -> Result<bool> {
    let valid: Vec<&Slide> = slides
        .iter()
        .filter(|s| s.local_path.as_ref().map(|p| p.is_file()).unwrap_or(false))
        .collect();
    if valid.is_empty() || !webcams.is_file() {
        return Ok(false);
    }
    let is_raster = |s: &&Slide| {
        let p = s.local_path.as_ref().map(|p| p.display().to_string().to_lowercase()).unwrap_or_default();
        p.ends_with(".png") || p.ends_with(".jpg") || p.ends_with(".jpeg")
    };
    let mut imgs: Vec<&Slide> = valid.iter().copied().filter(|s| is_raster(s)).collect();
    if imgs.is_empty() {
        return Ok(false);
    }
    if let Some(c) = cut.filter(|c| *c > 0.0) {
        imgs.retain(|s| s.start < c);
        if imgs.is_empty() {
            imgs = vec![valid[0]];
        }
    }

    let concat_path = output.parent().unwrap_or(Path::new(".")).join("slides_concat.txt");
    let mut body = String::from("ffconcat version 1.0\n");
    for s in &imgs {
        let mut duration = s.duration;
        if let Some(c) = cut.filter(|c| *c > 0.0) {
            if s.start + duration > c {
                duration = (c - s.start).max(1.0);
            }
        }
        let path = s.local_path.as_ref().expect("filtered to Some");
        body.push_str(&format!("file {}\nduration {}\n", concat_quote(path), duration));
    }
    let last = imgs.last().and_then(|s| s.local_path.as_ref()).expect("non-empty");
    body.push_str(&format!("file {}\n", concat_quote(last)));
    fs::write(&concat_path, body)?;

    let mut args: Vec<String> = vec![
        "-f".into(), "concat".into(), "-safe".into(), "0".into(),
        "-i".into(), concat_path.display().to_string(),
        "-i".into(), webcams.display().to_string(),
    ];
    if let Some(m) = metadata_file.filter(|p| p.is_file()) {
        args.extend(["-i".into(), m.display().to_string(), "-map_metadata".into(), "2".into()]);
    }
    args.extend(["-map", "0:v:0", "-map", "1:a:0", "-c:v", "libx264", "-preset", "ultrafast", "-crf", "24", "-pix_fmt", "yuv420p"].map(String::from));
    base_codec_args(&mut args);
    args.extend(["-shortest", "-movflags", "+faststart"].map(String::from));
    push_cut(&mut args, cut);
    args.push(output.display().to_string());

    let result = run_ffmpeg(&args);
    let _ = fs::remove_file(&concat_path);
    result?;
    Ok(output.is_file())
}

/// Compiles slide images into one multi-page PDF (JPEG q90 pages at 72 dpi).
/// Returns true when the PDF was written and is larger than 1 KB.
pub fn compile_slides_to_pdf(slides: &[Slide], output_pdf: &Path) -> Result<bool> {
    let paths: Vec<PathBuf> = slides
        .iter()
        .filter_map(|s| s.local_path.clone())
        .filter(|p| p.is_file())
        .collect();
    if paths.is_empty() {
        return Ok(false);
    }

    let mut pages = Vec::new();
    for p in &paths {
        let lower = p.display().to_string().to_lowercase();
        if !(lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".webp")) {
            continue;
        }
        if let Ok(img) = image::open(p) {
            pages.push(img.to_rgb8());
        }
    }
    if pages.is_empty() {
        return Ok(false);
    }

    pdf::write_jpeg_pdf(&pages, output_pdf, 90)?;
    Ok(fs::metadata(output_pdf).map(|m| m.len() > 1000).unwrap_or(false))
}

/// True when ffmpeg is installed, so the converters above can run.
pub fn ffmpeg_available() -> bool {
    find_ffmpeg().is_some()
}
