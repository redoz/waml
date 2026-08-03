//! Architecture guard for the Markdown parser seam.
//!
//! The Markdown front end talks to exactly one third-party parser, through
//! exactly one file. Stage 2 replaces that file's contents with a hand-written
//! scanner; this test is what makes that a one-file change instead of an
//! archaeology project.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The single file permitted to reference pulldown-cmark, repo-relative and
/// slash-normalised.
const SEAM: &str = "crates/waml-syntax/src/markdown/scan/pulldown.rs";

/// Every spelling of the dependency worth forbidding: the Rust crate path, and
/// the manifest spelling in case a `cfg`/doc string reaches for it.
const FORBIDDEN: [&str; 2] = ["pulldown_cmark", "pulldown-cmark"];

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is `<root>/crates/waml-syntax`.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest")
        .to_path_buf()
}

fn rust_sources(directory: &Path, into: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    for entry in entries {
        let path = entry.expect("read directory entry").path();
        if path.is_dir() {
            rust_sources(&path, into);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            into.push(path);
        }
    }
}

fn mentions_dependency(source: &str) -> bool {
    FORBIDDEN.iter().any(|forbidden| source.contains(forbidden))
}

#[test]
fn detector_recognises_the_dependency() {
    assert!(mentions_dependency("use pulldown_cmark::Parser;"));
    assert!(mentions_dependency("pulldown-cmark.workspace = true"));
    assert!(mentions_dependency(
        "    let _ = pulldown_cmark::Options::all();"
    ));
}

#[test]
fn detector_ignores_unrelated_sources() {
    assert!(!mentions_dependency("use super::scan::scan_blocks;"));
    assert!(!mentions_dependency("// the scan seam hides the parser"));
    assert!(!mentions_dependency(
        "fn malformed_scan_event_range_recovers() {}"
    ));
}

#[test]
fn the_seam_file_actually_uses_the_dependency() {
    let root = repo_root();
    let seam = root.join(SEAM);
    let source = fs::read_to_string(&seam)
        .unwrap_or_else(|error| panic!("read seam {}: {error}", seam.display()));
    assert!(
        mentions_dependency(&source),
        "{SEAM} must be the pulldown-cmark adapter; if the dependency is gone, \
         delete this guard along with it"
    );
}

#[test]
fn only_the_seam_file_references_the_dependency() {
    let root = repo_root();
    let mut sources = Vec::new();
    rust_sources(&root.join("crates/waml-syntax/src/markdown"), &mut sources);
    assert!(
        sources.len() > 5,
        "the markdown source walk found only {} files, so the guard is not \
         actually scanning anything",
        sources.len()
    );

    let mut violations = Vec::new();
    for path in sources {
        let relative = path
            .strip_prefix(&root)
            .expect("source lives under the repo root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative == SEAM {
            continue;
        }
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        if mentions_dependency(&source) {
            violations.push(relative);
        }
    }

    assert!(
        violations.is_empty(),
        "pulldown-cmark must stay behind the scan seam; found it in:\n  {}\n\
         Route the call through crates/waml-syntax/src/markdown/scan instead.",
        violations.join("\n  ")
    );
}
