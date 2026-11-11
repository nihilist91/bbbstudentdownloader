# BBB StudyForge 2.0 (bbbStudentDownloader)

> **The ultimate all-in-one BigBlueButton lecture archiver & AI study suite.**  
> Save complete lecture videos, high-resolution slide decks (PDF), podcast-ready audio, interactive chat transcripts, and AI study notes with a single click.

![platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue) ![python](https://img.shields.io/badge/python-3.9%2B-green) ![license](https://img.shields.io/badge/license-MIT-purple)

---

## 🌟 What's New in Version 2.0

| Feature | Original App | **BBB StudyForge 2.0** |
|---|---|---|
| **Lecture Video** | Only saved deskshare; skipped video if teacher used slides | **Synthesizes video from deskshare OR presentation slides!** |
| **Video Chapters** | ❌ None | **Embeds clickable MP4 chapter markers for every slide** |
| **Presentation Slides** | ❌ Skipped entirely ("Known limit") | **Exports slide deck as clean PDF & offline interactive HTML deck** |
| **Public Chat Log** | ❌ Not saved ("Known limit") | **Formatted Markdown table + offline searchable interactive HTML app** |
| **AI Study Companion** | ❌ None | **Extracts slide text, Q&A, and pre-crafts prompts for NotebookLM/ChatGPT** |
| **Link Inspector** | ❌ None | **Instantly probes title, date, duration, and tracks before downloading** |
| **Batch Queue** | ❌ 1 link at a time | **Paste 10+ links at once to download entire semester overnight** |
| **FFmpeg Setup** | ❌ Manual reinstall if missing | **1-Click Auto-Installer built directly into the app** |
| **Interfaces** | Desktop GUI only | **CustomTkinter Desktop GUI + Full CLI support** |

---

## 📦 What Ends Up in Your Study Folder

Each lecture is automatically organized into a tidy, ready-to-study folder named after your course:

```text
CS301 - Algorithms - Lecture 05/
  ├── 🎬 CS301 - Lecture 05.mp4             ← Full 1080p video with slide chapters
  ├── 🎙️ CS301 - Lecture 05 (audio).m4a     ← Crisp AAC audio with course ID3 tags
  ├── 📑 CS301 - Lecture 05 - Slides.pdf     ← Slide deck ready for iPad / tablet notes
  ├── 🌐 CS301 - Lecture 05 - Slides_Viewer.html ← Offline interactive slide presentation
  ├── 💬 CS301 - Lecture 05 - Chat.md        ← Formatted Markdown chat transcript
  ├── 🔍 CS301 - Lecture 05 - Chat_Viewer.html ← Offline searchable interactive chat
  ├── 🧠 CS301 - Lecture 05 - AI_Study_Companion.md ← Study guide & AI prompts
  └── 📁 slides/                             ← Individual high-res slide assets
```

---

## 🚀 How to Run

### 1. Desktop GUI (Recommended)
```bash
pip install --upgrade customtkinter
python bbbStudentDownloader.py
```

### 2. Command Line Interface (CLI)
```bash
python bbbStudentDownloader.py --cli "https://bbb.example.edu/playback/presentation/2.3/..." --output ./Lectures
```

---

## 💡 Key Features Walkthrough

### 1. Clickable Video Chapters
When you open the `.mp4` file in VLC, MPV, Windows Media Player, or QuickTime, each slide transition appears as a named chapter. Click any chapter to jump directly to that slide!

### 2. Presentation Slide Deck & 1-Click PDF
- `Slides_Viewer.html`: An offline, vector presentation viewer. Use arrow keys (`←` / `→`) or spacebar to flip slides.
- Click the **"Print / Save as PDF"** button (or press `Ctrl+P`) to export an ultra-sharp vector PDF matching your slides.
- If Pillow or FFmpeg is present, `Slides.pdf` is also built automatically on disk.

### 3. Interactive Chat Viewer
- `Chat_Viewer.html` works completely offline.
- Features real-time search, filters by student or professor, question highlights (`?`), and a one-click copy button.
- All links shared by classmates or the professor are organized into a quick-access panel.

### 4. AI-Ready Study Companion
`AI_Study_Companion.md` bundles the complete text extracted from every slide, the timeline of topics, and student Q&A. It comes with pre-written prompts for:
- 🎓 **Anki Flashcards Generator**
- 📝 **Practice Exam with Distractors**
- ⚡ **5-Minute Cheat Sheet Summary**

Simply copy-paste into **NotebookLM**, **ChatGPT**, or **Claude** to prepare for your exams!

---

## 🛠️ Building the Windows Installer (`.exe`)

You need Windows, Python 3.9+, and [Inno Setup](https://jrsoftware.org/isinfo.php).

```bash
pip install --upgrade customtkinter pyinstaller
python -m PyInstaller --clean --noconfirm --onefile --windowed --collect-all customtkinter --name bbbStudentDownloader bbbStudentDownloader.py
```

Then compile `setup.iss` in Inno Setup to generate `Output\bbbStudentDownloaderSetup.exe`.

---

## 📜 License

Distributed under the MIT License. See [LICENSE](LICENSE) for details.
