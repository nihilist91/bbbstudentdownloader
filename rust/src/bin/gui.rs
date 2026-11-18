//! BBB StudyForge desktop app (egui). Build with: cargo build --release --features gui

// No console window behind the app on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use eframe::egui::{self, Color32, RichText};

use bbb_studyforge::engine::{default_output_dir, inspect_recording, process_recording, read_url_list, Options, RecordingInfo, Reporter};
use bbb_studyforge::ffmpeg::{auto_download_ffmpeg, find_ffmpeg};
use bbb_studyforge::parser::{format_duration, parse_time_string};
use bbb_studyforge::update::{check_for_update, NewRelease};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const ACCENT: Color32 = Color32::from_rgb(0x7C, 0x5C, 0xFF);
const OK: Color32 = Color32::from_rgb(0x34, 0xD3, 0x99);
const WARN: Color32 = Color32::from_rgb(0xFB, 0xBF, 0x24);
const ERR: Color32 = Color32::from_rgb(0xF8, 0x71, 0x71);

// ---------------------------------------------------------------------------
// Settings (remembered between runs in the user's config folder)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Settings {
    output_dir: String,
    save_video: bool,
    save_audio: bool,
    save_slides: bool,
    save_chat: bool,
    save_study_pack: bool,
    auto_trim: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir().display().to_string(),
            save_video: true,
            save_audio: true,
            save_slides: true,
            save_chat: true,
            save_study_pack: true,
            auto_trim: true,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("bbb-studyforge").join("settings.json"))
}

impl Settings {
    fn load() -> Self {
        let mut s = Self::default();
        let Some(v) = settings_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        else {
            return s;
        };
        if let Some(d) = v.get("output_dir").and_then(|x| x.as_str()).filter(|d| !d.is_empty()) {
            s.output_dir = d.to_string();
        }
        let flag = |k: &str, cur: bool| v.get(k).and_then(|x| x.as_bool()).unwrap_or(cur);
        s.save_video = flag("save_video", s.save_video);
        s.save_audio = flag("save_audio", s.save_audio);
        s.save_slides = flag("save_slides", s.save_slides);
        s.save_chat = flag("save_chat", s.save_chat);
        s.save_study_pack = flag("save_study_pack", s.save_study_pack);
        s.auto_trim = flag("auto_trim", s.auto_trim);
        s
    }

    fn save(&self) {
        let Some(p) = settings_path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let v = serde_json::json!({
            "output_dir": self.output_dir,
            "save_video": self.save_video,
            "save_audio": self.save_audio,
            "save_slides": self.save_slides,
            "save_chat": self.save_chat,
            "save_study_pack": self.save_study_pack,
            "auto_trim": self.auto_trim,
        });
        let _ = std::fs::write(p, serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}

// ---------------------------------------------------------------------------
// Worker → UI messages
// ---------------------------------------------------------------------------

enum Event {
    Log(String),
    Status(usize, Status),
    Progress(usize, f64),
    Stage(usize, String),
    Inspected(usize, Box<RecordingInfo>),
    QueueFinished,
    InspectFinished,
    FfmpegProgress(f64, String),
    FfmpegDone(Result<PathBuf, String>),
    UpdateAvailable(NewRelease),
}

#[derive(Clone, PartialEq)]
enum Status {
    Pending,
    Inspecting,
    Ready,
    Downloading,
    Done(PathBuf),
    Failed(String),
    Skipped,
}

struct Job {
    url: String,
    status: Status,
    info: Option<RecordingInfo>,
    progress: f64,
    stage: String,
}

/// Sends pipeline progress for one job to the UI thread.
struct GuiReporter {
    job: usize,
    tx: Sender<Event>,
    ctx: egui::Context,
}

impl Reporter for GuiReporter {
    fn log(&self, message: &str) {
        let _ = self.tx.send(Event::Log(format!("  • {message}")));
        self.ctx.request_repaint();
    }
    fn progress(&self, fraction: f64) {
        let _ = self.tx.send(Event::Progress(self.job, fraction));
        self.ctx.request_repaint();
    }
    fn stage(&self, number: usize, description: &str) {
        let _ = self.tx.send(Event::Stage(self.job, format!("[{}/7] {description}", number + 1)));
        self.ctx.request_repaint();
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

struct App {
    settings: Settings,
    url_input: String,
    cookie_on: bool,
    cookie: String,
    cut_input: String,
    jobs: Vec<Job>,
    log: Vec<String>,
    busy: bool,
    stop_flag: Arc<AtomicBool>,
    ffmpeg: Option<PathBuf>,
    ffmpeg_busy: Option<(f64, String)>,
    error_banner: Option<String>,
    update: Option<NewRelease>,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let (tx, rx) = channel();

        // Best-effort update check in the background; silent when offline or up to date.
        {
            let (tx, ctx) = (tx.clone(), cc.egui_ctx.clone());
            thread::spawn(move || {
                if let Some(release) = check_for_update(VERSION) {
                    let _ = tx.send(Event::UpdateAvailable(release));
                    ctx.request_repaint();
                }
            });
        }
        Self {
            settings: Settings::load(),
            url_input: String::new(),
            cookie_on: false,
            cookie: String::new(),
            cut_input: String::new(),
            jobs: Vec::new(),
            log: vec![format!("BBB StudyForge v{VERSION} ready. Paste one or more playback links, one per line.")],
            busy: false,
            stop_flag: Arc::new(AtomicBool::new(false)),
            ffmpeg: find_ffmpeg(),
            ffmpeg_busy: None,
            error_banner: None,
            update: None,
            tx,
            rx,
        }
    }

    fn cookie(&self) -> Option<String> {
        let c = self.cookie.trim();
        (self.cookie_on && !c.is_empty()).then(|| c.to_string())
    }

    /// Adds every URL in the input box to the queue (skipping duplicates), then clears the box.
    fn add_urls(&mut self) -> usize {
        let mut added = 0;
        for url in read_url_list(&self.url_input) {
            if !self.jobs.iter().any(|j| j.url == url) {
                self.jobs.push(Job { url, status: Status::Pending, info: None, progress: 0.0, stage: String::new() });
                added += 1;
            }
        }
        if added > 0 {
            self.url_input.clear();
        }
        added
    }

    fn options(&self) -> Result<Options, String> {
        let cut = if self.cut_input.trim().is_empty() {
            None
        } else {
            Some(parse_time_string(&self.cut_input).ok_or_else(|| format!("Could not read cut time \"{}\". Try 1h 45m, 01:45:00 or 6300.", self.cut_input.trim()))?)
        };
        let s = &self.settings;
        Ok(Options {
            save_video: s.save_video,
            save_audio: s.save_audio,
            save_slides: s.save_slides,
            save_chat: s.save_chat,
            save_study_pack: s.save_study_pack,
            auto_trim: s.auto_trim,
            custom_cut_seconds: cut,
        })
    }

    fn start_inspect(&mut self, ctx: &egui::Context) {
        self.add_urls();
        let targets: Vec<(usize, String)> = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| matches!(j.status, Status::Pending | Status::Failed(_)))
            .map(|(i, j)| (i, j.url.clone()))
            .collect();
        if targets.is_empty() {
            self.error_banner = Some("Nothing to inspect. Paste a playback link first.".into());
            return;
        }
        self.busy = true;
        let (tx, ctx, cookie) = (self.tx.clone(), ctx.clone(), self.cookie());
        thread::spawn(move || {
            for (i, url) in targets {
                let _ = tx.send(Event::Status(i, Status::Inspecting));
                ctx.request_repaint();
                match inspect_recording(&url, cookie.as_deref()) {
                    Ok(info) => {
                        let _ = tx.send(Event::Inspected(i, Box::new(info)));
                        let _ = tx.send(Event::Status(i, Status::Ready));
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Log(format!("❌ Inspect failed for {url}: {e}")));
                        let _ = tx.send(Event::Status(i, Status::Failed(e.to_string())));
                    }
                }
                ctx.request_repaint();
            }
            let _ = tx.send(Event::InspectFinished);
            ctx.request_repaint();
        });
    }

    fn start_downloads(&mut self, ctx: &egui::Context) {
        self.add_urls();
        let opts = match self.options() {
            Ok(o) => o,
            Err(e) => {
                self.error_banner = Some(e);
                return;
            }
        };
        let targets: Vec<(usize, String)> = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| !matches!(j.status, Status::Done(_)))
            .map(|(i, j)| (i, j.url.clone()))
            .collect();
        if targets.is_empty() {
            self.error_banner = Some("Queue is empty. Paste one or more playback links first.".into());
            return;
        }
        if self.ffmpeg.is_none() && (opts.save_audio || opts.save_video) {
            self.log.push("⚠️ FFmpeg not found: audio and video will be skipped. Use \"Install FFmpeg\".".into());
        }
        self.settings.save();
        self.busy = true;
        self.error_banner = None;
        self.stop_flag.store(false, Ordering::SeqCst);

        let out_dir = PathBuf::from(self.settings.output_dir.trim());
        let (tx, ctx, cookie, stop) = (self.tx.clone(), ctx.clone(), self.cookie(), self.stop_flag.clone());
        thread::spawn(move || {
            for (i, url) in targets {
                if stop.load(Ordering::SeqCst) {
                    let _ = tx.send(Event::Status(i, Status::Skipped));
                    continue;
                }
                let _ = tx.send(Event::Status(i, Status::Downloading));
                let _ = tx.send(Event::Log(format!("━━ {url}")));
                ctx.request_repaint();
                match inspect_recording(&url, cookie.as_deref()) {
                    Ok(info) => {
                        let _ = tx.send(Event::Inspected(i, Box::new(info)));
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Log(format!("❌ {e}")));
                        let _ = tx.send(Event::Status(i, Status::Failed(e.to_string())));
                        ctx.request_repaint();
                        continue;
                    }
                }
                let rep = GuiReporter { job: i, tx: tx.clone(), ctx: ctx.clone() };
                match process_recording(&url, &out_dir, &opts, cookie.as_deref(), &rep) {
                    Ok(path) => {
                        let _ = tx.send(Event::Log(format!("🎉 Saved to {}", path.display())));
                        let _ = tx.send(Event::Status(i, Status::Done(path)));
                    }
                    Err(e) => {
                        let _ = tx.send(Event::Log(format!("❌ {e}")));
                        let _ = tx.send(Event::Status(i, Status::Failed(e.to_string())));
                    }
                }
                ctx.request_repaint();
            }
            let _ = tx.send(Event::QueueFinished);
            ctx.request_repaint();
        });
    }

    fn start_ffmpeg_install(&mut self, ctx: &egui::Context) {
        let target = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        self.ffmpeg_busy = Some((0.0, "Starting...".into()));
        let (tx, ctx) = (self.tx.clone(), ctx.clone());
        thread::spawn(move || {
            let tx2 = tx.clone();
            let ctx2 = ctx.clone();
            let res = auto_download_ffmpeg(&target, &move |f, m| {
                let _ = tx2.send(Event::FfmpegProgress(f, m.to_string()));
                ctx2.request_repaint();
            });
            let _ = tx.send(Event::FfmpegDone(res.map_err(|e| e.to_string())));
            ctx.request_repaint();
        });
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                Event::Log(m) => self.log.push(m),
                Event::Status(i, s) => {
                    if let Some(j) = self.jobs.get_mut(i) {
                        if matches!(s, Status::Downloading) {
                            j.progress = 0.0;
                            j.stage.clear();
                        }
                        if matches!(s, Status::Done(_)) {
                            j.progress = 1.0;
                        }
                        j.status = s;
                    }
                }
                Event::Progress(i, p) => {
                    if let Some(j) = self.jobs.get_mut(i) {
                        j.progress = p;
                    }
                }
                Event::Stage(i, s) => {
                    self.log.push(s.clone());
                    if let Some(j) = self.jobs.get_mut(i) {
                        j.stage = s;
                    }
                }
                Event::Inspected(i, info) => {
                    if let Some(j) = self.jobs.get_mut(i) {
                        j.info = Some(*info);
                    }
                }
                Event::QueueFinished => {
                    self.busy = false;
                    let done = self.jobs.iter().filter(|j| matches!(j.status, Status::Done(_))).count();
                    let failed = self.jobs.iter().filter(|j| matches!(j.status, Status::Failed(_))).count();
                    self.log.push(format!("Queue finished: {done} done, {failed} failed."));
                }
                Event::InspectFinished => {
                    self.busy = false;
                    let ok = self.jobs.iter().filter(|j| matches!(j.status, Status::Ready)).count();
                    self.log.push(format!("Inspection finished: {ok} ready."));
                }
                Event::UpdateAvailable(r) => {
                    self.log.push(format!("A new version is available: {} ({})", r.version, r.url));
                    self.update = Some(r);
                }
                Event::FfmpegProgress(f, m) => self.ffmpeg_busy = Some((f, m)),
                Event::FfmpegDone(res) => {
                    self.ffmpeg_busy = None;
                    match res {
                        Ok(p) => {
                            self.log.push(format!("FFmpeg installed at {}", p.display()));
                            self.ffmpeg = find_ffmpeg().or(Some(p));
                        }
                        Err(e) => self.error_banner = Some(format!("FFmpeg install failed: {e}")),
                    }
                }
            }
        }
        if self.log.len() > 2000 {
            let excess = self.log.len() - 2000;
            self.log.drain(..excess);
        }
    }
}

fn apply_theme(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x0B, 0x10, 0x20);
    v.window_fill = Color32::from_rgb(0x14, 0x1B, 0x2D);
    v.extreme_bg_color = Color32::from_rgb(0x1C, 0x25, 0x40);
    v.faint_bg_color = Color32::from_rgb(0x14, 0x1B, 0x2D);
    v.selection.bg_fill = ACCENT;
    v.hyperlink_color = Color32::from_rgb(0x93, 0x79, 0xFF);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.set_theme(egui::Theme::Dark);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
    });
}

fn open_in_file_manager(path: &Path) {
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(all(unix, not(target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

fn card(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_rgb(0x14, 0x1B, 0x2D))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(0x2A, 0x36, 0x58)))
        .corner_radius(12.0)
        .inner_margin(14.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

impl eframe::App for App {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        egui::Panel::bottom("log_panel").resizable(true).default_size(170.0).show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Log").strong());
                if ui.small_button("Clear").clicked() {
                    self.log.clear();
                }
                if ui.small_button("Copy").clicked() {
                    ui.ctx().copy_text(self.log.join("\n"));
                }
            });
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for line in &self.log {
                    let color = if line.starts_with('❌') {
                        ERR
                    } else if line.starts_with('⚠') {
                        WARN
                    } else if line.starts_with('🎉') {
                        OK
                    } else {
                        Color32::from_rgb(0xC8, 0xD0, 0xE0)
                    };
                    ui.label(RichText::new(line).monospace().size(12.0).color(color));
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🎓 BBB StudyForge").size(22.0).strong());
                    ui.label(RichText::new(format!("v{VERSION}")).color(Color32::GRAY));
                });
                ui.label(RichText::new("Save lecture video, audio, slides, chat and an AI study pack from BigBlueButton recordings.").color(Color32::GRAY));
                ui.add_space(4.0);

                if let Some(release) = self.update.clone() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("⬆ Version {} is available.", release.version)).color(OK).strong());
                        ui.hyperlink_to("Download", &release.url);
                        if ui.small_button("🗙").on_hover_text("Dismiss").clicked() {
                            self.update = None;
                        }
                    });
                }

                if let Some(msg) = self.error_banner.clone() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("⚠ {msg}")).color(WARN));
                        if ui.small_button("🗙").clicked() {
                            self.error_banner = None;
                        }
                    });
                }

                // Links
                card(ui, |ui| {
                    ui.label(RichText::new("Recording links").strong());
                    let links_id = egui::Id::unique("recording_links_input");
                    let links_box = ui.add(
                        egui::TextEdit::multiline(&mut self.url_input)
                            .id(links_id)
                            .hint_text("https://.../playback/presentation/2.3/<meetingId>\nOne link per line. Paste with Ctrl+V, right-click, or the Paste button.")
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                    // Pastes clipboard text into the links box (it must have focus to receive the paste).
                    // Starts a new line first so a second link never joins the first.
                    // Also moves the cursor to the end, so a right-click never pastes into the middle of a link.
                    let paste_into_links = |ui: &egui::Ui, text: &mut String| {
                        if !text.is_empty() && !text.ends_with('\n') {
                            text.push('\n');
                        }
                        let ctx = ui.ctx();
                        let mut state = egui::text_edit::TextEditState::load(ctx, links_id).unwrap_or_default();
                        let end = egui::text::CCursor::new(egui::text::CharIndex(text.chars().count()));
                        state.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
                        state.store(ctx, links_id);
                        ui.memory_mut(|m| m.request_focus(links_id));
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                    };
                    links_box.context_menu(|ui| {
                        if ui.button("📋 Paste").clicked() {
                            paste_into_links(ui, &mut self.url_input);
                            ui.close();
                        }
                        if ui.button("🗙 Clear").clicked() {
                            self.url_input.clear();
                            ui.close();
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.busy, egui::Button::new("📋 Paste")).clicked() {
                            paste_into_links(ui, &mut self.url_input);
                        }
                        if ui.add_enabled(!self.busy, egui::Button::new("➕ Add to queue")).clicked() {
                            let n = self.add_urls();
                            if n == 0 {
                                self.error_banner = Some("No new links found in the box.".into());
                            }
                        }
                        if ui.add_enabled(!self.busy, egui::Button::new("🔍 Inspect")).clicked() {
                            self.start_inspect(&ctx);
                        }
                        if ui.button("📂 Load list...").clicked() {
                            if let Some(p) = rfd::FileDialog::new().add_filter("Text", &["txt"]).pick_file() {
                                match std::fs::read_to_string(&p) {
                                    Ok(t) => {
                                        self.url_input.push_str(&format!("\n{t}"));
                                        self.add_urls();
                                    }
                                    Err(e) => self.error_banner = Some(format!("Could not read {}: {e}", p.display())),
                                }
                            }
                        }
                    });
                    ui.checkbox(&mut self.cookie_on, "This recording needs a login (cookies)");
                    if self.cookie_on {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.cookie)
                                .password(true)
                                .hint_text("Paste your session cookie (name=value; ...)")
                                .desired_width(f32::INFINITY),
                        );
                    }
                });

                ui.add_space(6.0);

                // Options
                card(ui, |ui| {
                    ui.label(RichText::new("What to save").strong());
                    ui.horizontal_wrapped(|ui| {
                        let s = &mut self.settings;
                        ui.checkbox(&mut s.save_video, "🎬 Video");
                        ui.checkbox(&mut s.save_audio, "🎵 Audio");
                        ui.checkbox(&mut s.save_slides, "📑 Slides + PDF");
                        ui.checkbox(&mut s.save_chat, "💬 Chat");
                        ui.checkbox(&mut s.save_study_pack, "📝 AI study pack");
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.settings.auto_trim, "✂ Auto-trim dead air after class");
                        ui.label("Cut at:");
                        ui.add(egui::TextEdit::singleline(&mut self.cut_input).hint_text("e.g. 1h 45m").desired_width(110.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Save to:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.output_dir).desired_width(ui.available_width() - 190.0));
                        if ui.button("Browse...").clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_directory(&self.settings.output_dir).pick_folder() {
                                self.settings.output_dir = p.display().to_string();
                                self.settings.save();
                            }
                        }
                        if ui.button("Open").clicked() {
                            let p = PathBuf::from(self.settings.output_dir.trim());
                            let _ = std::fs::create_dir_all(&p);
                            open_in_file_manager(&p);
                        }
                    });
                    ui.horizontal(|ui| match (&self.ffmpeg, &self.ffmpeg_busy) {
                        (_, Some((f, msg))) => {
                            ui.add(egui::ProgressBar::new(*f as f32).text(msg.clone()).desired_width(360.0));
                        }
                        (Some(p), None) => {
                            ui.label(RichText::new("✔ FFmpeg found").color(OK)).on_hover_text(p.display().to_string());
                        }
                        (None, None) => {
                            ui.label(RichText::new("✖ FFmpeg not found: video and audio need it").color(WARN));
                            if ui.button("⬇ Install FFmpeg").clicked() {
                                self.start_ffmpeg_install(&ctx);
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // Queue
                card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Queue ({})", self.jobs.len())).strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if self.busy {
                                if ui.button("⏹ Stop after current").clicked() {
                                    self.stop_flag.store(true, Ordering::SeqCst);
                                    self.log.push("Stopping after the current recording...".into());
                                }
                                ui.spinner();
                            } else {
                                let start = egui::Button::new(RichText::new("🚀 Download all").strong().color(Color32::WHITE)).fill(ACCENT);
                                if ui.add(start).clicked() {
                                    self.start_downloads(&ctx);
                                }
                                if ui.button("Clear finished").clicked() {
                                    self.jobs.retain(|j| !matches!(j.status, Status::Done(_) | Status::Skipped));
                                }
                                if ui.button("Clear all").clicked() {
                                    self.jobs.clear();
                                }
                            }
                        });
                    });
                    if self.jobs.is_empty() {
                        ui.label(RichText::new("No recordings yet. Paste links above, then Inspect or Download all.").color(Color32::GRAY));
                    }
                    let mut remove: Option<usize> = None;
                    for (i, job) in self.jobs.iter().enumerate() {
                        ui.separator();
                        ui.horizontal(|ui| {
                            let (icon, color) = match &job.status {
                                Status::Pending => ("⏳ Pending", Color32::GRAY),
                                Status::Inspecting => ("🔍 Inspecting", ACCENT),
                                Status::Ready => ("✔ Ready", OK),
                                Status::Downloading => ("⬇ Working", ACCENT),
                                Status::Done(_) => ("✅ Done", OK),
                                Status::Failed(_) => ("❌ Failed", ERR),
                                Status::Skipped => ("⏭ Skipped", Color32::GRAY),
                            };
                            ui.label(RichText::new(icon).color(color).strong());
                            let title = match &job.info {
                                Some(info) => format!("{} · {}", info.course_name, info.name),
                                None => job.url.clone(),
                            };
                            ui.label(RichText::new(title).strong()).on_hover_text(&job.url);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if !self.busy && ui.small_button("🗙").on_hover_text("Remove").clicked() {
                                    remove = Some(i);
                                }
                                if let Status::Done(p) = &job.status {
                                    if ui.small_button("📂 Open folder").clicked() {
                                        open_in_file_manager(p);
                                    }
                                }
                            });
                        });
                        if let Some(info) = &job.info {
                            let mut line = format!(
                                "{} · {} · {} · Audio {} · Screen share {} · {} slides · {} chat",
                                info.presenter,
                                info.date_str,
                                info.duration_formatted,
                                yes_no(info.assets.has_audio),
                                yes_no(info.assets.has_deskshare),
                                info.slide_count,
                                info.chat_count
                            );
                            if info.has_dead_time {
                                line.push_str(&format!(" · ✂ {} dead air (cut at {})", info.dead_time_formatted, format_duration(info.detected_cut_seconds)));
                            }
                            ui.label(RichText::new(line).size(12.0).color(Color32::GRAY));
                        }
                        match &job.status {
                            Status::Downloading => {
                                ui.add(egui::ProgressBar::new(job.progress as f32).show_percentage().text(job.stage.clone()));
                            }
                            Status::Failed(e) => {
                                ui.label(RichText::new(e).size(12.0).color(ERR));
                            }
                            _ => {}
                        }
                    }
                    if let Some(i) = remove {
                        self.jobs.remove(i);
                    }
                });
                ui.add_space(8.0);
            });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.settings.save();
    }
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(format!("BBB StudyForge v{VERSION}"))
        .with_inner_size([860.0, 900.0])
        .with_min_inner_size([640.0, 600.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/app_icon.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    eframe::run_native("BBB StudyForge", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
