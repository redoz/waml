//! Generate the embedded web-artifact table.
//!
//! Under `--features embed-web` this reads a directory produced by
//! `scripts/package-web-artifact.mjs` (named by `WAML_WEB_EMBED_DIR`) and emits
//! `include_bytes!` entries for every compressed asset. Without the feature it
//! emits an empty table plus the error text that tells a builder how to get a
//! real one, so the crate always compiles with no WASM toolchain present.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=WAML_WEB_EMBED_DIR");
    println!("cargo:rerun-if-changed=build.rs");

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let generated = out.join("web_artifact.rs");

    if env::var_os("CARGO_FEATURE_EMBED_WEB").is_none() {
        fs::write(
            &generated,
            "pub(crate) const EMBEDDED: &[EmbeddedAsset] = &[];\n",
        )
        .expect("write the empty embedded artifact table");
        return;
    }

    // `--all-features` turns embed-web on for every workspace lint and test run,
    // where no packaged artifact exists. That must not fail the build: emit an
    // empty table so the binary still compiles and explains itself at runtime.
    let Some(dir) = env::var_os("WAML_WEB_EMBED_DIR") else {
        println!(
            "cargo:warning=the embed-web feature is enabled but WAML_WEB_EMBED_DIR is not set; \
             building without an embedded web editor. Run \
             `node scripts/package-web-artifact.mjs <artifact-dir> <embed-dir>` and point \
             WAML_WEB_EMBED_DIR at <embed-dir> to embed one."
        );
        fs::write(
            &generated,
            "pub(crate) const EMBEDDED: &[EmbeddedAsset] = &[];\n",
        )
        .expect("write the empty embedded artifact table");
        return;
    };
    // A build script runs with the CRATE directory as its cwd, so a relative
    // `WAML_WEB_EMBED_DIR` -- which is what a workspace-root command line and
    // the Pages workflow both naturally pass -- would resolve under
    // crates/waml-cli and be missing. Resolve it against the workspace root
    // instead, which is where the packager writes.
    let dir = PathBuf::from(dir);
    let dir = if dir.is_absolute() {
        dir
    } else {
        workspace_root().join(dir)
    };
    let manifest = dir.join("manifest.txt");
    println!("cargo:rerun-if-changed={}", manifest.display());

    let text = fs::read_to_string(&manifest).unwrap_or_else(|error| {
        panic!(
            "cannot read the web artifact manifest {}: {error}. \
             Re-run scripts/package-web-artifact.mjs to produce it.",
            manifest.display()
        )
    });

    let mut entries = String::from("pub(crate) const EMBEDDED: &[EmbeddedAsset] = &[\n");
    let mut count = 0usize;
    for (line, row) in text.lines().enumerate() {
        if row.is_empty() {
            continue;
        }
        let Some((path, file)) = row.split_once('\t') else {
            panic!(
                "{}:{}: manifest rows must be `path<TAB>compressed-file`",
                manifest.display(),
                line + 1
            );
        };
        validate_path(path, &manifest, line + 1);
        if file.contains(['/', '\\']) || file.is_empty() {
            panic!(
                "{}:{}: the compressed file name {file:?} must be a plain file name",
                manifest.display(),
                line + 1
            );
        }
        let compressed = dir.join(file);
        if !compressed.is_file() {
            panic!(
                "{}:{}: the manifest names {} but that file is missing",
                manifest.display(),
                line + 1,
                compressed.display()
            );
        }
        println!("cargo:rerun-if-changed={}", compressed.display());
        entries.push_str(&format!(
            "    EmbeddedAsset {{ path: {path:?}, brotli: include_bytes!({:?}) }},\n",
            compressed.display().to_string()
        ));
        count += 1;
    }
    entries.push_str("];\n");

    if count == 0 {
        panic!(
            "the web artifact manifest {} lists no files",
            manifest.display()
        );
    }

    fs::write(&generated, entries).expect("write the embedded artifact table");
}

/// `crates/waml-cli` -> the workspace root two levels up.
fn workspace_root() -> PathBuf {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("crates/waml-cli sits two levels under the workspace root")
}

fn validate_path(path: &str, manifest: &Path, line: usize) {
    let invalid = path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..");
    if invalid {
        panic!(
            "{}:{line}: the artifact path {path:?} is not a safe relative path",
            manifest.display()
        );
    }
}
