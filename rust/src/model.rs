//! Plain data types shared across the pipeline (the Python code used dicts).

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub struct Metadata {
    pub meeting_id: String,
    pub name: String,
    pub course_name: String,
    pub presenter: String,
    pub date_str: String,
    pub date_iso: String,
    pub duration_seconds: f64,
    pub duration_formatted: String,
    pub participants: u64,
    pub description: String,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            meeting_id: String::new(),
            name: String::new(),
            course_name: String::new(),
            presenter: String::new(),
            date_str: String::new(),
            date_iso: String::new(),
            duration_seconds: 0.0,
            duration_formatted: "Unknown duration".to_string(),
            participants: 0,
            description: String::new(),
        }
    }
}

/// One slide from shapes.svg, plus download and text info once fetched.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Slide {
    pub index: usize,
    pub href: String,
    /// Display start time in seconds (Python key "in").
    pub start: f64,
    /// Display end time in seconds (Python key "out").
    pub end: f64,
    pub duration: f64,
    pub width: i64,
    pub height: i64,
    pub local_path: Option<PathBuf>,
    pub local_filename: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct ChatMessage {
    pub time_sec: f64,
    pub time_str: String,
    pub sender: String,
    pub message: String,
    pub is_question: bool,
    pub urls: Vec<String>,
}

/// Which BigBlueButton assets exist on the server.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Assets {
    pub metadata: bool,
    pub webcams_webm: bool,
    pub webcams_mp4: bool,
    pub deskshare_webm: bool,
    pub deskshare_mp4: bool,
    pub shapes_svg: bool,
    pub chat_xml: bool,
    pub slides_new_xml: bool,
    pub presentation_text_json: bool,
    pub notes_html: bool,
    pub cursor_xml: bool,
    pub panzooms_xml: bool,
    pub has_audio: bool,
    pub has_deskshare: bool,
    pub has_slides: bool,
    pub has_chat: bool,
}
