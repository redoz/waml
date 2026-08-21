//! Shared YAML 1.2 core scalar grammar — the one place the frontmatter's
//! scalar rules are written down.
//!
//! Three things live here, and each is a single source of truth on purpose:
//!
//! * [`classify_bare_scalar`] decides what a bare (unquoted, trimmed) scalar
//!   IS. Both the model (`FmValue` parsing) and the editor's syntax coloring
//!   call it, so a value painted as a `Number` can never be read back as a
//!   `Str`.
//! * [`scan_quoted_scalar`] decides where a quoted scalar ENDS — which
//!   delimiters open one, what closes it, and how escapes work.
//! * [`decode_quoted_scalar`] and [`decode_double_quoted_body`] decide what a
//!   quoted scalar MEANS once its extent is known.
//!
//! Keep them together. The scan and the decode have to define the same escape
//! set, and when they lived apart — the scan in two crates' worth of
//! hand-rolled loops — they drifted.

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

/// The extent of one quoted frontmatter scalar, as found in the source.
///
/// Every offset in here is on a UTF-8 character boundary of the `source` the
/// scan was handed, whatever bytes the scalar contains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QuotedScalarScan {
    /// Offset just past the closing quote. When the scalar is unterminated
    /// this is the scan limit — the scalar owns everything that was left.
    pub end: usize,
    /// True when no closing delimiter was found before the limit.
    pub unterminated: bool,
    /// Byte span of the FIRST escape a double-quoted scalar does not define,
    /// if any. Single-quoted scalars have no backslash escapes, so this is
    /// always `None` for them. Callers that only need the extent ignore it.
    pub invalid_escape: Option<(usize, usize)>,
}

/// Scans the quoted frontmatter scalar that opens at `start`, stopping at
/// `limit`. `None` when `start` does not sit on a `'` or `"`.
///
/// This is the single source of truth for the waml frontmatter's YAML-subset
/// quoting grammar. It was three near-copies across two crates — an
/// escape-classifying one in the parser, a blind two-byte-skip one in the
/// flow-collection splitters, and a validity state machine in the model — and
/// they only agreed on where a scalar ends by luck: every place their escape
/// handling differed happened to land on bytes that cannot close a quote. The
/// safety property was never shared, and the one copy that published an
/// intermediate offset shipped a UTF-8 bug with it.
///
/// The grammar, in full:
///
/// * `'...'` and `"..."` both open a scalar. The opening delimiter is the only
///   thing that closes it, so a `"` inside `'...'` is content.
/// * Inside `'...'`, `''` is a literal `'` — YAML's only single-quote escape.
///   There are no backslash escapes; a `\` is an ordinary character.
/// * Inside `"..."`, `\` escapes the next character. `\\ \" \n \r \t \0` and
///   `\uXXXX` (exactly four hex digits, all within the limit) are defined;
///   every other `\x` is reported through [`QuotedScalarScan::invalid_escape`]
///   and the scan steps over the WHOLE escaped character — which need not be
///   ASCII. A trailing `\` with nothing after it before the limit is an
///   ordinary character, not an escape.
/// * Reaching the limit without a closing delimiter is an unterminated
///   scalar. That is not an error here: the scalar extends to the limit and
///   the caller decides what it means. End-of-line and end-of-input give the
///   same answer — they differ only in the `limit` the caller passed.
/// * `''` and `""` are empty scalars, closed and well formed.
///
/// A `limit` past the end of `source`, or one that cuts a character in half,
/// is pulled back to the nearest character boundary at or below it, so the
/// boundary guarantee holds whatever the caller passed.
pub fn scan_quoted_scalar(source: &str, start: usize, limit: usize) -> Option<QuotedScalarScan> {
    let mut limit = limit.min(source.len());
    while limit > 0 && !source.is_char_boundary(limit) {
        limit -= 1;
    }
    if start >= limit {
        return None;
    }
    let bytes = source.as_bytes();
    // A `start` inside a multi-byte character points at a continuation byte,
    // which is never ASCII, so this rejects it without a boundary check.
    let quote = match bytes[start] {
        quote @ (b'\'' | b'"') => quote,
        _ => return None,
    };
    let mut at = start + 1;
    let mut invalid_escape = None;
    while at < limit {
        let byte = bytes[at];
        if byte == quote {
            if quote == b'\'' && at + 1 < limit && bytes[at + 1] == b'\'' {
                at += 2;
                continue;
            }
            return Some(QuotedScalarScan {
                end: at + 1,
                unterminated: false,
                invalid_escape,
            });
        }
        if quote == b'"' && byte == b'\\' && at + 1 < limit {
            let escape_start = at;
            match known_double_quote_escape(bytes, at + 1, limit) {
                Some(next) => at = next,
                None => {
                    // The escaped character need not be ASCII, so step over
                    // all of its bytes. A fixed two-byte step lands inside a
                    // UTF-8 sequence, and any offset built from it names a
                    // range the caller cannot slice with.
                    let escape_end = char_end(source, at + 1, limit);
                    if invalid_escape.is_none() {
                        invalid_escape = Some((escape_start, escape_end));
                    }
                    at = escape_end;
                }
            }
            continue;
        }
        at = char_end(source, at, limit);
    }
    Some(QuotedScalarScan {
        end: limit,
        unterminated: true,
        invalid_escape,
    })
}

/// The offset just past the character that starts at `at`, never past `limit`.
fn char_end(source: &str, at: usize, limit: usize) -> usize {
    let mut end = at + 1;
    while end < limit && !source.is_char_boundary(end) {
        end += 1;
    }
    end.min(limit)
}

/// The offset just past a double-quote escape body starting at `at` (the
/// character AFTER the backslash), or `None` when this decoder does not define
/// it. Every offset returned is ASCII, hence on a character boundary.
fn known_double_quote_escape(bytes: &[u8], at: usize, limit: usize) -> Option<usize> {
    match bytes.get(at) {
        Some(b'\\' | b'"' | b'n' | b'r' | b't' | b'0') => Some(at + 1),
        Some(b'u') => {
            let hex_start = at + 1;
            let hex_end = (hex_start + 4).min(limit);
            (hex_end - hex_start == 4
                && bytes[hex_start..hex_end].iter().all(u8::is_ascii_hexdigit))
            .then_some(hex_end)
        }
        _ => None,
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
    use proptest::prelude::*;
    use FrontmatterScalarKind::*;

    /// What one row of the grammar table expects: the source text the scan
    /// covered, whether it ran off the end, and the text of the first escape
    /// the grammar does not define. `None` for the row means "not a quoted
    /// scalar here".
    type Expectation<'a> = Option<(&'a str, bool, Option<&'a str>)>;

    #[track_caller]
    fn check(source: &str, start: usize, limit: usize, expected: Expectation<'_>) {
        let scan = scan_quoted_scalar(source, start, limit);
        let Some(scan) = scan else {
            assert_eq!(
                expected, None,
                "{source:?} [{start}..{limit}] is not quoted"
            );
            return;
        };
        // The boundary guarantee, asserted on every offset of every row --
        // this is what a fixed-width step over an escape used to break.
        assert!(
            source.is_char_boundary(scan.end),
            "{source:?} [{start}..{limit}] end {} is on a character boundary",
            scan.end
        );
        if let Some((escape_start, escape_end)) = scan.invalid_escape {
            assert!(
                source.is_char_boundary(escape_start) && source.is_char_boundary(escape_end),
                "{source:?} [{start}..{limit}] escape {escape_start}..{escape_end} is on \
                 character boundaries"
            );
        }
        let actual = (
            &source[start..scan.end],
            scan.unterminated,
            scan.invalid_escape
                .map(|(escape_start, escape_end)| &source[escape_start..escape_end]),
        );
        assert_eq!(Some(actual), expected, "{source:?} [{start}..{limit}]");
    }

    /// The frontmatter quoting grammar, one row per rule, scanned to
    /// end-of-input. Three near-copies of this scanner used to live across
    /// `waml` and `waml-syntax`; this table is what a fourth would have to
    /// agree with, and what the one that remains is pinned to.
    #[test]
    fn quoted_scalar_grammar_to_end_of_input() {
        // (source, expected)
        let table: &[(&str, Expectation<'_>)] = &[
            // --- double quotes: the basic shapes ---
            (r#""""#, Some((r#""""#, false, None))),
            (r#""a""#, Some((r#""a""#, false, None))),
            (r#""é""#, Some((r#""é""#, false, None))),
            // A single quote inside a double-quoted scalar is content: only
            // the OPENING delimiter can close.
            (r#""'""#, Some((r#""'""#, false, None))),
            // ...and `''` doubling belongs to single quotes only.
            (r#""a''b""#, Some((r#""a''b""#, false, None))),
            // --- double quotes: unterminated at end of input ---
            (r#""a"#, Some((r#""a"#, true, None))),
            (r#"""#, Some((r#"""#, true, None))),
            (r#""é"#, Some((r#""é"#, true, None))),
            // --- double quotes: the defined escapes ---
            (r#""\\""#, Some((r#""\\""#, false, None))),
            // The delimiter escaped by a backslash does NOT close.
            (r#""\"""#, Some((r#""\"""#, false, None))),
            (r#""\n""#, Some((r#""\n""#, false, None))),
            (r#""\r""#, Some((r#""\r""#, false, None))),
            (r#""\t""#, Some((r#""\t""#, false, None))),
            (r#""\0""#, Some((r#""\0""#, false, None))),
            (r#""\u00e9""#, Some((r#""\u00e9""#, false, None))),
            (r#""\uFFFF""#, Some((r#""\uFFFF""#, false, None))),
            // --- double quotes: the escapes it does NOT define ---
            (r#""\q""#, Some((r#""\q""#, false, Some(r"\q")))),
            (r#""\'""#, Some((r#""\'""#, false, Some(r"\'")))),
            // Fewer than four hex digits, or a non-hex digit among them, is
            // not a `\u` escape -- the `u` is an unknown escape and the rest
            // is ordinary content.
            (r#""\u00""#, Some((r#""\u00""#, false, Some(r"\u")))),
            (r#""\uzzzz""#, Some((r#""\uzzzz""#, false, Some(r"\u")))),
            (r#""\u""#, Some((r#""\u""#, false, Some(r"\u")))),
            // THE bug this scanner exists to kill: the escaped character need
            // not be ASCII, and both the step and the reported span must clear
            // all of its bytes.
            (r#""\é""#, Some((r#""\é""#, false, Some("\\é")))),
            (r#""\🕴""#, Some((r#""\🕴""#, false, Some("\\🕴")))),
            (r#""\𐀶""#, Some((r#""\𐀶""#, false, Some("\\𐀶")))),
            // Only the FIRST undefined escape is reported.
            (r#""\q\z""#, Some((r#""\q\z""#, false, Some(r"\q")))),
            // A backslash with nothing after it before the limit is ordinary
            // content, not an escape.
            (r#""a\"#, Some((r#""a\"#, true, None))),
            (r#""\"#, Some((r#""\"#, true, None))),
            // An escaped delimiter at the end leaves the scalar unterminated.
            (r#""a\""#, Some((r#""a\""#, true, None))),
            // --- double quotes: nothing past the close is consumed ---
            (r#""a" b"#, Some((r#""a""#, false, None))),
            (r#""a""b""#, Some((r#""a""#, false, None))),
            // --- single quotes ---
            ("''", Some(("''", false, None))),
            ("'a'", Some(("'a'", false, None))),
            ("'é'", Some(("'é'", false, None))),
            (r#"'"'"#, Some((r#"'"'"#, false, None))),
            // `''` is the ONE single-quote escape.
            ("''''", Some(("''''", false, None))),
            ("'a''b'", Some(("'a''b'", false, None))),
            // An odd run leaves the scalar open.
            ("'''", Some(("'''", true, None))),
            ("'a", Some(("'a", true, None))),
            ("'", Some(("'", true, None))),
            // A backslash is INERT in a single-quoted scalar, so the quote
            // after it still closes -- the sharpest contrast with `"..."`,
            // and the one a shared escape step would quietly erase.
            (r"'a\'", Some((r"'a\'", false, None))),
            (r"'\'", Some((r"'\'", false, None))),
            (r"'\é'", Some((r"'\é'", false, None))),
            (r"'\u00e9'", Some((r"'\u00e9'", false, None))),
            (r"'\n'", Some((r"'\n'", false, None))),
            // --- not a quoted scalar at all ---
            ("", None),
            ("a", None),
            (r#"a"b""#, None),
            ("[\"a\"]", None),
            (" 'a'", None),
        ];
        for (source, expected) in table {
            check(source, 0, source.len(), *expected);
        }
    }

    /// The same grammar stopped at end-of-LINE rather than end-of-input. The
    /// answers must be identical -- the limit is the only difference, and an
    /// escape must never reach across it.
    #[test]
    fn quoted_scalar_grammar_to_end_of_line() {
        let table: &[(&str, Expectation<'_>)] = &[
            ("\"a\"\nb\"", Some((r#""a""#, false, None))),
            ("\"a\nb\"", Some((r#""a"#, true, None))),
            ("'a\nb'", Some(("'a", true, None))),
            ("\"\nb\"", Some((r#"""#, true, None))),
            // The backslash sits at the limit, so it escapes nothing -- the
            // newline on the far side is not part of this scalar.
            ("\"\\\nx\"", Some((r#""\"#, true, None))),
            // A `\u` whose hex digits are cut off by the limit is an unknown
            // escape, exactly as it is at end-of-input.
            ("\"\\u00\ne9\"", Some((r#""\u00"#, true, Some(r"\u")))),
            ("\"\\u00e9\nx\"", Some((r#""\u00e9"#, true, None))),
        ];
        for (source, expected) in table {
            let limit = source.find('\n').expect("row carries a newline");
            check(source, 0, limit, *expected);
        }
    }

    /// Offsets the caller can hand in that are not scalar starts at all, and
    /// limits that do not fall where the caller thinks they do.
    #[test]
    fn quoted_scalar_scan_survives_hostile_offsets() {
        // Past the limit, and at it.
        assert_eq!(scan_quoted_scalar(r#""a""#, 3, 3), None);
        assert_eq!(scan_quoted_scalar(r#""a""#, 9, 3), None);
        // A start inside a multi-byte character points at a continuation
        // byte, which is never a delimiter.
        assert_eq!(scan_quoted_scalar("é\"a\"", 1, 5), None);
        // A limit past the end of the source is clamped...
        check(r#""a""#, 0, 999, Some((r#""a""#, false, None)));
        // ...and one that cuts a character in half is pulled back to the
        // boundary below it, so `é` is never half-scanned.
        check("\"é\"", 0, 2, Some((r#"""#, true, None)));
        check("\"é\"", 0, 3, Some(("\"é", true, None)));
        // A start that is not the beginning of the scalar still scans, so
        // callers may resume mid-string.
        check(r#"[ "a" ]"#, 2, 7, Some((r#""a""#, false, None)));
    }

    proptest! {
        /// The scanner never panics and never hands back an offset a caller
        /// cannot slice with -- the two properties every retired copy of this
        /// code was assumed to have and none of them stated.
        #[test]
        fn scan_offsets_stay_on_character_boundaries(
            source in any::<String>(),
            start in any::<usize>(),
            limit in any::<usize>(),
        ) {
            let start = if source.is_empty() { 0 } else { start % (source.len() + 1) };
            let limit = if source.is_empty() { 0 } else { limit % (source.len() + 1) };
            let Some(scan) = scan_quoted_scalar(&source, start, limit) else {
                return Ok(());
            };
            prop_assert!(source.is_char_boundary(scan.end));
            prop_assert!(scan.end > start);
            prop_assert!(scan.end <= limit);
            // Slicing is the whole point of returning offsets.
            prop_assert!(source.get(start..scan.end).is_some());
            if let Some((escape_start, escape_end)) = scan.invalid_escape {
                prop_assert!(source.get(escape_start..escape_end).is_some());
                prop_assert!(escape_start >= start && escape_end <= scan.end);
            }
        }

        /// A closed scalar always ends on its own delimiter, and an
        /// unterminated one always ends at the limit. Nothing else is a legal
        /// answer, so no caller has to guess which it got.
        #[test]
        fn scan_end_is_either_the_delimiter_or_the_limit(
            source in any::<String>(),
            limit in any::<usize>(),
        ) {
            let limit = if source.is_empty() { 0 } else { limit % (source.len() + 1) };
            let Some(scan) = scan_quoted_scalar(&source, 0, limit) else {
                return Ok(());
            };
            let quote = source.as_bytes()[0];
            if scan.unterminated {
                prop_assert!(scan.end <= limit);
            } else {
                prop_assert_eq!(source.as_bytes()[scan.end - 1], quote);
            }
        }
    }

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
