#!/usr/bin/env python3
"""
Lecture Saver - save BigBlueButton lecture recordings with a clean, modern window.

Saves each lecture into a folder named after the course, keeps only the finished
files, and removes the working pieces.

Setup:   pip install customtkinter
Run:     python LectureSaver.py
Build:   pyinstaller --onefile --windowed --collect-all customtkinter --name LectureSaver LectureSaver.py
"""
import html
import os
import queue
import re
import shutil
import subprocess
import sys
import threading
import tkinter as tk
import urllib.error
import urllib.request
from tkinter import filedialog, messagebox
from urllib.parse import urlparse

import customtkinter as ctk

# ---------- look ----------
BG = "#0B1020"
CARD = "#141B2D"
FIELD = "#1C2540"
LINE = "#2A3658"
ACCENT = "#7C5CFF"
ACCENT_HOVER = "#9379FF"
OK = "#34D399"
WARN = "#FBBF24"
BAD = "#F87171"
TEXT = "#F1F5F9"
MUTED = "#8A94A8"
STAGES = ["Download", "Audio", "Video", "Tidy up"]

# Files we try to fetch from the recording: (local name, path on the server)
PARTS = [
    ("metadata.xml", "metadata.xml"),
    ("shapes.svg", "shapes.svg"),
    ("slides_new.xml", "slides_new.xml"),
    ("webcams.webm", "video/webcams.webm"),
    ("deskshare.webm", "deskshare/deskshare.webm"),
]
WORKING_FILES = ["metadata.xml", "shapes.svg", "slides_new.xml"]


# ---------- background work (no GUI calls here) ----------
def fetch(url, dest, cookie, log, progress):
    headers = {"User-Agent": "Mozilla/5.0"}
    if cookie:
        headers["Cookie"] = cookie
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=60) as r, open(dest, "wb") as f:
            total = int(r.headers.get("Content-Length") or 0)
            done = 0
            while True:
                chunk = r.read(1 << 20)
                if not chunk:
                    break
                f.write(chunk)
                done += len(chunk)
                if total:
                    progress(done / total)
        log(f"✔ {os.path.basename(dest)}")
        return True
    except urllib.error.HTTPError as e:
        if os.path.exists(dest):
            os.remove(dest)
        log(f"· {os.path.basename(dest)} not on server (HTTP {e.code})")
        return False


def find_ffmpeg():
    """Use the ffmpeg.exe shipped next to the app; fall back to one installed on the system."""
    app_dir = os.path.dirname(sys.executable) if getattr(sys, "frozen", False) \
        else os.path.dirname(os.path.abspath(__file__))
    for candidate in (os.path.join(app_dir, "ffmpeg.exe"), os.path.join(app_dir, "ffmpeg")):
        if os.path.isfile(candidate):
            return candidate
    return shutil.which("ffmpeg")


def run_ffmpeg(args):
    kwargs = {}
    if os.name == "nt":
        kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW
    res = subprocess.run([find_ffmpeg() or "ffmpeg", "-y", "-hide_banner", "-loglevel", "error", *args],
                         capture_output=True, text=True, **kwargs)
    if res.returncode:
        raise RuntimeError(res.stderr.strip()[-300:] or "ffmpeg failed")


def read_text(path):
    try:
        with open(path, encoding="utf-8", errors="ignore") as f:
            return f.read()
    except OSError:
        return ""


def course_name(meta_path):
    """Find the course name stored in the recording's metadata (the 'context' field)."""
    txt = read_text(meta_path)
    fallback = ""
    for tag, val in re.findall(r"<([\w.\-]*context[\w.\-]*)>(.*?)</\1>", txt, re.I | re.S):
        val = html.unescape(val).strip()
        if not val:
            continue
        if "name" in tag.lower():
            return val
        fallback = fallback or val
    return fallback


def recording_name(meta_path):
    m = re.search(r"<name>(.*?)</name>", read_text(meta_path), re.S)
    return html.unescape(m.group(1)).strip() if m else ""


def safe_filename(name):
    return re.sub(r'[\\/:*?"<>|\r\n]+', " ", name).strip()[:120] or "Lecture"


def unique_folder(parent, name):
    path = os.path.join(parent, name)
    n = 2
    while os.path.exists(path):
        path = os.path.join(parent, f"{name} ({n})")
        n += 1
    return path


def remove_quiet(path):
    try:
        os.remove(path)
    except OSError:
        pass


def job(url, outdir, cookie, q):
    log = lambda m: q.put(("log", m))
    prog = lambda p: q.put(("progress", p))
    stage = lambda i: q.put(("stage", i))
    status = lambda s: q.put(("status", s))

    try:
        m = re.search(r"/playback/presentation/[\d.]+/([^/?#]+)", url)
        if not m:
            raise ValueError("That link doesn't look right. It should contain /playback/presentation/")
        meeting = m.group(1)
        u = urlparse(url)
        base = f"{u.scheme}://{u.netloc}/presentation/{meeting}"

        work = os.path.join(outdir, meeting)
        os.makedirs(work, exist_ok=True)
        p = lambda local: os.path.join(work, local)

        # Stage 0: download
        stage(0)
        status("Downloading the recording…")
        for i, (local, remote) in enumerate(PARTS):
            fetch(f"{base}/{remote}", p(local), cookie, log,
                  lambda frac, i=i: prog((i + frac) / len(PARTS) * 0.6))
            prog((i + 1) / len(PARTS) * 0.6)

        has_audio = os.path.exists(p("webcams.webm"))
        has_screen = os.path.exists(p("deskshare.webm"))
        if not has_audio and not has_screen:
            raise RuntimeError("No audio or video was found. The link may need a login (turn on the login switch).")

        have_ffmpeg = find_ffmpeg() is not None
        if not have_ffmpeg:
            log("ffmpeg wasn't found, so the original files will be kept as they are.")

        # Stage 1: audio
        stage(1)
        status("Making the audio file…")
        made_audio = have_ffmpeg and has_audio
        if made_audio:
            run_ffmpeg(["-i", p("webcams.webm"), "-vn", "-c:a", "aac", "-b:a", "128k", p("audio.m4a")])
            log("audio ready")
        prog(0.7)

        # Stage 2: video
        stage(2)
        made_video = have_ffmpeg and has_audio and has_screen
        if made_video:
            status("Building the video…")
            run_ffmpeg(["-i", p("deskshare.webm"), "-i", p("webcams.webm"),
                        "-map", "0:v:0", "-map", "1:a:0",
                        "-c:v", "libx264", "-preset", "veryfast", "-crf", "23",
                        "-c:a", "aac", "-b:a", "128k", "-shortest", p("lecture.mp4")])
            log("video ready")
        else:
            status("No screen share in this recording, skipping video…")
        prog(0.85)

        # Stage 3: tidy up and name everything after the course
        stage(3)
        status("Tidying up…")
        name = safe_filename(course_name(p("metadata.xml"))
                             or recording_name(p("metadata.xml")) or meeting)

        for local in WORKING_FILES:
            remove_quiet(p(local))
        if made_audio:
            os.replace(p("audio.m4a"), p(f"{name} (audio).m4a"))
            remove_quiet(p("webcams.webm"))
        elif has_audio:
            os.replace(p("webcams.webm"), p(f"{name}.webm"))
        if made_video:
            os.replace(p("lecture.mp4"), p(f"{name}.mp4"))
            remove_quiet(p("deskshare.webm"))
        elif has_screen:
            os.replace(p("deskshare.webm"), p(f"{name} (screen).webm"))

        final = unique_folder(outdir, name)
        os.replace(work, final)
        prog(1.0)
        q.put(("done", final))
    except Exception as e:
        q.put(("error", str(e)))


# ---------- the window ----------
class App(ctk.CTk):
    def __init__(self):
        super().__init__()
        ctk.set_appearance_mode("dark")
        ctk.set_default_color_theme("blue")
        self.title("Lecture Saver")
        self.geometry("640x780")
        self.minsize(600, 740)
        self.configure(fg_color=BG)

        self.f_title = ctk.CTkFont(family="Segoe UI", size=24, weight="bold")
        self.f_big = ctk.CTkFont(family="Segoe UI", size=16, weight="bold")
        self.f_body = ctk.CTkFont(family="Segoe UI", size=13)
        self.f_small = ctk.CTkFont(family="Segoe UI", size=11)
        self.f_label = ctk.CTkFont(family="Segoe UI", size=10, weight="bold")
        self.f_mono = ctk.CTkFont(family="Consolas", size=11)

        self.q = queue.Queue()
        self.busy = False
        self.result_folder = None
        self.details_on = False
        self.url_var = tk.StringVar()
        self.out_var = tk.StringVar(value=os.path.join(os.path.expanduser("~"), "Downloads", "Lectures"))
        self.cookie_var = tk.StringVar()
        self.cookie_on = tk.BooleanVar(value=False)

        self.build_footer()
        self.build_header()
        self.build_body()
        self.check_ffmpeg()
        self.after(100, self.poll)

    # ----- layout -----
    def build_footer(self):
        foot = ctk.CTkFrame(self, fg_color="transparent")
        foot.pack(side="bottom", fill="x", padx=28, pady=18)
        self.ff_pill = ctk.CTkLabel(foot, text="", corner_radius=10, height=28, font=self.f_small, padx=12)
        self.ff_pill.pack(side="left")
        self.open_btn = ctk.CTkButton(foot, text="Open folder", width=130, height=36, corner_radius=12,
                                      fg_color=FIELD, hover_color=LINE, text_color=TEXT, font=self.f_body,
                                      state="disabled", command=self.open_result)
        self.open_btn.pack(side="right")

    def build_header(self):
        head = ctk.CTkFrame(self, fg_color="transparent")
        head.pack(fill="x", padx=28, pady=(26, 6))
        ctk.CTkLabel(head, text="Lecture Saver", font=self.f_title, text_color=TEXT).pack(anchor="w")
        ctk.CTkLabel(head, text="Slides and sound from your recordings, saved neatly by course.",
                     font=self.f_body, text_color=MUTED).pack(anchor="w", pady=(2, 0))

        bar = ctk.CTkFrame(self, fg_color=CARD, corner_radius=14)
        bar.pack(fill="x", padx=28, pady=(16, 6))
        self.step_labels = []
        for i, label in enumerate(STAGES):
            lbl = ctk.CTkLabel(bar, text=f"{i + 1}  {label}", font=self.f_small, text_color=MUTED)
            lbl.pack(side="left", expand=True, fill="x", pady=12)
            self.step_labels.append(lbl)

    def label(self, parent, text):
        ctk.CTkLabel(parent, text=text, font=self.f_label, text_color=MUTED, anchor="w").pack(fill="x", pady=(8, 4))

    def build_body(self):
        card = ctk.CTkFrame(self, fg_color=CARD, corner_radius=18)
        card.pack(fill="x", padx=28, pady=8)
        inner = ctk.CTkFrame(card, fg_color="transparent")
        inner.pack(fill="x", padx=22, pady=16)

        self.label(inner, "RECORDING LINK")
        row = ctk.CTkFrame(inner, fg_color="transparent")
        row.pack(fill="x")
        ctk.CTkEntry(row, textvariable=self.url_var, placeholder_text="Paste the playback link here",
                     height=42, corner_radius=12, border_width=0, fg_color=FIELD,
                     text_color=TEXT, font=self.f_body).pack(side="left", fill="x", expand=True, padx=(0, 10))
        ctk.CTkButton(row, text="Paste", width=80, height=42, corner_radius=12, fg_color=FIELD,
                      hover_color=LINE, text_color=TEXT, font=self.f_body, command=self.paste).pack(side="left")

        self.label(inner, "SAVE INTO")
        row2 = ctk.CTkFrame(inner, fg_color="transparent")
        row2.pack(fill="x")
        ctk.CTkEntry(row2, textvariable=self.out_var, height=42, corner_radius=12, border_width=0,
                     fg_color=FIELD, text_color=TEXT, font=self.f_body).pack(side="left", fill="x", expand=True,
                                                                             padx=(0, 10))
        ctk.CTkButton(row2, text="Browse", width=80, height=42, corner_radius=12, fg_color=FIELD,
                      hover_color=LINE, text_color=TEXT, font=self.f_body, command=self.browse).pack(side="left")

        self.login_switch = ctk.CTkSwitch(inner, text="This link needs a login", variable=self.cookie_on,
                                          command=self.toggle_cookie, font=self.f_body, text_color=TEXT,
                                          progress_color=ACCENT)
        self.login_switch.pack(anchor="w", pady=(14, 0))
        self.cookie_entry = ctk.CTkEntry(inner, textvariable=self.cookie_var, show="•", height=38,
                                         corner_radius=12, border_width=0, fg_color=FIELD, text_color=TEXT,
                                         placeholder_text="Paste your browser cookie (stays on this PC)",
                                         font=self.f_body)

        self.go_btn = ctk.CTkButton(self, text="Save lecture", height=56, corner_radius=16, font=self.f_big,
                                    fg_color=ACCENT, hover_color=ACCENT_HOVER, text_color="white",
                                    command=self.start)
        self.go_btn.pack(fill="x", padx=28, pady=(16, 10))

        self.bar = ctk.CTkProgressBar(self, height=10, corner_radius=6, fg_color=FIELD, progress_color=OK)
        self.bar.set(0)
        self.bar.pack(fill="x", padx=28)

        self.status = ctk.CTkLabel(self, text="Ready when you are", font=self.f_body, text_color=TEXT, anchor="w")
        self.status.pack(fill="x", padx=28, pady=(12, 4))

        self.details_btn = ctk.CTkButton(self, text="Show details", width=110, height=26, corner_radius=8,
                                         fg_color="transparent", hover_color=CARD, text_color=MUTED,
                                         font=self.f_small, command=self.toggle_details)
        self.details_btn.pack(anchor="w", padx=22)

        self.log_box = ctk.CTkTextbox(self, height=150, corner_radius=12, fg_color=FIELD, text_color=MUTED,
                                      font=self.f_mono)
        self.set_stage(-1)

    # ----- small UI helpers -----
    def set_stage(self, i):
        for j, lbl in enumerate(self.step_labels):
            if i == 4 or j < i:
                lbl.configure(text=f"✓  {STAGES[j]}", text_color=OK)
            elif j == i:
                lbl.configure(text=f"{j + 1}  {STAGES[j]}", text_color=TEXT)
            else:
                lbl.configure(text=f"{j + 1}  {STAGES[j]}", text_color=MUTED)

    def toggle_cookie(self):
        if self.cookie_on.get():
            self.cookie_entry.pack(fill="x", pady=(8, 0))
        else:
            self.cookie_entry.pack_forget()

    def toggle_details(self):
        self.details_on = not self.details_on
        if self.details_on:
            self.log_box.pack(fill="both", expand=True, padx=28, pady=(6, 8))
            self.details_btn.configure(text="Hide details")
        else:
            self.log_box.pack_forget()
            self.details_btn.configure(text="Show details")

    def check_ffmpeg(self):
        if find_ffmpeg():
            self.ff_pill.configure(text="Converter ready", text_color="#06281C", fg_color=OK)
        else:
            self.ff_pill.configure(text="Converter missing: reinstall Lecture Saver", text_color="#3A2A00",
                                   fg_color=WARN)

    def log(self, msg):
        self.log_box.configure(state="normal")
        self.log_box.insert("end", msg + "\n")
        self.log_box.see("end")
        self.log_box.configure(state="disabled")

    def log_clear(self):
        self.log_box.configure(state="normal")
        self.log_box.delete("1.0", "end")
        self.log_box.configure(state="disabled")

    # ----- actions -----
    def paste(self):
        try:
            self.url_var.set(self.clipboard_get().strip())
            self.status.configure(text="Link pasted")
        except tk.TclError:
            messagebox.showinfo("Nothing to paste", "Copy the link first, then click Paste.")

    def browse(self):
        folder = filedialog.askdirectory(initialdir=self.out_var.get() or os.path.expanduser("~"))
        if folder:
            self.out_var.set(folder)

    def start(self):
        if self.busy:
            return
        url = self.url_var.get().strip()
        outdir = self.out_var.get().strip()
        if not url:
            messagebox.showwarning("Missing link", "Paste the recording link first.")
            return
        if not outdir:
            messagebox.showwarning("Missing folder", "Choose where to save first.")
            return
        try:
            os.makedirs(outdir, exist_ok=True)
        except OSError as e:
            messagebox.showerror("Folder problem", f"I couldn't use that folder:\n{e}")
            return

        cookie = self.cookie_var.get().strip() if self.cookie_on.get() else ""
        self.busy = True
        self.result_folder = None
        self.open_btn.configure(state="disabled")
        self.go_btn.configure(text="Working…", state="disabled", fg_color=LINE)
        self.bar.set(0)
        self.set_stage(-1)
        self.log_clear()
        self.log("Starting")
        threading.Thread(target=job, args=(url, outdir, cookie, self.q), daemon=True).start()

    def open_result(self):
        if self.result_folder and os.path.exists(self.result_folder):
            if hasattr(os, "startfile"):
                os.startfile(self.result_folder)
            else:
                subprocess.Popen(["xdg-open", self.result_folder])

    # ----- messages from the worker -----
    def poll(self):
        try:
            while True:
                kind, val = self.q.get_nowait()
                if kind == "log":
                    self.log(val)
                elif kind == "progress":
                    self.bar.set(val)
                elif kind == "status":
                    self.status.configure(text=val, text_color=TEXT)
                elif kind == "stage":
                    self.set_stage(val)
                elif kind == "done":
                    self.finish(val)
                elif kind == "error":
                    self.fail(val)
        except queue.Empty:
            pass
        self.after(100, self.poll)

    def finish(self, folder):
        self.busy = False
        self.result_folder = folder
        self.set_stage(4)
        self.bar.set(1)
        self.status.configure(text=f"Saved: {os.path.basename(folder)}", text_color=OK)
        self.log(f"Saved to: {folder}")
        self.go_btn.configure(text="Save another lecture", state="normal", fg_color=ACCENT)
        self.open_btn.configure(state="normal", fg_color=OK, text_color="#06281C")

    def fail(self, msg):
        self.busy = False
        self.bar.set(0)
        self.set_stage(-1)
        self.status.configure(text="Something went wrong", text_color=BAD)
        self.log(f"Error: {msg}")
        self.go_btn.configure(text="Try again", state="normal", fg_color=ACCENT)
        messagebox.showerror("Couldn't save the lecture", msg)


if __name__ == "__main__":
    App().mainloop()
