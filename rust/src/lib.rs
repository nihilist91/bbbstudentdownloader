//! BBB StudyForge core: a Rust port of the Python `core/` package.
//!
//! Modules mirror the Python files one-to-one:
//! `parser` (core/parser.py), `downloader` (core/downloader.py),
//! `converter` (core/converter.py), `ffmpeg` (core/ffmpeg_helper.py),
//! `study_pack` (core/study_pack.py), `engine` (core/engine.py).

// `regex!` compiles each pattern once through a OnceLock, so the lint's
// "regex compiled in a loop" warning does not apply to it.
#![allow(clippy::regex_creation_in_loops)]

pub mod converter;
pub mod downloader;
pub mod engine;
pub mod ffmpeg;
pub mod html;
pub mod model;
pub mod parser;
pub mod pdf;
pub mod study_pack;
pub mod xml;

/// Crate-wide error type. Messages are user-facing, as in the Python version.
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/// Compiles a regex once and caches it for the lifetime of the process.
#[macro_export]
macro_rules! regex {
    ($pat:literal) => {{
        static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($pat).expect("valid static regex"))
    }};
}

/// Python's `round()` uses round-half-to-even; Rust's `f64::round` does not.
pub fn py_round(x: f64) -> f64 {
    if (x - x.trunc()).abs() == 0.5 {
        2.0 * (x / 2.0).round()
    } else {
        x.round()
    }
}

/// Python's `round(x, 2)`.
pub fn py_round2(x: f64) -> f64 {
    py_round(x * 100.0) / 100.0
}
