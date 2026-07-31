# Markdown editor foundation — design

**Date:** 2026-07-31
**Status:** Approved in conversation; written-spec review pending
**Sequence:** 2 of 4
**Depends on:** Incremental Markdown syntax platform

## Problem

Makepad's `CodeEditor` has mature document interaction behavior, but its
fixed-cell layout cannot express variable-sized headings, proportional body
text, embedded images, or rendered tables. Makepad's `Markdown` widget renders
formatted documents but is read-only and does not own source selection, caret
geometry, history, IME, or clipboard behavior.

Patching either upstream widget would leave WAML constrained by an abstraction
that does not fit the required editor.

## Goal

Create a WAML-owned Markdown editor foundation that:

- retains Markdown source as the only editable document;
- adapts the proven Makepad editor interaction behavior;
- consumes immutable syntax snapshots from spec 1;
- supports mixed text metrics and exact source-to-screen mapping;
- exposes a clean widget/session boundary for presentation and application
  integration.

## Ownership and provenance

Add a dedicated `waml-markdown-editor` crate in this repository.

Fork or adapt the minimum cohesive parts of Makepad CodeEditor needed for:

- edit transactions;
- selections and multi-selection;
- undo/redo grouping;
- clipboard;
- IME composition;
- keyboard and pointer navigation;
- scrolling and caret visibility.

Each forked module records its upstream repository, commit revision, license,
and material changes. WAML owns subsequent evolution. The crate may depend on
Makepad's low-level widget, drawing, font, input, and platform primitives, but
not on the upstream `CodeEditor` or `Markdown` widgets.

## Document session

Name the widget-local editing authority `MarkdownDocumentSession` to avoid
confusion with the application-level `waml-editor::EditorSession`.

It owns:

- the current immutable document snapshot;
- current selections and primary caret;
- undo/redo history;
- active IME composition;
- view-local scroll and preferred-column state;
- a monotonic local edit revision.

The snapshot contains source text, line index, Markdown syntax snapshot, and
revision identity. Creating an edit yields a new snapshot; published snapshots
never mutate.

## Edit transactions

All text modifications use one transaction shape:

```rust
pub struct MarkdownEdit {
    pub base_revision: DocumentRevision,
    pub changes: Vec<TextChange>,
    pub selection_after: SelectionSet,
    pub history_group: HistoryGroup,
}
```

An edit is accepted only against its declared base revision. Insert, delete,
replace, paste, cut, indentation, automatic delimiter insertion, undo, redo,
and IME commit all lower to this representation.

The widget applies a valid local edit immediately, obtains the incremental
syntax update from spec 1, updates layout, and emits a proposed edit containing
both the exact changes and that immutable syntax update. An application host
validates and promotes the same update; it does not parse the same document
revision again. A standalone host may accept the proposal directly. The widget
never emits only a replacement full string.

## Positions and Unicode

Parser and document positions use WAML `TextSize`/`TextRange` UTF-8 byte
offsets. Every stored position must be a valid source boundary.

Adapters convert platform UTF-16 positions and line/column coordinates through
the snapshot's `LineIndex`. Cursor navigation and deletion respect user-visible
text boundaries; source offsets remain exact even for combining sequences,
emoji, and non-Latin input.

Selections are revision-bound ranges with explicit affinity. Translating a
selection across snapshots uses the applied text changes and affinity rules,
matching the snapshot-tracking model used by modern editors.

## Variable-metric layout contract

The foundation owns layout and hit-testing, while spec 3 supplies presentation
styles and embedded blocks.

Layout consumes:

- immutable source and syntax snapshots;
- presentation runs keyed to source ranges;
- viewport width and scroll window;
- font metrics and block measurements.

It produces:

- wrapped visual lines and block geometry;
- glyph clusters mapped to exact source ranges;
- caret stops and selection rectangles;
- source-to-point and point-to-source queries;
- visible-range and total-content measurements;
- stable element identities used by motion.

Text metrics may vary within and between lines. No fixed-cell assumption may
enter cursor movement, selection, wrapping, or scrolling.

## Input behavior

- A normal click places the caret; dragging extends selection.
- Double-click selects a word; triple-click selects the logical source line.
- Shift and platform selection modifiers follow native conventions.
- Clipboard text is always raw Markdown source.
- Undo/redo restores source and selection as one transaction.
- IME composition is visibly represented without publishing incomplete
  composition as a committed document revision.
- The caret remains visible through edits and font/layout changes.
- Read-only mode remains available for consumers that need it.

Link activation is not handled by the foundation; presentation exposes link
ranges and the host decides that Ctrl/Cmd-click navigates.

## Viewport behavior

Parsing covers the document snapshot, but glyph instancing and embedded-widget
work are limited to the visible block window plus a small overscan region.
Off-screen blocks retain measured summaries sufficient for scroll extent and
stable navigation.

Relayout starts at the syntax update's first affected block and continues until
the downstream geometry reaches a stable boundary. Changes in viewport width
invalidate wrapping without invalidating syntax.

## Error handling

- A stale edit is rejected with the current revision so the caller can rebase.
- Invalid source boundaries are typed errors, never rounded silently.
- Syntax diagnostics do not disable editing.
- Missing font glyphs use the platform fallback path while retaining source
  mapping.
- A layout failure draws an editable plain-text fallback for the affected block.
- IME cancellation restores the last committed snapshot.

## Testing

### Document operations

- Insert, delete, replace, paste, cut, indent, undo, redo, and grouped history.
- Multi-selection edit ordering and overlapping-selection normalization.
- Stale base-revision rejection and selection translation.
- CRLF, tabs, Unicode scalar boundaries, grapheme navigation, and IME.

### Geometry

- Source position to screen point to source position round trips.
- Mixed font sizes, proportional widths, wrapping, bidi-safe affinity, empty
  lines, and end-of-file.
- Selection and caret geometry across styled delimiter boundaries.
- Scroll extent, caret visibility, resize, and viewport virtualization.

### Fork parity

Characterize the Makepad CodeEditor behaviors being retained before adaptation.
Every deliberate divergence is named in a test or module-level provenance note.

## Success criteria

- Raw Markdown is the sole clipboard, history, and edit representation.
- Every edit carries an exact base revision and `TextChange`.
- Mixed-size layout preserves correct caret and selection behavior.
- UI interaction never reparses Markdown independently.
- The crate contains no dependency on Makepad's upstream editor or Markdown
  widget.
- The foundation is testable without mounting the complete WAML application.
