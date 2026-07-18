// A deliberately tolerant XML scanner for `.flame` files.
// See LICENSE (GPL-2.0-or-later) at the repo root.
//
// This is NOT a conforming XML parser, and must not be replaced with one.
// `TXForm.ToXMLString` omits the space after the `name` attribute:
//
//     ... opacity="1" name="foo"var_color="0.5" />
//
// which is malformed. Apophysis reads it back because its own TXmlScanner is
// lenient, so any strict parser would reject files Apophysis itself wrote.
// This scanner therefore resynchronises on identifier characters rather than
// requiring separators.

/// One element: its tag name, attributes in document order, and the text
/// content between its open and close tags (empty for self-closing tags).
#[derive(Debug, Clone, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    /// First attribute with this name, or `None`. Attribute lookup is
    /// case-sensitive, matching the original.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Parse an attribute as f64.
    ///
    /// Lenient about the decimal separator: Delphi's `%g` is locale-aware, so
    /// files written on a comma-decimal machine contain `0,5`. We accept both
    /// on read and always write `.` — see `write.rs`.
    pub fn attr_f64(&self, name: &str) -> Option<f64> {
        self.attr(name).and_then(parse_f64)
    }

    pub fn attr_usize(&self, name: &str) -> Option<usize> {
        self.attr(name).and_then(|v| v.trim().parse::<usize>().ok())
    }

    pub fn attr_i32(&self, name: &str) -> Option<i32> {
        self.attr(name).and_then(|v| v.trim().parse::<f64>().ok()).map(|v| v as i32)
    }

    /// Whitespace-separated floats, e.g. `coefs="1 0 0 1 0 0"`.
    pub fn attr_floats(&self, name: &str) -> Option<Vec<f64>> {
        self.attr(name).map(|v| v.split_whitespace().filter_map(parse_f64).collect())
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> {
        self.children.iter().filter(move |c| c.name == name)
    }
}

/// Accept both `.` and `,` as the decimal separator.
pub fn parse_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    match t.parse::<f64>() {
        Ok(v) => Some(v),
        // Only retry if a comma is plausibly a decimal point (exactly one, and
        // no '.' present) — never rewrite a list separator.
        Err(_) if t.matches(',').count() == 1 && !t.contains('.') => {
            t.replace(',', ".").parse::<f64>().ok()
        }
        Err(_) => None,
    }
}

struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Scanner { s: s.as_bytes(), i: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_ascii_whitespace()) {
            self.i += 1;
        }
    }

    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat.as_bytes())
    }

    /// Identifier: letters, digits, `_`, `-`, `:`, `.`
    fn ident(&mut self) -> String {
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b':' | b'.') {
                self.i += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.s[start..self.i]).into_owned()
    }

    /// Attributes up to `>` or `/>`.
    ///
    /// The key tolerance: after a closing quote we do not require whitespace,
    /// we simply look for the next identifier character.
    fn attrs(&mut self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                None => break,
                Some(b'>') | Some(b'/') => break,
                Some(c) if c.is_ascii_alphabetic() || c == b'_' => {}
                // Junk between attributes — skip a byte and resynchronise
                // rather than aborting the element.
                Some(_) => {
                    self.i += 1;
                    continue;
                }
            }

            let key = self.ident();
            self.skip_ws();
            if self.peek() != Some(b'=') {
                // Valueless attribute; record it as empty and move on.
                out.push((key, String::new()));
                continue;
            }
            self.i += 1;
            self.skip_ws();

            let quote = match self.peek() {
                Some(q @ (b'"' | b'\'')) => {
                    self.i += 1;
                    q
                }
                _ => {
                    // Unquoted value: read to whitespace or tag end.
                    let start = self.i;
                    while let Some(c) = self.peek() {
                        if c.is_ascii_whitespace() || c == b'>' || c == b'/' {
                            break;
                        }
                        self.i += 1;
                    }
                    out.push((key, String::from_utf8_lossy(&self.s[start..self.i]).into_owned()));
                    continue;
                }
            };

            let start = self.i;
            while let Some(c) = self.peek() {
                if c == quote {
                    break;
                }
                self.i += 1;
            }
            let value = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
            if self.peek() == Some(quote) {
                self.i += 1;
            }
            out.push((key, unescape(&value)));
        }
        out
    }

    fn parse_element(&mut self) -> Option<Element> {
        self.skip_ws();
        if self.peek() != Some(b'<') {
            return None;
        }
        self.i += 1;

        let name = self.ident();
        if name.is_empty() {
            return None;
        }
        let attrs = self.attrs();

        self.skip_ws();
        let self_closing = if self.starts_with("/>") {
            self.i += 2;
            true
        } else if self.peek() == Some(b'>') {
            self.i += 1;
            false
        } else {
            // Truncated tag; salvage what we have.
            true
        };

        let mut el = Element { name, attrs, text: String::new(), children: Vec::new() };
        if self_closing {
            return Some(el);
        }

        // Content: mixed text and child elements until the matching close tag.
        let mut text = String::new();
        loop {
            if self.i >= self.s.len() {
                break;
            }
            if self.starts_with("</") {
                self.i += 2;
                let _close = self.ident();
                while let Some(c) = self.peek() {
                    self.i += 1;
                    if c == b'>' {
                        break;
                    }
                }
                break;
            }
            if self.starts_with("<!--") {
                self.i += 4;
                while self.i < self.s.len() && !self.starts_with("-->") {
                    self.i += 1;
                }
                self.i = (self.i + 3).min(self.s.len());
                continue;
            }
            if self.peek() == Some(b'<') {
                match self.parse_element() {
                    Some(child) => el.children.push(child),
                    None => self.i += 1,
                }
                continue;
            }
            text.push(self.s[self.i] as char);
            self.i += 1;
        }
        el.text = unescape(&text);
        Some(el)
    }
}

fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Parse every top-level element in a document.
///
/// A `.flame` file is `<flames>...<flame/>...</flames>`, but bare `<flame>`
/// elements with no wrapper are common in the wild, so both are accepted.
pub fn parse(input: &str) -> Vec<Element> {
    let mut sc = Scanner::new(input);
    let mut out = Vec::new();
    while sc.i < sc.s.len() {
        sc.skip_ws();
        if sc.peek() != Some(b'<') {
            sc.i += 1;
            continue;
        }
        // Skip declarations, comments and doctypes.
        if sc.starts_with("<?") || sc.starts_with("<!") {
            while sc.i < sc.s.len() && sc.peek() != Some(b'>') {
                sc.i += 1;
            }
            sc.i = (sc.i + 1).min(sc.s.len());
            continue;
        }
        match sc.parse_element() {
            Some(el) => out.push(el),
            None => sc.i += 1,
        }
    }
    out
}

/// Collect every `<flame>` element, whether or not wrapped in `<flames>`.
pub fn find_flames(input: &str) -> Vec<Element> {
    let roots = parse(input);
    let mut out = Vec::new();
    for r in roots {
        if r.name == "flame" {
            out.push(r);
        } else {
            out.extend(r.children.into_iter().filter(|c| c.name == "flame"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason this scanner exists: Apophysis emits no space after
    /// the `name` attribute, which strict parsers reject.
    #[test]
    fn tolerates_missing_space_between_attributes() {
        let el = &parse(r#"<xform weight="1" name="foo"var_color="0.5"/>"#)[0];
        assert_eq!(el.attr("weight"), Some("1"));
        assert_eq!(el.attr("name"), Some("foo"));
        assert_eq!(el.attr("var_color"), Some("0.5"), "attribute after the gap was lost");
    }

    #[test]
    fn parses_nested_flames_and_self_closing_xforms() {
        let doc = r#"
            <flames name="batch">
              <flame name="one" size="640 480">
                <xform weight="1" linear="1" coefs="1 0 0 1 0 0"/>
                <xform weight="0.5" spherical="1" coefs="0.5 0 0 0.5 1 1"/>
              </flame>
            </flames>"#;
        let flames = find_flames(doc);
        assert_eq!(flames.len(), 1);
        assert_eq!(flames[0].attr("name"), Some("one"));
        assert_eq!(flames[0].children_named("xform").count(), 2);
        assert_eq!(flames[0].attr_floats("size").unwrap(), vec![640.0, 480.0]);
    }

    #[test]
    fn accepts_bare_flame_without_wrapper() {
        let flames = find_flames(r#"<flame name="solo"><xform weight="1"/></flame>"#);
        assert_eq!(flames.len(), 1);
        assert_eq!(flames[0].attr("name"), Some("solo"));
    }

    /// Files written on a comma-decimal locale must still load.
    #[test]
    fn accepts_comma_decimal_separator() {
        assert_eq!(parse_f64("0,5"), Some(0.5));
        assert_eq!(parse_f64("0.5"), Some(0.5));
        assert_eq!(parse_f64("-1,25"), Some(-1.25));
        // A thousands-style comma must not be silently reinterpreted.
        assert_eq!(parse_f64("1.234,5"), None);
    }

    #[test]
    fn reads_element_text_for_palettes() {
        let el = &parse("<palette count=\"2\" format=\"RGB\">\n  FF0000 00FF00\n</palette>")[0];
        assert_eq!(el.attr_usize("count"), Some(2));
        assert!(el.text.contains("FF0000"));
    }

    #[test]
    fn skips_declaration_and_comments() {
        let doc = r#"<?xml version="1.0"?><!-- a note --><flame name="x"/>"#;
        let flames = find_flames(doc);
        assert_eq!(flames.len(), 1);
        assert_eq!(flames[0].attr("name"), Some("x"));
    }
}
