//! Incremental reparse cost, measured against document size.
//!
//! The question this answers is narrow and specific: for a *fixed* edit, how
//! does `reparse_markdown` scale with the size of the document the edit lands
//! in? An incremental parser that is genuinely incremental should cost about
//! the same to type one character into a 6 KiB document as into a 200 KiB one.
//! If the curve rises with document size, the "incremental" path still carries
//! an Omega(n) floor and the only thing it buys is a smaller constant.
//!
//! Every scenario also parses the same post-edit text from scratch, so each row
//! carries its own full-parse baseline and an incremental/full ratio. A ratio
//! near 1.0 means the incremental path saved nothing; below 1.0 is the win.
//!
//! Scenarios are reported with the outcome the bridge actually took
//! (`Incremental` or `Full{reason}`). That matters: a scenario that falls back
//! to a full parse has a ratio of ~1.0 *by construction*, and reading it as a
//! measurement of incremental cost would be wrong.
//!
//! Deliberately dependency-free, matching `markdown_parse`: `harness = false`
//! plus `std::time::Instant`. Criterion would be a heavy new dependency for a
//! measurement whose signal (a factor-of-N slope across sizes) is far larger
//! than the sampling noise it would remove.
//!
//! MUST be run in release mode. `plan_window_reparse` keeps a
//! `#[cfg(debug_assertions)]` oracle that full-parses the document on every
//! successful window reparse, so a debug timing is not merely noisy, it
//! measures a different program.
//!
//! Run with: `cargo bench -p waml-syntax --bench markdown_reparse`

use std::{fs, hint::black_box, path::Path, sync::Arc, time::Duration, time::Instant};

use waml_syntax::{
    parse_markdown, reparse_markdown, DocumentRevision, MarkdownDialect, MarkdownReparseOutcome,
    SourceText, TextChange, TextRange, TextSize,
};

/// Real prose Markdown, not synthetic filler: the GFM 0.29 specification, which
/// the crate already vendors as a conformance fixture. It is a genuine
/// long-form document -- frontmatter, nested headings, prose, fenced code,
/// tables, link reference definitions -- which is exactly the shape the
/// incremental path exists to serve.
fn corpus() -> String {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gfm-0.29/spec.txt");
    fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read GFM fixture {}: {error}", fixture.display()))
}

/// Truncate `corpus` to roughly `target` bytes, cutting at the blank line
/// before a heading so the prefix is a well-formed document: no fence left
/// open, no paragraph left severed mid-sentence.
fn prefix(corpus: &str, target: usize) -> String {
    if target >= corpus.len() {
        return corpus.to_owned();
    }
    let cut = corpus[target..]
        .find("\n\n#")
        .map_or(corpus.len(), |at| target + at + 1);
    corpus[..cut].to_owned()
}

fn size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("bench offset fits in a TextSize")
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(size(start), size(end)).expect("bench range is ordered")
}

/// A byte offset inside ordinary prose near the middle of `text`.
///
/// Scans forward from the midpoint for a long line that starts with a letter --
/// a paragraph continuation, not a heading, fence, list marker or table row --
/// and returns an offset a few characters into it. Picking the raw midpoint
/// would land wherever it happened to land, and an edit that lands inside a
/// heading or a fence exercises the fallback guards rather than the reuse path.
fn prose_offset(text: &str) -> usize {
    let midpoint = text.len() / 2;
    let mut at = text[..midpoint].rfind('\n').map_or(0, |at| at + 1);
    while at < text.len() {
        let end = text[at..].find('\n').map_or(text.len(), |to| at + to);
        let line = &text[at..end];
        if at >= midpoint
            && line.len() > 40
            && line.starts_with(|ch: char| ch.is_ascii_alphabetic())
        {
            let offset = at + 10;
            if text.is_char_boundary(offset) {
                return offset;
            }
        }
        at = end + 1;
    }
    midpoint
}

/// The offset of the blank line ending the paragraph that contains `from`.
fn block_boundary(text: &str, from: usize) -> usize {
    text[from..].find("\n\n").map_or(text.len(), |at| from + at)
}

/// A few KiB of real prose lifted out of the corpus, to stand in for a paste.
fn paste_payload(corpus: &str) -> String {
    let start = corpus[corpus.len() / 5..]
        .find("\n\n#")
        .map_or(0, |at| corpus.len() / 5 + at + 2);
    let want = (start + 4096).min(corpus.len());
    let end = corpus[want..]
        .find("\n\n")
        .map_or(corpus.len(), |at| want + at + 1);
    corpus[start..end].to_owned()
}

struct Scenario {
    label: &'static str,
    change: TextChange,
    new_text: String,
}

impl Scenario {
    fn insert(label: &'static str, text: &str, at: usize, payload: &str) -> Self {
        let mut new_text = String::with_capacity(text.len() + payload.len());
        new_text.push_str(&text[..at]);
        new_text.push_str(payload);
        new_text.push_str(&text[at..]);
        Self {
            label,
            change: TextChange {
                old_range: range(at, at),
                replacement: Arc::from(payload),
            },
            new_text,
        }
    }

    fn delete(label: &'static str, text: &str, from: usize, to: usize) -> Self {
        let mut new_text = String::with_capacity(text.len());
        new_text.push_str(&text[..from]);
        new_text.push_str(&text[to..]);
        Self {
            label,
            change: TextChange {
                old_range: range(from, to),
                replacement: Arc::from(""),
            },
            new_text,
        }
    }

    /// An edit that changes nothing, submitted against a *fresh* `SourceText`
    /// allocation. It parses nothing at all, so whatever it costs is pure
    /// bookkeeping: the tree-wide rebase, the reprojection, and the snapshot
    /// rebuild. That number is the floor every real edit also pays.
    fn noop(text: &str) -> Self {
        Self {
            label: "no-op (floor)",
            change: TextChange {
                old_range: range(0, 0),
                replacement: Arc::from(""),
            },
            new_text: text.to_owned(),
        }
    }
}

fn scenarios(text: &str, corpus: &str) -> Vec<Scenario> {
    let mid = prose_offset(text);
    let boundary = block_boundary(text, mid);
    // Delete across the blank line so the two adjacent blocks merge -- the case
    // that forces block-level restructuring rather than a purely inline edit.
    let mut from = boundary.saturating_sub(20);
    while !text.is_char_boundary(from) {
        from -= 1;
    }
    let mut to = (boundary + 20).min(text.len());
    while !text.is_char_boundary(to) {
        to += 1;
    }
    vec![
        Scenario::noop(text),
        Scenario::insert("insert 1 char mid", text, mid, "x"),
        Scenario::insert("insert 1 char end", text, text.len(), "x"),
        Scenario::insert("paste ~4 KiB mid", text, mid, &paste_payload(corpus)),
        Scenario::delete("delete across block", text, from, to),
    ]
}

/// Time `run` repeatedly, returning `(best, mean)` in seconds.
///
/// Adaptive rather than fixed-count: a 6 KiB document reparses in microseconds
/// and a 200 KiB one in milliseconds, and a single round count that gives the
/// small case stable numbers would make the large case take minutes.
fn measure(mut run: impl FnMut()) -> (f64, f64) {
    const BUDGET: Duration = Duration::from_millis(400);
    const WARMUP: u32 = 3;
    const MIN_ROUNDS: u32 = 5;
    const MAX_ROUNDS: u32 = 500;

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

fn outcome_label(outcome: &MarkdownReparseOutcome) -> String {
    match outcome {
        MarkdownReparseOutcome::Incremental { .. } => "incremental".to_owned(),
        MarkdownReparseOutcome::Full { reason } => format!("FULL {reason:?}"),
    }
}

fn baseline(dialect: MarkdownDialect, new_text: &str) -> f64 {
    let (best, _) = measure(|| {
        let text = SourceText::new(new_text).expect("bench source is valid");
        let snapshot =
            parse_markdown(DocumentRevision::INITIAL, text, dialect).expect("bench text parses");
        black_box(snapshot);
    });
    best
}

fn main() {
    const DIALECT: MarkdownDialect = MarkdownDialect::WAML_DEFAULT;

    let corpus = corpus();
    println!(
        "corpus: tests/fixtures/gfm-0.29/spec.txt ({} bytes), dialect WAML_DEFAULT",
        corpus.len()
    );
    if cfg!(debug_assertions) {
        println!(
            "WARNING: debug assertions are on. plan_window_reparse full-parses the \
             document as an oracle in this configuration; these numbers are meaningless. \
             Re-run with `cargo bench`."
        );
    }
    println!();
    println!(
        "{:>8}  {:<20}  {:<34}  {:>10}  {:>10}  {:>10}  {:>7}  {:>9}",
        "bytes", "scenario", "outcome", "best ms", "mean ms", "full ms", "ratio", "us/KiB"
    );
    println!(
        "{:>8}  {:<20}  {:<34}  {:>10}  {:>10}  {:>10}  {:>7}  {:>9}",
        "", "", "", "", "", "", "inc/full", "flat==O(1)"
    );

    // A fixed edit measured at rising document sizes: the slope of `best ms`
    // across rows with the same scenario is the whole point of this bench.
    for target in [6_000, 12_000, 24_000, 48_000, 96_000, usize::MAX] {
        let text = prefix(&corpus, target);
        let source = SourceText::new(text.clone()).expect("bench prefix is valid");
        let previous = parse_markdown(DocumentRevision::INITIAL, source, DIALECT)
            .expect("bench prefix parses");

        for scenario in scenarios(&text, &corpus) {
            let new_source =
                SourceText::new(scenario.new_text.clone()).expect("bench edited source is valid");
            let changes = [scenario.change.clone()];
            let probe = reparse_markdown(
                &previous,
                DocumentRevision::new(1),
                new_source.clone(),
                &changes,
            )
            .expect("bench reparse succeeds");
            let outcome = outcome_label(&probe.outcome);
            drop(probe);

            let mut revision = 1_u64;
            let (best, mean) = measure(|| {
                revision += 1;
                // A fresh SourceText each round, because the production caller
                // hands the bridge a freshly allocated document and the
                // pointer-equality fast paths key off exactly that.
                let text = SourceText::new(scenario.new_text.as_str())
                    .expect("bench edited source is valid");
                let update =
                    reparse_markdown(&previous, DocumentRevision::new(revision), text, &changes)
                        .expect("bench reparse succeeds");
                black_box(update);
            });
            let full = baseline(DIALECT, &scenario.new_text);

            // `us/KiB` is the scaling verdict, and the reason the bench sweeps
            // sizes at all. The edit is the same size in every row, so a truly
            // incremental reparse would show this column *falling* as the
            // document grows. Flat means Theta(n): cost tracks the document,
            // not the edit.
            println!(
                "{:>8}  {:<20}  {:<34}  {:>10.4}  {:>10.4}  {:>10.4}  {:>7.2}  {:>9.1}",
                text.len(),
                scenario.label,
                outcome,
                best * 1000.0,
                mean * 1000.0,
                full * 1000.0,
                best / full,
                best * 1.0e6 / (text.len() as f64 / 1024.0),
            );
        }
        println!();

        if target == usize::MAX {
            break;
        }
    }
}
