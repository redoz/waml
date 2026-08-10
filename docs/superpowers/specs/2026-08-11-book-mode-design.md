# Book mode: a folder read as one continuous, laid-out document

Status: design approved 2026-08-11 (brainstorming session).
Implementable scope of THIS spec: **Phase 1 — the book shell (read-only)**. Phases 2–4 are
sketched at the end so Phase 1's seams are cut for them; each gets its own spec.

## The idea

A folder is a book. Opening it gives one continuous scroll instead of a list of links you
click one at a time. Every node in that scroll renders in the layout its author declared:
prose renders typeset, an outline renders as bullets, a container renders as a kanban
board, a diagram renders inline. The tree panel stops being "the place you open files
from" and becomes the book's table of contents: clicking a node scrolls the book to that
section, and scrolling the book marks the node you are in.

That turns waml from a documentation tool into a workspace where structure, prose, and
diagrams live in one readable surface — the Workflowy move, applied to a bundle on disk.

## Decisions locked in brainstorming

1. **Folder = book.** No new document format. Sections come from the folder's projected
   rows, in the order the projection already produces. If this proves too coarse, headings
   inside a file become sections later (a follow-up spec, not a v1 concession).
2. **Layout is declared in the source, per node kind.** A folder declares it in its
   `index.md` frontmatter `view:` chain; a concept declares it in its own frontmatter.
   Layout travels through git and is the same for everyone who opens the book.
3. **Editing is block-granular** (Phase 2): click a block, that block becomes an inline
   source editor over its own source range, commit reparses and re-renders. Structural
   changes (reorder, indent, move between columns) stay direct-manipulation and lower to
   `RowOp`. No full-document WYSIWYG cursor.
4. **The tree stays one tree.** When the active view is a book, a tree click scrolls
   instead of opening a tab, and book scroll marks the tree. No second TOC panel, no modal
   swap.
5. **Book mode is opt-in.** A folder that declares it opens as a book; everything else
   opens exactly as it does today.
6. **Diagram embeds render live, capped, read-only.** "Open full" opens that concept's own
   tab through the existing navigation path. No lightbox in v1.
7. **A board is a lens on any container row** — children are columns, grandchildren are
   cards. Folder-as-board moves files; concept-group-as-board moves concepts. One rule at
   every level (Phase 4).

## Why this is mostly composition, not new machinery

The pieces already exist and were built with this seam in mind:

| Need | What already exists |
| --- | --- |
| Ordered sections with lazy children | [`waml::view::row::Row`](crates/waml/src/view/row.rs) — `expand: Option<Chain>`, `caps`, `child_caps`, `surface: Option<SurfaceId>` |
| Per-node layout declared in source | the frontmatter `view:` chain ([`decl.rs`](crates/waml/src/view/decl.rs)), whose entries already include surface-resolution entries (`markdown`, `member:<href>`), resolved by `Projection::surface` |
| Layout registry with safe degradation | [`resolve_surface`](crates/waml/src/view/surface.rs) + [`KNOWN_SURFACES` / `resolve_surface_for`](crates/waml-editor/src/documents.rs) — an unknown id degrades to the default with an `UnknownSurface` warning, never a blank tab |
| Structural edit ops | [`RowOp`](crates/waml/src/view/projection.rs) (`Rename`, `Delete`, `Reorder`, `InsertConcept`, `MoveIn`, `MoveOut`) lowering to `okf::Op`, with per-row `caps`/`child_caps` refusals |
| Outline gestures | `enter_row_op` / `tab_row_op` / `shift_tab_row_op` / `reorder_row_op` / `rename_row_op` in [`folder_view.rs`](crates/waml-editor/src/folder_view.rs) |
| Block model with source ranges | [`ReadingBlock`](crates/waml-markdown-editor/src/reading/model.rs) — every block carries `source_range`, and the partition over the source is validated |
| Rendered-to-source mapping | `SourceMap` / `caret_for_span` on [`MarkdownViewer`](crates/waml-markdown-editor/src/reading/widget.rs) |
| A diagram that fits itself into an arbitrary rect, non-interactive | `ClassDiagramSurface` — fits the scene to the view on first draw; `set_interaction_enabled(false)` |
| Surface mutual exclusion + teardown | the `show_*` family and `hide_document_surfaces` on [`BodyWidgets`](crates/waml-editor/src/doc_view.rs) |
| Tree scroll-to + pulse | `reveal_key` / `pending_scroll_key` in [`tree_panel.rs`](crates/waml-editor/src/tree_panel.rs) |

So the book is **a surface that composes other surfaces inline, driven by the row
projection**. Phase 1 adds one surface, one view, one reveal variant, and a virtualizing
scroll — not a new document engine.

## Phase 1 architecture — the book shell

### The `book` surface

`SurfaceId::book()` joins the registered set: `KNOWN_SURFACES`, the core editor half's
`surfaces` list (whose parity is already pinned by
`todays_four_surfaces_are_registered_by_the_core_editor_half` — that test's name and
expected set move to five), a factory arm in the surface table, and an arm in
`locator_opens` (a book locator opens iff its directory is in the bundle — the same probe
`folder` uses).

A folder opts in by declaring it in its `index.md`:

```yaml
---
view: book
---
```

`book` is a surface-resolution entry in the `view:` chain, the same category `markdown`
already occupies — not a middleware stage. A chain may still carry middleware before it
(`view: [hide-refs, book]`): the projection is unchanged, only the surface it resolves to
differs. `Projection::surface` returns `SurfaceId::book()` for that folder; every other
folder keeps returning `folder_surface()`.

The default is unchanged: a folder with no declaration opens as today's folder listing.

### The section model

`BookSection` is built from the folder's projected rows, walked depth-first, following
each row's `expand` chain, bounded by the existing `ChainLimits::max_depth` (project
config, already read from `.waml/project.json`):

```rust
struct BookSection {
    row_id: RowId,
    depth: u8,
    title: String,
    body: SectionBody,
}

enum SectionBody {
    /// A folder row: a section heading, with its children following as sections.
    Heading,
    /// A markdown concept: the reading document compiled from its source.
    Prose { document: Arc<ReadingDocument>, source: Arc<str> },
    /// A diagram concept: the flattened scene, drawn read-only at a capped height.
    Diagram { scene: Scene, concept_id: String },
    /// A row whose declared surface this build does not render inline.
    Link { reason: LinkReason },
}
```

Each row's body is chosen by the editor's one resolution, `resolve_surface_for`, passing
`row.surface` as the requested id — never by re-deriving a surface locally, so a book
section and the tab that row would open can never disagree about what that row is. A row
carries an explicit `surface` only where a projection stage sets one; today none do, so in
Phase 1 every section resolves to the claims-based default (`default_surface_for`). That is
the seam a per-concept layout declaration arrives through in Phase 3, and it needs no book
change when it does. `markdown` becomes `Prose`, `canvas` becomes `Diagram`,
`folder` becomes `Heading`, and anything else (including `source`, `search:*`, and a
nested `book`) becomes `Link`: a one-line row with the row's icon and title that opens
that row's own tab. A nested book is deliberately a link, not an inlined book — recursive
inlining has no natural bottom and would make a bundle's root unbounded.

`Prose` reuses the exact compile path `ReadingView::install_snapshot` uses today
(`compile_presentation` → `build_reading_document`, with `WamlCodeHighlightHost`). A
section whose compile fails renders as a `Link` with the failure as its reason, and logs —
the same posture the reading view already takes, never a silently blank section.

The section model is pure data over an `OkfAnalysis` + `Analysis`, so it is unit-testable
from a `SourceBundle` fixture with no GUI.

### `BookView`

`BookView` implements `DocView` with `DocViewIdentity::Book`, drawing on a new
`book_surface` sibling in the DSL. `show_book_view` joins the `show_*` family (mine on,
siblings off) and `book_surface` joins `hide_document_surfaces`.

It owns one scroll offset for the whole book. Sections draw in order, each as:

- **Heading** — the title at a size derived from `depth`, plus a rule.
- **Prose** — a `MarkdownViewer` at `Height::Fit`, non-interactive in Phase 1.
- **Diagram** — a caption strip (title + "open full" `IconButton`) above a
  `ClassDiagramSurface` given a fixed-height walk with `set_interaction_enabled(false)`.
  The surface fits the scene to that rect on first draw. "Open full" issues the existing
  `NavigationIntent` for that concept, so it lands in a tab exactly as a tree click would.
- **Link** — one row, icon and title, click opens that row's tab.

**Virtualization is required, not an optimization.** A live `MarkdownViewer` or
`ClassDiagramSurface` per section will not survive a two-hundred-file book. Only sections
intersecting the viewport (plus one screen of margin either side) hold a live child
widget; every other section reserves its last measured height, and an unmeasured section
gets a per-kind estimate until it is first drawn. Section heights are cached by `RowId`
and invalidated when that row's revision changes.

### The tree as a table of contents

After each draw, `BookView` publishes an anchor table — `Vec<(RowId, f64)>`, each section's
top in book-scroll coordinates. Two directions ride on it:

- **Tree → book.** When the active view's identity is `Book` and the clicked tree row is a
  section of that book, the click resolves to a reveal instead of an open.
  `RevealTarget` gains a `Row { id: RowId }` variant; `DocView::reveal` already has a
  no-op default, so no other view changes. A tree row that is *not* in the active book
  (another part of the workspace) opens a tab exactly as it does today — the tree does not
  become book-only.
- **Book → tree.** On scroll, the nearest anchor at or above the fold is the current
  section; its `RowId` is handed to the tree's existing `reveal_key` path, which already
  owns the scroll-into-view and the highlight pulse.

Both directions key on `RowId`, which the projection guarantees is stable across a rebuild
(the mint/resolve round-trip invariant in `root.rs` is exactly this promise), so a book
that reloads does not lose its place.

### Chrome

`BodyChrome` for a book: breadcrumb on, tool dock off, view bar off. The header's
`view_toggle` offers the folder listing as the destination action, so a reader can always
drop from the book back to the plain list of the same directory — the same shape the
source toggle already has for markdown.

### Testing

- **Unit (no GUI):** section model over `SourceBundle` fixtures — order matches the
  projection; depth cap honored; each surface maps to the expected `SectionBody`; an
  unknown declared surface degrades to `Link` and emits the `UnknownSurface` warning; a
  concept whose presentation fails to compile becomes a `Link`, not a blank.
- **Typed UI scenarios** (`waml-ui-test`, the existing harness): opening a `view: book`
  folder shows `book_surface` and hides its siblings; a tree click on a section scrolls the
  book and opens no tab; a tree click outside the book still opens a tab; scrolling the
  book marks the tree; "open full" on a diagram embed opens that concept's tab.
- **Virtualization:** a fixture book with more sections than fit on screen instantiates
  live children only for the visible window, asserted through the section-state accessor
  rather than by pixel inspection.
- **Visual sign-off is owed to a human and is NOT part of the gate.** The implementer
  cannot verify appearance. The plan must land the behavior with automated coverage and
  leave an explicit visual checklist: prose typography inside the book matches the
  standalone reading view, diagram embeds are legible at their capped height, heading
  hierarchy reads as a hierarchy, and scrolling a long book stays smooth.

### Risks

- **Perf of composed live widgets.** Mitigated by virtualization above; it is the first
  thing to measure, not the last.
- **Makepad layout traps in a composed scroll.** Two are already known and must be
  respected: a `draw_walk` rect goes stale after a `Size::Fill` sibling, and a lone fixed
  child inside a fixed parent can blank its content. Sections are fixed-height or `Fit`,
  never `Fill`.
- **Diagram legibility at a capped height.** A complex diagram scaled into 400px is a
  thumbnail whether or not it is live. Accepted for v1: the caption strip and "open full"
  are the escape hatch, and the cap is a single constant to tune during visual sign-off.
- **A book whose folder is huge.** The depth cap bounds recursion, but not breadth. Phase
  1 renders what the projection produces; if a real bundle makes this unusable, breadth
  paging is a Phase 1.5 follow-up, not a blocker.

## Later phases (each its own spec)

**Phase 2 — edit ops and block-granular editing.** Click a block in a book's prose
section; that block swaps for an inline source editor scoped to `ReadingBlock::source_range`
(`SourceMap`/`caret_for_span` already map the two directions). Commit on blur or Enter
reparses and re-renders that section only. Structural ops go through `RowOp`, which already
lowers to `okf::Op` and already refuses on `caps`. This phase makes the book writable
without any lens knowing how to write.

**Phase 3 — the bullets lens.** `view: bullets` renders a container's rows as an outline
inside the book. Enter, Tab, Shift-Tab, and drag map onto `enter_row_op`, `tab_row_op`,
`shift_tab_row_op`, and `reorder_row_op` — which exist today in `folder_view.rs` and get
lifted into a shared module both the folder view and the bullets lens call.

**Phase 4 — the board lens.** `view: board` on any container row: children are columns,
grandchildren are cards. A card drag is `RowOp::MoveIn { from }` against the target
column's row, refused when that column's `child_caps.accept_move_in` is false — the refusal
path is already modeled, so an un-droppable column is a rendering question, not new logic.
Folder-as-board moves files; group-as-board moves concepts; the lens does not know which.
