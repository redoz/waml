# WAML Bundle Envelope v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace ambiguous headerless Markdown bundle splitting with one authoritative, versioned, nonce-delimited Bundle Envelope v1 codec and migrate every live CLI path and fixture to it.

**Architecture:** A new `waml::bundle_envelope` module owns the complete transport grammar, path codec, structured errors, decoder, encoder, and nonce selection. CLI ingress distinguishes plain Markdown from a valid or malformed envelope through the codec result, while CLI mutation output calls the same encoder; parsers, syntax trees, LSP, editor, and semantic models stay unaware of transport markers.

**Tech Stack:** Rust 2021 (MSRV 1.80), standard library only for the codec and nonce generation, existing `proptest = 1.8.0` dev dependency, Cargo workspace tests, Clippy, and rustfmt.

## Global Constraints

- Use the isolated worktree `C:\dev\waml\.worktrees\bundle-envelope-v1` on branch `codex/bundle-envelope-v1`; preserve the dirty primary checkout at `C:\dev\waml`.
- Read `AGENTS.md` and `RTK.md`; prefix every shell command with `rtk` and use ASD-STE100 Simplified Technical English in documentation and user-facing errors.
- Use `apply_patch` for edits. Do not edit, stage, discard, move, or overwrite unrelated or user-owned changes.
- `NONCE` is exactly 32 lowercase hexadecimal characters and represents one 128-bit value.
- The exact marker grammar is `"<!-- waml/1 part " NONCE " " ENCODED_PATH " -->" LINE_ENDING`; spaces are exact, there are no trailing spaces, and `LINE_ENDING` is LF or CRLF.
- Only a valid first marker at byte zero activates bundle mode. Input that does not start with `<!-- waml/` is plain Markdown. A malformed or unsupported byte-zero WAML sentinel is an error and must not downgrade to plain Markdown.
- Later boundaries split only when they use the exact active nonce. A same-nonce marker prefix with malformed syntax is an error. A different-nonce marker remains authored content.
- Preserve document bodies byte for byte. Do not insert separator padding between a body and the next marker.
- RFC 3986 unreserved bytes and `/` remain literal in paths. Encode every other UTF-8 byte as uppercase `%HH`.
- Reject malformed percent escapes, invalid UTF-8, invalid `BundlePath` values, duplicate paths, empty bundles, and exhausted or repeatedly colliding nonce sources.
- The nonce is a collision-resistant delimiter aid, not a security token. Add no new runtime dependency unless implementation evidence proves the standard library is insufficient.
- The old `<!-- path.md -->` form becomes ordinary Markdown. Do not retain a compatibility decoder or the magic `pasted/doc.md` fallback.
- Keep the marker grammar out of WAML parsing, syntax, incremental analysis, LSP, and editor modules.
- Run each RED test before implementation, record the expected failure, make the minimum change, then run the focused GREEN command before the task commit.

---

## File Structure

- Create `crates/waml/src/bundle_envelope.rs`: sole owner of constants, marker parsing, path percent encoding/decoding, `BundleEnvelopeError`, byte-zero detection, splitting, serialization, nonce injection, and production nonce generation.
- Modify `crates/waml/src/lib.rs:3-21`: publish `bundle_envelope` as a peer of `source`.
- Modify `crates/waml/src/source.rs:1-35`: remove `BUNDLE_MARKER_RE` and the legacy `split_bundle`; retain only source-bundle types and behavior.
- Create `crates/waml/tests/bundle_envelope.rs`: focused public codec examples and error-contract tests.
- Create `crates/waml/tests/bundle_envelope_properties.rs`: bounded property tests for round-trip and no-false-split guarantees.
- Modify `crates/waml-cli/src/io.rs:1-26,71-109,782-816`: migrate input expansion to the three-outcome decoder and propagate contextual `InvalidData` errors.
- Modify `crates/waml-cli/src/main.rs:352-414,585-591,625-700`: encode multi-document `fmt --stdout`, remove the local old-marker producer, and call the authoritative encoder for mutation `--stdout`.
- Modify `crates/waml-cli/tests/cli_e2e.rs:1-445`: verify v1 output and non-zero malformed-envelope handling through the binary.
- Modify `crates/waml/tests/fixtures/orders-domain.md:1-68`: replace all six live legacy boundaries with one fixed v1 nonce.
- Modify `crates/waml/tests/golden.rs:1,730-870` and `crates/waml/tests/ops_golden.rs:1-16`: unwrap the explicit valid-envelope outcome.
- Reconcile `C:\dev\waml\issues.md:98-119` only after integration: remove the now-resolved P1 block from the dirty primary file while preserving every other user change. The section is not present in the clean feature worktree, so it must not be fabricated there.

---

### Task 1: Authoritative Bundle Envelope Codec

**Files:**
- Create: `crates/waml/src/bundle_envelope.rs`
- Create: `crates/waml/tests/bundle_envelope.rs`
- Modify: `crates/waml/src/lib.rs:3-21`
- Modify: `crates/waml/src/source.rs:1-35`

**Interfaces:**
- Consumes: `crate::source::BundlePath::parse(path) -> Result<BundlePath, SourceError>` and `BundlePath::as_str() -> &str` for normalization and validation.
- Produces: `pub fn split_bundle(text: &str) -> Result<Option<Vec<(String, String)>>, BundleEnvelopeError>`.
- Produces: `pub fn encode_bundle_envelope(parts: &[(String, String)]) -> Result<String, BundleEnvelopeError>`.
- Produces: `pub fn encode_bundle_envelope_with(parts: &[(String, String)], next_nonce: impl FnMut() -> Option<u128>) -> Result<String, BundleEnvelopeError>` for deterministic collision and exhaustion tests.
- Produces: the exact public error enum below; later tasks match or display these variants without inventing adapter-specific detection logic.

- [ ] **Step 1: Add public behavior tests before the module exists**

Create `crates/waml/tests/bundle_envelope.rs` with imports, constants, and these concrete tests. Keep the path/content pairs ordered because order is part of the transport contract.

```rust
use waml::bundle_envelope::{
    encode_bundle_envelope_with, split_bundle, BundleEnvelopeError,
};

const A: &str = "0000000000000000000000000000000a";
const B: &str = "0000000000000000000000000000000b";

fn marker(nonce: &str, path: &str, eol: &str) -> String {
    format!("<!-- waml/1 part {nonce} {path} -->{eol}")
}

#[test]
fn decodes_one_lf_part_and_two_crlf_parts() {
    let one = format!("{}# One\n", marker(A, "shop/order.md", "\n"));
    assert_eq!(
        split_bundle(&one).unwrap(),
        Some(vec![("shop/order.md".into(), "# One\n".into())])
    );

    let two = format!(
        "{}first{}second",
        marker(A, "shop/one.md", "\r\n"),
        marker(A, "shop/two.md", "\r\n")
    );
    assert_eq!(
        split_bundle(&two).unwrap(),
        Some(vec![
            ("shop/one.md".into(), "first".into()),
            ("shop/two.md".into(), "second".into()),
        ])
    );
}

#[test]
fn plain_markdown_never_loses_its_prefix() {
    for text in [
        "# Before\n<!-- something.md -->\n# After\n",
        "```md\n<!-- waml/1 part 0000000000000000000000000000000a x.md -->\n```\n",
        "preamble\n<!-- waml/1 part 0000000000000000000000000000000a x.md -->\nbody",
        "<!-- old/path.md -->\nbody",
    ] {
        assert_eq!(split_bundle(text).unwrap(), None, "{text:?}");
    }
}

#[test]
fn different_nonce_marker_is_body_but_matching_marker_splits_at_any_offset() {
    let text = format!(
        "{}left{}kept{}right",
        marker(A, "left.md", "\n"),
        marker(B, "other.md", "\n"),
        marker(A, "right.md", "\n"),
    );
    assert_eq!(
        split_bundle(&text).unwrap(),
        Some(vec![
            ("left.md".into(), format!("left{}kept", marker(B, "other.md", "\n"))),
            ("right.md".into(), "right".into()),
        ])
    );
}

#[test]
fn byte_zero_waml_sentinel_reports_structured_errors() {
    assert!(matches!(
        split_bundle("<!-- waml/2 part 0000000000000000000000000000000a x.md -->\n"),
        Err(BundleEnvelopeError::UnsupportedVersion { version, offset: 0 }) if version == "2"
    ));
    assert!(matches!(
        split_bundle("<!-- waml/1 part ABC x.md -->\n"),
        Err(BundleEnvelopeError::InvalidNonce { offset: 17 })
    ));
    assert!(matches!(
        split_bundle("<!-- waml/1 part 0000000000000000000000000000000a x.md -->"),
        Err(BundleEnvelopeError::MalformedFirstMarker { offset: 0 })
    ));
    let malformed_later = format!(
        "{}body<!-- waml/1 part {}broken -->\n",
        marker(A, "x.md", "\n"),
        A
    );
    assert!(matches!(
        split_bundle(&malformed_later),
        Err(BundleEnvelopeError::MalformedPartMarker { .. })
    ));
}

#[test]
fn path_codec_accepts_required_escapes_and_rejects_bad_paths() {
    let text = format!(
        "{}a{}b{}c{}d",
        marker(A, "shop/special%20order.md", "\n"),
        marker(A, "shop/%E6%B3%A8%E6%96%87.md", "\n"),
        marker(A, "shop/percent%25.md", "\n"),
        marker(A, "shop/%3Clt%3E.md", "\n"),
    );
    let paths: Vec<_> = split_bundle(&text)
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|(path, _)| path)
        .collect();
    assert_eq!(
        paths,
        [
            "shop/special order.md",
            "shop/注文.md",
            "shop/percent%.md",
            "shop/<lt>.md",
        ]
    );

    assert!(matches!(
        split_bundle(&marker(A, "bad%2.md", "\n")),
        Err(BundleEnvelopeError::InvalidPercentEscape { .. })
    ));
    assert!(matches!(
        split_bundle(&marker(A, "bad%FF.md", "\n")),
        Err(BundleEnvelopeError::InvalidPathUtf8 { .. })
    ));
    assert!(matches!(
        split_bundle(&marker(A, "../bad.md", "\n")),
        Err(BundleEnvelopeError::InvalidBundlePath { .. })
    ));
}

#[test]
fn duplicate_paths_are_rejected_after_normalization() {
    let text = format!(
        "{}a{}b",
        marker(A, "shop/order.md", "\n"),
        marker(A, "shop%2Forder.md", "\n"),
    );
    assert!(matches!(
        split_bundle(&text),
        Err(BundleEnvelopeError::DuplicatePath { ref path, .. }) if path == "shop/order.md"
    ));
}

#[test]
fn encoder_preserves_empty_and_unterminated_bodies_and_retries_collision() {
    let first_prefix = "<!-- waml/1 part 0000000000000000000000000000000a";
    let parts = vec![
        ("empty.md".to_owned(), String::new()),
        ("shop/space order.md".to_owned(), format!("{first_prefix} authored")),
        ("shop/注文%.md".to_owned(), "tail-without-newline".to_owned()),
    ];
    let mut nonces = [Some(10_u128), Some(11_u128)].into_iter();
    let encoded = encode_bundle_envelope_with(&parts, || nonces.next().flatten()).unwrap();
    assert!(encoded.starts_with(&marker(B, "empty.md", "\n")));
    assert!(encoded.contains("shop/space%20order.md"));
    assert!(encoded.contains("shop/%E6%B3%A8%E6%96%87%25.md"));
    assert_eq!(split_bundle(&encoded).unwrap(), Some(parts));
}

#[test]
fn encoder_rejects_invalid_input_and_exhausted_nonce_source() {
    assert_eq!(
        encode_bundle_envelope_with(&[], || Some(1)).unwrap_err(),
        BundleEnvelopeError::EmptyBundle
    );
    assert!(matches!(
        encode_bundle_envelope_with(
            &[("bad.txt".into(), String::new())],
            || Some(1)
        ),
        Err(BundleEnvelopeError::InvalidBundlePath { .. })
    ));
    assert!(matches!(
        encode_bundle_envelope_with(
            &[("same.md".into(), String::new()), ("same.md".into(), String::new())],
            || Some(1)
        ),
        Err(BundleEnvelopeError::DuplicatePath { .. })
    ));
    assert_eq!(
        encode_bundle_envelope_with(
            &[("x.md".into(), "<!-- waml/1 part 00000000000000000000000000000001".into())],
            || None
        )
        .unwrap_err(),
        BundleEnvelopeError::NonceSelectionExhausted { attempts: 0 }
    );
}
```

- [ ] **Step 2: Run the focused test to verify RED**

Run:

```powershell
rtk cargo test -p waml --test bundle_envelope
```

Expected: FAIL to compile because `waml::bundle_envelope` does not exist. Do not weaken the assertions to make the legacy splitter pass.

- [ ] **Step 3: Publish the codec module and remove legacy authority**

Add this line beside `pub mod source;` in `crates/waml/src/lib.rs`:

```rust
pub mod bundle_envelope;
```

Delete `BUNDLE_MARKER_RE` and `source::split_bundle` from `crates/waml/src/source.rs`. Do not leave a wrapper with the old magic-path behavior.

Create `crates/waml/src/bundle_envelope.rs` with this public contract:

```rust
use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::source::BundlePath;

const SENTINEL: &str = "<!-- waml/";
const PART_PREFIX: &str = "<!-- waml/1 part ";
const MAX_NONCE_ATTEMPTS: usize = 64;
static NONCE_COUNTER: AtomicU64 = AtomicU64::new(0);

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

pub fn split_bundle(
    text: &str,
) -> Result<Option<Vec<(String, String)>>, BundleEnvelopeError>;

pub fn encode_bundle_envelope(
    parts: &[(String, String)],
) -> Result<String, BundleEnvelopeError>;

pub fn encode_bundle_envelope_with(
    parts: &[(String, String)],
    next_nonce: impl FnMut() -> Option<u128>,
) -> Result<String, BundleEnvelopeError>;
```

Implement `Display` with stable, useful messages. Include offsets where variants have them; use these exact message forms so CLI tests can assert durable fragments:

```rust
impl fmt::Display for BundleEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion { version, offset } => {
                write!(f, "unsupported WAML bundle envelope version {version} at byte {offset}")
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
                write!(f, "could not select a collision-free WAML bundle nonce after {attempts} attempts")
            }
        }
    }
}

impl std::error::Error for BundleEnvelopeError {}
```

- [ ] **Step 4: Implement exact path percent encoding and decoding**

Use byte-based encoding so Unicode is encoded from its UTF-8 representation. Uppercase escapes are canonical on output; accept either hex case on input. Reject literal bytes outside the allowed set, including raw non-ASCII, whitespace, `%` without two hex digits, `<`, and `>`.

```rust
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
```

- [ ] **Step 5: Implement marker parsing and byte-zero decoding**

Use a small hand-written parser, not a permissive regular expression. Introduce a private parsed marker with exact offsets:

```rust
struct Marker {
    nonce: String,
    path: String,
    body_start: usize,
}
```

The private `parse_marker(text, offset, expected_nonce, first)` must do these operations in order:

1. Find the next `\n`; reject a missing line ending.
2. Remove one optional terminal `\r` from the marker line and reject any other `\r`.
3. Require the exact `<!-- waml/` prefix.
4. Read the version up to the next ASCII space. Return `UnsupportedVersion` when it is not `1`.
5. Require exact `part ` after the version.
6. Read exactly 32 nonce bytes, require lowercase ASCII hex, then require one ASCII space. `InvalidNonce.offset` points at the nonce start plus the first invalid position; a short nonce points at its start.
7. Require an encoded path followed immediately by ` -->`, with no trailing spaces.
8. Decode and validate the path with `BundlePath::parse`; store `BundlePath::as_str()` so separator normalization cannot create hidden duplicates.
9. On fixed-token or suffix failure, return `MalformedFirstMarker` for the first marker and `MalformedPartMarker` for later markers.

Implement the three-way detection and same-nonce scan with this structure:

```rust
pub fn split_bundle(
    text: &str,
) -> Result<Option<Vec<(String, String)>>, BundleEnvelopeError> {
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
```

Do not search for generic HTML comments or `.md` lines. Searching only `active_prefix` is what preserves marker-like text with another nonce and makes malformed same-nonce prefixes explicit errors.

- [ ] **Step 6: Implement validated serialization and bounded nonce selection**

Validate and normalize all paths before asking for a nonce. Use a `BTreeSet` to reject duplicates. A candidate collides when its recognized prefix occurs in a normalized path, encoded path, or body. Do not append a newline after a body.

```rust
pub fn encode_bundle_envelope(
    parts: &[(String, String)],
) -> Result<String, BundleEnvelopeError> {
    encode_bundle_envelope_with(parts, production_nonce)
}

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

        let capacity = validated.iter().map(|(path, body)| path.len() + body.len() + 80).sum();
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

fn production_nonce() -> Option<u128> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = NONCE_COUNTER.fetch_add(1, Ordering::Relaxed) as u128;
    let process = u128::from(std::process::id());
    Some(time ^ process.rotate_left(47) ^ counter.rotate_left(89))
}
```

The production source is deliberately dependency-free and non-cryptographic. The collision scan, changing atomic counter, and 64-attempt bound provide the correctness contract.

- [ ] **Step 7: Run focused codec tests and format changed Rust files**

Run:

```powershell
rtk cargo test -p waml --test bundle_envelope
rtk rustfmt --edition 2021 crates/waml/src/bundle_envelope.rs crates/waml/src/lib.rs crates/waml/src/source.rs crates/waml/tests/bundle_envelope.rs
rtk cargo test -p waml --test bundle_envelope
```

Expected: all focused tests PASS. Inspect the exact test count in the command output. If an asserted error offset differs, fix parser accounting; do not weaken the test to `is_err()`.

- [ ] **Step 8: Commit the authoritative codec**

```powershell
rtk git add crates/waml/src/bundle_envelope.rs crates/waml/src/lib.rs crates/waml/src/source.rs crates/waml/tests/bundle_envelope.rs
rtk git commit -m "feat: add bundle envelope v1 codec"
```

Expected: one commit that contains the complete codec authority and its focused public contract tests.

---

### Task 2: Property Guarantees for Losslessness and Detection

**Files:**
- Create: `crates/waml/tests/bundle_envelope_properties.rs`

**Interfaces:**
- Consumes: `encode_bundle_envelope_with` and `split_bundle` from Task 1.
- Produces: bounded property evidence that arbitrary valid ordered bundles round-trip and arbitrary non-sentinel Markdown is never split.

- [ ] **Step 1: Write the property tests**

Create `crates/waml/tests/bundle_envelope_properties.rs`. Generate path segments from a controlled Unicode-safe alphabet, then include bodies with arbitrary UTF-8 and marker-like fragments. Deduplicate generated paths before encoding so the round-trip property tests valid bundles rather than error handling.

```rust
use std::collections::BTreeSet;

use proptest::prelude::*;
use waml::bundle_envelope::{encode_bundle_envelope_with, split_bundle};

fn valid_path() -> impl Strategy<Value = String> {
    prop::collection::vec("[A-Za-z0-9 _~%<>é注]{1,12}", 1..4).prop_map(|segments| {
        format!("{}.md", segments.join("/"))
    })
}

fn valid_bundle() -> impl Strategy<Value = Vec<(String, String)>> {
    prop::collection::vec((valid_path(), any::<String>()), 1..8).prop_filter_map(
        "bundle paths must be unique",
        |pairs| {
            let mut seen = BTreeSet::new();
            pairs
                .iter()
                .all(|(path, _)| seen.insert(path.replace('\\', "/")))
                .then_some(pairs)
        },
    )
}

fn non_sentinel_markdown() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>().prop_map(|mut text| {
            if text.starts_with("<!-- waml/") {
                text.insert(0, 'x');
            }
            text
        }),
        any::<String>().prop_map(|body| format!(
            "authored prefix\n<!-- waml/1 part 0000000000000000000000000000000a x.md -->\n{body}"
        )),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn arbitrary_valid_bundles_round_trip(parts in valid_bundle(), nonce in any::<u128>()) {
        let mut candidate = nonce;
        let encoded = encode_bundle_envelope_with(&parts, || {
            let current = candidate;
            candidate = candidate.wrapping_add(1);
            Some(current)
        }).unwrap();
        prop_assert_eq!(split_bundle(&encoded).unwrap(), Some(parts));
    }

    #[test]
    fn arbitrary_non_sentinel_markdown_is_never_split(text in non_sentinel_markdown()) {
        let original = text.clone();
        prop_assert_eq!(split_bundle(&text).unwrap(), None);
        prop_assert_eq!(text, original);
    }
}
```

If `BundlePath` rejects a generated string, narrow only the path strategy to the actual public `BundlePath` contract. Do not constrain document bodies; arbitrary body UTF-8 is the losslessness target.

- [ ] **Step 2: Run the property test to verify RED or mutation sensitivity**

Run the property test before any adjustment:

```powershell
rtk cargo test -p waml --test bundle_envelope_properties
```

Expected on the correct Task 1 implementation: PASS for 500 cases per property. To prove the suite detects the original defect, temporarily change `split_bundle` detection from `text.starts_with(SENTINEL)` to `text.contains(SENTINEL)` and rerun. Expected: `arbitrary_non_sentinel_markdown_is_never_split` FAILS. Restore the correct byte-zero check immediately and verify `rtk git diff` contains no mutation.

- [ ] **Step 3: Run property and focused codec tests together**

```powershell
rtk cargo test -p waml --test bundle_envelope --test bundle_envelope_properties
```

Expected: both test binaries PASS, with 500 property cases for each property.

- [ ] **Step 4: Commit the property guarantees**

```powershell
rtk git add crates/waml/tests/bundle_envelope_properties.rs
rtk git commit -m "test: fuzz bundle envelope boundaries"
```

Expected: a test-only commit that a reviewer can reject or tune without changing the codec API.

---

### Task 3: CLI Consumer and Producer Migration

**Files:**
- Modify: `crates/waml-cli/src/io.rs:1-26,71-109,782-816`
- Modify: `crates/waml-cli/src/main.rs:352-414,585-591,625-700`
- Modify: `crates/waml-cli/tests/cli_e2e.rs:1-445`

**Interfaces:**
- Consumes: `split_bundle(text) -> Result<Option<Vec<(String, String)>>, BundleEnvelopeError>` and `encode_bundle_envelope(parts) -> Result<String, BundleEnvelopeError>` from Task 1.
- Produces: `pub fn expand_text(display_path: &str, text: &str) -> std::io::Result<Vec<(String, String)>>`.
- Produces: CLI exit code 2 plus a contextual error on malformed envelope input; normal files retain their physical display path and exact full text.
- Produces: multi-document `fmt --stdout` and mutation `--stdout` output in v1 format through the authoritative encoder only; a one-document `fmt --stdout` remains raw Markdown for existing pipe compatibility.

- [ ] **Step 1: Replace CLI input tests with three-outcome tests**

In `crates/waml-cli/src/io.rs`, replace `expands_blob_text_into_docs` and update plain/comment tests to unwrap `io::Result`. Add malformed input coverage:

```rust
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
    assert!(message.contains("unsupported WAML bundle envelope version 2"), "{message}");
}
```

Keep `stray_comment_doc_is_one_doc`, but call `expand_text(...).unwrap()` so its byte-for-byte assertion remains an adapter regression.

- [ ] **Step 2: Run CLI I/O tests to verify RED**

```powershell
rtk cargo test -p waml-cli --bin waml io::tests::
```

Expected: FAIL because `expand_text` still returns a vector, still recognizes legacy markers, and cannot propagate envelope errors.

- [ ] **Step 3: Implement explicit CLI input expansion**

Replace the legacy import and function with:

```rust
use waml::bundle_envelope::split_bundle;

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
```

In both stdin and physical-file branches of `read_analysis_bundle`, use `?`:

```rust
let files = expand_text("stdin.md", &buf)?;
// ...
let expanded = expand_text(&path_key(rel), &text)?;
```

Delete the `text.contains("<!--")` precheck, the `pasted/doc.md` comment, and every fallback-path comparison.

- [ ] **Step 4: Add binary tests for malformed input and authoritative stdout**

Append these tests to `crates/waml-cli/tests/cli_e2e.rs`. The first proves errors reach a non-zero process boundary. The second proves the only concatenated producer emits a valid v1 envelope that the codec can decode.

```rust
#[test]
fn check_rejects_a_malformed_byte_zero_envelope() {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = bin()
        .args(["check", "--stdin", "--format", "json"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"<!-- waml/9 part 0000000000000000000000000000000a x.md -->\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("stdin.md"), "{stderr}");
    assert!(stderr.contains("unsupported WAML bundle envelope version 9"), "{stderr}");
}

#[test]
fn mutation_stdout_is_an_authoritative_v1_envelope() {
    let dir = tmp();
    std::fs::write(
        dir.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();
    let output = bin()
        .args(["attr", "add", "order", "total", "Money", "--stdout", "--dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("<!-- waml/1 part "), "{stdout}");
    let decoded = waml::bundle_envelope::split_bundle(&stdout)
        .unwrap()
        .expect("stdout is a bundle envelope");
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].0, "order.md");
    assert!(decoded[0].1.contains("- total: Money"));
}

#[test]
fn multi_document_fmt_stdout_is_a_v1_envelope() {
    let dir = tmp();
    std::fs::write(dir.join("a.md"), "# A\n").unwrap();
    std::fs::write(dir.join("b.md"), "# B\n").unwrap();
    let output = bin()
        .arg("fmt")
        .arg(&dir)
        .arg("--stdout")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let decoded = waml::bundle_envelope::split_bundle(&stdout)
        .unwrap()
        .expect("multi-document fmt output is a bundle envelope");
    assert_eq!(
        decoded,
        vec![("a.md".into(), "# A\n".into()), ("b.md".into(), "# B\n".into())]
    );
}
```

- [ ] **Step 5: Run the binary tests to verify producer RED**

```powershell
rtk cargo test -p waml-cli --test cli_e2e check_rejects_a_malformed_byte_zero_envelope
rtk cargo test -p waml-cli --test cli_e2e mutation_stdout_is_an_authoritative_v1_envelope
rtk cargo test -p waml-cli --test cli_e2e multi_document_fmt_stdout_is_a_v1_envelope
```

Expected: the malformed-input test becomes GREEN after Step 3. Both stdout tests FAIL: mutation output still uses `<!-- order.md -->`, and multi-document formatter output has no envelope.

- [ ] **Step 6: Remove the competing CLI producer**

Delete `fn to_blob` from `crates/waml-cli/src/main.rs`. Import the authoritative encoder:

```rust
use waml::bundle_envelope::encode_bundle_envelope;
```

Replace the `common.stdout` arm in `run_batch` with explicit error propagation:

```rust
if common.stdout {
    match encode_bundle_envelope(&new) {
        Ok(blob) => {
            print!("{blob}");
            0
        }
        Err(error) => {
            eprintln!("waml: {error}");
            2
        }
    }
} else if common.dry_run {
```

Do not recreate marker formatting in `main.rs`, `commands.rs`, or tests.

In the `Fmt` command, stop printing each `r.formatted` inside the validation loop. After that loop, collect only non-skipped results in plan order. Keep the current raw one-document behavior, and encode only concatenated output:

```rust
let stdout_files: Vec<_> = plan
    .iter()
    .filter(|result| !result.skipped)
    .map(|result| (result.path.clone(), result.formatted.clone()))
    .collect();
if stdout {
    match stdout_files.as_slice() {
        [] => {}
        [(_, text)] => print!("{text}"),
        files => match encode_bundle_envelope(files) {
            Ok(blob) => print!("{blob}"),
            Err(error) => {
                eprintln!("waml: {error}");
                exit = 2;
            }
        },
    }
}
```

Retain the existing skipped-file diagnostics and exit code logic. Keep the existing `fmt_stdout_preserves_generic_okf_exactly` test green to lock the one-document compatibility decision.

- [ ] **Step 7: Run all focused CLI tests and search for old authority**

```powershell
rtk cargo test -p waml-cli --bin waml io::tests::
rtk cargo test -p waml-cli --test cli_e2e
rtk rg -n "pasted/doc\.md|format!\(\"<!-- \{p\} -->|BUNDLE_MARKER_RE" crates --glob "*.rs"
```

Expected: CLI unit and e2e tests PASS. The final search returns no production matches. Historical plans under `docs/superpowers/plans/completed` are out of scope.

- [ ] **Step 8: Format and commit the CLI migration**

```powershell
rtk rustfmt --edition 2021 crates/waml-cli/src/io.rs crates/waml-cli/src/main.rs crates/waml-cli/tests/cli_e2e.rs
rtk git add crates/waml-cli/src/io.rs crates/waml-cli/src/main.rs crates/waml-cli/tests/cli_e2e.rs
rtk git commit -m "fix: migrate CLI bundle envelope transport"
```

Expected: one independently reviewable adapter commit. No parser, LSP, or editor file changes.

---

### Task 4: Live Fixture Migration and Direct API Callers

**Files:**
- Modify: `crates/waml/tests/fixtures/orders-domain.md:1-68`
- Modify: `crates/waml/tests/golden.rs:1,730-870`
- Modify: `crates/waml/tests/ops_golden.rs:1-16`

**Interfaces:**
- Consumes: `split_bundle` from Task 1.
- Produces: all checked-in live concatenated fixtures use v1; all direct codec callers acknowledge plain, valid, and malformed outcomes.

- [ ] **Step 1: Migrate direct callers before the fixture**

Change both imports from `waml::source::split_bundle` to:

```rust
use waml::bundle_envelope::split_bundle;
```

At each of the four call sites, require a valid envelope explicitly:

```rust
let bundle = split_bundle(FIXTURE)
    .expect("orders-domain envelope is valid")
    .expect("orders-domain fixture is an envelope");
```

For `ops_golden.rs`, use `blob` instead of `FIXTURE` in the same expression. Do not use `unwrap_or` or restore a fallback path; a fixture format regression must fail loudly.

- [ ] **Step 2: Run direct fixture tests to verify RED**

```powershell
rtk cargo test -p waml --test golden orders_domain
rtk cargo test -p waml --test ops_golden rename_on_orders_domain_fixture_rewrites_all_referrers
```

Expected: FAIL because the legacy fixture now decodes as `Ok(None)`.

- [ ] **Step 3: Replace all live fixture markers with one fixed nonce**

In `crates/waml/tests/fixtures/orders-domain.md`, use nonce `7d91ac42f5e649c4a6cd939cfa60b920` for all six markers. Replace only marker lines; preserve every document body byte:

```markdown
<!-- waml/1 part 7d91ac42f5e649c4a6cd939cfa60b920 shop/order.md -->
```

Apply the same exact prefix to `shop/order-line.md`, `shop/customer.md`, `shop/order-status.md`, `shop/money.md`, and `shop/orders-domain.md` at their existing boundaries. Do not add blank lines around markers.

- [ ] **Step 4: Verify migrated fixture behavior and live-marker inventory**

```powershell
rtk cargo test -p waml --test golden orders_domain
rtk cargo test -p waml --test golden every_doc_is_lossless_through_the_authoritative_shell
rtk cargo test -p waml --test ops_golden rename_on_orders_domain_fixture_rewrites_all_referrers
rtk rg -n "<!-- [^>]*\.md -->" crates --glob "!**/*.rs"
rtk rg -n "<!-- waml/1 part " crates/waml/tests/fixtures
```

Expected: all focused fixture tests PASS. The legacy-marker search returns no live non-Rust fixture. The v1 search returns the six migrated orders-domain markers. Do not rewrite historical design or completed-plan examples.

- [ ] **Step 5: Commit the fixture and caller migration**

```powershell
rtk git add crates/waml/tests/fixtures/orders-domain.md crates/waml/tests/golden.rs crates/waml/tests/ops_golden.rs
rtk git commit -m "test: migrate bundle fixture to v1"
```

Expected: one fixture-focused commit with no production behavior changes beyond API call-site compilation.

---

## Final Verification

- [ ] **Step 1: Verify the complete codec and CLI slice**

```powershell
rtk cargo test -p waml --test bundle_envelope --test bundle_envelope_properties --test golden --test ops_golden
rtk cargo test -p waml-cli --bin waml
rtk cargo test -p waml-cli --test cli_e2e
```

Expected: all focused tests PASS. The property test reports 500 cases per property.

- [ ] **Step 2: Run the full workspace verification**

```powershell
rtk cargo test --workspace
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: the complete workspace test suite passes and strict Clippy exits 0 with no warnings.

- [ ] **Step 3: Check formatting without rewriting unrelated files**

Run rustfmt only on changed Rust files first:

```powershell
rtk rustfmt --edition 2021 crates/waml/src/bundle_envelope.rs crates/waml/src/lib.rs crates/waml/src/source.rs crates/waml/tests/bundle_envelope.rs crates/waml/tests/bundle_envelope_properties.rs crates/waml/tests/golden.rs crates/waml/tests/ops_golden.rs crates/waml-cli/src/io.rs crates/waml-cli/src/main.rs crates/waml-cli/tests/cli_e2e.rs
rtk git diff --check 2d6588f5..HEAD
```

Then run the workspace check without applying changes:

```powershell
rtk cargo fmt --all -- --check
```

Expected: changed files are formatted and `git diff --check` exits 0. If the workspace format check reports pre-existing drift in unchanged files, record the exact paths and do not modify them in this branch.

- [ ] **Step 4: Prove authority and migration acceptance criteria**

```powershell
rtk rg -n "BUNDLE_MARKER_RE|pasted/doc\.md|format!\(\"<!-- \{p\} -->" crates --glob "*.rs"
rtk rg -n "waml/1 part" crates/waml/src crates/waml-cli/src
rtk rg -n "<!-- [^>]*\.md -->" crates/waml/tests/fixtures
rtk git status --short --branch
```

Expected:

- No legacy regex, magic fallback path, or local old-marker producer remains.
- The grammar string exists only in `crates/waml/src/bundle_envelope.rs`; CLI source names codec functions but does not contain marker grammar.
- No live fixture uses the old headerless syntax.
- Worktree status is clean on `codex/bundle-envelope-v1` after commits.

- [ ] **Step 5: Request independent code review before integration**

Give the reviewer the approved spec, this plan, and the branch diff from `2d6588f5` through `HEAD`. Ask the reviewer to verify byte offsets, CRLF consumption, arbitrary-offset same-nonce parsing, percent-decoding safety, duplicate normalization, collision termination, CLI error propagation, legacy-authority removal, and mutation-sensitive tests. Address important findings with a focused fix commit and rerun the affected commands plus the full workspace suite.

---

## Post-Integration Issue Reconciliation

The clean feature worktree does not contain the current P1 entry because `C:\dev\waml\issues.md` has user-owned uncommitted review updates. Do not copy the dirty file into this branch and do not overwrite it during integration.

After the reviewed branch is fast-forwarded into local `main`, perform these exact steps in the primary checkout:

1. Confirm `rtk git status --short -- issues.md` still reports only the user-owned modification.
2. Use `apply_patch` to remove only the section from `## P1 — Bundle-envelope autodetection can discard authored bytes` through its recommendation item `3. Test ordinary HTML comments, comments in fenced code, malformed markers, non-empty preambles, and both LF and CRLF.` Leave the following `## P1 — Native and CLI persistence have different transaction guarantees` header intact.
3. Run `rtk rg -n "Bundle-envelope autodetection|authored bytes" issues.md`; expect no match.
4. Run `rtk git diff -- issues.md` and verify all other user-authored issue edits remain byte for byte.
5. Do not stage or commit `issues.md` unless the user explicitly asks. Report that the resolved item was cleared from the user-owned issue ledger.

This closes the issue only after code, tests, review, and integration provide evidence that every acceptance criterion is true.
