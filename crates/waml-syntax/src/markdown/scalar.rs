//! Shared YAML 1.2 core scalar classifier.
//!
//! This is the single source of truth for classifying a bare (unquoted,
//! trimmed) frontmatter scalar. Both the model (`FmValue` parsing) and the
//! editor's syntax coloring call `classify_bare_scalar`, so a value painted
//! as a `Number` can never be read back as a `Str`.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontmatterScalarKind {
    Null,
    Bool,
    Number,
    Str,
}

/// Classify a BARE (unquoted, trimmed) scalar under the YAML 1.2 core schema,
/// minus the legacy misfeatures waml rejects (no 1.1 bool words, no dates).
pub fn classify_bare_scalar(s: &str) -> FrontmatterScalarKind {
    use FrontmatterScalarKind::*;
    match s {
        "" | "null" | "~" => return Null,
        "true" | "false" => return Bool,
        ".inf" | "-.inf" | "+.inf" | ".nan" => return Number,
        _ => {}
    }
    if is_int(s) || is_hex(s) || is_oct(s) || is_float(s) {
        return Number;
    }
    Str
}

/// `[-+]?[0-9]+`
fn is_int(s: &str) -> bool {
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// `0x[0-9a-fA-F]+` — no sign, per the 1.2 core schema.
fn is_hex(s: &str) -> bool {
    s.strip_prefix("0x")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// `0o[0-7]+`
fn is_oct(s: &str) -> bool {
    s.strip_prefix("0o")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| (b'0'..=b'7').contains(&b)))
}

/// `[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)([eE][-+]?[0-9]+)?`
///
/// Rejects the degenerate forms a careless scan admits: bare `.`, bare `e`,
/// `1e` with no exponent digits, and an empty mantissa.
fn is_float(s: &str) -> bool {
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    let (mantissa, exponent) = match body.split_once(['e', 'E']) {
        Some((m, e)) => {
            let digits = e.strip_prefix(['-', '+']).unwrap_or(e);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            (m, true)
        }
        None => (body, false),
    };
    let _ = exponent;
    match mantissa.split_once('.') {
        // ".5" — fraction required
        Some(("", frac)) => !frac.is_empty() && frac.bytes().all(|b| b.is_ascii_digit()),
        // "1." / "1.5" — fraction optional
        Some((int, frac)) => {
            !int.is_empty()
                && int.bytes().all(|b| b.is_ascii_digit())
                && frac.bytes().all(|b| b.is_ascii_digit())
        }
        // "1e3" — only valid without a dot when an exponent was present
        None => !mantissa.is_empty() && mantissa.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// Normalizes `\r\n` and lone `\r` to `\n`, so a decoded scalar has one
/// line-ending shape regardless of how the source was saved.
pub fn normalize_line_endings(value: &str) -> String {
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

/// Decodes a double-quoted scalar's INNER text (no surrounding quotes).
/// Understands `\\ \" \n \r \t \0` and `\uXXXX`; an unrecognized escape is
/// kept verbatim (backslash and the following character/digits preserved) —
/// the parser side flags this with `InvalidEscapeSequence` but no reader
/// ever panics on it.
pub fn decode_double_quoted_body(value: &str) -> String {
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

/// Decodes a quoted scalar token straight off the tree — `raw` still carries
/// its surrounding quotes. A token with no opening quote is returned as-is,
/// and an unterminated one (already flagged `UnterminatedQuotedScalar`) only
/// has its opening quote peeled. Shared so the parser's duplicate-key check
/// and the model's key/value reader agree on what a key IS.
pub fn decode_quoted_scalar(raw: &str) -> String {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 {
        return raw.to_string();
    }
    if !matches!(bytes[0], b'\'' | b'"') {
        return raw.to_string();
    }
    let closed = bytes[bytes.len() - 1] == bytes[0];
    let inner = if closed {
        &raw[1..raw.len() - 1]
    } else {
        &raw[1..]
    };
    match bytes[0] {
        b'\'' => inner.replace("''", "'"),
        _ => decode_double_quoted_body(inner),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use FrontmatterScalarKind::*;

    #[test]
    fn quoted_scalar_decoding_round_trips_key_shapes() {
        assert_eq!(decode_quoted_scalar("'a'"), "a");
        assert_eq!(decode_quoted_scalar("\"a\\nb\""), "a\nb");
        assert_eq!(decode_quoted_scalar("''''"), "'");
        assert_eq!(decode_quoted_scalar("bare"), "bare");
        // Unterminated: only the opening quote is peeled.
        assert_eq!(decode_quoted_scalar("\"ab"), "ab");
    }

    #[test]
    fn yaml_12_core_scalar_table() {
        // The spec's table, one row per rule. The Norway case is pinned by name.
        let table: &[(&str, FrontmatterScalarKind)] = &[
            ("true", Bool),
            ("false", Bool),
            // NOT bools — YAML 1.2 core dropped the 1.1 words (Norway problem):
            ("NO", Str),
            ("no", Str),
            ("yes", Str),
            ("on", Str),
            ("off", Str),
            ("y", Str),
            ("n", Str),
            ("True", Str),
            ("FALSE", Str),
            ("null", Null),
            ("~", Null),
            ("", Null),
            ("0", Number),
            ("42", Number),
            ("-7", Number),
            ("+7", Number),
            ("0x1A", Number),
            ("0o17", Number),
            ("3.5", Number),
            ("-3.5", Number),
            ("6.02e23", Number),
            (".5", Number),
            ("10.", Number),
            ("1e3", Number),
            (".inf", Number),
            ("-.inf", Number),
            ("+.inf", Number),
            (".nan", Number),
            // Date-shaped stays a string (no date type):
            ("2026-08-04", Str),
            ("uml.Class", Str),
            ("a b", Str),
            ("Null", Str),
            ("nan", Str),
            ("0x", Str),
            ("0o8", Str),
            ("1e", Str),
            ("-", Str),
            (".", Str),
        ];
        for (input, expected) in table {
            assert_eq!(classify_bare_scalar(input), *expected, "input {input:?}");
        }
    }
}
