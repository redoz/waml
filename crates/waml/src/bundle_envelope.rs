//! Bundle Envelope v1: the authoritative transport codec for concatenated WAML documents.
//!
//! This module owns the complete marker grammar, the path percent codec, and the
//! nonce selection rules. WAML parsing, syntax, analysis, LSP, and editor code must
//! stay unaware of these transport markers.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

use crate::source::BundlePath;

const SENTINEL: &str = "<!-- waml/";
const PART_PREFIX: &str = "<!-- waml/1 part ";
const NONCE_LEN: usize = 32;
const MAX_NONCE_ATTEMPTS: usize = 64;
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A failure of the Bundle Envelope v1 grammar, path codec, or nonce selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleEnvelopeError {
    UnsupportedVersion { version: String, offset: usize },
    MalformedFirstMarker { offset: usize },
    MalformedPartMarker { offset: usize },
    InvalidNonce { offset: usize },
    InvalidPercentEscape { offset: usize },
    InvalidPathUtf8 { offset: usize },
    InvalidPathEncoding { offset: usize },
    InvalidBundlePath { path: String, offset: usize },
    DuplicatePath { path: String, offset: usize },
    EmptyBundle,
    NonceSelectionExhausted { attempts: usize },
}

impl fmt::Display for BundleEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { version, offset } => {
                write!(
                    f,
                    "unsupported WAML bundle envelope version {version} at byte {offset}"
                )
            }
            Self::MalformedFirstMarker { offset } => {
                write!(f, "malformed WAML bundle envelope marker at byte {offset}")
            }
            Self::MalformedPartMarker { offset } => {
                write!(f, "malformed WAML bundle part marker at byte {offset}")
            }
            Self::InvalidNonce { offset } => {
                write!(f, "invalid WAML bundle nonce at byte {offset}")
            }
            Self::InvalidPercentEscape { offset } => {
                write!(f, "invalid bundle path percent escape at byte {offset}")
            }
            Self::InvalidPathUtf8 { offset } => {
                write!(f, "bundle path is not valid UTF-8 at byte {offset}")
            }
            Self::InvalidPathEncoding { offset } => {
                write!(f, "bundle path contains an unescaped byte at byte {offset}")
            }
            Self::InvalidBundlePath { path, offset } => {
                write!(f, "invalid bundle path {path:?} at byte {offset}")
            }
            Self::DuplicatePath { path, offset } => {
                write!(f, "duplicate bundle path {path:?} at byte {offset}")
            }
            Self::EmptyBundle => f.write_str("cannot encode an empty WAML bundle"),
            Self::NonceSelectionExhausted { attempts } => {
                write!(
                    f,
                    "could not select a collision-free WAML bundle nonce after {attempts} attempts"
                )
            }
        }
    }
}

impl std::error::Error for BundleEnvelopeError {}

fn is_literal_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/')
}

fn encode_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(path.len());
    for &byte in path.as_bytes() {
        if is_literal_path_byte(byte) {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[(byte >> 4) as usize]));
            out.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    out
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_path(encoded: &str, absolute_offset: usize) -> Result<String, BundleEnvelopeError> {
    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            byte if is_literal_path_byte(byte) => {
                decoded.push(byte);
                index += 1;
            }
            b'%' => {
                let high = bytes.get(index + 1).and_then(|byte| hex_value(*byte));
                let low = bytes.get(index + 2).and_then(|byte| hex_value(*byte));
                let (Some(high), Some(low)) = (high, low) else {
                    return Err(BundleEnvelopeError::InvalidPercentEscape {
                        offset: absolute_offset + index,
                    });
                };
                decoded.push((high << 4) | low);
                index += 3;
            }
            _ => {
                return Err(BundleEnvelopeError::InvalidPathEncoding {
                    offset: absolute_offset + index,
                });
            }
        }
    }
    String::from_utf8(decoded).map_err(|error| BundleEnvelopeError::InvalidPathUtf8 {
        offset: absolute_offset + error.utf8_error().valid_up_to(),
    })
}

struct Marker {
    nonce: String,
    path: String,
    body_start: usize,
}

fn parse_marker(
    text: &str,
    offset: usize,
    expected_nonce: Option<&str>,
    first: bool,
) -> Result<Marker, BundleEnvelopeError> {
    let malformed = || {
        if first {
            BundleEnvelopeError::MalformedFirstMarker { offset }
        } else {
            BundleEnvelopeError::MalformedPartMarker { offset }
        }
    };

    let rest = &text[offset..];
    let Some(newline) = rest.find('\n') else {
        return Err(malformed());
    };
    let body_start = offset + newline + 1;
    let mut line = &rest[..newline];
    if let Some(stripped) = line.strip_suffix('\r') {
        line = stripped;
    }
    if line.contains('\r') {
        return Err(malformed());
    }

    let Some(after_sentinel) = line.strip_prefix(SENTINEL) else {
        return Err(malformed());
    };
    let Some(version_end) = after_sentinel.find(' ') else {
        return Err(malformed());
    };
    let version = &after_sentinel[..version_end];
    if version != "1" {
        return Err(BundleEnvelopeError::UnsupportedVersion {
            version: version.to_owned(),
            offset,
        });
    }
    let Some(after_part) = after_sentinel[version_end + 1..].strip_prefix("part ") else {
        return Err(malformed());
    };

    let nonce_start = offset + PART_PREFIX.len();
    let bytes = after_part.as_bytes();
    if bytes.len() < NONCE_LEN {
        return Err(BundleEnvelopeError::InvalidNonce {
            offset: nonce_start,
        });
    }
    if let Some(index) = bytes[..NONCE_LEN]
        .iter()
        .position(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(BundleEnvelopeError::InvalidNonce {
            offset: nonce_start + index,
        });
    }
    let nonce = &after_part[..NONCE_LEN];
    if expected_nonce.is_some_and(|expected| expected != nonce) {
        return Err(malformed());
    }

    let Some(after_nonce) = after_part[NONCE_LEN..].strip_prefix(' ') else {
        return Err(malformed());
    };
    let Some(encoded_path) = after_nonce.strip_suffix(" -->") else {
        return Err(malformed());
    };
    if encoded_path.is_empty() {
        return Err(malformed());
    }

    let path_offset = nonce_start + NONCE_LEN + 1;
    let decoded = decode_path(encoded_path, path_offset)?;
    let path =
        BundlePath::parse(decoded.clone()).map_err(|_| BundleEnvelopeError::InvalidBundlePath {
            path: decoded,
            offset: path_offset,
        })?;

    Ok(Marker {
        nonce: nonce.to_owned(),
        path: path.as_str().to_owned(),
        body_start,
    })
}

/// Decode a Bundle Envelope v1 document, or report that the input is plain Markdown.
///
/// `Ok(None)` means the input does not start with the WAML sentinel and must stay
/// one unmodified document.
pub fn split_bundle(text: &str) -> Result<Option<Vec<(String, String)>>, BundleEnvelopeError> {
    if !text.starts_with(SENTINEL) {
        return Ok(None);
    }

    let first = parse_marker(text, 0, None, true)?;
    let active_prefix = format!("{PART_PREFIX}{}", first.nonce);
    let mut parts = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current_path = first.path;
    let mut body_start = first.body_start;

    loop {
        let Some(relative) = text[body_start..].find(&active_prefix) else {
            if !seen.insert(current_path.clone()) {
                return Err(BundleEnvelopeError::DuplicatePath {
                    path: current_path,
                    offset: body_start,
                });
            }
            parts.push((current_path, text[body_start..].to_owned()));
            break;
        };
        let marker_start = body_start + relative;
        let next = parse_marker(text, marker_start, Some(&first.nonce), false)?;
        if !seen.insert(current_path.clone()) {
            return Err(BundleEnvelopeError::DuplicatePath {
                path: current_path,
                offset: marker_start,
            });
        }
        parts.push((current_path, text[body_start..marker_start].to_owned()));
        current_path = next.path;
        body_start = next.body_start;
    }

    Ok(Some(parts))
}

/// Serialize ordered documents as one Bundle Envelope v1 payload.
pub fn encode_bundle_envelope(parts: &[(String, String)]) -> Result<String, BundleEnvelopeError> {
    encode_bundle_envelope_with(parts, production_nonce)
}

/// Serialize ordered documents with an explicit nonce source, for deterministic tests.
pub fn encode_bundle_envelope_with(
    parts: &[(String, String)],
    mut next_nonce: impl FnMut() -> Option<u128>,
) -> Result<String, BundleEnvelopeError> {
    if parts.is_empty() {
        return Err(BundleEnvelopeError::EmptyBundle);
    }

    let mut seen = BTreeSet::new();
    let mut validated = Vec::with_capacity(parts.len());
    for (path, body) in parts {
        let path = BundlePath::parse(path.clone()).map_err(|_| {
            BundleEnvelopeError::InvalidBundlePath {
                path: path.clone(),
                offset: 0,
            }
        })?;
        let normalized = path.as_str().to_owned();
        if !seen.insert(normalized.clone()) {
            return Err(BundleEnvelopeError::DuplicatePath {
                path: normalized,
                offset: 0,
            });
        }
        validated.push((normalized, body.as_str()));
    }

    for attempt in 0..MAX_NONCE_ATTEMPTS {
        let Some(value) = next_nonce() else {
            return Err(BundleEnvelopeError::NonceSelectionExhausted { attempts: attempt });
        };
        let nonce = format!("{value:032x}");
        let prefix = format!("{PART_PREFIX}{nonce}");
        let collides = validated.iter().any(|(path, body)| {
            path.contains(&prefix) || encode_path(path).contains(&prefix) || body.contains(&prefix)
        });
        if collides {
            continue;
        }

        let capacity = validated
            .iter()
            .map(|(path, body)| path.len() + body.len() + 80)
            .sum();
        let mut output = String::with_capacity(capacity);
        for (path, body) in &validated {
            output.push_str(PART_PREFIX);
            output.push_str(&nonce);
            output.push(' ');
            output.push_str(&encode_path(path));
            output.push_str(" -->\n");
            output.push_str(body);
        }
        return Ok(output);
    }

    Err(BundleEnvelopeError::NonceSelectionExhausted {
        attempts: MAX_NONCE_ATTEMPTS,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn production_nonce() -> Option<u128> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
    let process = u128::from(std::process::id());
    Some(time ^ process.rotate_left(47) ^ counter.rotate_left(89))
}

/// Nonces for `wasm32-unknown-unknown`, where neither a clock nor a process id
/// exists: `SystemTime::now()` PANICS there rather than returning an error, and
/// `std::process::id()` is equally unsupported. The browser editor encodes a
/// bundle on every **Export WAML bundle...**, so the shared version above would
/// take the whole app down.
///
/// The counter alone is sufficient. A nonce is not a secret and carries no
/// entropy requirement: it only has to be a string that appears nowhere in the
/// paths or bodies being wrapped, and `encode_bundle_envelope_with` already
/// retries with the next one when it collides.
#[cfg(target_arch = "wasm32")]
fn production_nonce() -> Option<u128> {
    let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
    Some(counter.rotate_left(89) ^ 0x7761_6d6c_5f77_6173_6d5f_6e6f_6e63_6531)
}

/// Expand a valid Bundle Envelope v1 or retain the input as one plain document.
pub fn expand_text(display_path: &str, text: &str) -> std::io::Result<Vec<(String, String)>> {
    match split_bundle(text).map_err(|source| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{display_path}: {source}"),
        )
    })? {
        Some(parts) => Ok(parts),
        None => Ok(vec![(display_path.to_owned(), text.to_owned())]),
    }
}

/// Serialize a bundle as pretty-printed JSON: `[[path, content], ...]`.
#[cfg(feature = "serde")]
pub fn render_bundle_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string_pretty(bundle).unwrap_or_else(|_| "[]".to_string())
}

/// True iff `name` can sit verbatim in `export const {name}` — an ASCII
/// identifier. Anything else would splice arbitrary text into the generated
/// TypeScript, so [`render_bundle_ts`] callers must reject it first.
pub fn is_ts_export_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Emit a checked-in TS module: `serde_json::to_string` per field reuses JSON string
/// escaping, which is valid JS/TS string literal syntax.
#[cfg(feature = "serde")]
pub fn render_bundle_ts(bundle: &[(String, String)], export_name: &str) -> String {
    let mut out = String::from("// AUTO-GENERATED by `waml bundle` — do not edit by hand.\n");
    out.push_str(&format!(
        "export const {export_name}: [string, string][] = [\n"
    ));
    for (path, content) in bundle {
        let path_json = serde_json::to_string(path).unwrap_or_else(|_| "\"\"".to_string());
        let content_json = serde_json::to_string(content).unwrap_or_else(|_| "\"\"".to_string());
        out.push_str(&format!("  [{path_json}, {content_json}],\n"));
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_v1_envelope_into_docs() {
        let nonce = "0000000000000000000000000000000a";
        let blob = format!(
            "<!-- waml/1 part {nonce} a/one.md -->\n# One\n<!-- waml/1 part {nonce} a/two.md -->\n# Two\n"
        );
        let docs = expand_text("stdin", &blob).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0], ("a/one.md".into(), "# One\n".into()));
        assert_eq!(docs[1], ("a/two.md".into(), "# Two\n".into()));
    }

    #[test]
    fn plain_and_legacy_text_use_the_physical_display_path() {
        for text in ["# Order\n", "before\n<!-- a/one.md -->\nafter\n"] {
            assert_eq!(
                expand_text("shop/order.md", text).unwrap(),
                vec![("shop/order.md".into(), text.into())]
            );
        }
    }

    #[test]
    fn malformed_envelope_includes_the_physical_input_name() {
        let error = expand_text(
            "imports/orders.bundle.md",
            "<!-- waml/2 part 0000000000000000000000000000000a x.md -->\n",
        )
        .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        let message = error.to_string();
        assert!(message.contains("imports/orders.bundle.md"), "{message}");
        assert!(
            message.contains("unsupported WAML bundle envelope version 2"),
            "{message}"
        );
    }

    #[test]
    fn stray_comment_doc_is_one_doc() {
        // A single .md doc that happens to contain a lone, non-marker HTML
        // comment (e.g. a review note) must not be split apart: it must
        // come back as exactly one doc, keyed by its real display path,
        // with the full content — including the unresolved relationship
        // section that follows the stray comment — intact.
        let text = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n<!-- reviewed: needs follow-up -->\n\n## Relationships\n- depends [Ghost](./ghost.md)\n";
        let docs = expand_text("shop/order.md", text).unwrap();
        assert_eq!(
            docs.len(),
            1,
            "a stray non-.md comment must not split the document"
        );
        assert_eq!(docs[0].0, "shop/order.md");
        assert_eq!(
            docs[0].1, text,
            "content must be kept intact, nothing discarded"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn render_bundle_json_round_trips() {
        let bundle = vec![("a/one.md".to_string(), "# One\n".to_string())];
        let out = render_bundle_json(&bundle);
        let v: Vec<(String, String)> = serde_json::from_str(&out).unwrap();
        assert_eq!(v, bundle);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn render_bundle_ts_emits_valid_export_with_escaped_content() {
        let bundle = vec![("a/one.md".to_string(), "line1\n\"quoted\"\n".to_string())];
        let out = render_bundle_ts(&bundle, "myBundle");
        assert!(out.starts_with("// AUTO-GENERATED"));
        assert!(out.contains("export const myBundle: [string, string][] = ["));
        assert!(out.contains("\"a/one.md\""));
        assert!(out.contains("\\n\\\"quoted\\\"\\n"));
    }

    #[test]
    fn ts_export_names_admit_identifiers_and_nothing_else() {
        for valid in ["myBundle", "_private", "$x", "b2", "SNAKE_CASE"] {
            assert!(is_ts_export_name(valid), "{valid}");
        }
        for invalid in [
            "",
            "2start",
            "has space",
            "a-b",
            "x;alert(1)//",
            "a\"] = evil; //",
            "café",
        ] {
            assert!(!is_ts_export_name(invalid), "{invalid}");
        }
    }
}
