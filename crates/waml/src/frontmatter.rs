use std::sync::LazyLock;
use regex::Regex;

static BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^---\n(.*?)\n---\n?(.*)$").unwrap());
static NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());

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

fn parse_value(s: &str) -> FmValue {
    if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
        let items = inner
            .split(',')
            .map(|x| parse_value(x.trim()))
            .filter(|v| !matches!(v, FmValue::Str(s) if s.is_empty()))
            .collect();
        return FmValue::List(items);
    }
    if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
        return FmValue::Str(inner.replace("\\\"", "\"").replace("\\\\", "\\"));
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

pub fn parse_frontmatter(text: &str) -> (Frontmatter, String) {
    let caps = match BLOCK_RE.captures(text) {
        Some(c) => c,
        None => return (Frontmatter::default(), text.to_string()),
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
    (Frontmatter { entries }, caps[2].to_string())
}

/// Render any `FmValue` in its canonical form. Total over parsed input: a
/// `List` renders each item recursively (so a nested `List` renders in its
/// own bracket form), so this never panics on anything `parse_value` can
/// produce — including the nested-bracket case (`x: [a, [b]]`).
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
        FmValue::Str(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
        FmValue::List(items) => {
            let inner = items.iter().map(render_value).collect::<Vec<_>>().join(", ");
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
        assert_eq!(fm.get_string_list("stereotype"), vec!["aggregateRoot", "entity"]);
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
            "type: \"uml.Class\"\nstereotype: [\"a\", \"b\"]\ntitle: \"Order\""
        );
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
}
