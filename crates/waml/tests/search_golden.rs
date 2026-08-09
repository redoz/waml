//! Golden query suite over the `docs/waml/` bundle.
//!
//! Ranking has no compile error when it is wrong -- a tokenizer, extraction,
//! or BM25 change can silently reorder or drop results. This suite pins the
//! current ranking for a fixed query list so any such drift shows up as a
//! diff here, before it ever reaches a surface.
//!
//! Regenerate the golden file after an *intentional* ranking change:
//!     WAML_BLESS=1 cargo test -p waml --test search_golden

use std::fs;
use std::path::{Path, PathBuf};

use waml::analysis::prepare_candidate;
use waml::search::extract::extract_bundle;
use waml::search::{HitTarget, MemSearchIndex, QueryScope, SearchIndex};
use waml::source::SourceBundle;

/// Query set: at least one `kind:` filter (`kind:actor`), one `in:` filter
/// (`in:goals/ scope`), one multi-term query (`sequence lifeline`,
/// `markdown editor`), and one short prefix-ish term (`mvp`). Every term
/// here is verified present in `docs/waml` -- an absent term would just pin
/// "(no results)" forever, which defends nothing.
const QUERIES: &[&str] = &[
    "editor",
    "mvp",
    "goals",
    "bundle",
    "kind:actor",
    "projection",
    "in:goals/ scope",
    "sequence lifeline",
    "markdown editor",
    "search",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("waml crate below workspace root")
        .to_owned()
}

/// Collects every `.md` file under `dir` into `out`, skipping `.waml/`
/// (editor state written on open, not model content -- mirrors
/// `waml-cli::io::collect_md`'s dot-directory skip). Walks in sorted order
/// so the resulting bundle is deterministic without a separate sort pass.
fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("read dir {}: {error}", dir.display()))
        .map(|entry| entry.expect("read dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some(".waml") {
                continue;
            }
            collect_md(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            out.push(path);
        }
    }
}

fn load_docs_waml_bundle() -> Vec<(String, String)> {
    let root = workspace_root().join("docs/waml");
    let mut files = Vec::new();
    collect_md(&root, &mut files);
    files
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(&root)
                .expect("collected file lives under docs/waml")
                .to_string_lossy()
                .replace('\\', "/");
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            (relative, text)
        })
        .collect()
}

fn build_index() -> MemSearchIndex {
    let pairs = load_docs_waml_bundle();
    let source = SourceBundle::try_from_pairs(
        pairs
            .iter()
            .map(|(path, text)| (path.as_str(), text.as_str())),
    )
    .expect("docs/waml is a well-formed bundle");
    let candidate = prepare_candidate(source.clone(), None, 0).expect("docs/waml analyzes cleanly");
    let fields = extract_bundle(&source, candidate.okf(), candidate.uml());
    MemSearchIndex::build(fields)
}

fn target_kind(target: &HitTarget) -> &'static str {
    match target {
        HitTarget::TextSpan { .. } => "text-span",
        HitTarget::ModelElement { .. } => "model-element",
    }
}

/// One line per hit, capped at the top 8 hits per query: `query | rank |
/// group | document | target-kind`. A query that legitimately returns
/// nothing pins `query | (no results)` -- an empty answer is a pinned answer
/// too.
fn render(index: &MemSearchIndex) -> String {
    let mut out = String::new();
    for query in QUERIES {
        let hits = index.query(query, &QueryScope::default());
        if hits.is_empty() {
            out.push_str(&format!("{query} | (no results)\n"));
            continue;
        }
        for (rank, hit) in hits.iter().take(8).enumerate() {
            out.push_str(&format!(
                "{query} | {} | {:?} | {} | {}\n",
                rank + 1,
                hit.group,
                hit.document,
                target_kind(&hit.target)
            ));
        }
    }
    out
}

fn golden_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/search_golden.txt")
}

#[test]
fn ranking_matches_the_golden_file() {
    let index = build_index();
    let actual = render(&index);

    if std::env::var("WAML_BLESS").as_deref() == Ok("1") {
        fs::write(golden_path(), &actual).expect("write golden file");
        return;
    }

    let expected = fs::read_to_string(golden_path()).unwrap_or_else(|error| {
        panic!(
            "read {}: {error}\nrun `WAML_BLESS=1 cargo test -p waml --test search_golden` to generate it",
            golden_path().display()
        )
    });
    assert_eq!(
        actual, expected,
        "search ranking drifted from the golden file -- if intentional, regenerate with \
         `WAML_BLESS=1 cargo test -p waml --test search_golden`"
    );
}

#[test]
fn ranking_is_deterministic_across_repeated_runs() {
    let index = build_index();
    assert_eq!(render(&index), render(&index));
}
