use waml_syntax::{
    FrontmatterScalarKind, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode,
    SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TextSize,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontmatterRewriteError {
    InvalidFrontmatter,
    NonStringScalar { key: String },
    InvalidReplacement,
}

impl std::fmt::Display for FrontmatterRewriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFrontmatter => formatter.write_str("frontmatter is invalid"),
            Self::NonStringScalar { key } => {
                write!(formatter, "frontmatter key '{key}' is not a string scalar")
            }
            Self::InvalidReplacement => formatter.write_str("replacement is not a scalar string"),
        }
    }
}

impl std::error::Error for FrontmatterRewriteError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrontmatterStringScalar {
    NoFrontmatter,
    NoScalar,
    String { value: String, range: TextRange },
}

fn trimmed_token_range(token: &SyntaxToken<OkfMarkdownLanguage>) -> TextRange {
    let authored = token.text().write_to_string();
    let trimmed = authored.trim();
    let leading = token
        .range()
        .len()
        .to_usize()
        .saturating_sub(authored.len())
        + authored.len()
        - authored.trim_start().len();
    let start = (token.range().start()
        + TextSize::try_from(leading).expect("token leading trivia fits TextSize"))
    .expect("trimmed token start fits TextSize");
    let end = (start + TextSize::try_from(trimmed.len()).expect("token spelling fits TextSize"))
        .expect("trimmed token end fits TextSize");
    TextRange::new(start, end).expect("trimmed token range is ordered")
}

/// Maximum `List`/`Map` nesting depth accepted from any input path (authored
/// frontmatter text and the serde wire form alike). Frontmatter is untrusted —
/// a hostile `[[[[…]]]]` or deeply nested `{"a":{"a":…}}` value must produce a
/// value or an error, never a stack overflow. Real frontmatter nests one or
/// two levels deep.
///
/// Pinned to the parser's own frontmatter cap rather than a second, tighter
/// number: nesting the parser accepts as clean must read back whole, or the
/// reader silently truncates a document the user is about to save.
const MAX_VALUE_DEPTH: usize = waml_syntax::FRONTMATTER_MAX_NESTING_DEPTH;

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

fn escape_quoted_string(value: &str) -> String {
    let normalized = waml_syntax::normalize_line_endings(value);
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

/// Splits a flow collection's inner text (`[...]` or `{...}` with the
/// brackets already stripped) on top-level commas, quote-aware and
/// depth-aware over BOTH bracket kinds — a flow sequence may hold flow
/// mappings and vice versa, so a comma inside either one does not split the
/// outer collection. A
/// quote character only opens a quoted scalar when it is the FIRST
/// significant byte of an ITEM — matching YAML, where a quote elsewhere
/// inside a plain (unquoted) scalar is just a literal character, not a
/// delimiter. An item boundary is the start of the string, right after a
/// (non-splitting) comma at any depth, or right after a `[` that opens a
/// nested list — that last case matters: without it, a quoted scalar that is
/// the first item of a NESTED list (e.g. `["[", ...]`) is not recognized as
/// quoted, so a `[`/`]` byte inside it is wrongly counted toward bracket
/// depth instead of being skipped as quoted content. A comma inside
/// `'...'`, `"..."`, or a nested `[...]` does not split.
fn split_flow_items(s: &str) -> Vec<&str> {
    if s.trim().is_empty() {
        return Vec::new();
    }
    let bytes = s.as_bytes();
    let mut items = Vec::new();
    let mut start = 0usize;
    let mut depth: i32 = 0;
    let mut i = 0usize;
    let mut at_item_start = true;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            i += 1;
            continue;
        }
        if at_item_start {
            if let Some(scan) = waml_syntax::scan_quoted_scalar(s, i, bytes.len()) {
                i = scan.end;
                at_item_start = false;
                continue;
            }
        }
        match bytes[i] {
            b'[' | b'{' => {
                depth += 1;
                i += 1;
                at_item_start = true;
            }
            b']' | b'}' => {
                depth -= 1;
                i += 1;
                at_item_start = false;
            }
            // A `: ` opens a mapping value, which may itself be a quoted
            // scalar (`{ id: "a, b" }`) — an item start for the quote
            // bookkeeping, exactly like the position after a comma. The
            // brackets are stripped before this runs, so a flow mapping's own
            // field separators sit at depth 0 here. The trailing space is
            // required, per YAML: a `:` not followed by one (`:'`) is an
            // ordinary character of a plain scalar, not an indicator.
            b':' if i + 1 < bytes.len() && bytes[i + 1] == b' ' => {
                i += 1;
                at_item_start = true;
            }
            b',' if depth <= 0 => {
                items.push(&s[start..i]);
                start = i + 1;
                i += 1;
                at_item_start = true;
            }
            b',' => {
                // A comma inside a nested bracket does not split the outer
                // sequence, but it does start a new item of the nested one —
                // a quote right after it must still be recognized as opening.
                i += 1;
                at_item_start = true;
            }
            _ => {
                i += 1;
                at_item_start = false;
            }
        }
    }
    items.push(&s[start..]);
    items
}

/// Reads a flow mapping's inner text (`{...}` with the braces stripped) into
/// ordered entries. `None` means "not a flat mapping" — an empty body, or a
/// field with no top-level `:` or an empty key — and the caller keeps the raw
/// text as a scalar instead of inventing structure for it.
fn parse_flow_map_entries(inner: &str, depth: usize) -> Option<Vec<(String, FmValue)>> {
    if inner.trim().is_empty() {
        return None;
    }
    let mut entries = Vec::new();
    for field in split_flow_items(inner) {
        let (key, value) = split_flow_map_field(field)?;
        let key = waml_syntax::decode_quoted_scalar(key.trim());
        if key.is_empty() {
            return None;
        }
        entries.push((key, parse_value_at(value.trim(), depth + 1)));
    }
    Some(entries)
}

/// Splits one flow-mapping field at its first top-level `:`, quote-aware and
/// depth-aware so a colon inside a quoted scalar or a nested flow collection
/// does not split. `None` when the field carries no such colon.
fn split_flow_map_field(field: &str) -> Option<(&str, &str)> {
    let bytes = field.as_bytes();
    let mut at = 0usize;
    let mut depth: i32 = 0;
    while at < bytes.len() {
        // A quote opens a scalar wherever it appears here, not only at an item
        // start: this runs on ONE field, already cut out by `split_flow_items`
        // under the item-start rule, so anything quote-shaped left in it is a
        // delimiter. Skipping the scalar keeps a `:` inside it from splitting.
        if let Some(scan) = waml_syntax::scan_quoted_scalar(field, at, bytes.len()) {
            at = scan.end;
            continue;
        }
        match bytes[at] {
            b'[' | b'{' => {
                depth += 1;
                at += 1;
            }
            b']' | b'}' => {
                depth -= 1;
                at += 1;
            }
            b':' if depth <= 0 => return Some((&field[..at], &field[at + 1..])),
            _ => at += 1,
        }
    }
    None
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
    // A flow mapping (`{ id: a, title: b }`) is the compact map form the OKF
    // spec writes `sources`/`parameters`/`verified` in. A shape that is not a
    // flat mapping — no fields, or a field with no `:` — is NOT one; it falls
    // through to the bare-scalar path and stays the `Str` it reads as, which
    // the writer quotes so it round-trips unchanged.
    if let Some(inner) = s.strip_prefix('{').and_then(|x| x.strip_suffix('}')) {
        if depth >= MAX_VALUE_DEPTH {
            return FmValue::Str(s.to_string());
        }
        if let Some(entries) = parse_flow_map_entries(inner, depth) {
            return FmValue::Map(entries);
        }
    }
    if s.len() >= 2 {
        if let Some(inner) = s.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
            return FmValue::Str(waml_syntax::decode_double_quoted_body(inner));
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
    // Radix integers: the exact `i64` path first, then a digit fold for a
    // literal too large for `i64` (`0xFFFFFFFFFFFFFFFFF`). Folding — rather
    // than the old `unwrap_or(0.0)` — keeps the magnitude instead of silently
    // rewriting the value as `0` on the next save.
    if let Some(hex) = s.strip_prefix("0x") {
        return i64::from_str_radix(hex, 16)
            .map(|n| n as f64)
            .unwrap_or_else(|_| fold_radix_digits(hex, 16.0));
    }
    if let Some(oct) = s.strip_prefix("0o") {
        return i64::from_str_radix(oct, 8)
            .map(|n| n as f64)
            .unwrap_or_else(|_| fold_radix_digits(oct, 8.0));
    }
    s.parse::<f64>().unwrap_or(0.0)
}

/// Accumulates already-validated radix digits into an `f64`. Loses precision
/// past 2^53 (as any `f64` must) but never the magnitude, and never fails —
/// the classifier guaranteed every byte is a digit of this radix.
fn fold_radix_digits(digits: &str, radix: f64) -> f64 {
    digits.chars().fold(0.0f64, |acc, c| {
        acc * radix + f64::from(c.to_digit(radix as u32).unwrap_or(0))
    })
}

fn contains_bad_frontmatter_token(node: &SyntaxNode<OkfMarkdownLanguage>) -> bool {
    node.children().any(|element| match element {
        SyntaxElement::Node(child) => contains_bad_frontmatter_token(&child),
        SyntaxElement::Token(token) => token.kind() == OkfMarkdownSyntaxKind::BadToken,
    })
}

/// True when `value` is exactly one closed quoted scalar and nothing else —
/// the shape the rewrite guard demands before it will touch a flow mapping.
fn quoted_flow_scalar_is_valid(value: &str) -> bool {
    waml_syntax::scan_quoted_scalar(value, 0, value.len())
        .is_some_and(|scan| !scan.unterminated && scan.end == value.len())
}

fn plain_flow_scalar_is_valid(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes
        .iter()
        .any(|byte| matches!(byte, b'\'' | b'"' | b'{' | b'}' | b'[' | b']'))
        && !bytes.ends_with(b":")
        && !bytes
            .windows(2)
            .any(|pair| pair[0] == b':' && pair[1].is_ascii_whitespace())
}

fn flow_map_field_is_valid(field: &str) -> bool {
    let field = field.trim();
    if field.is_empty() {
        return false;
    }
    let bytes = field.as_bytes();
    let mut at = 0;
    let mut colon = None;
    while at < bytes.len() {
        // Deliberately stricter than the READER: this guard treats a quote
        // anywhere as a delimiter, where `split_flow_items` only honours one
        // at an item start. It decides whether a rewrite may proceed, so
        // erring toward "not a flat mapping" costs a refused edit, while
        // erring the other way rewrites text we misread.
        if let Some(scan) = waml_syntax::scan_quoted_scalar(field, at, bytes.len()) {
            // An unterminated quote swallows the rest of the field, so no
            // top-level colon can follow it: not a `key: value`.
            if scan.unterminated {
                return false;
            }
            at = scan.end;
            continue;
        }
        if bytes[at] == b':' {
            colon = Some(at);
            break;
        }
        at += 1;
    }
    let Some(colon) = colon else {
        return false;
    };
    let key = field[..colon].trim();
    let mut key_bytes = key.bytes();
    let Some(first) = key_bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !key_bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return false;
    }
    let value = field[colon + 1..].trim();
    if value.is_empty() {
        return false;
    }
    if matches!(value.as_bytes().first(), Some(b'\'' | b'"')) {
        quoted_flow_scalar_is_valid(value)
    } else {
        plain_flow_scalar_is_valid(value)
    }
}

fn flat_flow_map_is_valid(value: &str) -> bool {
    let Some(inner) = value
        .strip_prefix('{')
        .and_then(|inner| inner.strip_suffix('}'))
    else {
        return false;
    };
    let inner = inner.trim();
    if inner.is_empty() {
        return false;
    }
    let bytes = inner.as_bytes();
    let mut field_start = 0;
    let mut at = 0;
    while at < bytes.len() {
        // Same conservative reading as `flow_map_field_is_valid`: a quote
        // anywhere is a delimiter, and one that never closes disqualifies the
        // whole mapping rather than being read as ordinary text.
        if let Some(scan) = waml_syntax::scan_quoted_scalar(inner, at, bytes.len()) {
            if scan.unterminated {
                return false;
            }
            at = scan.end;
            continue;
        }
        if bytes[at] == b',' {
            if !flow_map_field_is_valid(&inner[field_start..at]) {
                return false;
            }
            field_start = at + 1;
        }
        at += 1;
    }
    flow_map_field_is_valid(&inner[field_start..])
}

fn flow_map_value(line: &str) -> Option<&str> {
    let line = line.trim();
    let value = if let Some(value) = line.strip_prefix('-') {
        value.trim_start()
    } else {
        line.split_once(':')?.1.trim_start()
    };
    value.starts_with('{').then_some(value)
}

fn is_flow_map_recovery(source: &str, at: waml_syntax::TextSize) -> bool {
    let at = at.to_usize().min(source.len());
    let line_start = source[..at].rfind('\n').map_or(0, |newline| newline + 1);
    let line_end = source[at..]
        .find('\n')
        .map_or(source.len(), |newline| at + newline);
    flow_map_value(&source[line_start..line_end]).is_some_and(flat_flow_map_is_valid)
}

fn flow_maps_are_valid(source: &str) -> bool {
    source.lines().all(|line| {
        let Some(value) = flow_map_value(line) else {
            return true;
        };
        flat_flow_map_is_valid(value)
    })
}

fn validated_frontmatter_syntax(
    source: &str,
) -> Result<Option<SyntaxNode<OkfMarkdownLanguage>>, FrontmatterRewriteError> {
    let text = waml_syntax::SourceText::from_shared(std::sync::Arc::new(source.into()))
        .map_err(|_| FrontmatterRewriteError::InvalidFrontmatter)?;
    let snapshot = waml_syntax::parse_markdown(
        waml_syntax::DocumentRevision::INITIAL,
        text,
        waml_syntax::MarkdownDialect::WAML_DEFAULT,
    )
    .map_err(|_| FrontmatterRewriteError::InvalidFrontmatter)?;
    let Some(frontmatter) = snapshot
        .tree()
        .root()
        .children()
        .filter_map(SyntaxElement::into_node)
        .find(|node| node.kind() == OkfMarkdownSyntaxKind::Frontmatter)
    else {
        return Ok(None);
    };
    let closed = frontmatter.children().any(|element| {
        element.into_token().is_some_and(|token| {
            token.kind() == OkfMarkdownSyntaxKind::FrontmatterCloseFence
                && !token.flags().is_missing()
        })
    });
    if !closed {
        return Err(FrontmatterRewriteError::InvalidFrontmatter);
    }

    let range = frontmatter.range();
    let frontmatter_source = &source[range.start().to_usize()..range.end().to_usize()];
    if !flow_maps_are_valid(frontmatter_source) {
        return Err(FrontmatterRewriteError::InvalidFrontmatter);
    }
    let recovery_only = snapshot
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            diagnostic.range.start() >= range.start() && diagnostic.range.end() <= range.end()
        })
        .all(|diagnostic| match diagnostic.code {
            OkfSyntaxDiagnosticCode::FrontmatterNotClean => true,
            OkfSyntaxDiagnosticCode::MalformedFrontmatterEntry => {
                is_flow_map_recovery(source, diagnostic.range.start())
            }
            _ => false,
        });
    if !recovery_only || contains_bad_frontmatter_token(&frontmatter) {
        return Err(FrontmatterRewriteError::InvalidFrontmatter);
    }

    Ok(Some(frontmatter))
}

pub(crate) fn inspect_frontmatter_string_scalar(
    source: &str,
    key: &str,
) -> Result<FrontmatterStringScalar, FrontmatterRewriteError> {
    let Some(frontmatter) = validated_frontmatter_syntax(source)? else {
        return Ok(FrontmatterStringScalar::NoFrontmatter);
    };
    let Some(mapping) = frontmatter
        .children()
        .filter_map(SyntaxElement::into_node)
        .find(|node| node.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping)
    else {
        return Ok(FrontmatterStringScalar::NoScalar);
    };

    for entry in mapping
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|node| node.kind() == OkfMarkdownSyntaxKind::FrontmatterEntry)
    {
        let tokens = entry
            .children()
            .filter_map(SyntaxElement::into_token)
            .collect::<Vec<_>>();
        let Some(key_token) = tokens.first() else {
            continue;
        };
        let authored_key = match key_token.kind() {
            OkfMarkdownSyntaxKind::FrontmatterKey => key_token.text().write_to_string(),
            OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken => {
                waml_syntax::decode_quoted_scalar(&key_token.text().write_to_string())
            }
            _ => continue,
        };
        if authored_key != key {
            continue;
        }
        let Some(value_token) = tokens
            .iter()
            .skip_while(|token| token.kind() != OkfMarkdownSyntaxKind::ColonToken)
            .skip(1)
            .find(|token| {
                matches!(
                    token.kind(),
                    OkfMarkdownSyntaxKind::FrontmatterValue
                        | OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken
                )
            })
        else {
            return Err(FrontmatterRewriteError::NonStringScalar { key: key.into() });
        };
        let raw = value_token.text().write_to_string();
        let value = match value_token.kind() {
            OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken => {
                waml_syntax::decode_quoted_scalar(&raw)
            }
            OkfMarkdownSyntaxKind::FrontmatterValue => match parse_value(&raw) {
                FmValue::Str(value) => value,
                _ => {
                    return Err(FrontmatterRewriteError::NonStringScalar { key: key.into() });
                }
            },
            _ => return Err(FrontmatterRewriteError::NonStringScalar { key: key.into() }),
        };
        return Ok(FrontmatterStringScalar::String {
            value,
            range: trimmed_token_range(value_token),
        });
    }

    Ok(FrontmatterStringScalar::NoScalar)
}

/// Reads the leading frontmatter fence of a whole document's `source`, using
/// the same parser and the same value rules as every other reader. Returns
/// `None` when the source carries no closed frontmatter fence — a caller that
/// is about to rewrite the fence must then leave the bytes alone rather than
/// guess at their meaning.
pub(crate) fn parse_frontmatter_source(source: &str) -> Option<Frontmatter> {
    let text = waml_syntax::SourceText::from_shared(std::sync::Arc::new(source.into())).ok()?;
    let snapshot = waml_syntax::parse_markdown(
        waml_syntax::DocumentRevision::INITIAL,
        text,
        waml_syntax::MarkdownDialect::WAML_DEFAULT,
    )
    .ok()?;
    snapshot
        .tree()
        .root()
        .children()
        .filter_map(SyntaxElement::into_node)
        .find_map(|node| parse_closed_syntax(&node))
}

/// Replaces one top-level frontmatter string scalar without re-rendering the
/// document. The returned text differs only in the scalar token range.
pub fn replace_frontmatter_string_scalar(
    source: &str,
    key: &str,
    expected: &str,
    replacement: &str,
) -> Result<Option<String>, FrontmatterRewriteError> {
    let Some(frontmatter) = validated_frontmatter_syntax(source)? else {
        return Ok(None);
    };
    let Some(mapping) = frontmatter
        .children()
        .filter_map(SyntaxElement::into_node)
        .find(|node| node.kind() == OkfMarkdownSyntaxKind::FrontmatterMapping)
    else {
        return Ok(None);
    };

    for entry in mapping
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|node| node.kind() == OkfMarkdownSyntaxKind::FrontmatterEntry)
    {
        let tokens: Vec<_> = entry
            .children()
            .filter_map(SyntaxElement::into_token)
            .collect();
        let Some(key_token) = tokens.first() else {
            continue;
        };
        let authored_key = match key_token.kind() {
            OkfMarkdownSyntaxKind::FrontmatterKey => key_token.text().write_to_string(),
            OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken => {
                waml_syntax::decode_quoted_scalar(&key_token.text().write_to_string())
            }
            _ => continue,
        };
        if authored_key != key {
            continue;
        }
        let Some(value_token) = tokens
            .iter()
            .skip_while(|token| token.kind() != OkfMarkdownSyntaxKind::ColonToken)
            .skip(1)
            .find(|token| {
                matches!(
                    token.kind(),
                    OkfMarkdownSyntaxKind::FrontmatterValue
                        | OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken
                )
            })
        else {
            return Err(FrontmatterRewriteError::NonStringScalar { key: key.into() });
        };
        let raw = value_token.text().write_to_string();
        let (value, rendered) = match value_token.kind() {
            OkfMarkdownSyntaxKind::FrontmatterQuotedValueToken => {
                let quote =
                    raw.as_bytes().first().copied().ok_or_else(|| {
                        FrontmatterRewriteError::NonStringScalar { key: key.into() }
                    })?;
                if replacement.contains(['\r', '\n']) {
                    return Err(FrontmatterRewriteError::InvalidReplacement);
                }
                let rendered = match quote {
                    b'\'' => format!("'{}'", replacement.replace('\'', "''")),
                    b'\"' => format!(
                        "\"{}\"",
                        replacement.replace('\\', "\\\\").replace('\"', "\\\"")
                    ),
                    _ => return Err(FrontmatterRewriteError::NonStringScalar { key: key.into() }),
                };
                (waml_syntax::decode_quoted_scalar(&raw), rendered)
            }
            OkfMarkdownSyntaxKind::FrontmatterValue => {
                if waml_syntax::classify_bare_scalar(&raw) != FrontmatterScalarKind::Str {
                    return Err(FrontmatterRewriteError::NonStringScalar { key: key.into() });
                }
                if scalar_needs_quote(replacement) {
                    return Err(FrontmatterRewriteError::InvalidReplacement);
                }
                (raw, replacement.to_string())
            }
            _ => unreachable!("value token kind was filtered"),
        };
        if value != expected {
            return Ok(None);
        }
        let range = value_token.trimmed_range();
        let mut rewritten = source.to_string();
        rewritten.replace_range(range.start().to_usize()..range.end().to_usize(), &rendered);
        return Ok(Some(rewritten));
    }
    Ok(None)
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
                waml_syntax::decode_quoted_scalar(&key_token.text().write_to_string())
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
                return Some(FmValue::Str(waml_syntax::decode_quoted_scalar(
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
/// same `Str`, or would otherwise confuse the writer's own line/flow
/// structure — the cases where quoting is required. A bare comma is NOT
/// listed: a top-level scalar with a comma is safe bare (only bracketed
/// values split on commas); [`flow_item_needs_quote`] adds it back for list
/// items, where a comma DOES need escaping.
fn scalar_needs_quote(s: &str) -> bool {
    if s.is_empty() || s != s.trim() {
        return true;
    }
    // Would reparse as a non-string type: covers true/false, null/~/empty,
    // and every numeric form the classifier accepts (hex, octal, .inf,
    // .nan, exponents).
    if waml_syntax::classify_bare_scalar(s) != FrontmatterScalarKind::Str {
        return true;
    }
    // Structural first character: any of these would be read as opening a
    // flow collection, a comment, an anchor/alias/tag/directive, a block
    // scalar header, or a quoted scalar.
    let first = s.chars().next().expect("non-empty checked above");
    if matches!(
        first,
        '#' | '['
            | ']'
            | '{'
            | '}'
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '%'
            | '@'
            | '`'
            | '"'
            | '\''
            | ','
    ) {
        return true;
    }
    // A leading `- ` (or bare `-`) reads as a block sequence dash.
    if s == "-" || s.starts_with("- ") {
        return true;
    }
    // Would open a mapping or a comment mid-scalar, or confuse the writer's
    // own quoted forms, escapes, or line structure.
    s.contains(": ")
        || s.ends_with(':')
        || s.contains(" #")
        || s.contains('\\')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
}

/// A list item needs quoting whenever a bare scalar would ([`scalar_needs_quote`]),
/// or when it carries a byte the flow splitter treats as structural — `,`
/// (the item separator) or `[`/`]`/`{`/`}` (nested-collection boundaries, both
/// kinds, since the splitter tracks depth over both) — even though those bytes
/// are safe in a top-level bare scalar.
fn flow_item_needs_quote(s: &str) -> bool {
    scalar_needs_quote(s)
        || s.contains(',')
        || s.contains('[')
        || s.contains(']')
        || s.contains('{')
        || s.contains('}')
}

/// True when a bare `key` would not read back as the same key. Keys reach the
/// writer from `Concept.extra` wire JSON, not just from authored source, so
/// this must hold for arbitrary text: an empty key, one with leading/trailing
/// space, a structural first character, or one carrying `:`, `#`, or a line
/// break would otherwise re-parse as different structure.
fn key_needs_quote(key: &str) -> bool {
    scalar_needs_quote(key) || key.contains(':') || key.contains('#')
}

/// Renders a mapping key, quoting (and escaping) it when bare text would not
/// survive the round trip. The reader decodes a quoted key back to its text
/// (`decode_quoted_scalar`), so quoting is always safe.
fn render_key(key: &str) -> String {
    if key_needs_quote(key) {
        format!("\"{}\"", escape_quoted_string(key))
    } else {
        key.to_string()
    }
}

pub(crate) fn render_value(v: &FmValue) -> String {
    render_value_at(v, 0)
}

fn render_value_at(v: &FmValue, depth: usize) -> String {
    match v {
        FmValue::Null => "null".to_string(),
        FmValue::Num(n) => {
            // `.inf`/`.nan` have no plain-decimal form: rendering them via
            // `{n}` emits `inf`/`NaN`, which reparse as strings.
            if n.is_nan() {
                ".nan".to_string()
            } else if n.is_infinite() {
                if *n > 0.0 { ".inf" } else { "-.inf" }.to_string()
            } else if n.fract() == 0.0 && n.abs() < 9.0e18 {
                // Guarded: an `as i64` cast saturates, which would rewrite a
                // magnitude past `i64` (reachable from a big hex literal) as
                // `i64::MAX`.
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
                .map(|item| render_flow_item(item, depth + 1))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        // Bare block-mapping text. No writer path reaches this arm — every
        // caller that can hold a `Map` (`render_entry`, `render_flow_item`,
        // `render_block_sequence_item`) matches `Map` before delegating here,
        // so a map is always rendered with its key on its own line. Kept as a
        // total fallback rather than an `unreachable!`: this renderer runs on
        // untrusted document content, where a panic costs the session.
        FmValue::Map(entries) => render_map_block(entries, depth),
    }
}

/// Renders a map's entries as a two-space-indented block mapping, one
/// `key: value` (or `key:` + nested block) line per entry. An empty nested
/// map renders as `key:` with no value — the reader maps that back to
/// `Null`, a deliberate asymmetry pinned by `empty_map_renders_and_reparses_as_null`.
///
/// This is the flowless bare-`Map` renderer `render_value_at`'s `Map` arm
/// falls back to (a `Map` value reached without a carrying key — not a path
/// `render_frontmatter` itself takes, which always goes through
/// [`render_entry`] so a nested map's key sits on its own line).
fn render_map_block(entries: &[(String, FmValue)], indent: usize) -> String {
    if indent >= MAX_VALUE_DEPTH {
        return String::new();
    }
    let pad = "  ".repeat(indent);
    let mut out = String::new();
    for (k, v) in entries {
        match v {
            FmValue::Map(inner) if inner.is_empty() => {
                out.push_str(&format!("{pad}{}:\n", render_key(k)));
            }
            FmValue::Map(inner) => {
                out.push_str(&format!("{pad}{}:\n", render_key(k)));
                out.push_str(&render_map_block(inner, indent + 1));
            }
            other => {
                out.push_str(&format!(
                    "{pad}{}: {}\n",
                    render_key(k),
                    render_value_at(other, indent + 1)
                ));
            }
        }
    }
    out
}

/// A list item, in flow-sequence position: a bare scalar quotes when
/// [`flow_item_needs_quote`] says so (adds `,`/`[`/`]` to the top-level
/// scalar rule); any other shape (nested list, or a bare number/bool/null)
/// renders through the ordinary value renderer, since a nested flow list is
/// unambiguous inside brackets. A `Map` item normally never reaches here — a
/// list whose DIRECT item is a `Map` renders as a block sequence instead, in
/// [`render_entry`] — but a `Map` nested deeper (list-of-list-of-map) does,
/// and degrades to a quoted one-line `{k: v}` scalar so the output stays
/// parseable.
fn render_flow_item(item: &FmValue, depth: usize) -> String {
    match item {
        FmValue::Str(s) => {
            if flow_item_needs_quote(s) {
                format!("\"{}\"", escape_quoted_string(s))
            } else {
                s.clone()
            }
        }
        // A `Map` has no block form here (nothing carries its key) and
        // `render_map_block` is multi-line — raw newlines inside `[...]`
        // would write unparseable frontmatter back into the document. This
        // shape is reachable from wire JSON, so degrade to a quoted
        // single-line scalar rather than emitting invalid output.
        FmValue::Map(entries) => format!(
            "\"{}\"",
            escape_quoted_string(&render_flow_map(entries, depth))
        ),
        other => render_value_at(other, depth),
    }
}

/// Renders a `Map` that has no block form as a single-line `{k: v, ...}`
/// scalar body. Never emits a newline; the caller quotes the result.
fn render_flow_map(entries: &[(String, FmValue)], depth: usize) -> String {
    if depth >= MAX_VALUE_DEPTH {
        return "{}".to_string();
    }
    let inner = entries
        .iter()
        .map(|(k, v)| format!("{}: {}", render_key(k), render_flow_inline(v, depth + 1)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{{inner}}}")
}

/// Single-line rendering of a value already inside a degraded flow mapping:
/// nested containers stay inline so the whole scalar remains one line.
fn render_flow_inline(value: &FmValue, depth: usize) -> String {
    match value {
        FmValue::Map(entries) => render_flow_map(entries, depth),
        FmValue::List(items) => {
            if depth >= MAX_VALUE_DEPTH {
                return "[]".to_string();
            }
            let inner = items
                .iter()
                .map(|item| render_flow_inline(item, depth + 1))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        other => render_value_at(other, depth),
    }
}

/// Renders one `key: value` line (recursing into indented block form for
/// nested maps and sequences of maps). `indent` is the two-space indent
/// LEVEL the key itself sits at.
///
/// The sequence-of-maps check below is DIRECT-child only (`matches!(item,
/// Map(_))`, not "contains a `Map` anywhere"): a `List` item that is itself
/// a nested `List` containing a `Map` (list-of-list-of-map) has no block
/// form the current parser reads back — a bare `-` with nothing after it,
/// followed by a more-indented `-` continuation, does not open a nested
/// sequence the way `key:` does. That combination does not round-trip as a
/// `Map` (see `representable_value` in the tests below); the writer degrades
/// it to a quoted one-line `{k: v}` scalar in [`render_flow_item`] so the
/// emitted frontmatter still parses. Widening the parser to read that shape
/// back is future work, not this task's.
fn render_entry(out: &mut String, key: &str, value: &FmValue, indent: usize) {
    if indent >= MAX_VALUE_DEPTH {
        return;
    }
    let pad = "  ".repeat(indent);
    match value {
        FmValue::Map(entries) => {
            out.push_str(&format!("{pad}{}:\n", render_key(key)));
            // An empty map has no block form and no flow form; the reader
            // maps a valueless key back to `Null`, so this deliberately
            // round-trips as Null rather than Map([]).
            for (k, v) in entries {
                render_entry(out, k, v, indent + 1);
            }
        }
        FmValue::List(items) if items.iter().any(|item| matches!(item, FmValue::Map(_))) => {
            out.push_str(&format!("{pad}{}:\n", render_key(key)));
            for item in items {
                render_block_sequence_item(out, item, indent + 1);
            }
        }
        other => {
            out.push_str(&format!(
                "{pad}{}: {}\n",
                render_key(key),
                render_value_at(other, indent + 1)
            ));
        }
    }
}

/// Renders one `- ` block-sequence item at `indent`. A `Map` item shares its
/// first entry's line with the dash (`- name: Ana`); a nested container
/// under that first key drops to `indent + 2` (past both the dash and the
/// key). Any other item shape (including a nested `List`, whether or not it
/// carries a `Map` — see `render_entry`'s doc comment) is a single
/// `- <flow value>` line.
fn render_block_sequence_item(out: &mut String, item: &FmValue, indent: usize) {
    if indent >= MAX_VALUE_DEPTH {
        return;
    }
    let pad = "  ".repeat(indent);
    match item {
        FmValue::Map(entries) => {
            if entries.is_empty() {
                out.push_str(&format!("{pad}-\n"));
                return;
            }
            for (i, (k, v)) in entries.iter().enumerate() {
                if i == 0 {
                    match v {
                        // A `Map` always gets its own block (even empty —
                        // that intentionally round-trips as `Null`, the
                        // same asymmetry `render_entry` has and the
                        // strategy normalizes away for generated values).
                        // A `List` only needs a block when it directly
                        // holds a `Map` (a sequence of maps); an empty or
                        // scalar-only list has a perfectly good flow form
                        // and must NOT be treated as a nested container —
                        // that would emit a valueless key with nothing
                        // after it, silently turning `List([])` into `Null`.
                        FmValue::Map(_) => {
                            out.push_str(&format!("{pad}- {}:\n", render_key(k)));
                            render_nested_under(out, v, indent + 2);
                        }
                        FmValue::List(items)
                            if items.iter().any(|item| matches!(item, FmValue::Map(_))) =>
                        {
                            out.push_str(&format!("{pad}- {}:\n", render_key(k)));
                            render_nested_under(out, v, indent + 2);
                        }
                        scalar => {
                            out.push_str(&format!(
                                "{pad}- {}: {}\n",
                                render_key(k),
                                render_value(scalar)
                            ));
                        }
                    }
                } else {
                    render_entry(out, k, v, indent + 1);
                }
            }
        }
        other => out.push_str(&format!("{pad}- {}\n", render_value_at(other, indent + 1))),
    }
}

/// Renders a `Map`'s entries or a `List`'s items at `indent`, with no
/// carrying key of their own — used only for the container that sits under a
/// block-sequence item's first key (`- name:\n    <nested here>`).
fn render_nested_under(out: &mut String, value: &FmValue, indent: usize) {
    if indent >= MAX_VALUE_DEPTH {
        return;
    }
    match value {
        FmValue::Map(entries) => {
            for (k, v) in entries {
                render_entry(out, k, v, indent);
            }
        }
        FmValue::List(items) => {
            for item in items {
                render_block_sequence_item(out, item, indent);
            }
        }
        other => {
            let pad = "  ".repeat(indent);
            out.push_str(&format!("{pad}{}\n", render_value_at(other, indent)));
        }
    }
}

pub fn render_frontmatter(fm: &Frontmatter) -> String {
    let mut out = String::new();
    for (k, v) in &fm.entries {
        render_entry(&mut out, k, v, 0);
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

    /// A `Map` reached in FLOW position (inside `[...]`) has no block form,
    /// but it must still render on ONE line: a multi-line `render_map_block`
    /// inside brackets writes unparseable frontmatter back into the user's
    /// document. This shape is reachable from wire JSON (`FmValueSeed`
    /// accepts arbitrary nesting), so the writer degrades to a quoted
    /// single-line scalar instead.
    #[test]
    fn map_inside_a_flow_sequence_degrades_to_one_line() {
        let fm = Frontmatter {
            entries: vec![(
                "k".into(),
                FmValue::List(vec![FmValue::List(vec![FmValue::Map(vec![(
                    "a".into(),
                    FmValue::Str("b".into()),
                )])])]),
            )],
        };
        let rendered = render_frontmatter(&fm);
        assert_eq!(rendered.lines().count(), 1, "rendered: {rendered:?}");

        let reparsed = parse_frontmatter_for_test(&format!("---\n{rendered}\n---\n"));
        assert_eq!(
            reparsed.get("k"),
            Some(&FmValue::List(vec![FmValue::List(vec![FmValue::Str(
                "{a: b}".into()
            )])]))
        );
    }

    /// A `{...}` flow mapping is the compact map form the OKF spec writes its
    /// `sources`/`parameters`/`verified` examples in, so it must READ as a
    /// `Map` — otherwise the entry falls back to a `Str`, the shell's
    /// `promote_sources` refuses it, and the writer quotes the whole line back
    /// into the document. The canonical WRITTEN form stays the block sequence;
    /// what matters is that the round trip preserves the value.
    #[test]
    fn a_flow_mapping_reads_as_a_map_and_round_trips() {
        let source = "---\nsources:\n  - { id: a, resource: x.rs, title: \"A, B\" }\n  - { id: b, resource: y.rs }\n---\n";
        let fm = parse_frontmatter_for_test(source);
        let expected = FmValue::List(vec![
            FmValue::Map(vec![
                ("id".into(), FmValue::Str("a".into())),
                ("resource".into(), FmValue::Str("x.rs".into())),
                ("title".into(), FmValue::Str("A, B".into())),
            ]),
            FmValue::Map(vec![
                ("id".into(), FmValue::Str("b".into())),
                ("resource".into(), FmValue::Str("y.rs".into())),
            ]),
        ]);
        assert_eq!(fm.get("sources"), Some(&expected));

        let rendered = render_frontmatter(&fm);
        let reparsed = parse_frontmatter_for_test(&format!("---\n{rendered}\n---\n"));
        assert_eq!(reparsed.get("sources"), Some(&expected));
    }

    /// A flow mapping in mapping-VALUE position reads the same way, and a
    /// shape that is not a valid flat mapping stays the bare scalar it was
    /// (the writer then quotes it, which round-trips unchanged).
    #[test]
    fn flow_mapping_values_and_non_mappings() {
        assert_eq!(
            parse_value("{ a: 1, b: \"x, y\" }"),
            FmValue::Map(vec![
                ("a".into(), FmValue::Num(1.0)),
                ("b".into(), FmValue::Str("x, y".into())),
            ])
        );
        assert_eq!(
            parse_value("{ outer: { inner: 1 } }"),
            FmValue::Map(vec![(
                "outer".into(),
                FmValue::Map(vec![("inner".into(), FmValue::Num(1.0))]),
            )])
        );
        assert_eq!(
            parse_value("[{ a: 1 }, { b: 2 }]"),
            FmValue::List(vec![
                FmValue::Map(vec![("a".into(), FmValue::Num(1.0))]),
                FmValue::Map(vec![("b".into(), FmValue::Num(2.0))]),
            ])
        );
        // No fields, no colon, and an unterminated brace are not mappings.
        assert_eq!(parse_value("{}"), FmValue::Str("{}".into()));
        assert_eq!(parse_value("{ a }"), FmValue::Str("{ a }".into()));
        assert_eq!(parse_value("{ a: 1"), FmValue::Str("{ a: 1".into()));
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
        assert_eq!(parse_value("on"), FmValue::Str("on".into()));
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

    /// The model's cap must never be tighter than the parser's: nesting the
    /// parser accepts without a diagnostic has to read back whole. When the
    /// reader stopped short, the levels past the cap — and the leaf under
    /// them — were silently rewritten away on the next save.
    #[test]
    fn nesting_the_parser_accepts_reads_back_without_truncation() {
        let levels = 40;
        let mut source = String::from("---\n");
        for level in 0..levels {
            source.push_str(&"  ".repeat(level));
            source.push_str("k:\n");
        }
        source.push_str(&"  ".repeat(levels));
        source.push_str("leaf: 1\n---\n");

        let fm = parse_frontmatter_for_test(&source);
        let mut value = fm.get("k").expect("the top-level key is present");
        for level in 1..levels {
            let FmValue::Map(entries) = value else {
                panic!("level {level} truncated to {value:?}");
            };
            value = &entries
                .iter()
                .find(|(key, _)| key == "k")
                .unwrap_or_else(|| panic!("level {level} has no nested `k`"))
                .1;
        }
        assert_eq!(
            value,
            &FmValue::Map(vec![("leaf".to_string(), FmValue::Num(1.0))])
        );
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
    fn hash_after_space_is_safe_quoted_at_top_level_and_inside_a_flow_list() {
        // A quoted top-level scalar containing " #" round-trips fine: the
        // tokenizer only special-cases whitespace-then-# for BARE (unquoted)
        // scalars, and correctly stays inside a quoted run.
        let fm = Frontmatter {
            entries: vec![("a".into(), FmValue::Str(" #".into()))],
        };
        let rendered = render_frontmatter(&fm);
        let source = format!("---\n{rendered}\n---\n");
        assert_eq!(parse_frontmatter_for_test(&source), fm);

        // And inside a flow list: the token scan is quote-aware for the
        // whitespace-# cutoff too, so `[" #"]` keeps the whole run.
        let fm2 = Frontmatter {
            entries: vec![("a".into(), FmValue::List(vec![FmValue::Str(" #".into())]))],
        };
        let rendered2 = render_frontmatter(&fm2);
        let source2 = format!("---\n{rendered2}\n---\n");
        assert_eq!(parse_frontmatter_for_test(&source2), fm2);
    }

    #[test]
    fn block_sequence_item_that_is_a_flow_list_with_a_colon_round_trips() {
        // `- [": "]` must read as one flow-list item, not as a `- key: value`
        // map entry: the dash line's key scan stops at a `[`.
        let fm = Frontmatter {
            entries: vec![(
                "items".into(),
                FmValue::List(vec![
                    FmValue::List(vec![FmValue::Str(": ".into())]),
                    FmValue::Map(vec![("k".into(), FmValue::Num(1.0))]),
                ]),
            )],
        };
        let rendered = render_frontmatter(&fm);
        let source = format!("---\n{rendered}\n---\n");
        assert_eq!(parse_frontmatter_for_test(&source), fm, "{rendered}");
    }

    #[test]
    fn structural_keys_are_quoted_and_round_trip() {
        for key in [
            "",
            "a: b",
            "#lead",
            "- dash",
            " padded ",
            "with\nnewline",
            "true",
            "0x10",
            "[bracket]",
            "trailing:",
        ] {
            let fm = Frontmatter {
                entries: vec![(key.to_string(), FmValue::Str("v".into()))],
            };
            let rendered = render_frontmatter(&fm);
            let source = format!("---\n{rendered}\n---\n");
            assert_eq!(
                parse_frontmatter_for_test(&source),
                fm,
                "key {key:?} rendered as {rendered:?}"
            );
        }
    }

    #[test]
    fn nested_map_keys_are_quoted_and_round_trip() {
        let fm = Frontmatter {
            entries: vec![(
                "meta".into(),
                FmValue::Map(vec![("a: b".into(), FmValue::Str("v".into()))]),
            )],
        };
        let rendered = render_frontmatter(&fm);
        let source = format!("---\n{rendered}\n---\n");
        assert_eq!(parse_frontmatter_for_test(&source), fm, "{rendered}");
    }

    #[test]
    fn out_of_range_radix_literal_keeps_its_magnitude() {
        // `i64::from_str_radix` overflows here; the value must not collapse
        // to 0 (which the next save would then write out as `0`).
        let parsed = parse_value("0xFFFFFFFFFFFFFFFFF");
        let FmValue::Num(n) = parsed else {
            panic!("expected a number, got {parsed:?}");
        };
        assert!(n > 2.9e20 && n < 3.0e20, "got {n}");
        // And it survives a save/reload rather than saturating to i64::MAX.
        let fm = Frontmatter {
            entries: vec![("n".into(), FmValue::Num(n))],
        };
        let rendered = render_frontmatter(&fm);
        let source = format!("---\n{rendered}\n---\n");
        assert_eq!(parse_frontmatter_for_test(&source), fm, "{rendered}");
    }

    #[test]
    fn infinities_render_in_yaml_core_form() {
        for n in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
            let fm = Frontmatter {
                entries: vec![("n".into(), FmValue::Num(n))],
            };
            let rendered = render_frontmatter(&fm);
            let source = format!("---\n{rendered}\n---\n");
            assert_eq!(parse_frontmatter_for_test(&source), fm, "{rendered}");
        }
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
    fn sequence_of_maps_renders_as_block_sequence() {
        let fm = Frontmatter {
            entries: vec![(
                "authors".into(),
                FmValue::List(vec![FmValue::Map(vec![
                    ("name".into(), FmValue::Str("Ana".into())),
                    ("team".into(), FmValue::Str("platform".into())),
                ])]),
            )],
        };
        let rendered = render_frontmatter(&fm);
        assert_eq!(rendered, "authors:\n  - name: Ana\n    team: platform");
        let source = format!("---\n{rendered}\n---\n");
        assert_eq!(parse_frontmatter_for_test(&source), fm);
    }

    /// YAML lets a block sequence sit at its key's own indentation, and that
    /// is the form most authored frontmatter uses. Reading it as `Null` made
    /// every such list invisible to the model.
    #[test]
    fn block_sequence_at_the_keys_own_indent_reads_as_a_list() {
        let parsed = parse_frontmatter_for_test("---\ntags:\n- a\n- b\ntitle: T\n---\n");
        assert_eq!(
            parsed,
            Frontmatter {
                entries: vec![
                    (
                        "tags".into(),
                        FmValue::List(vec![FmValue::Str("a".into()), FmValue::Str("b".into()),]),
                    ),
                    ("title".into(), FmValue::Str("T".into())),
                ],
            }
        );
    }

    #[test]
    fn nested_block_sequence_at_its_keys_indent_reads_as_a_list() {
        let parsed =
            parse_frontmatter_for_test("---\nmeta:\n  tags:\n  - a\n  - b\n  owner: ana\n---\n");
        assert_eq!(
            parsed,
            Frontmatter {
                entries: vec![(
                    "meta".into(),
                    FmValue::Map(vec![
                        (
                            "tags".into(),
                            FmValue::List(
                                vec![FmValue::Str("a".into()), FmValue::Str("b".into()),]
                            ),
                        ),
                        ("owner".into(), FmValue::Str("ana".into())),
                    ]),
                )],
            }
        );
    }

    /// A sequence of maps at the key's own indent: the item's deeper keys must
    /// still land inside the item, not in the enclosing mapping.
    #[test]
    fn block_sequence_of_maps_at_the_keys_own_indent_reads_as_a_list() {
        let parsed =
            parse_frontmatter_for_test("---\nauthors:\n- name: Ana\n  team: platform\n---\n");
        assert_eq!(
            parsed,
            Frontmatter {
                entries: vec![(
                    "authors".into(),
                    FmValue::List(vec![FmValue::Map(vec![
                        ("name".into(), FmValue::Str("Ana".into())),
                        ("team".into(), FmValue::Str("platform".into())),
                    ])]),
                )],
            }
        );
    }

    /// A dash with no key waiting on a value is still not a sequence: it has
    /// no reading, and the model would drop it silently.
    #[test]
    fn dash_with_no_open_key_is_still_malformed() {
        assert_eq!(
            parse_frontmatter_for_test("---\ntitle: T\n- a\n---\n"),
            Frontmatter {
                entries: vec![("title".into(), FmValue::Str("T".into()))],
            }
        );
    }

    #[test]
    fn existing_documents_render_byte_identical() {
        // Compatibility pin: scalar-only lists stay flow, not block.
        let fm = Frontmatter {
            entries: vec![
                ("type".into(), FmValue::Str("uml.Class".into())),
                (
                    "stereotype".into(),
                    FmValue::List(vec![
                        FmValue::Str("aggregateRoot".into()),
                        FmValue::Str("entity".into()),
                    ]),
                ),
            ],
        };
        assert_eq!(
            render_frontmatter(&fm),
            "type: uml.Class\nstereotype: [aggregateRoot, entity]"
        );
    }

    #[test]
    fn norway_no_longer_needs_quoting_but_new_structurals_do() {
        assert!(!scalar_needs_quote("NO"));
        assert!(!scalar_needs_quote("yes"));
        for s in [
            "null", "~", ".inf", "-.inf", ".nan", "0x1A", "0o17", "1e3", "- item", "#x", "[x",
            "{a}", "&a", "*a", "!t", "|", ">f", "%v", "@a", "`c", "'q", "\"q", "a: b", "a #b",
            "key:",
        ] {
            assert!(scalar_needs_quote(s), "{s:?} must quote");
        }
        assert!(
            !scalar_needs_quote("a:b"),
            "colon without space is safe in YAML"
        );
        assert!(!scalar_needs_quote("2026-08-04"));
    }

    #[test]
    fn flow_list_items_quote_commas_and_brackets() {
        let fm = Frontmatter {
            entries: vec![(
                "items".into(),
                FmValue::List(vec![
                    FmValue::Str("a,b".into()),
                    FmValue::Str("[nested]".into()),
                    FmValue::Str("plain".into()),
                ]),
            )],
        };
        let rendered = render_frontmatter(&fm);
        let value_text = rendered.strip_prefix("items: ").unwrap();
        assert_eq!(parse_value(value_text), fm.entries[0].1);
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

    /// Mapping keys as they actually arrive: mostly identifier-shaped, but
    /// `Concept.extra` wire JSON can carry anything — empty, padded, or
    /// carrying `:`/`#`/a line break. A carriage return is excluded only
    /// because quoted decoding normalizes line endings (a CR key decodes back
    /// as LF), which is the documented scalar rule, not a key bug.
    fn key_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            "[a-z][a-z0-9_]{0,8}",
            "[ -~\\n]{0,6}",
            "(true|null|0x10|- x|#c|a: b)",
        ]
    }

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
        // Flow-list parsing is now quote-aware and nesting-aware
        // (`split_flow_items`), and the writer quotes any list item that
        // needs it (`flow_item_needs_quote`), so items may freely carry `,`
        // `[` `]` `"` `\` or be empty, and inner lists may hold more than one
        // item. `Map` is a value shape too — as a list item it forces the
        // whole list to a block sequence (`render_entry`'s `List` arm), and
        // as an entry value it renders as a block mapping — neither is
        // expressible as a single flow-value string, which is why the
        // entry-level property below reparses through the full document tree
        // instead of splitting the rendered text line by line.
        leaf.prop_recursive(4, 48, 8, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..6).prop_map(FmValue::List),
                prop::collection::vec((key_strategy(), inner), 0..4).prop_map(FmValue::Map),
            ]
        })
        // An empty `Map` has no representable form (block form needs at
        // least one entry, there is no flow-map syntax): the writer emits a
        // valueless `key:` line, which the reader maps back to `Null`. Fold
        // that asymmetry into the generator so the round-trip properties
        // assert what actually round-trips, rather than special-casing every
        // assertion site — pinned separately by
        // `empty_map_renders_and_reparses_as_null`.
        .prop_map(normalize_empty_maps)
    }

    /// Recursively replaces every empty `Map` (at any depth: a value, a list
    /// item, or a nested map's value) with `Null` — see the comment on
    /// [`fm_value_strategy`].
    fn normalize_empty_maps(v: FmValue) -> FmValue {
        match v {
            FmValue::Map(entries) if entries.is_empty() => FmValue::Null,
            FmValue::Map(entries) => FmValue::Map(
                entries
                    .into_iter()
                    .map(|(k, v)| (k, normalize_empty_maps(v)))
                    .collect(),
            ),
            FmValue::List(items) => {
                FmValue::List(items.into_iter().map(normalize_empty_maps).collect())
            }
            other => other,
        }
    }

    /// True if `v` holds a `Map` anywhere in its tree, including nested
    /// inside a `List`. Only [`render_entry`]'s block forms (a `Map` under a
    /// carrying key, or a direct sequence of maps) can render a `Map`: the
    /// flow-sequence form (`render_flow_item`) has no way to embed a
    /// multi-line block mapping inside `[...]`.
    fn value_contains_map(v: &FmValue) -> bool {
        match v {
            FmValue::Map(_) => true,
            FmValue::List(items) => items.iter().any(value_contains_map),
            _ => false,
        }
    }

    /// True when every `Map` in `v`'s tree (at any depth) has distinct keys.
    /// A duplicate key inside a nested map collapses to last-wins on reparse
    /// (`map_entries_from_mapping`, same as the top level) — a different,
    /// already-covered contract, not this property's.
    fn keys_unique(v: &FmValue) -> bool {
        match v {
            FmValue::Map(entries) => {
                let mut seen = std::collections::HashSet::new();
                entries.iter().all(|(k, _)| seen.insert(k.clone()))
                    && entries.iter().all(|(_, v)| keys_unique(v))
            }
            FmValue::List(items) => items.iter().all(keys_unique),
            _ => true,
        }
    }

    /// True when `v` is a shape the current writer+parser combo round-trips.
    /// A `Map` may sit under a key (any depth, via `render_entry`'s
    /// recursion) or as a DIRECT `List` item (a block sequence of maps,
    /// mixed with non-`Map` items freely). What is NOT representable: a
    /// `Map` nested inside a `List` that is itself a `List` item — e.g.
    /// list-of-list-of-map — because the parser has no block form for a
    /// bare `-` opening a nested sequence (see `render_entry`'s comment).
    /// The writer still emits VALID (if lossy) output for that shape — a
    /// quoted one-line `{k: v}` scalar, pinned by
    /// `map_inside_a_flow_sequence_degrades_to_one_line` — it just does not
    /// come back as a `Map`, so the round-trip property excludes it.
    fn representable_value(v: &FmValue) -> bool {
        match v {
            FmValue::Map(entries) => entries.iter().all(|(_, v)| representable_value(v)),
            FmValue::List(items) => items.iter().all(|item| match item {
                FmValue::Map(entries) => entries.iter().all(|(_, v)| representable_value(v)),
                // A non-Map list item renders as a flow value (even under a
                // block-sequence key, per `render_block_sequence_item`'s
                // fallback arm), so it must not carry a `Map` anywhere.
                other => !value_contains_map(other) && representable_value(other),
            }),
            _ => true,
        }
    }

    /// Conformance-sweep pin: a bare value that itself contains a colon (not
    /// followed by whitespace at the position that already matched the
    /// entry's key) is NOT a nested key — the scanner only treats the
    /// FIRST `: `/`:$` on the line as the key/value split, so the rest of
    /// the line is the value's own text.
    #[test]
    fn colon_in_bare_value_is_not_a_second_key() {
        let fm = parse_frontmatter_for_test("---\na: b: c\n---\n");
        assert_eq!(fm.get("a"), Some(&FmValue::Str("b: c".into())));
    }

    /// Conformance-sweep pin: `[a, [b, c]]` nests, matching
    /// `parse_value_follows_yaml_12_core`'s flow-splitting coverage but
    /// exercised through the full document parse (closed-syntax reader),
    /// not just the bare-value splitter.
    #[test]
    fn nested_flow_sequence_parses_through_full_document() {
        let fm = parse_frontmatter_for_test("---\nseq: [a, [b, c]]\n---\n");
        assert_eq!(
            fm.get("seq"),
            Some(&FmValue::List(vec![
                FmValue::Str("a".into()),
                FmValue::List(vec![FmValue::Str("b".into()), FmValue::Str("c".into())]),
            ]))
        );
    }

    /// Fuzz-seed no-panic pin (Task 10, Step 2): the Windows host cannot run
    /// `cargo fuzz`, so this feeds each `fuzz/seeds/parse_write` frontmatter
    /// seed through the full parse, a `write_to_string` round-trip identity
    /// check, and `parse_closed_syntax`, asserting none of them panic.
    #[test]
    fn fuzz_seeds_parse_without_panic() {
        let seeds: &[(&str, &str)] = &[
            (
                "frontmatter-nested.md",
                include_str!("../../../fuzz/seeds/parse_write/frontmatter-nested.md"),
            ),
            (
                "frontmatter-block-scalars.md",
                include_str!("../../../fuzz/seeds/parse_write/frontmatter-block-scalars.md"),
            ),
            (
                "frontmatter-hostile.md",
                include_str!("../../../fuzz/seeds/parse_write/frontmatter-hostile.md"),
            ),
        ];
        for (name, source) in seeds {
            let text = SourceText::from_shared(std::sync::Arc::new((*source).into())).unwrap();
            let snapshot = parse_markdown(
                DocumentRevision::INITIAL,
                text,
                MarkdownDialect::WAML_DEFAULT,
            )
            .unwrap_or_else(|_| panic!("{name}: bounded UTF-8 markdown must parse"));
            assert_eq!(
                snapshot.tree().write_to_string(),
                *source,
                "{name}: round-trip must be lossless"
            );
            for element in snapshot.tree().root().children() {
                if let Some(node) = element.into_node() {
                    let _ = parse_closed_syntax(&node);
                }
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1024))]

        /// THE writer contract for standalone values: parse_value(render_value(v)) == v.
        /// `Map` is excluded here — a nested map only has a representable
        /// rendering under a carrying key (a block form), never as a single
        /// value string; its round-trip is covered by
        /// `rendered_frontmatter_entries_reparse` below instead.
        #[test]
        fn rendered_value_reparses_identically(
            v in fm_value_strategy()
                .prop_filter(
                    "Map (at any depth) is only representable under a key; see rendered_frontmatter_entries_reparse",
                    |v| !value_contains_map(v),
                )
        ) {
            let rendered = render_value(&v);
            prop_assert_eq!(parse_value(&rendered), v.clone(), "rendered as {:?}", rendered);
        }

        /// Entry-level contract: a whole rendered frontmatter reparses, through
        /// the real document tree (block mappings/sequences are multi-line
        /// forms a per-line split cannot recover), back to the same entries.
        #[test]
        fn rendered_frontmatter_entries_reparse(
            entries in prop::collection::vec((key_strategy(), fm_value_strategy()), 0..5)
                // Duplicate top-level keys collapse to last-wins on reparse
                // (by design — see `map_entries_from_mapping`), which is a
                // different, already-covered contract; keep this property to
                // documents with distinct keys.
                .prop_filter("keys must be unique for this property", |entries| {
                    let mut seen = std::collections::HashSet::new();
                    entries.iter().all(|(k, _)| seen.insert(k.clone()))
                })
                .prop_filter(
                    "list-of-list-of-map has no writer block form yet (see representable_value)",
                    |entries| entries.iter().all(|(_, v)| representable_value(v)),
                )
                .prop_filter("nested map keys must be unique too", |entries| {
                    entries.iter().all(|(_, v)| keys_unique(v))
                })
        ) {
            let fm = Frontmatter { entries: entries.clone() };
            let rendered = render_frontmatter(&fm);
            let source = format!("---\n{rendered}\n---\n");
            let parsed = parse_frontmatter_for_test(&source);
            prop_assert_eq!(parsed, fm, "rendered as {:?}", rendered);
        }
    }
}
