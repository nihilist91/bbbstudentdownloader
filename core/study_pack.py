"""
core/study_pack.py - Generates rich Markdown transcripts, interactive offline HTML chat viewer,
interactive slide deck viewer, and AI study companion packages.
"""
import html
import re
from typing import List, Dict, Any


def generate_chat_markdown(metadata: dict, messages: List[dict], links: List[str]) -> str:
    """Generates a clean, readable GitHub-style Markdown transcript of the lecture chat."""
    title = metadata.get("name") or "Lecture Chat"
    course = metadata.get("course_name") or "Course"
    date_str = metadata.get("date_str") or "Unknown Date"
    presenter = metadata.get("presenter") or "Instructor"
    duration = metadata.get("duration_formatted") or ""

    lines = [
        f"# 💬 Public Chat Transcript - {course}",
        f"**Session:** {title}  ",
        f"**Date:** {date_str} | **Instructor:** {presenter} | **Duration:** {duration}  ",
        f"**Total Messages:** {len(messages)}  ",
        "",
        "---",
        "",
    ]

    if links:
        lines.append("## 🔗 Shared Links & Resources")
        for u in links:
            lines.append(f"- [{u}]({u})")
        lines.append("")
        lines.append("---")
        lines.append("")

    lines.append("## 📜 Message Log")
    lines.append("")
    lines.append("| Time | Participant | Message |")
    lines.append("| :--- | :--- | :--- |")

    for m in messages:
        ts = f"`{m['time_str']}`"
        sender = f"**{m['sender']}**"
        text = m['message'].replace("\n", " ")
        if m.get("is_question"):
            text = f"❓ *{text}*"
        lines.append(f"| {ts} | {sender} | {text} |")

    lines.append("")
    lines.append("---")
    lines.append("*Generated automatically by BigBlueButton StudyForge.*")
    return "\n".join(lines)


def generate_interactive_chat_html(metadata: dict, messages: List[dict], links: List[str]) -> str:
    """
    Generates a self-contained, offline interactive HTML chat viewer with real-time search,
    participant filters, question highlights, and dark-mode styling.
    Zero external dependencies (works offline without internet).
    """
    title = html.escape(metadata.get("name") or "Lecture Chat")
    course = html.escape(metadata.get("course_name") or "Course")
    date_str = html.escape(metadata.get("date_str") or "")
    presenter = html.escape(metadata.get("presenter") or "Instructor")

    # Collect unique senders
    senders = sorted(list({m["sender"] for m in messages}))

    # JSON serialize messages safely
    import json
    messages_json = json.dumps(messages)
    links_json = json.dumps(links)

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{course} - {title} (Chat Transcript)</title>
<style>
  :root {{
    --bg: #0B1020;
    --card: #141B2D;
    --field: #1C2540;
    --line: #2A3658;
    --accent: #7C5CFF;
    --accent-hover: #9379FF;
    --ok: #34D399;
    --warn: #FBBF24;
    --text: #F1F5F9;
    --muted: #8A94A8;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }}
  body {{ background: var(--bg); color: var(--text); padding: 24px; min-height: 100vh; }}
  .container {{ max-width: 900px; margin: 0 auto; }}
  header {{ background: var(--card); padding: 24px; border-radius: 16px; border: 1px solid var(--line); margin-bottom: 20px; }}
  h1 {{ font-size: 22px; margin-bottom: 6px; color: var(--text); }}
  .sub {{ font-size: 14px; color: var(--muted); }}
  .badge {{ display: inline-block; background: var(--field); color: var(--accent-hover); padding: 4px 10px; border-radius: 8px; font-size: 12px; margin-top: 8px; font-weight: 600; }}
  
  .controls {{ display: flex; gap: 12px; margin-bottom: 20px; flex-wrap: wrap; }}
  .search-box {{ flex: 1; min-width: 240px; position: relative; }}
  .search-box input {{ width: 100%; background: var(--field); border: 1px solid var(--line); border-radius: 12px; color: var(--text); padding: 12px 16px; font-size: 14px; outline: none; }}
  .search-box input:focus {{ border-color: var(--accent); }}
  
  select, button {{ background: var(--field); border: 1px solid var(--line); border-radius: 12px; color: var(--text); padding: 12px 16px; font-size: 14px; cursor: pointer; outline: none; }}
  select:focus, button:hover {{ border-color: var(--accent); }}
  button.active {{ background: var(--accent); border-color: var(--accent); color: #fff; }}
  
  .links-panel {{ background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 16px; margin-bottom: 20px; }}
  .links-title {{ font-size: 13px; font-weight: 700; color: var(--muted); text-transform: uppercase; margin-bottom: 10px; }}
  .links-list {{ display: flex; flex-wrap: wrap; gap: 8px; }}
  .link-pill {{ background: var(--field); color: var(--ok); text-decoration: none; padding: 6px 12px; border-radius: 8px; font-size: 12px; border: 1px solid var(--line); word-break: break-all; }}
  .link-pill:hover {{ background: var(--line); }}

  .chat-stream {{ display: flex; flex-direction: column; gap: 10px; }}
  .msg-card {{ background: var(--card); border: 1px solid var(--line); border-radius: 14px; padding: 14px 18px; transition: transform 0.1s ease; }}
  .msg-card.is-question {{ border-left: 4px solid var(--warn); }}
  .msg-head {{ display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px; font-size: 12px; }}
  .msg-sender {{ font-weight: 700; color: var(--text); display: flex; align-items: center; gap: 8px; }}
  .avatar {{ width: 24px; height: 24px; border-radius: 50%; background: var(--accent); color: #fff; display: inline-flex; align-items: center; justify-content: center; font-size: 10px; font-weight: bold; }}
  .msg-time {{ font-family: monospace; background: var(--field); padding: 2px 8px; border-radius: 6px; color: var(--muted); }}
  .msg-body {{ font-size: 14px; line-height: 1.5; color: #E2E8F0; word-break: break-word; }}
  .msg-body a {{ color: var(--accent-hover); }}
  .highlight {{ background: rgba(251, 191, 36, 0.3); padding: 0 2px; border-radius: 2px; }}
  .empty-state {{ text-align: center; color: var(--muted); padding: 40px; font-size: 15px; }}
</style>
</head>
<body>
<div class="container">
  <header>
    <h1>💬 {title}</h1>
    <div class="sub">Course: <strong>{course}</strong> &bull; Presenter: {presenter} &bull; Date: {date_str}</div>
    <div class="badge" id="statsBadge">{len(messages)} Messages</div>
  </header>

  <div id="linksContainer" class="links-panel" style="display: {'block' if links else 'none'};">
    <div class="links-title">🔗 Shared Links ({len(links)})</div>
    <div class="links-list" id="linksList"></div>
  </div>

  <div class="controls">
    <div class="search-box">
      <input type="text" id="searchInput" placeholder="Search message content or sender..." autocomplete="off">
    </div>
    <select id="senderFilter">
      <option value="">All Participants</option>
      {''.join(f'<option value="{html.escape(s)}">{html.escape(s)}</option>' for s in senders)}
    </select>
    <button id="questionsOnlyBtn" onclick="toggleQuestions()">❓ Questions Only</button>
    <button onclick="copyAll()">📋 Copy All</button>
  </div>

  <div class="chat-stream" id="chatStream"></div>
  <div class="empty-state" id="emptyState" style="display:none;">No messages match your search filter.</div>
</div>

<script>
  const messages = {messages_json};
  const links = {links_json};
  let filterQuestions = false;

  const stream = document.getElementById("chatStream");
  const searchInput = document.getElementById("searchInput");
  const senderFilter = document.getElementById("senderFilter");
  const qBtn = document.getElementById("questionsOnlyBtn");
  const emptyState = document.getElementById("emptyState");
  const statsBadge = document.getElementById("statsBadge");

  // Render Links
  const linksList = document.getElementById("linksList");
  links.forEach(u => {{
    const a = document.createElement("a");
    a.className = "link-pill";
    a.href = u;
    a.target = "_blank";
    a.textContent = u;
    linksList.appendChild(a);
  }});

  function getInitials(name) {{
    return (name || "?").split(" ").map(w => w[0]).slice(0, 2).join("").toUpperCase();
  }}

  function render() {{
    const query = searchInput.value.trim().toLowerCase();
    const sender = senderFilter.value;
    stream.innerHTML = "";
    let visibleCount = 0;

    messages.forEach(m => {{
      if (filterQuestions && !m.is_question) return;
      if (sender && m.sender !== sender) return;
      if (query && !m.message.toLowerCase().includes(query) && !m.sender.toLowerCase().includes(query)) return;

      visibleCount++;
      const card = document.createElement("div");
      card.className = "msg-card" + (m.is_question ? " is-question" : "");

      let bodyHtml = m.message
        .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
        .replace(/(https?:\\/\\/[^\\s<>]+)/g, '<a href="$1" target="_blank">$1<\\/a>');

      if (query) {{
        const reg = new RegExp("(" + query.replace(/[-\\/\\\\^$*+?.()|[\\]{{}}]/g, '\\\\$&') + ")", "gi");
        bodyHtml = bodyHtml.replace(reg, '<span class="highlight">$1<\\/span>');
      }}

      card.innerHTML = `
        <div class="msg-head">
          <div class="msg-sender">
            <span class="avatar">${{getInitials(m.sender)}}</span>
            <span>${{m.sender}}</span>
          </div>
          <span class="msg-time">${{m.time_str}}</span>
        </div>
        <div class="msg-body">${{bodyHtml}}</div>
      `;
      stream.appendChild(card);
    }});

    emptyState.style.display = visibleCount === 0 ? "block" : "none";
    statsBadge.textContent = `${{visibleCount}} of ${{messages.length}} Messages`;
  }}

  function toggleQuestions() {{
    filterQuestions = !filterQuestions;
    qBtn.classList.toggle("active", filterQuestions);
    render();
  }}

  function copyAll() {{
    const text = messages.map(m => `[${{m.time_str}}] ${{m.sender}}: ${{m.message}}`).join("\\n");
    navigator.clipboard.writeText(text).then(() => alert("Copied all messages to clipboard!"));
  }}

  searchInput.addEventListener("input", render);
  senderFilter.addEventListener("change", render);
  render();
</script>
</body>
</html>
"""


def generate_slide_deck_html(metadata: dict, slide_files: List[dict]) -> str:
    """
    Generates a full-featured offline presentation deck viewer in HTML.
    Supports keyboard navigation (Left/Right arrows, Space), fullscreen,
    thumbnail drawer, and a print stylesheet (@media print) allowing 1-click
    export to PDF directly from any web browser!
    """
    title = html.escape(metadata.get("name") or "Lecture Presentation")
    course = html.escape(metadata.get("course_name") or "Course")
    total_slides = len(slide_files)

    slide_divs = []
    thumb_divs = []

    for s in slide_files:
        idx = s.get("index", 1)
        rel_path = html.escape(s.get("local_filename") or s.get("href") or "")
        time_in = s.get("in", 0.0)
        dur = s.get("duration", 0.0)
        time_str = f"{int(time_in)//60:02d}:{int(time_in)%60:02d}"

        # Slide container
        slide_divs.append(f"""
        <div class="slide-item {'active' if idx==1 else ''}" id="slide-{idx}" data-index="{idx}">
          <div class="slide-header">Slide {idx} of {total_slides} &bull; Timestamp: {time_str}</div>
          <div class="slide-media">
            <img src="{rel_path}" alt="Slide {idx}" loading="lazy">
          </div>
        </div>
        """)

        # Thumbnail
        thumb_divs.append(f"""
        <div class="thumb-item {'active' if idx==1 else ''}" onclick="goToSlide({idx})">
          <img src="{rel_path}" alt="Slide {idx}">
          <div class="thumb-label">#{idx} ({time_str})</div>
        </div>
        """)

    return f"""<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{course} - {title} (Slides)</title>
<style>
  :root {{
    --bg: #0B1020;
    --card: #141B2D;
    --line: #2A3658;
    --accent: #7C5CFF;
    --text: #F1F5F9;
    --muted: #8A94A8;
  }}
  * {{ box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }}
  body {{ background: var(--bg); color: var(--text); overflow-x: hidden; }}
  
  .top-bar {{
    position: fixed; top: 0; left: 0; right: 0; height: 56px;
    background: var(--card); border-bottom: 1px solid var(--line);
    display: flex; align-items: center; justify-content: space-between; padding: 0 20px;
    z-index: 100;
  }}
  .top-title {{ font-size: 15px; font-weight: 700; color: var(--text); }}
  .top-actions {{ display: flex; gap: 10px; align-items: center; }}
  button {{
    background: #1C2540; border: 1px solid var(--line); color: var(--text);
    padding: 8px 14px; border-radius: 8px; font-size: 13px; cursor: pointer;
  }}
  button:hover {{ border-color: var(--accent); }}
  .btn-accent {{ background: var(--accent); color: #fff; border-color: var(--accent); font-weight: 600; }}

  .stage {{
    margin-top: 56px; margin-bottom: 90px; display: flex; justify-content: center;
    align-items: center; min-height: calc(100vh - 146px); padding: 20px;
  }}
  .slide-item {{ display: none; width: 100%; max-width: 1200px; text-align: center; }}
  .slide-item.active {{ display: block; }}
  .slide-header {{ font-size: 13px; color: var(--muted); margin-bottom: 10px; font-weight: 600; }}
  .slide-media img {{
    max-width: 100%; max-height: 75vh; border-radius: 12px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.5); border: 1px solid var(--line);
    background: #fff;
  }}

  .nav-bar {{
    position: fixed; bottom: 0; left: 0; right: 0; height: 70px;
    background: var(--card); border-top: 1px solid var(--line);
    display: flex; align-items: center; justify-content: center; gap: 20px; z-index: 100;
  }}
  .nav-info {{ font-size: 14px; font-weight: 600; color: var(--text); min-width: 120px; text-align: center; }}

  /* Print Stylesheet for 1-Click PDF Export */
  @media print {{
    .top-bar, .nav-bar {{ display: none !important; }}
    .stage {{ margin: 0; padding: 0; display: block; min-height: auto; }}
    .slide-item {{ display: block !important; page-break-after: always; break-after: page; width: 100vw; height: 100vh; display: flex !important; flex-direction: column; justify-content: center; align-items: center; }}
    .slide-media img {{ max-height: 90vh; max-width: 95vw; box-shadow: none; border: none; }}
  }}
</style>
</head>
<body>
<div class="top-bar">
  <div class="top-title">{course} - {title}</div>
  <div class="top-actions">
    <button onclick="window.print()" class="btn-accent">🖨️ Print / Save as PDF</button>
  </div>
</div>

<div class="stage">
  {"".join(slide_divs)}
</div>

<div class="nav-bar">
  <button onclick="prevSlide()">◀ Previous (←)</button>
  <div class="nav-info" id="navCounter">Slide 1 of {total_slides}</div>
  <button onclick="nextSlide()">Next (→) ▶</button>
</div>

<script>
  let current = 1;
  const total = {total_slides};

  function update() {{
    document.querySelectorAll(".slide-item").forEach(el => {{
      el.classList.toggle("active", parseInt(el.dataset.index) === current);
    }});
    document.getElementById("navCounter").textContent = `Slide ${{current}} of ${{total}}`;
  }}

  function nextSlide() {{
    if (current < total) {{ current++; update(); }}
  }}
  function prevSlide() {{
    if (current > 1) {{ current--; update(); }}
  }}
  function goToSlide(n) {{
    current = Math.max(1, Math.min(total, n));
    update();
  }}

  document.addEventListener("keydown", e => {{
    if (e.key === "ArrowRight" || e.key === " " || e.key === "PageDown") nextSlide();
    else if (e.key === "ArrowLeft" || e.key === "PageUp") prevSlide();
    else if (e.key === "Home") goToSlide(1);
    else if (e.key === "End") goToSlide(total);
  }});
</script>
</body>
</html>
"""


def generate_ai_study_companion(metadata: dict, slides: List[dict], messages: List[dict],
                                 questions: List[dict], slide_texts: Dict[str, str]) -> str:
    """
    Generates a structured, AI-ready Markdown package that students can drop into
    NotebookLM, ChatGPT, or Claude to generate flashcards, practice exams, and summaries.
    """
    title = metadata.get("name") or "Lecture Study Pack"
    course = metadata.get("course_name") or "Course"
    date_str = metadata.get("date_str") or "Unknown Date"
    presenter = metadata.get("presenter") or "Instructor"
    duration = metadata.get("duration_formatted") or ""

    lines = [
        f"# 🎓 AI Study Companion: {course}",
        f"**Session:** {title}  ",
        f"**Date:** {date_str} | **Instructor:** {presenter} | **Duration:** {duration}  ",
        "",
        "---",
        "",
        "## 💡 Ready-to-Use AI Study Prompts",
        "Copy and paste any of these prompts into **NotebookLM**, **ChatGPT**, or **Claude** alongside this document:",
        "",
        "### 🧠 Prompt 1: High-Yield Anki Flashcards",
        "> \"Act as a university professor in this field. Using the lecture content, slide texts, and Q&A below, create 15 high-yield Anki flashcards. Format as Front (Concept/Question) and Back (Clear explanation, formula, or definition). Focus on key principles, edge cases, and definitions likely to be on exams.\"",
        "",
        "### 📝 Prompt 2: Practice Exam with Distractors",
        "> \"Create a 10-question multiple-choice practice quiz based strictly on this lecture. For each question, provide 4 options (A-D) with realistic distractors, indicate the correct answer, and provide a 2-sentence explanation of why it is correct.\"",
        "",
        "### ⚡ Prompt 3: 5-Minute Executive Summary & Cheat Sheet",
        "> \"Synthesize this entire lecture into a 1-page structured cheat sheet: include core objectives, the 3 most crucial theorems/concepts, any equations with variable definitions, and a list of common student misconceptions clarified during the session.\"",
        "",
        "---",
        "",
        "## 📌 Slide & Chapter Timeline",
    ]

    for s in slides:
        idx = s.get("index", 1)
        time_in = s.get("in", 0.0)
        time_str = f"{int(time_in)//60:02d}:{int(time_in)%60:02d}"
        txt = slide_texts.get(str(idx)) or slide_texts.get(f"slide-{idx}") or ""
        snippet = f": *{txt[:80]}...*" if txt else ""
        lines.append(f"- **Slide {idx}** `[{time_str}]`{snippet}")

    lines.append("")
    lines.append("---")
    lines.append("")

    if slide_texts:
        lines.append("## 📑 Slide Text Transcripts")
        lines.append("")
        for sid, text in slide_texts.items():
            lines.append(f"### Slide {sid}")
            lines.append(f"> {text}")
            lines.append("")
        lines.append("---")
        lines.append("")

    if questions:
        lines.append("## ❓ Live Classroom Q&A Highlights")
        lines.append("Key questions asked by students during the live lecture:")
        lines.append("")
        for q in questions:
            lines.append(f"- `[{q['time_str']}]` **{q['sender']}**: *{q['message']}*")
        lines.append("")
        lines.append("---")

    lines.append("*Study Companion generated by BigBlueButton StudyForge.*")
    return "\n".join(lines)
