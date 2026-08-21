//! Per-edit semantic reanalysis cost, measured against *bundle* size.
//!
//! The question this answers: when one character is typed into one document of
//! a bundle holding N documents, how much of `prepare_candidate` is
//! proportional to N rather than to the document that was edited?
//!
//! A pipeline that reuses what the edit cannot have touched would cost about
//! the same to type into a 10-document bundle as into a 500-document one. If
//! the curve rises with N, then the ceiling audit finding A14 names is real and
//! this bench says where it sits: at what bundle size a keystroke stops being
//! instant (~16 ms to hold 60 fps, ~100 ms before it feels laggy).
//!
//! Every size also runs a cold analysis of the same bundle from scratch
//! (`previous: None`), so each row carries its own from-scratch baseline and a
//! reuse ratio. A ratio near 1.0 means the reuse machinery saved nothing.
//!
//! The `no-op (floor)` scenario submits an edit that changes no bytes, against
//! a freshly allocated `SourceText` for the target document. Nothing is
//! reparsed and no semantics change, so whatever it costs is pure per-edit
//! bookkeeping — the floor that every real keystroke also pays.
//!
//! Deliberately dependency-free, matching `waml-syntax`'s `markdown_reparse`:
//! `harness = false` plus `std::time::Instant`. Criterion would be a heavy new
//! dependency for a measurement whose signal (a factor-of-N slope) is far
//! larger than the sampling noise it would remove.
//!
//! MUST be run in release mode; a debug timing measures a different program.
//!
//! Run with: `cargo bench -p waml --bench bundle_reanalysis`

use std::{
    collections::BTreeMap,
    fs,
    hint::black_box,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use waml::{
    analysis::{prepare_candidate, PreparedCandidate, PreviousAnalyses},
    edit::apply_exact_source_edit,
    source::{BundlePath, SourceBundle},
};
use waml_syntax::{SourceText, TextChange, TextRange, TextSize};

/// Real multi-document WAML, not synthetic filler: this repository's own
/// architecture bundle. Every document carries frontmatter, a title heading,
/// and UML islands (`## Attributes`, `## Relationships`, `## Notes`) with
/// relative cross-document links — the exact shape the analysis pipeline
/// exists to serve.
const CORPUS: &str = "../../docs/waml";

fn corpus() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(CORPUS);
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(
        !files.is_empty(),
        "bench corpus {} holds no Markdown",
        root.display()
    );
    files
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) => panic!("read bench corpus {}: {error}", dir.display()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // Skip dot-directories: `.waml/` is bundle metadata, not authored
        // content, and `BundlePath` would reject the leading dot anyway.
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect(root, &path, out);
        } else if path.extension().is_some_and(|ext| ext == "md") {
            let relative = relative_slug(root, &path);
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            // CRLF checkouts would make byte offsets differ between machines;
            // the analysis pipeline handles both, but the bench should not.
            out.push((relative, text.replace("\r\n", "\n")));
        }
    }
}

fn relative_slug(root: &Path, path: &Path) -> String {
    let relative: PathBuf = path
        .strip_prefix(root)
        .unwrap_or_else(|_| panic!("{} is under {}", path.display(), root.display()))
        .to_path_buf();
    relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// A bundle of exactly `documents` documents built from the corpus.
///
/// Beyond the corpus's own size the corpus is replicated, each replica under
/// its own top-level directory. Links inside the corpus are relative
/// (`./sibling.md`), so a replica resolves against itself exactly as the
/// original does: the replicated bundle is N documents of real content with
/// real intra-bundle reference structure, not N copies fighting over one
/// namespace.
fn bundle(corpus: &[(String, String)], documents: usize) -> SourceBundle {
    let mut pairs = Vec::with_capacity(documents);
    let mut replica = 0_usize;
    while pairs.len() < documents {
        for (path, text) in corpus {
            if pairs.len() == documents {
                break;
            }
            pairs.push((format!("r{replica}/{path}"), text.clone()));
        }
        replica += 1;
    }
    SourceBundle::try_from_pairs(pairs).expect("bench corpus paths are valid bundle paths")
}

fn size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("bench offset fits in a TextSize")
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(size(start), size(end)).expect("bench range is ordered")
}

/// The end offset of the first member line (`- something`) under an
/// `## Attributes` or `## Relationships` heading: a byte inside a real UML
/// island, not merely inside the Markdown body.
///
/// This is the expensive-but-ordinary edit. It invalidates the island tree of
/// the document it lands in, which is exactly the edit the island-reuse
/// machinery exists to *contain* to that one document. Typing into prose
/// instead would let every island in the bundle be reused by pointer, which
/// flatters the pipeline rather than testing it.
fn island_offset(text: &str) -> Option<usize> {
    let heading = ["## Attributes\n", "## Relationships\n"]
        .iter()
        .filter_map(|heading| text.find(heading).map(|at| at + heading.len()))
        .min()?;
    let mut at = heading;
    while at < text.len() {
        let end = text[at..].find('\n').map_or(text.len(), |to| at + to);
        let line = &text[at..end];
        if line.starts_with("- ") && end > at + 4 {
            return Some(end);
        }
        // The section is over at the first non-blank line that is not a member.
        if !line.is_empty() {
            return None;
        }
        at = end + 1;
    }
    None
}

struct Scenario {
    label: &'static str,
    changes: Vec<TextChange>,
    new_text: String,
}

impl Scenario {
    /// An edit that changes no bytes, submitted as a *fresh* `SourceText`.
    /// Nothing reparses and no semantics move, so its cost is the pure
    /// per-edit floor.
    fn noop(text: &str) -> Self {
        Self {
            label: "no-op (floor)",
            changes: vec![TextChange {
                old_range: range(0, 0),
                replacement: Arc::from(""),
            }],
            new_text: text.to_owned(),
        }
    }

    fn insert(label: &'static str, text: &str, at: usize, payload: &str) -> Self {
        let mut new_text = String::with_capacity(text.len() + payload.len());
        new_text.push_str(&text[..at]);
        new_text.push_str(payload);
        new_text.push_str(&text[at..]);
        Self {
            label,
            changes: vec![TextChange {
                old_range: range(at, at),
                replacement: Arc::from(payload),
            }],
            new_text,
        }
    }
}

/// Time `run` repeatedly, returning `(best, mean)` in seconds.
///
/// Adaptive rather than fixed-count: a 10-document bundle reanalyses in
/// microseconds and a 500-document one in tens of milliseconds, and one round
/// count that gives the small case stable numbers would make the large case
/// take minutes.
fn measure(mut run: impl FnMut()) -> (f64, f64) {
    const BUDGET: Duration = Duration::from_millis(600);
    const WARMUP: u32 = 2;
    const MIN_ROUNDS: u32 = 3;
    const MAX_ROUNDS: u32 = 200;

    for _ in 0..WARMUP {
        run();
    }
    let mut best = f64::MAX;
    let mut total = 0.0_f64;
    let mut rounds = 0_u32;
    let started = Instant::now();
    while rounds < MIN_ROUNDS || (started.elapsed() < BUDGET && rounds < MAX_ROUNDS) {
        let round = Instant::now();
        run();
        let elapsed = round.elapsed().as_secs_f64();
        best = best.min(elapsed);
        total += elapsed;
        rounds += 1;
    }
    (best, total / f64::from(rounds))
}

fn analyse(source: SourceBundle, revision: u64) -> PreparedCandidate {
    prepare_candidate(source, None, revision).expect("bench bundle analyses")
}

/// Pick the document an edit lands in: the middle document that actually holds
/// a UML island, so the edit exercises the semantic pipeline and the answer is
/// not an artefact of the first or last document in the bundle.
fn target(source: &SourceBundle) -> (BundlePath, usize) {
    let candidates: Vec<_> = source
        .documents()
        .iter()
        .filter_map(|document| {
            island_offset(document.text()).map(|at| (document.path().clone(), at))
        })
        .collect();
    assert!(
        !candidates.is_empty(),
        "bench corpus holds no document with a UML island"
    );
    candidates[candidates.len() / 2].clone()
}

fn main() {
    let corpus = corpus();
    let corpus_bytes: usize = corpus.iter().map(|(_, text)| text.len()).sum();
    println!(
        "corpus: {CORPUS} ({} documents, {corpus_bytes} bytes); larger bundles replicate it \
         under r0/, r1/, ...",
        corpus.len()
    );
    if cfg!(debug_assertions) {
        println!(
            "WARNING: debug assertions are on; these numbers are meaningless. \
             Re-run with `cargo bench`."
        );
    }
    println!();
    println!(
        "{:>6}  {:>9}  {:<18}  {:>10}  {:>10}  {:>10}  {:>8}  {:>9}",
        "docs", "bytes", "scenario", "best ms", "mean ms", "cold ms", "edit/", "us/doc"
    );
    println!(
        "{:>6}  {:>9}  {:<18}  {:>10}  {:>10}  {:>10}  {:>8}  {:>9}",
        "", "", "", "", "", "", "cold", "flat==O(1)"
    );

    for documents in [10_usize, 50, 200, 500] {
        let source = bundle(&corpus, documents);
        let bytes: usize = source
            .documents()
            .iter()
            .map(|document| document.text().len())
            .sum();
        let (path, at) = target(&source);
        let baseline = analyse(source.clone(), 1);
        let text = source
            .document(&path)
            .expect("target document is in the bundle")
            .text()
            .to_owned();

        let scenarios = vec![
            Scenario::noop(&text),
            Scenario::insert("insert 1 char", &text, at, "x"),
        ];

        // The cold baseline: analysing the same bundle with no previous
        // analysis to reuse. Measured once per size, shared by every scenario
        // row, because it does not depend on the edit.
        let (cold, _) = measure(|| {
            black_box(analyse(source.clone(), 1));
        });

        for scenario in &scenarios {
            let accepted = baseline
                .okf()
                .catalog
                .id_for_path(&path)
                .and_then(|id| baseline.okf().catalog.document(id))
                .expect("target document is in the catalog")
                .text()
                .clone();

            let mut revision = 1_u64;
            let (best, mean) = measure(|| {
                revision += 1;
                // A fresh `SourceText` each round, because the production
                // caller hands the pipeline a freshly allocated document and
                // every reuse fast path keys off exactly that pointer.
                let replacement =
                    SourceText::new(scenario.new_text.as_str()).expect("bench text is valid");
                let edited = apply_exact_source_edit(
                    &source,
                    &path,
                    &accepted,
                    &scenario.changes,
                    replacement,
                )
                .expect("bench edit applies")
                .source;
                let candidate = prepare_candidate(
                    edited,
                    Some(PreviousAnalyses {
                        okf: baseline.okf(),
                        uml: baseline.uml(),
                    }),
                    revision,
                )
                .expect("bench candidate analyses");
                black_box(candidate);
            });

            // `us/doc` is the scaling verdict, and the reason the bench sweeps
            // bundle sizes at all. The edit is the same in every row, so a
            // pipeline that reuses what the edit cannot have touched would show
            // this column *falling* as the bundle grows. Flat means Theta(N):
            // the cost tracks the bundle, not the edit.
            println!(
                "{:>6}  {:>9}  {:<18}  {:>10.3}  {:>10.3}  {:>10.3}  {:>8.2}  {:>9.1}",
                documents,
                bytes,
                scenario.label,
                best * 1000.0,
                mean * 1000.0,
                cold * 1000.0,
                best / cold,
                best * 1.0e6 / documents as f64,
            );
        }
        println!();
    }

    // What the pipeline actually reuses, counted rather than timed: how many
    // documents keep their Markdown snapshot and their UML island trees across
    // a one-character edit. If reuse were the whole story these would be
    // N-1 out of N, and the timings above would be flat.
    println!("reuse census (one-character edit inside a UML island, 200 documents)");
    let source = bundle(&corpus, 200);
    let (path, at) = target(&source);
    let baseline = analyse(source.clone(), 1);
    let text = source
        .document(&path)
        .expect("target document is in the bundle")
        .text()
        .to_owned();
    println!("  edited document:           {path}");
    let scenario = Scenario::insert("insert 1 char", &text, at, "x");
    let accepted = baseline
        .okf()
        .catalog
        .id_for_path(&path)
        .and_then(|id| baseline.okf().catalog.document(id))
        .expect("target document is in the catalog")
        .text()
        .clone();
    let replacement = SourceText::new(scenario.new_text.as_str()).expect("bench text is valid");
    let edited = apply_exact_source_edit(&source, &path, &accepted, &scenario.changes, replacement)
        .expect("bench edit applies")
        .source;
    let candidate = prepare_candidate(
        edited,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    )
    .expect("bench candidate analyses");

    let mut markdown_reused = 0_usize;
    let mut markdown_total = 0_usize;
    let mut island_reused = 0_usize;
    let mut island_total = 0_usize;
    for document in source.documents() {
        let Some(id) = candidate.okf().catalog.id_for_path(document.path()) else {
            continue;
        };
        markdown_total += 1;
        if let (Some(before), Some(after)) = (
            baseline.okf().markdown_snapshot(id),
            candidate.okf().markdown_snapshot(id),
        ) {
            if Arc::ptr_eq(before, after) {
                markdown_reused += 1;
            }
        }
        let (Some(before), Some(after)) = (
            baseline.uml().island_syntax.document(id),
            candidate.uml().island_syntax.document(id),
        ) else {
            continue;
        };
        let prior: BTreeMap<_, _> = before
            .values()
            .map(|snapshot| (snapshot.owner(), snapshot.syntax().clone()))
            .collect();
        for snapshot in after.values() {
            island_total += 1;
            if prior
                .get(&snapshot.owner())
                .is_some_and(|tree| Arc::ptr_eq(tree, snapshot.syntax()))
            {
                island_reused += 1;
            }
        }
    }
    println!("  markdown snapshots reused: {markdown_reused}/{markdown_total} documents");
    println!("  UML island trees reused:   {island_reused}/{island_total} islands");
}
