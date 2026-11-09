# bbbstudentDownloader: Screen-by-Screen Guide

This guide explains what each part of the app does, what each message means, and what ends up in your folder.

---

## Part 1: The bbbstudentDownloader window

### The step bar (top)

The four labels show the work in order. The current step is bright white. Finished steps show a green ✓.

| Step | What's happening |
|---|---|
| **1 Download** | The app fetches the recording's pieces from the server. |
| **2 Audio** | It converts the lecture's sound into an `.m4a` file. |
| **3 Video** | It builds the screen-share video, if the recording has one. If not, this step is skipped. |
| **4 Tidy up** | It names the files after the course, deletes the working files, and moves everything into one folder. |

### Recording link

Paste the playback link here. It should contain `/playback/presentation/`. The **Paste** button fills the box from your clipboard.

### Save into

This is where the course folder will be created. The default is `Downloads\Lectures`. Click **Browse** to pick a different folder.

### This link needs a login

Leave this **off** for most recordings. Turn it on only if the app says it can't find the recording. It then shows a box for your browser's cookie text. Treat that text like a password and don't share it.

### Save lecture (the big purple button)

This starts the process. Its label changes:

| Label | Meaning |
|---|---|
| **Save lecture** | Ready to start. |
| **Working…** | In progress. The button is greyed out, so don't close the window. |
| **Save another lecture** | The last lecture finished. Paste a new link to save another one. |
| **Try again** | Something went wrong. Check the details (see below) and try again. |

### Progress bar and status line

The bar fills from left to right as the app works. The line under it describes the current step, for example "Downloading the recording…" or "Making the audio file…".

### Show details

Click this to open the log. It lists each file as it's downloaded and each conversion as it finishes. If something fails, the error appears here first. Copy it when you ask for help.

### Footer

| Badge | Meaning |
|---|---|
| **Converter ready** (green) | The converter that makes the audio and video files is available. Everything works. |
| **Converter missing** (yellow) | The converter isn't found. Reinstall bbbstudentDownloader using the official installer. The app will still save the original `.webm` files. |

### Open folder (bottom right)

This button turns green when a lecture is saved. Click it to open the course folder.

---

## Part 2: The recording page

After you open the recording link in a new tab, you'll see the BigBlueButton player. Here's what its parts are and how they relate to bbbstudentDownloader.

| Part of the page | What it is | Saved by bbbstudentDownloader? |
|---|---|---|
| **Address bar** | The playback link you copy. | You copy this link. |
| **Title** (for example "Séance 19-10-2026 à 19h30") | The session name. The app uses the course name from the recording's details to name the output. | Used for the file names. |
| **Left panel: chat** | The messages from the live session, with timestamps. | **No.** The chat isn't included in the output. |
| **Bottom strip: slide thumbnails** | Each small picture is one slide. A monitor icon marks a screen-share segment. | Screen-share segments are included. Slides are **not yet** included (see Known limits). |
| **Big play button** | Plays the recording in the browser. | Not needed for the app. |

---

## Part 3: What ends up in your folder

The app creates one folder per lecture, named after the course:

```
Downloads\Lectures\
  Course Name\
    Course Name (audio).m4a    ← the lecture's sound (always, when there's audio)
    Course Name.mp4            ← screen share + sound (only if the session had a screen share)
```

If the converter was missing, you'll get the original `Course Name.webm` and, if there was a screen share, `Course Name (screen).webm` instead. These files play in VLC.

Working files, such as the metadata and slide data, are deleted automatically.

---

## Known limits

- **Slides are not yet included.** The output contains the screen-share video and the audio. Presentation slides that were shown in the normal slide view are not yet built into the video.
- **Chat is not saved.**
- **Downloading takes a while.** Longer recordings take longer, especially if the server is slow.
- **Login links.** Some recordings need you to be logged in. Use the login option described above.

---

## Quick fixes

| Problem | Try this |
|---|---|
| "No audio or video was found" | Check the link contains `/playback/presentation/`. If it does, turn on the login option. |
| Footer says "Converter missing" | Reinstall with the official installer. |
| Files end in `.webm` | The converter was missing when you saved. Reinstall, then save again. |
| The window closed during saving | Delete the partly saved folder and start again. |
| Anything else | Click **Show details**, copy the log, and ask for help. |
