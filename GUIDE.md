# BBB StudyForge 2.0: Complete Student Guide

This guide explains how to use **BBB StudyForge** (formerly bbbStudentDownloader) to save complete lectures, slides, chat transcripts, and study notes from BigBlueButton.

---

## Part 1: Finding Your Recording Link

1. Open your course platform (Moodle, Canvas, Blackboard, or BigBlueButton).
2. Open the recording you want to save.
3. Check the browser address bar. The link should look like one of these:
   - `https://.../playback/presentation/2.3/<meetingId>`
   - `https://.../playback/presentation/2.0/playback.html?meetingId=<meetingId>`
   - `https://.../presentation/<meetingId>`
4. Copy the link.

> **Tip:** If the recording is embedded in an iframe inside Moodle, right-click inside the player area and choose **Open frame in new tab** to copy the clean URL.

---

## Part 2: Using the Desktop App

### 1. The Quick Save Tab

- **Recording Link:** Paste your copied link. Click **Inspect** to see an immediate preview of the lecture title, date, duration, and available tracks (Audio, Screen Share, Slides, Chat).
- **Save Directory:** Choose where your lecture folder should be saved (default: `Downloads\Lectures`).
- **This link needs a login:** Turn this on only if your school restricts recording access to logged-in sessions. Paste your browser session cookie into the box (e.g. `name=value; session=...`).
- **Export Modules:**
  - ☑ **Video (.mp4 + Chapters):** Builds the full 1080p lecture video with clickable slide chapter markers.
  - ☑ **Audio (.m4a):** Extracts the lecture audio with course name and presenter tags.
  - ☑ **Slides (PDF & Viewer):** Downloads all slides as high-resolution images, generates an offline presentation deck, and compiles a PDF.
  - ☑ **Chat (.md & .html):** Saves the full public chat transcript.
  - ☑ **AI Study Pack:** Creates an AI study guide with flashcard and quiz prompts ready for NotebookLM/ChatGPT.
- Click **⚡ Download Complete Study Suite**.

### 2. The Batch Queue Tab

Need to download 5 or 10 lectures from your semester?
1. Click the **Batch Queue** tab.
2. Paste all your recording links, one per line.
3. Click **🚀 Start Batch Download**.
4. The app processes each lecture sequentially and alerts you when everything is finished!

### 3. Settings & Tools Tab

- **1-Click Auto Install FFmpeg:** If your computer does not have the media converter installed, click this button to automatically download and set it up.

---

## Part 3: Exploring Your Study Suite Folder

Each saved lecture includes:

| File | How to use it |
|---|---|
| `Course - Title.mp4` | Open in VLC or QuickTime. Click the chapter list in the player menu to jump to any slide! |
| `Course - Title (audio).m4a` | Listen to the lecture like a podcast on your phone or computer. |
| `Course - Title - Slides_Viewer.html` | Open in any browser. Press arrow keys to flip slides. Click **Print / Save as PDF** for vector PDF slides! |
| `Course - Title - Slides.pdf` | Direct PDF file ready for stylus annotation on iPad or Android tablets. |
| `Course - Title - Chat_Viewer.html` | Search for student questions, filter by presenter, and find links shared during class. |
| `Course - Title - Chat.md` | Plain Markdown log of all chat messages. |
| `Course - Title - AI_Study_Companion.md` | Contains slide text transcripts and pre-made prompts to feed into ChatGPT or NotebookLM for instant flashcards and practice exams. |

---

## Quick Troubleshooting

| Problem | Cause | Solution |
|---|---|---|
| "Converter missing" | FFmpeg binary not found | Go to the **Tools** tab in the app and click **1-Click Auto Install FFmpeg**. |
| "No audio or video found" | Lecture requires login | Toggle **This link needs a login** and paste your session cookie. |
| Slide video has no screen share | Teacher only used slides | BBB StudyForge automatically synthesizes a video from the slide images and audio! |
