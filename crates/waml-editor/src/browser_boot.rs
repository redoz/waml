//! Decide what the browser build opens at startup, from the URL alone.
//!
//! A page can be reached four ways, and they are not equal: a share link
//! (`#w1.<payload>`) carries the whole model in the URL and must win over
//! anything the query string asks for, because it is the more specific
//! request and the only one the user can have edited by hand. Then `?api=`,
//! then a static `?bundle=` as exported sites use, and finally the start
//! screen when the URL says nothing.
//!
//! Selection is pure: it reads `WebParams.search` and `WebParams.hash` once at
//! startup and answers with a [`BrowserBootSource`]. Nothing downstream gets
//! to re-inspect platform URLs, so the precedence lives in exactly one place
//! and is testable on the host.

use waml::source::SourceBundle;

/// What the browser build should open, chosen from the page URL.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) enum BrowserBootSource {
    /// `#w1.<payload>`: a whole model deflated into the fragment.
    Share(String),
    /// `?api=<base>[&token=<token>]`: a live model server.
    Api { base: String, token: Option<String> },
    /// `?bundle=<url>`: a static Bundle Envelope v1 file to fetch.
    Bundle(String),
    /// The URL asks for nothing: show the start screen.
    Start,
}

/// Pick the boot source for a page, given its query string and fragment.
///
/// `search` and `hash` are taken verbatim as the browser reports them, leading
/// `?` and `#` included or absent. An `Err` means the query string itself is
/// malformed (bad percent-encoding); a URL that merely asks for nothing is
/// [`BrowserBootSource::Start`], not an error.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn select_browser_boot(search: &str, hash: &str) -> Result<BrowserBootSource, String> {
    if waml::share::is_share_link(hash) {
        return Ok(BrowserBootSource::Share(hash.to_owned()));
    }
    let params = parse_query(search)?;
    let value = |name: &str| {
        params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
            .filter(|value| !value.is_empty())
    };
    if let Some(base) = value("api") {
        return Ok(BrowserBootSource::Api {
            base: base.to_owned(),
            token: value("token").map(str::to_owned),
        });
    }
    if let Some(url) = value("bundle") {
        return Ok(BrowserBootSource::Bundle(url.to_owned()));
    }
    Ok(BrowserBootSource::Start)
}

/// Decode fetched bytes as a Bundle Envelope v1 model.
///
/// A fetched `?bundle=` is required to be a real envelope: unlike the CLI,
/// which treats a lone document as a one-file bundle, a site that serves
/// something else at that URL (an error page, an index.html) would otherwise
/// open as a document full of HTML. The specific reason is carried out so the
/// user sees why the editor came up empty.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn decode_boot_bundle(bytes: &[u8]) -> Result<SourceBundle, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "the file is not UTF-8 text".to_string())?;
    let parts = waml::bundle_envelope::split_bundle(text)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "the file is not a WAML bundle".to_string())?;
    SourceBundle::try_from_pairs(parts).map_err(|error| error.to_string())
}

/// The message shown when a `?bundle=` URL could not be fetched.
///
/// `status` is `None` when the request never produced one -- a dead host, or a
/// cross-origin response the browser refused to hand us. Those two are
/// indistinguishable from wasm by design, so say both.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn boot_fetch_error(url: &str, status: Option<u16>) -> String {
    match status {
        Some(status) => format!("could not load {url}: HTTP {status}"),
        None => format!("could not load {url}: the request failed or was blocked by CORS"),
    }
}

/// Percent-decode a query string into its key/value pairs.
fn parse_query(search: &str) -> Result<Vec<(String, String)>, String> {
    let search = search.strip_prefix('?').unwrap_or(search);
    let mut pairs = Vec::new();
    for field in search.split('&').filter(|field| !field.is_empty()) {
        let (key, value) = field.split_once('=').unwrap_or((field, ""));
        pairs.push((percent_decode(key)?, percent_decode(value)?));
    }
    Ok(pairs)
}

/// Percent-decode one query component, `+` meaning a space as forms encode it.
fn percent_decode(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' => {
                let hex = bytes
                    .get(index + 1..index + 3)
                    .and_then(|hex| std::str::from_utf8(hex).ok())
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                    .ok_or_else(|| format!("malformed percent-encoding in {raw:?}"))?;
                out.push(hex);
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| format!("malformed percent-encoding in {raw:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share_hash() -> String {
        waml::share::encode_source(&SourceBundle::try_from_pairs([("a.md", "# A\n")]).unwrap())
    }

    #[test]
    fn share_fragment_beats_api() {
        let hash = format!("#{}", share_hash());
        assert_eq!(
            select_browser_boot("?api=https://example.test", &hash).unwrap(),
            BrowserBootSource::Share(hash)
        );
    }

    #[test]
    fn share_fragment_beats_bundle() {
        let hash = format!("#{}", share_hash());
        assert_eq!(
            select_browser_boot("?bundle=bundle.waml", &hash).unwrap(),
            BrowserBootSource::Share(hash)
        );
    }

    #[test]
    fn api_beats_bundle() {
        assert_eq!(
            select_browser_boot("?bundle=bundle.waml&api=https://example.test&token=t", "")
                .unwrap(),
            BrowserBootSource::Api {
                base: "https://example.test".to_string(),
                token: Some("t".to_string()),
            }
        );
    }

    #[test]
    fn api_without_token_carries_none() {
        assert_eq!(
            select_browser_boot("?api=https://example.test", "").unwrap(),
            BrowserBootSource::Api {
                base: "https://example.test".to_string(),
                token: None,
            }
        );
    }

    #[test]
    fn relative_bundle_url_is_kept_as_written() {
        assert_eq!(
            select_browser_boot("?bundle=bundle.waml", "").unwrap(),
            BrowserBootSource::Bundle("bundle.waml".to_string())
        );
    }

    #[test]
    fn absolute_bundle_url_is_percent_decoded() {
        assert_eq!(
            select_browser_boot("?bundle=https%3A%2F%2Fexample.test%2Fa%20b.waml", "").unwrap(),
            BrowserBootSource::Bundle("https://example.test/a b.waml".to_string())
        );
    }

    #[test]
    fn empty_values_are_ignored() {
        assert_eq!(
            select_browser_boot("?api=&bundle=", "").unwrap(),
            BrowserBootSource::Start
        );
    }

    #[test]
    fn empty_url_is_the_start_screen() {
        assert_eq!(
            select_browser_boot("", "").unwrap(),
            BrowserBootSource::Start
        );
    }

    #[test]
    fn a_non_share_anchor_is_not_our_business() {
        assert_eq!(
            select_browser_boot("", "#section-2").unwrap(),
            BrowserBootSource::Start
        );
    }

    #[test]
    fn a_non_share_anchor_still_allows_a_bundle() {
        assert_eq!(
            select_browser_boot("?bundle=bundle.waml", "#section-2").unwrap(),
            BrowserBootSource::Bundle("bundle.waml".to_string())
        );
    }

    #[test]
    fn malformed_percent_encoding_is_an_error() {
        let error = select_browser_boot("?bundle=%zz", "").unwrap_err();
        assert!(error.contains("percent-encoding"), "{error}");
    }

    #[test]
    fn a_valid_envelope_decodes() {
        let bundle = SourceBundle::try_from_pairs([("a.md", "# A\n"), ("b.md", "# B\n")]).unwrap();
        let bytes = waml::bundle_envelope::encode_bundle_envelope(&bundle.to_pairs())
            .unwrap()
            .into_bytes();
        let decoded = decode_boot_bundle(&bytes).unwrap();
        assert_eq!(decoded.to_pairs(), bundle.to_pairs());
    }

    #[test]
    fn a_plain_document_is_not_a_bundle() {
        let error = decode_boot_bundle(b"<html>not a bundle</html>").unwrap_err();
        assert_eq!(error, "the file is not a WAML bundle");
    }

    #[test]
    fn http_status_failure_names_the_url_and_status() {
        assert_eq!(
            boot_fetch_error("bundle.waml", Some(404)),
            "could not load bundle.waml: HTTP 404"
        );
    }

    #[test]
    fn network_failure_says_cors_too() {
        let message = boot_fetch_error("bundle.waml", None);
        assert!(message.contains("CORS"), "{message}");
    }
}
