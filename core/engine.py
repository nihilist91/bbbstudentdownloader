"""
core/engine.py - The central coordinator that downloads, processes, and packages
BigBlueButton recordings into complete study suites, featuring smart lecture end-trimming.
"""
import os
import shutil
import traceback
from typing import Dict, Any, Callable, Optional, List

from core.parser import (
    parse_bbb_url, parse_metadata, parse_shapes_svg,
    parse_chat_xml, parse_presentation_text, safe_filename,
    detect_lecture_end_time, format_duration
)
from core.downloader import (
    fetch_text, fetch_file, probe_recording_assets,
    download_all_slides
)
from core.converter import (
    build_audio, build_deskshare_video, build_slide_sync_video,
    generate_ffmetadata_file, compile_slides_to_pdf
)
from core.study_pack import (
    generate_chat_markdown, generate_interactive_chat_html,
    generate_slide_deck_html, generate_ai_study_companion
)
from core.ffmpeg_helper import find_ffmpeg


def inspect_recording(url: str, cookie: Optional[str] = None) -> Dict[str, Any]:
    """
    Rapidly probes a BigBlueButton recording to extract metadata and detect all available tracks,
    including smart detection of active class duration vs trailing dead time.
    """
    meeting_id, base_url, _ = parse_bbb_url(url)
    meta_xml = fetch_text(f"{base_url}/metadata.xml", cookie=cookie, timeout=10)
    metadata = parse_metadata(meta_xml, meeting_id_fallback=meeting_id)

    assets = probe_recording_assets(base_url, cookie=cookie)

    slides = []
    slide_count = 0
    if assets["shapes_svg"]:
        svg_text = fetch_text(f"{base_url}/shapes.svg", cookie=cookie, timeout=10)
        slides = parse_shapes_svg(svg_text)
        slide_count = len(slides)

    msgs = []
    chat_count = 0
    if assets["chat_xml"]:
        chat_xml = fetch_text(f"{base_url}/chat.xml", cookie=cookie, timeout=8)
        msgs, _, _ = parse_chat_xml(chat_xml)
        chat_count = len(msgs)

    cut_sec, has_dead, dead_sec = detect_lecture_end_time(metadata["duration_seconds"], slides, msgs)

    return {
        "meeting_id": meeting_id,
        "base_url": base_url,
        "name": metadata["name"] or "Untitled Lecture",
        "course_name": metadata["course_name"] or "Course",
        "presenter": metadata["presenter"] or "Instructor",
        "date_str": metadata["date_str"],
        "duration_seconds": metadata["duration_seconds"],
        "duration_formatted": metadata["duration_formatted"],
        "participants": metadata["participants"],
        "assets": assets,
        "slide_count": slide_count,
        "chat_count": chat_count,
        "detected_cut_seconds": cut_sec,
        "detected_cut_formatted": format_duration(cut_sec),
        "has_dead_time": has_dead,
        "dead_time_seconds": dead_sec,
        "dead_time_formatted": format_duration(dead_sec),
    }


def process_recording(url: str, output_parent_dir: str,
                      options: Optional[Dict[str, Any]] = None,
                      cookie: Optional[str] = None,
                      log_callback: Optional[Callable[[str], None]] = None,
                      progress_callback: Optional[Callable[[float], None]] = None,
                      stage_callback: Optional[Callable[[int, str], None]] = None) -> str:
    """
    Main job that executes the end-to-end download, smart trimming, and synthesis workflow.
    """
    def log(m):
        if log_callback:
            log_callback(m)

    def prog(p):
        if progress_callback:
            progress_callback(min(1.0, max(0.0, p)))

    def stage(num, desc):
        if stage_callback:
            stage_callback(num, desc)

    opts = {
        "save_video": True,
        "save_audio": True,
        "save_slides": True,
        "save_chat": True,
        "save_study_pack": True,
        "auto_trim": True,
        "custom_cut_seconds": None,
    }
    if options:
        opts.update(options)

    # 1. Parse URL & create staging folder
    meeting_id, base_url, _ = parse_bbb_url(url)
    work_dir = os.path.join(output_parent_dir, f"__work_{meeting_id}")
    os.makedirs(work_dir, exist_ok=True)

    try:
        stage(0, "Inspecting recording & fetching manifests...")
        log(f"Inspecting BigBlueButton session: {meeting_id}")
        prog(0.05)

        # Fetch metadata
        meta_xml = fetch_text(f"{base_url}/metadata.xml", cookie=cookie)
        metadata = parse_metadata(meta_xml, meeting_id_fallback=meeting_id)

        course_clean = safe_filename(metadata["course_name"])
        title_clean = safe_filename(metadata["name"] or "Lecture")
        folder_base_name = f"{course_clean} - {title_clean}" if course_clean != title_clean else course_clean

        log(f"Course: {metadata['course_name']}")
        log(f"Session: {metadata['name']}")
        log(f"Duration: {metadata['duration_formatted']} | Date: {metadata['date_str']}")
        prog(0.12)

        # Discover & download shapes.svg
        stage(1, "Analyzing presentation slides & timeline...")
        slides = []
        svg_text = fetch_text(f"{base_url}/shapes.svg", cookie=cookie)
        if svg_text:
            slides = parse_shapes_svg(svg_text)
            log(f"Discovered {len(slides)} presentation slides with timestamps.")
        else:
            log("No shapes.svg found (session may have used only screen share).")
        prog(0.20)

        # Fetch chat.xml & presentation text
        chat_messages = []
        chat_links = []
        chat_questions = []
        chat_xml = fetch_text(f"{base_url}/chat.xml", cookie=cookie)
        if not chat_xml:
            chat_xml = fetch_text(f"{base_url}/slides_new.xml", cookie=cookie)
        if chat_xml:
            chat_messages, chat_links, chat_questions = parse_chat_xml(chat_xml)
            log(f"Extracted {len(chat_messages)} chat messages ({len(chat_questions)} questions).")

        pres_text_json = fetch_text(f"{base_url}/presentation_text.json", cookie=cookie)
        slide_texts = parse_presentation_text(pres_text_json)

        # Smart Lecture End Trimming Calculation
        active_cut = None
        custom_cut = opts.get("custom_cut_seconds")
        if custom_cut is not None and float(custom_cut) > 0:
            active_cut = float(custom_cut)
            log(f"✂️ Trimming lecture at user-specified cut point: {format_duration(active_cut)}")
        elif opts.get("auto_trim", True):
            cut_sec, has_dead, dead_sec = detect_lecture_end_time(metadata["duration_seconds"], slides, chat_messages)
            if has_dead:
                active_cut = cut_sec
                log(f"✂️ Detected {format_duration(dead_sec)} of inactive dead time after class ended. Auto-trimming to {format_duration(active_cut)}.")

        prog(0.28)

        # Stage 2: Download Media
        stage(2, "Downloading audio & video streams...")
        webcams_path = os.path.join(work_dir, "webcams.webm")
        deskshare_path = os.path.join(work_dir, "deskshare.webm")

        has_webcams = fetch_file(f"{base_url}/video/webcams.webm", webcams_path, cookie=cookie,
                                 progress_callback=lambda f, d, t: prog(0.28 + f * 0.20))
        if not has_webcams:
            webcams_mp4 = os.path.join(work_dir, "webcams.mp4")
            if fetch_file(f"{base_url}/video/webcams.mp4", webcams_mp4, cookie=cookie):
                webcams_path = webcams_mp4
                has_webcams = True

        prog(0.48)
        has_deskshare = fetch_file(f"{base_url}/deskshare/deskshare.webm", deskshare_path, cookie=cookie,
                                   progress_callback=lambda f, d, t: prog(0.48 + f * 0.15))
        if not has_deskshare:
            deskshare_mp4 = os.path.join(work_dir, "deskshare.mp4")
            if fetch_file(f"{base_url}/deskshare/deskshare.mp4", deskshare_mp4, cookie=cookie):
                deskshare_path = deskshare_mp4
                has_deskshare = True

        prog(0.63)

        if not has_webcams and not has_deskshare:
            raise RuntimeError("No audio or video streams could be downloaded. "
                               "If this lecture requires university authentication, please provide session cookies.")

        # Download Slide Images if slides requested
        slide_objects = []
        if slides and opts["save_slides"]:
            log(f"Downloading {len(slides)} slide assets...")
            slide_objects = download_all_slides(base_url, slides, work_dir, cookie=cookie,
                                                progress_callback=lambda f, msg: prog(0.63 + f * 0.10))
        prog(0.73)

        # Stage 3: Audio synthesis
        stage(3, "Processing audio & ID3 tags...")
        audio_dest = os.path.join(work_dir, f"{folder_base_name} (audio).m4a")
        have_ffmpeg = find_ffmpeg() is not None

        if opts["save_audio"] and has_webcams:
            if have_ffmpeg:
                log("Encoding high-quality AAC audio with metadata...")
                build_audio(webcams_path, audio_dest, metadata, cut_duration=active_cut)
                log("Audio ready.")
            else:
                log("FFmpeg missing; preserving raw audio stream.")
                shutil.copyfile(webcams_path, os.path.join(work_dir, f"{folder_base_name}.webm"))
        prog(0.80)

        # Stage 4: Video synthesis (with Chapter injection and smart trimming!)
        stage(4, "Synthesizing lecture video & chapters...")
        video_dest = os.path.join(work_dir, f"{folder_base_name}.mp4")

        # Generate chapter file if slides exist
        chapters_file = None
        if slides:
            chapters_file = os.path.join(work_dir, "chapters.txt")
            generate_ffmetadata_file(metadata, slides, chapters_file, cut_duration=active_cut)

        if opts["save_video"] and have_ffmpeg:
            if has_deskshare:
                log("Building screen-share video with synchronized chapters...")
                build_deskshare_video(deskshare_path, webcams_path, video_dest,
                                      metadata_file=chapters_file, cut_duration=active_cut)
                log("Deskshare video created.")
            elif slide_objects:
                log("No deskshare stream found. Synthesizing video from presentation slides...")
                ok_slide_vid = build_slide_sync_video(slide_objects, webcams_path, video_dest,
                                                      metadata_file=chapters_file, cut_duration=active_cut)
                if ok_slide_vid:
                    log("Slide-synchronized video created successfully!")
                else:
                    log("Slide video synthesis skipped (slides are vector-only or incomplete).")
            else:
                log("No video track or slides available to construct video.")
        prog(0.88)

        # Stage 5: Study Pack Generation (Transcripts, HTML Deck, AI Study Guide)
        stage(5, "Generating study pack & transcripts...")

        # 1. Chat outputs
        if opts["save_chat"] and chat_messages:
            chat_md = generate_chat_markdown(metadata, chat_messages, chat_links)
            with open(os.path.join(work_dir, f"{folder_base_name} - Chat.md"), "w", encoding="utf-8") as f:
                f.write(chat_md)

            chat_html = generate_interactive_chat_html(metadata, chat_messages, chat_links)
            with open(os.path.join(work_dir, f"{folder_base_name} - Chat_Viewer.html"), "w", encoding="utf-8") as f:
                f.write(chat_html)
            log("Interactive chat viewer generated.")

        # 2. Slide Deck outputs
        if opts["save_slides"] and slide_objects:
            deck_html = generate_slide_deck_html(metadata, slide_objects)
            with open(os.path.join(work_dir, f"{folder_base_name} - Slides_Viewer.html"), "w", encoding="utf-8") as f:
                f.write(deck_html)

            pdf_dest = os.path.join(work_dir, f"{folder_base_name} - Slides.pdf")
            if compile_slides_to_pdf(slide_objects, pdf_dest):
                log("Compiled presentation slides into PDF.")

        # 3. AI Study Companion
        if opts["save_study_pack"]:
            ai_doc = generate_ai_study_companion(metadata, slides, chat_messages, chat_questions, slide_texts)
            with open(os.path.join(work_dir, f"{folder_base_name} - AI_Study_Companion.md"), "w", encoding="utf-8") as f:
                f.write(ai_doc)
            log("AI Study Companion generated.")

        prog(0.95)

        # Stage 6: Finalize & organize folder
        stage(6, "Tidying up...")
        for temp_file in ["webcams.webm", "webcams.mp4", "deskshare.webm", "deskshare.mp4", "chapters.txt", "slides_concat.txt"]:
            tp = os.path.join(work_dir, temp_file)
            if os.path.isfile(tp):
                try:
                    os.remove(tp)
                except OSError:
                    pass

        # Final destination folder
        final_dir = os.path.join(output_parent_dir, folder_base_name)
        idx = 2
        while os.path.exists(final_dir):
            final_dir = os.path.join(output_parent_dir, f"{folder_base_name} ({idx})")
            idx += 1

        os.replace(work_dir, final_dir)
        prog(1.0)
        log(f"All files saved to: {final_dir}")
        return final_dir

    except Exception as e:
        if os.path.exists(work_dir):
            try:
                shutil.rmtree(work_dir)
            except OSError:
                pass
        raise e
