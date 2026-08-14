# Markdown Reading View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a markdown **reading view (viewer)** widget, distinct from the markdown **editor** widget, so that an OKF concept renders as readable prose, and remove the interim `hide_syntax` path once the viewer is live.

**Architecture:** The split seam is `PresentationPlan`, not the parser. The **editor** styles RAW markdown (source line == visual row) and keeps `waml-markdown-editor`'s `LayoutEngine`. The **viewer** RENDERS markdown as blocks with no source-line correspondence, and is a thin `PresentationPlan` -> makepad `TextFlow` driver — a few hundred lines, **not** a new block layout engine. `waml-syntax`, `compile_presentation`, `PresentationStyles`, decorations, highlighters, and assets are shared; the `LayoutEngine` rows/lanes, motion, selection, input, and IME are not.

**Rejected alternatives (do not relitigate, do not re-propose):**
- **makepad's `Markdown` widget.** It calls `pulldown_cmark::Parser` itself, which would mean two independent parses of one document, and it cannot see `MarkdownDialect`. This is a *correctness* rejection, not taste.
- **Substitute-text runs** (a glyph backed by no source range). That would break the "everything drawn maps back to source" invariant. Bullets are drawn as **decorations**, never as text.
- **A new block layout engine.** `TextFlow` (`C:/dev/makepad/widgets/src/text_flow.rs`) already provides block flow, selection, copy, and `point_to_index`.

**Tech Stack:** Rust (workspace edition/rust-version), makepad (writable fork clone at `C:/dev/makepad`, branch `waml`, pinned by rev in this repo's root `Cargo.toml`), `makepad_widgets::TextFlow`, existing `waml-markdown-editor::presentation` module. **No new third-party dependencies.**

## Global Constraints

- **Full gate green before every commit.** Every task ends with all four of these at exit 0:
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets` — must be **0 warnings**. The gate runs `-D warnings`, which promotes `dead_code` to a hard error, so **never land an item, field, method, or enum variant that nothing reads**. If a task produces a type that only the *next* task consumes, that type must be exercised by a unit test in its own task.
  - `cargo fmt --all -- --check`
  - `cd editors/vscode && pnpm build && pnpm lint && pnpm test`
- **Do NOT commit `editors/vscode/package-lock.json`.** The repo has none by design. `npm install` creates one; **delete it before `git add -A`**. (`npm ci` fails — there is no lockfile.)
- **Never commit `proptest-regressions/`.**
- **Commit messages:** conventional-commit subject + body. **No Claude co-author trailer** — the user considers it advertising.
- **Work only inside the worktree** `C:/dev/waml/.worktrees/markdown-hide-syntax`. Verify with `git rev-parse --show-toplevel` before the first edit of every task. **Never edit the main checkout at `C:/dev/waml`.** Always pass **absolute** paths to the Edit tool — the Edit tool has no cwd, and a main-root path silently edits main and then "passes" as baseline.
- **Fork changes are a separate commit plus a rev bump.** This repo builds against `C:/dev/makepad`, checked out on branch `waml`, pinned by rev in the root `Cargo.toml`. If the viewer needs a `TextFlow` change, it is a **fork-side commit on the fork's `waml` integration branch** followed by editing every `rev = "..."` in this repo's root `Cargo.toml` to the **new SHA — never a branch name**. Task 2 batches *all* fork changes into one fork commit so there is exactly one rev bump in this plan.
- **`PresentationPlan::validate_source_partition` is the safety invariant:** every source byte is in exactly one text run. **Never DROP a run to hide it.** The viewer's block model carries the same invariant and must test it.
- **`TextMetrics.line_spacing` is a MULTIPLIER, not pixels.** Using it as a height once put a bullet above its own list item.
- **`TextStyle.font_size` is in POINTS.** A past bug double-applied 96/72. If you touch sizing, verify against `prepare_single_line_run`.
- **Several files in this repo are CRLF** (`crates/waml-markdown-editor/tests/layout_geometry.rs`, `crates/waml-markdown-editor/src/layout/makepad.rs`). Use the Edit tool. Python string replacement with `\n` literals silently matches nothing.
- **A green test is not a visual check.** Every task that changes what is drawn ends with a real screenshot verification step (exact commands given in the task). Launch by pid, capture by pid, kill by pid **in one tool call** — **never `Stop-Process -Name waml-editor`**, which kills the user's own editor.

---

## File Structure

**Created (this repo):**
- `crates/waml-markdown-editor/src/reading/mod.rs` — module root, re-exports.
- `crates/waml-markdown-editor/src/reading/model.rs` — the viewer-facing block model: `ReadingDocument`, `ReadingBlock`, `ReadingBlockKind`, `ReadingPiece`, `ReadingError`, and `build_reading_document`. Pure data, no makepad types, no drawing. This is the *only* place that decides which runs a reading view suppresses.
- `crates/waml-markdown-editor/src/reading/widget.rs` — `MarkdownViewer`, the `ReadingDocument` -> `TextFlow` driver widget, plus `SourceMap` (flow-byte-index -> source `TextRange`). Owns no layout engine.
- `crates/waml-markdown-editor/src/reading/bullet.rs` — `DrawReadingBullet`, the bullet decoration shader + `bullet_shape_for_level`.
- `crates/waml-markdown-editor/tests/reading_model.rs` — model unit tests, including the source-partition invariant.
- `crates/waml-markdown-editor/tests/reading_source_map.rs` — `SourceMap` unit tests.
- `crates/waml-editor/src/reading_view.rs` — `ReadingView`, the `waml-editor`-side analogue of `SourceView`: compiles a plan with `PresentationStyles::balanced()`, builds a `ReadingDocument`, installs it on the `MarkdownViewer`.
- `crates/waml-editor/tests/fixtures/okf-only/reading.md` — a rich OKF concept fixture (heading, prose, nested bullets, ordered list, quote, fenced code, inline code, link, table) used as the visual-verification target.

**Modified:**
- `crates/waml-markdown-editor/src/lib.rs` — declares `pub mod reading;`.
- `crates/waml-markdown-editor/src/widget.rs` — `script_mod` also registers the viewer's widgets.
- `crates/waml-editor/src/app.rs` — a `markdown_viewer_surface` sibling of `markdown_surface` in the app DSL, plus the source-toggle button; `script_mod` order.
- `crates/waml-editor/src/doc_view.rs` — `BodyWidgets` gains `markdown_viewer()`, `show_markdown_viewer()`, and hides the viewer in `show_markdown_editor` / `show_canvas`.
- `crates/waml-editor/src/generic_okf_view.rs` — switches from `SourceView` + `set_hide_syntax(true)` to `ReadingView`, with an explicit "show markdown source" toggle into the editor.
- `crates/waml-editor/src/lib.rs` — declares `pub mod reading_view;`.
- `crates/waml-editor/src/source_view.rs` — **Task 5** deletes `hide_syntax`, `set_hide_syntax`, `hides_syntax`, and the `hide_syntax` parameter of `compile`.
- `crates/waml-markdown-editor/src/presentation/style.rs` — **Task 5** deletes `PresentationStyles::hide_syntax` and `hiding_syntax()`.
- `crates/waml-markdown-editor/src/presentation/compile.rs` — **Task 5** deletes the `styles.hide_syntax` branches, `push_text_hidden`, `is_unordered_marker`, `list_nesting_level`, and the `ListBullet` decoration emission.
- `Cargo.toml` (root) — **Task 2** bumps every makepad `rev` to the new fork SHA.

**Created (fork, `C:/dev/makepad`, branch `waml`) — one commit, Task 2:**
- `widgets/src/text_flow.rs` — adds `TextFlow::selection_range()` and `TextFlow::begin_list_item_gutter()`.

**Deleted:**
- `crates/waml-markdown-editor/tests/presentation_hidden_syntax.rs` — **Task 5**. Its 8 tests all assert `hide_syntax` behaviour that no longer exists.

## Design Decisions (verified against the code; these are the contract)

1. **The viewer's suppression decision lives in `reading/model.rs`, never in `compile.rs`.** `compile_presentation` produces one canonical plan for both surfaces. The model asks `TextRole::is_syntax_marker()` and marks the piece `emit: false`. **`TextRole::is_syntax_marker()` stays** — it is shared vocabulary. Only the `hide_syntax` *flag* and the plumbing that threaded it through `PresentationStyles` go away.
2. **`ReadingPiece` keeps every source byte.** A suppressed marker is a piece with `emit: false`, not a missing piece. `ReadingDocument::validate_source_partition` mirrors `PresentationPlan::validate_source_partition` and is tested. This is the invariant that made hiding safe and it must survive the port.
3. **`TextFlow`'s selection index space is its own accumulated `SelectionTracker.text` buffer, not source offsets.** Verified at `text_flow.rs:351-392`. The driver therefore maintains a `SourceMap`: an ordered list of `(flow: Range<usize>, source: Option<TextRange>)` pieces. Structural newlines TextFlow injects (`push_newline`, from `end_list_item` / `end_quote` / `end_code` / `new_line_collapsed`) map to `None`.
4. **`TextFlow::draw_text` TRIMS its input** (`text_flow.rs:1877-1884`): it returns early for `" "`/`""` when `first_thing_on_a_line`, `trim_start()`s when `first_thing_on_a_line`, and always `trim_end_matches("\n")`. A raw source slice handed to it would therefore land in the tracker at a *different* length than the slice, silently desynchronising the map. **The driver must trim the slice itself, adjust the recorded `TextRange` by the trimmed byte counts, and then `debug_assert_eq!` the tracker's length delta against the trimmed slice length.** That assertion is the map's guard rail.
5. **Bullets are decorations, drawn by us.** `TextFlow::begin_list_item(cx, dot, pad)` draws `dot` as *text* (`text_flow.rs:1481`), which is exactly the rejected substitute-text run. The fork gains `begin_list_item_gutter`, which performs `begin_list_item`'s identical indent arithmetic but reserves an empty `gutter`-wide box and **returns its `Rect`** so the caller draws a decoration into it.
6. **Bullet shape varies by nesting level** (today it is a plain square at every level, with `level` threaded but unused). The viewer draws `level % 3`: `0` = filled disc, `1` = hollow ring, `2` = filled square. An **ordered** marker (`1.`) is content, so it is drawn as real text from its source range and recorded in the `SourceMap`.
7. **Copy from the viewer yields rendered prose, not markdown.** That is the point of a reading view, and it falls out of `TextFlow::selected_text()` for free. The `SourceMap` exists to carry the *caret* into the editor on the source toggle, not to rewrite copy.
8. **The source toggle does not make concepts writable.** `GenericOkfView` discards `outcome.source_edit` today, so a writable editor would silently drop edits. The toggle switches *rendering* (viewer <-> read-only editor). Making concepts writable is explicitly **out of scope**; leave it as a follow-up.
9. **`combine_spaces` and `ignore_newlines` on `TextFlow` are declared and cleared but never read** (`text_flow.rs:656,658,1182,1183`). Do not rely on them.
10. **The `FakeShaper` fix and the ascender/descender floor fix are KEPT.** They are real bugs in shared code, unrelated to hiding. Task 5 must not touch `crates/waml-markdown-editor/tests/layout_geometry.rs`'s `FakeShaper` or `layout/engine.rs::push_paragraph`'s `font_size * 0.8` floor. Any new test double must mirror the real shaper the same way.

---

### Task 1: The viewer block model

A pure-data `ReadingDocument` derived from a `PresentationPlan`. Nothing is drawn. This task is complete and reviewable on its own.

**Files:**
- Create: `crates/waml-markdown-editor/src/reading/mod.rs`
- Create: `crates/waml-markdown-editor/src/reading/model.rs`
- Create: `crates/waml-markdown-editor/tests/reading_model.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`

**Interfaces:**
- Consumes: `waml_markdown_editor::presentation::{PresentationPlan, PresentationItem, PresentationBlock, PresentationBlockKind, BlockDecorationKind, TextRole, TextStyle}` and `waml_syntax::{TextRange, TextSize}` — all already public.
- Produces, for Tasks 2-4:
  ```rust
  pub fn build_reading_document(plan: &PresentationPlan) -> Result<ReadingDocument, ReadingError>;

  pub struct ReadingDocument { pub roots: Vec<ReadingBlock>, pub source_len: TextSize }
  impl ReadingDocument { pub fn validate_source_partition(&self) -> Result<(), ReadingError>; }

  pub struct ReadingBlock {
      pub kind: ReadingBlockKind,
      pub source_range: TextRange,
      pub pieces: Vec<ReadingPiece>,
      pub children: Vec<ReadingBlock>,
  }

  pub enum ReadingBlockKind {
      Paragraph,
      Heading(u8),
      BulletItem { level: u8 },
      OrderedItem { level: u8 },
      Quote,
      Code,
      Table { columns: u32 },
      TableRow,
      TableCell { column: u32 },
      Image,
      ThematicBreak,
  }

  pub struct ReadingPiece {
      pub range: TextRange,
      pub role: TextRole,
      pub style: TextStyle,
      /// `false` for markdown punctuation the reading view suppresses. The
      /// piece is KEPT so the source partition stays complete.
      pub emit: bool,
  }

  pub enum ReadingError {
      Gap { expected: TextSize, actual: TextSize },
      Overlap { previous_end: TextSize, next: TextRange },
      UnknownParent(usize),
  }
  ```

- [ ] **Step 1: Verify the worktree**

Run (PowerShell tool):

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax; git rev-parse --show-toplevel
```

Expected: `C:/dev/waml/.worktrees/markdown-hide-syntax`. If it prints anything else, STOP.

- [ ] **Step 2: Write the failing tests**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/tests/reading_model.rs`:

```rust
//! The viewer block model derived from a `PresentationPlan`.
//!
//! The model keeps every source byte: a suppressed marker is a piece with
//! `emit == false`, never a missing piece. That is the same invariant
//! `PresentationPlan::validate_source_partition` enforces, and it is what
//! makes "everything drawn maps back to source" checkable.

use std::sync::Arc;

use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles, TextRole,
};
use waml_markdown_editor::reading::{build_reading_document, ReadingBlockKind, ReadingDocument};
use waml_markdown_editor::syntax::{parse_markdown, MarkdownDialect, SourceText};

fn document(source: &str) -> ReadingDocument {
    let text = SourceText::from(source);
    let syntax = parse_markdown(text, MarkdownDialect::Okf).expect("markdown parses");
    let styles = Arc::new(PresentationStyles::balanced());
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles");
    build_reading_document(&plan).expect("reading model builds")
}

fn kinds(doc: &ReadingDocument) -> Vec<ReadingBlockKind> {
    fn walk(blocks: &[waml_markdown_editor::reading::ReadingBlock], out: &mut Vec<ReadingBlockKind>) {
        for block in blocks {
            out.push(block.kind);
            walk(&block.children, out);
        }
    }
    let mut out = Vec::new();
    walk(&doc.roots, &mut out);
    out
}

#[test]
fn every_source_byte_lands_in_exactly_one_piece() {
    let doc = document("# Title\n\nBody *emphasis* and `code`.\n\n- one\n- two\n");
    doc.validate_source_partition()
        .expect("the reading model must cover the source exactly once");
}

#[test]
fn a_suppressed_marker_is_kept_as_a_non_emitting_piece() {
    let doc = document("# Title\n");
    let heading = doc
        .roots
        .iter()
        .find(|block| matches!(block.kind, ReadingBlockKind::Heading(1)))
        .expect("an h1 block");
    let marker = heading
        .pieces
        .iter()
        .find(|piece| matches!(piece.role, TextRole::HeadingMarker(1)))
        .expect("the `#` run survives as a piece");
    assert!(
        !marker.emit,
        "a reading view suppresses the `#` but must not drop its source range"
    );
    assert!(
        heading.pieces.iter().any(|piece| piece.emit),
        "the heading text itself still emits"
    );
}

#[test]
fn frontmatter_is_suppressed_but_still_covered() {
    let doc = document("---\ntitle: Notes\n---\n\n# Notes\n");
    doc.validate_source_partition()
        .expect("frontmatter bytes stay in the partition");
    fn any_emitting_frontmatter(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> bool {
        blocks.iter().any(|block| {
            block
                .pieces
                .iter()
                .any(|piece| piece.role == TextRole::Frontmatter && piece.emit)
                || any_emitting_frontmatter(&block.children)
        })
    }
    assert!(
        !any_emitting_frontmatter(&doc.roots),
        "frontmatter is document metadata, not prose"
    );
}

#[test]
fn an_unordered_item_becomes_a_bullet_item_and_its_marker_does_not_emit() {
    let doc = document("- one\n- two\n");
    let items: Vec<_> = kinds(&doc)
        .into_iter()
        .filter(|kind| matches!(kind, ReadingBlockKind::BulletItem { .. }))
        .collect();
    assert_eq!(items.len(), 2, "two bullet items");
    assert_eq!(items[0], ReadingBlockKind::BulletItem { level: 0 });
    fn markers_emit(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> bool {
        blocks.iter().any(|block| {
            block
                .pieces
                .iter()
                .any(|piece| piece.role == TextRole::ListMarker && piece.emit)
                || markers_emit(&block.children)
        })
    }
    assert!(!markers_emit(&doc.roots), "a bullet character is punctuation");
}

#[test]
fn an_ordered_number_is_content_and_still_emits() {
    let doc = document("1. one\n2. two\n");
    let items: Vec<_> = kinds(&doc)
        .into_iter()
        .filter(|kind| matches!(kind, ReadingBlockKind::OrderedItem { .. }))
        .collect();
    assert_eq!(items.len(), 2, "two ordered items");
    fn emitting_markers(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> usize {
        blocks
            .iter()
            .map(|block| {
                block
                    .pieces
                    .iter()
                    .filter(|piece| piece.role == TextRole::ListMarker && piece.emit)
                    .count()
                    + emitting_markers(&block.children)
            })
            .sum()
    }
    assert_eq!(
        emitting_markers(&doc.roots),
        2,
        "an ordered number is content a reader needs"
    );
}

#[test]
fn nested_items_report_their_nesting_level() {
    let doc = document("- outer\n  - inner\n");
    let levels: Vec<u8> = kinds(&doc)
        .into_iter()
        .filter_map(|kind| match kind {
            ReadingBlockKind::BulletItem { level } => Some(level),
            _ => None,
        })
        .collect();
    assert_eq!(levels, vec![0, 1], "nesting depth drives the bullet shape");
}

#[test]
fn a_quote_nests_its_paragraph_and_suppresses_the_angle_bracket() {
    let doc = document("> quoted\n");
    let quote = doc
        .roots
        .iter()
        .find(|block| block.kind == ReadingBlockKind::Quote)
        .expect("a quote block");
    assert!(
        !quote.children.is_empty(),
        "a quote owns the paragraph it wraps"
    );
    fn quote_markers_emit(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> bool {
        blocks.iter().any(|block| {
            block
                .pieces
                .iter()
                .any(|piece| piece.role == TextRole::QuoteMarker && piece.emit)
                || quote_markers_emit(&block.children)
        })
    }
    assert!(!quote_markers_emit(&doc.roots), "`>` is punctuation");
}

#[test]
fn fenced_code_keeps_its_content_and_suppresses_its_fences() {
    let doc = document("```rust\nlet x = 1;\n```\n");
    assert!(
        kinds(&doc).contains(&ReadingBlockKind::Code),
        "a fenced block becomes a code block"
    );
    fn roles(blocks: &[waml_markdown_editor::reading::ReadingBlock]) -> Vec<(TextRole, bool)> {
        let mut out = Vec::new();
        for block in blocks {
            out.extend(block.pieces.iter().map(|piece| (piece.role, piece.emit)));
            out.extend(roles(&block.children));
        }
        out
    }
    let roles = roles(&doc.roots);
    assert!(
        roles.iter().any(|(role, emit)| *role == TextRole::CodeFence && !*emit),
        "the ``` fences are suppressed"
    );
    assert!(
        roles
            .iter()
            .any(|(role, emit)| matches!(role, TextRole::CodeContent | TextRole::CodeToken(_)) && *emit),
        "the code itself is drawn"
    );
}

#[test]
fn an_empty_document_is_a_valid_empty_model() {
    let doc = document("");
    assert!(doc.roots.is_empty(), "no blocks");
    doc.validate_source_partition()
        .expect("a zero-length source is trivially covered");
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-markdown-editor --test reading_model`
Expected: FAIL — `unresolved import waml_markdown_editor::reading`.

- [ ] **Step 4: Create the module root**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/reading/mod.rs`:

```rust
//! The markdown **reading view**: a block-rendered presentation of a document.
//!
//! The reading view is a different surface from the editor. The editor styles
//! RAW markdown, so a source line is a visual row. The reading view RENDERS
//! markdown, so there is no source-line correspondence at all. The split seam
//! is `PresentationPlan`: both surfaces consume the same compiled plan, the
//! same styles, the same decorations, highlighters and assets. Neither shares
//! the other's layout engine, motion, selection, input or IME.

pub mod model;

pub use model::{
    build_reading_document, ReadingBlock, ReadingBlockKind, ReadingDocument, ReadingError,
    ReadingPiece,
};
```

- [ ] **Step 5: Declare the module**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/lib.rs`, inserting after the `pub mod presentation;` line:

```rust
pub mod reading;
```

- [ ] **Step 6: Write the model**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/reading/model.rs`:

```rust
//! The viewer-facing block model, derived from a `PresentationPlan`.
//!
//! This is the ONLY place that decides which runs a reading view suppresses.
//! It never drops a run: a suppressed marker is a `ReadingPiece` with
//! `emit == false`, so the model keeps the plan's guarantee that every source
//! byte lies in exactly one piece. Dropping a run instead would make
//! "everything drawn maps back to source" unverifiable.

use std::fmt;

use waml_syntax::{TextRange, TextSize};

use crate::presentation::{
    BlockDecorationKind, PresentationBlockKind, PresentationItem, PresentationPlan, TextRole,
    TextStyle,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingBlockKind {
    Paragraph,
    Heading(u8),
    BulletItem { level: u8 },
    OrderedItem { level: u8 },
    Quote,
    Code,
    Table { columns: u32 },
    TableRow,
    TableCell { column: u32 },
    Image,
    ThematicBreak,
}

/// One text run of a block, in source order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReadingPiece {
    pub range: TextRange,
    pub role: TextRole,
    pub style: TextStyle,
    /// `false` for markdown punctuation a reading view suppresses. The piece is
    /// KEPT so the source partition stays complete.
    pub emit: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadingBlock {
    pub kind: ReadingBlockKind,
    pub source_range: TextRange,
    pub pieces: Vec<ReadingPiece>,
    pub children: Vec<ReadingBlock>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReadingDocument {
    pub roots: Vec<ReadingBlock>,
    pub source_len: TextSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadingError {
    Gap { expected: TextSize, actual: TextSize },
    Overlap { previous_end: TextSize, next: TextRange },
    UnknownParent(usize),
}

impl fmt::Display for ReadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gap { expected, actual } => write!(
                f,
                "reading model gap: expected a piece at {} but the next boundary is {}",
                expected.to_usize(),
                actual.to_usize()
            ),
            Self::Overlap { previous_end, next } => write!(
                f,
                "reading model overlap: {}..{} starts before {}",
                next.start().to_usize(),
                next.end().to_usize(),
                previous_end.to_usize()
            ),
            Self::UnknownParent(index) => {
                write!(f, "reading model block {index} names a parent that follows it")
            }
        }
    }
}

impl std::error::Error for ReadingError {}

impl ReadingDocument {
    /// Every source byte lies in exactly one piece, in order. Mirrors
    /// `PresentationPlan::validate_source_partition`; see the module note for
    /// why the model must not shrink the partition to hide something.
    pub fn validate_source_partition(&self) -> Result<(), ReadingError> {
        let mut expected = TextSize::default();
        fn walk(
            blocks: &[ReadingBlock],
            expected: &mut TextSize,
        ) -> Result<(), ReadingError> {
            for block in blocks {
                for piece in &block.pieces {
                    if piece.range.start() < *expected {
                        return Err(ReadingError::Overlap {
                            previous_end: *expected,
                            next: piece.range,
                        });
                    }
                    if piece.range.start() > *expected {
                        return Err(ReadingError::Gap {
                            expected: *expected,
                            actual: piece.range.start(),
                        });
                    }
                    *expected = piece.range.end();
                }
                walk(&block.children, expected)?;
            }
            Ok(())
        }
        walk(&self.roots, &mut expected)?;
        if expected != self.source_len {
            return Err(ReadingError::Gap {
                expected,
                actual: self.source_len,
            });
        }
        Ok(())
    }
}

/// Builds the reading model. Pieces are emitted in source order, and each is
/// attached to the deepest block whose source range contains it; a piece
/// inside no block becomes its own synthetic `Paragraph` so the partition
/// stays complete (blank lines and inter-block whitespace take this path).
pub fn build_reading_document(plan: &PresentationPlan) -> Result<ReadingDocument, ReadingError> {
    // 1. Turn the plan's flat block list into reading kinds, keeping the
    //    parent indices. Levels come from ancestor list-item depth.
    let mut kinds: Vec<ReadingBlockKind> = Vec::with_capacity(plan.blocks.len());
    let mut bullet: Vec<bool> = Vec::with_capacity(plan.blocks.len());
    for (index, block) in plan.blocks.iter().enumerate() {
        if let Some(parent) = block.parent {
            if parent >= index {
                return Err(ReadingError::UnknownParent(index));
            }
        }
        let level = list_depth(plan, index);
        let is_bullet = matches!(block.kind, PresentationBlockKind::ListItem { marker_range })
            if_unordered(plan, marker_range);
        bullet.push(is_bullet);
        kinds.push(match block.kind {
            PresentationBlockKind::Paragraph => ReadingBlockKind::Paragraph,
            PresentationBlockKind::Heading(level) => ReadingBlockKind::Heading(level),
            PresentationBlockKind::ListItem { .. } => {
                if is_bullet {
                    ReadingBlockKind::BulletItem { level }
                } else {
                    ReadingBlockKind::OrderedItem { level }
                }
            }
            PresentationBlockKind::Quote => ReadingBlockKind::Quote,
            PresentationBlockKind::Code => ReadingBlockKind::Code,
            PresentationBlockKind::Table { columns } => ReadingBlockKind::Table { columns },
            PresentationBlockKind::TableRow => ReadingBlockKind::TableRow,
            PresentationBlockKind::TableCell { column, .. } => {
                ReadingBlockKind::TableCell { column }
            }
            PresentationBlockKind::Image => ReadingBlockKind::Image,
        });
    }

    // 2. Bucket every text run into the deepest containing block.
    let mut buckets: Vec<Vec<ReadingPiece>> = vec![Vec::new(); plan.blocks.len()];
    let mut orphans: Vec<ReadingPiece> = Vec::new();
    for item in plan.items.iter() {
        let PresentationItem::TextRun {
            range, role, style, ..
        } = item
        else {
            continue;
        };
        let piece = ReadingPiece {
            range: *range,
            role: *role,
            style: *style,
            emit: emits(plan, *role, *range),
        };
        match deepest_block(plan, *range) {
            Some(index) => buckets[index].push(piece),
            None => orphans.push(piece),
        }
    }

    // 3. Assemble the tree, interleaving orphan pieces in source order so the
    //    partition walk in `validate_source_partition` stays monotone.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); plan.blocks.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (index, block) in plan.blocks.iter().enumerate() {
        match block.parent {
            Some(parent) => children[parent].push(index),
            None => roots.push(index),
        }
    }

    fn assemble(
        index: usize,
        kinds: &[ReadingBlockKind],
        buckets: &mut [Vec<ReadingPiece>],
        children: &[Vec<usize>],
        ranges: &[TextRange],
    ) -> ReadingBlock {
        let kids = children[index]
            .iter()
            .map(|child| assemble(*child, kinds, buckets, children, ranges))
            .collect::<Vec<_>>();
        ReadingBlock {
            kind: kinds[index],
            source_range: ranges[index],
            pieces: std::mem::take(&mut buckets[index]),
            children: kids,
        }
    }

    let ranges: Vec<TextRange> = plan.blocks.iter().map(|block| block.source_range).collect();
    let mut assembled: Vec<ReadingBlock> = Vec::new();
    let mut orphan_iter = orphans.into_iter().peekable();
    for root in roots {
        while let Some(piece) = orphan_iter.peek() {
            if piece.range.start() >= ranges[root].start() {
                break;
            }
            let piece = orphan_iter.next().expect("peeked");
            assembled.push(gap_block(piece));
        }
        assembled.push(assemble(
            root,
            &kinds,
            &mut buckets,
            &children,
            &ranges,
        ));
    }
    for piece in orphan_iter {
        assembled.push(gap_block(piece));
    }

    let document = ReadingDocument {
        roots: assembled,
        source_len: plan.source_len,
    };
    document.validate_source_partition()?;
    Ok(document)
}

/// A run that lies in no parsed block (blank lines, inter-block whitespace).
/// It becomes its own paragraph so the model still covers the source.
fn gap_block(piece: ReadingPiece) -> ReadingBlock {
    ReadingBlock {
        kind: ReadingBlockKind::Paragraph,
        source_range: piece.range,
        pieces: vec![piece],
        children: Vec::new(),
    }
}

/// Whether a run is drawn. Markdown punctuation is suppressed; an ordered list
/// number is content, an unordered bullet character is not.
fn emits(plan: &PresentationPlan, role: TextRole, range: TextRange) -> bool {
    if role.is_syntax_marker() {
        return false;
    }
    if role == TextRole::ListMarker {
        return !has_bullet_decoration(plan, range);
    }
    true
}

/// The compiler already knows which markers are bullets: an unordered item
/// carries a `ListBullet` decoration over the marker's own range. Reading that
/// back keeps the "is this a bullet?" answer in one place.
fn has_bullet_decoration(plan: &PresentationPlan, range: TextRange) -> bool {
    plan.items.iter().any(|item| {
        matches!(
            item,
            PresentationItem::BlockDecoration {
                source_range,
                kind: BlockDecorationKind::ListBullet { .. },
                ..
            } if *source_range == range
        )
    })
}

fn if_unordered(plan: &PresentationPlan, marker_range: TextRange) -> bool {
    has_bullet_decoration(plan, marker_range)
}

/// Nesting depth of block `index`, counting only `ListItem` ancestors.
fn list_depth(plan: &PresentationPlan, index: usize) -> u8 {
    let mut depth: u8 = 0;
    let mut cursor = plan.blocks[index].parent;
    while let Some(parent) = cursor {
        if matches!(plan.blocks[parent].kind, PresentationBlockKind::ListItem { .. }) {
            depth = depth.saturating_add(1);
        }
        cursor = plan.blocks[parent].parent;
    }
    depth
}

/// Index of the innermost block whose source range contains `range`.
fn deepest_block(plan: &PresentationPlan, range: TextRange) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (index, block) in plan.blocks.iter().enumerate() {
        if block.source_range.start() > range.start() || block.source_range.end() < range.end() {
            continue;
        }
        let span = (block.source_range.end().to_usize() - block.source_range.start().to_usize())
            as u32;
        if best.is_none_or(|(_, best_span)| span < best_span) {
            best = Some((index, span));
        }
    }
    best.map(|(index, _)| index)
}
```

**Note on the `is_bullet` line above:** the fragment

```rust
let is_bullet = matches!(block.kind, PresentationBlockKind::ListItem { marker_range })
    if_unordered(plan, marker_range);
```

is deliberately written out longhand in the next step, because `matches!` cannot bind and call like that. Replace it with:

```rust
let is_bullet = match block.kind {
    PresentationBlockKind::ListItem { marker_range } => if_unordered(plan, marker_range),
    _ => false,
};
```

- [ ] **Step 7: Apply the `is_bullet` correction**

Edit `crates/waml-markdown-editor/src/reading/model.rs`, replacing

```rust
        let is_bullet = matches!(block.kind, PresentationBlockKind::ListItem { marker_range })
            if_unordered(plan, marker_range);
```

with

```rust
        let is_bullet = match block.kind {
            PresentationBlockKind::ListItem { marker_range } => if_unordered(plan, marker_range),
            _ => false,
        };
```

- [ ] **Step 8: Run the tests**

Run: `cargo test -p waml-markdown-editor --test reading_model`
Expected: PASS, 9 tests.

If `has_bullet_decoration` returns `false` for every marker, the cause is that `BlockDecorationKind::ListBullet` is currently emitted **only when `styles.hide_syntax` is set** (`compile.rs:57-66`), and this test compiles with `PresentationStyles::balanced()`. The fix is in `compile.rs`: make the `ListBullet` decoration and the `is_unordered_marker` classification **unconditional** (drop `styles.hide_syntax &&` from line 57 only — leave line 528 alone, Task 5 removes it). The decoration is descriptive metadata about the plan; whether anything draws it is the consumer's business. Apply that edit and re-run.

- [ ] **Step 9: Full gate**

Run each and confirm exit 0:

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd editors/vscode; pnpm build; pnpm lint; pnpm test
```

If `cargo fmt --all -- --check` fails, run `cargo fmt --all` and re-check.

- [ ] **Step 10: Delete the npm lockfile and commit**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
rm -f editors/vscode/package-lock.json
git add -A
git status --short
git commit -m "feat(markdown-reading): derive a viewer block model from the presentation plan

The reading view renders markdown as blocks, with no source-line
correspondence; the editor styles raw markdown, where a source line is a
visual row. The split seam is PresentationPlan, so both surfaces share one
parse, one compile, one style table.

ReadingDocument keeps every source byte: a suppressed marker is a piece with
emit == false, never a dropped run, so validate_source_partition still proves
that everything drawn maps back to source."
```

Before committing, confirm `git status --short` lists **no** `editors/vscode/package-lock.json` and **no** `proptest-regressions/`.

---

### Task 2: The `TextFlow` driver widget

The `MarkdownViewer` widget: a `ReadingDocument` -> `TextFlow` driver, plus the bullet decoration. Harness-only — nothing in `waml-editor` uses it yet. This task also lands **every** fork change this plan needs, in one fork commit and one rev bump.

**Files:**
- Create (fork): commit on `C:/dev/makepad` branch `waml`, file `widgets/src/text_flow.rs`
- Modify: `C:/dev/waml/.worktrees/markdown-hide-syntax/Cargo.toml` (rev bump)
- Create: `crates/waml-markdown-editor/src/reading/bullet.rs`
- Create: `crates/waml-markdown-editor/src/reading/widget.rs`
- Create: `crates/waml-markdown-editor/tests/reading_source_map.rs`
- Modify: `crates/waml-markdown-editor/src/reading/mod.rs`
- Modify: `crates/waml-markdown-editor/src/widget.rs`
- Modify: `crates/waml-editor/src/app.rs` (harness surface only, `visible: false`)
- Modify: `crates/waml-editor/src/doc_view.rs`

**Interfaces:**
- Consumes (Task 1): `build_reading_document`, `ReadingDocument`, `ReadingBlock`, `ReadingBlockKind`, `ReadingPiece`.
- Consumes (fork, new in this task):
  ```rust
  impl TextFlow {
      /// The current selection as `(start, end)` byte indices into the
      /// accumulated selection buffer, or `None` when nothing is selected.
      pub fn selection_range(&self) -> Option<(usize, usize)>;

      /// `begin_list_item` without a text marker: reserves a `gutter`-wide
      /// empty box in the hanging-marker column and returns its screen rect so
      /// the caller can draw a DECORATION into it. A reading view must never
      /// draw a bullet as substitute text — a glyph backed by no source range
      /// would break "everything drawn maps back to source".
      pub fn begin_list_item_gutter(&mut self, cx: &mut Cx2d, gutter: f64, pad: f64) -> Rect;
  }
  ```
- Produces, for Tasks 3-4:
  ```rust
  #[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
  pub struct MarkdownViewer { /* ... */ }

  impl MarkdownViewer {
      pub fn install_document(&mut self, cx: &mut Cx, document: Arc<ReadingDocument>, source: Arc<str>);
      pub fn source_map(&self) -> &SourceMap;
  }

  pub trait MarkdownViewerWidgetRefExt {
      fn as_markdown_viewer(&self) -> MarkdownViewerRef;
  }

  impl MarkdownViewerRef {
      pub fn install_document(&self, cx: &mut Cx, document: Arc<ReadingDocument>, source: Arc<str>);
  }

  /// Maps `TextFlow`'s selection-buffer byte indices back to source ranges.
  #[derive(Clone, Debug, Default)]
  pub struct SourceMap { /* ... */ }

  impl SourceMap {
      pub fn clear(&mut self);
      pub fn push(&mut self, flow: std::ops::Range<usize>, source: Option<TextRange>);
      pub fn source_offset(&self, flow_index: usize) -> Option<TextSize>;
      pub fn source_span(&self, flow: std::ops::Range<usize>) -> Option<TextRange>;
      pub fn is_empty(&self) -> bool;
  }
  ```

- [ ] **Step 1: Verify BOTH trees**

Run (PowerShell tool):

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax; git rev-parse --show-toplevel
cd C:/dev/makepad; git rev-parse --show-toplevel; git branch --show-current; git status --short
```

Expected: the worktree path; `C:/dev/makepad`; branch `waml`; a clean fork status. If the fork is dirty or on another branch, STOP and report — do not stash someone else's work.

- [ ] **Step 2: Add `selection_range` to the fork**

Edit `C:/dev/makepad/widgets/src/text_flow.rs`. Immediately after `pub fn has_selection(&self) -> bool {` 's closing brace (the method beginning at line 1344), insert:

```rust
    /// The current selection as `(start, end)` byte indices into the
    /// accumulated selection buffer, normalised so `start <= end`. `None` when
    /// nothing is selected. A host that needs to map a selection back onto its
    /// own source needs the pair, not just the extracted string.
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        if !self.has_selection() {
            return None;
        }
        Some((
            self.selection_anchor.min(self.selection_cursor),
            self.selection_anchor.max(self.selection_cursor),
        ))
    }
```

- [ ] **Step 3: Add `begin_list_item_gutter` to the fork**

Edit `C:/dev/makepad/widgets/src/text_flow.rs`. Immediately before `pub fn end_list_item(&mut self, cx: &mut Cx2d) {`, insert:

```rust
    /// `begin_list_item` for callers that draw their marker as a DECORATION
    /// rather than as text. Reserves a `gutter`-wide empty box in the hanging
    /// marker column and returns its screen rect. `begin_list_item` draws its
    /// `dot` argument through `draw_text`, which puts a glyph in the selection
    /// buffer backed by no host source range; a reading view cannot do that
    /// without breaking "everything drawn maps back to source".
    pub fn begin_list_item_gutter(&mut self, cx: &mut Cx2d, gutter: f64, pad: f64) -> Rect {
        let fs = *self.font_sizes.last().unwrap_or(&self.font_size);
        let font_based_padding = fs as f64 * pad;

        cx.begin_turtle(
            self.list_item_walk,
            Layout {
                padding: Inset {
                    left: self.list_item_layout.padding.left + font_based_padding,
                    ..self.list_item_layout.padding
                },
                ..self.list_item_layout
            },
        );

        cx.turtle_mut()
            .move_right_down(dvec2(-font_based_padding, 0.0));

        let gutter_rect = TextFlow::walk_margin(cx, gutter);
        TextFlow::walk_margin(cx, self.list_item_marker_pad);

        // Match `begin_list_item`: wrapped rows align with the text after the
        // marker column, not with the marker itself.
        let actual_indent = cx.turtle().pos().x - cx.turtle().origin().x;
        cx.turtle_mut().set_padding_left(actual_indent);
        // Deliberately NOT pushed onto `area_stack` (see `begin_list_item`).
        gutter_rect
    }
```

- [ ] **Step 4: Build the fork and commit it**

```bash
cd C:/dev/makepad
cargo check -p makepad-widgets
git add widgets/src/text_flow.rs
git commit -m "feat(text_flow): expose the selection pair and a decoration gutter for list items

selection_range() gives a host the (start, end) buffer indices it needs to map
a selection back onto its own source; selected_text() alone loses the offsets.

begin_list_item_gutter() reserves the hanging-marker column and returns its
rect instead of drawing a text bullet, so a host can draw a marker decoration
without pushing a glyph that no source range backs."
git rev-parse HEAD
```

Expected: `cargo check` exit 0; note the printed SHA — call it `<NEW_SHA>`.

- [ ] **Step 5: Bump the rev in this repo**

Run to find every pin:

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
grep -n 'redoz/makepad' Cargo.toml
```

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/Cargo.toml`, replacing every occurrence of the old rev string with `<NEW_SHA>` (the full 40-character SHA — **never a branch name**; a branch pin silently drifts under the build).

Then: `cargo check -p waml-markdown-editor` — expected exit 0, and `Cargo.lock` updates.

- [ ] **Step 6: Write the `SourceMap` tests**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/tests/reading_source_map.rs`:

```rust
//! `SourceMap` maps `TextFlow`'s selection-buffer indices back to source.
//!
//! TextFlow's selection index space is its own accumulated `SelectionTracker`
//! text buffer, NOT source offsets: it holds only the runs that were drawn,
//! plus structural newlines it injects itself. Anything that carries a viewer
//! selection back to the editor has to translate.

use waml_markdown_editor::reading::SourceMap;
use waml_syntax::{TextRange, TextSize};

fn size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).expect("in range")
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(size(start), size(end)).expect("ordered")
}

fn map() -> SourceMap {
    // Renders "# Title\n\nBody\n" as "Title" + "\n" + "Body": the `# ` marker
    // and the blank line never reach the flow buffer.
    let mut map = SourceMap::default();
    map.push(0..5, Some(range(2, 7))); // "Title"
    map.push(5..6, None); // structural newline
    map.push(6..10, Some(range(9, 13))); // "Body"
    map
}

#[test]
fn an_empty_map_reports_itself_empty() {
    assert!(SourceMap::default().is_empty());
    assert_eq!(SourceMap::default().source_offset(0), None);
}

#[test]
fn a_flow_index_inside_a_piece_maps_to_the_matching_source_offset() {
    let map = map();
    assert_eq!(map.source_offset(0), Some(size(2)), "start of the run");
    assert_eq!(map.source_offset(3), Some(size(5)), "offset within the run");
    assert_eq!(map.source_offset(6), Some(size(9)), "start of the next run");
}

#[test]
fn an_index_in_a_structural_gap_falls_forward_to_the_next_real_piece() {
    let map = map();
    assert_eq!(
        map.source_offset(5),
        Some(size(9)),
        "a newline TextFlow injected has no source of its own; the caret belongs \
         to the next drawn run"
    );
}

#[test]
fn an_index_past_the_end_maps_to_the_end_of_the_last_real_piece() {
    let map = map();
    assert_eq!(map.source_offset(10), Some(size(13)));
    assert_eq!(map.source_offset(999), Some(size(13)));
}

#[test]
fn a_flow_span_becomes_the_enclosing_source_span() {
    let map = map();
    assert_eq!(
        map.source_span(0..10),
        Some(range(2, 13)),
        "a selection over both runs spans the source between them, including \
         the punctuation it skipped over"
    );
    assert_eq!(map.source_span(1..4), Some(range(3, 6)));
}

#[test]
fn a_span_entirely_inside_a_gap_has_no_source_span() {
    let mut map = SourceMap::default();
    map.push(0..1, None);
    assert_eq!(map.source_span(0..1), None);
}

#[test]
fn clear_resets_the_map() {
    let mut map = map();
    map.clear();
    assert!(map.is_empty());
}
```

- [ ] **Step 7: Run them and watch them fail**

Run: `cargo test -p waml-markdown-editor --test reading_source_map`
Expected: FAIL — `cannot find type SourceMap`.

- [ ] **Step 8: Write the bullet decoration**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/reading/bullet.rs`:

```rust
//! The list-item marker of the reading view, drawn as a DECORATION.
//!
//! A bullet is never substitute text. A glyph backed by no source range would
//! break the invariant that everything drawn maps back to source, so the
//! marker is a shape the viewer draws into the gutter `TextFlow` reserved.
//!
//! Shape varies with nesting depth, which is what makes a nested list legible:
//! disc, then ring, then square, cycling.

use makepad_widgets::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BulletShape {
    Disc,
    Ring,
    Square,
}

/// The shape for a list item at nesting depth `level`.
pub fn bullet_shape_for_level(level: u8) -> BulletShape {
    match level % 3 {
        0 => BulletShape::Disc,
        1 => BulletShape::Ring,
        _ => BulletShape::Square,
    }
}

impl BulletShape {
    /// The `shape` uniform the shader switches on.
    pub fn shader_index(self) -> f32 {
        match self {
            Self::Disc => 0.0,
            Self::Ring => 1.0,
            Self::Square => 2.0,
        }
    }
}

script_mod! {
    pub DrawReadingBullet = {{DrawReadingBullet}} {}
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawReadingBullet {
    #[deref]
    pub draw_super: DrawQuad,
    /// 0 = disc, 1 = ring, 2 = square. See `BulletShape::shader_index`.
    #[live(0.0)]
    pub shape: f32,
    #[live]
    pub color: Vec4f,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nesting_depth_cycles_the_bullet_shape() {
        assert_eq!(bullet_shape_for_level(0), BulletShape::Disc);
        assert_eq!(bullet_shape_for_level(1), BulletShape::Ring);
        assert_eq!(bullet_shape_for_level(2), BulletShape::Square);
        assert_eq!(
            bullet_shape_for_level(3),
            BulletShape::Disc,
            "deep nesting cycles rather than running out of shapes"
        );
    }

    #[test]
    fn every_shape_has_a_distinct_shader_index() {
        let mut indices = [
            BulletShape::Disc.shader_index(),
            BulletShape::Ring.shader_index(),
            BulletShape::Square.shader_index(),
        ];
        indices.sort_by(f32::total_cmp);
        assert_eq!(indices, [0.0, 1.0, 2.0]);
    }
}
```

Then add the shader body to the `script_mod!` block. Replace `pub DrawReadingBullet = {{DrawReadingBullet}} {}` with:

```rust
    pub DrawReadingBullet = {{DrawReadingBullet}} {
        // `sdf.box(.., 0)` floods the quad in this fork -- use `sdf.rect` for
        // the square case rather than a zero-radius box.
        pixel: fn() {
            let sdf = Sdf2d::viewport(self.pos * self.rect_size)
            let r = min(self.rect_size.x, self.rect_size.y) * 0.5
            let c = self.rect_size * 0.5
            if self.shape < 0.5 {
                sdf.circle(c.x, c.y, r)
                sdf.fill(self.color)
            } else if self.shape < 1.5 {
                sdf.circle(c.x, c.y, r)
                sdf.stroke(self.color, max(1.0, r * 0.4))
            } else {
                sdf.rect(c.x - r, c.y - r, r * 2.0, r * 2.0)
                sdf.fill(self.color)
            }
            return sdf.result
        }
    }
```

- [ ] **Step 9: Write the driver widget**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/reading/widget.rs`:

```rust
//! `MarkdownViewer`: a `ReadingDocument` -> `TextFlow` driver.
//!
//! This widget owns NO layout engine. `TextFlow` already provides block flow,
//! selection, copy and `point_to_index`; the driver's whole job is to walk the
//! reading model and issue the matching `TextFlow` calls, drawing markers as
//! decorations and recording a `SourceMap` as it goes.
//!
//! makepad's own `Markdown` widget was rejected: it calls `pulldown_cmark`
//! itself, which would mean two independent parses of one document, and it
//! cannot see `MarkdownDialect`.

use std::{ops::Range, sync::Arc};

use makepad_widgets::*;
use waml_syntax::{TextRange, TextSize};

use crate::presentation::{ColorRole, FontSizeRole, TextRole};

use super::{
    bullet::{bullet_shape_for_level, DrawReadingBullet},
    ReadingBlock, ReadingBlockKind, ReadingDocument, ReadingPiece,
};

script_mod! {
    use link.widgets.*;

    pub MarkdownViewer = {{MarkdownViewer}} {
        width: Fill,
        height: Fill,
        flow: Down,
        flow_body: <TextFlow> {
            width: Fill,
            height: Fit,
            selectable: true,
        }
        draw_bullet: {}
    }
}

/// One contiguous stretch of `TextFlow`'s selection buffer and the source it
/// came from. `source: None` marks a structural gap TextFlow injected itself
/// (`push_newline` from `end_list_item`, `end_quote`, `end_code`,
/// `new_line_collapsed`), which no source byte backs.
#[derive(Clone, Copy, Debug, PartialEq)]
struct MapPiece {
    flow: Range<usize>,
    source: Option<TextRange>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    pieces: Vec<MapPiece>,
}

impl SourceMap {
    pub fn clear(&mut self) {
        self.pieces.clear();
    }

    pub fn push(&mut self, flow: Range<usize>, source: Option<TextRange>) {
        if flow.is_empty() {
            return;
        }
        self.pieces.push(MapPiece { flow, source });
    }

    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// The source offset a flow index points at. An index inside a structural
    /// gap falls FORWARD to the next real piece; an index past the end lands
    /// on the end of the last real piece.
    pub fn source_offset(&self, flow_index: usize) -> Option<TextSize> {
        let mut last_end = None;
        for piece in &self.pieces {
            if let Some(source) = piece.source {
                if flow_index < piece.flow.end {
                    let within = flow_index.saturating_sub(piece.flow.start);
                    let offset = source.start().to_usize() + within;
                    return TextSize::try_from_usize(offset.min(source.end().to_usize())).ok();
                }
                last_end = Some(source.end());
            } else if flow_index < piece.flow.end {
                // Inside a gap: fall forward to the next real piece.
                return self
                    .pieces
                    .iter()
                    .find(|next| next.flow.start >= piece.flow.end && next.source.is_some())
                    .and_then(|next| next.source.map(|source| source.start()))
                    .or(last_end);
            }
        }
        last_end
    }

    /// The source span a flow span covers, or `None` when the span touches no
    /// source-backed piece.
    pub fn source_span(&self, flow: Range<usize>) -> Option<TextRange> {
        let mut start: Option<TextSize> = None;
        let mut end: Option<TextSize> = None;
        for piece in &self.pieces {
            let Some(source) = piece.source else { continue };
            if piece.flow.end <= flow.start || piece.flow.start >= flow.end {
                continue;
            }
            let lead = flow.start.saturating_sub(piece.flow.start);
            let trail = piece.flow.end.saturating_sub(flow.end);
            let piece_start = (source.start().to_usize() + lead).min(source.end().to_usize());
            let piece_end = source
                .end()
                .to_usize()
                .saturating_sub(trail)
                .max(piece_start);
            let piece_start = TextSize::try_from_usize(piece_start).ok()?;
            let piece_end = TextSize::try_from_usize(piece_end).ok()?;
            start = Some(start.map_or(piece_start, |value: TextSize| value.min(piece_start)));
            end = Some(end.map_or(piece_end, |value: TextSize| value.max(piece_end)));
        }
        TextRange::new(start?, end?).ok()
    }
}

#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct MarkdownViewer {
    #[deref]
    view: View,
    #[live]
    draw_bullet: DrawReadingBullet,
    /// Side of the bullet, as a fraction of the body font size.
    #[live(0.30)]
    bullet_scale: f64,
    /// Width of the hanging-marker column, as a fraction of the font size.
    #[live(1.2)]
    bullet_gutter_scale: f64,
    /// `begin_list_item_gutter`'s `pad`, in font-size multiples.
    #[live(1.0)]
    list_indent_scale: f64,

    #[rust]
    document: Option<Arc<ReadingDocument>>,
    #[rust]
    source: Option<Arc<str>>,
    #[rust]
    source_map: SourceMap,
}

impl MarkdownViewer {
    pub fn install_document(
        &mut self,
        cx: &mut Cx,
        document: Arc<ReadingDocument>,
        source: Arc<str>,
    ) {
        self.document = Some(document);
        self.source = Some(source);
        self.source_map.clear();
        self.redraw(cx);
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    fn flow(&self) -> TextFlowRef {
        self.view.text_flow(id!(flow_body))
    }

    /// Draws one piece and records its flow span. `TextFlow::draw_text` trims
    /// its input (leading whitespace when it is first on a line, and any
    /// trailing newlines) BEFORE pushing it into the selection buffer, so the
    /// driver trims first and adjusts the recorded range to match. Without
    /// that, the map silently desynchronises by the trimmed byte count.
    fn draw_piece(
        flow: &mut TextFlow,
        map: &mut SourceMap,
        cx: &mut Cx2d,
        source: &str,
        piece: &ReadingPiece,
    ) {
        if !piece.emit {
            return;
        }
        let start = piece.range.start().to_usize();
        let end = piece.range.end().to_usize();
        let Some(raw) = source.get(start..end) else {
            return;
        };
        let trimmed = raw.trim_start().trim_end_matches('\n');
        if trimmed.is_empty() {
            return;
        }
        let lead = raw.len() - raw.trim_start().len();
        let range = TextRange::new(
            TextSize::try_from_usize(start + lead).expect("in range"),
            TextSize::try_from_usize(start + lead + trimmed.len()).expect("in range"),
        )
        .expect("ordered");

        let before = flow.selection_tracker.total_len();
        flow.draw_text(cx, trimmed);
        let after = flow.selection_tracker.total_len();
        debug_assert_eq!(
            after - before,
            trimmed.len(),
            "TextFlow reshaped the run; the source map would drift"
        );
        map.push(before..after, Some(range));
    }

    fn draw_block(&mut self, cx: &mut Cx2d, block: &ReadingBlock, source: &str) {
        let flow_ref = self.flow();
        let Some(mut flow) = flow_ref.borrow_mut() else {
            return;
        };
        match block.kind {
            ReadingBlockKind::Heading(level) => {
                let scale = match level {
                    1 => 1.8,
                    2 => 1.5,
                    3 => 1.3,
                    4 => 1.15,
                    5 => 1.05,
                    _ => 1.0,
                };
                flow.push_size_abs_scale(scale);
                flow.bold.push();
                for piece in &block.pieces {
                    Self::draw_piece(&mut flow, &mut self.source_map, cx, source, piece);
                }
                flow.bold.pop();
                flow.font_sizes.pop();
                flow.new_line_collapsed(cx);
                self.source_map.push(
                    flow.selection_tracker.total_len() - 1..flow.selection_tracker.total_len(),
                    None,
                );
            }
            ReadingBlockKind::BulletItem { level } | ReadingBlockKind::OrderedItem { level } => {
                let font_size = flow.font_size as f64;
                let gutter = font_size * self.bullet_gutter_scale;
                let rect =
                    flow.begin_list_item_gutter(cx, gutter, self.list_indent_scale * level as f64);
                if matches!(block.kind, ReadingBlockKind::BulletItem { .. }) {
                    let size = font_size * self.bullet_scale;
                    self.draw_bullet.shape = bullet_shape_for_level(level).shader_index();
                    self.draw_bullet.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(
                                rect.pos.x + (gutter - size) * 0.5,
                                rect.pos.y + (rect.size.y - size) * 0.5,
                            ),
                            size: dvec2(size, size),
                        },
                    );
                }
                for piece in &block.pieces {
                    Self::draw_piece(&mut flow, &mut self.source_map, cx, source, piece);
                }
                drop(flow);
                self.draw_children(cx, block, source);
                let Some(mut flow) = flow_ref.borrow_mut() else {
                    return;
                };
                let before = flow.selection_tracker.total_len();
                flow.end_list_item(cx);
                let after = flow.selection_tracker.total_len();
                self.source_map.push(before..after, None);
                return;
            }
            ReadingBlockKind::Quote => {
                flow.begin_quote(cx);
                for piece in &block.pieces {
                    Self::draw_piece(&mut flow, &mut self.source_map, cx, source, piece);
                }
                drop(flow);
                self.draw_children(cx, block, source);
                let Some(mut flow) = flow_ref.borrow_mut() else {
                    return;
                };
                let before = flow.selection_tracker.total_len();
                flow.end_quote(cx);
                let after = flow.selection_tracker.total_len();
                self.source_map.push(before..after, None);
                return;
            }
            ReadingBlockKind::Code => {
                flow.begin_code(cx);
                flow.fixed.push();
                for piece in &block.pieces {
                    Self::draw_piece(&mut flow, &mut self.source_map, cx, source, piece);
                }
                flow.fixed.pop();
                let before = flow.selection_tracker.total_len();
                flow.end_code(cx);
                let after = flow.selection_tracker.total_len();
                self.source_map.push(before..after, None);
            }
            ReadingBlockKind::ThematicBreak => {
                flow.sep(cx);
            }
            _ => {
                for piece in &block.pieces {
                    let style_is_code = matches!(piece.style.size, FontSizeRole::Code)
                        || piece.role == TextRole::InlineCode;
                    let emphasised = piece.style.italic;
                    let strong = matches!(
                        piece.role,
                        TextRole::Strong | TextRole::StrongEmphasis
                    );
                    if style_is_code {
                        flow.inline_code.push();
                        flow.fixed.push();
                    }
                    if emphasised {
                        flow.italic.push();
                    }
                    if strong {
                        flow.bold.push();
                    }
                    Self::draw_piece(&mut flow, &mut self.source_map, cx, source, piece);
                    if strong {
                        flow.bold.pop();
                    }
                    if emphasised {
                        flow.italic.pop();
                    }
                    if style_is_code {
                        flow.fixed.pop();
                        flow.inline_code.pop();
                    }
                }
                if !block.pieces.is_empty() {
                    let before = flow.selection_tracker.total_len();
                    flow.new_line_collapsed(cx);
                    let after = flow.selection_tracker.total_len();
                    self.source_map.push(before..after, None);
                }
            }
        }
        drop(flow);
        self.draw_children(cx, block, source);
    }

    fn draw_children(&mut self, cx: &mut Cx2d, block: &ReadingBlock, source: &str) {
        // `children` is cloned so the recursive borrow of `self` is legal. A
        // reading model is per-revision and editor-sized, so the clone is not
        // on any hot path; `install_document` runs once per compile.
        let children = block.children.clone();
        for child in &children {
            self.draw_block(cx, child, source);
        }
    }
}

impl Widget for MarkdownViewer {
    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let (Some(document), Some(source)) = (self.document.clone(), self.source.clone()) else {
            return self.view.draw_walk(cx, scope, walk);
        };
        self.source_map.clear();
        let flow_ref = self.flow();
        if let Some(mut flow) = flow_ref.borrow_mut() {
            flow.begin(cx, walk);
        }
        for block in &document.roots {
            self.draw_block(cx, block, &source);
        }
        if let Some(mut flow) = flow_ref.borrow_mut() {
            flow.end(cx);
        }
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Selection, copy and point_to_index are TextFlow's, not ours.
        self.view.handle_event(cx, event, scope);
    }
}

impl MarkdownViewerRef {
    pub fn install_document(
        &self,
        cx: &mut Cx,
        document: Arc<ReadingDocument>,
        source: Arc<str>,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.install_document(cx, document, source);
        }
    }

    pub fn selected_source_span(&self) -> Option<TextRange> {
        let inner = self.borrow()?;
        let flow = inner.flow();
        let flow = flow.borrow()?;
        let (start, end) = flow.selection_range()?;
        inner.source_map.source_span(start..end)
    }
}

pub trait MarkdownViewerWidgetRefExt {
    fn as_markdown_viewer(&self) -> MarkdownViewerRef;
}

impl MarkdownViewerWidgetRefExt for WidgetRef {
    fn as_markdown_viewer(&self) -> MarkdownViewerRef {
        MarkdownViewerRef(self.clone())
    }
}
```

**Adaptation note:** the exact `script_mod!`, `Widget`, and `WidgetRef` boilerplate must match this repo's house pattern. Before writing this file, read `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/icon_button.rs` end-to-end — it is the smallest complete widget in the tree — and mirror its `script_mod!` block shape, its `Widget` impl, its `*Ref` extension trait, and its `#[live]`/`#[rust]` attribute usage exactly. Where this plan's code and that pattern disagree, **the repo pattern wins**; the logic above is the contract, the boilerplate is not.

- [ ] **Step 10: Export the new items**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/reading/mod.rs`, replacing its `pub mod model;` / `pub use` block with:

```rust
pub mod bullet;
pub mod model;
pub mod widget;

pub use bullet::{bullet_shape_for_level, BulletShape, DrawReadingBullet};
pub use model::{
    build_reading_document, ReadingBlock, ReadingBlockKind, ReadingDocument, ReadingError,
    ReadingPiece,
};
pub use widget::{MarkdownViewer, MarkdownViewerRef, MarkdownViewerWidgetRefExt, SourceMap};

pub fn script_mod(vm: &mut makepad_widgets::ScriptVm) -> makepad_widgets::ScriptValue {
    bullet::script_mod(vm);
    widget::script_mod(vm)
}
```

- [ ] **Step 11: Register the widgets**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/widget.rs`, changing `register_script_mod` to:

```rust
pub(crate) fn register_script_mod(vm: &mut ScriptVm) -> ScriptValue {
    // A child widget is dead and invisible unless its script_mod registers
    // BEFORE its consumer's, so the bullet and the viewer go first.
    crate::reading::script_mod(vm);
    script_mod(vm)
}
```

- [ ] **Step 12: Run the `SourceMap` and bullet tests**

Run:

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo test -p waml-markdown-editor --test reading_source_map
cargo test -p waml-markdown-editor reading::bullet
```

Expected: PASS, 7 tests and 2 tests.

- [ ] **Step 13: Add the harness surface to the app DSL**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/app.rs`. Immediately after the `markdown_surface := View{ ... }` block's closing brace, add a sibling:

```rust
                                // Reading view: the concept surface. Mutually
                                // exclusive with `markdown_surface`, which is
                                // the raw-markdown editor.
                                markdown_viewer_surface := View{
                                    width: Fill
                                    height: Fill
                                    visible: false
                                    show_bg: true
                                    draw_bg +: {
                                        color: atlas.surface
                                        pixel: fn() {
                                            return vec4(self.color.rgb * self.color.a, self.color.a)
                                        }
                                    }
                                    flow: Down
                                    viewer := MarkdownViewer{
                                        width: Fill
                                        height: Fill
                                        draw_bullet +: { color: atlas.text }
                                        flow_body +: {
                                            font_color: atlas.text
                                        }
                                    }
                                }
```

- [ ] **Step 14: Give `BodyWidgets` the viewer handle**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/doc_view.rs`:

1. Extend the import line to
   ```rust
   use waml_markdown_editor::widget::{MarkdownEditorRef, MarkdownEditorWidgetRefExt};
   use waml_markdown_editor::reading::{MarkdownViewerRef, MarkdownViewerWidgetRefExt};
   ```
2. Add a field beside `markdown_editor: MarkdownEditorRef,`:
   ```rust
   markdown_viewer: MarkdownViewerRef,
   ```
3. Beside the existing `markdown_editor: ui.widget(...).as_markdown_editor(),` initialiser, add:
   ```rust
   markdown_viewer: ui
       .widget(cx, ids!(markdown_viewer_surface.viewer))
       .as_markdown_viewer(),
   ```
   (match the existing initialiser's exact `ids!` path shape; read the surrounding lines first).
4. Add, next to `show_markdown_editor`:
   ```rust
   /// Show the reading view. Mutually exclusive with the markdown editor and
   /// the canvas: a concept is either being read or being edited.
   pub fn show_markdown_viewer(&self, cx: &mut Cx) {
       self.ui
           .widget(cx, ids!(markdown_viewer_surface))
           .set_visible(cx, true);
       self.ui
           .widget(cx, ids!(markdown_surface))
           .set_visible(cx, false);
       self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
       self.set_canvas_interaction_enabled(cx, false);
   }

   pub fn markdown_viewer(&self) -> MarkdownViewerRef {
       self.markdown_viewer.clone()
   }
   ```
5. In `show_markdown_editor` **and** `show_canvas`, add the line that hides the viewer:
   ```rust
   self.ui
       .widget(cx, ids!(markdown_viewer_surface))
       .set_visible(cx, false);
   ```

- [ ] **Step 15: Full gate**

Run each and confirm exit 0:

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd editors/vscode; pnpm build; pnpm lint; pnpm test
```

- [ ] **Step 16: Visual check — the widget draws at all**

The viewer is not wired to any view yet, so the check at this stage is only that the app **still boots and renders** with the new surface and the new script_mod registration in place. A widget missing from `script_mod` is silently dead: no draw, no hit, `ids!()` empty, and the gate stays green — only the screenshot catches it.

Run in ONE PowerShell tool call (launch by pid, capture by pid, kill by pid — **never `Stop-Process -Name`**, which kills the user's own editor):

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo build -p waml-editor --bin waml-editor
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$p = Start-Process -PassThru -FilePath ".\target\debug\waml-editor.exe" -ArgumentList "crates/waml-editor/tests/fixtures/okf-only"
Start-Sleep -Seconds 20
pwsh -File scripts/capture-window.ps1 -Out "$env:TEMP\viewer-task2.png" -ProcessId $p.Id
Stop-Process -Id $p.Id -Force
```

Then `Read` `%TEMP%\viewer-task2.png`. Expected: the editor window renders normally with `notes.md` open, all chrome intact. If the window is blank or the caption text vanished, the `script_mod!` namespace is malformed (it must be ONE object literal, not field-by-field) — fix that before proceeding.

- [ ] **Step 17: Delete the npm lockfile and commit**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
rm -f editors/vscode/package-lock.json
git add -A
git status --short
git commit -m "feat(markdown-reading): add the TextFlow driver widget and its source map

MarkdownViewer walks a ReadingDocument and issues TextFlow calls; TextFlow
already supplies block flow, selection, copy and point_to_index, so the driver
owns no layout engine of its own.

Markers are decorations, never substitute text: makepad's begin_list_item
draws its dot through draw_text, which would put a glyph in the selection
buffer that no source range backs. The fork gains begin_list_item_gutter,
which reserves the marker column and hands back its rect instead.

TextFlow's selection indices address its own accumulated buffer, not source
offsets, and draw_text trims its input before recording it, so the driver
trims first and keeps a SourceMap with a debug assertion on the length delta."
```

---

### Task 3: Carrying a viewer selection back to source

Wires `SourceMap` into a real, testable behaviour: the viewer reports which source range the user has selected, and answers a screen point with a source offset. This is what makes the source toggle in Task 4 land the caret in the right place.

**Files:**
- Modify: `crates/waml-markdown-editor/src/reading/widget.rs`
- Modify: `crates/waml-markdown-editor/tests/reading_source_map.rs`

**Interfaces:**
- Consumes (Task 2): `SourceMap`, `MarkdownViewer`, `MarkdownViewerRef`, and the fork's `TextFlow::selection_range`.
- Produces, for Task 4:
  ```rust
  #[derive(Clone, Copy, Debug, Default, PartialEq)]
  pub enum MarkdownViewerAction {
      /// The reader asked to see this document as markdown source. The offset
      /// is where the caret should land in the editor.
      SourceRequested { caret: TextSize },
      #[default]
      None,
  }

  impl MarkdownViewerRef {
      /// The source range the user has selected, or `None`.
      pub fn selected_source_span(&self) -> Option<TextRange>;
      /// The source offset under a screen point, for a caret handoff.
      pub fn source_offset_at(&self, cx: &Cx, point: DVec2) -> Option<TextSize>;
      /// The source offset the caret should carry into the editor: the start
      /// of the selection, or 0 when nothing is selected.
      pub fn caret_for_handoff(&self) -> TextSize;
  }
  ```

- [ ] **Step 1: Verify the worktree**

Run: `cd C:/dev/waml/.worktrees/markdown-hide-syntax; git rev-parse --show-toplevel`
Expected: `C:/dev/waml/.worktrees/markdown-hide-syntax`.

- [ ] **Step 2: Write the failing tests**

Append to `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/tests/reading_source_map.rs`:

```rust
#[test]
fn a_handoff_caret_defaults_to_the_start_of_the_document() {
    // No selection means the reader pressed the source toggle without
    // pointing at anything; the editor opens at the top.
    assert_eq!(
        waml_markdown_editor::reading::caret_for_span(None),
        size(0)
    );
}

#[test]
fn a_handoff_caret_is_the_start_of_the_selection() {
    assert_eq!(
        waml_markdown_editor::reading::caret_for_span(Some(range(9, 13))),
        size(9),
        "the editor opens where the reader was looking"
    );
}

#[test]
fn a_selection_that_spans_suppressed_punctuation_still_yields_one_source_span() {
    // Renders "**bold** tail" as "bold" + " tail": the two `**` runs never
    // reach the flow buffer, but a selection across the whole line must map
    // back onto a contiguous source range that includes them.
    let mut map = SourceMap::default();
    map.push(0..4, Some(range(2, 6))); // "bold", source 2..6
    map.push(4..9, Some(range(8, 13))); // " tail", source 8..13
    assert_eq!(
        map.source_span(0..9),
        Some(range(2, 13)),
        "the hidden `**` at 6..8 lies inside the span, not outside it"
    );
}
```

- [ ] **Step 3: Run them and watch them fail**

Run: `cargo test -p waml-markdown-editor --test reading_source_map`
Expected: FAIL — `cannot find function caret_for_span`.

- [ ] **Step 4: Add the handoff helpers**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/reading/widget.rs`. Add, after the `impl SourceMap` block:

```rust
/// The caret a source handoff should carry into the editor: the start of the
/// reader's selection, or the top of the document when nothing is selected.
/// Free-standing so it is testable without a live widget tree.
pub fn caret_for_span(span: Option<TextRange>) -> TextSize {
    span.map(|span| span.start()).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MarkdownViewerAction {
    /// The reader asked to see this document as markdown source. `caret` is
    /// where the editor should place its caret.
    SourceRequested { caret: TextSize },
    #[default]
    None,
}
```

Add to `impl MarkdownViewerRef` (`selected_source_span` already exists from Task 2):

```rust
    /// The source offset under a screen point. `TextFlow::point_to_index`
    /// answers in its own buffer's index space, so the map translates.
    pub fn source_offset_at(&self, cx: &Cx, point: DVec2) -> Option<TextSize> {
        let inner = self.borrow()?;
        let flow = inner.flow();
        let flow = flow.borrow()?;
        let index = flow.selection_tracker.point_to_index(cx, point)?;
        inner.source_map.source_offset(index)
    }

    /// The caret a source handoff carries into the editor.
    pub fn caret_for_handoff(&self) -> TextSize {
        caret_for_span(self.selected_source_span())
    }
```

Export the new items: edit `crates/waml-markdown-editor/src/reading/mod.rs`, changing the `pub use widget::{...}` line to

```rust
pub use widget::{
    caret_for_span, MarkdownViewer, MarkdownViewerAction, MarkdownViewerRef,
    MarkdownViewerWidgetRefExt, SourceMap,
};
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p waml-markdown-editor --test reading_source_map`
Expected: PASS, 10 tests.

- [ ] **Step 6: Full gate**

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd editors/vscode; pnpm build; pnpm lint; pnpm test
```

`clippy` will fail with `dead_code` on `MarkdownViewerAction` and `source_offset_at` if nothing reads them yet and the gate denies warnings. If it does, do **not** add `#[allow(dead_code)]` — the constraint forbids landing unread items. Instead, move `MarkdownViewerAction` and `source_offset_at` into **Task 4**, where the toggle consumes them, and land this task with `caret_for_span`, `selected_source_span` and `caret_for_handoff` only (all three are covered by the tests above). Note the move in the commit body.

- [ ] **Step 7: Delete the npm lockfile and commit**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
rm -f editors/vscode/package-lock.json
git add -A
git status --short
git commit -m "feat(markdown-reading): map a viewer selection back onto source offsets

TextFlow answers selection and point_to_index in its own accumulated buffer's
index space, which holds only the runs the reading view drew plus the newlines
TextFlow injected. The source map translates, so a selection that spans
suppressed punctuation still yields one contiguous source range, and a source
handoff can put the editor's caret where the reader was looking."
```

---

### Task 4: Wire `GenericOkfView` to the viewer

The OKF concept view stops being a syntax-hiding editor and becomes the reading view, with an explicit toggle to the markdown source.

**Note on scope:** the toggle switches *rendering*, not writability. `GenericOkfView::handle` discards `outcome.source_edit`, so a writable editor would silently drop edits. The editor side of the toggle stays read-only. Making concepts writable is a follow-up, explicitly out of scope here.

**Files:**
- Create: `crates/waml-editor/src/reading_view.rs`
- Create: `crates/waml-editor/tests/fixtures/okf-only/reading.md`
- Modify: `crates/waml-editor/src/lib.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Modify: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes (Tasks 1-3): `build_reading_document`, `ReadingDocument`, `MarkdownViewerRef`, `MarkdownViewerWidgetRefExt`, `caret_for_span`, `BodyWidgets::show_markdown_viewer`, `BodyWidgets::markdown_viewer`.
- Consumes (existing): `SourceView::resolve_document`, `compile_presentation`, `PresentationStyles::balanced`, `WamlCodeHighlightHost::registry`, `BodyWidgets::show_markdown_editor`.
- Produces:
  ```rust
  pub struct ReadingView { /* ... */ }

  impl ReadingView {
      pub fn new_with_asset_host(key: String, assets: SharedMarkdownAssetHost) -> ReadingView;
      pub fn install_snapshot(&mut self, cx: &mut Cx, body: &BodyWidgets, snapshot: &EditorSessionSnapshot);
      pub fn showing_source(&self) -> bool;
      pub fn set_showing_source(&mut self, showing: bool);
  }
  ```

- [ ] **Step 1: Verify the worktree**

Run: `cd C:/dev/waml/.worktrees/markdown-hide-syntax; git rev-parse --show-toplevel`

- [ ] **Step 2: Add the rich fixture**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/tests/fixtures/okf-only/reading.md`:

```markdown
---
title: Reading View
---

# Reading View

A concept opens as prose to read. Markdown punctuation is *rendered*, not
shown, and `inline code` keeps its own face.

## Nesting

- outer bullet
  - inner bullet
    - deepest bullet
- second outer

1. first ordered
2. second ordered

> A quote wraps the paragraph it contains, and its angle bracket is
> punctuation the reader never sees.

```rust
fn main() {
    println!("fenced code keeps its content");
}
```

| Column | Meaning |
| ------ | ------- |
| one    | first   |
| two    | second  |
```

- [ ] **Step 3: Write the failing tests**

Add to `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/generic_okf_view.rs`, inside `mod tests`, replacing the existing `opening_a_concept_hides_markdown_syntax_so_editing_stays_an_explicit_action` test:

```rust
    #[test]
    fn a_concept_opens_in_the_reading_view() {
        let view = generic_view();
        assert!(
            !view.showing_source(),
            "a concept opens as rendered prose, not as markdown to edit"
        );
    }

    #[test]
    fn the_source_toggle_switches_between_the_viewer_and_the_editor() {
        let mut view = generic_view();
        view.toggle_source();
        assert!(view.showing_source(), "the toggle reveals the markdown source");
        view.toggle_source();
        assert!(!view.showing_source(), "and puts it back");
    }
```

- [ ] **Step 4: Run them and watch them fail**

Run: `cargo test -p waml-editor generic_okf_view`
Expected: FAIL — `no method named showing_source`.

- [ ] **Step 5: Write `ReadingView`**

Create `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/reading_view.rs`:

```rust
//! The `waml-editor` side of the markdown reading view.
//!
//! Mirrors `SourceView`, but installs a `ReadingDocument` on the
//! `MarkdownViewer` instead of an `InstalledPresentation` on the editor. The
//! two surfaces share the parse, the compile and the styles; they share no
//! layout engine.

use std::sync::Arc;

use makepad_widgets::*;
use waml_markdown_editor::presentation::{compile_presentation, PresentationStyles};
use waml_markdown_editor::reading::{build_reading_document, ReadingDocument};

use crate::doc_view::BodyWidgets;
use crate::editor_session::EditorSessionSnapshot;
use crate::markdown_hosts::{
    EditorMarkdownAssetHost, MarkdownAssetLease, SharedMarkdownAssetHost, WamlCodeHighlightHost,
};
use crate::source_view::SourceView;

pub struct ReadingView {
    key: String,
    /// `true` once the reader has asked to see the markdown source. The
    /// editor side stays read-only: this toggles RENDERING, not writability.
    showing_source: bool,
    document: Option<Arc<ReadingDocument>>,
    revision: Option<waml_markdown_editor::syntax::DocumentRevision>,
    asset_lease: Option<MarkdownAssetLease>,
}

impl ReadingView {
    pub fn new_with_asset_host(key: String, assets: SharedMarkdownAssetHost) -> ReadingView {
        ReadingView {
            key,
            showing_source: false,
            document: None,
            revision: None,
            asset_lease: Some(EditorMarkdownAssetHost::open_lease(&assets)),
        }
    }

    pub fn showing_source(&self) -> bool {
        self.showing_source
    }

    pub fn set_showing_source(&mut self, showing: bool) {
        self.showing_source = showing;
    }

    pub fn install_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        let Some((_document, syntax)) = SourceView::resolve_document(snapshot, &self.key) else {
            return;
        };
        if self.revision == Some(syntax.revision()) {
            return;
        }
        let styles = Arc::new(PresentationStyles::balanced());
        let highlighters =
            WamlCodeHighlightHost::registry(Arc::new(snapshot.workspace_for_highlighting()));
        let Ok(plan) = compile_presentation(&syntax, &styles, &highlighters) else {
            return;
        };
        let Ok(document) = build_reading_document(&plan) else {
            return;
        };
        let source: Arc<str> = Arc::from(syntax.text().as_str());
        self.revision = Some(syntax.revision());
        self.document = Some(Arc::new(document));
        body.markdown_viewer().install_document(
            cx,
            self.document.clone().expect("just installed"),
            source,
        );
    }
}
```

**Adaptation note:** `WamlCodeHighlightHost::registry` currently takes `Arc::new(workspace.clone())` inside `SourceView::install_snapshot`. Read `crates/waml-editor/src/source_view.rs` around line 354 and mirror **exactly** how it obtains its `workspace` from the snapshot; `snapshot.workspace_for_highlighting()` above is a placeholder name for whatever that expression actually is. Likewise mirror how it gets `syntax.text()` as a `&str`. Do not invent accessors.

- [ ] **Step 6: Declare the module**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/lib.rs`, adding `pub mod reading_view;` in alphabetical position among the other `pub mod` lines.

- [ ] **Step 7: Rewire `GenericOkfView`**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/generic_okf_view.rs`:

Replace the struct and constructor with:

```rust
pub struct GenericOkfView {
    /// The reading surface. A concept opens here.
    reading: crate::reading_view::ReadingView,
    /// The raw-markdown surface, reached by the explicit source toggle. It
    /// stays read-only: this view discards `source_edit`, so a writable
    /// editor would silently drop what the user typed.
    source: SourceView,
}

impl GenericOkfView {
    #[cfg(test)]
    pub fn new(concept_id: String) -> Self {
        Self::new_with_asset_host(
            concept_id,
            crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
        )
    }

    pub fn new_with_asset_host(
        concept_id: String,
        assets: crate::markdown_hosts::SharedMarkdownAssetHost,
    ) -> Self {
        // Opening a concept is a reading action: it renders. Seeing the
        // markdown behind it is a separate, explicit action.
        Self {
            reading: crate::reading_view::ReadingView::new_with_asset_host(
                concept_id.clone(),
                assets.clone(),
            ),
            source: SourceView::new_read_only(concept_id, assets),
        }
    }

    pub(crate) fn showing_source(&self) -> bool {
        self.reading.showing_source()
    }

    pub(crate) fn toggle_source(&mut self) {
        let showing = self.reading.showing_source();
        self.reading.set_showing_source(!showing);
    }
}
```

Replace `sync` with:

```rust
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        if self.reading.showing_source() {
            body.show_markdown_editor(cx);
            body.markdown_editor().set_read_only(cx, true);
        } else {
            body.show_markdown_viewer(cx);
        }
    }
```

Replace `sync_from_session` with:

```rust
    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.sync(cx, body, snapshot.borrowed().into());
        self.reading.install_snapshot(cx, body, snapshot);
        self.source
            .install_snapshot(cx, body, snapshot, HostSnapshotCause::InitialLoad);
    }
```

Replace `after_session_snapshot`'s body so it feeds both surfaces:

```rust
        let cause = if change.source_changed {
            HostSnapshotCause::ApplicationHistory
        } else {
            HostSnapshotCause::AcknowledgedLocalEdit
        };
        self.reading.install_snapshot(cx, body, snapshot);
        self.source.install_snapshot(cx, body, snapshot, cause);
```

Extend `handle` so the toggle button acts:

```rust
    fn handle(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome {
        if body
            .ui()
            .button(cx, ids!(markdown_viewer_surface.source_toggle))
            .clicked(actions)
        {
            self.toggle_source();
            self.sync(cx, body, data.reborrow());
        }
        let mut outcome = self.source.handle(cx, body, actions, data);
        outcome.source_edit = None;
        outcome
    }
```

**Adaptation note:** `body.ui()`, `.button(cx, ids!(...))`, `.clicked(actions)`, and `ViewData::reborrow` are the shapes used elsewhere in this crate. Before writing this, read one existing `DocView::handle` that reacts to a button (grep `clicked(actions)` under `crates/waml-editor/src/`) and mirror it exactly. If `ViewData` is not reborrowable, restructure to compute the toggle first and call `sync` with the same `data` afterwards.

- [ ] **Step 8: Add the source-toggle button to the DSL**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/app.rs`, inside the `markdown_viewer_surface := View{...}` block added in Task 2. Change its `flow: Down` to `flow: Overlay` and add, after `viewer := MarkdownViewer{...}`:

```rust
                                    // Explicit "show me the markdown" action.
                                    // Reading and editing are separate surfaces;
                                    // this is the only door between them.
                                    source_toggle_wrap := View{
                                        width: Fill
                                        height: Fit
                                        align: { x: 1.0, y: 0.0 }
                                        padding: { top: 8.0, right: 8.0 }
                                        source_toggle := IconButton{
                                            icon: (ICON_FILE_CODE)
                                            tooltip: "Show markdown source"
                                        }
                                    }
```

**Adaptation note:** read `crates/waml-editor/src/icon_button.rs` and one existing `IconButton{...}` call site in `app.rs` and copy the property names verbatim; `icon:` / `tooltip:` above are placeholders for whatever that widget actually exposes. `file-code` already exists at `resources/icons/file-code.svg`; if it is not yet registered in `crates/waml-editor/src/icons.rs`, register it there in the same commit, respecting that file's `enum == field == DSL == get == ALL == label` ordering and its count assertions.

- [ ] **Step 9: Run the view tests**

Run: `cargo test -p waml-editor generic_okf_view`
Expected: PASS.

- [ ] **Step 10: Full gate**

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd editors/vscode; pnpm build; pnpm lint; pnpm test
```

- [ ] **Step 11: Visual check — the concept actually renders as prose**

This is the check that was OWED and never done: the editor has never been run against a rendered concept.

Run in ONE PowerShell tool call:

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo build -p waml-editor --bin waml-editor
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$p = Start-Process -PassThru -FilePath ".\target\debug\waml-editor.exe" -ArgumentList "crates/waml-editor/tests/fixtures/okf-only"
Start-Sleep -Seconds 20
pwsh -File scripts/capture-window.ps1 -Out "$env:TEMP\viewer-task4.png" -ProcessId $p.Id
Stop-Process -Id $p.Id -Force
```

Then `Read` `%TEMP%\viewer-task4.png` and check **every** one of these against the `reading.md` fixture:

1. The `---` frontmatter block is absent — no `title: Reading View` line.
2. `# Reading View` renders as a large bold heading with **no** leading `#`.
3. `*rendered*` is italic with no asterisks; `` `inline code` `` is monospace with no backticks.
4. Nested bullets show three distinct marker shapes (disc, ring, square) at three indents — **not** three identical squares, and no marker sits above its own item's text line.
5. `1.` and `2.` are visible as text on the ordered items.
6. The quote is inset with a rule and no `>` characters.
7. The fenced block has a code surface, monospace content, and no ``` fences.
8. The table has a grid and no `|` or `---` delimiter row.
9. The source-toggle button is visible in the top-right of the surface.

If any check fails, fix it and re-run this step before proceeding. Do not commit on a failed visual check.

- [ ] **Step 12: Delete the npm lockfile and commit**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
rm -f editors/vscode/package-lock.json
git add -A
git status --short
git commit -m "feat(editor): open OKF concepts in the markdown reading view

A concept is prose to read, so it opens in the viewer: blocks, rendered
punctuation, bullets as decorations. Seeing the markdown behind it is a
separate, explicit action on its own surface.

The source side stays read-only. This view discards source_edit, so a writable
editor would silently drop what the user typed; making concepts writable is a
separate change."
```

---

### Task 5: Remove the interim `hide_syntax` path

`hide_syntax` was the interim answer to "how does a concept render". The viewer is now the answer, and the project must not keep two. This task deletes the flag and everything that existed only to thread it.

**KEEP, do not touch:**
- The `FakeShaper` fix in `crates/waml-markdown-editor/tests/layout_geometry.rs` — it mirrors the real shaper (honours `ShapeSpan.hidden`, derives row height from glyphs rather than from `font_size`). It is a real fix to a test double that was testing its own arithmetic.
- The ascender/descender `font_size * 0.8` floor in `crates/waml-markdown-editor/src/layout/engine.rs::push_paragraph`.
- `ShapeSpan.hidden` / `ShapedCluster.hidden` and `extract_clusters`' zeroing in `crates/waml-markdown-editor/src/layout/makepad.rs` — these stay, because `PresentationItem::TextRun::hidden` stays. What goes is only the *flag that sets it for whole roles*.
- `TextRole::is_syntax_marker()` — the reading model calls it.
- `BlockDecorationKind::ListBullet` and its `draw.rs` arm — the plan emits the decoration unconditionally (Task 1 Step 8) and the viewer reads it.

**Files:**
- Modify: `crates/waml-markdown-editor/src/presentation/style.rs`
- Modify: `crates/waml-markdown-editor/src/presentation/compile.rs`
- Modify: `crates/waml-editor/src/source_view.rs`
- Modify: `crates/waml-editor/src/generic_okf_view.rs`
- Delete: `crates/waml-markdown-editor/tests/presentation_hidden_syntax.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces: `PresentationStyles` becomes a fieldless unit-like config again — `PresentationStyles::balanced()` is the only constructor, `hiding_syntax()` is gone, and `PresentationStyles::hide_syntax` no longer exists. Any code still naming them will not compile, which is the point.

- [ ] **Step 1: Verify the worktree and inventory the call sites**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
git rev-parse --show-toplevel
grep -rn 'hide_syntax\|hiding_syntax\|hides_syntax\|push_text_hidden\|is_unordered_marker\|list_nesting_level' crates/ --include=*.rs
```

Expected: hits in exactly `presentation/style.rs`, `presentation/compile.rs`, `source_view.rs`, `generic_okf_view.rs` (a doc comment only, after Task 4), and `tests/presentation_hidden_syntax.rs`. Anything else is a call site this plan missed — handle it the same way and note it in the commit body.

- [ ] **Step 2: Delete the dead test file**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
git rm crates/waml-markdown-editor/tests/presentation_hidden_syntax.rs
```

All 8 of its tests assert `hide_syntax` behaviour. Their intent — "the reading view suppresses punctuation without dropping source" — is now covered by `crates/waml-markdown-editor/tests/reading_model.rs`, which tests the same property at the seam that actually renders.

- [ ] **Step 3: Strip the flag from `PresentationStyles`**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/presentation/style.rs`. Replace

```rust
pub struct PresentationStyles {
    /// Hides markdown punctuation, for views that are read rather than edited.
    /// Purely presentational: the source and its run coverage are unchanged.
    pub hide_syntax: bool,
}
```

with

```rust
pub struct PresentationStyles;
```

and replace

```rust
    /// The balanced document style. Metrics are fixed logical values and do not
    /// depend on device pixel ratio.
    pub fn balanced() -> Self {
        Self::default()
    }

    /// The balanced style with markdown punctuation hidden.
    pub fn hiding_syntax() -> Self {
        Self { hide_syntax: true }
    }
```

with

```rust
    /// The balanced document style. Metrics are fixed logical values and do not
    /// depend on device pixel ratio.
    ///
    /// There is exactly one style table. Whether markdown punctuation is drawn
    /// is a property of the SURFACE, not of the plan: the editor styles raw
    /// markdown, the reading view renders it. A `hide_syntax` flag here would
    /// be a second answer to that question.
    pub fn balanced() -> Self {
        Self
    }
```

If `PresentationStyles` derives `Default`, keep the derive — a unit struct derives it fine.

- [ ] **Step 4: Strip the branches from `compile.rs`**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-markdown-editor/src/presentation/compile.rs`.

Replace the `ListMarker` arm (around line 57, as amended by Task 1 Step 8) with:

```rust
        } else if role == TextRole::ListMarker {
            // An ordered number is content a reader needs; a bullet character
            // is punctuation. The decoration records which it is; whether
            // either is DRAWN is the surface's decision, not the plan's.
            builder.push_text(span.range, role, span.owner);
            if is_unordered_marker(text, span.range) {
                let level = list_nesting_level(text, span.range);
                builder.push_block(
                    span.owner,
                    span.range,
                    BlockDecorationKind::ListBullet { level },
                );
            }
```

Replace `push_text` (around line 527) with:

```rust
    fn push_text(&mut self, range: TextRange, role: TextRole, owner: SyntaxIdentity) {
        let presentation_role = PresentationRole::Text(role);
        let fragment_ordinal = self.next_ordinal(owner, presentation_role);
        self.items.push(PresentationItem::TextRun {
            id: PresentationItemId {
                owner,
                role: presentation_role,
                fragment_ordinal,
            },
            range,
            role,
            style: self.styles.text_style(role),
            hidden: false,
        });
    }
```

and **delete** `push_text_hidden` entirely.

`is_unordered_marker` and `list_nesting_level` **stay** — they are now unconditional plan metadata.

- [ ] **Step 5: Strip `hide_syntax` from `SourceView`**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/source_view.rs`:

1. Delete the `hide_syntax: bool,` field and its doc comment (around line 110-112).
2. Delete `hide_syntax: false,` from the `new_with_asset_host` initialiser.
3. Delete `set_hide_syntax` and `hides_syntax` entirely.
4. Delete the `hide_syntax: bool,` parameter from `fn compile` and replace its body's opening

   ```rust
       let styles = Arc::new(if hide_syntax {
           PresentationStyles::hiding_syntax()
       } else {
           PresentationStyles::balanced()
       });
   ```

   with

   ```rust
       let styles = Arc::new(PresentationStyles::balanced());
   ```

5. Delete the `self.hide_syntax,` argument at the `Self::compile(...)` call site (around line 367).
6. Delete the `hide_syntax_reaches_the_compiled_presentation_without_touching_the_source` test (around line 1502) and any other test in that file that names `hide_syntax`.

- [ ] **Step 6: Clean the `GenericOkfView` comment**

Edit `C:/dev/waml/.worktrees/markdown-hide-syntax/crates/waml-editor/src/generic_okf_view.rs` and remove any remaining prose that describes hiding syntax in the editor, if Task 4 left any. The surviving comment should say the concept opens in the reading view.

- [ ] **Step 7: Compile and let the compiler find the stragglers**

Run: `cargo check --workspace --all-targets`
Expected: exit 0. Every remaining error is a call site the grep in Step 1 missed — fix each by removing the flag, never by reintroducing it.

- [ ] **Step 8: Full gate**

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd editors/vscode; pnpm build; pnpm lint; pnpm test
```

- [ ] **Step 9: Confirm the kept fixes are still in place**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
git diff HEAD --stat -- crates/waml-markdown-editor/tests/layout_geometry.rs crates/waml-markdown-editor/src/layout/engine.rs crates/waml-markdown-editor/src/layout/makepad.rs
```

Expected: **empty output**. If any of those three files appears, the deletion overreached — the `FakeShaper` fix, the `font_size * 0.8` floor, and `ShapedCluster.hidden` all stay. Revert those files and re-run the gate.

- [ ] **Step 10: Visual regression check**

The editor surface (not the viewer) must still render raw markdown exactly as before. Run in ONE PowerShell tool call:

```powershell
cd C:/dev/waml/.worktrees/markdown-hide-syntax
cargo build -p waml-editor --bin waml-editor
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$p = Start-Process -PassThru -FilePath ".\target\debug\waml-editor.exe" -ArgumentList "crates/waml-editor/tests/fixtures/okf-only"
Start-Sleep -Seconds 20
pwsh -File scripts/capture-window.ps1 -Out "$env:TEMP\viewer-task5.png" -ProcessId $p.Id
Stop-Process -Id $p.Id -Force
```

Then `Read` `%TEMP%\viewer-task5.png`. Expected: `reading.md` still renders as prose exactly as it did in Task 4 Step 11 — removing the interim path must change nothing visible.

- [ ] **Step 11: Delete the npm lockfile and commit**

```bash
cd C:/dev/waml/.worktrees/markdown-hide-syntax
rm -f editors/vscode/package-lock.json
git add -A
git status --short
git commit -m "refactor(markdown): remove the interim hide-syntax path

hide_syntax was a stopgap answer to how a concept renders, and the reading
view is now the real one. Two answers to that question is one too many, so the
flag, its style constructor, and the compile branches that existed only to
thread it are gone.

Kept: the FakeShaper fix, which stopped the layout tests measuring their own
arithmetic, and the ascender/descender floor in push_paragraph. Both are real
fixes to shared code and have nothing to do with hiding. ShapedCluster.hidden
stays too; only the flag that set it for whole roles is removed."
```

---

## Self-Review

**1. Spec coverage.** Every requirement from the brief maps to a task:

| Requirement | Task |
| --- | --- |
| Separate viewer widget, distinct from the editor | 2 |
| Split seam is `PresentationPlan`, not the parser | 1 (model derives from the plan; `compile_presentation` untouched apart from making a decoration unconditional) |
| Shared: `waml-syntax`, `compile_presentation`, `PresentationStyles`, decorations, highlighters, assets | 1, 4 |
| Not shared: `LayoutEngine`, motion, selection, input, IME | 2 (the viewer references none of them) |
| `PresentationPlan` -> `TextFlow` driver, hundreds of lines, not a layout engine | 2 |
| Makepad `Markdown` widget rejected | Architecture section + `widget.rs` module doc |
| Substitute-text runs rejected; bullets as decorations | 2 (fork `begin_list_item_gutter`, `bullet.rs`) |
| `validate_source_partition` invariant preserved | 1 (`ReadingDocument::validate_source_partition`, tested) |
| `line_spacing` is a multiplier | Global Constraints; the viewer never uses it as a height |
| Row-height floor / `ShapedCluster.hidden` | Task 5 KEEP list + Step 9 guard |
| `FakeShaper` stays mirroring the real shaper | Task 5 KEEP list + Step 9 guard |
| Points-vs-pixels font sizing | Global Constraints |
| CRLF files need the Edit tool | Global Constraints |
| Visual verification is owed | Tasks 2, 4, 5 — with the exact launch/capture/kill-by-pid command |
| Bullet shape decided | Design Decision 6: disc / ring / square by `level % 3` |
| Selection / copy / `point_to_index` back to source | 3 |
| `GenericOkfView` wired to the viewer + explicit edit action | 4 |
| `hide_syntax` removed, sequenced last | 5 |
| Full gate, no lockfile, no proptest-regressions, no co-author trailer | Global Constraints, repeated in every task's commit step |
| Fork change = fork commit + SHA rev bump | Global Constraints + Task 2 Steps 2-5 |

**2. Placeholder scan.** Three steps carry explicit **Adaptation notes** rather than final code (Task 2 Step 9's widget boilerplate, Task 4 Step 5's highlighter/workspace accessor, Task 4 Step 8's `IconButton` properties). Each names the exact file to read first, states that the repo's pattern wins over the plan's sketch, and gives the complete logic the boilerplate must carry. That is a deliberate, bounded hand-off, not a "TBD" — makepad widget boilerplate cannot be written blind against an unbuilt tree, and inventing accessor names would be worse than pointing at the real ones. Everything else is literal code.

**3. Type consistency.** `ReadingDocument` / `ReadingBlock` / `ReadingBlockKind` / `ReadingPiece` / `ReadingError` are spelled identically in Tasks 1-4. `build_reading_document` takes `&PresentationPlan` and returns `Result<ReadingDocument, ReadingError>` everywhere. `SourceMap::{clear, push, source_offset, source_span, is_empty}` match between Task 2's definition and Task 3's tests. `MarkdownViewerRef::install_document(cx, Arc<ReadingDocument>, Arc<str>)` matches its call in `ReadingView::install_snapshot`. `caret_for_span(Option<TextRange>) -> TextSize` matches its test. `show_markdown_viewer` / `markdown_viewer` match between Task 2's `doc_view.rs` edit and Task 4's use.

**Two corrections made after reading the code, worth flagging:**
- Task 1 Step 8 has to make the `ListBullet` decoration **unconditional** in `compile.rs`, because today it is emitted only under `hide_syntax` — and the viewer compiles with `balanced()`. Without that, the model cannot tell a bullet from an ordered number.
- Task 3 Step 6 anticipates a `dead_code` clippy failure on `MarkdownViewerAction` and `source_offset_at`, and says to move them into Task 4 rather than to `#[allow]` them. The gate's `-D warnings` makes unread items a hard error, so a task that produces a type only its successor consumes must either test it or defer it.
