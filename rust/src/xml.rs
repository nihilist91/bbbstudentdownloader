//! A small DOM built on quick-xml, shaped like the subset of ElementTree that the
//! Python parser relied on: `findtext`, `find`, `findall`, `.text`, `.get`.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

#[derive(Debug, Clone, Default)]
pub struct Element {
    pub tag: String,
    pub attrs: Vec<(String, String)>,
    /// Text before the first child element (ElementTree's `.text`).
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// `findtext(tag)` on direct children: the first child's text, or "" if absent.
    pub fn child_text(&self, tag: &str) -> String {
        self.children
            .iter()
            .find(|c| c.tag == tag)
            .map(|c| c.text.clone())
            .unwrap_or_default()
    }

    /// All descendants (not self) in document order, like `.//*`.
    pub fn descendants(&self) -> Vec<&Element> {
        fn walk<'a>(e: &'a Element, out: &mut Vec<&'a Element>) {
            for c in &e.children {
                out.push(c);
                walk(c, out);
            }
        }
        let mut out = Vec::new();
        walk(self, &mut out);
        out
    }

    /// `.//tag`
    pub fn find_all(&self, tag: &str) -> Vec<&Element> {
        self.descendants().into_iter().filter(|e| e.tag == tag).collect()
    }

    /// `.//tag` (first match)
    pub fn find(&self, tag: &str) -> Option<&Element> {
        self.descendants().into_iter().find(|e| e.tag == tag)
    }

    /// `.//a/b` findtext: text of the first `b` child of any `a` descendant.
    pub fn find_path_text(&self, a: &str, b: &str) -> String {
        for el in self.find_all(a) {
            if let Some(child) = el.children.iter().find(|c| c.tag == b) {
                return child.text.clone();
            }
        }
        String::new()
    }
}

/// Parses a document and returns its root element, or `None` on malformed XML
/// (ElementTree raises ParseError in the same situations).
pub fn parse(src: &str) -> Option<Element> {
    let mut reader = Reader::from_str(src);
    reader.config_mut().trim_text(false);
    let mut stack = vec![Element::default()]; // synthetic document node
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => stack.push(start_element(&e)?),
            Ok(Event::Empty(e)) => {
                let el = start_element(&e)?;
                stack.last_mut()?.children.push(el);
            }
            Ok(Event::End(_)) => {
                let el = stack.pop()?;
                if stack.is_empty() {
                    return None;
                }
                stack.last_mut()?.children.push(el);
            }
            Ok(Event::Text(t)) => {
                let txt = t.unescape().ok()?;
                append_text(stack.last_mut()?, &txt);
            }
            Ok(Event::CData(c)) => {
                let txt = String::from_utf8_lossy(&c.into_inner()).into_owned();
                append_text(stack.last_mut()?, &txt);
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    if stack.len() != 1 {
        return None;
    }
    let doc = stack.pop()?;
    doc.children.into_iter().next()
}

fn append_text(el: &mut Element, txt: &str) {
    if el.children.is_empty() {
        el.text.push_str(txt);
    }
}

fn start_element(e: &BytesStart) -> Option<Element> {
    let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.ok()?;
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let value = a.unescape_value().ok()?.into_owned();
        attrs.push((key, value));
    }
    Some(Element { tag, attrs, text: String::new(), children: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_text_like_elementtree() {
        let root = parse("<recording><meta><context>CS1</context></meta><id>r1</id></recording>").unwrap();
        assert_eq!(root.find_path_text("meta", "context"), "CS1");
        assert_eq!(root.child_text("id"), "r1");
        assert!(root.find("missing").is_none());
    }

    #[test]
    fn rejects_malformed_xml() {
        assert!(parse("<a><b></a>").is_none());
    }
}
