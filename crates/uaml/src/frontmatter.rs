use std::sync::LazyLock;
use regex::Regex;

static BLOCK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^---\n(.*?)\n---\n?(.*)$").unwrap());
static NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^-?\d+(\.\d+)?$").unwrap());

#[derive(Debug, Clone, PartialEq)]
pub enum FmValue {
    Str(String),
    Bool(bool),
    Num(f64),
    List(Vec<FmValue>),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frontmatter {
    pub entries: Vec<(String, FmValue)>,
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

fn scalar(v: &FmValue) -> String {
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
        FmValue::List(_) => unreachable!("lists are not scalars"),
    }
}

pub fn render_frontmatter(fm: &Frontmatter) -> String {
    fm.entries
        .iter()
        .map(|(k, v)| match v {
            FmValue::List(items) => {
                let inner = items.iter().map(scalar).collect::<Vec<_>>().join(", ");
                format!("{k}: [{inner}]")
            }
            other => format!("{k}: {}", scalar(other)),
        })
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
}
