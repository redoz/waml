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

#[cfg(test)]
mod tests {
    use super::*;
    use FrontmatterScalarKind::*;

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
