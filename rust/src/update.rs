//! Checks GitHub for a newer Rust release (tags named `rust-v<version>`).

use std::time::Duration;

use serde_json::Value;

pub const RELEASES_API: &str = "https://api.github.com/repos/nihilist91/bbbstudentdownloader/releases?per_page=30";
pub const RELEASES_PAGE: &str = "https://github.com/nihilist91/bbbstudentdownloader/releases";

/// A release newer than the running version.
#[derive(Debug, Clone, PartialEq)]
pub struct NewRelease {
    pub version: String,
    pub url: String,
}

/// "2.10.1" -> [2, 10, 1]. Returns None for anything that is not dotted numbers.
fn parse_version(v: &str) -> Option<Vec<u64>> {
    let parts: Option<Vec<u64>> = v.trim().split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

fn is_newer(candidate: &[u64], current: &[u64]) -> bool {
    let len = candidate.len().max(current.len());
    for i in 0..len {
        let (a, b) = (candidate.get(i).copied().unwrap_or(0), current.get(i).copied().unwrap_or(0));
        if a != b {
            return a > b;
        }
    }
    false
}

/// Picks the newest published `rust-v*` release from the GitHub releases JSON, if it is newer than `current`.
/// Drafts, pre-releases and Python releases (`v*` tags) are ignored.
pub fn newest_release(current: &str, releases_json: &str) -> Option<NewRelease> {
    let current_v = parse_version(current)?;
    let list: Vec<Value> = serde_json::from_str(releases_json).ok()?;
    let mut best: Option<(Vec<u64>, NewRelease)> = None;
    for r in &list {
        if r.get("draft").and_then(Value::as_bool).unwrap_or(false)
            || r.get("prerelease").and_then(Value::as_bool).unwrap_or(false)
        {
            continue;
        }
        let Some(tag) = r.get("tag_name").and_then(Value::as_str) else { continue };
        let Some(ver_str) = tag.strip_prefix("rust-v") else { continue };
        let Some(ver) = parse_version(ver_str) else { continue };
        if !is_newer(&ver, &current_v) || best.as_ref().is_some_and(|(b, _)| !is_newer(&ver, b)) {
            continue;
        }
        let url = r.get("html_url").and_then(Value::as_str).unwrap_or(RELEASES_PAGE).to_string();
        best = Some((ver, NewRelease { version: ver_str.to_string(), url }));
    }
    best.map(|(_, r)| r)
}

/// Asks GitHub for releases. Any network or parse problem returns None (the check is best-effort).
pub fn check_for_update(current: &str) -> Option<NewRelease> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(8))
        .build();
    let body = agent
        .get(RELEASES_API)
        .set("User-Agent", "bbb-studyforge-update-check")
        .set("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .into_string()
        .ok()?;
    newest_release(current, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: &str = r#"[
        {"tag_name": "v3.0.0", "html_url": "https://x/py", "draft": false, "prerelease": false},
        {"tag_name": "rust-v2.4.0-beta", "html_url": "https://x/beta", "draft": false, "prerelease": false},
        {"tag_name": "rust-v2.5.0", "html_url": "https://x/pre", "draft": false, "prerelease": true},
        {"tag_name": "rust-v2.3.1", "html_url": "https://x/231", "draft": false, "prerelease": false},
        {"tag_name": "rust-v2.10.0", "html_url": "https://x/2100", "draft": true, "prerelease": false},
        {"tag_name": "rust-v2.3.0", "html_url": "https://x/230", "draft": false, "prerelease": false}
    ]"#;

    #[test]
    fn picks_newest_stable_rust_release() {
        let r = newest_release("2.2.0", JSON).unwrap();
        assert_eq!(r, NewRelease { version: "2.3.1".into(), url: "https://x/231".into() });
    }

    #[test]
    fn nothing_when_up_to_date_or_bad_input() {
        assert_eq!(newest_release("2.3.1", JSON), None);
        assert_eq!(newest_release("2.4", JSON), None);
        assert_eq!(newest_release("2.2.0", "not json"), None);
        assert!(is_newer(&[2, 10, 0], &[2, 9, 9]));
        assert!(!is_newer(&[2, 2], &[2, 2, 0]));
    }
}
