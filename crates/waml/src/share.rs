//! Encode a whole OKF bundle into a URL fragment, and decode it back.
//!
//! Retained compatibility framing for self-contained bundle fragments.
//!
//! Format, versioned by the `w1.` prefix so a later revision can change the
//! framing without silently misreading old links:
//!
//! ```text
//! "w1." base64url_nopad( deflate_raw( payload ) )
//!
//! payload := u32le file_count , { u32le path_len , path_utf8 ,
//!                                 u32le body_len , body_utf8 }
//! ```
//!
//! Files are emitted in the order given. [`crate::okf`] bundles arrive sorted
//! by path, which makes the encoding deterministic: the same bundle always
//! yields the same link, so a shared URL is stable and diffable.
//!
//! This is deliberately *not* compatible with the fragment format of the React
//! application this project once resembled; the two have long since diverged.

use crate::source::SourceBundle;
use miniz_oxide::deflate::compress_to_vec;
use miniz_oxide::inflate::decompress_to_vec_with_limit;

/// Version tag that opens every fragment. Bump alongside the framing.
const PREFIX: &str = "w1.";

/// Ceiling on the *decompressed* payload, so a hostile link cannot make the
/// editor allocate without bound. 64 MiB is far beyond any real bundle (the
/// fragment itself would have to survive browser URL limits first).
const MAX_DECOMPRESSED: usize = 64 * 1024 * 1024;

/// miniz_oxide compression level. 10 is the maximum; a fragment is written
/// once and read many times, and shorter URLs are the whole point.
const COMPRESSION_LEVEL: u8 = 10;

/// Why a fragment could not be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareError {
    /// Missing the `w1.` prefix -- not a bundle fragment at all (or a newer
    /// format this build does not understand).
    NotAShareLink,
    /// A character outside the base64url alphabet.
    BadBase64,
    /// The deflate stream would not inflate, or exceeded [`MAX_DECOMPRESSED`].
    BadDeflate,
    /// The payload ended mid-record, or declared a length past its own end.
    Truncated,
    /// A path or body was not valid UTF-8.
    NotUtf8,
    /// A decoded path violates current bundle path invariants.
    InvalidSource,
}

impl std::fmt::Display for ShareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            Self::NotAShareLink => "not a waml share link",
            Self::BadBase64 => "share link contains invalid base64url",
            Self::BadDeflate => "share link payload is not valid deflate data",
            Self::Truncated => "share link payload is truncated",
            Self::NotUtf8 => "share link payload is not valid UTF-8",
            Self::InvalidSource => "share link contains an invalid bundle path",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for ShareError {}

/// Encode `bundle` (as produced by the editor's `read_bundle`) into a fragment
/// body, *without* a leading `#`.
pub fn encode_source(bundle: &SourceBundle) -> String {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(bundle.len() as u32).to_le_bytes());
    for document in bundle.documents() {
        let path = document.path().as_str();
        let body = document.text();
        payload.extend_from_slice(&(path.len() as u32).to_le_bytes());
        payload.extend_from_slice(path.as_bytes());
        payload.extend_from_slice(&(body.len() as u32).to_le_bytes());
        payload.extend_from_slice(body.as_bytes());
    }
    let compressed = compress_to_vec(&payload, COMPRESSION_LEVEL);
    format!("{PREFIX}{}", base64url_encode(&compressed))
}

pub fn encode(bundle: &[(String, String)]) -> String {
    let source =
        SourceBundle::try_from_pairs(bundle.iter().cloned()).expect("bundle paths must be valid");
    encode_source(&source)
}

/// Decode a fragment back into a bundle. A leading `#` is accepted and ignored,
/// so callers can hand over `location.hash` verbatim.
pub fn decode_source(fragment: &str) -> Result<SourceBundle, ShareError> {
    let body = fragment.strip_prefix('#').unwrap_or(fragment);
    let b64 = body.strip_prefix(PREFIX).ok_or(ShareError::NotAShareLink)?;
    let compressed = base64url_decode(b64)?;
    let payload = decompress_to_vec_with_limit(&compressed, MAX_DECOMPRESSED)
        .map_err(|_| ShareError::BadDeflate)?;

    let mut cursor = 0usize;
    let count = read_u32(&payload, &mut cursor)? as usize;
    // A bogus count must not pre-allocate: grow as records are actually read.
    let mut out = Vec::new();
    for _ in 0..count {
        let path = read_str(&payload, &mut cursor)?;
        let body = read_str(&payload, &mut cursor)?;
        out.push((path, body));
    }
    SourceBundle::try_from_pairs(out).map_err(|_| ShareError::InvalidSource)
}

pub fn decode(fragment: &str) -> Result<Vec<(String, String)>, ShareError> {
    decode_source(fragment).map(|bundle| bundle.to_pairs())
}

/// True when `fragment` looks like a share link. Lets a caller distinguish
/// "no document in the URL" from "a document that failed to decode".
pub fn is_share_link(fragment: &str) -> bool {
    fragment
        .strip_prefix('#')
        .unwrap_or(fragment)
        .starts_with(PREFIX)
}

fn read_u32(buf: &[u8], cursor: &mut usize) -> Result<u32, ShareError> {
    let end = cursor.checked_add(4).ok_or(ShareError::Truncated)?;
    let bytes = buf.get(*cursor..end).ok_or(ShareError::Truncated)?;
    *cursor = end;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn read_str(buf: &[u8], cursor: &mut usize) -> Result<String, ShareError> {
    let len = read_u32(buf, cursor)? as usize;
    let end = cursor.checked_add(len).ok_or(ShareError::Truncated)?;
    let bytes = buf.get(*cursor..end).ok_or(ShareError::Truncated)?;
    *cursor = end;
    String::from_utf8(bytes.to_vec()).map_err(|_| ShareError::NotUtf8)
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url without padding (RFC 4648 §5). Hand-rolled to keep the dependency
/// footprint at exactly one crate (miniz_oxide); the alphabet is URL-safe so
/// the fragment needs no percent-encoding.
fn base64url_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        // 3 input bytes -> 4 output chars; a short final chunk emits only the
        // characters its bits actually cover, which is what "no padding" means.
        let chars = chunk.len() + 1;
        for i in 0..chars {
            out.push(ALPHABET[((n >> (18 - 6 * i)) & 0x3f) as usize] as char);
        }
    }
    out
}

fn base64url_decode(s: &str) -> Result<Vec<u8>, ShareError> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for ch in s.bytes() {
        let v = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(ShareError::BadBase64),
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    // Canonicality (RFC 4648 §3.5): a trailing chunk of one char (len % 4 == 1)
    // carries fewer bits than a byte and encodes nothing, and any nonzero bits
    // left over past the last full byte mean the input was not produced by the
    // encoder. Reject both so every payload has exactly one accepted encoding.
    if bits >= 6 || (acc & ((1 << bits) - 1)) != 0 {
        return Err(ShareError::BadBase64);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<(String, String)> {
        vec![
            ("customer.md".into(), "---\ntitle: Customer\n---\n".into()),
            ("shop/order.md".into(), "# Order\n\nBody text.\n".into()),
        ]
    }

    #[test]
    fn base64url_decode_rejects_non_canonical_encodings() {
        // len % 4 == 1 can never be produced by the encoder.
        assert!(matches!(
            base64url_decode("AAAAA"),
            Err(ShareError::BadBase64)
        ));
        assert!(matches!(base64url_decode("A"), Err(ShareError::BadBase64)));
        // Nonzero dangling bits: "AB" and "AA" both decoded to [0] before; only
        // "AA" (dangling bits zero) is canonical.
        assert_eq!(base64url_decode("AA").unwrap(), vec![0]);
        assert!(matches!(base64url_decode("AB"), Err(ShareError::BadBase64)));
        assert!(matches!(
            base64url_decode("AAB"),
            Err(ShareError::BadBase64)
        ));
        // Round-trip stays intact for every remainder class.
        for payload in [&b""[..], b"x", b"xy", b"xyz", b"xyzw"] {
            assert_eq!(
                base64url_decode(&base64url_encode(payload)).unwrap(),
                payload
            );
        }
    }

    #[test]
    fn round_trips_a_bundle() {
        let encoded = encode(&sample());
        assert_eq!(decode(&encoded).unwrap(), sample());
    }

    #[test]
    fn accepts_a_leading_hash() {
        let encoded = format!("#{}", encode(&sample()));
        assert_eq!(decode(&encoded).unwrap(), sample());
    }

    #[test]
    fn round_trips_an_empty_bundle() {
        let encoded = encode(&[]);
        assert_eq!(decode(&encoded).unwrap(), Vec::new());
    }

    #[test]
    fn round_trips_non_ascii_content() {
        // Byte lengths, not char counts -- a multi-byte body must survive.
        let bundle = vec![("café.md".to_string(), "→ ünïcödé ✓\n".to_string())];
        assert_eq!(decode(&encode(&bundle)).unwrap(), bundle);
    }

    #[test]
    fn encoding_is_deterministic() {
        assert_eq!(encode(&sample()), encode(&sample()));
    }

    #[test]
    fn fragment_is_url_safe() {
        let encoded = encode(&sample());
        assert!(
            encoded
                .strip_prefix(PREFIX)
                .unwrap()
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_'),
            "fragment must need no percent-encoding: {encoded}"
        );
    }

    #[test]
    fn compresses_repetitive_markdown() {
        // The whole point of deflating: a realistic bundle repeats its
        // frontmatter keys and heading shapes heavily.
        let body = "---\ntitle: Thing\nkind: entity\n---\n\n## Attributes\n\n- id\n";
        let bundle: Vec<(String, String)> = (0..40)
            .map(|i| (format!("n{i}.md"), body.to_string()))
            .collect();
        let raw: usize = bundle.iter().map(|(p, b)| p.len() + b.len()).sum();
        assert!(
            encode(&bundle).len() < raw / 4,
            "expected >4x compression on repetitive input"
        );
    }

    #[test]
    fn rejects_a_plain_fragment() {
        assert_eq!(decode("#some-anchor"), Err(ShareError::NotAShareLink));
        assert!(!is_share_link("#some-anchor"));
        assert!(is_share_link("#w1.abc"));
    }

    #[test]
    fn rejects_bad_base64() {
        assert_eq!(decode("w1.not*valid"), Err(ShareError::BadBase64));
    }

    #[test]
    fn rejects_corrupt_deflate() {
        assert_eq!(decode("w1.AAAAAAAA"), Err(ShareError::BadDeflate));
    }

    #[test]
    fn rejects_a_truncated_payload() {
        // A count of 1 with no record following it must not panic or hang.
        let payload = 1u32.to_le_bytes().to_vec();
        let fragment = format!(
            "{PREFIX}{}",
            base64url_encode(&compress_to_vec(&payload, COMPRESSION_LEVEL))
        );
        assert_eq!(decode(&fragment), Err(ShareError::Truncated));
    }

    #[test]
    fn rejects_a_lying_length_without_allocating() {
        // Declares a 4 GiB path inside a 12-byte payload.
        let mut payload = 1u32.to_le_bytes().to_vec();
        payload.extend_from_slice(&u32::MAX.to_le_bytes());
        payload.extend_from_slice(b"short");
        let fragment = format!(
            "{PREFIX}{}",
            base64url_encode(&compress_to_vec(&payload, COMPRESSION_LEVEL))
        );
        assert_eq!(decode(&fragment), Err(ShareError::Truncated));
    }

    #[test]
    fn base64_round_trips_every_tail_length() {
        // 3-byte chunking makes the last chunk the interesting case.
        for len in 0..8usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(37)).collect();
            assert_eq!(base64url_decode(&base64url_encode(&bytes)).unwrap(), bytes);
        }
    }
}
