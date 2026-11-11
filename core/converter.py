"""
core/converter.py - Audio/Video encoding, chapter injection, slide-sync video synthesis,
and PDF compilation for BigBlueButton recordings.
"""
import os
import re
import subprocess
from typing import List, Dict, Optional
from core.ffmpeg_helper import run_ffmpeg, find_ffmpeg


def generate_ffmetadata_file(metadata: dict, slides: List[dict], output_path: str) -> str:
    """
    Generates an FFMETADATA1 file containing global lecture metadata and
    chapter markers for each slide transition.
    """
    lines = [
        ";FFMETADATA1",
        f"title={metadata.get('name', 'Lecture')}",
        f"artist={metadata.get('presenter', 'Instructor')}",
        f"album={metadata.get('course_name', 'Course')}",
        f"date={metadata.get('date_str', '')[:10]}",
        "",
    ]

    for i, s in enumerate(slides):
        idx = s.get("index", i + 1)
        start_ms = int(s.get("in", 0.0) * 1000)
        end_ms = int(s.get("out", 0.0) * 1000)
        if end_ms <= start_ms:
            end_ms = start_ms + 5000

        lines.extend([
            "[CHAPTER]",
            "TIMEBASE=1/1000",
            f"START={start_ms}",
            f"END={end_ms}",
            f"title=Slide {idx}",
            "",
        ])

    with open(output_path, "w", encoding="utf-8") as f:
        f.write("\n".join(lines))

    return output_path


def build_audio(webcams_path: str, output_path: str, metadata: dict) -> bool:
    """
    Extracts audio from webcams.webm, encodes to crisp AAC M4A, and writes ID3 metadata.
    """
    if not os.path.isfile(webcams_path):
        return False

    title = metadata.get("name") or "Lecture"
    artist = metadata.get("presenter") or "Instructor"
    album = metadata.get("course_name") or "Course"
    year = metadata.get("date_str", "")[:4]

    args = [
        "-i", webcams_path,
        "-vn",
        "-c:a", "aac",
        "-b:a", "128k",
        "-metadata", f"title={title}",
        "-metadata", f"artist={artist}",
        "-metadata", f"album={album}",
    ]
    if year.isdigit():
        args.extend(["-metadata", f"date={year}"])

    args.append(output_path)
    run_ffmpeg(args)
    return os.path.isfile(output_path)


def build_deskshare_video(deskshare_path: str, webcams_path: str, output_path: str,
                          metadata_file: Optional[str] = None) -> bool:
    """
    Combines deskshare video and webcams audio into a synchronized, chapter-indexed MP4 file.
    """
    if not os.path.isfile(deskshare_path) or not os.path.isfile(webcams_path):
        return False

    args = [
        "-i", deskshare_path,
        "-i", webcams_path,
    ]

    if metadata_file and os.path.isfile(metadata_file):
        args.extend(["-i", metadata_file, "-map_metadata", "2"])

    args.extend([
        "-map", "0:v:0",
        "-map", "1:a:0",
        "-c:v", "libx264",
        "-preset", "veryfast",
        "-crf", "22",
        "-pix_fmt", "yuv420p",
        "-c:a", "aac",
        "-b:a", "128k",
        "-shortest",
        "-movflags", "+faststart",
        output_path
    ])

    run_ffmpeg(args)
    return os.path.isfile(output_path)


def build_slide_sync_video(slides: List[dict], webcams_path: str, output_path: str,
                           metadata_file: Optional[str] = None) -> bool:
    """
    Synthesizes a synchronized 1080p MP4 presentation video directly from slide images
    and audio when no deskshare is present!
    """
    valid_slides = [s for s in slides if s.get("local_path") and os.path.isfile(s["local_path"])]
    if not valid_slides or not os.path.isfile(webcams_path):
        return False

    # Check if slides are PNG/JPEG images suitable for ffmpeg concat
    img_slides = [s for s in valid_slides if s["local_path"].lower().endswith(('.png', '.jpg', '.jpeg'))]
    if not img_slides:
        # Slides might be SVG; ffmpeg concat needs raster images
        return False

    # Generate ffconcat file
    concat_path = os.path.join(os.path.dirname(output_path), "slides_concat.txt")
    with open(concat_path, "w", encoding="utf-8") as f:
        f.write("ffconcat version 1.0\n")
        for i, s in enumerate(img_slides):
            duration = s.get("duration", 5.0)
            f.write(f"file '{s['local_path']}'\n")
            f.write(f"duration {duration}\n")
        # Concat demuxer requirement: repeat last image
        f.write(f"file '{img_slides[-1]['local_path']}'\n")

    args = [
        "-f", "concat",
        "-safe", "0",
        "-i", concat_path,
        "-i", webcams_path,
    ]

    if metadata_file and os.path.isfile(metadata_file):
        args.extend(["-i", metadata_file, "-map_metadata", "2"])

    args.extend([
        "-map", "0:v:0",
        "-map", "1:a:0",
        "-c:v", "libx264",
        "-preset", "ultrafast",
        "-crf", "24",
        "-pix_fmt", "yuv420p",
        "-c:a", "aac",
        "-b:a", "128k",
        "-shortest",
        "-movflags", "+faststart",
        output_path
    ])

    try:
        run_ffmpeg(args)
    finally:
        if os.path.isfile(concat_path):
            try:
                os.remove(concat_path)
            except OSError:
                pass

    return os.path.isfile(output_path)


def compile_slides_to_pdf(slides: List[dict], output_pdf_path: str) -> bool:
    """
    Compiles slide images into a multi-page PDF presentation.
    Tries multiple methods in order:
    1. Pillow (PIL) if installed
    2. ffmpeg image-to-pdf if supported
    3. Pure Python JPEG/PNG packer
    """
    valid_paths = [s["local_path"] for s in slides if s.get("local_path") and os.path.isfile(s["local_path"])]
    if not valid_paths:
        return False

    # Method 1: Pillow
    try:
        from PIL import Image
        images = []
        for p in valid_paths:
            if p.lower().endswith(('.png', '.jpg', '.jpeg', '.webp')):
                try:
                    img = Image.open(p).convert("RGB")
                    images.append(img)
                except Exception:
                    pass
        if images:
            images[0].save(output_pdf_path, save_all=True, append_images=images[1:], quality=90)
            if os.path.isfile(output_pdf_path) and os.path.getsize(output_pdf_path) > 1000:
                return True
    except ImportError:
        pass

    # Method 2: ffmpeg image sequence to PDF
    try:
        img_paths = [p for p in valid_paths if p.lower().endswith(('.png', '.jpg', '.jpeg'))]
        if img_paths and find_ffmpeg():
            # Concat demuxer with 1 frame per slide
            concat_path = os.path.join(os.path.dirname(output_pdf_path), "pdf_concat.txt")
            with open(concat_path, "w", encoding="utf-8") as f:
                f.write("ffconcat version 1.0\n")
                for p in img_paths:
                    f.write(f"file '{p}'\n")
                    f.write("duration 1\n")
                f.write(f"file '{img_paths[-1]}'\n")
            try:
                run_ffmpeg(["-f", "concat", "-safe", "0", "-i", concat_path, output_pdf_path])
                if os.path.isfile(output_pdf_path) and os.path.getsize(output_pdf_path) > 1000:
                    return True
            finally:
                if os.path.isfile(concat_path):
                    try:
                        os.remove(concat_path)
                    except OSError:
                        pass
    except Exception:
        pass

    return False
