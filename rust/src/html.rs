//! HTML escaping that matches Python's `html.escape(quote=True)` and a practical
//! subset of `html.unescape` (named basics plus numeric references).

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        match decode_entity(tail) {
            Some((decoded, len)) => {
                out.push_str(&decoded);
                rest = &tail[len..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `tail` starts with '&'. Returns the decoded text and the number of bytes consumed.
fn decode_entity(tail: &str) -> Option<(String, usize)> {
    let semi = tail[1..].find(';')? + 1;
    if semi > 12 {
        return None;
    }
    let name = &tail[1..semi];
    let consumed = semi + 1;
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse::<u32>().ok()?,
        };
        let ch = char::from_u32(code).unwrap_or('\u{FFFD}');
        return Some((ch.to_string(), consumed));
    }
    let decoded = match name {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => "\u{a0}",
        _ => return None,
    };
    Some((decoded.to_string(), consumed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_and_unescape_round_trip() {
        assert_eq!(escape("<a href='x'>&</a>"), "&lt;a href=&#x27;x&#x27;&gt;&amp;&lt;/a&gt;");
        assert_eq!(unescape("A &amp; B &lt;3 &#65;&#x42; &unknown;"), "A & B <3 AB &unknown;");
    }
}
