# View Source tab: rendered markdown — design

**Date:** 2026-07-24
**Status:** approved, ready for plan

## Problem

The editor has multiple document views. A "View Source" tab already exists and
is fully wired: both the ProjectTree right-click menu and the diagram node
context menu fire `NodeMenuCommand::ViewSource`, which calls
`OpenTabs::open_source(key, title)` and mints a distinct **Source** tab. Today
that tab's body is an opaque, empty placeholder (`source_view := SolidView`),
because real markdown rendering was a deferred follow-up.

Goal: render the subject's markdown in that Source tab so the user can read the
markdown representation of a classifier.

## Non-goals

- No new triggers. Both context-menu paths (tree item + diagram node) already
  open the Source tab; nothing about the menu, `open_source`, or tab lifecycle
  changes.
- No editing. Read-only render of on-disk source.
- No support for subjects absent from `Model.nodes` (packages, diagrams) — the
  existing `ViewSource` handler already only opens for classifier nodes
  (`app.rs:1118`, guarded by `self.model.nodes.iter().find(...)`). Unchanged.

## Approach

The `App` owns the raw bundle (`self.bundle: Vec<(rel_path, contents)>`, the
verbatim on-disk `.md` files) and already special-cases the Source tab in
`sync_active_tab`. Makepad's upstream `Markdown` widget
(`widgets/src/markdown.rs`, DSL `Markdown`, `MarkdownRef::set_text`) renders
headings / lists / links / code / tables with its own wrapping text-flow. So:
App looks up the source text and pushes it into a `Markdown` child of the slot.
No `DocView` trait change — `SourceView` keeps only its chrome-hiding role.

### 1. Slot widget (`app.rs` live_design, replaces `source_view := SolidView` at ~`app.rs:208`)

Opaque, vertically-scrolling `View` wrapping a `Markdown`:

```
source_view := View {
    width: Fill, height: Fill, visible: false
    show_bg: true
    draw_bg.color: atlas.canvas_ground        // stays opaque -> occludes canvas
    flow: Down
    scroll_bars: ScrollBars { scroll_bar_y: ScrollBar { /* Atlas-visible, mirror inspector_panel.rs:122 */ } }
    md = Markdown { width: Fill, height: Fit }
}
```

The bg color and `width/height: Fill` preserve the existing occlusion contract
(the Source tab hides the canvas by drawing over it). `Markdown` is `height:
Fit`; the wrapping `View` scrolls it vertically when the document overflows.

### 2. Feed the text (`sync_active_tab`, at `app.rs:409` where the slot visibility is already toggled)

Where the slot is already toggled visible for a Source tab, additionally: when
`active.kind == TabKind::Source`, resolve the source string for `active.key`
and call the `Markdown` ref's `set_text(cx, &source)`. Reached via
`self.ui.widget(cx, ids!(source_view, md)).as_markdown()`.

### 3. key -> markdown source

A small helper: return the contents of the `self.bundle` entry whose path
basename (minus `.md`) equals `key`. `key` is the classifier slug (`order`);
the bundle path may be nested (`shop/order.md`) — basename match handles both.

- **Source = raw bundle file text**, verbatim (not `serialize_document`
  re-render). It is the true source: preserves hand-authored / Unknown sections
  and exact formatting, and needs no reparse.
- **Missing / no match** → feed a short italic note
  `` *No source for `<key>`* `` rather than leaving the previous tab's text or a
  blank surface.

## Data flow

```
right-click subject (tree row OR diagram node)
  -> NodeMenuCommand::ViewSource            (already wired, unchanged)
  -> OpenTabs::open_source(key, title)      (already wired, unchanged)
  -> new Source tab focused
  -> sync_active_tab:
       source_view.set_visible(true)        (existing)
       source_for(key) -> Markdown.set_text (NEW)
```

## Testing

- Unit: key→source helper — basename match, nested path, missing key returns
  `None` (so the caller renders the italic note).
- Existing `doc_view` / `doc_tabs` tests unaffected (no trait/tab changes).
- Manual per-pid visual verify: right-click a classifier in the tree and on the
  canvas; confirm the Source tab shows rendered markdown (headings/lists/links),
  scrolls when long, and still occludes the canvas.

## Risk / notes

- Fork parity: upstream makepad `Markdown` is available (fork kept current with
  upstream). Confirm the DSL block name and `as_markdown()` accessor resolve in
  the widget registry before wiring — `Markdown` must be reachable from the
  app's `live_design!`.
- `set_text` runs every `sync_active_tab` for a Source tab; cheap, but only the
  content changes, so re-feeding identical text is acceptable (no diff needed).
