use waml_syntax::{
    FrontmatterScalarKind, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, SyntaxElement, SyntaxNode,
};

/// Maximum `List`/`Map` nesting depth accepted from any input path (authored
/// frontmatter text and the serde wire form alike). Frontmatter is untrusted —
/// a hostile `[[[[…]]]]` or deeply nested `{"a":{"a":…}}` value must produce a
/// value or an error, never a stack overflow. Real frontmatter nests one or
/// two levels deep.
const MAX_VALUE_DEPTH: usize = 32;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum FmValue {
    // Ordering matters for untagged-style deserialize: a JSON string only
    // matches `Str`, a bool only `Bool`, a number only `Num`, an array only
    // `List` (see the manual `Deserialize` impl below, which also depth-caps).
    // `Null` and `Map` are handled by a manual `Serialize` impl, not this
    // derive, since a `Vec<(String, FmValue)>` would otherwise serialize as
    // an array of pairs rather than a JSON object.
    Null,
    Str(String),
    Bool(bool),
    Num(f64),
    List(Vec<FmValue>),
    Map(Vec<(String, FmValue)>),
}

// Manual impl instead of a derive: `Num(f64)` under a derived `PartialEq`
// makes `NaN != NaN`, which would break change detection if a NaN ever got in.
// Both admission paths (the classifier, JSON) exclude NaN today except via the
// explicit `.nan` token, so `total_cmp` keeps the derive's behaviour for
// normal numbers while staying a sound equivalence if that ever changes. (The
// one visible shift: `-0.0 != 0.0` under `total_cmp` — an authored `-0` vs
// `0` now reads as a change, which is fine.)
impl PartialEq for FmValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FmValue::Null, FmValue::Null) => true,
            (FmValue::Str(a), FmValue::Str(b)) => a == b,
            (FmValue::Bool(a), FmValue::Bool(b)) => a == b,
            (FmValue::Num(a), FmValue::Num(b)) => a.total_cmp(b) == std::cmp::Ordering::Equal,
            (FmValue::List(a), FmValue::List(b)) => a == b,
            (FmValue::Map(a), FmValue::Map(b)) => a == b,
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
/// derived untagged impl recursed without bound on a hostile `[[[[…]]]]` or
/// `{"a":{"a":…}}`.
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
                f.write_str("a frontmatter value (null, string, bool, number, array, or object)")
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<FmValue, E> {
                Ok(FmValue::Null)
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<FmValue, E> {
                Ok(FmValue::Null)
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
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut s: S,
            ) -> Result<FmValue, S::Error> {
                if self.depth >= MAX_VALUE_DEPTH {
                    return Err(serde::de::Error::custom(
                        "frontmatter value nests too deeply",
                    ));
                }
                let mut items = Vec::new();
                while let Some(item) = s.next_element_seed(FmValueSeed {
                    depth: self.depth + 1,
                })? {
                    items.push(item);
                }
                Ok(FmValue::List(items))
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut m: M,
            ) -> Result<FmValue, M::Error> {
                if self.depth >= MAX_VALUE_DEPTH {
                    return Err(serde::de::Error::custom(
                        "frontmatter value nests too deeply",
                    ));
                }
                let mut entries = Vec::new();
                while let Some(key) = m.next_key::<String>()? {
                    let value = m.next_value_seed(FmValueSeed {
                        depth: self.depth + 1,
                    })?;
                    entries.push((key, value));
                }
                Ok(FmValue::Map(entries))
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
            '\t' => escaped.push_str("\\t"),
            '\0' => escaped.push_str("\\0"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Decodes a double-quoted scalar's inner text (no surrounding quotes).
/// Understands `\\ \" \n \r \t \0` and `\uXXXX`; an unrecognized escape is
/// kept verbatim (backslash and the following character/digits preserved) —
/// the parser side flags this with `InvalidEscapeSequence` but the model
/// never panics on it.
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
            Some('t') => decoded.push('\t'),
            Some('0') => decoded.push('\0'),
            Some('u') => {
                let hex: String = characters.clone().take(4).collect();
                if hex.chars().count() == 4 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    if let Some(decoded_char) =
                        u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                    {
                        decoded.push(decoded_char);
                        for _ in 0..4 {
                            characters.next();
                        }
                        continue;
                    }
                }
                // Malformed/unknown \u escape: keep verbatim.
                decoded.push('\\');
                decoded.push('u');
            }
            Some(other) => {
                decoded.push('\\');
                decoded.push(other);
            }
            None => decoded.push('\\'),
        }
    }
    normalize_line_endings(&decoded)
}

/// Splits a flow sequence's inner text (`[...]` with the brackets already
/// stripped) on top-level commas, quote-aware and bracket-depth-aware. A
/// quote character only opens a quoted scalar when it is the FIRST
/// significant byte of an item — matching YAML, where a quote elsewhere
/// inside a plain (unquoted) scalar is just a literal character, not a
/// delimiter. A comma inside `'...'`, `"..."`, or a nested `[...]` does not
/// split.
fn split_flow_items(s: &str) -> Vec<&str> {
    if s.trim().is_empty() {
        return Vec::new();
    }
    let bytes = s.as_bytes();
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut i = 0usize;
    while i < bytes.len() {
        let at_item_start = bytes[start..i].iter().all(|&b| b == b' ');
        if at_item_start && (bytes[i] == b'\'' || bytes[i] == b'"') {
            let quote = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == quote {
                    if quote == b'\'' && i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                if quote == b'"' && bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                i += 1;
            }
            continue;
        }
        match bytes[i] {
            b'[' => {
                depth += 1;
                i += 1;
            }
            b']' => {
                depth -= 1;
                i += 1;
            }
            b',' if depth <= 0 => {
                items.push(&s[start..i]);
                start = i + 1;
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    items.push(&s[start..]);
    items
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
        let items = split_flow_items(inner)
            .into_iter()
            .map(|item| parse_value_at(item.trim(), depth + 1))
            .collect();
        return FmValue::List(items);
    }
    if s.len() >= 2 {
        if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
            return FmValue::Str(decode_quoted_string(inner));
        }
        if let Some(inner) = s.strip_prefix('\'').and_then(|x| x.strip_suffix('\'')) {
            return FmValue::Str(inner.replace("''", "'"));
        }
    }
    bare_scalar_value(s)
}

/// Classifies a bare (unquoted) scalar via the shared YAML 1.2 core
/// classifier and converts it to the matching `FmValue`. `classify_bare_scalar`
/// already validated the numeric shape, so the `unwrap_or` fallbacks below are
/// defensive only — never reachable from a string the classifier called
/// `Number`.
fn bare_scalar_value(s: &str) -> FmValue {
    match waml_syntax::classify_bare_scalar(s) {
        FrontmatterScalarKind::Null => FmValue::Null,
        FrontmatterScalarKind::Bool => FmValue::Bool(s == "true"),
        FrontmatterScalarKind::Number => FmValue::Num(parse_core_number(s)),
        FrontmatterScalarKind::Str => FmValue::Str(s.to_string()),
    }
}

/// Converts a scalar the classifier already confirmed is `Number`-shaped:
/// hex/octal integers, `.inf`/`-.inf`/`+.inf`/`.nan`, or a plain decimal.
fn parse_core_number(s: &str) -> f64 {
    match s {
        ".inf" | "+.inf" => return f64::INFINITY,
        "-.inf" => return f64::NEG_INFINITY,
        ".nan" => return f64::NAN,
        _ => {}
    }
    if let Some(hex) = s.strip_prefix("0x") {
        return i64::from_str_radix(hex, 16)
            .map(|n| n as f64)
            .unwrap_or(0.0);
    }
    if let Some(oct) = s.strip_prefix("0o") {
        return i64::from_str_radix(oct, 8).map(|n| n as f64).unwrap_or(0.0);
    }
    s.parse::<f64>().unwrap_or(0.0)
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

    let entry_parent = node
        .children()
        .filter_map(SyntaxElement::into_node)
        .find(|n| n.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping);
    let entries = match &entry_parent {
        Some(mapping) => map_entries_from_mapping(mapping, 0),
        None => Vec::new(),
    };
    Some(Frontmatter { entries })
}

/// Minimal single/double-quoted scalar decoding for a token straight off the
/// tree (still carries its surrounding quotes, unlike the value-position
/// strings `parse_value`/`decode_quoted_string` are handed).
fn decode_quoted_scalar(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 {
        return raw.to_string();
    }
    // An unterminated quoted scalar (already flagged by
    // `UnterminatedQuotedScalar`) has no closing quote byte to strip — only
    // peel the opening quote so a multi-byte final character stays intact.
    let closed = matches!(bytes[bytes.len() - 1], b'\'' | b'"') && bytes.len() > 1;
    let inner = if closed {
        &raw[1..raw.len() - 1]
    } else {
        &raw[1..]
    };
    match bytes[0] {
        b'\'' => inner.replace("''", "'"),
        b'"' => decode_quoted_string(inner),
        _ => raw.to_string(),
    }
}

/// Reads every `FrontmatterEntry` child of a `FrontmatterMapping` node into
/// ordered `(key, value)` pairs, last-duplicate-wins in place (matching
/// `Frontmatter::get`'s first-match lookup — a repeated key updates the
/// value at its FIRST position rather than appending a second entry).
/// Depth-capped like every other value reader: a hostile tree past the cap
/// reads as an empty map rather than recursing without bound.
fn map_entries_from_mapping(
    mapping: &SyntaxNode<OkfMarkdownLanguage>,
    depth: usize,
) -> Vec<(String, FmValue)> {
    if depth >= MAX_VALUE_DEPTH {
        return Vec::new();
    }
    let mut entries: Vec<(String, FmValue)> = Vec::new();
    for element in mapping.children() {
        let Some(entry) = element.into_node() else {
            continue;
        };
        if entry.kind() != OkfMarkdownSyntaxKind::FrontmatterEntry {
            continue;
        }
        let mut children = entry.children();
        let Some(key_element) = children.next() else {
            continue;
        };
        let Some(key_token) = key_element.into_token() else {
            continue;
        };
        let key_text = match key_token.kind() {
            OkfMarkdownSyntaxKind::FrontmatterKey => key_token.text().write_to_string(),
            OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken => {
                decode_quoted_scalar(&key_token.text().write_to_string())
            }
            // Comment-only / blank-line entries, or a `BadToken` recovery
            // entry: no key to read.
            _ => continue,
        };
        let rest: Vec<_> = children.collect();
        let rest_after_colon = if matches!(
            rest.first(),
            Some(SyntaxElement::Token(t)) if t.kind() == OkfMarkdownSyntaxKind::ColonToken
        ) {
            &rest[1..]
        } else {
            &rest[..]
        };
        let value = value_from_elements(rest_after_colon, depth + 1).unwrap_or(FmValue::Null);
        if let Some(existing) = entries.iter_mut().find(|(k, _)| *k == key_text) {
            existing.1 = value;
        } else {
            entries.push((key_text, value));
        }
    }
    entries
}

/// Reads every `FrontmatterSequenceItem` child of a `FrontmatterSequence`
/// node into an ordered `Vec<FmValue>`.
fn list_items_from_sequence(
    sequence: &SyntaxNode<OkfMarkdownLanguage>,
    depth: usize,
) -> Vec<FmValue> {
    if depth >= MAX_VALUE_DEPTH {
        return Vec::new();
    }
    let mut items = Vec::new();
    for element in sequence.children() {
        let Some(item) = element.into_node() else {
            continue;
        };
        if item.kind() != OkfMarkdownSyntaxKind::FrontmatterSequenceItem {
            continue;
        }
        let mut children = item.children();
        let Some(dash_element) = children.next() else {
            continue;
        };
        if !matches!(
            &dash_element,
            SyntaxElement::Token(t) if t.kind() == OkfMarkdownSyntaxKind::FrontmatterDashToken
        ) {
            continue;
        }
        let rest: Vec<_> = children.collect();
        items.push(value_from_elements(&rest, depth + 1).unwrap_or(FmValue::Null));
    }
    items
}

/// Reads the value alternatives that can follow a key/colon or a dash in the
/// tree: a nested `FrontmatterMapping`/`FrontmatterSequence` node, a block
/// scalar (header token + content lines), a quoted scalar token, or a bare
/// `FrontmatterValue` token. `None` only when nothing recognizable is present
/// (the caller treats that as `Null`, matching an empty/missing value).
fn value_from_elements(
    elements: &[SyntaxElement<OkfMarkdownLanguage>],
    depth: usize,
) -> Option<FmValue> {
    if depth >= MAX_VALUE_DEPTH {
        return Some(FmValue::Null);
    }
    for element in elements {
        if let SyntaxElement::Node(n) = element {
            match n.kind() {
                OkfMarkdownSyntaxKind::FrontmatterMapping => {
                    return Some(FmValue::Map(map_entries_from_mapping(n, depth)));
                }
                OkfMarkdownSyntaxKind::FrontmatterSequence => {
                    return Some(FmValue::List(list_items_from_sequence(n, depth)));
                }
                _ => {}
            }
        }
    }
    if let Some(header_index) = elements.iter().position(|element| {
        matches!(
            element,
            SyntaxElement::Token(t)
                if t.kind() == OkfMarkdownSyntaxKind::FrontmatterBlockScalarHeaderToken
        )
    }) {
        let SyntaxElement::Token(header_token) = &elements[header_index] else {
            unreachable!("position() found a token");
        };
        let header_text = header_token.text().write_to_string();
        let content_lines: Vec<String> = elements[header_index + 1..]
            .iter()
            .filter_map(|element| match element {
                SyntaxElement::Token(t)
                    if t.kind() == OkfMarkdownSyntaxKind::FrontmatterValue
                        && !t.flags().is_missing() =>
                {
                    Some(t.text().write_to_string())
                }
                _ => None,
            })
            .collect();
        return Some(FmValue::Str(decode_block_scalar(
            &header_text,
            &content_lines,
        )));
    }
    for element in elements {
        if let SyntaxElement::Token(t) = element {
            if t.kind() == OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken {
                return Some(FmValue::Str(decode_quoted_scalar(
                    &t.text().write_to_string(),
                )));
            }
        }
    }
    for element in elements {
        if let SyntaxElement::Token(t) = element {
            if t.kind() == OkfMarkdownSyntaxKind::FrontmatterValue && !t.flags().is_missing() {
                return Some(parse_value(&t.text().write_to_string()));
            }
        }
    }
    Some(FmValue::Null)
}

enum Chomp {
    Clip,
    Strip,
    Keep,
}

/// Assembles a block scalar's decoded string from its header (`|`/`>` plus
/// chomping modifiers) and its content lines (each the FULL source line,
/// including its own leading whitespace — the builder does not fold that
/// into leading trivia for block-scalar content). The strip width is the
/// indentation of the first non-blank content line; an explicit indentation
/// indicator digit (`|2`) is not separately honored — real content always
/// has a first non-blank line whose own indentation already reflects it.
fn decode_block_scalar(header_text: &str, content_lines: &[String]) -> String {
    let literal = header_text.starts_with('|');
    let mut chomp = Chomp::Clip;
    for c in header_text.chars().skip(1) {
        match c {
            '-' => chomp = Chomp::Strip,
            '+' => chomp = Chomp::Keep,
            _ => {}
        }
    }
    let base_indent = content_lines
        .iter()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start_matches(' ').len())
        .unwrap_or(0);
    let stripped: Vec<String> = content_lines
        .iter()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                strip_leading_spaces(line, base_indent).to_string()
            }
        })
        .collect();
    let body = if literal {
        stripped
            .iter()
            .map(|line| format!("{line}\n"))
            .collect::<String>()
    } else {
        fold_block_lines(&stripped)
    };
    apply_chomp(body, chomp)
}

fn strip_leading_spaces(line: &str, count: usize) -> &str {
    // Each stripped char is a single ASCII space (1 byte), so the byte
    // offset doubles as the stripped-count check — no separate counter.
    let mut byte_index = 0;
    while byte_index < count && line.as_bytes().get(byte_index) == Some(&b' ') {
        byte_index += 1;
    }
    &line[byte_index..]
}

/// YAML folded-scalar folding: a single newline between two non-blank lines
/// becomes a space; a blank line stays a literal newline.
fn fold_block_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            out.push('\n');
            continue;
        }
        out.push_str(line);
        match lines.get(i + 1) {
            Some(next) if next.is_empty() => out.push('\n'),
            Some(_) => out.push(' '),
            None => out.push('\n'),
        }
    }
    out
}

fn apply_chomp(body: String, chomp: Chomp) -> String {
    match chomp {
        Chomp::Keep => body,
        Chomp::Strip => body.trim_end_matches('\n').to_string(),
        Chomp::Clip => {
            let trimmed = body.trim_end_matches('\n');
            if body.ends_with('\n') {
                format!("{trimmed}\n")
            } else {
                trimmed.to_string()
            }
        }
    }
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
        // Would reparse as a non-string type: covers true/false, null/~/empty,
        // and every numeric form the classifier accepts (hex, octal, .inf,
        // .nan, exponents) — replaces the old NUM_RE / literal true|false checks.
        || waml_syntax::classify_bare_scalar(s) != FrontmatterScalarKind::Str
        || (s.starts_with('[') && s.ends_with(']'))
        || s.starts_with('"')
        // A leading `'` would be read back as opening a single-quoted
        // scalar (Task 8 generalizes this into the full structural-leader
        // set; this one is load-bearing now because an unquoted leading `'`
        // swallows the rest of a flow list's comma splitting).
        || s.starts_with('\'')
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
        FmValue::Null => "null".to_string(),
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
        // Bare block-mapping text, used only when a `Map` appears as a value
        // outside `render_frontmatter`'s own entry rendering (debug/test
        // paths) — `render_frontmatter` itself renders a `Map` entry via
        // `render_top_entry`/`render_map_block` with the key on its own line.
        // The full key-carrying nested/sequence-of-maps writer is Task 8's.
        FmValue::Map(entries) => render_map_block(entries, depth),
    }
}

/// Renders a map's entries as a two-space-indented block mapping, one
/// `key: value` (or `key:` + nested block) line per entry. An empty nested
/// map renders as `key:` with no value — the reader maps that back to
/// `Null`, an asymmetry Task 8 pins with a named test.
fn render_map_block(entries: &[(String, FmValue)], indent: usize) -> String {
    if indent >= MAX_VALUE_DEPTH {
        return String::new();
    }
    let pad = "  ".repeat(indent);
    let mut out = String::new();
    for (k, v) in entries {
        match v {
            FmValue::Map(inner) if inner.is_empty() => {
                out.push_str(&format!("{pad}{k}:\n"));
            }
            FmValue::Map(inner) => {
                out.push_str(&format!("{pad}{k}:\n"));
                out.push_str(&render_map_block(inner, indent + 1));
            }
            other => {
                out.push_str(&format!(
                    "{pad}{k}: {}\n",
                    render_value_at(other, indent + 1)
                ));
            }
        }
    }
    out
}

fn render_top_entry(out: &mut String, key: &str, value: &FmValue) {
    match value {
        FmValue::Map(entries) if entries.is_empty() => {
            out.push_str(&format!("{key}:\n"));
        }
        FmValue::Map(entries) => {
            out.push_str(&format!("{key}:\n"));
            out.push_str(&render_map_block(entries, 1));
        }
        other => out.push_str(&format!("{key}: {}\n", render_value(other))),
    }
}

pub fn render_frontmatter(fm: &Frontmatter) -> String {
    let mut out = String::new();
    for (k, v) in &fm.entries {
        render_top_entry(&mut out, k, v);
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

    /// Parses a whole document and returns the `Frontmatter` read from its
    /// leading fence, mirroring `crates/waml/src/okf/shell.rs::shell_fields`.
    fn parse_frontmatter_for_test(source: &str) -> Frontmatter {
        let text = SourceText::from_shared(std::sync::Arc::new(source.into())).unwrap();
        let snapshot = parse_markdown(
            DocumentRevision::INITIAL,
            text,
            MarkdownDialect::WAML_DEFAULT,
        )
        .expect("test source parses");
        for element in snapshot.tree().root().children() {
            let Some(node) = element.into_node() else {
                continue;
            };
            if let Some(fm) = parse_closed_syntax(&node) {
                return fm;
            }
        }
        Frontmatter::default()
    }

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
    fn parse_value_follows_yaml_12_core() {
        assert_eq!(parse_value("null"), FmValue::Null);
        assert_eq!(parse_value("~"), FmValue::Null);
        assert_eq!(parse_value(""), FmValue::Null);
        assert_eq!(parse_value("NO"), FmValue::Str("NO".into()));
        assert_eq!(parse_value("yes"), FmValue::Str("yes".into()));
        assert_eq!(parse_value("0x1A"), FmValue::Num(26.0));
        assert_eq!(parse_value("0o17"), FmValue::Num(15.0));
        assert_eq!(parse_value(".inf"), FmValue::Num(f64::INFINITY));
        assert_eq!(parse_value("-.inf"), FmValue::Num(f64::NEG_INFINITY));
        assert!(matches!(parse_value(".nan"), FmValue::Num(n) if n.is_nan()));
        assert_eq!(parse_value("6.02e23"), FmValue::Num(6.02e23));
        assert_eq!(parse_value("2026-08-04"), FmValue::Str("2026-08-04".into()));
        // Quoted is ALWAYS Str:
        assert_eq!(parse_value("\"true\""), FmValue::Str("true".into()));
        assert_eq!(parse_value("'it''s'"), FmValue::Str("it's".into()));
        // Nested flow with quote-aware, nesting-aware splitting:
        assert_eq!(
            parse_value("[a, [b, c], \"x,y\"]"),
            FmValue::List(vec![
                FmValue::Str("a".into()),
                FmValue::List(vec![FmValue::Str("b".into()), FmValue::Str("c".into())]),
                FmValue::Str("x,y".into()),
            ])
        );
    }

    #[test]
    fn double_quote_escapes_gain_tab_nul_unicode() {
        assert_eq!(
            parse_value("\"a\\tb\\0c\\u00e9\""),
            FmValue::Str("a\tb\0c\u{e9}".into())
        );
    }

    #[test]
    fn nested_frontmatter_reads_into_maps_and_lists() {
        let source = "---\nmeta:\n  owner: platform\n  n: 3\nauthors:\n  - name: Ana\n    team: platform\nstereotype:\n  - aggregateRoot\n  - entity\ndup: 1\ndup: 2\ndesc: |\n  line one\n  line two\nfolded: >\n  a\n  b\nempty:\n---\n";
        let fm = parse_frontmatter_for_test(source);
        assert_eq!(
            fm.get("meta"),
            Some(&FmValue::Map(vec![
                ("owner".into(), FmValue::Str("platform".into())),
                ("n".into(), FmValue::Num(3.0)),
            ]))
        );
        assert_eq!(
            fm.get("authors"),
            Some(&FmValue::List(vec![FmValue::Map(vec![
                ("name".into(), FmValue::Str("Ana".into())),
                ("team".into(), FmValue::Str("platform".into())),
            ])]))
        );
        assert_eq!(
            fm.get("stereotype"),
            Some(&FmValue::List(vec![
                FmValue::Str("aggregateRoot".into()),
                FmValue::Str("entity".into()),
            ]))
        );
        assert_eq!(
            fm.get("dup"),
            Some(&FmValue::Num(2.0)),
            "last duplicate wins"
        );
        assert_eq!(
            fm.get("desc"),
            Some(&FmValue::Str("line one\nline two\n".into())),
            "| keeps newlines, clip chomping"
        );
        assert_eq!(
            fm.get("folded"),
            Some(&FmValue::Str("a b\n".into())),
            "> folds to spaces"
        );
        assert_eq!(fm.get("empty"), Some(&FmValue::Null));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn wire_null_and_map_round_trip() {
        let v: FmValue = serde_json::from_str("null").unwrap();
        assert_eq!(
            v,
            FmValue::Null,
            "JSON null must be Null, not the string \"null\""
        );
        let v: FmValue = serde_json::from_str("{\"a\": 1, \"b\": null}").unwrap();
        assert_eq!(
            v,
            FmValue::Map(vec![
                ("a".into(), FmValue::Num(1.0)),
                ("b".into(), FmValue::Null),
            ])
        );
        // Hostile depth via maps is capped like lists:
        let deep = format!("{}1{}", "{\"k\":".repeat(100), "}".repeat(100));
        assert!(serde_json::from_str::<FmValue>(&deep).is_err());
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
        // A string that looks like a bool/num/list/empty/null must stay
        // quoted so it round-trips as a Str, not the type it resembles.
        for raw in [
            "true",
            "false",
            "42",
            "-3.5",
            "[a]",
            "",
            " leading",
            "has\"quote",
            "null",
            "~",
            ".inf",
            "0x1A",
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

    #[test]
    fn nested_map_renders_as_block_mapping_and_round_trips() {
        let fm = Frontmatter {
            entries: vec![(
                "meta".into(),
                FmValue::Map(vec![
                    ("owner".into(), FmValue::Str("platform".into())),
                    (
                        "detail".into(),
                        FmValue::Map(vec![("level".into(), FmValue::Num(3.0))]),
                    ),
                ]),
            )],
        };
        let rendered = render_frontmatter(&fm);
        assert_eq!(
            rendered,
            "meta:\n  owner: platform\n  detail:\n    level: 3"
        );
        let source = format!("---\n{rendered}\n---\n");
        let parsed = parse_frontmatter_for_test(&source);
        assert_eq!(parsed, fm);
    }

    #[test]
    fn empty_map_renders_and_reparses_as_null() {
        let fm = Frontmatter {
            entries: vec![("meta".into(), FmValue::Map(vec![]))],
        };
        let rendered = render_frontmatter(&fm);
        assert_eq!(rendered, "meta:");
        let source = format!("---\n{rendered}\n---\n");
        let parsed = parse_frontmatter_for_test(&source);
        assert_eq!(parsed.get("meta"), Some(&FmValue::Null));
    }

    use proptest::prelude::*;

    fn fm_value_strategy() -> impl Strategy<Value = FmValue> {
        let leaf = prop_oneof![
            Just(FmValue::Null),
            any::<bool>().prop_map(FmValue::Bool),
            // Finite, non-NaN, and re-parseable: round through the renderer's own
            // formatting so 1.0 vs 1 formatting differences don't fail spuriously.
            // Exclude -0.0: `total_cmp`-based PartialEq distinguishes it from
            // 0.0, but the renderer emits "0" for both.
            (-1.0e9f64..1.0e9)
                .prop_map(|n| (n * 1000.0).round() / 1000.0)
                .prop_map(|n| if n == 0.0 { 0.0 } else { n })
                .prop_map(FmValue::Num),
            // Printable ASCII plus some unicode; includes strings that LOOK like
            // bools, numbers, lists, quotes, backslashes — the quoting stress cases.
            prop_oneof![
                "[ -~]{0,24}",
                "(true|false|null|~|yes|NO|on|off)",
                "-?[0-9]{1,6}(\\.[0-9]{1,3})?",
                "\\[[a-z, ]{0,10}\\]?",
                "[\"'\\\\#:>|&*!%@`,\\-\\[\\]{} ]{1,8}",
                "\\PC{0,12}",
            ]
            .prop_map(FmValue::Str),
        ];
        // NOTE: `Map` is not yet part of this recursive strategy. Adding a
        // `Map` arm here (as a list item AND as an entry-level value) requires
        // the entry-level round-trip property below to reparse through the
        // full document tree (a Map entry cannot be checked as a single
        // rendered line) and requires sequence-of-map block rendering — both
        // are Task 8's job, landing together with the strategy addition so
        // the two never go out of sync. Task 7 lands `Map`-capable reading
        // and writing (see the tests above), pinned by targeted unit tests
        // instead of the shared property strategy.
        leaf.prop_recursive(4, 48, 8, |inner| {
            // NOTE: the filter runs on the Vec<FmValue>, BEFORE the prop_map wraps
            // it in FmValue::List. Filtering after the wrap does not compile —
            // `iter()` is not available on `&FmValue`.
            prop::collection::vec(inner, 0..6)
                // MANDATORY EXCLUSIONS — see the note below. Today's flow-list
                // parser splits on EVERY comma, quote- and nesting-blind, and the
                // writer never quotes list items. These shapes CANNOT round-trip
                // against current code. Task 8 makes flow parsing quote-aware and
                // nesting-aware and DELETES this filter as its first act.
                .prop_filter(
                    "Task 8 removes: flow list items cannot carry , [ ] \" \\ or be empty/nested",
                    |list| {
                        !list.iter().any(|item| match item {
                            FmValue::Str(s) => {
                                s.is_empty()
                                    || s.contains(',')
                                    || s.contains('[')
                                    || s.contains(']')
                                    || s.contains('"')
                                    || s.contains('\\')
                            }
                            // A nested list re-parses through the same comma splitter,
                            // so any multi-item inner list flattens. Single-item and
                            // empty inner lists survive.
                            FmValue::List(inner) => inner.len() > 1,
                            _ => false,
                        })
                    },
                )
                .prop_map(FmValue::List)
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// THE writer contract: parse_value(render_value(v)) == v, for every value.
        #[test]
        fn rendered_value_reparses_identically(v in fm_value_strategy()) {
            let rendered = render_value(&v);
            prop_assert_eq!(parse_value(&rendered), v.clone(), "rendered as {:?}", rendered);
        }

        /// Entry-level contract: a whole rendered frontmatter reparses line by line.
        #[test]
        fn rendered_frontmatter_entries_reparse(
            entries in prop::collection::vec(("[a-z][a-z0-9_]{0,8}", fm_value_strategy()), 0..5)
        ) {
            let fm = Frontmatter { entries: entries.clone() };
            let rendered = render_frontmatter(&fm);
            for (line, (key, value)) in rendered.lines().zip(&fm.entries) {
                let value_text = line.strip_prefix(&format!("{key}: "))
                    .or_else(|| line.strip_prefix(&format!("{key}:")))
                    .unwrap_or("");
                prop_assert_eq!(&parse_value(value_text), value, "line {:?}", line);
            }
        }
    }
}
