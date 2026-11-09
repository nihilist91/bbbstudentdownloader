# Lecture Saver

Save BigBlueButton lecture recordings (slides and sound) to your computer, as a clean file named after the course.

![status](https://img.shields.io/badge/platform-Windows-blue) ![python](https://img.shields.io/badge/python-3.9%2B-green)

## What it does

- Takes a recording link from your course platform.
- Downloads the audio and the screen-share parts of the recording.
- Converts them into ready-to-play files (`.m4a` for sound, `.mp4` when there is a screen share).
- Names the output after the course and deletes the working files, so the folder stays clean.

## Install (for users)

1. Go to the **Releases** page of this repository and download `LectureSaverSetup.exe`.
2. Run it. If Windows shows "Windows protected your PC", click **More info → Run anyway**. This happens because the app is not digitally signed.
3. Follow the installer. It includes everything the app needs, including ffmpeg, so you don't install anything else.
4. Open **Lecture Saver** from the Start menu or the desktop shortcut.

## How to use

For a screen-by-screen explanation of every button and message, see the [Guide](GUIDE.md).

1. **Get the recording link.** Open the lecture on your course platform and click the recording. Right-click the video area and choose **Open frame in new tab**. The address should contain `/playback/presentation/`. Copy it.
2. **Paste it** into the *Recording link* box, or click **Paste**.
3. **Choose where to save.** The default is `Downloads\Lectures`. Use **Browse** to pick another folder.
4. Click **Save lecture**. The four steps at the top light up as the app works:
   - **Download**: fetches the recording pieces.
   - **Audio**: creates the sound file.
   - **Video**: builds the screen-share video, if the recording has one.
   - **Tidy up**: names the files after the course and deletes the working files.
5. Click **Open folder** when it says it's done.

The output folder contains only the finished files, for example:

```
Course Name/
  Course Name (audio).m4a
  Course Name.mp4        (only if there was a screen share)
```

### Link needs a login?

If the app says it can't find the recording, turn on **This link needs a login**, then paste your browser's cookie text into the box that appears. The cookie is your login session, so treat it like a password:

- Get it from your browser's developer tools (F12 → Application or Storage → Cookies), copy the `name=value` pairs, and paste them in.
- Don't share it with anyone or post it online.
- The app keeps it only in memory for that session and does not save it to disk.

Never paste your account password into the app.

## Troubleshooting

| What you see | What it means | What to do |
|---|---|---|
| "Converter missing" in the footer | `ffmpeg.exe` is not next to the app | Reinstall with the official `LectureSaverSetup.exe` |
| "No audio or video was found" | The link needs a login, or it's not a recording link | Check the link contains `/playback/presentation/`, or turn on the login option |
| "Couldn't save" with an HTTP error | The server refused a file | Try again, or use the login option |
| Files are kept as `.webm` | The converter was missing, so the originals were kept | Reinstall. `.webm` files still play in VLC |

Click **Show details** in the app to see the full log.

## Building from source (for developers)

You need Windows, Python 3.9 or newer, and [Inno Setup](https://jrsoftware.org/isinfo.php).

```
pip install --upgrade customtkinter pyinstaller
python -m PyInstaller --clean --noconfirm --onefile --windowed --collect-all customtkinter --name LectureSaver LectureSaver.py
```

Then build the installer:

1. Create an `ffmpeg` folder in the project and copy `ffmpeg.exe` into it. Download ffmpeg from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) (the "release essentials" build). Copy any license files into the same folder.
2. Open `setup.iss` in Inno Setup and press **F9**.
3. The installer is written to `Output\LectureSaverSetup.exe`.

The `ffmpeg` folder, `dist`, and `build` are not stored in the repository. Upload the built installer to a GitHub Release.

## Notes and responsible use

- Only download recordings you're allowed to access. Check your institution's policy before sharing any recording.
- ffmpeg is distributed under the GPL/LGPL licenses. The installer includes its license files.
- The app is unsigned, so Windows SmartScreen will show a warning the first time.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
