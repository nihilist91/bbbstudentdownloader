//! Port of core/engine.py: inspect a recording, then run the full download/convert/package pipeline.

use std::fs;
use std::path::{Path, PathBuf};

use crate::converter::{build_audio, build_deskshare_video, build_slide_sync_video, compile_slides_to_pdf, generate_ffmetadata_file};
use crate::downloader::{download_all_slides, fetch_file, fetch_text, probe_recording_assets};
use crate::ffmpeg::find_ffmpeg;
use crate::model::{Assets, Metadata, Slide};
use crate::parser::{detect_lecture_end_time, format_duration, parse_bbb_url, parse_chat_xml, parse_metadata, parse_presentation_text, parse_shapes_svg, safe_filename, ChatParse};
use crate::study_pack::{generate_ai_study_companion, generate_chat_markdown, generate_interactive_chat_html, generate_slide_deck_html};
use crate::{Error, Result};

/// What to produce. Defaults match the Python defaults (everything on, auto-trim on).
#[derive(Debug, Clone)]
pub struct Options {
    pub save_video: bool,
    pub save_audio: bool,
    pub save_slides: bool,
    pub save_chat: bool,
    pub save_study_pack: bool,
    pub auto_trim: bool,
    pub custom_cut_seconds: Option<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            save_video: true,
            save_audio: true,
            save_slides: true,
            save_chat: true,
            save_study_pack: true,
            auto_trim: true,
            custom_cut_seconds: None,
        }
    }
}

/// Receives progress from the pipeline. Implement only what you need.
pub trait Reporter {
    fn log(&self, _message: &str) {}
    /// Overall progress, clamped to 0.0..=1.0.
    fn progress(&self, _fraction: f64) {}
    /// Stage number (0..=6) and description.
    fn stage(&self, _number: usize, _description: &str) {}
}

pub struct NoopReporter;
impl Reporter for NoopReporter {}

#[derive(Debug, Clone)]
pub struct RecordingInfo {
    pub meeting_id: String,
    pub base_url: String,
    pub name: String,
    pub course_name: String,
    pub presenter: String,
    pub date_str: String,
    pub duration_seconds: f64,
    pub duration_formatted: String,
    pub participants: u64,
    pub assets: Assets,
    pub slide_count: usize,
    pub chat_count: usize,
    pub detected_cut_seconds: f64,
    pub has_dead_time: bool,
    pub dead_time_seconds: f64,
    pub dead_time_formatted: String,
}

/// Probes a recording: metadata, available tracks, slide and chat counts, and dead-time detection.
pub fn inspect_recording(url: &str, cookie: Option<&str>) -> Result<RecordingInfo> {
    let bbb = parse_bbb_url(url)?;
    let base = &bbb.base_url;
    let meta_xml = fetch_text(&format!("{base}/metadata.xml"), cookie, 10);
    let metadata = parse_metadata(&meta_xml, &bbb.meeting_id);
    let assets = probe_recording_assets(base, cookie);

    let slides = if assets.shapes_svg {
        parse_shapes_svg(&fetch_text(&format!("{base}/shapes.svg"), cookie, 10))
    } else {
        Vec::new()
    };

    let messages = if assets.chat_xml {
        parse_chat_xml(&fetch_text(&format!("{base}/chat.xml"), cookie, 8)).messages
    } else {
        Vec::new()
    };

    let (cut, has_dead, dead) = detect_lecture_end_time(metadata.duration_seconds, &slides, &messages, 600.0);

    Ok(RecordingInfo {
        meeting_id: bbb.meeting_id,
        base_url: bbb.base_url,
        name: non_empty_or(&metadata.name, "Untitled Lecture"),
        course_name: non_empty_or(&metadata.course_name, "Course"),
        presenter: non_empty_or(&metadata.presenter, "Instructor"),
        date_str: metadata.date_str,
        duration_seconds: metadata.duration_seconds,
        duration_formatted: metadata.duration_formatted,
        participants: metadata.participants,
        assets,
        slide_count: slides.len(),
        chat_count: messages.len(),
        detected_cut_seconds: cut,
        has_dead_time: has_dead,
        dead_time_seconds: dead,
        dead_time_formatted: format_duration(dead),
    })
}

fn non_empty_or(v: &str, fallback: &str) -> String {
    if v.is_empty() { fallback.to_string() } else { v.to_string() }
}

/// Runs the full pipeline and returns the final folder path.
/// On failure the staging folder is removed.
pub fn process_recording(
    url: &str,
    output_parent: &Path,
    opts: &Options,
    cookie: Option<&str>,
    reporter: &dyn Reporter,
) -> Result<PathBuf> {
    let bbb = parse_bbb_url(url)?;
    let work_dir = output_parent.join(format!("__work_{}", bbb.meeting_id));
    fs::create_dir_all(&work_dir)?;

    match run_pipeline(&bbb.base_url, &bbb.meeting_id, &work_dir, output_parent, opts, cookie, reporter) {
        Ok(path) => Ok(path),
        Err(e) => {
            let _ = fs::remove_dir_all(&work_dir);
            Err(e)
        }
    }
}

fn run_pipeline(
    base: &str,
    meeting_id: &str,
    work_dir: &Path,
    output_parent: &Path,
    opts: &Options,
    cookie: Option<&str>,
    rep: &dyn Reporter,
) -> Result<PathBuf> {
    let log = |m: &str| rep.log(m);
    let prog = |p: f64| rep.progress(p.clamp(0.0, 1.0));

    // Stage 0: metadata
    rep.stage(0, "Inspecting recording & fetching manifests...");
    log(&format!("Inspecting BigBlueButton session: {meeting_id}"));
    prog(0.05);

    let meta_xml = fetch_text(&format!("{base}/metadata.xml"), cookie, 15);
    let metadata: Metadata = parse_metadata(&meta_xml, meeting_id);

    let course_clean = safe_filename(&metadata.course_name, 100);
    let title_src = if metadata.name.is_empty() { "Lecture" } else { metadata.name.as_str() };
    let title_clean = safe_filename(title_src, 100);
    let folder_base = if course_clean != title_clean {
        format!("{course_clean} - {title_clean}")
    } else {
        course_clean.clone()
    };

    log(&format!("Course: {}", metadata.course_name));
    log(&format!("Session: {}", metadata.name));
    log(&format!("Duration: {} | Date: {}", metadata.duration_formatted, metadata.date_str));
    prog(0.12);

    // Stage 1: slide timeline, chat and slide text
    rep.stage(1, "Analyzing presentation slides & timeline...");
    let svg_text = fetch_text(&format!("{base}/shapes.svg"), cookie, 15);
    let slides: Vec<Slide> = if !svg_text.is_empty() {
        let s = parse_shapes_svg(&svg_text);
        log(&format!("Discovered {} presentation slides with timestamps.", s.len()));
        s
    } else {
        log("No shapes.svg found (session may have used only screen share).");
        Vec::new()
    };
    prog(0.20);

    let mut chat_xml = fetch_text(&format!("{base}/chat.xml"), cookie, 15);
    if chat_xml.is_empty() {
        chat_xml = fetch_text(&format!("{base}/slides_new.xml"), cookie, 15);
    }
    let chat: ChatParse = if !chat_xml.is_empty() {
        let c = parse_chat_xml(&chat_xml);
        log(&format!("Extracted {} chat messages ({} questions).", c.messages.len(), c.questions.len()));
        c
    } else {
        ChatParse { messages: Vec::new(), links: Vec::new(), questions: Vec::new() }
    };

    let pres_text_json = fetch_text(&format!("{base}/presentation_text.json"), cookie, 15);
    let slide_texts = parse_presentation_text(&pres_text_json);

    // Lecture end trimming
    let mut active_cut: Option<f64> = None;
    match opts.custom_cut_seconds {
        Some(c) if c > 0.0 => {
            active_cut = Some(c);
            log(&format!("✂️ Trimming lecture at user-specified cut point: {}", format_duration(c)));
        }
        _ if opts.auto_trim => {
            let (cut, has_dead, dead) = detect_lecture_end_time(metadata.duration_seconds, &slides, &chat.messages, 600.0);
            if has_dead {
                active_cut = Some(cut);
                log(&format!(
                    "✂️ Detected {} of inactive dead time after class ended. Auto-trimming to {}.",
                    format_duration(dead),
                    format_duration(cut)
                ));
            }
        }
        _ => {}
    }
    prog(0.28);

    // Stage 2: media downloads
    rep.stage(2, "Downloading audio & video streams...");
    let mut webcams_path = work_dir.join("webcams.webm");
    let mut deskshare_path = work_dir.join("deskshare.webm");

    let mut has_webcams = fetch_file(
        &format!("{base}/video/webcams.webm"),
        &webcams_path,
        cookie,
        Some(&|f: f64, _: u64, _: u64| prog(0.28 + f * 0.20)),
        60,
    );
    if !has_webcams {
        let mp4 = work_dir.join("webcams.mp4");
        if fetch_file(&format!("{base}/video/webcams.mp4"), &mp4, cookie, None, 60) {
            webcams_path = mp4;
            has_webcams = true;
        }
    }
    prog(0.48);

    let mut has_deskshare = fetch_file(
        &format!("{base}/deskshare/deskshare.webm"),
        &deskshare_path,
        cookie,
        Some(&|f: f64, _: u64, _: u64| prog(0.48 + f * 0.15)),
        60,
    );
    if !has_deskshare {
        let mp4 = work_dir.join("deskshare.mp4");
        if fetch_file(&format!("{base}/deskshare/deskshare.mp4"), &mp4, cookie, None, 60) {
            deskshare_path = mp4;
            has_deskshare = true;
        }
    }
    prog(0.63);

    if !has_webcams && !has_deskshare {
        return Err(Error::from(
            "No audio or video streams could be downloaded. \
             If this lecture requires university authentication, please provide session cookies.",
        ));
    }

    // Slide images
    let mut slide_objects: Vec<Slide> = Vec::new();
    if !slides.is_empty() && opts.save_slides {
        log(&format!("Downloading {} slide assets...", slides.len()));
        slide_objects = download_all_slides(base, &slides, work_dir, cookie, Some(&|f: f64, _: &str| prog(0.63 + f * 0.10)));
        for s in &mut slide_objects {
            s.text = slide_texts.get(&s.index.to_string()).cloned().unwrap_or_default();
        }
    }
    prog(0.73);

    // Stage 3: audio
    rep.stage(3, "Processing audio & ID3 tags...");
    let audio_dest = work_dir.join(format!("{folder_base} (audio).m4a"));
    let have_ffmpeg = find_ffmpeg().is_some();

    if opts.save_audio && has_webcams {
        if have_ffmpeg {
            log("Encoding high-quality AAC audio with metadata...");
            build_audio(&webcams_path, &audio_dest, &metadata, active_cut)?;
            log("Audio ready.");
        } else {
            log("FFmpeg missing; preserving raw audio stream.");
            fs::copy(&webcams_path, work_dir.join(format!("{folder_base}.webm")))?;
        }
    }
    prog(0.80);

    // Stage 4: video
    rep.stage(4, "Synthesizing lecture video & chapters...");
    let video_dest = work_dir.join(format!("{folder_base}.mp4"));

    let chapters_file = if !slides.is_empty() {
        let p = work_dir.join("chapters.txt");
        generate_ffmetadata_file(&metadata, &slides, &p, active_cut)?;
        Some(p)
    } else {
        None
    };

    if opts.save_video && have_ffmpeg {
        if has_deskshare {
            log("Building screen-share video with synchronized chapters...");
            build_deskshare_video(&deskshare_path, &webcams_path, &video_dest, chapters_file.as_deref(), active_cut)?;
            log("Deskshare video created.");
        } else if !slide_objects.is_empty() {
            log("No deskshare stream found. Synthesizing video from presentation slides...");
            if build_slide_sync_video(&slide_objects, &webcams_path, &video_dest, chapters_file.as_deref(), active_cut)? {
                log("Slide-synchronized video created successfully!");
            } else {
                log("Slide video synthesis skipped (slides are vector-only or incomplete).");
            }
        } else {
            log("No video track or slides available to construct video.");
        }
    }
    prog(0.88);

    // Stage 5: study pack
    rep.stage(5, "Generating study pack & transcripts...");

    if opts.save_chat && !chat.messages.is_empty() {
        fs::write(
            work_dir.join(format!("{folder_base} - Chat.md")),
            generate_chat_markdown(&metadata, &chat.messages, &chat.links),
        )?;
        fs::write(
            work_dir.join(format!("{folder_base} - Chat_Viewer.html")),
            generate_interactive_chat_html(&metadata, &chat.messages, &chat.links),
        )?;
        log("Interactive chat viewer generated.");
    }

    if opts.save_slides && !slide_objects.is_empty() {
        fs::write(
            work_dir.join(format!("{folder_base} - Slides_Viewer.html")),
            generate_slide_deck_html(&metadata, &slide_objects),
        )?;
        if compile_slides_to_pdf(&slide_objects, &work_dir.join(format!("{folder_base} - Slides.pdf")))? {
            log("Compiled presentation slides into PDF.");
        }
    }

    if opts.save_study_pack {
        let doc = generate_ai_study_companion(&metadata, &slides, &chat.messages, &chat.questions, &slide_texts);
        fs::write(work_dir.join(format!("{folder_base} - AI_Study_Companion.md")), doc)?;
        log("AI Study Companion generated.");
    }
    prog(0.95);

    // Stage 6: tidy up
    rep.stage(6, "Tidying up...");
    for temp in ["webcams.webm", "webcams.mp4", "deskshare.webm", "deskshare.mp4", "chapters.txt", "slides_concat.txt"] {
        let _ = fs::remove_file(work_dir.join(temp));
    }

    let mut final_dir = output_parent.join(&folder_base);
    let mut idx = 2;
    while final_dir.exists() {
        final_dir = output_parent.join(format!("{folder_base} ({idx})"));
        idx += 1;
    }
    fs::rename(work_dir, &final_dir)?;
    prog(1.0);
    log(&format!("All files saved to: {}", final_dir.display()));
    Ok(final_dir)
}
