"""
core/ffmpeg_helper.py - Robust FFMPEG locator, execution wrapper, and 1-click auto-installer.
"""
import os
import sys
import shutil
import zipfile
import tarfile
import subprocess
import urllib.request
from typing import Optional, Callable


def find_ffmpeg() -> Optional[str]:
    """
    Finds the ffmpeg binary across common locations:
    1. Next to the executable (PyInstaller frozen or portable folder)
    2. In an 'ffmpeg' subfolder next to the script/exe
    3. PyInstaller temporary extraction folder (_MEIPASS)
    4. System PATH
    """
    candidates = []

    if getattr(sys, "frozen", False):
        exe_dir = os.path.dirname(sys.executable)
        candidates.append(os.path.join(exe_dir, "ffmpeg.exe"))
        candidates.append(os.path.join(exe_dir, "ffmpeg", "ffmpeg.exe"))
        candidates.append(os.path.join(exe_dir, "ffmpeg"))
        if hasattr(sys, "_MEIPASS"):
            candidates.append(os.path.join(sys._MEIPASS, "ffmpeg.exe"))
            candidates.append(os.path.join(sys._MEIPASS, "ffmpeg", "ffmpeg.exe"))
            candidates.append(os.path.join(sys._MEIPASS, "ffmpeg"))
    else:
        here = os.path.dirname(os.path.abspath(__file__))
        root = os.path.dirname(here)
        candidates.append(os.path.join(root, "ffmpeg.exe"))
        candidates.append(os.path.join(root, "ffmpeg", "ffmpeg.exe"))
        candidates.append(os.path.join(root, "ffmpeg"))
        candidates.append(os.path.join(here, "ffmpeg.exe"))
        candidates.append(os.path.join(here, "ffmpeg"))

    for c in candidates:
        if os.path.isfile(c) and os.access(c, os.X_OK if os.name != "nt" else os.F_OK):
            return c

    # System PATH lookup
    system_path = shutil.which("ffmpeg")
    if system_path:
        return system_path

    return None


def run_ffmpeg(args: list, timeout: Optional[int] = None) -> None:
    """Runs an ffmpeg command synchronously, suppressing windows on Windows and capturing errors."""
    ffmpeg_bin = find_ffmpeg()
    if not ffmpeg_bin:
        raise RuntimeError("FFmpeg converter is not available. Please install FFmpeg or use the Auto-Install button.")

    cmd = [ffmpeg_bin, "-y", "-hide_banner", "-loglevel", "error", *args]
    kwargs = {}
    if os.name == "nt":
        kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW

    res = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, **kwargs)
    if res.returncode != 0:
        err_msg = res.stderr.strip() or "FFmpeg process failed with non-zero exit code"
        raise RuntimeError(f"FFmpeg conversion error: {err_msg[-400:]}")


def auto_download_ffmpeg(target_dir: Optional[str] = None,
                         progress_callback: Optional[Callable[[float, str], None]] = None) -> str:
    """
    Automatically downloads and sets up a standalone FFmpeg binary:
    - On Windows: Official Gyan.dev release essentials zip.
    - On Linux: John Van Sickle static build.
    Returns the path to the downloaded ffmpeg binary.
    """
    if target_dir is None:
        if getattr(sys, "frozen", False):
            target_dir = os.path.dirname(sys.executable)
        else:
            target_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

    dest_ffmpeg_dir = os.path.join(target_dir, "ffmpeg")
    os.makedirs(dest_ffmpeg_dir, exist_ok=True)

    if os.name == "nt":
        # Windows 64-bit Gyan.dev release essentials
        url = "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip"
        zip_path = os.path.join(target_dir, "ffmpeg_download.zip")

        if progress_callback:
            progress_callback(0.1, "Connecting to FFmpeg repository...")

        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=60) as resp, open(zip_path, "wb") as f:
            total = int(resp.headers.get("Content-Length") or 0)
            done = 0
            while True:
                chunk = resp.read(1 << 20)
                if not chunk:
                    break
                f.write(chunk)
                done += len(chunk)
                if progress_callback and total > 0:
                    progress_callback(0.1 + 0.7 * (done / total), f"Downloading FFmpeg... {done // (1024*1024)}MB / {total // (1024*1024)}MB")

        if progress_callback:
            progress_callback(0.85, "Extracting FFmpeg binary...")

        with zipfile.ZipFile(zip_path, "r") as z:
            for member in z.namelist():
                if member.endswith("ffmpeg.exe"):
                    source = z.open(member)
                    target_file = os.path.join(dest_ffmpeg_dir, "ffmpeg.exe")
                    with open(target_file, "wb") as tf:
                        shutil.copyfileobj(source, tf)
                    break

        try:
            os.remove(zip_path)
        except OSError:
            pass

        final_bin = os.path.join(dest_ffmpeg_dir, "ffmpeg.exe")
        if os.path.isfile(final_bin):
            if progress_callback:
                progress_callback(1.0, "FFmpeg installed successfully!")
            return final_bin
        raise RuntimeError("Failed to extract ffmpeg.exe from archive.")

    else:
        # Linux 64-bit static build
        url = "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
        tar_path = os.path.join(target_dir, "ffmpeg_download.tar.xz")

        if progress_callback:
            progress_callback(0.1, "Connecting to Linux FFmpeg mirror...")

        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        with urllib.request.urlopen(req, timeout=60) as resp, open(tar_path, "wb") as f:
            total = int(resp.headers.get("Content-Length") or 0)
            done = 0
            while True:
                chunk = resp.read(1 << 20)
                if not chunk:
                    break
                f.write(chunk)
                done += len(chunk)
                if progress_callback and total > 0:
                    progress_callback(0.1 + 0.7 * (done / total), f"Downloading FFmpeg... {done // (1024*1024)}MB")

        if progress_callback:
            progress_callback(0.85, "Extracting FFmpeg static binary...")

        with tarfile.open(tar_path, "r:xz") as tar:
            for member in tar.getmembers():
                if member.name.endswith("/ffmpeg") or member.name == "ffmpeg":
                    member.name = os.path.basename(member.name)
                    tar.extract(member, dest_ffmpeg_dir)
                    break

        try:
            os.remove(tar_path)
        except OSError:
            pass

        final_bin = os.path.join(dest_ffmpeg_dir, "ffmpeg")
        if os.path.isfile(final_bin):
            os.chmod(final_bin, 0o755)
            if progress_callback:
                progress_callback(1.0, "FFmpeg installed successfully!")
            return final_bin
        raise RuntimeError("Failed to extract ffmpeg from archive.")
