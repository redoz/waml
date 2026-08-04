use regex::Regex;
use std::sync::LazyLock;
use waml_syntax::{OkfMarkdownLanguage, OkfMarkdownSyntaxKind, SyntaxElement, SyntaxNode};

static NUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());

/// Maximum `List` nesting depth accepted from any input path (authored
/// frontmatter text and the serde wire form alike). Frontmatter is untrusted —
/// a hostile `[[[[…]]]]` value must produce a value or an error, never a
/// stack overflow. Real frontmatter nests one or two levels deep.
const MAX_VALUE_DEPTH: usize = 32;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum FmValue {
    // Ordering matters for untagged-style deserialize: a JSON string only
    // matches `Str`, a bool only `Bool`, a number only `Num`, an array only
    // `List` (see the manual `Deserialize` impl below, which also depth-caps).
    Str(String),
    Bool(bool),
    Num(f64),
    List(Vec<FmValue>),
}

// Manual impl instead of a derive: `Num(f64)` under a derived `PartialEq`
// makes `NaN != NaN`, which would break change detection if a NaN ever got in.
// Both admission paths (NUM_RE, JSON) exclude NaN today, so `total_cmp` keeps
// the derive's behaviour for normal numbers while staying a sound equivalence
// if that ever changes. (The one visible shift: `-0.0 != 0.0` under
// `total_cmp` — an authored `-0` vs `0` now reads as a change, which is fine.)
impl PartialEq for FmValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FmValue::Str(a), FmValue::Str(b)) => a == b,
            (FmValue::Bool(a), FmValue::Bool(b)) => a == b,
            (FmValue::Num(a), FmValue::Num(b)) => a.total_cmp(b) == std::cmp::Ordering::Equal,
            (FmValue::List(a), FmValue::List(b)) => a == b,
            _ => false,
        }
    }
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

/// Depth-carrying seed so the wire form obeys [`MAX_VALUE_DEPTH`] too — the
/// derived untagged impl recursed without bound on a hostile `[[[[…]]]]`.
#[cfg(feature = "serde")]
struct FmValueSeed {
    depth: usize,
}

#[cfg(feature = "serde")]
impl<'de> serde::de::DeserializeSeed<'de> for FmValueSeed {
    type Value = FmValue;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<FmValue, D::Error> {
        struct V {
            depth: usize,
        }
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = FmValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a frontmatter value (string, bool, number, or array)")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<FmValue, E> {
                Ok(FmValue::Str(v.to_owned()))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<FmValue, E> {
                Ok(FmValue::Str(v))
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<FmValue, E> {
                Ok(FmValue::Bool(v))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<FmValue, E> {
                Ok(FmValue::Num(v))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<FmValue, E> {
                Ok(FmValue::Num(v as f64))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<FmValue, E> {
                Ok(FmValue::Num(v as f64))
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(self, mut s: S) -> Result<FmValue, S::Error> {
                if self.depth >= MAX_VALUE_DEPTH {
                    return Err(serde::de::Error::custom("frontmatter value nests too deeply"));
                }
                let mut items = Vec::new();
                while let Some(item) = s.next_element_seed(FmValueSeed {
                    depth: self.depth + 1,
                })? {
                    items.push(item);
                }
                Ok(FmValue::List(items))
            }
        }
        d.deserialize_any(V { depth: self.depth })
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FmValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<FmValue, D::Error> {
        serde::de::DeserializeSeed::deserialize(FmValueSeed { depth: 0 }, d)
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
    parse_value_at(s, 0)
}

fn parse_value_at(s: &str, depth: usize) -> FmValue {
    // Depth cap: frontmatter is untrusted, and each bracket level recurses.
    // Past the cap a bracketed value is kept verbatim as a `Str` instead of
    // overflowing the stack; authored frontmatter never nests this deep.
    if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        if depth >= MAX_VALUE_DEPTH {
            return FmValue::Str(s.to_string());
        }
        let items = inner
            .split(',')
            .map(|x| parse_value_at(x.trim(), depth + 1))
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
    render_value_at(v, 0)
}

fn render_value_at(v: &FmValue, depth: usize) -> String {
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
            // Depth cap mirrors `parse_value_at`/the serde seed: no input path
            // can build a deeper value, so this only fires on a value nested
            // programmatically past the cap — render it empty rather than
            // recursing without bound.
            if depth >= MAX_VALUE_DEPTH {
                return "[]".to_string();
            }
            let inner = items
                .iter()
                .map(|item| render_value_at(item, depth + 1))
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
    fn parses_scalar_and_list_values() {
        assert_eq!(parse_value("uml.Class"), FmValue::Str("uml.Class".into()));
        assert_eq!(parse_value("true"), FmValue::Bool(true));
        assert_eq!(parse_value("-3.5"), FmValue::Num(-3.5));
        assert_eq!(
            parse_value("[aggregateRoot, entity]"),
            FmValue::List(vec![
                FmValue::Str("aggregateRoot".into()),
                FmValue::Str("entity".into()),
            ])
        );
        assert_eq!(
            parse_value("\"A \\\"placed\\\" order.\""),
            FmValue::Str("A \"placed\" order.".into())
        );
    }

    #[test]
    fn render_preserves_entry_order() {
        let fm = Frontmatter {
            entries: vec![
                ("type".into(), FmValue::Str("uml.Class".into())),
                (
                    "stereotype".into(),
                    FmValue::List(vec![FmValue::Str("a".into()), FmValue::Str("b".into())]),
                ),
                ("title".into(), FmValue::Str("Order".into())),
            ],
        };
        assert_eq!(
            render_frontmatter(&fm),
            "type: uml.Class\nstereotype: [a, b]\ntitle: Order"
        );
    }

    #[test]
    fn render_leaves_safe_scalars_unquoted() {
        let fm = Frontmatter {
            entries: vec![
                ("type".into(), FmValue::Str("uml.Class".into())),
                ("title".into(), FmValue::Str("New Package flow".into())),
                (
                    "description".into(),
                    FmValue::Str("Re-roots docs under <parent>/<slug>/, then appends.".into()),
                ),
            ],
        };
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
            let rendered_value = rendered.strip_prefix("k: ").unwrap();
            assert_eq!(
                parse_value(rendered_value),
                FmValue::Str(raw.to_string()),
                "must round-trip {raw:?}; rendered as {rendered}"
            );
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
        assert_eq!(
            parse_value(
                render_frontmatter(&fm)
                    .strip_prefix("stereotype: ")
                    .unwrap()
            ),
            fm.entries[0].1
        );
    }

    #[test]
    fn render_does_not_panic_on_nested_list() {
        let fm = Frontmatter {
            entries: vec![("x".into(), parse_value("[a, [b]]"))],
        };
        let rendered = render_frontmatter(&fm);
        assert_eq!(
            parse_value(rendered.strip_prefix("x: ").unwrap()),
            fm.entries[0].1,
            "round-trip must preserve the nested structure"
        );
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
        let parsed = parse_value(rendered.strip_prefix("description: ").unwrap());
        assert_eq!(parsed, FmValue::Str("First\nSecond\nThird\nFourth".into()));
    }

    #[test]
    fn hostile_deep_nesting_parses_without_overflowing_the_stack() {
        // A hostile document can put an arbitrarily deep `[[[[…]]]]` in a
        // frontmatter value; parse and render must both stay bounded.
        let depth = 100_000;
        let hostile = format!("{}a{}", "[".repeat(depth), "]".repeat(depth));
        let parsed = parse_value(&hostile);
        // Beyond the cap the bracketed remainder is kept verbatim as a Str.
        let rendered = render_value(&parsed);
        assert_eq!(parse_value(&rendered), parsed, "capped value round-trips");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn hostile_deep_nesting_on_the_wire_is_rejected() {
        let depth = 100;
        let json = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
        let result: Result<FmValue, _> = serde_json::from_str(&json);
        assert!(result.is_err(), "deep wire value must error, not overflow");

        let shallow: FmValue = serde_json::from_str("[\"a\", [1, true]]").unwrap();
        assert_eq!(
            shallow,
            FmValue::List(vec![
                FmValue::Str("a".into()),
                FmValue::List(vec![FmValue::Num(1.0), FmValue::Bool(true)]),
            ])
        );
    }

    #[test]
    fn nan_num_compares_equal_to_itself() {
        // `total_cmp`-based Eq: a NaN (unreachable from input today) must not
        // read as perpetually "changed".
        assert_eq!(FmValue::Num(f64::NAN), FmValue::Num(f64::NAN));
        assert_ne!(FmValue::Num(1.0), FmValue::Num(2.0));
        assert_eq!(FmValue::Num(-3.5), FmValue::Num(-3.5));
    }

    #[test]
    fn quoted_escape_decoder_distinguishes_newline_from_literal_backslash_n() {
        let fm = Frontmatter {
            entries: vec![("description".into(), FmValue::Str(r"Line\nLiteral".into()))],
        };

        let rendered = render_frontmatter(&fm);
        let parsed = parse_value(rendered.strip_prefix("description: ").unwrap());

        assert_eq!(parsed, fm.entries[0].1);
    }
}
