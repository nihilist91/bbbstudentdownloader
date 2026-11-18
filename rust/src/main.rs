//! Command-line entry point for BBB StudyForge (Rust port).

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use bbb_studyforge::engine::{inspect_recording, process_recording, Options, Reporter};
use bbb_studyforge::ffmpeg::{auto_download_ffmpeg, find_ffmpeg};
use bbb_studyforge::parser::parse_time_string;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "bbb-studyforge",
    version = VERSION,
    about = "BigBlueButton lecture archiver & AI study suite"
)]
struct Cli {
    /// Recording playback URL to download
    #[arg(long = "cli", value_name = "URL")]
    url: Option<String>,

    /// Output folder (default: ~/Downloads/Lectures)
    #[arg(short, long, value_name = "DIR")]
    output: Option<PathBuf>,

    /// Session cookie string for recordings that need a login
    #[arg(short, long, value_name = "COOKIE")]
    cookie: Option<String>,

    /// Only inspect the recording and print what is available
    #[arg(long)]
    inspect: bool,

    /// Cut the lecture at this time (e.g. "1h 45m", "01:45:00", "5400")
    #[arg(long, value_name = "TIME")]
    cut: Option<String>,

    /// Disable automatic dead-time trimming
    #[arg(long)]
    no_trim: bool,

    /// Skip the MP4 video
    #[arg(long)]
    no_video: bool,

    /// Skip the M4A audio
    #[arg(long)]
    no_audio: bool,

    /// Skip slide images, slide viewer and PDF
    #[arg(long)]
    no_slides: bool,

    /// Skip chat transcript and chat viewer
    #[arg(long)]
    no_chat: bool,

    /// Skip the AI study companion
    #[arg(long)]
    no_study_pack: bool,

    /// Download a static ffmpeg build into ./ffmpeg and exit
    #[arg(long)]
    download_ffmpeg: bool,
}

struct CliReporter;

impl Reporter for CliReporter {
    fn log(&self, message: &str) {
        println!("  • {message}");
    }

    fn progress(&self, fraction: f64) {
        let pct = (fraction * 100.0) as usize;
        print!("\r  Progress: [{pct:3}%] {:<25}", "#".repeat(pct / 4));
        let _ = io::stdout().flush();
    }

    fn stage(&self, number: usize, description: &str) {
        println!("\n[{}/7] {description}", number + 1);
    }
}

fn default_output_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Downloads").join("Lectures")
}

fn run(cli: Cli) -> bbb_studyforge::Result<()> {
    if cli.download_ffmpeg {
        let dir = std::env::current_dir()?;
        let bin = auto_download_ffmpeg(&dir, &|frac, msg| println!("  [{:3.0}%] {msg}", frac * 100.0))?;
        println!("ffmpeg installed at {}", bin.display());
        return Ok(());
    }

    let url = cli.url.clone().ok_or(
        "No URL given. Use --cli <URL>. The desktop GUI is not part of the Rust port yet.",
    )?;
    let out_dir = cli.output.clone().unwrap_or_else(default_output_dir);
    let cookie = cli.cookie.as_deref().filter(|c| !c.is_empty());

    println!("\n🚀 BBB StudyForge CLI v{VERSION}");
    println!("Target URL: {url}");
    println!("Output Directory: {}\n", out_dir.display());

    println!("🔍 Inspecting recording...");
    let info = inspect_recording(&url, cookie)?;
    println!("   Course:    {}", info.course_name);
    println!("   Session:   {}", info.name);
    println!("   Presenter: {}", info.presenter);
    println!("   Duration:  {} | Date: {}", info.duration_formatted, info.date_str);
    println!(
        "   Tracks:    Audio: {} | Deskshare: {} | Slides: {} | Chat: {}\n",
        info.assets.has_audio,
        info.assets.has_deskshare,
        info.slide_count,
        info.chat_count
    );
    if info.has_dead_time {
        println!(
            "   Dead air:  {} after class (auto-trim would cut at {})\n",
            info.dead_time_formatted,
            bbb_studyforge::parser::format_duration(info.detected_cut_seconds)
        );
    }
    if cli.inspect {
        return Ok(());
    }

    let custom_cut = match cli.cut.as_deref() {
        Some(raw) => Some(parse_time_string(raw).ok_or_else(|| format!("Could not parse cut time: {raw}"))?),
        None => None,
    };
    let opts = Options {
        save_video: !cli.no_video,
        save_audio: !cli.no_audio,
        save_slides: !cli.no_slides,
        save_chat: !cli.no_chat,
        save_study_pack: !cli.no_study_pack,
        auto_trim: !cli.no_trim,
        custom_cut_seconds: custom_cut,
    };
    if find_ffmpeg().is_none() {
        println!("⚠️  FFmpeg not found: audio and video will be skipped. Run with --download-ffmpeg or install ffmpeg.\n");
    }

    let result = process_recording(&url, &out_dir, &opts, cookie, &CliReporter)?;
    println!("\n\n🎉 Done! All files saved to:\n   {}\n", result.display());
    Ok(())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("\n❌ Error: {e}\n");
            ExitCode::FAILURE
        }
    }
}
