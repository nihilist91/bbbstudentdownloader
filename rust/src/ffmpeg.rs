//! Port of core/ffmpeg_helper.py: locate ffmpeg, run it, and optionally download a static build.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Error, Result};

fn exe_name(base: &str) -> String {
    if cfg!(windows) { format!("{base}.exe") } else { base.to_string() }
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.is_file() && fs::metadata(p).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// Finds ffmpeg next to the executable, in an `ffmpeg/` subfolder, or on PATH.
pub fn find_ffmpeg() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidates.push(dir.join(exe_name("ffmpeg")));
        candidates.push(dir.join("ffmpeg").join(exe_name("ffmpeg")));
    }
    candidates.push(Path::new("ffmpeg").join(exe_name("ffmpeg")));

    if let Some(found) = candidates.into_iter().find(|c| is_executable(c)) {
        return Some(found);
    }

    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(exe_name("ffmpeg")))
        .find(|c| is_executable(c))
}

/// Runs ffmpeg with quiet logging. Errors carry the last 400 characters of stderr.
pub fn run_ffmpeg(args: &[String]) -> Result<()> {
    let bin = find_ffmpeg().ok_or(
        "FFmpeg converter is not available. Please install FFmpeg or use the auto-install option.",
    )?;

    let mut cmd = Command::new(bin);
    cmd.args(["-y", "-hide_banner", "-loglevel", "error"]).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let out = cmd.output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let trimmed = stderr.trim();
        let msg = if trimmed.is_empty() {
            "FFmpeg process failed with non-zero exit code".to_string()
        } else {
            let tail: String = {
                let chars: Vec<char> = trimmed.chars().collect();
                chars[chars.len().saturating_sub(400)..].iter().collect()
            };
            tail
        };
        return Err(format!("FFmpeg conversion error: {msg}").into());
    }
    Ok(())
}

/// Streams a URL into `dest`, reporting (fraction, label) like the Python helper.
fn download_with_progress(url: &str, dest: &Path, progress: &dyn Fn(f64, &str)) -> Result<()> {
    let resp = ureq::get(url)
        .set("User-Agent", "Mozilla/5.0")
        .timeout(std::time::Duration::from_secs(600))
        .call()
        .map_err(|e| Error::from(format!("FFmpeg download failed: {e}")))?;
    let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut reader = resp.into_reader();
    let mut file = fs::File::create(dest)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut done: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        done += n as u64;
        if total > 0 {
            progress(
                0.1 + 0.7 * (done as f64 / total as f64),
                &format!("Downloading FFmpeg... {}MB / {}MB", done >> 20, total >> 20),
            );
        }
    }
    Ok(())
}

/// Downloads a standalone ffmpeg into `<target_dir>/ffmpeg/`.
/// Windows: Gyan.dev essentials zip. Linux: John Van Sickle static tar.xz.
pub fn auto_download_ffmpeg(target_dir: &Path, progress: &dyn Fn(f64, &str)) -> Result<PathBuf> {
    let dest_dir = target_dir.join("ffmpeg");
    fs::create_dir_all(&dest_dir)?;
    progress(0.1, "Connecting to FFmpeg repository...");

    if cfg!(windows) {
        let url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip";
        let zip_path = target_dir.join("ffmpeg_download.zip");
        download_with_progress(url, &zip_path, progress)?;
        progress(0.85, "Extracting FFmpeg binary...");
        let file = fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)?;
        let target = dest_dir.join("ffmpeg.exe");
        let mut found = false;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            if entry.name().ends_with("ffmpeg.exe") {
                let mut out = fs::File::create(&target)?;
                std::io::copy(&mut entry, &mut out)?;
                found = true;
                break;
            }
        }
        let _ = fs::remove_file(&zip_path);
        if !found || !target.is_file() {
            return Err("Failed to extract ffmpeg.exe from archive.".into());
        }
        progress(1.0, "FFmpeg installed successfully!");
        Ok(target)
    } else {
        let url = "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";
        let tar_path = target_dir.join("ffmpeg_download.tar.xz");
        download_with_progress(url, &tar_path, progress)?;
        progress(0.85, "Extracting FFmpeg static binary...");
        let file = fs::File::open(&tar_path)?;
        let mut archive = tar::Archive::new(xz2::read::XzDecoder::new(file));
        let target = dest_dir.join("ffmpeg");
        let mut found = false;
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            let is_bin = path.file_name().map(|n| n == "ffmpeg").unwrap_or(false)
                && entry.header().entry_type().is_file();
            if is_bin {
                entry.unpack(&target)?;
                found = true;
                break;
            }
        }
        let _ = fs::remove_file(&tar_path);
        if !found || !target.is_file() {
            return Err("Failed to extract ffmpeg from archive.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o755))?;
        }
        progress(1.0, "FFmpeg installed successfully!");
        Ok(target)
    }
}
