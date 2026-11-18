//! Command-line entry point for BBB StudyForge (Rust port).

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use bbb_studyforge::engine::{default_output_dir, inspect_recording, process_recording, read_url_list, Options, Reporter};
use bbb_studyforge::ffmpeg::{auto_download_ffmpeg, find_ffmpeg};
use bbb_studyforge::parser::{format_duration, parse_time_string};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(
    name = "bbb-studyforge",
    version = VERSION,
    about = "BigBlueButton lecture archiver & AI study suite",
    after_help = "Examples:\n  bbb-studyforge --cli <URL> -o .\\Lectures\n  bbb-studyforge <URL1> <URL2> <URL3>\n  bbb-studyforge --batch semester.txt -o .\\Lectures"
)]
struct Cli {
    /// Recording playback URLs to download (positional; any number)
    #[arg(value_name = "URL")]
    urls: Vec<String>,

    /// Recording playback URL (kept for compatibility with the Python CLI; repeatable)
    #[arg(long = "cli", value_name = "URL")]
    cli_urls: Vec<String>,

    /// Text file with one URL per line (blank lines and # comments are ignored)
    #[arg(short, long, value_name = "FILE")]
    batch: Option<PathBuf>,

    /// Output folder (default: ~/Downloads/Lectures)
    #[arg(short, long, value_name = "DIR")]
    output: Option<PathBuf>,

    /// Session cookie string for recordings that need a login
    #[arg(short, long, value_name = "COOKIE")]
    cookie: Option<String>,

    /// Only inspect the recordings and print what is available
    #[arg(long)]
    inspect: bool,

    /// Cut every lecture at this time (e.g. "1h 45m", "01:45:00", "5400")
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

fn collect_urls(cli: &Cli) -> bbb_studyforge::Result<Vec<String>> {
    let mut urls: Vec<String> = cli.cli_urls.iter().chain(cli.urls.iter()).cloned().collect();
    if let Some(path) = &cli.batch {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Could not read batch file {}: {e}", path.display()))?;
        urls.extend(read_url_list(&text));
    }
    let mut seen = std::collections::HashSet::new();
    urls.retain(|u| seen.insert(u.clone()));
    Ok(urls)
}

/// Inspects and (unless `--inspect`) downloads one recording.
fn run_one(url: &str, cli: &Cli, opts: &Options, out_dir: &std::path::Path) -> bbb_studyforge::Result<Option<PathBuf>> {
    let cookie = cli.cookie.as_deref().filter(|c| !c.is_empty());
    println!("🔍 Inspecting {url}");
    let info = inspect_recording(url, cookie)?;
    println!("   Course:    {}", info.course_name);
    println!("   Session:   {}", info.name);
    println!("   Presenter: {}", info.presenter);
    println!("   Duration:  {} | Date: {}", info.duration_formatted, info.date_str);
    println!(
        "   Tracks:    Audio: {} | Deskshare: {} | Slides: {} | Chat: {}",
        info.assets.has_audio, info.assets.has_deskshare, info.slide_count, info.chat_count
    );
    if info.has_dead_time {
        println!(
            "   Dead air:  {} after class (auto-trim would cut at {})",
            info.dead_time_formatted,
            format_duration(info.detected_cut_seconds)
        );
    }
    println!();
    if cli.inspect {
        return Ok(None);
    }
    let result = process_recording(url, out_dir, opts, cookie, &CliReporter)?;
    println!("\n\n🎉 Saved to:\n   {}\n", result.display());
    Ok(Some(result))
}

fn run(cli: Cli) -> bbb_studyforge::Result<bool> {
    if cli.download_ffmpeg {
        let dir = std::env::current_dir()?;
        let bin = auto_download_ffmpeg(&dir, &|frac, msg| println!("  [{:3.0}%] {msg}", frac * 100.0))?;
        println!("ffmpeg installed at {}", bin.display());
        return Ok(true);
    }

    let urls = collect_urls(&cli)?;
    if urls.is_empty() {
        return Err("No URL given. Pass one or more URLs, use --cli <URL>, or --batch <file>.".into());
    }
    let out_dir = cli.output.clone().unwrap_or_else(default_output_dir);

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

    println!("\n🚀 BBB StudyForge CLI v{VERSION}");
    println!("Recordings: {}", urls.len());
    println!("Output Directory: {}\n", out_dir.display());
    if !cli.inspect && find_ffmpeg().is_none() {
        println!("⚠️  FFmpeg not found: audio and video will be skipped. Run with --download-ffmpeg or install ffmpeg.\n");
    }

    // Single URL: behave exactly like before, errors go straight to the caller.
    if urls.len() == 1 {
        run_one(&urls[0], &cli, &opts, &out_dir)?;
        return Ok(true);
    }

    // Batch: keep going past failures and summarize at the end.
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut saved: Vec<PathBuf> = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        println!("━━━━━━━━ Recording {}/{} ━━━━━━━━", i + 1, urls.len());
        match run_one(url, &cli, &opts, &out_dir) {
            Ok(Some(path)) => saved.push(path),
            Ok(None) => {}
            Err(e) => {
                eprintln!("\n❌ Failed: {e}\n");
                failures.push((url.clone(), e.to_string()));
            }
        }
    }

    println!("━━━━━━━━ Batch summary ━━━━━━━━");
    println!("  Succeeded: {}", urls.len() - failures.len());
    for p in &saved {
        println!("    ✅ {}", p.display());
    }
    println!("  Failed:    {}", failures.len());
    for (url, err) in &failures {
        println!("    ❌ {url}\n       {err}");
    }
    Ok(failures.is_empty())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("\n❌ Error: {e}\n");
            ExitCode::FAILURE
        }
    }
}
