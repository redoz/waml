//! Assemble a self-contained WAML site from the embedded web editor.
//!
//! A site is the built editor plus one boot source: either a `bundle.waml`
//! sitting next to it (`waml export site DIR`) or a live model server (the
//! future `waml serve`). The two differ by exactly two things -- whether a
//! bundle file is written, and what `waml-boot.txt` names -- so they share one
//! assembler rather than growing two writers that drift.
//!
//! The whole site is built in memory before anything is written, so a failure
//! part-way through cannot leave half a site on disk.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::Path;

use crate::web_artifact::EmbeddedAsset;

/// The file a site declares its boot source in, read by the editor at startup.
///
/// It holds a query string (`?bundle=…` or `?api=…`) because that is the
/// grammar the editor already parses for URLs. Writing it beside the editor,
/// rather than pushing it into the address bar, keeps a visitor's URL clean.
/// Must match `BOOT_CONFIG_FILE` in `crates/waml-editor/src/browser_boot.rs`.
pub(crate) const BOOT_CONFIG_FILE: &str = "waml-boot.txt";

/// Where an assembled site gets its model from.
#[derive(Debug, Clone, PartialEq, Eq)]
// `Api` is built by the future `waml serve`; only tests construct it today.
#[allow(dead_code)]
pub(crate) enum SiteSource {
    /// A `bundle.waml` written beside the editor, holding these bytes.
    Static(Vec<u8>),
    /// A model server mounted at `/api` on the same origin.
    Api,
}

/// The name a static site's bundle is written under, and boots from.
pub(crate) const BUNDLE_FILE: &str = "bundle.waml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SiteError {
    DuplicatePath(String),
    UnsafePath(String),
    MissingIndex,
    Decompress { path: String, message: String },
}

impl fmt::Display for SiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePath(path) => {
                write!(f, "the embedded web editor lists {path:?} twice")
            }
            Self::UnsafePath(path) => write!(
                f,
                "the embedded web editor carries {path:?}, which is not a safe relative path"
            ),
            Self::MissingIndex => {
                f.write_str("the embedded web editor has no index.html to boot from")
            }
            Self::Decompress { path, message } => {
                write!(f, "cannot decompress the embedded {path}: {message}")
            }
        }
    }
}

impl std::error::Error for SiteError {}

/// Largest decompressed size accepted for one embedded asset.
///
/// The compressed bytes are ours, not user input, so this is a corruption
/// guard rather than a security boundary -- generous enough for the wasm
/// (tens of MB) and finite enough that a malformed stream cannot fill a disk.
const MAX_ASSET_BYTES: u64 = 256 * 1024 * 1024;

/// Build the complete site as `path -> bytes`, ready to write.
pub(crate) fn assemble_site(
    artifact: &[EmbeddedAsset],
    source: SiteSource,
) -> Result<BTreeMap<String, Vec<u8>>, SiteError> {
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for asset in artifact {
        if !is_safe_relative_path(asset.path) {
            return Err(SiteError::UnsafePath(asset.path.to_string()));
        }
        let bytes = decompress(asset)?;
        if files.insert(asset.path.to_string(), bytes).is_some() {
            return Err(SiteError::DuplicatePath(asset.path.to_string()));
        }
    }

    if !files.contains_key("index.html") {
        return Err(SiteError::MissingIndex);
    }
    let boot_query = match &source {
        SiteSource::Static(_) => format!("?bundle={BUNDLE_FILE}"),
        SiteSource::Api => "?api=/api".to_string(),
    };
    files.insert(BOOT_CONFIG_FILE.to_string(), boot_query.into_bytes());

    if let SiteSource::Static(bundle) = source {
        files.insert(BUNDLE_FILE.to_string(), bundle);
    }
    Ok(files)
}

/// Write an assembled site into `out`.
///
/// Without `force` the directory must be missing or empty, so an export can
/// never quietly mix into someone's unrelated files. With `force` only the
/// paths the site itself owns are replaced: the alternative -- deleting the
/// output tree -- would make a mistyped `--out` destructive.
pub(crate) fn write_site(
    out: &Path,
    files: &BTreeMap<String, Vec<u8>>,
    force: bool,
) -> std::io::Result<()> {
    if !force && !is_missing_or_empty(out)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "{} is not empty. Pass --force to overwrite the site files in it.",
                out.display()
            ),
        ));
    }
    for (path, bytes) in files {
        let target = out.join(path);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, bytes)?;
    }
    Ok(())
}

fn is_missing_or_empty(out: &Path) -> std::io::Result<bool> {
    match fs::read_dir(out) {
        Ok(mut entries) => Ok(entries.next().is_none()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

fn decompress(asset: &EmbeddedAsset) -> Result<Vec<u8>, SiteError> {
    let mut out = Vec::new();
    brotli::Decompressor::new(asset.brotli, 4096)
        .take(MAX_ASSET_BYTES)
        .read_to_end(&mut out)
        .map_err(|error| SiteError::Decompress {
            path: asset.path.to_string(),
            message: error.to_string(),
        })?;
    Ok(out)
}

/// The same safety rule the build script applies to the manifest, re-checked
/// here: what is written to disk is validated where the writing happens.
fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compress a fixture the way `scripts/package-web-artifact.mjs` does, so
    /// tests exercise the real decoder rather than a stub.
    fn brotli_stored(bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut input = bytes;
        brotli::BrotliCompress(
            &mut input,
            &mut out,
            &brotli::enc::BrotliEncoderParams::default(),
        )
        .unwrap();
        out
    }

    fn asset(path: &'static str, body: &str) -> (String, Vec<u8>) {
        (path.to_string(), brotli_stored(body.as_bytes()))
    }

    /// `EmbeddedAsset` holds `&'static` slices, so tests leak their fixtures.
    fn artifact(parts: Vec<(String, Vec<u8>)>) -> Vec<EmbeddedAsset> {
        parts
            .into_iter()
            .map(|(path, brotli)| EmbeddedAsset {
                path: Box::leak(path.into_boxed_str()),
                brotli: Box::leak(brotli.into_boxed_slice()),
            })
            .collect()
    }

    fn index_html() -> String {
        "<!doctype html><title>waml</title>".to_string()
    }

    #[test]
    fn a_static_site_carries_the_editor_the_bundle_and_a_bundle_boot_config() {
        let artifact = artifact(vec![
            asset("index.html", &index_html()),
            asset("waml-editor.wasm", "wasm bytes"),
        ]);

        let site = assemble_site(&artifact, SiteSource::Static(b"bundle bytes".to_vec())).unwrap();

        assert_eq!(site["waml-editor.wasm"], b"wasm bytes");
        assert_eq!(site[BUNDLE_FILE], b"bundle bytes");
        assert_eq!(site[BOOT_CONFIG_FILE], b"?bundle=bundle.waml");
        // The address bar is the visitor's; the site says nothing in it.
        assert_eq!(
            String::from_utf8(site["index.html"].clone()).unwrap(),
            index_html()
        );
    }

    #[test]
    fn an_api_site_has_no_bundle_and_boots_from_the_server() {
        let artifact = artifact(vec![asset("index.html", &index_html())]);

        let site = assemble_site(&artifact, SiteSource::Api).unwrap();

        assert!(!site.contains_key(BUNDLE_FILE));
        assert_eq!(site[BOOT_CONFIG_FILE], b"?api=/api");
    }

    #[test]
    fn a_duplicate_path_is_rejected() {
        let artifact = artifact(vec![
            asset("index.html", &index_html()),
            asset("index.html", &index_html()),
        ]);

        assert_eq!(
            assemble_site(&artifact, SiteSource::Api).unwrap_err(),
            SiteError::DuplicatePath("index.html".to_string())
        );
    }

    #[test]
    fn an_escaping_path_is_rejected() {
        let artifact = artifact(vec![asset("../evil.js", "boom")]);

        assert_eq!(
            assemble_site(&artifact, SiteSource::Api).unwrap_err(),
            SiteError::UnsafePath("../evil.js".to_string())
        );
    }

    #[test]
    fn an_artifact_without_an_entry_point_is_rejected() {
        let artifact = artifact(vec![asset("waml-editor.wasm", "wasm bytes")]);

        assert_eq!(
            assemble_site(&artifact, SiteSource::Api).unwrap_err(),
            SiteError::MissingIndex
        );
    }

    fn site() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("index.html".to_string(), b"<!doctype html>".to_vec()),
            ("nested/app.js".to_string(), b"app".to_vec()),
        ])
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("waml-site-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn an_absent_output_directory_is_created() {
        let out = temp_dir("absent");

        write_site(&out, &site(), false).unwrap();

        assert_eq!(
            fs::read(out.join("index.html")).unwrap(),
            b"<!doctype html>"
        );
        assert_eq!(fs::read(out.join("nested/app.js")).unwrap(), b"app");
        fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn an_empty_output_directory_is_accepted() {
        let out = temp_dir("empty");
        fs::create_dir_all(&out).unwrap();

        write_site(&out, &site(), false).unwrap();

        assert!(out.join("index.html").is_file());
        fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn a_non_empty_output_directory_is_refused_without_force() {
        let out = temp_dir("occupied");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("notes.txt"), b"mine").unwrap();

        let error = write_site(&out, &site(), false).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert!(error.to_string().contains("--force"), "{error}");
        assert!(!out.join("index.html").exists());
        fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn force_replaces_site_files_and_leaves_the_rest_alone() {
        let out = temp_dir("forced");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("notes.txt"), b"mine").unwrap();
        fs::write(out.join("index.html"), b"stale").unwrap();

        write_site(&out, &site(), true).unwrap();

        assert_eq!(
            fs::read(out.join("index.html")).unwrap(),
            b"<!doctype html>"
        );
        assert_eq!(fs::read(out.join("notes.txt")).unwrap(), b"mine");
        fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn a_corrupt_asset_names_itself() {
        let artifact = artifact(vec![("index.html".to_string(), vec![0xff, 0xff, 0xff])]);

        let error = assemble_site(&artifact, SiteSource::Api).unwrap_err();

        assert!(
            matches!(&error, SiteError::Decompress { path, .. } if path == "index.html"),
            "{error}"
        );
    }
}
