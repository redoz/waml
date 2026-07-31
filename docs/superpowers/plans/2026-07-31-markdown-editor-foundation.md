# Markdown Editor Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a standalone, WAML-owned Markdown editing crate with immutable revisioned document snapshots, exact edit transactions, Unicode-safe selections and IME, variable-metric layout, viewport virtualization, and a Makepad widget that does not depend on Makepad's `CodeEditor` or `Markdown` widgets.

**Architecture:** `waml-markdown-editor` separates a framework-independent document session from layout geometry and the Makepad widget adapter. The session consumes the immutable Markdown syntax snapshots from spec 1, and every accepted edit advances one local revision and carries the exact `TextChange` list plus the already-computed `MarkdownSyntaxUpdate`. Spec 3 builds the foundation-owned low-level `LayoutDocument`, the foundation produces immutable `LayoutSnapshot` geometry, and presentation motion interpolates that same snapshot for every draw layer.

**Tech Stack:** Rust 2021 (MSRV 1.80), `waml-syntax`, Makepad low-level widget/drawing/font/input primitives, `unicode-segmentation` 1.12, Cargo unit and integration tests.

## Global Constraints

- Raw Markdown source is the only editable, clipboard, and history representation.
- Parser and document positions use `waml_syntax::TextSize` and `waml_syntax::TextRange` UTF-8 byte offsets; invalid source boundaries return typed errors and are never rounded.
- `MarkdownDocumentSession` is the widget-local editing authority; it must not be confused with application-level `waml_editor::EditorSession`.
- Published document, syntax, and layout snapshots are immutable and revision-tagged.
- Every edit uses `MarkdownEdit { base_revision, changes, selection_after, history_group }`; stale base revisions are rejected with the current revision.
- The widget promotes the exact `MarkdownSyntaxUpdate` computed for a local edit; no UI path reparses the same revision independently.
- Syntax diagnostics do not disable editing.
- Layout and hit-testing support mixed font metrics and proportional widths; no fixed-cell assumption can enter wrapping, cursor movement, selection, or scrolling.
- Glyph and embedded-widget work is limited to the visible block window plus overscan; off-screen blocks retain measured summaries.
- Each adapted Makepad module records `https://github.com/redoz/makepad.git`, commit `c38f529984eda61e258ca69fb50c6712d85c74c1`, the MIT license, and its material changes.
- `waml-markdown-editor` may use Makepad low-level widget, drawing, font, input, and platform primitives, but Cargo and source imports must not reference Makepad's upstream `CodeEditor` or `Markdown` widgets.
- The new crate inherits workspace edition `2021`, Rust version `1.80`, and license `MPL-2.0`.

## File Structure

- Modify `Cargo.toml` to register the crate and the shared Unicode dependency.
- Create `crates/waml-markdown-editor/Cargo.toml` for the standalone foundation crate.
- Create `crates/waml-markdown-editor/PROVENANCE.md` as the auditable fork/adaptation ledger.
- Create `crates/waml-markdown-editor/src/lib.rs` for public exports and Makepad widget registration.
- Create `crates/waml-markdown-editor/src/document.rs` for immutable document snapshots and revision/boundary validation.
- Create `crates/waml-markdown-editor/src/selection.rs` for affinity-aware, revision-bound selections and normalization.
- Create `crates/waml-markdown-editor/src/edit.rs` for the one transaction shape, edit validation, rebasing, and command lowering.
- Create `crates/waml-markdown-editor/src/history.rs` for grouped inverse transactions and selection restoration.
- Create `crates/waml-markdown-editor/src/unicode.rs` for grapheme, word, source-line, UTF-16, and line/column adapters.
- Create `crates/waml-markdown-editor/src/ime.rs` for uncommitted composition state, commit, and cancellation.
- Create `crates/waml-markdown-editor/src/session.rs` for the editing authority and proposed-edit output.
- Create `crates/waml-markdown-editor/src/layout/mod.rs` for layout input types and public exports.
- Create `crates/waml-markdown-editor/src/layout/geometry.rs` for immutable visual-line, block, cluster, caret, and selection geometry.
- Create `crates/waml-markdown-editor/src/layout/engine.rs` for variable-metric wrapping, incremental relayout, fallback blocks, and viewport virtualization.
- Create `crates/waml-markdown-editor/src/layout/makepad.rs` for Makepad `DrawText::layout` shaping and font fallback.
- Create `crates/waml-markdown-editor/src/input.rs` for platform-neutral keyboard, pointer, clipboard, and scroll commands.
- Create `crates/waml-markdown-editor/src/widget.rs` for Makepad event adaptation, scrolling, IME placement, and ordered drawing.
- Create `crates/waml-markdown-editor/tests/document_ops.rs` for transaction, selection, history, and edit-command behavior.
- Create `crates/waml-markdown-editor/tests/unicode_ime.rs` for CRLF, Unicode, UTF-16, grapheme, and IME behavior.
- Create `crates/waml-markdown-editor/tests/layout_geometry.rs` for variable-metric geometry, round trips, virtualization, and fallback.
- Create `crates/waml-markdown-editor/tests/widget_parity.rs` for retained Makepad input behavior and deliberate divergences.
- Create `crates/waml-markdown-editor/tests/provenance.rs` for dependency and provenance enforcement.

---

### Task 1: Scaffold the standalone crate and lock provenance

**Files:**
- Modify: `Cargo.toml:1-24`
- Create: `crates/waml-markdown-editor/Cargo.toml`
- Create: `crates/waml-markdown-editor/PROVENANCE.md`
- Create: `crates/waml-markdown-editor/src/lib.rs`
- Create: `crates/waml-markdown-editor/tests/provenance.rs`

**Interfaces:**
- Consumes: spec 1 exports from `waml-syntax`; the existing workspace Makepad pin `c38f529984eda61e258ca69fb50c6712d85c74c1`.
- Produces: crate `waml_markdown_editor`; `pub fn live_design(cx: &mut makepad_widgets::Cx)`; public modules named in the file map.

- [ ] **Step 1: Write the dependency and provenance tests**

Create `tests/provenance.rs`:

```rust
use std::{fs, path::PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn provenance_names_every_adapted_module_and_exact_upstream_revision() {
    let text = fs::read_to_string(crate_root().join("PROVENANCE.md")).unwrap();
    assert!(text.contains("https://github.com/redoz/makepad.git"));
    assert!(text.contains("c38f529984eda61e258ca69fb50c6712d85c74c1"));
    assert!(text.contains("MIT License"));
    for module in ["selection.rs", "history.rs", "input.rs", "widget.rs"] {
        assert!(text.contains(module), "missing provenance for {module}");
    }
}

#[test]
fn crate_does_not_depend_on_upstream_editor_or_markdown_widgets() {
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml")).unwrap();
    let source = [
        "src/lib.rs",
        "src/document.rs",
        "src/selection.rs",
        "src/edit.rs",
        "src/history.rs",
        "src/unicode.rs",
        "src/ime.rs",
        "src/session.rs",
        "src/input.rs",
        "src/widget.rs",
        "src/layout/mod.rs",
        "src/layout/geometry.rs",
        "src/layout/engine.rs",
        "src/layout/makepad.rs",
    ]
    .into_iter()
    .map(|path| fs::read_to_string(crate_root().join(path)).unwrap())
    .collect::<String>();
    assert!(!manifest.contains("makepad-code-editor"));
    assert!(!source.contains("makepad_code_editor"));
    assert!(!source.contains("MarkdownAction"));
    assert!(!source.contains("as_markdown()"));
}
```

- [ ] **Step 2: Run the test and verify the crate is absent**

Run: `rtk cargo test -p waml-markdown-editor --test provenance`

Expected: FAIL with `package ID specification 'waml-markdown-editor' did not match any packages`.

- [ ] **Step 3: Add the workspace member and exact dependencies**

In the root `Cargo.toml`, add `"crates/waml-markdown-editor"` to `workspace.members` and add:

```toml
unicode-segmentation = "1.12"
```

Create `crates/waml-markdown-editor/Cargo.toml`:

```toml
[package]
name = "waml-markdown-editor"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "WAML-owned variable-metric Markdown editing foundation."

[dependencies]
waml-syntax = { path = "../waml-syntax" }
makepad-widgets = { git = "https://github.com/redoz/makepad.git", rev = "c38f529984eda61e258ca69fb50c6712d85c74c1" }
unicode-segmentation = { workspace = true }
```

Create `src/lib.rs` with all module declarations and an initially empty `live_design` entry point:

```rust
pub mod document;
pub mod edit;
pub mod history;
pub mod ime;
pub mod input;
pub mod layout;
pub mod selection;
pub mod session;
pub mod unicode;
pub mod widget;

pub fn live_design(_cx: &mut makepad_widgets::Cx) {}
```

Create every declared module as an empty file so the crate compiles; use `src/layout/mod.rs` for the `layout` module and do not create `src/layout.rs`. Write `PROVENANCE.md` with one table row per adapted module:

```markdown
# Makepad CodeEditor adaptation provenance

The source behavior characterized here comes from
https://github.com/redoz/makepad.git at commit
`c38f529984eda61e258ca69fb50c6712d85c74c1`, package
`makepad-code-editor` 2.0.0, licensed `MIT OR Apache-2.0`. WAML uses the MIT
license grant; retain `Copyright (c) 2023 Makepad B.V.` with this record.

| WAML module | Upstream source | Retained behavior | Material changes |
|---|---|---|---|
| `selection.rs` | `code_editor/src/selection.rs` | ordered multi-selection, overlap normalization, affinity | UTF-8 `TextSize` positions, revision binding, Unicode grapheme boundaries, geometry-driven vertical movement |
| `history.rs` | `code_editor/src/history.rs` | grouped undo/redo and selection restoration | exact `MarkdownEdit` transactions and immutable snapshots replace mutable `Text` edits |
| `input.rs` | `code_editor/src/session.rs`, `code_editor/src/code_editor.rs` | insert/delete/paste/cut/indent/navigation/click-count behavior | commands lower before mutation; read-only and IME are explicit typed state |
| `widget.rs` | `code_editor/src/code_editor.rs`, `code_editor/src/code_view.rs`, `code_editor/src/draw_selection.rs` | focus, clipboard, pointer selection, caret visibility, scrolling, drawing | variable-metric `LayoutSnapshot`, visible-block virtualization, explicit draw order, no upstream `CodeEditor` or `Markdown` dependency |

The new layout implementation does not copy `code_editor/src/layout.rs` or
`wrap.rs`; those modules assume a fixed cell grid. Every deliberate behavioral
divergence has a named test in `tests/widget_parity.rs`.
```

- [ ] **Step 4: Run the provenance test**

Run: `rtk cargo test -p waml-markdown-editor --test provenance`

Expected: PASS, 2 tests.

- [ ] **Step 5: Commit the isolated scaffold**

```bash
rtk git add Cargo.toml Cargo.lock crates/waml-markdown-editor
rtk git commit -m "chore: scaffold markdown editor foundation"
```

---

### Task 2: Add immutable document snapshots and revision-bound selections

**Files:**
- Create: `crates/waml-markdown-editor/src/document.rs`
- Create: `crates/waml-markdown-editor/src/selection.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`
- Create: `crates/waml-markdown-editor/tests/document_ops.rs`

**Interfaces:**
- Consumes: `waml_syntax::{DocumentRevision, LineIndex, MarkdownSyntaxSnapshot, SourceText, TextError, TextRange, TextSize}`; `DocumentRevision::{INITIAL, checked_next, get}` from spec 1.
- Produces: `MarkdownDocumentSnapshot`; `Affinity`; `TextPosition`; `Selection`; `SelectionSet`; `SelectionError`.

- [ ] **Step 1: Write failing snapshot and selection tests**

Create the first tests in `tests/document_ops.rs`:

```rust
use std::sync::Arc;
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    selection::{Affinity, Selection, SelectionError, SelectionSet, TextPosition},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextSize,
};

fn snapshot(text: &str, revision: u64) -> Arc<MarkdownDocumentSnapshot> {
    let text = SourceText::from_shared(Arc::new(text.to_owned())).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::new(revision),
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    Arc::new(MarkdownDocumentSnapshot::new(syntax))
}

#[test]
fn document_snapshot_shares_the_syntax_text_and_builds_one_line_index() {
    let snapshot = snapshot("a\r\nβ\n", 7);
    assert_eq!(snapshot.revision().get(), 7);
    assert_eq!(snapshot.text().shared().as_str(), "a\r\nβ\n");
    assert_eq!(
        snapshot
            .line_index()
            .line_col(snapshot.text(), TextSize::try_from_usize(5).unwrap())
            .unwrap()
            .line,
        1
    );
}

#[test]
fn selection_set_rejects_wrong_revision_and_non_boundaries() {
    let snapshot = snapshot("a😀b", 3);
    let inside_emoji = TextSize::try_from_usize(2).unwrap();
    assert!(matches!(
        SelectionSet::single(
            &snapshot,
            Selection::caret(TextPosition::new(inside_emoji, Affinity::Before))
        ),
        Err(SelectionError::InvalidBoundary { offset }) if offset == inside_emoji
    ));
    let set = SelectionSet::caret(&snapshot, TextSize::try_from_usize(1).unwrap()).unwrap();
    assert_eq!(set.revision(), snapshot.revision());
    assert_eq!(set.primary_index(), 0);
}

#[test]
fn overlapping_selections_are_sorted_and_normalized() {
    let snapshot = snapshot("abcdef", 4);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let set = SelectionSet::from_selections(
        &snapshot,
        vec![
            Selection::new(p(4), p(1)),
            Selection::new(p(3), p(5)),
            Selection::caret(p(0)),
        ],
        1,
    )
    .unwrap();
    assert_eq!(set.as_slice().len(), 2);
    assert_eq!(set.as_slice()[1].range().start().to_usize(), 1);
    assert_eq!(set.as_slice()[1].range().end().to_usize(), 5);
    assert_eq!(set.primary_index(), 1);
}
```

- [ ] **Step 2: Run the focused tests and verify missing types**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: FAIL with unresolved imports for `MarkdownDocumentSnapshot`, `SelectionSet`, and related types.

- [ ] **Step 3: Implement the immutable snapshot**

In `document.rs`, implement:

```rust
use std::sync::Arc;
use waml_syntax::{
    DocumentRevision, LineIndex, MarkdownSyntaxSnapshot, SourceText,
};

#[derive(Clone)]
pub struct MarkdownDocumentSnapshot {
    syntax: Arc<MarkdownSyntaxSnapshot>,
    line_index: Arc<LineIndex>,
}

impl MarkdownDocumentSnapshot {
    pub fn new(syntax: Arc<MarkdownSyntaxSnapshot>) -> Self {
        let line_index = Arc::new(LineIndex::new(syntax.text()));
        Self { syntax, line_index }
    }

    pub fn revision(&self) -> DocumentRevision {
        self.syntax.revision()
    }

    pub fn text(&self) -> &SourceText {
        self.syntax.text()
    }

    pub fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    pub fn syntax(&self) -> &Arc<MarkdownSyntaxSnapshot> {
        &self.syntax
    }
}
```

Add a manual `Debug` implementation that prints only `revision` and source byte length. It must not require `MarkdownSyntaxSnapshot: Debug`; this keeps `ProposedMarkdownEdit` and widget actions debuggable without exposing syntax internals.

- [ ] **Step 4: Implement selection validation and normalization**

In `selection.rs`, define these exact public shapes:

```rust
use crate::document::MarkdownDocumentSnapshot;
use waml_syntax::{DocumentRevision, TextError, TextRange, TextSize};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Affinity {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextPosition {
    pub offset: TextSize,
    pub affinity: Affinity,
}

impl TextPosition {
    pub fn new(offset: TextSize, affinity: Affinity) -> Self {
        Self { offset, affinity }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Selection {
    pub anchor: TextPosition,
    pub cursor: TextPosition,
}

impl Selection {
    pub fn new(anchor: TextPosition, cursor: TextPosition) -> Self {
        Self { anchor, cursor }
    }

    pub fn caret(position: TextPosition) -> Self {
        Self::new(position, position)
    }

    pub fn is_empty(self) -> bool {
        self.anchor.offset == self.cursor.offset
    }

    pub fn range(self) -> TextRange {
        TextRange::new(
            self.anchor.offset.min(self.cursor.offset),
            self.anchor.offset.max(self.cursor.offset),
        )
        .expect("ordered selection endpoints")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionSet {
    revision: DocumentRevision,
    selections: Vec<Selection>,
    primary: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionError {
    EmptySet,
    PrimaryOutOfBounds { primary: usize, len: usize },
    InvalidBoundary { offset: TextSize },
    Text(TextError),
}
```

Implement `caret`, `single`, `from_selections`, `caret_in_text(revision, text, offset)`, `from_source(revision, text, selections, primary)`, `revision`, `as_slice`, `primary`, and `primary_index`. The snapshot-taking constructors use `snapshot.revision()` and delegate to the source-taking constructors. Validate every endpoint with a zero-width `SourceText::slice`; sort by range start; merge touching carets and overlapping non-empty selections; and remap the requested primary index to the merged selection that contains its cursor. Do not silently clamp or round offsets.

- [ ] **Step 5: Run the document tests**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: PASS, 3 tests.

- [ ] **Step 6: Commit snapshot and selection authority**

```bash
rtk git add crates/waml-markdown-editor/src/document.rs crates/waml-markdown-editor/src/selection.rs crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/tests/document_ops.rs
rtk git commit -m "feat: add markdown snapshots and selections"
```

---

### Task 3: Apply exact edits and promote the incremental syntax update

**Files:**
- Create: `crates/waml-markdown-editor/src/edit.rs`
- Create: `crates/waml-markdown-editor/src/session.rs`
- Modify: `crates/waml-markdown-editor/src/selection.rs`
- Modify: `crates/waml-markdown-editor/tests/document_ops.rs`

**Interfaces:**
- Consumes: `waml_syntax::reparse_markdown(previous, revision, new_text, changes)`; `TextChange { old_range: TextRange, replacement: Arc<str> }`; `ChangeMap::checked`; `DocumentRevision::checked_next`.
- Produces: `HistoryGroup`; `MarkdownEdit`; `ProposedMarkdownEdit`; `MarkdownEditError`; `MarkdownDocumentSession::{new, snapshot, selections, local_revision, apply_edit}`.

- [ ] **Step 1: Add failing exact-edit, stale-edit, and boundary tests**

Append to `tests/document_ops.rs`:

```rust
use waml_markdown_editor::{
    edit::{HistoryGroup, MarkdownEdit, MarkdownEditError},
    session::MarkdownDocumentSession,
};
use waml_syntax::{TextChange, TextRange};

fn replace(start: usize, end: usize, replacement: &str) -> TextChange {
    TextChange {
        old_range: TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap(),
        replacement: Arc::from(replacement),
    }
}

#[test]
fn exact_edit_advances_once_and_reuses_the_returned_syntax_update() {
    let before = snapshot("# A\n", 10);
    let expected_text = SourceText::from_shared(Arc::new("# Bee\n".to_owned())).unwrap();
    let after_selection = SelectionSet::caret_in_text(
        DocumentRevision::new(11),
        &expected_text,
        TextSize::try_from_usize(5).unwrap(),
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::new(before);
    let proposal = session
        .apply_edit(MarkdownEdit {
            base_revision: DocumentRevision::new(10),
            changes: vec![replace(2, 3, "Bee")],
            selection_after: after_selection,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap();
    assert_eq!(session.local_revision().get(), 11);
    assert_eq!(session.snapshot().text().shared().as_str(), "# Bee\n");
    assert!(Arc::ptr_eq(
        session.snapshot().syntax(),
        &proposal.syntax_update.snapshot
    ));
    assert_eq!(proposal.edit.changes.len(), 1);
}

#[test]
fn stale_edit_reports_current_revision_without_mutation() {
    let before = snapshot("abc", 5);
    let selections = SelectionSet::caret(&before, TextSize::try_from_usize(0).unwrap()).unwrap();
    let mut session = MarkdownDocumentSession::new(before.clone());
    let error = session
        .apply_edit(MarkdownEdit {
            base_revision: DocumentRevision::new(4),
            changes: vec![replace(0, 0, "x")],
            selection_after: selections,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap_err();
    assert!(matches!(
        error,
        MarkdownEditError::StaleRevision {
            base,
            current,
        } if base == DocumentRevision::new(4)
            && current == DocumentRevision::new(5)
    ));
    assert_eq!(session.snapshot().text().shared().as_str(), "abc");
}

#[test]
fn invalid_utf8_change_is_typed_and_does_not_advance() {
    let before = snapshot("a😀b", 8);
    let selections = SelectionSet::caret_in_text(
        DocumentRevision::new(9),
        before.text(),
        TextSize::try_from_usize(0).unwrap(),
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::new(before);
    let error = session
        .apply_edit(MarkdownEdit {
            base_revision: DocumentRevision::new(8),
            changes: vec![replace(2, 2, "x")],
            selection_after: selections,
            history_group: HistoryGroup::isolated(),
        })
        .unwrap_err();
    assert!(matches!(error, MarkdownEditError::InvalidBoundary { .. }));
    assert_eq!(session.local_revision().get(), 8);
}
```

- [ ] **Step 2: Run the tests and verify edit/session imports fail**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: FAIL with unresolved `HistoryGroup`, `MarkdownEdit`, and `MarkdownDocumentSession`.

- [ ] **Step 3: Implement the one transaction and proposal shapes**

In `edit.rs`:

```rust
use crate::{document::MarkdownDocumentSnapshot, selection::SelectionSet};
use std::sync::Arc;
use waml_syntax::{
    DocumentRevision, MarkdownSyntaxUpdate, ParseError, TextChange, TextError, TextSize,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HistoryGroup(u64);

impl HistoryGroup {
    pub const fn isolated() -> Self {
        Self(0)
    }

    pub const fn named(id: u64) -> Self {
        Self(id.saturating_add(1))
    }

    pub const fn can_merge(self, next: Self) -> bool {
        self.0 != 0 && self.0 == next.0
    }
}

#[derive(Clone, Debug)]
pub struct MarkdownEdit {
    pub base_revision: DocumentRevision,
    pub changes: Vec<TextChange>,
    pub selection_after: SelectionSet,
    pub history_group: HistoryGroup,
}

#[derive(Clone, Debug)]
pub struct ProposedMarkdownEdit {
    pub edit: MarkdownEdit,
    pub snapshot: Arc<MarkdownDocumentSnapshot>,
    pub syntax_update: MarkdownSyntaxUpdate,
}

#[derive(Debug)]
pub enum MarkdownEditError {
    StaleRevision {
        base: DocumentRevision,
        current: DocumentRevision,
    },
    SelectionRevision {
        selection: DocumentRevision,
        expected: DocumentRevision,
    },
    InvalidBoundary {
        offset: TextSize,
    },
    Text(TextError),
    Parse(ParseError),
    RevisionOverflow {
        current: DocumentRevision,
    },
}
```

Add `Display`, `Error`, and lossless `From<TextError>`/`From<ParseError>` conversions. Implement `Debug` for `ProposedMarkdownEdit` manually by printing its edit, snapshot revision, and affected ranges; `MarkdownSyntaxUpdate` itself is only required to implement `Clone`.

- [ ] **Step 4: Implement atomic edit application**

In `session.rs`, define:

```rust
pub struct MarkdownDocumentSession {
    snapshot: Arc<MarkdownDocumentSnapshot>,
    selections: SelectionSet,
    read_only: bool,
}
```

`new` initializes selections at offset zero for the supplied revision. `apply_edit` must:

1. Compare `edit.base_revision` with `self.snapshot.revision()` before any mutation.
2. Require `edit.selection_after.revision()` to equal `base_revision.checked_next()`.
3. Validate sorted, non-overlapping `TextChange` ranges with `ChangeMap::checked`.
4. Build the new string in reverse change order, using only validated UTF-8 ranges.
5. Create `SourceText`, validate every `selection_after` endpoint against that new text, call `reparse_markdown` exactly once, and construct `MarkdownDocumentSnapshot` from `syntax_update.snapshot.clone()`.
6. Record history only after every fallible operation succeeds.
7. Install snapshot and selection together, then return `ProposedMarkdownEdit` containing the same `Arc` snapshot and syntax update.

Add `snapshot`, `selections`, `local_revision`, and `is_read_only` accessors. Keep a private `apply_edit_without_history` for undo/redo in Task 4.

- [ ] **Step 5: Run the transaction tests**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: PASS, 6 tests.

- [ ] **Step 6: Commit exact revisioned edits**

```bash
rtk git add crates/waml-markdown-editor/src/edit.rs crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/src/selection.rs crates/waml-markdown-editor/tests/document_ops.rs
rtk git commit -m "feat: apply exact markdown edit transactions"
```

---

### Task 4: Add multi-selection command lowering and grouped history

**Files:**
- Create: `crates/waml-markdown-editor/src/history.rs`
- Modify: `crates/waml-markdown-editor/src/edit.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`
- Modify: `crates/waml-markdown-editor/tests/document_ops.rs`

**Interfaces:**
- Consumes: `MarkdownEdit`, `TextChange`, selection affinity, and current immutable snapshot.
- Produces: `EditCommand::{Insert, DeleteBackward, DeleteForward, ReplaceSelections, Paste, Cut, Indent, Outdent}`; `EditOutcome { proposal, clipboard }`; `MarkdownDocumentSession::{execute, undo, redo, can_undo, can_redo, break_history_group}`.

- [ ] **Step 1: Add failing command and history tests**

Append tests that use these exact cases:

```rust
use waml_markdown_editor::edit::EditCommand;

#[test]
fn multi_selection_insert_is_lowered_from_end_to_start() {
    let before = snapshot("ab cd", 20);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![Selection::caret(p(1)), Selection::new(p(3), p(5))],
        1,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    let outcome = session
        .execute(EditCommand::Insert(Arc::from("X")), HistoryGroup::named(9))
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "aXb X");
    assert_eq!(outcome.proposal.unwrap().edit.changes.len(), 2);
}

#[test]
fn overlapping_selections_are_normalized_before_one_delete() {
    let before = snapshot("abcdef", 30);
    let p = |n| TextPosition::new(TextSize::try_from_usize(n).unwrap(), Affinity::Before);
    let selections = SelectionSet::from_selections(
        &before,
        vec![Selection::new(p(1), p(4)), Selection::new(p(3), p(5))],
        0,
    )
    .unwrap();
    let mut session = MarkdownDocumentSession::with_selections(before, selections).unwrap();
    let outcome = session
        .execute(EditCommand::DeleteBackward, HistoryGroup::named(2))
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "af");
    assert_eq!(outcome.proposal.unwrap().edit.changes.len(), 1);
}

#[test]
fn grouped_undo_and_redo_restore_source_and_selection_together() {
    let before = snapshot("", 40);
    let mut session = MarkdownDocumentSession::new(before);
    for ch in ["a", "b", "c"] {
        session
            .execute(EditCommand::Insert(Arc::from(ch)), HistoryGroup::named(1))
            .unwrap();
    }
    assert!(session.can_undo());
    let undo = session.undo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "");
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 0);
    assert_eq!(undo.edit.changes.len(), 1);
    session.redo().unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "abc");
}

#[test]
fn paste_cut_and_indent_keep_raw_markdown_in_transactions() {
    let before = snapshot("- a\n- b\n", 50);
    let mut session = MarkdownDocumentSession::new(before);
    session.select_all().unwrap();
    let cut = session
        .execute(EditCommand::Cut, HistoryGroup::isolated())
        .unwrap();
    assert_eq!(cut.clipboard.as_deref(), Some("- a\n- b\n"));
    assert_eq!(session.snapshot().text().shared().as_str(), "");
    session
        .execute(
            EditCommand::Paste(Arc::from("- a\n- b\n")),
            HistoryGroup::isolated(),
        )
        .unwrap();
    session.select_all().unwrap();
    session
        .execute(EditCommand::Indent { spaces: 2 }, HistoryGroup::isolated())
        .unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "  - a\n  - b\n");
}
```

- [ ] **Step 2: Run the tests and verify commands are absent**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: FAIL with unresolved `EditCommand` and missing session methods.

- [ ] **Step 3: Implement deterministic command lowering**

Define in `edit.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditCommand {
    Insert(Arc<str>),
    DeleteBackward,
    DeleteForward,
    ReplaceSelections(Arc<str>),
    Paste(Arc<str>),
    Cut,
    Indent { spaces: usize },
    Outdent { spaces: usize },
}

#[derive(Clone, Debug)]
pub struct EditOutcome {
    pub proposal: Option<ProposedMarkdownEdit>,
    pub clipboard: Option<String>,
}
```

For selection edits, iterate normalized selections by ascending old range to construct valid `TextChange` values. Calculate every resulting caret with the accumulated byte delta, give insertions `Affinity::After`, create `SelectionSet` bound to `next_revision`, and pass the resulting `MarkdownEdit` to `apply_edit`. Empty delete commands return `EditOutcome { proposal: None, clipboard: None }` without advancing revision.

For a one-character `Insert`, adapt Makepad's delimiter behavior with the exact pair table `('(', ')')`, `('[', ']')`, `('{', '}')`, `('"', '"')`, and ``('`', '`')``. Wrap each non-empty selection. At an empty caret, insert the pair only before EOF, whitespace, or a closing delimiter and leave the caret between the pair. Typing the already-present closing delimiter moves the selection after it without changing source or revision. The source-changing wrap/pair cases still produce one `MarkdownEdit`.

For indentation, enumerate each selected logical source line through `LineIndex`, produce one insertion or deletion per unique line, preserve CRLF bytes, and translate selections with `ChangeMap` affinities.

- [ ] **Step 4: Implement inverse history groups**

In `history.rs`, store:

```rust
#[derive(Clone, Debug)]
pub(crate) struct HistoryEntry {
    pub before: Arc<MarkdownDocumentSnapshot>,
    pub before_selection: SelectionSet,
    pub after: Arc<MarkdownDocumentSnapshot>,
    pub after_selection: SelectionSet,
    pub forward_changes: Vec<TextChange>,
    pub inverse_changes: Vec<TextChange>,
    pub group: HistoryGroup,
}

#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Vec<HistoryEntry>>,
    redo: Vec<Vec<HistoryEntry>>,
}
```

Add `history: History` to `MarkdownDocumentSession`. Compute inverse changes from the old source and `ChangeMap`, merge only adjacent entries whose nonzero `HistoryGroup` values match, clear redo on a new edit, and coalesce an undo group's inverse changes into one valid `MarkdownEdit` against the current revision. Rebind the restored endpoint offsets and affinities to the undo/redo transaction's `checked_next()` revision before constructing `selection_after`. `undo` and `redo` must call the same private application path as normal edits and emit `ProposedMarkdownEdit`; never replace the full string.

- [ ] **Step 5: Run all document operation tests**

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: PASS, 10 tests.

- [ ] **Step 6: Commit commands and history**

```bash
rtk git add crates/waml-markdown-editor/src/edit.rs crates/waml-markdown-editor/src/history.rs crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/document_ops.rs
rtk git commit -m "feat: add markdown commands and grouped history"
```

---

### Task 5: Add Unicode navigation, selection translation, and platform positions

**Files:**
- Create: `crates/waml-markdown-editor/src/unicode.rs`
- Modify: `crates/waml-markdown-editor/src/selection.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`
- Create: `crates/waml-markdown-editor/tests/unicode_ime.rs`

**Interfaces:**
- Consumes: `unicode_segmentation::UnicodeSegmentation`; `LineIndex`; `ChangeMap`; source `TextChange` values.
- Produces: `Utf16Position { line: u32, character: u32 }`; `PositionError`; `translate_selection_set`; `MarkdownDocumentSession::{move_left, move_right, move_vertical, select_word_at, select_line_at}`.

- [ ] **Step 1: Write failing CRLF, grapheme, UTF-16, and affinity tests**

Create `tests/unicode_ime.rs` with:

```rust
use std::sync::Arc;
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    selection::{Affinity, Selection, SelectionSet, TextPosition},
    session::MarkdownDocumentSession,
    unicode::{offset_to_utf16, utf16_to_offset, Utf16Position},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextChange, TextRange, TextSize,
};

fn session(text: &str) -> MarkdownDocumentSession {
    let source = SourceText::from_shared(Arc::new(text.to_owned())).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)))
}

#[test]
fn horizontal_navigation_moves_by_extended_grapheme_cluster() {
    let mut session = session("a👩‍💻e\u{301}z");
    session.set_primary_offset(TextSize::try_from_usize(1).unwrap()).unwrap();
    session.move_right(false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 12);
    session.move_right(false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 15);
    session.move_left(false).unwrap();
    assert_eq!(session.selections().primary().cursor.offset.to_usize(), 12);
}

#[test]
fn utf8_and_utf16_positions_round_trip_across_crlf_and_emoji() {
    let session = session("a\r\n😀b\n");
    let offset = TextSize::try_from_usize(7).unwrap();
    let position = offset_to_utf16(session.snapshot(), offset).unwrap();
    assert_eq!(position, Utf16Position { line: 1, character: 2 });
    assert_eq!(utf16_to_offset(session.snapshot(), position).unwrap(), offset);
}

#[test]
fn insertion_affinity_translates_equal_boundaries_differently() {
    let old = session("ab");
    let at = TextSize::try_from_usize(1).unwrap();
    let before = TextPosition::new(at, Affinity::Before);
    let after = TextPosition::new(at, Affinity::After);
    let change = TextChange {
        old_range: TextRange::new(at, at).unwrap(),
        replacement: Arc::from("X"),
    };
    assert_eq!(
        waml_markdown_editor::selection::translate_position(old.snapshot(), before, &[change.clone()])
            .unwrap()
            .offset
            .to_usize(),
        1
    );
    assert_eq!(
        waml_markdown_editor::selection::translate_position(old.snapshot(), after, &[change])
            .unwrap()
            .offset
            .to_usize(),
        2
    );
}

#[test]
fn triple_click_selects_one_logical_crlf_source_line() {
    let mut session = session("first\r\nsecond\r\n");
    let selection = session
        .select_line_at(TextSize::try_from_usize(9).unwrap())
        .unwrap();
    assert_eq!(
        session
            .snapshot()
            .text()
            .slice(selection.range())
            .unwrap(),
        "second\r\n"
    );
}
```

- [ ] **Step 2: Run the Unicode test and verify APIs are missing**

Run: `rtk cargo test -p waml-markdown-editor --test unicode_ime`

Expected: FAIL with unresolved Unicode adapter and navigation APIs.

- [ ] **Step 3: Implement Unicode adapters**

In `unicode.rs`, use `UnicodeSegmentation::grapheme_indices(text, true)` for previous/next extended grapheme boundaries and `unicode_word_indices` for double-click words. Do not copy Makepad's `str.rs::Graphemes`, which advances by Unicode scalar and is a deliberate divergence.

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Utf16Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PositionError {
    LineOutOfBounds { line: u32 },
    Utf16ColumnOutOfBounds { line: u32, character: u32 },
    SplitUtf16Scalar { line: u32, character: u32 },
    Text(waml_syntax::TextError),
}
```

Implement `offset_to_utf16` with `LineIndex::line_col` and `utf16_to_offset` by scanning the exact logical line from its UTF-8 start. Reject a UTF-16 column inside a surrogate pair. Treat `\r\n` as one logical newline while preserving both source bytes.

- [ ] **Step 4: Implement affinity-aware selection translation and navigation**

Add `translate_position(snapshot, position, changes)` and `translate_selection_set(old, new, changes)` in `selection.rs`. Use `ChangeMap` for unaffected offsets; for equal insertion boundaries, keep `Before` at the old boundary and move `After` past inserted bytes; for replaced ranges, map start affinity to the new start and end affinity to the new end. Validate the translated set against the new snapshot.

Add session navigation methods. Horizontal motion uses grapheme boundaries. `select_word_at` uses Unicode word segments and falls back to the containing grapheme for punctuation. `select_line_at` includes the line terminator except at EOF. Leave vertical navigation and preferred-horizontal-position state to `LayoutSnapshot::move_vertical` in Task 7 so the stored preference is measured in pixels, not columns.

- [ ] **Step 5: Run Unicode tests**

Run: `rtk cargo test -p waml-markdown-editor --test unicode_ime`

Expected: PASS, 4 tests.

- [ ] **Step 6: Commit Unicode-safe positions**

```bash
rtk git add Cargo.toml Cargo.lock crates/waml-markdown-editor/src/unicode.rs crates/waml-markdown-editor/src/selection.rs crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/unicode_ime.rs
rtk git commit -m "feat: add unicode-safe markdown positions"
```

---

### Task 6: Add uncommitted IME composition and cancellation

**Files:**
- Create: `crates/waml-markdown-editor/src/ime.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`
- Modify: `crates/waml-markdown-editor/tests/unicode_ime.rs`

**Interfaces:**
- Consumes: committed `MarkdownDocumentSnapshot`, primary selection, and platform UTF-16 composition selection.
- Produces: `ImeComposition`; `ImeError`; `MarkdownDocumentSession::{begin_ime, update_ime, commit_ime, cancel_ime, ime}`.

- [ ] **Step 1: Add failing composition lifecycle tests**

Append:

```rust
#[test]
fn ime_preedit_is_visible_state_but_not_a_published_revision() {
    let mut session = session("ab");
    session.set_primary_offset(TextSize::try_from_usize(1).unwrap()).unwrap();
    session.begin_ime().unwrap();
    session.update_ime("に", 0..1).unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "ab");
    assert_eq!(session.local_revision(), DocumentRevision::INITIAL);
    assert_eq!(session.ime().unwrap().preedit(), "に");

    let proposal = session.commit_ime(HistoryGroup::isolated()).unwrap().unwrap();
    assert_eq!(session.snapshot().text().shared().as_str(), "aにb");
    assert_eq!(session.local_revision().get(), 1);
    assert_eq!(proposal.edit.changes.len(), 1);
    assert!(session.ime().is_none());
}

#[test]
fn ime_cancel_restores_the_last_committed_snapshot_and_selection() {
    let mut session = session("a😀b");
    session.set_primary_offset(TextSize::try_from_usize(1).unwrap()).unwrap();
    let committed = session.snapshot().clone();
    let selection = session.selections().clone();
    session.begin_ime().unwrap();
    session.update_ime("漢字", 0..2).unwrap();
    session.cancel_ime();
    assert!(Arc::ptr_eq(session.snapshot(), &committed));
    assert_eq!(session.selections(), &selection);
    assert!(session.ime().is_none());
}
```

- [ ] **Step 2: Run and verify IME methods are absent**

Run: `rtk cargo test -p waml-markdown-editor --test unicode_ime`

Expected: FAIL with unresolved `HistoryGroup` import and missing IME methods.

- [ ] **Step 3: Implement composition state without source mutation**

In `ime.rs`:

```rust
use crate::selection::SelectionSet;
use std::{ops::Range, sync::Arc};
use waml_syntax::{DocumentRevision, TextRange};

#[derive(Clone, Debug)]
pub struct ImeComposition {
    base_revision: DocumentRevision,
    replace_range: TextRange,
    committed_snapshot: Arc<crate::document::MarkdownDocumentSnapshot>,
    committed_selection: SelectionSet,
    preedit: String,
    utf16_selection: Range<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImeError {
    AlreadyActive,
    NotActive,
    StaleRevision {
        base: DocumentRevision,
        current: DocumentRevision,
    },
    InvalidUtf16Selection {
        start: u32,
        end: u32,
        preedit_units: u32,
    },
    ReadOnly,
}
```

`begin_ime` captures the primary selection and committed `Arc` snapshot. `update_ime` validates the preedit UTF-16 range but modifies only `ImeComposition`. `commit_ime` lowers the final preedit to one `MarkdownEdit` replacing `replace_range`; an empty preedit acts as cancellation. `cancel_ime` restores the captured snapshot and selection without incrementing the revision. Any normal committed edit cancels an active composition first.

Add `ime: Option<ImeComposition>` to `MarkdownDocumentSession` and initialize it to `None`.

- [ ] **Step 4: Run IME and transaction tests**

Run: `rtk cargo test -p waml-markdown-editor --test unicode_ime`

Expected: PASS, 6 tests.

Run: `rtk cargo test -p waml-markdown-editor --test document_ops`

Expected: PASS, 10 tests.

- [ ] **Step 5: Commit IME behavior**

```bash
rtk git add crates/waml-markdown-editor/src/ime.rs crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/unicode_ime.rs
rtk git commit -m "feat: add markdown IME composition"
```

---

### Task 7: Define variable-metric layout geometry and exact queries

**Files:**
- Create: `crates/waml-markdown-editor/src/layout/mod.rs`
- Create: `crates/waml-markdown-editor/src/layout/geometry.rs`
- Create: `crates/waml-markdown-editor/tests/layout_geometry.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`

**Interfaces:**
- Consumes: spec 3 supplies a foundation-owned `LayoutDocument`; spec 1 supplies `SyntaxIdentity`; Makepad supplies `DVec2` and `Rect`.
- Produces: `LayoutElementId`; `GeometryElementId`; `FontKey`; `FontWeight`; `TextMetrics`; `LayoutTextRun`; `LayoutBlock`; `BlockLayoutSpec`; `MeasuredBlock`; `LayoutDocument`; `GlyphCluster`; `CaretStop`; `VisualLine`; `BlockGeometry`; `LayoutSnapshot`; source/point/selection/vertical-motion queries.

- [ ] **Step 1: Write failing hand-built geometry query tests**

Create `tests/layout_geometry.rs`:

```rust
use std::sync::Arc;
use makepad_widgets::{dvec2, DVec2, Rect};
use waml_markdown_editor::{
    layout::{
        Affinity, BlockGeometry, CaretStop, GlyphCluster, LayoutSnapshot, VisualLine,
    },
    selection::{Selection, TextPosition},
};
use waml_syntax::{DocumentRevision, TextRange, TextSize};

fn t(n: usize) -> TextSize {
    TextSize::try_from_usize(n).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}

#[test]
fn source_point_round_trip_handles_proportional_clusters_and_affinity() {
    let snapshot = LayoutSnapshot::from_parts_for_test(
        DocumentRevision::new(3),
        dvec2(120.0, 24.0),
        vec![VisualLine::for_test(range(0, 3), 0.0, 24.0)],
        vec![GlyphCluster::for_test(
            range(0, 3),
            Rect { pos: dvec2(0.0, 0.0), size: dvec2(30.0, 24.0) },
            vec![
                CaretStop::new(TextPosition::new(t(0), Affinity::Before), dvec2(0.0, 0.0)),
                CaretStop::new(TextPosition::new(t(1), Affinity::After), dvec2(9.0, 0.0)),
                CaretStop::new(TextPosition::new(t(3), Affinity::After), dvec2(30.0, 0.0)),
            ],
        )],
        Vec::<BlockGeometry>::new(),
    );
    for position in [
        TextPosition::new(t(0), Affinity::Before),
        TextPosition::new(t(1), Affinity::After),
        TextPosition::new(t(3), Affinity::After),
    ] {
        let point = snapshot.source_to_point(position).unwrap().rect.pos;
        assert_eq!(snapshot.point_to_source(point), position);
    }
}

#[test]
fn selection_rects_split_across_wrapped_mixed_height_lines() {
    let snapshot = LayoutSnapshot::wrapped_fixture_for_test();
    let selection = Selection::new(
        TextPosition::new(t(1), Affinity::Before),
        TextPosition::new(t(8), Affinity::After),
    );
    let rects = snapshot.selection_rects(selection).unwrap();
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].size.y, 18.0);
    assert_eq!(rects[1].size.y, 30.0);
}

#[test]
fn vertical_motion_uses_preferred_pixels_not_character_columns() {
    let snapshot = LayoutSnapshot::proportional_fixture_for_test();
    let start = TextPosition::new(t(2), Affinity::After);
    let (down, preferred_x) = snapshot.move_vertical(start, None, 1).unwrap();
    assert_eq!(preferred_x, 26.0);
    let (up, _) = snapshot.move_vertical(down, Some(preferred_x), -1).unwrap();
    assert_eq!(up, start);
}
```

- [ ] **Step 2: Run and verify geometry types are absent**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: FAIL with unresolved layout geometry types.

- [ ] **Step 3: Define the low-level presentation-to-layout seam**

In `layout/mod.rs`, re-export `crate::selection::Affinity` and define:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LayoutElementId {
    pub owner: waml_syntax::SyntaxIdentity,
    pub fragment_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GeometryElementId {
    pub layout: LayoutElementId,
    pub cluster_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontKey(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FontWeight(pub u16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextMetrics {
    pub font: FontKey,
    pub font_size: f32,
    pub line_spacing: f32,
    pub weight: FontWeight,
    pub italic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutTextRun {
    pub id: LayoutElementId,
    pub range: TextRange,
    pub metrics: TextMetrics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub size: DVec2,
    pub baseline: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColumnAlignment {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColumnConstraint {
    pub min_width: f64,
    pub max_width: Option<f64>,
    pub alignment: ColumnAlignment,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockFlow {
    Paragraph,
    Hanging {
        marker_range: TextRange,
        content_indent: f64,
    },
    Quote,
    Code,
    Table,
    TableRow,
    TableCell {
        column: u32,
    },
    Embedded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockLayoutSpec {
    pub flow: BlockFlow,
    pub insets: EdgeInsets,
    pub space_before: f64,
    pub space_after: f64,
    pub columns: Arc<[ColumnConstraint]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutBlock {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub spec: BlockLayoutSpec,
}

#[derive(Clone, Debug)]
pub struct LayoutDocument {
    pub revision: DocumentRevision,
    pub content_insets: EdgeInsets,
    pub blocks: Arc<[LayoutBlock]>,
    pub text_runs: Arc<[LayoutTextRun]>,
    pub embedded_blocks: Arc<[MeasuredBlock]>,
}
```

`LayoutDocument` is owned by the foundation crate but built by spec 3. Its `revision` must equal both the live `MarkdownDocumentSession` revision and the produced `LayoutSnapshot` revision; the layout engine returns `LayoutError::RevisionMismatch` before using input from another snapshot. `content_insets` carries the spec-3 24-logical-pixel document inset without hard-coding that visual choice in the engine. The parent-linked `LayoutBlock` tree gives the engine enough neutral flow information for paragraph/heading spacing, hanging lists, nested quotes, code blocks, table rows/cells and columns, and embedded blocks. It intentionally contains no semantic role, color, decoration, or link type, which prevents a foundation/presentation crate dependency cycle.

- [ ] **Step 4: Implement one immutable geometry authority**

In `layout/geometry.rs`, define immutable `VisualLine`, `BlockGeometry`, `GlyphCluster`, `CaretStop`, `CaretGeometry`, and:

```rust
#[derive(Clone, Debug)]
pub struct LayoutSnapshot {
    revision: DocumentRevision,
    viewport_width: f64,
    content_size: DVec2,
    visual_lines: Arc<[VisualLine]>,
    blocks: Arc<[BlockGeometry]>,
    clusters: Arc<[GlyphCluster]>,
    visible_source_range: TextRange,
    visible_block_range: Range<usize>,
}
```

Expose `revision`, `content_size`, `visible_source_range`, `visible_block_range`, `source_to_point`, `point_to_source`, `selection_rects`, and `move_vertical`. Binary-search source-sorted caret stops for source queries and visual-line y ranges for point queries. Resolve equal offsets with exact `Affinity`; never reconstruct geometry separately for caret or selection. `move_vertical` keeps the original x in logical pixels as `preferred_x`.

Each `GlyphCluster` must carry a unique `GeometryElementId`. Assign `cluster_ordinal` in stable source-cluster order within one `LayoutTextRun`, independent of wrapping and visual bidi order. Spec 3 motion matches clusters by `(LayoutElementId, cluster_ordinal)`; two glyph clusters from one run must never share an identity.

Expose the small hand-built geometry constructors used by `tests/layout_geometry.rs` as `#[doc(hidden)] pub` functions. Do not put them behind `#[cfg(test)]`, because integration tests compile the library without that configuration; do not add a feature that can change production behavior.

- [ ] **Step 5: Run geometry query tests**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: PASS, 3 tests.

- [ ] **Step 6: Connect session vertical movement**

Add `MarkdownDocumentSession::move_vertical(&mut self, layout: &LayoutSnapshot, lines: i32, extend: bool)`. Reject a layout whose revision differs from the session revision, call `LayoutSnapshot::move_vertical`, store `preferred_x`, and reset `preferred_x` on horizontal movement, edits, and pointer placement.

Add `preferred_x: Option<f64>` to `MarkdownDocumentSession` and initialize it to `None`.

- [ ] **Step 7: Commit shared geometry contracts**

```bash
rtk git add crates/waml-markdown-editor/src/layout crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "feat: define variable metric markdown geometry"
```

---

### Task 8: Implement shaping, wrapping, fallback, and viewport virtualization

**Files:**
- Create: `crates/waml-markdown-editor/src/layout/engine.rs`
- Create: `crates/waml-markdown-editor/src/layout/makepad.rs`
- Modify: `crates/waml-markdown-editor/src/layout/mod.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`

**Interfaces:**
- Consumes: `LayoutDocument`, immutable document snapshot, `MarkdownSyntaxUpdate::affected_ranges`, and Makepad `DrawText::layout`.
- Produces: `TextShaper`; `ShapedRun`; `LayoutViewport`; `LayoutInvalidation`; `LayoutError`; `LayoutEngine::layout`; `BlockSummary`; `MakepadTextShaper`.

- [ ] **Step 1: Add failing mixed-metric, resize, fallback, and virtualization tests**

Use a deterministic fake `TextShaper` whose glyph advances equal values supplied by `FontKey`. Add:

```rust
#[test]
fn mixed_metrics_wrap_without_a_cell_width() {
    let (document, presentation, mut shaper) =
        fixtures::mixed_heading_and_body(80.0);
    let mut engine = LayoutEngine::default();
    let layout = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(80.0, 60.0, 0.0, 24.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(layout.visual_lines()[0].height(), 30.0);
    assert_eq!(layout.visual_lines()[1].height(), 16.0);
    assert!(layout.visual_lines().len() > 2);
}

#[test]
fn viewport_shapes_only_visible_blocks_plus_overscan() {
    let (document, presentation, mut shaper) = fixtures::one_hundred_blocks();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 100.0, 800.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert!(shaper.shaped_block_count() < 20);
    assert_eq!(layout.block_summaries().len(), 100);
    assert!(layout.content_size().y >= 2_000.0);
}

#[test]
fn width_change_rewraps_without_changing_document_revision() {
    let (document, presentation, mut shaper) = fixtures::paragraph();
    let mut engine = LayoutEngine::default();
    let wide = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    let narrow = engine
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(120.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::ViewportWidth,
            &mut shaper,
        )
        .unwrap();
    assert_eq!(wide.revision(), narrow.revision());
    assert!(narrow.visual_lines().len() > wide.visual_lines().len());
}

#[test]
fn failed_block_uses_editable_plain_text_fallback() {
    let (document, presentation, mut shaper) = fixtures::failing_second_block();
    let layout = LayoutEngine::default()
        .layout(
            &document,
            &presentation,
            LayoutViewport::new(400.0, 200.0, 0.0, 40.0),
            LayoutInvalidation::Document,
            &mut shaper,
        )
        .unwrap();
    assert!(layout.blocks()[1].is_plain_text_fallback());
    let source = layout.blocks()[1].source_range();
    assert_eq!(
        layout.point_to_source(layout.source_to_point(TextPosition::new(
            source.start(),
            Affinity::Before
        )).unwrap().rect.pos),
        TextPosition::new(source.start(), Affinity::Before)
    );
}
```

- [ ] **Step 2: Run and verify the engine APIs are absent**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: FAIL with unresolved `LayoutEngine`, `LayoutViewport`, and `LayoutInvalidation`.

- [ ] **Step 3: Implement a testable shaping boundary**

Define:

```rust
pub trait TextShaper {
    fn shape(
        &mut self,
        source: &SourceText,
        run: &LayoutTextRun,
        max_width: f64,
    ) -> Result<ShapedRun, LayoutError>;
}

#[derive(Clone, Debug)]
pub struct ShapedRun {
    pub clusters: Arc<[ShapedCluster]>,
    pub ascender: f64,
    pub descender: f64,
    pub line_gap: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutViewport {
    pub width: f64,
    pub height: f64,
    pub scroll_y: f64,
    pub overscan: f64,
}

#[derive(Clone, Debug)]
pub enum LayoutInvalidation {
    Document,
    SyntaxUpdate(MarkdownSyntaxUpdate),
    ViewportWidth,
    BlockMeasurement(LayoutElementId),
}
```

`ShapedCluster` carries source range, visual advance, bidi level, and a source-ordered array of caret offsets. Missing glyphs remain shaped by Makepad fallback and retain the same source range.

- [ ] **Step 4: Implement block summaries and incremental relayout**

`LayoutEngine` caches summaries by `LayoutElementId`:

```rust
#[derive(Clone, Debug)]
pub struct BlockSummary {
    pub id: LayoutElementId,
    pub source_range: TextRange,
    pub parent: Option<LayoutElementId>,
    pub flow_fingerprint: u64,
    pub y: f64,
    pub height: f64,
    pub width_key: u64,
    pub content_fingerprint: u64,
}
```

For a syntax update, find the first block intersecting the first `affected_range`, invalidate through the last affected block, then continue laying out downstream blocks until id, parent, flow spec, fingerprint, width key, y delta, and height equal the cached summary. For viewport-width invalidation, invalidate every text block's wrap and table column solution while retaining syntax identity. Shape only summaries intersecting `[scroll_y - overscan, scroll_y + height + overscan]`; keep all summaries for total scroll extent and stable navigation.

Lay out the parent-linked block tree inside `LayoutDocument::content_insets`. Apply before/after spacing at sibling boundaries, add nested `EdgeInsets`, preserve hanging marker flow while indenting content, and solve table column widths from all visible row/cell `ColumnConstraint` values plus cached off-screen minimum widths. Wrap only at shaped cluster boundaries. The visual line height is the maximum ascender/descender/line-gap of its runs and embedded inline items. An individual block shaping failure creates a plain body-style run over the block's literal source and marks `BlockGeometry::is_plain_text_fallback`.

- [ ] **Step 5: Implement the Makepad shaper**

In `layout/makepad.rs`, define `FontResolver` and `MakepadTextShaper`:

```rust
pub trait FontResolver {
    fn configure_draw_text(&mut self, key: FontKey, metrics: TextMetrics, draw: &mut DrawText);
}

pub struct MakepadTextShaper<'a, R> {
    pub cx: &'a mut Cx,
    pub draw_text: &'a mut DrawText,
    pub fonts: &'a mut R,
}
```

Call `DrawText::layout(cx, 0.0, 0.0, Some(max_width as f32), true, Align::default(), text)`. Convert `LaidoutText.rows`, each `LaidoutGlyph.cluster`, glyph origin, advance, ascender, and descender into exact UTF-8 `ShapedCluster` ranges relative to `LayoutTextRun::range`. Use Makepad's row visual order for bidi geometry and retain source affinity at duplicate bidi boundaries. `FontResolver` maps the spec-3 `FontKey` to Makepad font family/style; a missing key configures the body fallback rather than dropping text.

- [ ] **Step 6: Run layout tests**

Run: `rtk cargo test -p waml-markdown-editor --test layout_geometry`

Expected: PASS, 7 tests.

- [ ] **Step 7: Commit layout and virtualization**

```bash
rtk git add crates/waml-markdown-editor/src/layout crates/waml-markdown-editor/tests/layout_geometry.rs
rtk git commit -m "feat: lay out visible markdown blocks"
```

---

### Task 9: Add platform-neutral input, read-only behavior, and caret scrolling

**Files:**
- Create: `crates/waml-markdown-editor/src/input.rs`
- Modify: `crates/waml-markdown-editor/src/session.rs`
- Create: `crates/waml-markdown-editor/tests/widget_parity.rs`

**Interfaces:**
- Consumes: session command lowering and `LayoutSnapshot` queries.
- Produces: `EditorInput`; `SelectionModifier`; `PointerGesture`; `EditorResponse`; `ScrollState`; `ScrollAnchor`; `ScrollAdjustment`; `MarkdownEditorController::handle`.

- [ ] **Step 1: Write failing retained-behavior characterization tests**

Create `tests/widget_parity.rs`:

```rust
#[test]
fn click_drag_double_and_triple_click_match_retained_editor_behavior() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(2, 1, SelectionModifier::Replace);
    assert!(fixture.primary().is_empty());
    fixture.drag_to_offset(5);
    assert_eq!(fixture.selected_text(), "pha");
    fixture.click_at_offset(8, 2, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "beta");
    fixture.click_at_offset(13, 3, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "second\n");
}

#[test]
fn platform_modifier_adds_selection_and_shift_extends_primary() {
    let mut fixture = Fixture::new("one two");
    fixture.click_at_offset(1, 1, SelectionModifier::Replace);
    fixture.click_at_offset(5, 1, SelectionModifier::Add);
    assert_eq!(fixture.session().selections().as_slice().len(), 2);
    fixture.click_at_offset(7, 1, SelectionModifier::Extend);
    assert_eq!(fixture.selected_text(), "two");
}

#[test]
fn read_only_mode_allows_selection_and_copy_but_not_mutation() {
    let mut fixture = Fixture::new("raw *markdown*");
    fixture.session_mut().set_read_only(true);
    fixture.select_all();
    assert_eq!(fixture.copy(), "raw *markdown*");
    let response = fixture.type_text("x");
    assert!(response.proposals.is_empty());
    assert_eq!(fixture.text(), "raw *markdown*");
}

#[test]
fn caret_visibility_and_resize_use_geometry_not_line_numbers() {
    let mut fixture = Fixture::with_variable_layout();
    fixture.set_viewport(100.0, 40.0);
    fixture.place_caret_at_end();
    let first = fixture.ensure_caret_visible();
    assert!(first.scroll_y > 0.0);
    fixture.resize_width(50.0);
    let second = fixture.ensure_caret_visible();
    assert!(second.scroll_y >= first.scroll_y);
}
```

- [ ] **Step 2: Run and verify the controller is absent**

Run: `rtk cargo test -p waml-markdown-editor --test widget_parity`

Expected: FAIL with unresolved input/controller types.

- [ ] **Step 3: Implement typed input and response**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionModifier {
    Replace,
    Extend,
    Add,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorInput {
    Text(Arc<str>),
    Paste(Arc<str>),
    Copy,
    Cut,
    Key(EditorKey),
    PointerDown(PointerGesture),
    PointerMove { point: DVec2 },
    PointerUp,
    ImeStart,
    ImeUpdate { preedit: String, selection: Range<u32> },
    ImeCommit,
    ImeCancel,
}

#[derive(Default)]
pub struct EditorResponse {
    pub proposals: Vec<ProposedMarkdownEdit>,
    pub clipboard: Option<String>,
    pub request_redraw: bool,
    pub request_ime_at: Option<DVec2>,
}
```

Map normal click to caret, drag/Shift to extension, double-click to Unicode word, triple-click to logical source line, and the platform add-selection modifier to a new selection. Copy always concatenates literal selected Markdown source in selection order. Cut, paste, typing, Enter, Tab, Shift-Tab, Delete, Backspace, undo, redo, and IME commit all call the same session transaction APIs. In read-only mode, preserve focus, navigation, selection, scrolling, and copy while returning no edit proposal.

- [ ] **Step 4: Implement scroll state and anchoring**

Define:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollState {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAnchor {
    pub position: TextPosition,
    pub viewport_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollAdjustment {
    pub scroll_y: f64,
}
```

`ensure_primary_caret_visible` uses `LayoutSnapshot::source_to_point` plus one caret-height pad above and two below. `capture_scroll_anchor` records the primary caret's viewport y. `restore_scroll_anchor` computes the new scroll y from the new layout geometry after edits, font changes, embedded-block measurement, or viewport resize. Clamp only scroll coordinates, never source offsets.

Add `scroll: ScrollState` to `MarkdownDocumentSession` and initialize it to `ScrollState::default()`.

- [ ] **Step 5: Run parity tests**

Run: `rtk cargo test -p waml-markdown-editor --test widget_parity`

Expected: PASS, 4 tests.

- [ ] **Step 6: Commit controller and scroll behavior**

```bash
rtk git add crates/waml-markdown-editor/src/input.rs crates/waml-markdown-editor/src/session.rs crates/waml-markdown-editor/tests/widget_parity.rs
rtk git commit -m "feat: add markdown editor input controller"
```

---

### Task 10: Add the Makepad widget and one ordered draw pipeline

**Files:**
- Create: `crates/waml-markdown-editor/src/widget.rs`
- Modify: `crates/waml-markdown-editor/src/lib.rs`
- Modify: `crates/waml-markdown-editor/tests/widget_parity.rs`
- Modify: `crates/waml-markdown-editor/tests/provenance.rs`

**Interfaces:**
- Consumes: external `&mut MarkdownDocumentSession` through Makepad `Scope`, `LayoutEngine`, `MakepadTextShaper`, and one current/interpolated `Arc<LayoutSnapshot>`.
- Produces: `MarkdownEditor`; `MarkdownEditorAction::{ProposedEdit, SelectionChanged, NavigationRequested, EmbeddedBlockEvent}`; `DrawLayer`; `EmbeddedBlockEvent`; `MarkdownEditorRef` extension methods; `live_design`.

- [ ] **Step 1: Add failing mounted-widget and draw-order tests**

Add tests that mount the widget with Makepad's test VM:

```rust
#[test]
fn mounted_widget_emits_the_exact_proposal_from_text_input() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    widget.set_key_focus(&mut cx);
    let actions = widget.test_handle_input(
        &mut cx,
        &mut session,
        EditorInput::Text(Arc::from("x")),
    );
    let proposal = MarkdownEditorRef::proposed_edit(&actions).unwrap();
    assert_eq!(proposal.edit.base_revision, DocumentRevision::INITIAL);
    assert_eq!(proposal.edit.changes.len(), 1);
    assert_eq!(proposal.snapshot.text().shared().as_str(), "xab");
}

#[test]
fn every_layer_uses_one_layout_snapshot_in_required_order() {
    let mut recorder = DrawRecorder::default();
    let layout = Arc::new(LayoutSnapshot::wrapped_fixture_for_test());
    draw_visible_layers_for_test(&layout, &mut recorder);
    assert_eq!(
        recorder.layers(),
        &[
            DrawLayer::BlockBackground,
            DrawLayer::Selection,
            DrawLayer::Text,
            DrawLayer::Decoration,
            DrawLayer::EmbeddedBlock,
            DrawLayer::CaretAndIme,
        ]
    );
    assert!(recorder
        .snapshot_ptrs()
        .iter()
        .all(|ptr| *ptr == Arc::as_ptr(&layout)));
}

#[test]
fn ime_window_uses_current_interpolated_caret_geometry() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    let target = Arc::new(LayoutSnapshot::wrapped_fixture_for_test());
    widget.test_set_layout(target.clone());
    widget.test_show_ime(&mut cx, &mut session);
    assert_eq!(
        widget.test_last_ime_point(),
        target
            .source_to_point(session.selections().primary().cursor)
            .unwrap()
            .rect
            .pos
    );
}
```

- [ ] **Step 2: Run and verify widget types are absent**

Run: `rtk cargo test -p waml-markdown-editor --test widget_parity`

Expected: FAIL with unresolved `MarkdownEditor`, `MarkdownEditorRef`, and `DrawLayer`.

- [ ] **Step 3: Implement the explicit session/widget boundary**

Adapt Makepad's `CodeEditor::draw_walk_editor`/`handle_event` pattern, not `CodeView`'s privately owned `CodeSession`. Define the exact host scope first:

```rust
pub struct MarkdownEditorScope<'a> {
    pub session: &'a mut MarkdownDocumentSession,
}
```

`MarkdownEditorScope` carries only the live editing session. Presentation input is installed through `MarkdownEditorRef`; it is not duplicated in event scope. `MarkdownEditor` keeps only widget-local rendering/input state:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct MarkdownEditor {
    #[deref]
    view: View,
    #[rust]
    controller: MarkdownEditorController,
    #[rust]
    layout_engine: LayoutEngine,
    #[rust]
    layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    presentation: Option<Arc<LayoutDocument>>,
    #[rust]
    pointer_drag_active: bool,
    #[live(false)]
    read_only: bool,
    #[live(false)]
    reduced_motion: bool,
}
```

Pass `MarkdownEditorScope<'_> { session: &mut MarkdownDocumentSession }` through `Scope::with_data` for both event and draw. This makes the host (`SourceView` in spec 4) own the live session while the widget owns no second source authority. At the end of this foundation plan, `set_layout_document` installs neutral layout input and the widget draws the engine's target snapshot. Spec 3 then adds `MotionController` to this same widget and replaces host use of this provisional method with one revision-checked `install_presentation` call. No host owns or installs an independent `LayoutSnapshot`.

- [ ] **Step 4: Implement event adaptation and typed actions**

Convert Makepad `Hit::TextInput`, `TextCopy`, `TextCut`, key events, finger down/move/up, focus, and scroll events into `EditorInput`. Forward `EditorResponse` proposals without converting them to full strings:

```rust
#[derive(Clone, Debug, DefaultNone)]
pub enum MarkdownEditorAction {
    ProposedEdit(ProposedMarkdownEdit),
    SelectionChanged,
    NavigationRequested { position: TextPosition },
    EmbeddedBlockEvent {
        id: LayoutElementId,
        event: EmbeddedBlockEvent,
    },
    None,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmbeddedBlockEvent {
    Activated,
    RetryRequested,
    MeasurementChanged { size: DVec2 },
}
```

The widget must call `cx.show_text_ime` with the caret from the exact `LayoutSnapshot` used for drawing and `cx.hide_text_ime` on focus loss or read-only mode. Ctrl/Cmd-click emits `NavigationRequested`; the foundation does not resolve or activate links. Embedded children use the same action channel and identify their stable `LayoutElementId`; spec 3 or the host maps that id to image/link semantics.

- [ ] **Step 5: Implement the fixed draw order**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawLayer {
    BlockBackground,
    Selection,
    Text,
    Decoration,
    EmbeddedBlock,
    CaretAndIme,
}
```

Draw only `layout.visible_block_range()` and its cluster geometry. Every layer function takes `&Arc<LayoutSnapshot>`; selection, caret, IME, and hit-testing must call snapshot queries instead of recomputing glyph geometry. Use Makepad fallback glyph rendering from the shaped layout. An affected-block fallback remains fully editable and uses the same selection/caret layers.

- [ ] **Step 6: Register the widget and expose one host API**

Implement `live_design(cx)` by registering `widget::live_design(cx)`. Expose these exact `MarkdownEditorRef` methods; do not add a second presentation or embedded-widget action channel:

```rust
impl MarkdownEditorRef {
    pub fn set_key_focus(&self, cx: &mut Cx);
    pub fn redraw(&self, cx: &mut Cx);
    pub fn set_read_only(&self, cx: &mut Cx, read_only: bool);
    pub fn set_reduced_motion(&self, cx: &mut Cx, reduced: bool);
    pub fn set_layout_document(&self, cx: &mut Cx, document: Arc<LayoutDocument>);
    pub fn target_layout(&self) -> Option<Arc<LayoutSnapshot>>;
    pub fn proposed_edit(actions: &Actions) -> Option<ProposedMarkdownEdit>;
    pub fn selection_changed(actions: &Actions) -> bool;
    pub fn navigation_request(actions: &Actions) -> Option<TextPosition>;
    pub fn embedded_block_event(
        actions: &Actions,
    ) -> Option<(LayoutElementId, EmbeddedBlockEvent)>;
}
```

Do not register or instantiate Makepad `Markdown`.

- [ ] **Step 7: Run widget and provenance tests**

Run: `rtk cargo test -p waml-markdown-editor --test widget_parity`

Expected: PASS, 7 tests.

Run: `rtk cargo test -p waml-markdown-editor --test provenance`

Expected: PASS, 2 tests.

- [ ] **Step 8: Commit the standalone widget**

```bash
rtk git add crates/waml-markdown-editor/src/widget.rs crates/waml-markdown-editor/src/lib.rs crates/waml-markdown-editor/tests/widget_parity.rs crates/waml-markdown-editor/tests/provenance.rs
rtk git commit -m "feat: add makepad markdown editor widget"
```

---

### Task 11: Close the fork-parity, geometry, and failure matrix

**Files:**
- Modify: `crates/waml-markdown-editor/tests/document_ops.rs`
- Modify: `crates/waml-markdown-editor/tests/unicode_ime.rs`
- Modify: `crates/waml-markdown-editor/tests/layout_geometry.rs`
- Modify: `crates/waml-markdown-editor/tests/widget_parity.rs`
- Modify: `crates/waml-markdown-editor/PROVENANCE.md`

**Interfaces:**
- Consumes: all public foundation APIs.
- Produces: named evidence for each retained behavior and each deliberate divergence; no new production interface.

- [ ] **Step 1: Add the remaining document-operation matrix**

Add table-driven tests for insert, delete, replace, paste, cut, indent, outdent, automatic delimiter insertion, undo, redo, and history group breaks. Include tabs, empty source, EOF, multiple carets, selections that touch, and a stale proposal after one accepted local edit. Assert exact old ranges, replacement strings, resulting source, selection revision, and one revision increment per accepted transaction.

- [ ] **Step 2: Add the remaining Unicode and IME matrix**

Add cases for LF, CRLF, mixed line endings, combining marks, emoji ZWJ sequences, regional-indicator flags, non-Latin words, malformed UTF-16 columns, composition replacement of a non-empty selection, composition cancellation on focus loss, and stale composition commit. Every invalid position must match a concrete `PositionError`, `SelectionError`, or `ImeError` variant.

- [ ] **Step 3: Add the remaining geometry matrix**

Add deterministic tests for mixed font sizes, proportional widths, bidi visual order with both affinities at a boundary, empty lines, EOF, styled delimiter boundaries, selection across blocks, content extent, caret visibility, viewport resize, embedded-block measurement invalidation, affected-range stable-boundary stopping, and off-screen virtualization. Assert source-to-point-to-source round trips for every caret stop.

- [ ] **Step 4: Name retained and divergent Makepad behaviors**

In `widget_parity.rs`, keep these names explicit:

```text
retained_normal_click_places_caret
retained_drag_extends_selection
retained_double_click_selects_word
retained_triple_click_selects_source_line
retained_platform_modifier_adds_selection
retained_copy_cut_paste_use_source_text
retained_undo_redo_restore_selection
retained_keyboard_motion_keeps_caret_visible
divergence_extended_graphemes_replace_scalar_steps
divergence_variable_metrics_replace_fixed_cell_grid
divergence_ime_preedit_is_not_committed_text
divergence_widget_emits_exact_changes_not_full_string
```

Update the corresponding `PROVENANCE.md` material-change row when a divergence test reveals a behavior not already listed.

- [ ] **Step 5: Run all crate tests**

Run: `rtk cargo test -p waml-markdown-editor`

Expected: PASS with all unit and integration tests; no ignored parity or geometry tests.

- [ ] **Step 6: Commit the completed behavior matrix**

```bash
rtk git add crates/waml-markdown-editor/PROVENANCE.md crates/waml-markdown-editor/tests
rtk git commit -m "test: close markdown editor behavior matrix"
```

---

### Task 12: Verify the workspace and dependency boundary

**Files:**
- Modify only if verification finds a defect: files already named in Tasks 1-11.

**Interfaces:**
- Consumes: the completed crate.
- Produces: formatting, test, lint, dependency, and forbidden-import evidence.

- [ ] **Step 1: Format the implementation**

Run: `rtk cargo fmt --all`

Expected: exit 0.

- [ ] **Step 2: Verify formatting**

Run: `rtk cargo fmt --all --check`

Expected: exit 0 with no diff.

- [ ] **Step 3: Run the focused crate**

Run: `rtk cargo test -p waml-markdown-editor`

Expected: PASS.

- [ ] **Step 4: Run the full workspace**

Run: `rtk cargo test --workspace`

Expected: PASS.

- [ ] **Step 5: Run strict Clippy**

Run: `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings`

Expected: exit 0 with no warnings.

- [ ] **Step 6: Prove the forbidden upstream abstractions are absent**

Run:

```bash
rtk rg -n "makepad[-_]code[-_]editor|MarkdownAction|as_markdown\(\)|\bCodeEditor\b|\bCodeSession\b" crates/waml-markdown-editor
```

Expected: matches occur only in `PROVENANCE.md` and test names/documentation that describe the forbidden dependency; no Cargo dependency or production import match.

- [ ] **Step 7: Inspect the dependency tree**

Run:

```bash
rtk cargo tree -p waml-markdown-editor
```

Expected: includes `waml-syntax`, `makepad-widgets`, and `unicode-segmentation`; excludes `makepad-code-editor` and any Makepad Markdown widget crate.

- [ ] **Step 8: Commit verification-only formatting if needed**

If `cargo fmt` changed files:

```bash
rtk git add crates/waml-markdown-editor Cargo.toml Cargo.lock
rtk git commit -m "chore: format markdown editor foundation"
```

If it changed no files, do not create an empty commit.

---

## Cross-Plan Interface Contract

Spec 1 must land these exact `waml-syntax` exports before Task 2:

```rust
pub struct DocumentRevision(u64);
impl DocumentRevision {
    pub const INITIAL: Self;
    pub const fn new(value: u64) -> Self;
    pub const fn get(self) -> u64;
    pub fn checked_next(self) -> Option<Self>;
}

pub fn parse_markdown(
    revision: DocumentRevision,
    text: SourceText,
    dialect: MarkdownDialect,
) -> Result<Arc<MarkdownSyntaxSnapshot>, ParseError>;

pub fn reparse_markdown(
    previous: &MarkdownSyntaxSnapshot,
    revision: DocumentRevision,
    new_text: SourceText,
    changes: &[TextChange],
) -> Result<MarkdownSyntaxUpdate, ParseError>;

#[derive(Clone, Debug)]
pub struct TextChange {
    pub old_range: TextRange,
    pub replacement: Arc<str>,
}

#[derive(Clone)]
pub struct MarkdownSyntaxUpdate {
    pub snapshot: Arc<MarkdownSyntaxSnapshot>,
    pub affected_ranges: Arc<[TextRange]>,
    pub outcome: MarkdownReparseOutcome,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxIdentity(NonZeroU64);
```

Spec 3 owns semantic `PresentationItem` generation and motion. It must:

1. Convert semantic presentation into foundation `LayoutDocument`.
2. Use `SyntaxIdentity` plus a fragment ordinal as `LayoutElementId`, and preserve the foundation-assigned cluster ordinal for `GeometryElementId`.
3. Build `LayoutBlock` parent/flow specs for paragraph spacing, hanging lists, quotes, code, tables, and embedded blocks; set the balanced document inset through `LayoutDocument::content_insets`.
4. Map live typography tokens to `TextMetrics`/`FontKey`.
5. Add `MotionController`, previous/target/frame `Arc<LayoutSnapshot>` values, and frame scheduling to `MarkdownEditor`; the widget is the sole layout-snapshot and motion owner.
6. Define one immutable, revision-checked `InstalledPresentation` bundle containing the presentation plan, styles, `LayoutDocument`, diagnostics, and embedded-asset frame.
7. Replace host use of the provisional `set_layout_document` method with `MarkdownEditorRef::install_presentation(cx, Arc<InstalledPresentation>, LayoutChangeCause)`. Do not expose `set_resolved_layout` to the host.
8. Use `set_reduced_motion` for the motion accessibility cutover.
9. Put diagnostics and link decoration into `DrawLayer::Decoration`.
10. Route embedded image events through `MarkdownEditorAction::EmbeddedBlockEvent`.
11. Preserve the foundation draw order and never reconstruct caret, selection, diagnostic, or embedded-block geometry independently.

Spec 4 must:

1. Store `MarkdownDocumentSession` in `SourceView`.
2. Pass only that session through `MarkdownEditorScope`; install the current spec-3 `InstalledPresentation` through `MarkdownEditorRef::install_presentation`.
3. Route `MarkdownEditorAction::ProposedEdit` through `DocumentHost` and application `EditorSession`.
4. Promote `ProposedMarkdownEdit.syntax_update` instead of parsing the accepted revision again.
5. Preserve `ScrollState` and selection as view-local state.
6. Keep persistence, conflict policy, navigation, and external reload ownership outside this crate.
