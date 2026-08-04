//! Decide what the browser build opens at startup, from the URL alone.
//!
//! A page can be reached four ways, and they are not equal: a share link
//! (`#w1.<payload>`) carries the whole model in the URL and must win over
//! anything the query string asks for, because it is the more specific
//! request and the only one the user can have edited by hand. Then `?api=`,
//! then a static `?bundle=`, and finally the start screen -- which, on an
//! exported site, is where [`select_site_boot`] takes over: the site's own
//! `waml-boot.txt` names what to open when the URL names nothing, so the
//! address bar stays clean.
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
    /// `?api=<base>`, with an optional bearer token: a live model server.
    ///
    /// SECURITY: the token belongs in the URL *fragment* (`#token=<t>`), never
    /// the query string. A query-string secret lands in browser history, proxy
    /// and server access logs, and potentially `Referer` headers; the fragment
    /// is never sent to any server. The query spelling is still accepted as a
    /// deprecated fallback, but when `waml serve` ships, its boot path must
    /// also strip the token from the address bar (`history.replaceState`)
    /// immediately after this read.
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
        // Prefer a fragment-carried token (`#token=<t>`) over the leaky
        // query-string spelling -- see the `Api` variant's SECURITY note. The
        // fragment is parsed leniently: a page can carry an ordinary anchor
        // (`#section-2`) instead, which simply yields no token.
        let fragment_token = fragment_token(hash);
        return Ok(BrowserBootSource::Api {
            base: base.to_owned(),
            token: fragment_token.or_else(|| value("token").map(str::to_owned)),
        });
    }
    if let Some(url) = value("bundle") {
        return Ok(BrowserBootSource::Bundle(url.to_owned()));
    }
    Ok(BrowserBootSource::Start)
}

/// The file an exported site carries beside its editor, naming what to boot.
///
/// The boot source used to travel in the URL, which meant every visitor saw
/// `?bundle=bundle.waml` in the address bar for a choice they did not make. A
/// sibling file carries the same string out of sight. It holds a query string,
/// not a new format, so one grammar and one parser serve both channels.
///
/// The name itself lives in `waml::site_boot`, not here: `waml-cli` writes
/// this file and this crate reads it, and the two share no other dependency
/// edge for the literal.
pub(crate) use waml::site_boot::SITE_BOOT_CONFIG_FILE as BOOT_CONFIG_FILE;

/// Pick the boot source a site declares for itself.
///
/// The contents are a query string as `select_browser_boot` reads one. A host
/// that serves an error page (or an `index.html`) at this path answers 200 with
/// HTML, so anything that does not open with `?` is refused rather than parsed
/// into nonsense. The empty file is not an error: it is a site that declares
/// the start screen.
///
/// Only the page URL can carry a share fragment, so the config is parsed with
/// an empty `hash` and cannot reach [`BrowserBootSource::Share`].
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn select_site_boot(config: &str) -> Result<BrowserBootSource, String> {
    let query = config.trim();
    if query.is_empty() {
        return Ok(BrowserBootSource::Start);
    }
    if !query.starts_with('?') {
        return Err(format!(
            "{BOOT_CONFIG_FILE} must hold a query string that starts with '?'"
        ));
    }
    let query = query.split_once('#').map_or(query, |(head, _)| head);
    select_browser_boot(query, "")
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

/// The bearer token an `?api=` boot carries in the URL fragment, if any.
///
/// The fragment is the non-leaky channel for a secret (never sent to servers,
/// not kept in proxy logs), so it wins over a query-string `token`. Lenient on
/// purpose: a fragment that is not a `key=value` list (a plain `#anchor`, or
/// malformed percent-encoding) is somebody else's fragment, not an error.
fn fragment_token(hash: &str) -> Option<String> {
    let hash = hash.strip_prefix('#').unwrap_or(hash);
    parse_query(hash)
        .ok()?
        .into_iter()
        .find(|(key, value)| key == "token" && !value.is_empty())
        .map(|(_, value)| value)
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
    fn api_token_in_the_fragment_is_preferred_over_the_query() {
        assert_eq!(
            select_browser_boot("?api=https://example.test&token=leaky", "#token=safe").unwrap(),
            BrowserBootSource::Api {
                base: "https://example.test".to_string(),
                token: Some("safe".to_string()),
            }
        );
    }

    #[test]
    fn api_token_can_arrive_in_the_fragment_alone() {
        assert_eq!(
            select_browser_boot("?api=https://example.test", "#token=t%20v").unwrap(),
            BrowserBootSource::Api {
                base: "https://example.test".to_string(),
                token: Some("t v".to_string()),
            }
        );
    }

    #[test]
    fn a_plain_anchor_fragment_is_not_a_token() {
        assert_eq!(
            select_browser_boot("?api=https://example.test", "#section-2").unwrap(),
            BrowserBootSource::Api {
                base: "https://example.test".to_string(),
                token: None,
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

    #[test]
    fn a_site_config_naming_a_bundle_boots_it() {
        assert_eq!(
            select_site_boot("?bundle=bundle.waml\n").unwrap(),
            BrowserBootSource::Bundle("bundle.waml".to_string())
        );
    }

    #[test]
    fn a_site_config_naming_a_server_boots_the_api() {
        assert_eq!(
            select_site_boot("?api=/api").unwrap(),
            BrowserBootSource::Api {
                base: "/api".to_string(),
                token: None,
            }
        );
    }

    #[test]
    fn an_html_error_page_is_not_a_site_config() {
        let error = select_site_boot("<!doctype html><title>404</title>").unwrap_err();
        assert!(error.contains("waml-boot.txt"), "{error}");
    }

    #[test]
    fn an_empty_site_config_is_the_start_screen() {
        assert_eq!(select_site_boot("   \n").unwrap(), BrowserBootSource::Start);
    }

    #[test]
    fn a_site_config_can_never_be_a_share_link() {
        // The fragment is the page's, not the file's: a config that tries to
        // carry one would let an exported site's own bytes decide what a
        // visitor's URL means.
        assert_eq!(
            select_site_boot("?bundle=a.waml#w1.abc").unwrap(),
            BrowserBootSource::Bundle("a.waml".to_string())
        );
    }
}
