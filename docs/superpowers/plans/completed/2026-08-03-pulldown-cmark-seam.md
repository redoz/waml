# pulldown-cmark Seam (Stage 0 + Stage 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a parse-time performance baseline and confine every `pulldown_cmark` reference in the tree to exactly one file, behind waml's own block-scan event vocabulary, without changing a single byte of parser output.

**Architecture:** A new `crates/waml-syntax/src/markdown/scan/` module defines waml's own parser-interface vocabulary (`ScanEvent`, `ScanTag`, `ScanTagKind`, `ScanAlignment`, `ScanProfile`, `BlockScan`) in `scan/mod.rs`, and a single adapter in `scan/pulldown.rs` implements that vocabulary on top of pulldown-cmark. The four current consumers (`block.rs`, `mod.rs`, `gfm.rs`, `inline.rs`) are moved onto the seam one at a time, each move a standalone green commit. A guard test then locks the invariant: `pulldown_cmark` may appear in `scan/pulldown.rs` and nowhere else under `src/markdown/`. Behavior is byte-identical throughout; the 652-example CommonMark conformance suite and the 24-example GFM suite are the oracle, and they run off waml's *own* green tree (not pulldown's HTML renderer), so they genuinely validate a parser swap.

**Motive (stated by the user, verbatim in spirit):** dependency independence **on principle**, while **staying CommonMark-conformant**. Licensing is explicitly *not* a motive — pulldown-cmark is MIT and perfectly compatible with this repo's MPL-2.0.

**Tech Stack:** Rust (edition/rust-version from the workspace), pulldown-cmark 0.13.4 (`default-features = false`), `serde` + `serde_json` (existing dev-dependencies, used by the fixture loaders and the new bench), `std::time::Instant` + `std::hint::black_box` for the benchmark harness. No new dependencies of any kind.

## Global Constraints

- **Full gate green before every commit.** Every task ends with all four of these at exit 0:
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets` (must be **0 warnings** — the gate runs `-D warnings`, which promotes `dead_code` to a hard error, so never land an item, field, or enum variant that nothing reads)
  - `cargo fmt --all -- --check`
  - `cd editors/vscode && pnpm build && pnpm lint && pnpm test`
- **`commonmark_conformance` and `gfm_extension_conformance` must stay green with NO skip list at every single commit.** A conformance regression is a hard stop. Never add a skip, an `#[ignore]`, or an allow-list.
- **Behavior must be byte-identical through all of Stage 1.** This is a pure refactor: the conformance HTML and the green trees must not change. If any test output changes, the change is wrong.
- **Do NOT commit `editors/vscode/package-lock.json`.** The repo has none by design. `npm install` creates one; delete it before `git add -A`. (`npm ci` fails — there is no lockfile.)
- **Never commit `proptest-regressions/`.**
- **Commit messages:** conventional-commit subject + body. **No Claude co-author trailer** — the user considers it advertising.
- **Do not add any new third-party dependency**, production or dev. The whole point of this work is dependency independence.
- **Several files in this repo are CRLF.** Use the Edit tool for edits. Python string replacement with `\n` literals silently matches nothing.
- **Work only inside the worktree** `C:/dev/waml/.worktrees/markdown-hide-syntax` (verify with `git rev-parse --show-toplevel`). **Never edit the main checkout at `C:/dev/waml`.** Use absolute paths in the Edit tool — a main-root path silently edits main and "passes".
- Stages 2, 3, and 4 are **out of scope**. Do not start a hand-written scanner, do not touch `Cargo.toml`'s `pulldown-cmark` entry, do not relax `crates/waml/tests/no_legacy_authority.rs`'s pulldown exemption.

---

## File Structure

**Created:**
- `crates/waml-syntax/benches/markdown_parse.rs` — Stage 0 baseline. `harness = false`, plain `std::time::Instant`, parses the whole CommonMark spec corpus under two dialects and prints best/mean/throughput. Native-only by nature (`Instant` is fine in a bench; note that `SystemTime::now()` panics on wasm in this project, and a bench never runs on wasm anyway).
- `crates/waml-syntax/src/markdown/scan/mod.rs` — waml's own block-scan vocabulary and the crate-internal entry points. Contains **no** pulldown reference of any kind.
- `crates/waml-syntax/src/markdown/scan/pulldown.rs` — the **only** file in the tree permitted to `use pulldown_cmark`. Option profiles, tag mapping, the balance stack, and the two inline helpers.
- `crates/waml-syntax/tests/scan_seam.rs` — architecture guard: walks `crates/waml-syntax/src/markdown/**/*.rs` and asserts `pulldown_cmark` appears in exactly one file. Self-tests its own detector against synthetic sources, in the style of `crates/waml/tests/no_legacy_authority.rs`.

**Modified:**
- `crates/waml-syntax/Cargo.toml` — adds a `[[bench]]` section. No dependency change.
- `crates/waml-syntax/tests/markdown_conformance.rs` — adds a corpus-size guard so the oracle cannot silently shrink.
- `crates/waml-syntax/src/markdown/mod.rs` — declares `scan`; `shell_map` moves onto `scan_blocks(.., ScanProfile::Shell)`; `structure_options`/`protects`/`opaque_container`/`protects_end`/`heading_level` are rewritten or deleted.
- `crates/waml-syntax/src/markdown/block.rs` — moves onto `scan_blocks(.., ScanProfile::Tree)`; `pulldown_options` moves into the adapter; `start_kind`/`end_kind`/`heading_is_setext` are rewritten against the scan vocabulary.
- `crates/waml-syntax/src/markdown/gfm.rs` — `TableAlignment::from_pulldown` becomes `from_scan`.
- `crates/waml-syntax/src/markdown/inline.rs` — `decode_entity` and `is_raw_html` move onto the scan inline helpers.

## Design Decisions (state these; they are the contract Stage 2 must honour)

1. **Two option profiles, because the two consumers genuinely differ.** `block.rs` builds from `Options::empty()` and *inserts* `ENABLE_TABLES` / `ENABLE_STRIKETHROUGH` / `ENABLE_TASKLISTS` per dialect. `mod.rs::shell_map` builds from `Options::all()` and *removes* those same three per dialect. That asymmetry is deliberate (the shell map protects every construct pulldown can see; the tree uses the narrow public profile), so `ScanProfile { Tree, Shell }` is a parameter of the seam, not a hidden default.
2. **Inline-level Start/End tags are filtered out of `scan_blocks`.** Both current consumers fall through inline tags with `_ => {}`. The adapter drops an unmapped `Start` *and its matching `End`* symmetrically via a small open-tag stack, so stack balance is preserved exactly. This shrinks the contract a hand-written Stage 2 scanner has to reproduce.
3. **`End` reports a precise `ScanTagKind`.** pulldown's `TagEnd::CodeBlock` is one variant covering both indented and fenced blocks, and `TagEnd::Heading` carries a level. The adapter's open-tag stack lets `End` report exactly the kind that opened. Neither current consumer needs this (`block.rs` only asks "does this close a block?", `mod.rs` only tests set membership), but it makes the contract unambiguous for Stage 2.
4. **`ScanTagKind::Heading` is fieldless, and the level check on `End` is dropped.** `shell_map` currently asserts `expected == heading_level(level)` on `TagEnd::Heading`. With the adapter's stack, the `End` is guaranteed to be the `End` of the heading that opened, and pulldown never pairs a heading start with a differently-levelled end — the comparison is a tautology. Dropping it is a deliberate, argued simplification, and the conformance suite is the check.
5. **`BlockScan` is eager (a `Vec`), not an iterator.** Tradeoff: one allocation proportional to the event count, and the whole event stream is materialised before the tree builder starts. Accepted because the seam is dramatically simpler (no lifetime plumbing, no borrow dance around `reference_definitions()`, which must be read before the parser is consumed), documents are editor-sized, and Stage 2's hand-written scanner is far easier to write against a `Vec` than against a streaming iterator. Revisit only if the Stage 0 bench shows it matters.
6. **Reference-definition spans are returned unsorted, in the adapter's iteration order.** `block.rs` validates each span *and then* sorts. Preserving that exact order of operations (definitions validated first, then events) matters: validation can raise `BlockBuildError::MalformedEventRange`, which `parse` recovers via `recover_raw_text`. Do not sort inside `scan_blocks`.
7. **Offsets are relative to the `&str` handed to `scan_blocks`.** `block.rs` passes a slice (`&source[event_start..end]`) and re-bases with `event_start + offset`; `mod.rs` passes the whole source and uses offsets as-is. The seam does not re-base for you.
8. **`gfm.rs` must not know pulldown exists.** `TableAlignment::from_pulldown(Alignment)` becomes `TableAlignment::from_scan(ScanAlignment)`.

---

### Task 1: Stage 0 — parse-time benchmark harness and a conformance corpus-size guard

Stage 0 is the safety net. Before anything moves, we need (a) a repeatable parse-time number so a later hand-written scanner can be compared against pulldown, and (b) a guard that the conformance oracle cannot silently shrink — a plan like this fails catastrophically and invisibly if someone trims the fixture instead of fixing the parser.

The corpus counts are **verified facts**, not guesses: `crates/waml-syntax/tests/fixtures/commonmark-0.31.2/spec.json` deserializes to exactly **652** examples, and `gfm_extension_examples()` yields exactly **24** examples from its five named sections (the fixture contains 672 example fences in total; the loader keeps only those inside the five sections).

**Files:**
- Create: `crates/waml-syntax/benches/markdown_parse.rs`
- Modify: `crates/waml-syntax/Cargo.toml`
- Test: `crates/waml-syntax/tests/markdown_conformance.rs`

**Interfaces:**
- Consumes: `waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText}` — the existing public API. Signature: `pub fn parse_markdown(revision: DocumentRevision, text: SourceText, dialect: MarkdownDialect) -> Result<Arc<MarkdownSyntaxSnapshot>, ParseError>`.
- Produces: nothing consumed by later tasks. This task is a pure safety net; every later task simply has to keep it green.

- [ ] **Step 1: Write the failing corpus-size guard**

Add this test at the end of `crates/waml-syntax/tests/markdown_conformance.rs`. It uses the two loaders that already exist in that file (`commonmark_examples()` at line 18 and `gfm_extension_examples()` at line 188) — do not write new loaders.

```rust
/// The conformance suites are the oracle for every parser change. If a fixture
/// is trimmed or a loader silently stops matching, the suites keep passing
/// while covering less. Pin the counts so shrinkage is a test failure.
#[test]
fn conformance_corpus_is_complete() {
    assert_eq!(
        commonmark_examples().len(),
        652,
        "CommonMark 0.31.2 fixture must keep all 652 examples; a smaller corpus \
         means the conformance oracle shrank instead of the parser improving"
    );
    assert_eq!(
        gfm_extension_examples().len(),
        24,
        "GFM 0.29 loader must keep all 24 examples from its five extension \
         sections; a smaller corpus means the oracle shrank"
    );
}
```

- [ ] **Step 2: Run the guard and confirm the counts are right**

Run: `cargo test -p waml-syntax --test markdown_conformance conformance_corpus_is_complete -- --nocapture`
Expected: PASS. If it fails, the printed counts are the truth — fix the constants in the test to the real numbers, do **not** touch the fixtures.

- [ ] **Step 3: Register the bench target**

Edit `crates/waml-syntax/Cargo.toml`, appending after the `[dev-dependencies]` block. `harness = false` means Rust's built-in test harness is not linked and our `fn main` runs directly — that is what lets us avoid criterion/divan entirely.

```toml

[[bench]]
name = "markdown_parse"
harness = false
```

Do not add a dependency. `serde`, `serde_json`, and `proptest` are already dev-dependencies of this crate, and bench targets link dev-dependencies.

- [ ] **Step 4: Write the benchmark**

Create `crates/waml-syntax/benches/markdown_parse.rs`:

```rust
//! Stage 0 parse-time baseline for the Markdown front end.
//!
//! Deliberately dependency-free: `harness = false` plus `std::time::Instant`,
//! because the whole point of the pulldown-cmark removal is fewer third-party
//! crates, and adding criterion or divan to measure that would be absurd.
//!
//! Native-only by construction. Benches never run on wasm, which matters here
//! because this project's wasm target has no clock at all.
//!
//! Run with: `cargo bench -p waml-syntax`

use std::{fs, hint::black_box, path::Path, time::Instant};

use waml_syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText};

/// Only the `markdown` field is needed; `serde` ignores the rest of each entry.
#[derive(serde::Deserialize)]
struct Example {
    markdown: String,
}

fn corpus() -> Vec<String> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/commonmark-0.31.2/spec.json");
    let source = fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read CommonMark fixture {}: {error}", fixture.display()));
    let examples: Vec<Example> =
        serde_json::from_str(&source).expect("deserialize CommonMark fixture");
    examples.into_iter().map(|example| example.markdown).collect()
}

fn parse_corpus(corpus: &[String], dialect: MarkdownDialect) {
    for markdown in corpus {
        let text = SourceText::new(markdown.as_str()).expect("fixture source is valid");
        let snapshot =
            parse_markdown(DocumentRevision::INITIAL, text, dialect).expect("fixture parses");
        black_box(snapshot);
    }
}

fn main() {
    const WARMUP: u32 = 2;
    const ROUNDS: u32 = 10;

    let corpus = corpus();
    let bytes: usize = corpus.iter().map(String::len).sum();
    println!("corpus: {} examples, {bytes} bytes", corpus.len());

    for (label, dialect) in [
        ("commonmark", MarkdownDialect::COMMONMARK_0_31_2),
        ("waml", MarkdownDialect::WAML_DEFAULT),
    ] {
        for _ in 0..WARMUP {
            parse_corpus(&corpus, dialect);
        }
        let mut best = f64::MAX;
        let mut total = 0.0_f64;
        for _ in 0..ROUNDS {
            let started = Instant::now();
            parse_corpus(&corpus, dialect);
            let elapsed = started.elapsed().as_secs_f64();
            best = best.min(elapsed);
            total += elapsed;
        }
        let mean = total / f64::from(ROUNDS);
        let throughput = (bytes as f64 / (1024.0 * 1024.0)) / best;
        println!(
            "{label:<11} best {:>9.3} ms   mean {:>9.3} ms   {throughput:>7.1} MiB/s",
            best * 1000.0,
            mean * 1000.0,
        );
    }
}
```

- [ ] **Step 5: Run the benchmark and record the baseline**

Run: `cargo bench -p waml-syntax`
Expected: two lines of output, one per dialect, each with a non-zero millisecond figure and a plausible MiB/s throughput. Paste the exact output into the commit body — that is the Stage 0 baseline that Stage 2 will be measured against.

- [ ] **Step 6: Run the full gate**

Run, all four, all must exit 0:
```
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd editors/vscode && pnpm build && pnpm lint && pnpm test
```
`--all-targets` is what type-checks and lints the bench, since `cargo test` does not build bench targets by default.

- [ ] **Step 7: Commit**

Delete `editors/vscode/package-lock.json` if `npm` created one, then:

```bash
git add crates/waml-syntax/Cargo.toml crates/waml-syntax/benches/markdown_parse.rs crates/waml-syntax/tests/markdown_conformance.rs
git commit -m "test(waml-syntax): add a parse-time baseline and pin the conformance corpus

Stage 0 of removing the pulldown-cmark dependency. Adds a dependency-free
benchmark (harness = false, std::time::Instant) over the CommonMark 0.31.2
spec corpus so a future hand-written scanner can be compared against the
current parser, and pins the conformance corpus at 652 CommonMark plus 24
GFM examples so the oracle cannot shrink silently."
```

---

### Task 2: Stage 1a — the `scan` module: waml's own block event vocabulary, pulldown-backed

Create the seam with nothing consuming it yet. Because `cargo clippy --all-targets` runs with `-D warnings` and that promotes `dead_code` to a hard error, the module declaration carries a temporary `#[allow(dead_code)]` which **Task 6 removes** once every consumer has moved. There is precedent for this pattern in the crate (`BlockParse` in `block.rs` carries the same attribute).

**Files:**
- Create: `crates/waml-syntax/src/markdown/scan/mod.rs`
- Create: `crates/waml-syntax/src/markdown/scan/pulldown.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs` (module declaration only)
- Test: unit tests inside `crates/waml-syntax/src/markdown/scan/mod.rs`

**Interfaces:**
- Consumes: `crate::MarkdownDialect` and its predicates `tables() -> bool`, `strikethrough() -> bool`, `task_lists() -> bool`.
- Produces, all `pub(crate)`, all reachable as `super::scan::…` from `block.rs` / `inline.rs` / `gfm.rs` and as `scan::…` from `markdown/mod.rs`:
  - `enum ScanProfile { Tree, Shell }` — `Copy`
  - `enum ScanAlignment { None, Left, Center, Right }` — `Copy`
  - `enum ScanTagKind { Paragraph, Heading, BlockQuote, IndentedCodeBlock, FencedCodeBlock, HtmlBlock, List, Item, Table, TableHead, TableRow, TableCell, FootnoteDefinition, DefinitionList, DefinitionListDefinition }` — `Copy`
  - `enum ScanTag { Paragraph, Heading { level: u8 }, BlockQuote, IndentedCodeBlock, FencedCodeBlock, HtmlBlock, List, Item, Table { alignments: Vec<ScanAlignment> }, TableHead, TableRow, TableCell, FootnoteDefinition, DefinitionList, DefinitionListDefinition }`
  - `fn ScanTag::kind(&self) -> ScanTagKind`
  - `enum ScanEvent { Start(ScanTag), End(ScanTagKind), Rule }`
  - `struct BlockScan { pub events: Vec<(ScanEvent, Range<usize>)>, pub reference_definitions: Vec<Range<usize>> }`
  - `fn scan_blocks(source: &str, dialect: MarkdownDialect, profile: ScanProfile) -> BlockScan`
  - `fn scan_text_entities(spelling: &str) -> String`
  - `fn scan_is_inline_html(candidate: &str) -> bool`

Note the deliberate absence of payloads that nothing reads: `List` carries no `ordered` flag and `FootnoteDefinition` no label, because an unread field is a hard `dead_code` error under this repo's gate. Stage 2 can add them when a consumer needs them.

- [ ] **Step 1: Write the vocabulary**

Create `crates/waml-syntax/src/markdown/scan/mod.rs`:

```rust
//! waml's own Markdown block-scan vocabulary.
//!
//! This module names the events the tree builder and the shell mapper need,
//! independently of any third-party parser. Exactly one implementation exists
//! today ([`pulldown`]), and it is the only file in the tree permitted to
//! reference `pulldown_cmark`; `tests/scan_seam.rs` enforces that.
//!
//! Contract notes for any future implementation:
//!
//! * Offsets in [`BlockScan::events`] are byte ranges **relative to the `source`
//!   passed in**. Callers that scan a slice re-base them themselves.
//! * Inline-level constructs are not reported. An implementation that cannot
//!   report a construct must omit its start *and* its end, so the event stream
//!   stays balanced.
//! * [`ScanEvent::End`] names the exact kind that opened, including the
//!   indented/fenced code-block distinction.
//! * [`BlockScan::reference_definitions`] is returned in implementation order,
//!   unsorted. `block.rs` validates each span before sorting, and that order of
//!   operations is load-bearing for its recovery path.

mod pulldown;

use std::ops::Range;

pub(crate) use pulldown::{scan_blocks, scan_is_inline_html, scan_text_entities};

/// Which construct set the scan should recognise.
///
/// The two profiles are genuinely different and both are in use: the tree
/// builder opts *in* to the GFM constructs its dialect enables, while the shell
/// mapper starts from everything the parser can see and opts *out* of the same
/// three. The shell mapper protects more than the tree represents on purpose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanProfile {
    /// The narrow profile used to build the syntax tree.
    Tree,
    /// The wide profile used to map shell structure.
    Shell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanAlignment {
    None,
    Left,
    Center,
    Right,
}

/// A block tag identity with no payload, used for ends and set membership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScanTagKind {
    Paragraph,
    Heading,
    BlockQuote,
    IndentedCodeBlock,
    FencedCodeBlock,
    HtmlBlock,
    List,
    Item,
    Table,
    TableHead,
    TableRow,
    TableCell,
    FootnoteDefinition,
    DefinitionList,
    DefinitionListDefinition,
}

/// A block tag opening, with the payload the consumers actually read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScanTag {
    Paragraph,
    Heading { level: u8 },
    BlockQuote,
    IndentedCodeBlock,
    FencedCodeBlock,
    HtmlBlock,
    List,
    Item,
    Table { alignments: Vec<ScanAlignment> },
    TableHead,
    TableRow,
    TableCell,
    FootnoteDefinition,
    DefinitionList,
    DefinitionListDefinition,
}

impl ScanTag {
    pub(crate) fn kind(&self) -> ScanTagKind {
        match self {
            Self::Paragraph => ScanTagKind::Paragraph,
            Self::Heading { .. } => ScanTagKind::Heading,
            Self::BlockQuote => ScanTagKind::BlockQuote,
            Self::IndentedCodeBlock => ScanTagKind::IndentedCodeBlock,
            Self::FencedCodeBlock => ScanTagKind::FencedCodeBlock,
            Self::HtmlBlock => ScanTagKind::HtmlBlock,
            Self::List => ScanTagKind::List,
            Self::Item => ScanTagKind::Item,
            Self::Table { .. } => ScanTagKind::Table,
            Self::TableHead => ScanTagKind::TableHead,
            Self::TableRow => ScanTagKind::TableRow,
            Self::TableCell => ScanTagKind::TableCell,
            Self::FootnoteDefinition => ScanTagKind::FootnoteDefinition,
            Self::DefinitionList => ScanTagKind::DefinitionList,
            Self::DefinitionListDefinition => ScanTagKind::DefinitionListDefinition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ScanEvent {
    Start(ScanTag),
    End(ScanTagKind),
    Rule,
}

/// One block scan of one source slice.
///
/// Eager rather than streaming: the whole event stream is materialised. That
/// costs one allocation proportional to the event count and buys a seam with no
/// lifetime plumbing, which matters because the reference definitions must be
/// read before the underlying parser is consumed. Documents here are
/// editor-sized; revisit only if `benches/markdown_parse.rs` says otherwise.
#[derive(Debug, Default)]
pub(crate) struct BlockScan {
    pub events: Vec<(ScanEvent, Range<usize>)>,
    pub reference_definitions: Vec<Range<usize>>,
}
```

- [ ] **Step 2: Write the pulldown adapter**

Create `crates/waml-syntax/src/markdown/scan/pulldown.rs`. The two option profiles are moved here verbatim from `block.rs::pulldown_options` and `markdown/mod.rs::structure_options` — do not "tidy" them, the asymmetry is the point.

```rust
//! The one and only place in this tree that knows pulldown-cmark exists.
//!
//! `tests/scan_seam.rs` fails the build if `pulldown_cmark` appears in any
//! other file under `src/markdown/`.

use std::ops::Range;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag};

use crate::MarkdownDialect;

use super::{BlockScan, ScanAlignment, ScanEvent, ScanProfile, ScanTag, ScanTagKind};

/// The tree profile opts *in* to the GFM constructs the dialect enables.
fn tree_options(dialect: MarkdownDialect) -> Options {
    let mut options = Options::empty();
    if dialect.tables() {
        options.insert(Options::ENABLE_TABLES);
    }
    if dialect.strikethrough() {
        options.insert(Options::ENABLE_STRIKETHROUGH);
    }
    if dialect.task_lists() {
        options.insert(Options::ENABLE_TASKLISTS);
    }
    options
}

/// The shell profile starts from everything the parser can see and opts *out*.
///
/// The shell structure contract protects every construct the parser can
/// identify, which is deliberately wider than what the syntax tree represents.
fn shell_options(dialect: MarkdownDialect) -> Options {
    let mut options = Options::all();
    if !dialect.tables() {
        options.remove(Options::ENABLE_TABLES);
    }
    if !dialect.task_lists() {
        options.remove(Options::ENABLE_TASKLISTS);
    }
    if !dialect.strikethrough() {
        options.remove(Options::ENABLE_STRIKETHROUGH);
    }
    options
}

fn options(dialect: MarkdownDialect, profile: ScanProfile) -> Options {
    match profile {
        ScanProfile::Tree => tree_options(dialect),
        ScanProfile::Shell => shell_options(dialect),
    }
}

fn alignment(value: Alignment) -> ScanAlignment {
    match value {
        Alignment::None => ScanAlignment::None,
        Alignment::Left => ScanAlignment::Left,
        Alignment::Center => ScanAlignment::Center,
        Alignment::Right => ScanAlignment::Right,
    }
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Maps an opening tag, or `None` for constructs the scan vocabulary does not
/// report (every inline tag, metadata blocks, definition-list titles, ...).
fn start_tag(tag: Tag<'_>) -> Option<ScanTag> {
    Some(match tag {
        Tag::Paragraph => ScanTag::Paragraph,
        Tag::Heading { level, .. } => ScanTag::Heading {
            level: heading_level(level),
        },
        Tag::BlockQuote(_) => ScanTag::BlockQuote,
        Tag::CodeBlock(CodeBlockKind::Indented) => ScanTag::IndentedCodeBlock,
        Tag::CodeBlock(CodeBlockKind::Fenced(_)) => ScanTag::FencedCodeBlock,
        Tag::HtmlBlock => ScanTag::HtmlBlock,
        Tag::List(_) => ScanTag::List,
        Tag::Item => ScanTag::Item,
        Tag::Table(alignments) => ScanTag::Table {
            alignments: alignments.into_iter().map(alignment).collect(),
        },
        Tag::TableHead => ScanTag::TableHead,
        Tag::TableRow => ScanTag::TableRow,
        Tag::TableCell => ScanTag::TableCell,
        Tag::FootnoteDefinition(_) => ScanTag::FootnoteDefinition,
        Tag::DefinitionList => ScanTag::DefinitionList,
        Tag::DefinitionListDefinition => ScanTag::DefinitionListDefinition,
        _ => return None,
    })
}

pub(crate) fn scan_blocks(
    source: &str,
    dialect: MarkdownDialect,
    profile: ScanProfile,
) -> BlockScan {
    let parser = Parser::new_ext(source, options(dialect, profile));

    // Must be read before `into_offset_iter` consumes the parser. Order is the
    // parser's own; callers validate before sorting and that order matters.
    let reference_definitions: Vec<Range<usize>> = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| definition.span.clone())
        .collect();

    let mut events = Vec::new();
    // One slot per open tag. `None` marks a construct the vocabulary drops, so
    // its end is dropped too and the stream stays balanced.
    let mut open: Vec<Option<ScanTagKind>> = Vec::new();
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => {
                let mapped = start_tag(tag);
                open.push(mapped.as_ref().map(ScanTag::kind));
                if let Some(tag) = mapped {
                    events.push((ScanEvent::Start(tag), range));
                }
            }
            Event::End(_) => {
                if let Some(Some(kind)) = open.pop() {
                    events.push((ScanEvent::End(kind), range));
                }
            }
            Event::Rule => events.push((ScanEvent::Rule, range)),
            _ => {}
        }
    }

    BlockScan {
        events,
        reference_definitions,
    }
}

/// Concatenates the text the parser decodes from `spelling`, entities resolved.
pub(crate) fn scan_text_entities(spelling: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(spelling) {
        if let Event::Text(value) = event {
            text.push_str(&value);
        }
    }
    text
}

/// Whether `candidate` — angle brackets included — is a raw HTML tag.
pub(crate) fn scan_is_inline_html(candidate: &str) -> bool {
    Parser::new(candidate).any(
        |event| matches!(event, Event::InlineHtml(html) | Event::Html(html) if html.as_ref() == candidate),
    )
}
```

- [ ] **Step 3: Declare the module**

In `crates/waml-syntax/src/markdown/mod.rs`, the module list at lines 3–13 is alphabetical. Insert `scan` between `reference` and `reparse`, with the temporary allow. Use the Edit tool (this file may be CRLF).

Replace:
```rust
pub(crate) mod reference;
pub(crate) mod reparse;
```
with:
```rust
pub(crate) mod reference;
pub(crate) mod reparse;
// Consumers land in the following tasks; the allow is removed once they have.
#[allow(dead_code)]
pub(crate) mod scan;
```

Then run `cargo fmt --all` — rustfmt will not reorder these, so if `scan` ends up after `reparse` alphabetically that is correct as written (r < s).

- [ ] **Step 4: Write the unit tests**

Append to `crates/waml-syntax/src/markdown/scan/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::MarkdownDialect;

    fn kinds(source: &str, profile: ScanProfile) -> Vec<ScanEvent> {
        scan_blocks(source, MarkdownDialect::WAML_DEFAULT, profile)
            .events
            .into_iter()
            .map(|(event, _)| event)
            .collect()
    }

    #[test]
    fn reports_a_paragraph_with_its_byte_range() {
        let scan = scan_blocks(
            "hello\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Tree,
        );
        assert_eq!(
            scan.events,
            vec![
                (ScanEvent::Start(ScanTag::Paragraph), 0..6),
                (ScanEvent::End(ScanTagKind::Paragraph), 0..6),
            ]
        );
    }

    #[test]
    fn inline_constructs_are_dropped_symmetrically() {
        // Emphasis opens and closes inside the paragraph. Neither is reported,
        // and the paragraph's own pair still balances.
        assert_eq!(
            kinds("a *b* c\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::Paragraph),
                ScanEvent::End(ScanTagKind::Paragraph),
            ]
        );
    }

    #[test]
    fn code_block_ends_distinguish_indented_from_fenced() {
        assert_eq!(
            kinds("    code\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::IndentedCodeBlock),
                ScanEvent::End(ScanTagKind::IndentedCodeBlock),
            ]
        );
        assert_eq!(
            kinds("```\ncode\n```\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::FencedCodeBlock),
                ScanEvent::End(ScanTagKind::FencedCodeBlock),
            ]
        );
    }

    #[test]
    fn heading_carries_its_level() {
        assert_eq!(
            kinds("### t\n", ScanProfile::Tree),
            vec![
                ScanEvent::Start(ScanTag::Heading { level: 3 }),
                ScanEvent::End(ScanTagKind::Heading),
            ]
        );
    }

    #[test]
    fn thematic_break_is_a_rule() {
        assert_eq!(kinds("---\n\n", ScanProfile::Tree), vec![ScanEvent::Rule]);
    }

    #[test]
    fn table_alignments_survive_the_seam() {
        let events = kinds("|a|b|c|\n|:-|:-:|-:|\n|1|2|3|\n", ScanProfile::Tree);
        assert_eq!(
            events.first(),
            Some(&ScanEvent::Start(ScanTag::Table {
                alignments: vec![
                    ScanAlignment::Left,
                    ScanAlignment::Center,
                    ScanAlignment::Right,
                ],
            }))
        );
    }

    #[test]
    fn the_shell_profile_sees_footnotes_the_tree_profile_does_not() {
        let source = "[^a]: note\n";
        assert!(kinds(source, ScanProfile::Shell)
            .contains(&ScanEvent::Start(ScanTag::FootnoteDefinition)));
        assert!(!kinds(source, ScanProfile::Tree)
            .contains(&ScanEvent::Start(ScanTag::FootnoteDefinition)));
    }

    #[test]
    fn every_start_has_a_matching_end() {
        let source = "> - a\n> - b\n\n# h\n\n```r\nx\n```\n\npara [l]: x\n\n[l]: /u\n";
        for profile in [ScanProfile::Tree, ScanProfile::Shell] {
            let mut stack = Vec::new();
            for event in kinds(source, profile) {
                match event {
                    ScanEvent::Start(tag) => stack.push(tag.kind()),
                    ScanEvent::End(kind) => {
                        assert_eq!(stack.pop(), Some(kind), "unbalanced end in {profile:?}");
                    }
                    ScanEvent::Rule => {}
                }
            }
            assert!(stack.is_empty(), "unclosed starts in {profile:?}");
        }
    }

    #[test]
    fn reference_definitions_are_reported() {
        let scan = scan_blocks(
            "[l]: /u\n\ntext\n",
            MarkdownDialect::WAML_DEFAULT,
            ScanProfile::Tree,
        );
        assert_eq!(scan.reference_definitions, vec![0..8]);
    }

    #[test]
    fn text_entities_are_decoded() {
        assert_eq!(scan_text_entities("&amp;"), "&");
        assert_eq!(scan_text_entities("plain"), "plain");
    }

    #[test]
    fn inline_html_is_recognised_with_its_brackets() {
        assert!(scan_is_inline_html("<span>"));
        assert!(!scan_is_inline_html("<not a tag>"));
    }
}
```

- [ ] **Step 5: Run the scan unit tests**

Run: `cargo test -p waml-syntax markdown::scan`
Expected: all pass. If `reference_definitions_are_reported` or `thematic_break_is_a_rule` fails on the exact range, adjust the expected range to what the adapter actually reports and note it — those are observations of the underlying parser, not requirements.

- [ ] **Step 6: Run the full gate**

Run all four gate commands from Global Constraints. Expected: all exit 0. In particular `cargo clippy --workspace --all-targets` must report zero warnings — if `dead_code` fires despite the `#[allow(dead_code)]`, the attribute is on the wrong item; it belongs on the `mod scan;` declaration in `markdown/mod.rs`.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-syntax/src/markdown/scan crates/waml-syntax/src/markdown/mod.rs
git commit -m "refactor(waml-syntax): add the markdown scan seam

Stage 1a of removing the pulldown-cmark dependency. Introduces waml's own
block-scan vocabulary in markdown/scan, with pulldown-cmark as the sole
implementation confined to scan/pulldown.rs. Nothing consumes it yet, so the
module carries a temporary dead_code allow that the seam-lock task removes.

Inline tags are dropped symmetrically via an open-tag stack, which also lets
End report the exact kind that opened, including the indented/fenced code
block distinction pulldown collapses into one variant."
```

---

### Task 3: Stage 1b — move the tree builder and the GFM alignment map onto the seam

`block.rs` is the main consumer. After this task both `block.rs` and `gfm.rs` are free of `pulldown_cmark`.

**Files:**
- Modify: `crates/waml-syntax/src/markdown/block.rs` (line 3 import; `pulldown_options` at line 20; `parse_strict` around lines 88–125; `start_kind` at line 508; `end_kind` at line 532; `heading_is_setext` at line 1418; test name at line 1501)
- Modify: `crates/waml-syntax/src/markdown/gfm.rs` (line 1 import; `from_pulldown` at line 23)
- Test: existing `crates/waml-syntax/tests/markdown_conformance.rs`, `markdown_blocks.rs`, `markdown_gfm.rs`, `markdown_recovery.rs`, `markdown_snapshot.rs` plus the in-file `#[cfg(test)]` module in `block.rs`

**Interfaces:**
- Consumes from Task 2: `super::scan::{scan_blocks, BlockScan, ScanAlignment, ScanEvent, ScanProfile, ScanTag, ScanTagKind}` with `fn scan_blocks(source: &str, dialect: MarkdownDialect, profile: ScanProfile) -> BlockScan`.
- Produces: `pub(crate) fn TableAlignment::from_scan(value: ScanAlignment) -> Self` in `gfm.rs`, replacing `from_pulldown`. `block.rs::pulldown_options` is **deleted** — it has exactly one caller (itself, line 88) and its logic now lives in `scan/pulldown.rs::tree_options`.

- [ ] **Step 1: Convert the GFM alignment map**

In `crates/waml-syntax/src/markdown/gfm.rs`, replace line 1:
```rust
use pulldown_cmark::Alignment;
```
with:
```rust
use super::scan::ScanAlignment;
```

And replace the `from_pulldown` method (lines 23–30):
```rust
    pub(crate) fn from_pulldown(value: Alignment) -> Self {
        match value {
            Alignment::None => Self::None,
            Alignment::Left => Self::Left,
            Alignment::Center => Self::Center,
            Alignment::Right => Self::Right,
        }
    }
```
with:
```rust
    pub(crate) fn from_scan(value: ScanAlignment) -> Self {
        match value {
            ScanAlignment::None => Self::None,
            ScanAlignment::Left => Self::Left,
            ScanAlignment::Center => Self::Center,
            ScanAlignment::Right => Self::Right,
        }
    }
```

- [ ] **Step 2: Swap `block.rs`'s import and delete `pulldown_options`**

Replace line 3 of `crates/waml-syntax/src/markdown/block.rs`:
```rust
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
```
with:
```rust
use super::scan::{scan_blocks, ScanEvent, ScanProfile, ScanTag, ScanTagKind};
```

Delete the whole `pulldown_options` function (lines 20–31):
```rust
pub(crate) fn pulldown_options(dialect: MarkdownDialect) -> Options {
    let mut options = Options::empty();
    if dialect.tables() {
        options.insert(Options::ENABLE_TABLES);
    }
    if dialect.strikethrough() {
        options.insert(Options::ENABLE_STRIKETHROUGH);
    }
    if dialect.task_lists() {
        options.insert(Options::ENABLE_TASKLISTS);
    }
    options
}
```
It has no callers outside this file (verified) and its body is now `scan/pulldown.rs::tree_options`.

- [ ] **Step 3: Rewrite the scan-driving part of `parse_strict`**

Replace the block from `let parser = Parser::new_ext(...)` through the end of the `for (event, offsets) in parser.into_offset_iter()` loop header and its three event arms. The **order of operations is load-bearing**: reference spans are validated first (a failure there raises `MalformedEventRange`, which `parse` recovers via `recover_raw_text`), then sorted, then events are walked.

Replace:
```rust
    let parser = Parser::new_ext(&source[event_start..end], pulldown_options(dialect));
    let mut reference_spans = Vec::new();
    for (_, definition) in parser.reference_definitions().iter() {
        let span = (event_start + definition.span.start)..(event_start + definition.span.end);
        validate_event_range(source, event_start, end, &span)?;
        reference_spans.push(span);
    }
    reference_spans.sort_by_key(|definition| (definition.start, definition.end));
    for (event, offsets) in parser.into_offset_iter() {
```
with:
```rust
    // Offsets are relative to the scanned slice; re-base them onto `source`.
    let scan = scan_blocks(&source[event_start..end], dialect, ScanProfile::Tree);
    let mut reference_spans = Vec::new();
    for definition in &scan.reference_definitions {
        let span = (event_start + definition.start)..(event_start + definition.end);
        validate_event_range(source, event_start, end, &span)?;
        reference_spans.push(span);
    }
    reference_spans.sort_by_key(|definition| (definition.start, definition.end));
    for (event, offsets) in scan.events {
```

Then, inside that loop, replace `Event::Start(tag) => {` with `ScanEvent::Start(tag) => {`, and replace the `table_alignments` expression:
```rust
                    let table_alignments = match &tag {
                        Tag::Table(alignments) => alignments
                            .iter()
                            .copied()
                            .map(super::gfm::TableAlignment::from_pulldown)
                            .collect(),
                        _ => Vec::new(),
                    };
```
with:
```rust
                    let table_alignments = match &tag {
                        ScanTag::Table { alignments } => alignments
                            .iter()
                            .copied()
                            .map(super::gfm::TableAlignment::from_scan)
                            .collect(),
                        _ => Vec::new(),
                    };
```

Replace the end arm:
```rust
            Event::End(tag) => {
                if end_kind(tag).is_some() {
```
with:
```rust
            ScanEvent::End(kind) => {
                if end_closes_block(kind) {
```

Replace `Event::Rule => {` with `ScanEvent::Rule => {`, and **delete the trailing `_ => {}` arm of that match** — `ScanEvent` has exactly three variants, so the match is now exhaustive and a wildcard would be an `unreachable_patterns` warning.

- [ ] **Step 4: Rewrite `start_kind`, `end_kind`, and `heading_is_setext`**

Replace `start_kind` (line 508):
```rust
fn start_kind(tag: &Tag<'_>, source: &str, range: &Range<usize>) -> Option<Kind> {
    Some(match tag {
        Tag::Paragraph => Kind::Paragraph,
        Tag::Heading { level, .. } => {
            if heading_is_setext(source, range, *level) {
                Kind::SetextHeading
            } else {
                Kind::AtxHeading
            }
        }
        Tag::BlockQuote(_) => Kind::BlockQuote,
        Tag::CodeBlock(CodeBlockKind::Indented) => Kind::IndentedCodeBlock,
        Tag::CodeBlock(CodeBlockKind::Fenced(_)) => Kind::FencedCodeBlock,
        Tag::HtmlBlock => Kind::HtmlBlock,
        Tag::List(_) => Kind::List,
        Tag::Item => Kind::ListItem,
        Tag::Table(_) => Kind::Table,
        Tag::TableHead => Kind::TableHead,
        Tag::TableRow => Kind::TableRow,
        Tag::TableCell => Kind::TableCell,
        _ => return None,
    })
}
```
with:
```rust
fn start_kind(tag: &ScanTag, source: &str, range: &Range<usize>) -> Option<Kind> {
    Some(match tag {
        ScanTag::Paragraph => Kind::Paragraph,
        ScanTag::Heading { .. } => {
            if heading_is_setext(source, range) {
                Kind::SetextHeading
            } else {
                Kind::AtxHeading
            }
        }
        ScanTag::BlockQuote => Kind::BlockQuote,
        ScanTag::IndentedCodeBlock => Kind::IndentedCodeBlock,
        ScanTag::FencedCodeBlock => Kind::FencedCodeBlock,
        ScanTag::HtmlBlock => Kind::HtmlBlock,
        ScanTag::List => Kind::List,
        ScanTag::Item => Kind::ListItem,
        ScanTag::Table { .. } => Kind::Table,
        ScanTag::TableHead => Kind::TableHead,
        ScanTag::TableRow => Kind::TableRow,
        ScanTag::TableCell => Kind::TableCell,
        _ => return None,
    })
}
```
The `_ => return None` arm still matters: the scan vocabulary reports `FootnoteDefinition`, `DefinitionList`, and `DefinitionListDefinition`, which the tree does not represent. The `Tree` profile never enables them today, but the drop must stay so the filter is symmetric with `end_closes_block`.

Replace `end_kind` (line 532) with a boolean — the returned `Kind` was only ever tested with `.is_some()`:
```rust
/// Whether this end closes a frame the tree builder opened.
///
/// Must mirror `start_kind`'s `None` cases exactly, or the frame stack unwinds
/// out of step.
fn end_closes_block(kind: ScanTagKind) -> bool {
    matches!(
        kind,
        ScanTagKind::Paragraph
            | ScanTagKind::Heading
            | ScanTagKind::BlockQuote
            | ScanTagKind::IndentedCodeBlock
            | ScanTagKind::FencedCodeBlock
            | ScanTagKind::HtmlBlock
            | ScanTagKind::List
            | ScanTagKind::Item
            | ScanTagKind::Table
            | ScanTagKind::TableHead
            | ScanTagKind::TableRow
            | ScanTagKind::TableCell
    )
}
```

Replace `heading_is_setext` (line 1418) — its `_level` parameter was already unused:
```rust
fn heading_is_setext(source: &str, range: &Range<usize>) -> bool {
    source[range.clone()]
        .trim_end_matches(['\r', '\n'])
        .rsplit_once('\n')
        .map(|(_, line)| line)
        .is_some_and(|line| {
            let line = line.trim();
            !line.is_empty() && line.bytes().all(|byte| matches!(byte, b'=' | b'-'))
        })
}
```

- [ ] **Step 5: Rename the recovery test so nothing here still says "pulldown"**

At line 1501 of `block.rs`, rename `malformed_pulldown_event_range_recovers_as_raw_text` to `malformed_scan_event_range_recovers_as_raw_text`.

- [ ] **Step 6: Build and run the block + conformance suites**

Run: `cargo test -p waml-syntax`
Expected: PASS, with `commonmark_conformance` and `gfm_extension_conformance` green and no skip list. Any conformance failure means the mapping diverged — fix the mapping, never the fixture.

- [ ] **Step 7: Confirm block.rs and gfm.rs are pulldown-free**

Run: `git grep -n "pulldown_cmark" -- crates/waml-syntax/src/markdown/block.rs crates/waml-syntax/src/markdown/gfm.rs`
Expected: no output (exit 1).

- [ ] **Step 8: Run the full gate**

Run all four gate commands from Global Constraints. Expected: all exit 0.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-syntax/src/markdown/block.rs crates/waml-syntax/src/markdown/gfm.rs
git commit -m "refactor(waml-syntax): build the markdown tree from the scan seam

Stage 1b. block.rs now drives its frame stack from scan_blocks with the Tree
profile, and gfm::TableAlignment::from_pulldown becomes from_scan. Neither
file references pulldown-cmark any more.

end_kind collapses to end_closes_block because callers only ever asked
whether the end closed a frame, and heading_is_setext drops the heading level
parameter it already ignored. The reference-definition spans are still
validated before the event walk so the raw-text recovery path is unchanged."
```

---

### Task 4: Stage 1c — move the shell structure mapper onto the seam

`shell_map` uses the wide profile. After this task `markdown/mod.rs` is free of `pulldown_cmark`.

**Files:**
- Modify: `crates/waml-syntax/src/markdown/mod.rs` (import at line 28; doc comment at line 35; the event loop at lines 73–144; `structure_options` at lines 166–182; `protects` at line 253; `opaque_container` at line 268; `protects_end` at line 271; `heading_level` at line 285)
- Test: existing `crates/waml-syntax/tests/markdown_queries.rs`, `markdown_snapshot.rs`, `markdown_incremental.rs`, `properties.rs`, and the whole conformance suite

**Interfaces:**
- Consumes from Task 2: `scan::{scan_blocks, ScanEvent, ScanProfile, ScanTag, ScanTagKind}`.
- Produces: no new public surface. `structure_options`, `protects_end`, and `heading_level` are **deleted**; `protects` and `opaque_container` change signature to `fn protects(kind: ScanTagKind) -> bool` and `fn opaque_container(kind: ScanTagKind) -> bool`.

- [ ] **Step 1: Swap the import**

Replace line 28 of `crates/waml-syntax/src/markdown/mod.rs`:
```rust
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd};
```
with:
```rust
use scan::{scan_blocks, ScanEvent, ScanProfile, ScanTag, ScanTagKind};
```

- [ ] **Step 2: Fix the doc comment that names pulldown**

Replace lines 34–36:
```rust
/// Maps CommonMark block structure without making any OKF-specific claims.
/// Container depth is deliberately tracked independently of pulldown's event
/// ranges: a heading inside a quote/list/code/HTML container must never become
```
with:
```rust
/// Maps CommonMark block structure without making any OKF-specific claims.
/// Container depth is deliberately tracked independently of the scan's event
/// ranges: a heading inside a quote/list/code/HTML container must never become
```

- [ ] **Step 3: Rewrite the event loop**

Replace the loop header and both event arms (lines 73–143). Note the heading-end arm loses its `expected == heading_level(level)` comparison: the scan's open-tag stack guarantees this `End` closes the heading that opened, so the comparison was a tautology (see Design Decision 4).

Replace:
```rust
    for (event, offsets) in Parser::new_ext(source, structure_options(dialect)).into_offset_iter() {
        let start = offsets.start;
        let end = offsets.end;
        if start < frontmatter_end {
            continue;
        }
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    let level = heading_level(level);
                    if dialect.waml_sections() && containers.is_empty() {
                        pending = Some((level, start, end));
                    }
                }
                Tag::Item => {
                    if containers.len() == 1 {
                        let line_start = line_start(source, start);
                        let line_end = line_end(source, line_start).unwrap_or(len);
                        list_item_lines.push(range(line_start, line_end)?);
                    }
                    containers.push(start);
                    opaque_starts.push(None);
                }
                tag if protects(&tag) => {
                    if matches!(&tag, Tag::CodeBlock(CodeBlockKind::Indented))
                        && containers.is_empty()
                    {
                        collect_tab_indented_items(
                            source,
                            start,
                            end,
                            &mut tab_indented_item_lines,
                        )?;
                    }
                    let is_opaque = opaque_container(&tag);
                    containers.push(start);
                    opaque_starts.push(is_opaque.then_some(start));
                }
                _ => {}
            },
            Event::End(end_tag) => match end_tag {
                TagEnd::Heading(level) => {
                    if let Some((expected, heading_start, heading_end)) = pending.take() {
                        if expected == heading_level(level) && containers.is_empty() {
                            let heading_end = heading_end.max(end).min(len);
                            let text_start = heading_text_start(source, heading_start, heading_end);
                            let heading = ConfirmedHeading {
                                level: expected,
                                range: range(heading_start, heading_end)?,
                                text_range: range(text_start, heading_end)?,
                            };
                            if expected <= 2 {
                                headings.push(heading);
                            } else {
                                nested_headings.push(heading);
                            }
                        }
                    }
                }
                end_tag if protects_end(end_tag) => {
                    if let Some(container_start) = containers.pop() {
                        protected.push(range(container_start, end.max(container_start).min(len))?);
                    }
                    if let Some(Some(opaque_start)) = opaque_starts.pop() {
                        opaque.push(range(opaque_start, end.max(opaque_start).min(len))?);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
```
with:
```rust
    for (event, offsets) in scan_blocks(source, dialect, ScanProfile::Shell).events {
        let start = offsets.start;
        let end = offsets.end;
        if start < frontmatter_end {
            continue;
        }
        match event {
            ScanEvent::Start(tag) => match tag {
                ScanTag::Heading { level } => {
                    if dialect.waml_sections() && containers.is_empty() {
                        pending = Some((level, start, end));
                    }
                }
                ScanTag::Item => {
                    if containers.len() == 1 {
                        let line_start = line_start(source, start);
                        let line_end = line_end(source, line_start).unwrap_or(len);
                        list_item_lines.push(range(line_start, line_end)?);
                    }
                    containers.push(start);
                    opaque_starts.push(None);
                }
                tag if protects(tag.kind()) => {
                    if tag.kind() == ScanTagKind::IndentedCodeBlock && containers.is_empty() {
                        collect_tab_indented_items(
                            source,
                            start,
                            end,
                            &mut tab_indented_item_lines,
                        )?;
                    }
                    let is_opaque = opaque_container(tag.kind());
                    containers.push(start);
                    opaque_starts.push(is_opaque.then_some(start));
                }
                _ => {}
            },
            ScanEvent::End(kind) => match kind {
                // The scan's open-tag stack guarantees this end closes the
                // heading that opened, so the level needs no re-check.
                ScanTagKind::Heading => {
                    if let Some((expected, heading_start, heading_end)) = pending.take() {
                        if containers.is_empty() {
                            let heading_end = heading_end.max(end).min(len);
                            let text_start = heading_text_start(source, heading_start, heading_end);
                            let heading = ConfirmedHeading {
                                level: expected,
                                range: range(heading_start, heading_end)?,
                                text_range: range(text_start, heading_end)?,
                            };
                            if expected <= 2 {
                                headings.push(heading);
                            } else {
                                nested_headings.push(heading);
                            }
                        }
                    }
                }
                kind if protects(kind) => {
                    if let Some(container_start) = containers.pop() {
                        protected.push(range(container_start, end.max(container_start).min(len))?);
                    }
                    if let Some(Some(opaque_start)) = opaque_starts.pop() {
                        opaque.push(range(opaque_start, end.max(opaque_start).min(len))?);
                    }
                }
                _ => {}
            },
            ScanEvent::Rule => {}
        }
    }
```

- [ ] **Step 4: Delete `structure_options`**

Delete lines 166–182 in full:
```rust
fn structure_options(dialect: MarkdownDialect) -> pulldown_cmark::Options {
    // The existing shell structure contract still protects every construct
    // that pulldown can identify. The syntax tree itself uses the narrower
    // public profile above; this compatibility map is removed with the shell
    // structure migration.
    let mut options = pulldown_cmark::Options::all();
    if !dialect.tables() {
        options.remove(pulldown_cmark::Options::ENABLE_TABLES);
    }
    if !dialect.task_lists() {
        options.remove(pulldown_cmark::Options::ENABLE_TASKLISTS);
    }
    if !dialect.strikethrough() {
        options.remove(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    }
    options
}
```
Its body now lives in `scan/pulldown.rs::shell_options`, selected by `ScanProfile::Shell`.

- [ ] **Step 5: Rewrite the predicate helpers**

Replace `protects`, `opaque_container`, and `protects_end` (lines 253–284) with two functions. `protects_end` disappears because the scan reports a precise end kind, so one predicate now serves both directions — which is also what makes `ScanTag::kind()` earn its keep.

Replace:
```rust
fn protects(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::BlockQuote(_)
            | Tag::CodeBlock(_)
            | Tag::HtmlBlock
            | Tag::List(_)
            | Tag::Item
            | Tag::FootnoteDefinition(_)
            | Tag::Table(_)
            | Tag::DefinitionList
            | Tag::DefinitionListDefinition
    )
}

fn opaque_container(tag: &Tag<'_>) -> bool {
    !matches!(tag, Tag::List(_) | Tag::Item)
}
fn protects_end(tag: TagEnd) -> bool {
    matches!(
        tag,
        TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::FootnoteDefinition
            | TagEnd::Table
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListDefinition
    )
}
```
with:
```rust
/// Containers whose interior must never yield a shell boundary.
///
/// Serves both directions: the scan reports a precise end kind, so the same
/// set decides which starts push a container and which ends pop one.
fn protects(kind: ScanTagKind) -> bool {
    matches!(
        kind,
        ScanTagKind::BlockQuote
            | ScanTagKind::IndentedCodeBlock
            | ScanTagKind::FencedCodeBlock
            | ScanTagKind::HtmlBlock
            | ScanTagKind::List
            | ScanTagKind::Item
            | ScanTagKind::FootnoteDefinition
            | ScanTagKind::Table
            | ScanTagKind::DefinitionList
            | ScanTagKind::DefinitionListDefinition
    )
}

fn opaque_container(kind: ScanTagKind) -> bool {
    !matches!(kind, ScanTagKind::List | ScanTagKind::Item)
}
```

- [ ] **Step 6: Delete `heading_level`**

Delete lines 285–294:
```rust
fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}
```
The scan already delivers `u8`. Leaving it would be an unused item and therefore a hard `dead_code` error.

- [ ] **Step 7: Run the structure-sensitive suites**

Run: `cargo test -p waml-syntax`
Expected: PASS. Pay particular attention to `markdown_queries`, `markdown_incremental`, and `properties` — the shell map feeds heading confirmation and protected ranges, so a mapping slip shows up there first.

- [ ] **Step 8: Confirm mod.rs is pulldown-free**

Run: `git grep -n "pulldown" -- crates/waml-syntax/src/markdown/mod.rs`
Expected: no output (exit 1).

- [ ] **Step 9: Run the full gate**

Run all four gate commands from Global Constraints. Expected: all exit 0.

- [ ] **Step 10: Commit**

```bash
git add crates/waml-syntax/src/markdown/mod.rs
git commit -m "refactor(waml-syntax): map shell structure from the scan seam

Stage 1c. shell_map now drives from scan_blocks with the Shell profile, which
carries the wide option set the shell contract needs. structure_options,
protects_end, and heading_level are deleted: the scan reports precise end
kinds and u8 heading levels, so one protects() predicate now serves both
directions.

The heading-end level re-check is dropped as a tautology - the scan's
open-tag stack guarantees the end closes the heading that opened."
```

---

### Task 5: Stage 1d — move the two inline helpers onto the seam

Inline parsing is already hand-written. Only two small helpers reach for pulldown: entity decoding and raw-HTML recognition.

**Files:**
- Modify: `crates/waml-syntax/src/markdown/inline.rs` (import at line 3; `decode_entity` at line 1349; the tail of `is_raw_html` at line 1375)
- Test: existing `crates/waml-syntax/tests/markdown_inlines.rs`, `markdown_extensions.rs`, and the conformance suite

**Interfaces:**
- Consumes from Task 2: `super::scan::{scan_is_inline_html, scan_text_entities}` with `fn scan_text_entities(spelling: &str) -> String` and `fn scan_is_inline_html(candidate: &str) -> bool`.
- Produces: no signature change. `decode_entity` stays `pub(crate) fn decode_entity(spelling: &str) -> Option<String>` and `is_raw_html` stays `fn is_raw_html(value: &str) -> bool`.

- [ ] **Step 1: Swap the import**

Replace line 3 of `crates/waml-syntax/src/markdown/inline.rs`:
```rust
use pulldown_cmark::{Event, Parser};
```
with:
```rust
use super::scan::{scan_is_inline_html, scan_text_entities};
```

- [ ] **Step 2: Rewrite `decode_entity`**

Replace:
```rust
pub(crate) fn decode_entity(spelling: &str) -> Option<String> {
    let mut text = String::new();
    for event in Parser::new(spelling) {
        if let Event::Text(value) = event {
            text.push_str(&value);
        }
    }
    (text != spelling && !text.is_empty()).then_some(text)
}
```
with:
```rust
pub(crate) fn decode_entity(spelling: &str) -> Option<String> {
    let text = scan_text_entities(spelling);
    (text != spelling && !text.is_empty()).then_some(text)
}
```
The "changed and non-empty" test stays here, in the consumer: it is `decode_entity`'s policy, not the scanner's.

- [ ] **Step 3: Rewrite the tail of `is_raw_html`**

Replace the last two statements of `is_raw_html`:
```rust
    let candidate = format!("<{value}>");
    Parser::new(&candidate).any(
        |event| matches!(event, Event::InlineHtml(html) | Event::Html(html) if html.as_ref() == candidate),
    )
```
with:
```rust
    let candidate = format!("<{value}>");
    scan_is_inline_html(&candidate)
```
Leave the comment/CDATA/processing-instruction/DOCTYPE fast paths above it exactly as they are — they are hand-written already.

- [ ] **Step 4: Run the inline suites**

Run: `cargo test -p waml-syntax`
Expected: PASS, including the full conformance suite. The entity and raw-HTML paths are exercised heavily by the CommonMark "Entity and numeric character references" and "Raw HTML" sections.

- [ ] **Step 5: Confirm inline.rs is pulldown-free**

Run: `git grep -n "pulldown" -- crates/waml-syntax/src/markdown/inline.rs`
Expected: no output (exit 1).

- [ ] **Step 6: Run the full gate**

Run all four gate commands from Global Constraints. Expected: all exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-syntax/src/markdown/inline.rs
git commit -m "refactor(waml-syntax): route the inline helpers through the scan seam

Stage 1d. decode_entity and is_raw_html were the last two pulldown-cmark uses
in the otherwise hand-written inline parser. They now call
scan_text_entities and scan_is_inline_html. The 'changed and non-empty'
policy stays in decode_entity, where it belongs."
```

---

### Task 6: Stage 1e — lock the seam

Every consumer has moved, so the temporary `dead_code` allow comes off and a guard test makes the invariant permanent: `pulldown_cmark` may appear in `src/markdown/scan/pulldown.rs` and nowhere else under `src/markdown/`.

The guard follows the style of `crates/waml/tests/no_legacy_authority.rs`, which self-tests its detectors against synthetic sources rather than trusting them. That matters here: a guard that silently matches nothing would pass forever while enforcing nothing.

**Files:**
- Create: `crates/waml-syntax/tests/scan_seam.rs`
- Modify: `crates/waml-syntax/src/markdown/mod.rs` (remove the `#[allow(dead_code)]` added in Task 2)

**Interfaces:**
- Consumes: nothing from earlier tasks at the type level — the guard reads source files as text.
- Produces: nothing consumed by later work. This is the terminal task of Stage 1.

- [ ] **Step 1: Write the guard, expecting it to pass**

Create `crates/waml-syntax/tests/scan_seam.rs`:

```rust
//! Architecture guard for the Markdown parser seam.
//!
//! The Markdown front end talks to exactly one third-party parser, through
//! exactly one file. Stage 2 replaces that file's contents with a hand-written
//! scanner; this test is what makes that a one-file change instead of an
//! archaeology project.

use std::{fs, path::{Path, PathBuf}};

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
    FORBIDDEN
        .iter()
        .any(|forbidden| source.contains(forbidden))
}

#[test]
fn detector_recognises_the_dependency() {
    assert!(mentions_dependency("use pulldown_cmark::Parser;"));
    assert!(mentions_dependency("pulldown-cmark.workspace = true"));
    assert!(mentions_dependency("    let _ = pulldown_cmark::Options::all();"));
}

#[test]
fn detector_ignores_unrelated_sources() {
    assert!(!mentions_dependency("use super::scan::scan_blocks;"));
    assert!(!mentions_dependency("// the scan seam hides the parser"));
    assert!(!mentions_dependency("fn malformed_scan_event_range_recovers() {}"));
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
```

- [ ] **Step 2: Run the guard**

Run: `cargo test -p waml-syntax --test scan_seam`
Expected: four tests PASS. If `only_the_seam_file_references_the_dependency` fails, a Stage 1 task left a reference behind — fix the source file, never the guard's allow-list.

- [ ] **Step 3: Prove the guard bites**

Temporarily add `// pulldown_cmark` as the first line of `crates/waml-syntax/src/markdown/gfm.rs`.
Run: `cargo test -p waml-syntax --test scan_seam`
Expected: FAIL, naming `crates/waml-syntax/src/markdown/gfm.rs`.
Then remove the line and re-run: PASS. Do not commit the temporary line.

- [ ] **Step 4: Remove the temporary dead_code allow**

In `crates/waml-syntax/src/markdown/mod.rs`, replace:
```rust
// Consumers land in the following tasks; the allow is removed once they have.
#[allow(dead_code)]
pub(crate) mod scan;
```
with:
```rust
pub(crate) mod scan;
```

- [ ] **Step 5: Verify nothing in the seam is unused**

Run: `cargo clippy --workspace --all-targets`
Expected: 0 warnings. If `dead_code` fires on a `ScanTag` field, a `ScanTagKind` variant, or a `scan` function, **delete the unused item** — do not re-add the allow. An unread payload is exactly the kind of speculative surface Stage 2 should be free to design fresh.

- [ ] **Step 6: Run the full gate**

Run all four gate commands from Global Constraints. Expected: all exit 0. Confirm one last time:

Run: `git grep -ln "pulldown_cmark" -- crates/waml-syntax/src/`
Expected: exactly one line, `crates/waml-syntax/src/markdown/scan/pulldown.rs`.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-syntax/tests/scan_seam.rs crates/waml-syntax/src/markdown/mod.rs
git commit -m "test(waml-syntax): lock pulldown-cmark behind the scan seam

Stage 1e. Adds an architecture guard asserting that pulldown-cmark is
referenced by exactly one file under src/markdown, and drops the temporary
dead_code allow now that every consumer has moved onto the seam.

The guard self-tests its detector against synthetic sources and asserts the
source walk found files, so it cannot pass by silently matching nothing."
```

---

## Follow-on stages (out of scope — no tasks here)

Stage 1 leaves the tree behaviourally identical with the dependency confined to one file. The remaining stages, each a separate plan:

- **Stage 2 — hand-written block scanner.** Reimplement `scan_blocks` for `ScanProfile::Tree` and `ScanProfile::Shell` without pulldown: line classification, container stack (block quotes, list items, indented and fenced code, HTML blocks), setext/ATX headings, thematic breaks, GFM tables with alignments, and link reference definition collection. `benches/markdown_parse.rs` from Task 1 is the performance gate; the 652 + 24 conformance examples are the correctness gate.
- **Stage 3 — hand-written inline pieces.** `scan_text_entities` needs the HTML5 named-entity table plus numeric references; `scan_is_inline_html` needs the CommonMark raw-HTML tag grammar. The rest of inline parsing (delimiter runs, links, emphasis) is already hand-written in `inline.rs`.
- **Stage 4 — drop the dependency.** Delete `scan/pulldown.rs`, delete `crates/waml-syntax/Cargo.toml`'s `pulldown-cmark.workspace = true` and the workspace-root entry, remove the pulldown exemption at `crates/waml/tests/no_legacy_authority.rs:362`, and retire or repoint `tests/scan_seam.rs`.

---

## Self-Review

**1. Spec coverage.**
- Stage 0 benchmark harness, `harness = false`, std timing, spec.json corpus, no new deps → Task 1 Steps 3–5.
- Stage 0 conformance corpus-size guard, real counts not invented → Task 1 Steps 1–2; both counts verified against the fixtures (652 CommonMark; 24 GFM, from a fixture containing 672 example fences total, of which the loader's five named sections hold 24).
- `scan/mod.rs` + `scan/pulldown.rs` with the vocabulary, pulldown-backed `scan_blocks`, unit tests, nothing consuming it → Task 2.
- `block.rs` onto `ScanProfile::Tree`, `from_pulldown` → `from_scan`, both files lose the import → Task 3.
- `mod.rs::shell_map` onto `ScanProfile::Shell`, `protects`/`opaque_container`/`protects_end`/`heading_level` rewritten → Task 4.
- `inline.rs` helpers onto the seam → Task 5.
- Seam-lock guard with positive and negative self-assertions → Task 6.
- Stages 2–4 mentioned briefly, no tasks → "Follow-on stages".
- Every Global Constraint from the brief is reproduced in the Global Constraints section, and every task's final steps run the full four-command gate.

**2. Placeholder scan.** No TBD, no "handle edge cases", no "similar to Task N" — Task 4's loop rewrite repeats the whole body rather than referring back to Task 3. Every code step carries the actual code. The only intentionally open value is the exact byte range in two Task 2 unit tests, and that step says explicitly what to do if the observed range differs (record the observation; it is not a requirement).

**3. Type consistency.** `scan_blocks` / `scan_text_entities` / `scan_is_inline_html` are spelled identically in Tasks 2, 3, 4, 5. `ScanTag::kind()` is defined in Task 2 and used in Tasks 4 and 6's reasoning. `TableAlignment::from_scan` is defined in Task 3 Step 1 and called in Task 3 Step 3. `end_closes_block` replaces `end_kind` consistently within Task 3. `ScanProfile::{Tree, Shell}` line up with `tree_options`/`shell_options`. The `#[allow(dead_code)]` added in Task 2 Step 3 is removed in Task 6 Step 4 with the identical surrounding text.

**Corrections made to the incoming sketch** (found by re-reading the source):
- `ScanTag::List { ordered: bool }` and a labelled `FootnoteDefinition` were dropped to fieldless variants. Nothing reads those payloads, and this repo's gate runs clippy with `-D warnings`, which turns an unread field into a hard build error.
- `heading_is_setext` already ignored its heading-level argument (`_level: HeadingLevel`), so the parameter is removed outright rather than retyped.
- `block.rs::end_kind` returned a `Kind` that no caller read (`.is_some()` only), so it becomes `end_closes_block(kind) -> bool`.
- `shell_map`'s `TagEnd::Heading(level)` arm compared the end level against the pending start level. With a precise scan end kind that comparison is a tautology; it is dropped, and the reasoning is written into the code as a comment and into Design Decision 4.
- `mod.rs::protects_end` disappears entirely — with precise end kinds, one `protects(ScanTagKind)` predicate serves both directions.
- `block.rs::pulldown_options` is `pub(crate)` but has exactly one caller, in its own file, so it is deleted rather than moved-and-re-exported.
- Task 2 needs a temporary `#[allow(dead_code)]`; the sketch's "nothing consumes it yet" task would otherwise fail the gate on the very first commit.
- The adapter must read `reference_definitions()` *before* `into_offset_iter()` consumes the parser, which is a concrete reason `BlockScan` is eager rather than an iterator — added to Design Decision 5.
- `ScanProfile::Shell` uses `Options::all()`, which admits tags the vocabulary does not name (metadata blocks, definition-list titles, superscript, ...). The adapter's `open` stack drops those starts *and* their ends, which is what makes the drop safe; this is called out in the module docs and covered by the `every_start_has_a_matching_end` test.
