//! The file name an exported site and the editor that boots it both agree on.
//!
//! `waml export site` (in `waml-cli`) writes this file beside the exported
//! editor; the wasm editor (in `waml-editor`) fetches it at startup. The two
//! crates share no other dependency edge for this string, so the constant
//! lives here, in the one crate both already depend on -- a renamed literal
//! on either side would otherwise ship a silently broken site.

/// The file a site declares its boot source in, read by the editor at startup.
///
/// It holds a query string (`?bundle=…` or `?api=…`) because that is the
/// grammar the editor already parses for URLs. Writing it beside the editor,
/// rather than pushing it into the address bar, keeps a visitor's URL clean.
pub const SITE_BOOT_CONFIG_FILE: &str = "waml-boot.txt";
