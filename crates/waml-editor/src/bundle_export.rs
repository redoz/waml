//! Export the model the editor currently holds as one `.waml` bundle file.
//!
//! The encoding is Bundle Envelope v1, the same transport the CLI reads, so an
//! exported file can be piped straight back into `waml check` or dropped onto
//! an exported site. What is exported is the live session source, not what is
//! on disk: an unsaved edit, or one that only ever existed in a share URL, is
//! part of the model and must survive the round trip.
//!
//! Delivery differs per platform and is therefore behind an adapter: native
//! opens a save dialog and writes the bytes, the browser hands them to the
//! download bridge. Tests inject their own adapter and assert on the bytes.

use makepad_widgets::*;
use waml::bundle_envelope::encode_bundle_envelope;
use waml::source::SourceBundle;

/// The MIME type an exported bundle is delivered under. It is not registered
/// anywhere; it exists so the browser does not guess `text/html` from the
/// bytes and render the download instead of saving it.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) const BUNDLE_MIME_TYPE: &str = "application/vnd.waml.bundle";

/// Encode the current model as one Bundle Envelope v1 payload.
pub(crate) fn encode_current_bundle(source: &SourceBundle) -> Result<Vec<u8>, String> {
    encode_bundle_envelope(&source.to_pairs())
        .map(String::into_bytes)
        .map_err(|error| error.to_string())
}

/// Turn a model title into a file name a save dialog can offer.
///
/// Anything that is not a plain name character becomes `-`, so a title with a
/// path separator, a quote, or a colon cannot steer the write. An empty result
/// falls back to `model`, because a dialog pre-filled with `.waml` is worse
/// than one pre-filled with a boring name.
pub(crate) fn suggested_file_name(title: &str) -> String {
    let mut name = String::with_capacity(title.len());
    let mut last_was_dash = false;
    for character in title.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            name.push(character.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            name.push('-');
            last_was_dash = true;
        }
    }
    let trimmed = name.trim_matches('-');
    if trimmed.is_empty() {
        "model.waml".to_owned()
    } else {
        format!("{trimmed}.waml")
    }
}

/// How encoded bundle bytes reach the user. One implementation per platform,
/// plus whatever a test injects.
pub(crate) trait BundleExportAdapter {
    fn export(&mut self, cx: &mut Cx, suggested_name: &str, bytes: Vec<u8>) -> Result<(), String>;
}

/// Encode `source` and hand it to `adapter`.
///
/// A user who cancels the save dialog has not hit an error, so the adapter
/// reports cancellation as `Ok`.
pub(crate) fn export_bundle(
    cx: &mut Cx,
    adapter: &mut dyn BundleExportAdapter,
    title: &str,
    source: &SourceBundle,
) -> Result<(), String> {
    let bytes = encode_current_bundle(source)?;
    adapter.export(cx, &suggested_file_name(title), bytes)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct PlatformBundleExport;

#[cfg(not(target_arch = "wasm32"))]
impl BundleExportAdapter for PlatformBundleExport {
    fn export(&mut self, _cx: &mut Cx, suggested_name: &str, bytes: Vec<u8>) -> Result<(), String> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export WAML bundle")
            .set_file_name(suggested_name)
            .add_filter("WAML bundle", &["waml"])
            .save_file()
        else {
            return Ok(()); // Cancelled: the user changed their mind, not a failure.
        };
        std::fs::write(&path, bytes).map_err(|error| format!("{}: {error}", path.display()))
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) struct PlatformBundleExport;

#[cfg(target_arch = "wasm32")]
impl BundleExportAdapter for PlatformBundleExport {
    fn export(&mut self, cx: &mut Cx, suggested_name: &str, bytes: Vec<u8>) -> Result<(), String> {
        cx.download_file(suggested_name, bytes, BUNDLE_MIME_TYPE);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::bundle_envelope::split_bundle;

    fn bundle() -> SourceBundle {
        SourceBundle::try_from_pairs(vec![
            ("shop/order.md".to_owned(), "# Order\n".to_owned()),
            ("shop/customer.md".to_owned(), "# Customer\n".to_owned()),
        ])
        .unwrap()
    }

    #[test]
    fn the_encoded_bundle_decodes_back_to_the_same_documents() {
        let source = bundle();
        let bytes = encode_current_bundle(&source).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(
            split_bundle(&text).unwrap(),
            Some(source.to_pairs()),
            "an exported bundle must read back as the model that produced it",
        );
    }

    #[test]
    fn an_empty_model_reports_why_it_cannot_be_exported() {
        let source = SourceBundle::try_from_pairs(Vec::<(String, String)>::new()).unwrap();
        let error = encode_current_bundle(&source).unwrap_err();
        assert!(error.contains("empty"), "{error}");
    }

    #[derive(Default)]
    struct RecordingAdapter {
        exported: Vec<(String, Vec<u8>)>,
    }

    impl BundleExportAdapter for RecordingAdapter {
        fn export(
            &mut self,
            _cx: &mut Cx,
            suggested_name: &str,
            bytes: Vec<u8>,
        ) -> Result<(), String> {
            self.exported.push((suggested_name.to_owned(), bytes));
            Ok(())
        }
    }

    #[test]
    fn the_adapter_receives_the_suggested_name_and_the_exact_envelope_bytes() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        let source = bundle();
        let mut adapter = RecordingAdapter::default();

        export_bundle(&mut cx, &mut adapter, "Orders Domain", &source).unwrap();

        assert_eq!(adapter.exported.len(), 1);
        let (name, bytes) = &adapter.exported[0];
        assert_eq!(name, "orders-domain.waml");
        // Two encodes pick two nonces, so compare what the bytes mean rather
        // than the bytes themselves.
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert_eq!(
            split_bundle(&text).unwrap(),
            Some(source.to_pairs()),
            "the adapter must receive the whole model as one envelope",
        );
    }

    #[test]
    fn a_title_becomes_a_safe_waml_file_name() {
        assert_eq!(suggested_file_name("Orders Domain"), "orders-domain.waml");
        assert_eq!(suggested_file_name("shop/../etc"), "shop-etc.waml");
        assert_eq!(suggested_file_name("  "), "model.waml");
        assert_eq!(suggested_file_name(""), "model.waml");
        assert_eq!(suggested_file_name("注文"), "model.waml");
    }
}
