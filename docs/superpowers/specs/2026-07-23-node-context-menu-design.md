# Node context menu — uniform per-subject menu + context additions

**Date:** 2026-07-23
**Status:** design (approved for planning)

## Problem

Right-clicking a subject should open a **uniform menu** — the same base
actions for every node regardless of where it was invoked — with **context
additions** contributed by the surface it was invoked from. Two base actions to
start:

- **View Source** — open a tab rendering the subject's markdown (each element is
  its own markdown file).
- **Find in diagrams** — list every diagram containing the subject.

Right-clicking a node **in the model view** yields the base menu only.
Right-clicking a node **in a diagram** yields the diagram's per-node context
items **plus** the base menu.

## Current state (what this pivots)

A node context menu already exists, but as a **radial** wheel:

- `crates/waml-editor/src/canvas.rs` — secondary-button `FingerDown` over a node
  emits `GraphCanvasAction::NodeMenu { abs, node }`.
- `crates/waml-editor/src/app.rs:1420` — App relays it to
  `PopupRoot::show_at(PopupSpec::Radial { tag: node_menu, items: node_radial_items(), .. })`.
- `node_radial_items()` (`app.rs:842`) + `canvas::NodeCommand { Open, Style,
  Markdown, Remove }` + `node_command_for` (`canvas.rs:428`) — all four commands
  are **`log!`-only stubs**; they perform no real action.
- No context menu exists in the **model view** (project tree) at all.

The user's ask is a linear `MenuPopup`, not a wheel, on the same right-click
gesture. This spec **pivots the node menu from radial to linear** and adds the
second entry point (the model tree). Because the four radial commands are pure
logging stubs, replacing them loses no real behavior.

## Non-goals / deferred

- **No separator affordance yet.** `PopupItem` has no divider field, and adding
  one touches every `PopupItem` struct literal (logo/burger/select). The context
  list is **empty** in this pass, so a base-only menu never needs a divider.
  The separator lands with the first real context items (YAGNI).
- **No real source text.** View Source opens an **empty** markdown view for now;
  wiring `Subject` → the element's markdown file is deferred.
- **Find in diagrams is a stub** — it `log!`s the subject; no results UI.
- **No diagram-context items populated.** The canvas contributes an **empty**
  context list. The seam is proven (compose merges context above base); real
  per-node-type items land later.
- The `RadialPopup` surface is **kept** (shared infra, still reachable via
  `PopupSpec::Radial`); only the node menu stops using it.
- Right-click targets a **node** subject (`Subject::Classifier(key)`) only.
  Edges / diagrams / packages are out of scope.

## Architecture

Reuses the established popup pattern exactly (mirrors burger/logo): a surface
emits a request → `App` relays to `PopupRoot::show_at` → the close comes back
through the tag-filtered `PopupRoot::closed` queue, and an id→command mapper
dispatches.

```
canvas / project-tree  (secondary-button press over a node/row)
   │  emit request { subject, anchor }   + set inspector subject (select-on-right-click)
   ▼
App (relay)
   │  items = node_menu::compose(context, node_menu::base_items())
   │           context = canvas.context_items(&subject)  (diagram)  |  []  (model view)
   ▼
PopupRoot::show_at(PopupSpec::Menu { tag: node_menu, anchor, bounds, items, open: Popup })
   ▼
MenuPopup  → close → PopupRoot::closed(actions, node_menu) → Invoked(id)
   ▼
App: node_menu::command_for(id) → { ViewSource → open source tab | FindInDiagrams → log }
```

### Menu composition — `crates/waml-editor/src/popup/node_menu.rs` (new)

```rust
/// Base (per-subject) node commands. Uniform across every invocation site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeMenuCommand { ViewSource, FindInDiagrams }

/// The base items every node menu ends with. Ids are what `MenuPopup` reports
/// on commit; `command_for` maps them back (mirrors `logo_command_for`).
pub fn base_items() -> Vec<PopupItem>;              // View Source, Find in diagrams

pub fn command_for(id: LiveId) -> Option<NodeMenuCommand>;

/// Context items first, base items last (base is the stable bottom zone).
/// With an empty `context`, returns `base` unchanged.
pub fn compose(context: Vec<PopupItem>, base: Vec<PopupItem>) -> Vec<PopupItem>;
```

`compose` = `context` then `base` (concatenation). Base actions always occupy
the bottom of the list regardless of invocation site — a stable target zone.

### Entry point 1 — diagram (canvas)

- Keep the existing secondary-`FingerDown` handler; change the action payload to
  carry the subject key so App need not re-map the node index:
  `GraphCanvasAction::NodeMenu { abs, key }`.
- Add `GraphCanvas::context_items(&self, subject: &Subject) -> Vec<PopupItem>` —
  returns **empty** now (the diagram-context seam).
- **Select-on-right-click:** App points the inspector at
  `Subject::Classifier(key)` when handling the request (same call
  `NodeSelect` already makes), so the inspector follows the right-clicked node.

### Entry point 2 — model view (project tree)

- `crates/waml-editor/src/tree_panel.rs`: on a secondary-button press over a
  classifier row, emit `ProjectTreeAction::ContextMenu { key, anchor }` with a
  reader `context_menu_request(&self, actions) -> Option<(String, DVec2)>`
  (mirrors `filter_request`/`scope_request`'s anchor-return readers).
- Model-view menus pass an **empty** context list → base-only.
- Select-on-right-click: reuse the existing `FocusClassifier` selection path.

### App relay + dispatch — `crates/waml-editor/src/app.rs`

- Replace the `NodeMenu` arm's `PopupSpec::Radial` with
  `PopupSpec::Menu { tag: live_id!(node_menu), anchor: abs, bounds, items:
  compose(context, base_items()), open: MenuOpen::Popup }`, and set the
  inspector subject before opening.
- Handle the tree's `context_menu_request` identically (empty context).
- In `handle_actions`, the existing `node_closed` branch swaps
  `canvas::node_command_for` → `node_menu::command_for`:
  - `ViewSource` → open a source tab for the subject (below).
  - `FindInDiagrams` → `log!("find in diagrams: {key}")`.
- **Remove** the now-unused `node_radial_items()`, `canvas::NodeCommand`, and
  `canvas::node_command_for` (the clippy `-D warnings` gate promotes their
  `dead_code` to a hard error). Keep `RadialPopup` (still referenced by
  `PopupSpec::Radial`).

### View Source tab — `crates/waml-editor/src/doc_tabs.rs` + `app.rs`

- Add `TabKind::Source` and `OpenTabs::open_source(key, title)` mirroring
  `open_preview` (single preview slot reuse, id derived from key, never
  duplicates). Title = element title.
- Tab bodies today all render into the shared `canvas` widget via
  `sync_active_tab`'s `match active.kind`. Add a sibling `source_view` in the DSL
  hosting a makepad `Markdown` widget with an **empty** document. In the new
  `TabKind::Source` match arm: show `source_view`, hide `canvas`, hide the tool
  dock + element picker.
- **Fallback:** if the fork's `script_mod!` widget set cannot host makepad's
  `Markdown` without extra registration, ship an empty placeholder `View` in its
  slot and note the follow-up — "empty markdown view for now" is satisfied
  either way. Prefer the real `Markdown` widget.

## Data model

`Subject` (`inspector.rs`) stays `None | Classifier(String)`; the menu targets
`Classifier(key)`. No new subject variants.

## Testing

Pure-function unit tests (no GPU), matching the `node_command_maps_the_four_committed_ids`
and `logo_command_for_maps_ids_and_rejects_others` style:

- `compose` puts context first, base last; empty context returns base unchanged.
- `command_for` maps `view_source` → `ViewSource`, `find_in_diagrams` →
  `FindInDiagrams`, and unknown → `None`.
- `base_items` yields exactly the two base entries in order.
- `OpenTabs::open_source` reuses the preview slot, dedupes by key, activates —
  mirrors the existing `open_preview` tab tests.

Visual/interaction (right-click opens the linear card at the cursor, select
follows, source tab shows an empty markdown pane) is verified by running the
editor — the repo-standard self-screenshot recipe — not asserted in units.
