# BBB StudyForge: Rust port (core + CLI)

A Rust port of the Python `core/` package and the `--cli` mode of `bbbStudentDownloader.py`.
The Python app stays as it is. This crate is a standalone binary, `bbb-studyforge`.

## Build

```bash
cd rust
cargo build --release
# binary: target/release/bbb-studyforge
```

Requires Rust 1.80+ and ffmpeg on `PATH` (or next to the binary, or in `./ffmpeg/`).
Run `bbb-studyforge --download-ffmpeg` to fetch a static build into `./ffmpeg/`.

## Usage

```bash
bbb-studyforge --cli "https://bbb.example.edu/playback/presentation/2.3/<meetingId>" -o ./Lectures
bbb-studyforge --cli "<url>" --inspect                 # probe only: tracks, duration, dead air
bbb-studyforge --cli "<url>" -c "name=value; ..."      # session cookie for login-protected recordings
bbb-studyforge --cli "<url>" --cut 1h45m               # cut at a fixed time
bbb-studyforge --cli "<url>" --no-trim --no-video      # other toggles: --no-audio --no-slides --no-chat --no-study-pack
```

Output folder layout matches the Python version (`<Course> - <Session>/` with MP4, M4A, slides PDF,
the HTML viewers, `Chat.md` and `AI_Study_Companion.md`).

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

## Verification

- `cargo test`: 16 unit tests covering the same areas as `test_suite.py` (URL parsing, metadata, SVG, chat, time strings, dead-time detection, study pack, FFMETADATA), plus helpers (HTML escaping, XML, downloader).
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

## Not ported yet

- The CustomTkinter desktop GUI, and the Windows-only pieces that go with it (single-instance mutex,
  toast notifications, clipboard link sniffer, in-app update checker, crash log, Inno Setup packaging).
- The GUI batch queue. The CLI handles one URL per run.
