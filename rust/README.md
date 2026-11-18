# BBB StudyForge: Rust port

A Rust port of BBB StudyForge: the `core/` engine, the command line, and a desktop app.
The Python app stays as it is. This crate builds two binaries:

- `bbb-studyforge`: command line, including batch downloads
- `bbb-studyforge-gui`: desktop app (egui)

## Download

Windows builds are on the [Releases](../../releases) page as a portable zip with both programs and
FFmpeg bundled. Unzip anywhere and run `bbb-studyforge-gui.exe`.

## Build

```bash
cd rust
cargo build --release                  # CLI only
cargo build --release --features gui   # CLI + desktop app
# binaries: target/release/bbb-studyforge(.exe), target/release/bbb-studyforge-gui(.exe)
```

On Windows you need Rust (`winget install Rustlang.Rustup`) and the Visual C++ build tools.

Requires Rust 1.80+ and ffmpeg on `PATH` (or next to the binary, or in `./ffmpeg/`).
Run `bbb-studyforge --download-ffmpeg` to fetch a static build into `./ffmpeg/`.

## Desktop app

Paste one or more playback links (one per line) or load a `.txt` list, press **Inspect** to see the
course, duration, tracks and dead air, then **Download all**. Each recording shows its stage and
progress, and an **Open folder** button when done. Options, the output folder and the cookie field
work like the CLI flags below. The output folder and options are remembered between runs.
If FFmpeg is missing, **Install FFmpeg** downloads it next to the app.

## Command line

```bash
bbb-studyforge --cli "https://bbb.example.edu/playback/presentation/2.3/<meetingId>" -o ./Lectures
bbb-studyforge <URL1> <URL2> <URL3> -o ./Lectures      # several recordings in one run
bbb-studyforge --batch semester.txt -o ./Lectures      # one URL per line, # for comments
bbb-studyforge --cli "<url>" --inspect                 # probe only: tracks, duration, dead air
bbb-studyforge --cli "<url>" -c "name=value; ..."      # session cookie for login-protected recordings
bbb-studyforge --cli "<url>" --cut 1h45m               # cut at a fixed time
bbb-studyforge --cli "<url>" --no-trim --no-video      # other toggles: --no-audio --no-slides --no-chat --no-study-pack
```

Output folder layout matches the Python version (`<Course> - <Session>/` with MP4, M4A, slides PDF,
the HTML viewers, `Chat.md` and `AI_Study_Companion.md`).

In batch mode a failed recording does not stop the run. A summary at the end lists what was saved
and what failed, and the exit code is 1 if anything failed.

## Releases (CI)

`.github/workflows/rust.yml` runs tests and clippy on Linux and Windows for every push that touches
`rust/`, then builds a Windows zip (both programs plus FFmpeg) as a workflow artifact. Pushing a tag
that starts with `rust-v` also publishes it as a GitHub release:

```bash
git tag rust-v2.2.0 && git push origin rust-v2.2.0
```

`rust-v*` tags do not trigger the Python release workflow, which uses `v*` tags.

## Module map

| Rust module | Python source |
|---|---|
| `parser` | `core/parser.py` |
| `downloader` | `core/downloader.py` |
| `converter`, `pdf` | `core/converter.py` (Pillow PDF save is replaced by `pdf`) |
| `ffmpeg` | `core/ffmpeg_helper.py` |
| `study_pack`, `templates/` | `core/study_pack.py` (HTML moved into real `.html` template files) |
| `engine` | `core/engine.py` |
| `main.rs` | `run_cli` in `bbbStudentDownloader.py` |
| `bin/gui.rs` | `build_gui` in `bbbStudentDownloader.py` |

## Verification

- `cargo test`: 17 unit tests covering the same areas as `test_suite.py` (URL parsing, metadata, SVG, chat, time strings, dead-time detection, study pack, FFMETADATA), plus helpers (HTML escaping, XML, downloader).
- Parity run: the same fake BigBlueButton recording (3 slides, webcam audio, deskshare) was processed by the Python
  and Rust CLIs. `Chat.md`, the AI companion, the M4A, and the deskshare MP4 are byte-identical. Chapters, ID3 tags
  and page counts match.

## Intentional differences from the Python version

- **Escaping in the chat viewer.** Sender names and message text are HTML-escaped, and `</script>` in chat is escaped
  in the embedded JSON. The Python viewer inserted sender names as raw HTML.
- **FFMETADATA escaping.** `=`, `;`, `#`, `\` and newlines in titles are escaped. Python wrote them raw, which could
  corrupt chapter metadata.
- **Concat file quoting.** Slide paths containing `'` are escaped for ffconcat.
- **Asset probing runs in parallel.** The probes are independent HEAD-style requests, so they now run concurrently.
- **No ffmpeg PDF fallback.** The Python fallback called `ffmpeg` to write a PDF, but ffmpeg has no PDF muxer
  (checked in this environment), so it never worked. The Rust version writes the PDF itself.
- **PDF size.** Pages are JPEG q90 at 72 dpi, as in Pillow, but the PDF is larger (about 84 KB vs 51 KB for the
  three-slide test). Pillow and the Rust encoder subsample chroma differently.
- **Stage counter** reads `[n/7]`. Python printed `[7/6]`.
- **Downloads have no overall time limit.** Only connecting and each read time out, so long lectures finish.
- **Inspect fails early** when nothing exists at a link, instead of reporting an empty recording.

## Not ported yet

From the Python desktop app: single-instance lock, Windows toast notifications, clipboard link
sniffer, in-app update checker, crash log, overnight shutdown mode, and the Inno Setup installer
(the Rust release is a portable zip).
