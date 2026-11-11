#!/usr/bin/env python3
"""
bbbStudentDownloader / BBB StudyForge v2.0
The complete BigBlueButton lecture archiver & AI study suite.

Features:
- Video (.mp4) with embedded slide chapters (screen share OR slide-sync video)
- High-res Presentation Slide Deck (.pdf and offline HTML viewer)
- Audio (.m4a) with ID3 course metadata tags
- Interactive Chat Transcript (Markdown table & offline searchable HTML viewer)
- AI Study Companion (.md) with ready-to-use prompts for NotebookLM, ChatGPT & Claude
- Live Link Inspector (probes course name, duration, slides, and tracks in 1s)
- Batch Downloader Queue (download multiple lectures in one click)
- Built-in 1-Click FFMPEG auto-installer
- Modern CustomTkinter Desktop GUI + CLI

Usage:
  python bbbStudentDownloader.py           # Launch Desktop GUI
  python bbbStudentDownloader.py --cli <url> [--output <dir>]
"""
import os
import sys
import argparse
import queue
import threading
from typing import Optional

from core.engine import inspect_recording, process_recording
from core.ffmpeg_helper import find_ffmpeg, auto_download_ffmpeg


def run_cli(args):
    """Executes a command-line download."""
    url = args.url
    out_dir = args.output or os.path.join(os.path.expanduser("~"), "Downloads", "Lectures")
    cookie = args.cookie or ""

    print(f"\n🚀 BBB StudyForge CLI v2.0")
    print(f"Target URL: {url}")
    print(f"Output Directory: {out_dir}\n")

    try:
        print("🔍 Inspecting recording...")
        info = inspect_recording(url, cookie=cookie)
        print(f"   Course:    {info['course_name']}")
        print(f"   Session:   {info['name']}")
        print(f"   Presenter: {info['presenter']}")
        print(f"   Duration:  {info['duration_formatted']} | Date: {info['date_str']}")
        print(f"   Tracks:    Audio: {info['assets']['has_audio']} | Deskshare: {info['assets']['has_deskshare']} | Slides: {info['slide_count']} | Chat: {info['chat_count']}\n")

        def on_log(msg):
            print(f"  • {msg}")

        def on_stage(num, desc):
            print(f"\n[{num+1}/6] {desc}")

        def on_prog(p):
            pct = int(p * 100)
            sys.stdout.write(f"\r  Progress: [{pct:3d}%] {'#' * (pct // 4):25s}")
            sys.stdout.flush()

        res = process_recording(
            url, out_dir, cookie=cookie,
            log_callback=on_log,
            progress_callback=on_prog,
            stage_callback=on_stage
        )
        print(f"\n\n🎉 Done! All files saved to:\n   {res}\n")
    except Exception as e:
        print(f"\n❌ Error: {e}\n")
        sys.exit(1)


def build_gui():
    """Builds and launches the CustomTkinter Desktop application."""
    import tkinter as tk
    from tkinter import filedialog, messagebox
    import customtkinter as ctk

    # Color Palette - Glassmorphism Midnight Dark
    BG = "#070B14"
    CARD = "#0F172A"
    FIELD = "#19243E"
    LINE = "#24345A"
    ACCENT = "#818CF8"
    ACCENT_HOVER = "#6366F1"
    OK = "#10B981"
    WARN = "#F59E0B"
    BAD = "#EF4444"
    TEXT = "#F8FAFC"
    MUTED = "#94A3B8"

    STAGES = ["Inspect", "Slides", "Download", "Audio", "Video", "Study Pack"]

    class App(ctk.CTk):
        def __init__(self):
            super().__init__()
            ctk.set_appearance_mode("dark")
            ctk.set_default_color_theme("blue")
            self.title("BBB StudyForge 2.0 - BigBlueButton Archiver & Study Suite")
            self.geometry("720x860")
            self.minsize(660, 800)
            self.configure(fg_color=BG)

            self.f_title = ctk.CTkFont(family="Segoe UI", size=22, weight="bold")
            self.f_section = ctk.CTkFont(family="Segoe UI", size=14, weight="bold")
            self.f_body = ctk.CTkFont(family="Segoe UI", size=13)
            self.f_small = ctk.CTkFont(family="Segoe UI", size=11)
            self.f_mono = ctk.CTkFont(family="Consolas", size=11)

            self.q = queue.Queue()
            self.busy = False
            self.result_folder = None
            self.details_on = False

            # Variables
            self.url_var = tk.StringVar()
            self.out_var = tk.StringVar(value=os.path.join(os.path.expanduser("~"), "Downloads", "Lectures"))
            self.cookie_var = tk.StringVar()
            self.cookie_on = tk.BooleanVar(value=False)

            # Export selection options
            self.opt_video = tk.BooleanVar(value=True)
            self.opt_audio = tk.BooleanVar(value=True)
            self.opt_slides = tk.BooleanVar(value=True)
            self.opt_chat = tk.BooleanVar(value=True)
            self.opt_ai = tk.BooleanVar(value=True)

            # Trim / lecture end detection options
            self.auto_trim = tk.BooleanVar(value=True)
            self.cut_time_var = tk.StringVar()

            # Set window icon
            try:
                icon_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "assets", "app_icon.ico")
                if os.path.isfile(icon_path):
                    self.iconbitmap(icon_path)
            except Exception:
                pass

            self.build_ui()
            self.check_ffmpeg()
            self.after(100, self.poll)

        def build_ui(self):
            # Header
            head = ctk.CTkFrame(self, fg_color="transparent")
            head.pack(fill="x", padx=28, pady=(20, 8))

            top_row = ctk.CTkFrame(head, fg_color="transparent")
            top_row.pack(fill="x")
            ctk.CTkLabel(top_row, text="🚀 BBB StudyForge", font=self.f_title, text_color=TEXT).pack(side="left")
            self.ff_pill = ctk.CTkLabel(top_row, text="", corner_radius=10, height=26, font=self.f_small, padx=10)
            self.ff_pill.pack(side="right")

            ctk.CTkLabel(head, text="Save full lecture videos, slides deck (PDF), transcripts & AI study notes.",
                         font=self.f_body, text_color=MUTED).pack(anchor="w", pady=(2, 0))

            # Tab View for Single vs Batch Downloader
            self.tabview = ctk.CTkTabview(self, fg_color=CARD, corner_radius=16,
                                          segmented_button_selected_color=ACCENT_HOVER,
                                          segmented_button_unselected_color=FIELD)
            self.tabview.pack(fill="both", expand=True, padx=28, pady=(10, 10))

            self.tab_single = self.tabview.add("  Quick Save  ")
            self.tab_batch = self.tabview.add("  Batch Queue  ")
            self.tab_tools = self.tabview.add("  Settings & Tools  ")

            self.build_single_tab()
            self.build_batch_tab()
            self.build_tools_tab()

            # Global Footer
            foot = ctk.CTkFrame(self, fg_color="transparent")
            foot.pack(fill="x", padx=28, pady=(0, 16))

            self.bar = ctk.CTkProgressBar(foot, height=8, corner_radius=5, fg_color=FIELD, progress_color=OK)
            self.bar.set(0)
            self.bar.pack(fill="x", pady=(4, 8))

            status_row = ctk.CTkFrame(foot, fg_color="transparent")
            status_row.pack(fill="x")
            self.status_lbl = ctk.CTkLabel(status_row, text="Ready when you are", font=self.f_body, text_color=TEXT)
            self.status_lbl.pack(side="left")

            self.open_btn = ctk.CTkButton(status_row, text="📂 Open Folder", width=120, height=32, corner_radius=10,
                                          fg_color=FIELD, hover_color=LINE, text_color=TEXT, font=self.f_small,
                                          state="disabled", command=self.open_result)
            self.open_btn.pack(side="right")

        def build_single_tab(self):
            t = self.tab_single

            # URL input row
            ctk.CTkLabel(t, text="RECORDING PLAYBACK LINK", font=self.f_small, text_color=MUTED).pack(anchor="w", pady=(6, 4))
            row1 = ctk.CTkFrame(t, fg_color="transparent")
            row1.pack(fill="x", pady=(0, 10))

            self.url_entry = ctk.CTkEntry(row1, textvariable=self.url_var, placeholder_text="Paste BigBlueButton recording link here",
                                          height=40, corner_radius=10, border_width=1, border_color=LINE, fg_color=FIELD,
                                          text_color=TEXT, font=self.f_body)
            self.url_entry.pack(side="left", fill="x", expand=True, padx=(0, 8))

            ctk.CTkButton(row1, text="Paste", width=70, height=40, corner_radius=10, fg_color=FIELD,
                          hover_color=LINE, font=self.f_body, command=self.paste).pack(side="left", padx=(0, 6))
            ctk.CTkButton(row1, text="Inspect", width=80, height=40, corner_radius=10, fg_color=FIELD,
                          hover_color=LINE, font=self.f_body, command=self.inspect_link).pack(side="left")

            # Live Inspection Result Frame
            self.inspect_frame = ctk.CTkFrame(t, fg_color=FIELD, corner_radius=12, border_width=1, border_color=LINE)
            self.inspect_title = ctk.CTkLabel(self.inspect_frame, text="", font=self.f_section, text_color=ACCENT, anchor="w")
            self.inspect_title.pack(fill="x", padx=14, pady=(10, 2))
            self.inspect_details = ctk.CTkLabel(self.inspect_frame, text="", font=self.f_small, text_color=MUTED, anchor="w")
            self.inspect_details.pack(fill="x", padx=14, pady=(0, 6))
            self.inspect_chips = ctk.CTkLabel(self.inspect_frame, text="", font=self.f_small, text_color=OK, anchor="w")
            self.inspect_chips.pack(fill="x", padx=14, pady=(0, 10))

            # Save Into Directory
            ctk.CTkLabel(t, text="SAVE DIRECTORY", font=self.f_small, text_color=MUTED).pack(anchor="w", pady=(8, 4))
            row2 = ctk.CTkFrame(t, fg_color="transparent")
            row2.pack(fill="x", pady=(0, 8))
            ctk.CTkEntry(row2, textvariable=self.out_var, height=40, corner_radius=10, border_width=1,
                         border_color=LINE, fg_color=FIELD, text_color=TEXT, font=self.f_body).pack(side="left", fill="x", expand=True, padx=(0, 8))
            ctk.CTkButton(row2, text="Browse", width=80, height=40, corner_radius=10, fg_color=FIELD,
                          hover_color=LINE, font=self.f_body, command=self.browse).pack(side="left")

            # Cookie Switch
            self.cookie_switch = ctk.CTkSwitch(t, text="This recording needs a login (cookies)", variable=self.cookie_on,
                                               command=self.toggle_cookie, font=self.f_small, progress_color=ACCENT_HOVER)
            self.cookie_switch.pack(anchor="w", pady=(6, 4))
            self.cookie_entry = ctk.CTkEntry(t, textvariable=self.cookie_var, show="•", height=36, corner_radius=10,
                                             border_width=1, border_color=LINE, fg_color=FIELD, text_color=TEXT,
                                             placeholder_text="Paste your session cookie (name=value; ...)", font=self.f_body)

            # ── SMART TRIM — Remove Dead Time After Class ──────────────────────
            trim_frame = ctk.CTkFrame(t, fg_color=FIELD, corner_radius=12, border_width=1, border_color=LINE)
            trim_frame.pack(fill="x", pady=(10, 4))
            trim_inner = ctk.CTkFrame(trim_frame, fg_color="transparent")
            trim_inner.pack(fill="x", padx=14, pady=10)

            ctk.CTkLabel(trim_inner, text="✂️  SMART TRIM — Remove Dead Time After Class",
                         font=self.f_small, text_color=MUTED, anchor="w").pack(fill="x", pady=(0, 6))
            trim_row = ctk.CTkFrame(trim_inner, fg_color="transparent")
            trim_row.pack(fill="x")
            self.auto_trim_switch = ctk.CTkSwitch(trim_row, text="Auto-detect and cut silent recording tail",
                                                   variable=self.auto_trim, font=self.f_small,
                                                   progress_color=ACCENT_HOVER)
            self.auto_trim_switch.pack(side="left")

            ctk.CTkLabel(trim_inner, text="Or set a manual cut point (overrides auto):",
                         font=self.f_small, text_color=MUTED, anchor="w").pack(fill="x", pady=(8, 2))
            self.cut_entry = ctk.CTkEntry(trim_inner, textvariable=self.cut_time_var, height=34, corner_radius=10,
                                          border_width=1, border_color=LINE, fg_color=CARD, text_color=TEXT,
                                          placeholder_text="e.g.  1h 45m   or   01:45:00   (blank = none)",
                                          font=self.f_body)
            self.cut_entry.pack(fill="x")

            self.trim_warn_lbl = ctk.CTkLabel(trim_inner, text="", font=self.f_small, text_color=WARN, anchor="w")
            self.trim_warn_lbl.pack(fill="x", pady=(4, 0))
            # ──────────────────────────────────────────────────────────────────

            # Checkbox options
            ctk.CTkLabel(t, text="EXPORT MODULES", font=self.f_small, text_color=MUTED).pack(anchor="w", pady=(8, 4))
            opts_frame = ctk.CTkFrame(t, fg_color="transparent")
            opts_frame.pack(fill="x", pady=(0, 10))
            ctk.CTkCheckBox(opts_frame, text="Video (.mp4 + Chapters)", variable=self.opt_video, font=self.f_small).pack(side="left", padx=(0, 10))
            ctk.CTkCheckBox(opts_frame, text="Audio (.m4a)", variable=self.opt_audio, font=self.f_small).pack(side="left", padx=(0, 10))
            ctk.CTkCheckBox(opts_frame, text="Slides (PDF)", variable=self.opt_slides, font=self.f_small).pack(side="left", padx=(0, 10))
            ctk.CTkCheckBox(opts_frame, text="Chat (.md/.html)", variable=self.opt_chat, font=self.f_small).pack(side="left", padx=(0, 10))
            ctk.CTkCheckBox(opts_frame, text="AI Study Pack", variable=self.opt_ai, font=self.f_small).pack(side="left")

            # Main Action Button
            self.go_btn = ctk.CTkButton(t, text="⚡ Download Complete Study Suite", height=50, corner_radius=14,
                                        font=self.f_section, fg_color=ACCENT_HOVER, hover_color=ACCENT, text_color="#fff",
                                        command=self.start_single)
            self.go_btn.pack(fill="x", pady=(12, 10))

            # Toggle Log Details Button & Textbox
            log_head = ctk.CTkFrame(t, fg_color="transparent")
            log_head.pack(fill="x", pady=(4, 2))
            self.details_btn = ctk.CTkButton(log_head, text="Show Log Details ▼", width=120, height=24,
                                             corner_radius=6, fg_color="transparent", hover_color=FIELD,
                                             text_color=MUTED, font=self.f_small, command=self.toggle_details)
            self.details_btn.pack(side="left")

            self.log_box = ctk.CTkTextbox(t, height=120, corner_radius=10, fg_color=FIELD, text_color=MUTED, font=self.f_mono)

        def build_batch_tab(self):
            t = self.tab_batch
            ctk.CTkLabel(t, text="BATCH LECTURE QUEUE", font=self.f_section, text_color=TEXT).pack(anchor="w", pady=(10, 4))
            ctk.CTkLabel(t, text="Paste multiple BigBlueButton recording links (one per line) to download all of them overnight.",
                         font=self.f_small, text_color=MUTED).pack(anchor="w", pady=(0, 8))

            self.batch_text = ctk.CTkTextbox(t, height=220, corner_radius=12, fg_color=FIELD, text_color=TEXT, font=self.f_mono)
            self.batch_text.pack(fill="both", expand=True, pady=(0, 12))

            self.batch_btn = ctk.CTkButton(t, text="🚀 Start Batch Download", height=46, corner_radius=12,
                                           font=self.f_section, fg_color=ACCENT_HOVER, hover_color=ACCENT,
                                           command=self.start_batch)
            self.batch_btn.pack(fill="x")

        def build_tools_tab(self):
            t = self.tab_tools
            ctk.CTkLabel(t, text="SYSTEM TOOLS & CONVERTER", font=self.f_section, text_color=TEXT).pack(anchor="w", pady=(12, 6))

            # FFmpeg Card
            card1 = ctk.CTkFrame(t, fg_color=FIELD, corner_radius=14, border_width=1, border_color=LINE)
            card1.pack(fill="x", pady=(0, 14), padx=4)
            ctk.CTkLabel(card1, text="FFmpeg Media Converter", font=self.f_section, text_color=TEXT).pack(anchor="w", padx=16, pady=(12, 4))
            ctk.CTkLabel(card1, text="Required for encoding MP4 video with slide chapters and M4A audio.\nIf missing or uninstalled, click below to set it up in one click.",
                         font=self.f_small, text_color=MUTED, justify="left").pack(anchor="w", padx=16, pady=(0, 10))

            btn_row = ctk.CTkFrame(card1, fg_color="transparent")
            btn_row.pack(fill="x", padx=16, pady=(0, 14))
            self.ff_auto_btn = ctk.CTkButton(btn_row, text="📥 1-Click Auto Install FFmpeg", height=36, corner_radius=10,
                                             fg_color=OK, text_color="#000", font=self.f_small, command=self.auto_install_ff)
            self.ff_auto_btn.pack(side="left")

        def check_ffmpeg(self):
            if find_ffmpeg():
                self.ff_pill.configure(text="✔ Converter Ready", text_color="#06281C", fg_color=OK)
                self.ff_auto_btn.configure(text="✔ FFmpeg is Installed", state="disabled", fg_color=FIELD, text_color=MUTED)
            else:
                self.ff_pill.configure(text="⚠ Converter Missing", text_color="#3A2A00", fg_color=WARN)

        def auto_install_ff(self):
            self.ff_auto_btn.configure(text="Downloading FFmpeg...", state="disabled")
            self.log("Starting automatic FFmpeg setup...")
            def worker():
                try:
                    res = auto_download_ffmpeg(progress_callback=lambda f, msg: self.q.put(("status", msg)))
                    self.q.put(("log", f"FFmpeg installed at: {res}"))
                    self.q.put(("check_ff", None))
                except Exception as ex:
                    self.q.put(("error", f"FFmpeg install failed: {ex}"))
            threading.Thread(target=worker, daemon=True).start()

        def toggle_cookie(self):
            if self.cookie_on.get():
                self.cookie_entry.pack(fill="x", pady=(0, 8))
            else:
                self.cookie_entry.pack_forget()

        def toggle_details(self):
            self.details_on = not self.details_on
            if self.details_on:
                self.log_box.pack(fill="both", expand=True, pady=(6, 8))
                self.details_btn.configure(text="Hide Log Details ▲")
            else:
                self.log_box.pack_forget()
                self.details_btn.configure(text="Show Log Details ▼")

        def log(self, msg):
            self.log_box.configure(state="normal")
            self.log_box.insert("end", f"• {msg}\n")
            self.log_box.see("end")
            self.log_box.configure(state="disabled")

        def paste(self):
            try:
                txt = self.clipboard_get().strip()
                self.url_var.set(txt)
                self.inspect_link()
            except Exception:
                pass

        def browse(self):
            f = filedialog.askdirectory(initialdir=self.out_var.get() or os.path.expanduser("~"))
            if f:
                self.out_var.set(f)

        def inspect_link(self):
            url = self.url_var.get().strip()
            if not url:
                return
            cookie = self.cookie_var.get().strip() if self.cookie_on.get() else ""
            self.inspect_frame.pack(fill="x", pady=(0, 10))
            self.inspect_title.configure(text="Inspecting recording...")
            self.inspect_details.configure(text="Connecting to BigBlueButton server...")
            self.inspect_chips.configure(text="")

            def worker():
                try:
                    data = inspect_recording(url, cookie=cookie)
                    self.q.put(("inspect_ok", data))
                except Exception as ex:
                    self.q.put(("inspect_err", str(ex)))
            threading.Thread(target=worker, daemon=True).start()

        def start_single(self):
            if self.busy:
                return
            url = self.url_var.get().strip()
            out_dir = self.out_var.get().strip()
            if not url:
                messagebox.showwarning("Missing Link", "Paste the BigBlueButton recording link first.")
                return

            cookie = self.cookie_var.get().strip() if self.cookie_on.get() else ""
            from core.parser import parse_time_string
            cut_raw = self.cut_time_var.get().strip()
            custom_cut = parse_time_string(cut_raw) if cut_raw else None
            options = {
                "save_video": self.opt_video.get(),
                "save_audio": self.opt_audio.get(),
                "save_slides": self.opt_slides.get(),
                "save_chat": self.opt_chat.get(),
                "save_study_pack": self.opt_ai.get(),
                "auto_trim": self.auto_trim.get(),
                "custom_cut_seconds": custom_cut,
            }

            self.busy = True
            self.result_folder = None
            self.open_btn.configure(state="disabled")
            self.go_btn.configure(text="Working on lecture...", state="disabled", fg_color=LINE)
            self.bar.set(0)

            def worker():
                try:
                    res = process_recording(
                        url, out_dir, options=options, cookie=cookie,
                        log_callback=lambda m: self.q.put(("log", m)),
                        progress_callback=lambda p: self.q.put(("progress", p)),
                        stage_callback=lambda num, desc: self.q.put(("stage", (num, desc)))
                    )
                    self.q.put(("done", res))
                except Exception as ex:
                    self.q.put(("error", str(ex)))

            threading.Thread(target=worker, daemon=True).start()

        def start_batch(self):
            lines = [l.strip() for l in self.batch_text.get("1.0", "end").splitlines() if l.strip()]
            if not lines:
                messagebox.showinfo("Empty Queue", "Paste one or more recording links into the box.")
                return

            out_dir = self.out_var.get().strip()
            cookie = self.cookie_var.get().strip() if self.cookie_on.get() else ""
            self.batch_btn.configure(text="Processing Queue...", state="disabled")

            def batch_worker():
                total = len(lines)
                for idx, url in enumerate(lines, start=1):
                    self.q.put(("status", f"Batch item [{idx}/{total}]..."))
                    try:
                        res = process_recording(url, out_dir, cookie=cookie,
                                                log_callback=lambda m: self.q.put(("log", m)),
                                                progress_callback=lambda p: self.q.put(("progress", (idx - 1 + p) / total)))
                        self.q.put(("log", f"Finished batch item {idx}: {res}"))
                    except Exception as e:
                        self.q.put(("log", f"Failed batch item {idx}: {e}"))
                self.q.put(("batch_done", total))

            threading.Thread(target=batch_worker, daemon=True).start()

        def open_result(self):
            if self.result_folder and os.path.exists(self.result_folder):
                if hasattr(os, "startfile"):
                    os.startfile(self.result_folder)
                else:
                    import subprocess
                    subprocess.Popen(["xdg-open", self.result_folder])

        def poll(self):
            try:
                while True:
                    kind, val = self.q.get_nowait()
                    if kind == "log":
                        self.log(val)
                    elif kind == "progress":
                        self.bar.set(val)
                    elif kind == "status":
                        self.status_lbl.configure(text=val)
                    elif kind == "stage":
                        num, desc = val
                        self.status_lbl.configure(text=f"Step {num+1}/6: {desc}")
                    elif kind == "inspect_ok":
                        self.inspect_title.configure(text=f"{val['course_name']} - {val['name']}")
                        self.inspect_details.configure(
                            text=f"Duration: {val['duration_formatted']} | Date: {val['date_str']} | Presenter: {val['presenter']}"
                        )
                        chips_str = f"Tracks: Audio: {'✔' if val['assets']['has_audio'] else '✖'} | Screen: {'✔' if val['assets']['has_deskshare'] else '✖'} | Slides: {val['slide_count']} | Chat: {val['chat_count']}"
                        self.inspect_chips.configure(text=chips_str)
                        # Update smart trim warning
                        if val.get("has_dead_time"):
                            dead = val.get("dead_time_formatted", "")
                            cut = val.get("detected_cut_formatted", "")
                            self.trim_warn_lbl.configure(
                                text=f"⚠  {dead} of dead time detected after class ended. Auto-trim to {cut}."
                            )
                            if not self.cut_time_var.get().strip():
                                self.cut_time_var.set(cut)
                        else:
                            self.trim_warn_lbl.configure(text="")
                    elif kind == "inspect_err":
                        self.inspect_title.configure(text="Inspection failed")
                        self.inspect_details.configure(text=val)
                    elif kind == "check_ff":
                        self.check_ffmpeg()
                    elif kind == "done":
                        self.finish(val)
                    elif kind == "batch_done":
                        self.batch_btn.configure(text="🚀 Start Batch Download", state="normal")
                        messagebox.showinfo("Batch Finished", f"Successfully processed {val} lectures.")
                    elif kind == "error":
                        self.fail(val)
            except queue.Empty:
                pass
            self.after(100, self.poll)

        def finish(self, folder):
            self.busy = False
            self.result_folder = folder
            self.bar.set(1.0)
            self.status_lbl.configure(text=f"Saved: {os.path.basename(folder)}", text_color=OK)
            self.go_btn.configure(text="⚡ Save Another Lecture", state="normal", fg_color=ACCENT_HOVER)
            self.open_btn.configure(state="normal", fg_color=OK, text_color="#000")

        def fail(self, msg):
            self.busy = False
            self.bar.set(0)
            self.status_lbl.configure(text="Something went wrong", text_color=BAD)
            self.go_btn.configure(text="Try Again", state="normal", fg_color=ACCENT_HOVER)
            messagebox.showerror("Download Error", msg)

    App().mainloop()


def main():
    parser = argparse.ArgumentParser(description="BBB StudyForge 2.0 - BigBlueButton Archiver & AI Study Suite")
    parser.add_argument("--cli", dest="url", help="Download recording via command-line (pass playback URL)")
    parser.add_argument("--output", "-o", help="Target output folder")
    parser.add_argument("--cookie", "-c", help="Session cookie string for authenticated recordings")

    args = parser.parse_args()

    if args.url:
        run_cli(args)
    else:
        # Launch Desktop GUI
        try:
            import tkinter
            import customtkinter
            build_gui()
        except (ImportError, ModuleNotFoundError) as e:
            print(f"\n❌ Error launching GUI: {e}")
            print("To run the desktop GUI, please install customtkinter:\n    pip install customtkinter")
            print("Or use the CLI mode:\n    python bbbStudentDownloader.py --cli <URL> [--output <DIR>]\n")
            sys.exit(1)
        except Exception as e:
            print(f"\n❌ Error: {e}\n")
            sys.exit(1)


if __name__ == "__main__":
    main()
