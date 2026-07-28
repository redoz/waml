use regex::Regex;
use std::sync::LazyLock;
use waml_syntax::{OkfMarkdownLanguage, OkfMarkdownSyntaxKind, SyntaxElement, SyntaxNode};

static BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^---\n(.*?)\n(?:---|\.\.\.)\n?(.*)$").unwrap());
static NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum FmValue {
    // Ordering matters for untagged deserialize: a JSON string only matches
    // `Str`, a bool only `Bool`, a number only `Num`, an array only `List`.
    Str(String),
    Bool(bool),
    Num(f64),
    List(Vec<FmValue>),
}

/// Ordered key→value frontmatter. On the wire (`Concept.extra`) it serializes
/// as a plain JSON object; insertion order is preserved on serialize.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub entries: Vec<(String, FmValue)>,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Frontmatter {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = s.serialize_map(Some(self.entries.len()))?;
        for (k, v) in &self.entries {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Frontmatter {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Frontmatter, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Frontmatter;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a frontmatter object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut m: M,
            ) -> Result<Frontmatter, M::Error> {
                let mut entries = Vec::new();
                while let Some((k, v)) = m.next_entry::<String, FmValue>()? {
                    entries.push((k, v));
                }
                Ok(Frontmatter { entries })
            }
        }
        d.deserialize_map(V)
    }
}

impl Frontmatter {
    pub fn get(&self, key: &str) -> Option<&FmValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    pub fn get_str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(FmValue::Str(s)) => Some(s.as_str()),
            _ => None,
        }
    }
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.get(key) {
            Some(FmValue::Bool(b)) => Some(*b),
            _ => None,
        }
    }
    /// A `stereotype` may be a scalar or a list; normalize to a Vec of strings.
    pub fn get_string_list(&self, key: &str) -> Vec<String> {
        match self.get(key) {
            Some(FmValue::List(items)) => items
                .iter()
                .filter_map(|v| match v {
                    FmValue::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            Some(FmValue::Str(s)) if !s.is_empty() => vec![s.clone()],
            _ => Vec::new(),
        }
    }
}

fn normalize_line_endings(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' {
            if characters.peek() == Some(&'\n') {
                characters.next();
            }
            normalized.push('\n');
        } else {
            normalized.push(character);
        }
    }
    normalized
}

fn escape_quoted_string(value: &str) -> String {
    let normalized = normalize_line_endings(value);
    let mut escaped = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn decode_quoted_string(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('\\') => decoded.push('\\'),
            Some('"') => decoded.push('"'),
            Some('n') => decoded.push('\n'),
            Some('r') => decoded.push('\r'),
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }
    normalize_line_endings(&decoded)
}

pub(crate) fn parse_value(s: &str) -> FmValue {
    if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        let items = inner
            .split(',')
            .map(|x| parse_value(x.trim()))
            .filter(|v| !matches!(v, FmValue::Str(s) if s.is_empty()))
            .collect();
        return FmValue::List(items);
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        return FmValue::Str(decode_quoted_string(inner));
    }
    if NUM_RE.is_match(s) {
        if let Ok(n) = s.parse::<f64>() {
            return FmValue::Num(n);
        }
    }
    match s {
        "true" => FmValue::Bool(true),
        "false" => FmValue::Bool(false),
        other => FmValue::Str(other.to_string()),
    }
}

pub(crate) fn parse_closed_syntax(node: &SyntaxNode<OkfMarkdownLanguage>) -> Option<Frontmatter> {
    if node.kind() != OkfMarkdownSyntaxKind::Frontmatter
        || !node.children().any(|element| {
            element.into_token().is_some_and(|token| {
                token.kind() == OkfMarkdownSyntaxKind::FrontmatterCloseFence
                    && !token.flags().is_missing()
            })
        })
    {
        return None;
    }

    let mut frontmatter = Frontmatter::default();
    for entry in node.children().filter_map(SyntaxElement::into_node) {
        if entry.kind() != OkfMarkdownSyntaxKind::FrontmatterEntry {
            continue;
        }
        let mut key = None;
        let mut value = None;
        for token in entry.children().filter_map(SyntaxElement::into_token) {
            match token.kind() {
                OkfMarkdownSyntaxKind::FrontmatterKey => key = Some(token.text().write_to_string()),
                OkfMarkdownSyntaxKind::FrontmatterValue if !token.flags().is_missing() => {
                    value = Some(token.text().write_to_string())
                }
                _ => {}
            }
        }
        if let (Some(key), Some(value)) = (key, value) {
            frontmatter.entries.push((key, parse_value(&value)));
        }
    }
    Some(frontmatter)
}

pub struct ParsedFrontmatter {
    pub frontmatter: Frontmatter,
    pub body_range: std::ops::Range<usize>,
}

pub fn parse_frontmatter_spanned(text: &str) -> ParsedFrontmatter {
    let caps = match BLOCK_RE.captures(text) {
        Some(c) => c,
        None => {
            return ParsedFrontmatter {
                frontmatter: Frontmatter::default(),
                body_range: 0..text.len(),
            };
        }
    };
    let mut entries = Vec::new();
    for raw in caps[1].split('\n') {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some(ci) = line.find(':') else { continue };
        let key = line[..ci].trim().to_string();
        let rest = line[ci + 1..].trim();
        if rest.is_empty() {
            continue; // nested-object frontmatter unsupported (UML-only, flat)
        }
        entries.push((key, parse_value(rest)));
    }
    ParsedFrontmatter {
        frontmatter: Frontmatter { entries },
        body_range: caps.get(2).expect("body capture exists").range(),
    }
}

pub fn parse_frontmatter(text: &str) -> (Frontmatter, String) {
    let parsed = parse_frontmatter_spanned(text);
    (parsed.frontmatter, text[parsed.body_range].to_owned())
}

/// Render any `FmValue` in its canonical form. Total over parsed input: a
/// `List` renders each item recursively (so a nested `List` renders in its
/// own bracket form), so this never panics on anything `parse_value` can
/// produce — including the nested-bracket case (`x: [a, [b]]`).
/// True when a bare `s` would `parse_value` back as something other than the
/// same `Str` — the only cases where quoting is required. A comma is NOT listed:
/// a top-level scalar with a comma is safe bare (only bracketed values split),
/// and a *list item* with a comma is unrepresentable either way (the parser
/// splits list items on every comma, quote-blind), so quoting it wouldn't help.
fn scalar_needs_quote(s: &str) -> bool {
    s.is_empty()
        || s != s.trim()
        || s == "true"
        || s == "false"
        || NUM_RE.is_match(s)
        || (s.starts_with('[') && s.ends_with(']'))
        || s.starts_with('"')
        || s.contains('"')
        || s.contains('\\')
        || s.contains('\n')
        || s.contains('\r')
}

fn render_value(v: &FmValue) -> String {
    match v {
        FmValue::Num(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            }
        }
        FmValue::Bool(b) => b.to_string(),
        FmValue::Str(s) => {
            if scalar_needs_quote(s) {
                format!("\"{}\"", escape_quoted_string(s))
            } else {
                s.clone()
            }
        }
        FmValue::List(items) => {
            let inner = items
                .iter()
                .map(render_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
    }
}

pub fn render_frontmatter(fm: &Frontmatter) -> String {
    fm.entries
        .iter()
        .map(|(k, v)| format!("{k}: {}", render_value(v)))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_lists_and_body() {
        let text = "---\ntype: uml.Class\ntitle: Order\nstereotype: [aggregateRoot, entity]\nabstract: true\n---\n# Order\n\nbody text";
        let (fm, body) = parse_frontmatter(text);
        assert_eq!(fm.get_str("type"), Some("uml.Class"));
        assert_eq!(fm.get_str("title"), Some("Order"));
        assert_eq!(
            fm.get_string_list("stereotype"),
            vec!["aggregateRoot", "entity"]
        );
        assert_eq!(fm.get_bool("abstract"), Some(true));
        assert_eq!(body, "# Order\n\nbody text");
    }

    #[test]
    fn no_frontmatter_returns_whole_text_as_body() {
        let (fm, body) = parse_frontmatter("# Just markdown");
        assert!(fm.entries.is_empty());
        assert_eq!(body, "# Just markdown");
    }

    #[test]
    fn spanned_frontmatter_points_into_the_original_body() {
        let text = "---\ntype: uml.Class\n---\n# Café\n";
        let parsed = parse_frontmatter_spanned(text);
        assert_eq!(parsed.frontmatter.get_str("type"), Some("uml.Class"));
        assert_eq!(&text[parsed.body_range], "# Café\n");
    }

    #[test]
    fn spanned_frontmatter_uses_the_whole_document_when_absent() {
        let text = "# Just markdown";
        let parsed = parse_frontmatter_spanned(text);
        assert!(parsed.frontmatter.entries.is_empty());
        assert_eq!(parsed.body_range, 0..text.len());
    }

    #[test]
    fn parses_quoted_string_with_escapes() {
        let (fm, _) = parse_frontmatter("---\ndescription: \"A \\\"placed\\\" order.\"\n---\n");
        assert_eq!(fm.get_str("description"), Some("A \"placed\" order."));
    }

    #[test]
    fn render_round_trips_order() {
        let text = "---\ntype: uml.Class\nstereotype: [a, b]\ntitle: Order\n---\nbody";
        let (fm, _) = parse_frontmatter(text);
        assert_eq!(
            render_frontmatter(&fm),
            "type: uml.Class\nstereotype: [a, b]\ntitle: Order"
        );
    }

    #[test]
    fn render_leaves_safe_scalars_unquoted() {
        // Plain string scalars that can't be misread as another type render
        // bare — no decorative quotes on `type`, `title`, prose, slashes, colons.
        let text = "---\ntype: uml.Class\ntitle: New Package flow\ndescription: Re-roots docs under <parent>/<slug>/, then appends.\n---\n";
        let (fm, _) = parse_frontmatter(text);
        assert_eq!(
            render_frontmatter(&fm),
            "type: uml.Class\ntitle: New Package flow\ndescription: Re-roots docs under <parent>/<slug>/, then appends.",
        );
    }

    #[test]
    fn render_quotes_scalars_that_would_reparse_wrong() {
        // A string that looks like a bool/num/list/empty must stay quoted so it
        // round-trips as a Str, not the ambiguous type it resembles.
        for raw in [
            "true",
            "false",
            "42",
            "-3.5",
            "[a]",
            "",
            " leading",
            "has\"quote",
        ] {
            let fm = Frontmatter {
                entries: vec![("k".into(), FmValue::Str(raw.to_string()))],
            };
            let rendered = render_frontmatter(&fm);
            let (fm2, _) = parse_frontmatter(&format!("---\n{rendered}\n---\n"));
            assert_eq!(fm, fm2, "must round-trip {raw:?}; rendered as {rendered}");
        }
    }

    #[test]
    fn render_list_items_unquote_but_quote_type_lookalikes() {
        // List items render bare too, except one that would reparse as a Num
        // (or Bool/list) — it stays quoted so it round-trips as a Str.
        let fm = Frontmatter {
            entries: vec![(
                "stereotype".into(),
                FmValue::List(vec![
                    FmValue::Str("entity".into()),
                    FmValue::Str("42".into()),
                ]),
            )],
        };
        assert_eq!(render_frontmatter(&fm), "stereotype: [entity, \"42\"]");
        let (fm2, _) = parse_frontmatter(&format!("---\n{}\n---\n", render_frontmatter(&fm)));
        assert_eq!(fm, fm2);
    }

    #[test]
    fn render_does_not_panic_on_nested_list() {
        // `x: [a, [b]]` parses to a nested List value (parse_value recurses on
        // comma-split bracket items). render_frontmatter/scalar() must be total
        // over parsed input — it must render this, not panic.
        let text = "---\nx: [a, [b]]\n---\n";
        let (fm, _) = parse_frontmatter(text);
        let rendered = render_frontmatter(&fm);
        let (fm2, _) = parse_frontmatter(&format!("---\n{rendered}\n---\n"));
        assert_eq!(fm, fm2, "round-trip must preserve the nested structure");
    }

    #[test]
    fn quoted_multiline_strings_round_trip_as_normalized_lf() {
        let fm = Frontmatter {
            entries: vec![(
                "description".into(),
                FmValue::Str("First\r\nSecond\rThird\nFourth".into()),
            )],
        };

        let rendered = render_frontmatter(&fm);

        assert_eq!(rendered, r#"description: "First\nSecond\nThird\nFourth""#);
        let (parsed, _) = parse_frontmatter(&format!("---\n{rendered}\n---\n"));
        assert_eq!(
            parsed.get_str("description"),
            Some("First\nSecond\nThird\nFourth")
        );
    }

    #[test]
    fn quoted_escape_decoder_distinguishes_newline_from_literal_backslash_n() {
        let fm = Frontmatter {
            entries: vec![("description".into(), FmValue::Str(r"Line\nLiteral".into()))],
        };

        let rendered = render_frontmatter(&fm);
        let (parsed, _) = parse_frontmatter(&format!("---\n{rendered}\n---\n"));

        assert_eq!(parsed, fm);
    }
}
