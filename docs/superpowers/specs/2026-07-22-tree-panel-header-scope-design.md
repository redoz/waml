# Tree Panel Header — Scope, Search, Type-Filter & Chrome

**Date:** 2026-07-22
**Status:** Design approved, pending spec review
**Frontend:** makepad `waml-editor` only (native). Web `NavigatorBody.svelte` covers
the same ground but was **not** used as a source — this is a clean-room design
(honours the no-port rule).

## Problem

The makepad `ProjectTree` panel (`tree_panel.rs`) renders only the tree body:
the Atlas frame + `FileTree` rows. Everything above the rows in the target
mockup is missing:

- A **scope** mechanism — narrow the tree to one package's subtree.
- A **search** field with type-filter.
- Panel **chrome** — collapse, pin, and a hover-driven translucency.

This spec adds that header band and the scope/search/filter logic behind it.

## Scope of this spec

In:

- A pure Rust nav module (`nav.rs`) producing the filtered/scoped view a widget renders.
- Header **title dropdown** — the only way to change scope (KISS).
- **Search** field (with an inline magnifier glyph) + rotating **type-filter** chip.
- Three search empty-states.
- Header **chrome**: collapse (header-only accordion) + pin, plus hover-driven
  translucency shared in behaviour (not code) with the inspector panel.

Out (explicitly, for now):

- Breadcrumb scope path (dropped in favour of the title dropdown).
- Row-based scope-in (double-click / right-click / command wheel).
- A `×` close button.
- Context menu / drag-reorder / CRUD (rename/delete/create) — separate effort.
- A shared `PanelChrome` unit — chrome is per-panel per-widget for now.

## Architecture

New module `crates/waml-editor/src/nav.rs` — the scope/search/filter seam. Pure,
no makepad, unit-testable like the existing `tree.rs`. `tree.rs`'s `build_tree`
stays the full-tree builder; `nav.rs` sits on top of `&Model`.

Data flow:

```
Model ──build_tree──▶ (full tree)
  │
  └── nav::view(&Model, &NavState) ──▶ NavView ──set_view──▶ ProjectTree widget
```

- The **app** owns `NavState { scope, query, filter }`. On any scope/query/filter
  change it rebuilds `NavView` and pushes it to the widget (like today's `set_tree`).
- The **widget** stays a thin renderer of `NavView`. It emits actions
  (`ScopeTo`, `Query`, `RotateFilter`, `Collapse`, `Pin`); the app mutates
  `NavState` and re-pushes. Collapse + hover translucency are panel-local (no app
  round-trip).

## `nav.rs`

```rust
pub struct NavState {
    pub scope: String,            // package key; "" = whole model
    pub query: String,            // search text; "" = browse
    pub filter: Option<TreeKind>, // None = All
}

pub enum NavView {
    Browse(ProjectTree),    // scoped subtree, type-filtered, no query
    Results(ProjectTree),   // query matches inside scope (matches + ancestor pkgs kept)
    Elsewhere(ProjectTree), // no scope match; whole-model matches (shown under a note)
    Empty,                  // nothing matches anywhere
}

pub struct PackageRow { pub key: String, pub title: String, pub depth: usize }

pub fn view(model: &Model, state: &NavState) -> NavView;
pub fn packages(model: &Model) -> Vec<PackageRow>;   // package-only tree + synthetic root
pub fn kinds_in_model(model: &Model) -> Vec<TreeKind>; // distinct, stable order
```

Behaviour:

- **Scope** — root the display at `scope`'s subtree (its members at depth 0; the
  scope package itself is not shown as a node). `scope == ""` roots at the whole
  model. Inline folder expansion is preserved within the scoped subtree.
- **Type filter** — `Some(kind)` keeps only rows of that kind; ancestor packages
  of a kept row are retained for structure. `None` = All.
- **Query** — case-insensitive substring on title. Non-matching leaves pruned; a
  package is kept if any descendant matches.
- **Three states** — matches within scope → `Results`. None in scope but some in
  the whole model → `Elsewhere`. None anywhere → `Empty`. (An empty query yields
  `Browse`, never a search state.)
- **`packages()`** — nested package-only rows (depth-indented) for the title
  dropdown, prepended with a synthetic root row (label = `model.path` or
  `"Untitled"`, key `""`) = whole-model scope.
- **`kinds_in_model()`** — the distinct `TreeKind`s present anywhere in the model,
  in a stable order; drives the type-filter chip's cycle. Computed on Model load,
  not per keystroke.

Reuses `TreeKind` and the `ProjectTree`/`TreeNode` types from `tree.rs`.

New unit tests in `nav.rs` (clean-room, written against the behaviour above, not
the TS tests):

- scope roots at the package's subtree; `""` roots at whole model;
- type-filter keeps matching kinds + ancestor packages, prunes the rest;
- query prunes non-matching leaves, keeps matching branches;
- the three states fire on the right inputs (matches / elsewhere / empty);
- `packages()` nests and carries the synthetic root;
- `kinds_in_model()` is distinct and stably ordered.

## Header title dropdown (scope picker)

The title row shows the current scope label (scope package title, or `"Untitled"`
at root) followed by a `⌄` glyph. The whole title is the click target (mockup:
the big "Untitled" is the button).

- Click → a **PopupRoot** popup anchored under the title (reuses the landed
  single-active + light-dismiss seam).
- Popup body = a package-only nested tree from `nav::packages`, depth-indented,
  each row carrying the package glyph. The top row is the synthetic root
  (`"Untitled"` / model path) = whole-model scope.
- Selecting a row emits `ScopeTo(key)`; the app sets `NavState.scope`, closes the
  popup, and re-pushes `NavView`. The current scope's row is marked (accent /
  check, matching the active-tab row treatment).
- Reuses the picker-popup pattern (hand-drawn field + owned immediate-mode list)
  so package glyphs render as real SDF icons.

## Search row + type chip + search icon

One row under the title: search field (`Fill`) + type chip (`Fit`).

- **Search field** — makepad `TextInput`, placeholder `"Search model"`. A
  magnifier glyph is drawn inside at the left (immediate-mode overlay, like the
  tree row glyphs, or a leading-icon slot). Editing emits `Query(String)`; the
  app updates `NavState.query` and re-pushes.
- **Type chip** — a rotating button cycling `[All, <kinds_in_model()...>]`. Label
  de-prefixes (`uml.Class` → `Class`); trailing `⌄` glyph is decoration. Click
  emits `RotateFilter`; the app advances `NavState.filter`. No popup (KISS).
- **Empty states** render in the tree area:
  - `Elsewhere` → a dim `"No matches in <scope>"` line, then an
    `"Elsewhere in model"` header, then the whole-model matches.
  - `Empty` → centered `"No matches found"`.

**Glyphs** — verify the SDF catalog (`icons.rs`) before adding: a magnifier
(`Search`) for the field, and up/down chevrons for the collapse toggle. `Pin` /
`PinOff` already exist. Any new glyph is add-only and must respect the
enum==field==DSL==get==ALL==label ordering invariant + count bumps.

## Header chrome + hover translucency

Header right cluster: **collapse `^`** + **pin**. No `×`.

- **Collapse** — toggle; collapsed = header row only, with the search row + tree
  body hidden. The glyph flips `^`/`v`. Panel-local state.
- **Pin** — toggle; locks the panel opacity to `1.0`. Uses the `Pin` / `PinOff`
  glyph swap (per the landed inspector pin).
- **Hover translucency** — panel opacity is `1.0` when the pointer is over the
  panel **or** the panel is pinned, else `0.55`. Hover-driven (enter/leave), not
  focus-driven.

The **inspector panel** grows the same hover-translucency (opacity 0.55 unhovered,
1.0 when hovered or pinned) alongside its existing pin — implemented per-widget,
not via a shared unit.

## Testing

- `nav.rs` unit tests as listed above (pure, no `Cx`).
- Widget: app owns `NavState`, rebuilds `NavView` on every scope/query/filter
  change and re-pushes; widget emits `ScopeTo`/`Query`/`RotateFilter`/`Collapse`/
  `Pin`. Collapse + hover translucency are panel-local.
- Manual: title dropdown scopes; chip cycles the model's kinds; search shows the
  three states; collapse hides the body; pin locks opacity; an unhovered panel
  dims to 0.55 (both tree and inspector).
