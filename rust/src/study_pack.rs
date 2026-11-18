//! Port of core/study_pack.py: Markdown transcript, HTML chat viewer, HTML slide deck,
//! and the AI study companion.

use std::collections::BTreeSet;

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::html::escape;
use crate::model::{ChatMessage, Metadata, Slide};

const CHAT_VIEWER_TEMPLATE: &str = include_str!("templates/chat_viewer.html");
const SLIDE_DECK_TEMPLATE: &str = include_str!("templates/slide_deck.html");

/// Serializes JSON for embedding in a <script> block (escapes "</" so it cannot close the tag).
fn json_for_script(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "null".into()).replace("</", "<\\/")
}

/// "mm:ss" of a time in seconds, truncated like Python's int().
fn mmss(seconds: f64) -> String {
    let t = seconds as i64;
    format!("{:02}:{:02}", t / 60, t % 60)
}

pub fn generate_chat_markdown(meta: &Metadata, messages: &[ChatMessage], links: &[String]) -> String {
    let title = non_empty(&meta.name, "Lecture Chat");
    let course = non_empty(&meta.course_name, "Course");
    let date = non_empty(&meta.date_str, "Unknown Date");
    let presenter = non_empty(&meta.presenter, "Instructor");

    let mut lines: Vec<String> = vec![
        format!("# 💬 Public Chat Transcript - {course}"),
        format!("**Session:** {title}  "),
        format!("**Date:** {date} | **Instructor:** {presenter} | **Duration:** {}  ", meta.duration_formatted),
        format!("**Total Messages:** {}  ", messages.len()),
        String::new(),
        "---".into(),
        String::new(),
    ];

    if !links.is_empty() {
        lines.push("## 🔗 Shared Links & Resources".into());
        for u in links {
            lines.push(format!("- [{u}]({u})"));
        }
        lines.extend(["".into(), "---".into(), "".into()]);
    }

    lines.extend([
        "## 📜 Message Log".into(),
        String::new(),
        "| Time | Participant | Message |".into(),
        "| :--- | :--- | :--- |".into(),
    ]);
    for m in messages {
        let mut text = m.message.replace('\n', " ");
        if m.is_question {
            text = format!("❓ *{text}*");
        }
        lines.push(format!("| `{}` | **{}** | {} |", m.time_str, m.sender, text));
    }
    lines.extend(["".into(), "---".into(), "*Generated automatically by BigBlueButton StudyForge.*".into()]);
    lines.join("\n")
}

pub fn generate_interactive_chat_html(meta: &Metadata, messages: &[ChatMessage], links: &[String]) -> String {
    let title = escape(non_empty(&meta.name, "Lecture Chat"));
    let course = escape(non_empty(&meta.course_name, "Course"));
    let date = escape(&meta.date_str);
    let presenter = escape(non_empty(&meta.presenter, "Instructor"));

    let senders: BTreeSet<&str> = messages.iter().map(|m| m.sender.as_str()).collect();
    let sender_options: String = senders
        .iter()
        .map(|s| format!(r#"<option value="{0}">{0}</option>"#, escape(s)))
        .collect::<Vec<_>>()
        .join("");

    let messages_json = json_for_script(&serde_json::to_value(messages).unwrap_or(Value::Null));
    let links_json = json_for_script(&Value::from(links.to_vec()));

    fill(
        CHAT_VIEWER_TEMPLATE,
        &[
            ("PAGE_TITLE", format!("{course} - {title} (Chat Transcript)")),
            ("TITLE", title.clone()),
            ("COURSE", course),
            ("PRESENTER", presenter),
            ("DATE", date),
            ("MSG_COUNT", messages.len().to_string()),
            ("LINKS_DISPLAY", if links.is_empty() { "none".into() } else { "block".into() }),
            ("LINKS_COUNT", links.len().to_string()),
            ("SENDER_OPTIONS", sender_options),
            ("MESSAGES_JSON", messages_json),
            ("LINKS_JSON", links_json),
        ],
    )
}

/// `slides` must carry `local_filename` (or `href`) and `text`.
pub fn generate_slide_deck_html(meta: &Metadata, slides: &[Slide]) -> String {
    let title = escape(non_empty(&meta.name, "Lecture Presentation"));
    let course = escape(non_empty(&meta.course_name, "Course"));
    let total = slides.len();

    let mut slide_divs = String::new();
    let mut texts: Map<String, Value> = Map::new();
    for s in slides {
        let idx = s.index;
        texts.insert(idx.to_string(), Value::from(s.text.clone()));
        let src = escape(s.local_filename.as_deref().unwrap_or(&s.href));
        let active = if idx == 1 { " active" } else { "" };
        slide_divs.push_str(&format!(
            r#"<div class="slide-item{active}" id="slide-{idx}" data-index="{idx}">
          <div class="slide-header">Slide {idx} of {total} &bull; Timestamp: {ts}</div>
          <div class="slide-media">
            <img src="{src}" alt="Slide {idx}" loading="lazy">
          </div>
        </div>
        "#,
            ts = mmss(s.start),
        ));
    }

    fill(
        SLIDE_DECK_TEMPLATE,
        &[
            ("PAGE_TITLE", format!("{course} - {title} (Slides)")),
            ("TOP_TITLE", format!("{course} - {title}")),
            ("TOTAL", total.to_string()),
            ("SLIDES", slide_divs),
            ("SLIDE_TEXTS", json_for_script(&Value::Object(texts))),
        ],
    )
}

pub fn generate_ai_study_companion(
    meta: &Metadata,
    slides: &[Slide],
    _messages: &[ChatMessage],
    questions: &[ChatMessage],
    slide_texts: &IndexMap<String, String>,
) -> String {
    let title = non_empty(&meta.name, "Lecture Study Pack");
    let course = non_empty(&meta.course_name, "Course");
    let date = non_empty(&meta.date_str, "Unknown Date");
    let presenter = non_empty(&meta.presenter, "Instructor");

    let mut lines: Vec<String> = vec![
        format!("# 🎓 AI Study Companion: {course}"),
        format!("**Session:** {title}  "),
        format!("**Date:** {date} | **Instructor:** {presenter} | **Duration:** {}  ", meta.duration_formatted),
        String::new(),
        "---".into(),
        String::new(),
        "## 💡 Ready-to-Use AI Study Prompts".into(),
        "Copy and paste any of these prompts into **NotebookLM**, **ChatGPT**, or **Claude** alongside this document:".into(),
        String::new(),
        "### 🧠 Prompt 1: High-Yield Anki Flashcards".into(),
        "> \"Act as a university professor in this field. Using the lecture content, slide texts, and Q&A below, create 15 high-yield Anki flashcards. Format as Front (Concept/Question) and Back (Clear explanation, formula, or definition). Focus on key principles, edge cases, and definitions likely to be on exams.\"".into(),
        String::new(),
        "### 📝 Prompt 2: Practice Exam with Distractors".into(),
        "> \"Create a 10-question multiple-choice practice quiz based strictly on this lecture. For each question, provide 4 options (A-D) with realistic distractors, indicate the correct answer, and provide a 2-sentence explanation of why it is correct.\"".into(),
        String::new(),
        "### ⚡ Prompt 3: 5-Minute Executive Summary & Cheat Sheet".into(),
        "> \"Synthesize this entire lecture into a 1-page structured cheat sheet: include core objectives, the 3 most crucial theorems/concepts, any equations with variable definitions, and a list of common student misconceptions clarified during the session.\"".into(),
        String::new(),
        "---".into(),
        String::new(),
        "## 📌 Slide & Chapter Timeline".into(),
    ];

    for s in slides {
        let snippet = match slide_texts
            .get(&s.index.to_string())
            .or_else(|| slide_texts.get(&format!("slide-{}", s.index)))
        {
            Some(txt) if !txt.is_empty() => format!(": *{}...*", txt.chars().take(80).collect::<String>()),
            _ => String::new(),
        };
        lines.push(format!("- **Slide {}** `[{}]`{snippet}", s.index, mmss(s.start)));
    }
    lines.extend(["".into(), "---".into(), "".into()]);

    if !slide_texts.is_empty() {
        lines.extend(["## 📑 Slide Text Transcripts".into(), String::new()]);
        for (sid, text) in slide_texts {
            lines.push(format!("### Slide {sid}"));
            lines.push(format!("> {text}"));
            lines.push(String::new());
        }
        lines.extend(["---".into(), String::new()]);
    }

    if !questions.is_empty() {
        lines.extend([
            "## ❓ Live Classroom Q&A Highlights".into(),
            "Key questions asked by students during the live lecture:".into(),
            String::new(),
        ]);
        for q in questions {
            lines.push(format!("- `[{}]` **{}**: *{}*", q.time_str, q.sender, q.message));
        }
        lines.extend(["".into(), "---".into()]);
    }

    lines.push("*Study Companion generated by BigBlueButton StudyForge.*".into());
    lines.join("\n")
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}

/// Replaces each `@@KEY@@` marker in the template with its value.
fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("@@{key}@@"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Metadata {
        Metadata {
            name: "Lecture 01".into(),
            course_name: "CS101".into(),
            presenter: "Prof. Turing".into(),
            date_str: "2026-11-09".into(),
            duration_formatted: "1h 00m".into(),
            ..Default::default()
        }
    }

    fn msgs() -> Vec<ChatMessage> {
        vec![
            ChatMessage { time_sec: 10.0, time_str: "00:10".into(), sender: "Alice".into(), message: "Good morning!".into(), ..Default::default() },
            ChatMessage { time_sec: 45.0, time_str: "00:45".into(), sender: "Bob".into(), message: "Is this on the exam?".into(), is_question: true, ..Default::default() },
        ]
    }

    fn slides() -> Vec<Slide> {
        vec![
            Slide { index: 1, start: 0.0, end: 60.0, duration: 60.0, href: "slide-1.png".into(), local_filename: Some("slides/slide_001.png".into()), ..Default::default() },
            Slide { index: 2, start: 60.0, end: 120.0, duration: 60.0, href: "slide-2.png".into(), local_filename: Some("slides/slide_002.png".into()), ..Default::default() },
        ]
    }

    #[test]
    fn markdown_transcript() {
        let md = generate_chat_markdown(&meta(), &msgs(), &["https://example.com/syllabus".into()]);
        assert!(md.contains("# 💬 Public Chat Transcript - CS101"));
        assert!(md.contains("`00:45`"));
        assert!(md.contains("❓ *Is this on the exam?*"));
    }

    #[test]
    fn chat_viewer_escapes_script_and_names() {
        let mut m = msgs();
        m[0].sender = "<img onerror=x>".into();
        m[1].message = "evil </script>".into();
        let doc = generate_interactive_chat_html(&meta(), &m, &[]);
        assert!(doc.starts_with("<!DOCTYPE html>"));
        assert!(!doc.contains("</script>\"") && !doc.contains("evil </script>"));
        assert!(doc.contains("&lt;img onerror=x&gt;"));
        assert!(!doc.contains("@@"));
    }

    #[test]
    fn slide_deck_html() {
        let deck = generate_slide_deck_html(&meta(), &slides());
        assert!(deck.contains("slides/slide_001.png"));
        assert!(deck.contains("@media print"));
        assert!(!deck.contains("@@"));
    }

    #[test]
    fn ai_companion() {
        let mut texts = IndexMap::new();
        texts.insert("1".to_string(), "Intro text".to_string());
        let doc = generate_ai_study_companion(&meta(), &slides(), &msgs(), &[msgs()[1].clone()], &texts);
        assert!(doc.contains("# 🎓 AI Study Companion: CS101"));
        assert!(doc.contains("Prompt 1: High-Yield Anki Flashcards"));
        assert!(doc.contains("Intro text"));
    }
}
