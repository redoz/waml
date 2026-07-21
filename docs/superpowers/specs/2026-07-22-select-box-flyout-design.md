# SelectBox + SelectFlyout — a reusable combo control

**Date:** 2026-07-22
**Status:** design (approved for planning)

## Problem

The inspector's element-picker currently routes through `MenuPopup` (the shared
`PopupRoot` menu surface, `PopupSpec::Menu`, tag `element_picker`). A menu is the
wrong affordance for it:

- The card hugs its widest **label**, ignoring the control's width — it reads as
  a free-floating menu, not a value selector bound to the field below it.
- Rows are a fixed `icon + label` shape (`PopupItem`), so node rows lost their
  per-type colored badge (they now draw a generic `Icon::PackageOpen`) and edge
  rows can only carry one flat glyph.
- The picker field is not drawn "active" while the list is open — no visual link
  between the open list and the control it belongs to.

We want a **combo / select box**: a card that is at least as wide as its control,
sits tight under it, marks the current selection, renders each row's own visual,
and shows the control as active while open. And we want it **reusable** — this is
the standard select control for future call sites, not a one-off.

## Non-goals

- Not a fork of the existing menu behavior for burger/logo/node — `Menu` stays.
- Not arbitrary per-item draw callbacks (the A2 option). Per-item variety comes
  from a closed, extensible `SelectLead` descriptor (A1). Add an arm when a new
  row shape actually appears.
- No secondary/two-line row text yet (inspector rows are single-line). YAGNI.
- No keyboard navigation of the list in this pass (mouse hover + click, matching
  the existing menu). Can follow later.

## Authority constraint (drives the split)

The codebase just unified every popup under one `PopupRoot`: a single active
surface, one light-dismiss seam, cross-tree placement only from the composition
root. A self-contained select widget that owned its own overlay would break that
invariant. So the work splits in two:

- **`SelectBox`** owns the **closed control** (the box you see when nothing is
  open) — reusable, lives in the consumer's own widget tree.
- **`SelectFlyout`** is the **open list**, a third `PopupRoot` surface alongside
  `MenuPopup` and `RadialPopup`.

They are glued by the established emit-request → `App`-relay → tag-filtered-close
pattern (identical to burger/logo/node): the box emits an open request, `App`
relays it to `PopupRoot::show_at`, and the close comes back through the
tag-filtered `PopupRoot::closed` queue.

## Architecture

```
SelectBox  (control, consumer tree)
   │  click → SelectBoxAction::OpenRequested { anchor, min_width, items }
   ▼
App (relay)  → PopupRoot::show_at(PopupSpec::Select { … })
   ▼
PopupRoot  → SelectFlyout (surface)   [ActiveKind::Select]
   │  commit/dismiss → PopupRootAction::Closed { tag, result }
   ▼
App  → SelectBox::on_closed(cx, result)   (clears `open`, applies pick)
```

## Units

### Unit 1 — item model (`popup/select.rs`)

```rust
/// A leading visual for one row. Closed set; extend with a new arm when a new
/// row shape appears (YAGNI over an open-ended draw callback).
pub enum SelectLead {
    None,
    Icon(Icon),                             // edge rows = Icon(Icon::Spline)
    Badge { color: Vec4, letter: String },  // node rows = per-type badge
}

/// One selectable row. `id` is opaque to the surface — the opener resolves it on
/// commit (same contract as `PopupItem.id`).
pub struct SelectItem {
    pub id: LiveId,
    pub lead: SelectLead,
    pub label: String,
    pub selected: bool,   // current value → trailing mark + subtle fill
    pub enabled: bool,    // disabled rows draw dimmed, never arm/commit
}
```

`SelectLead::Icon(Icon::Spline)` restores edge glyphs; `SelectLead::Badge`
restores the per-type colored square + letter node rows lost when they moved onto
`PopupItem`.

### Unit 2 — `SelectFlyout` surface (`popup/select.rs`)

A linear card, same HUD material as `MenuPopup` (Atlas `AccentFrame{field_bg}` +
`IconSet` glyph rows), reusing:

- **`LinearGeom`** for card/row rects and `row_at` hit-testing. `LinearGeom`
  already takes an explicit width via `set_width`; the flyout computes and feeds
  it — no new geometry type.
- **`MarkingCore` in popup mode** (`begin_popup`) for state: open → hover-arm on
  `MouseMove` → commit on `MouseUp` over a row → outside primary press is
  `Ignored` (PopupRoot dismisses). No marking-drag, no `DRAG_THRESHOLD` gesture
  branch, no danger rows, no dividers.

Deltas from `MenuPopup`:

- **Width = combo width, not label-hug.**
  `width = max(label_hug, min_width).min(cap)` where
  `label_hug = LEAD_GUTTER + widest_measured_label + PAD_R`,
  `min_width` = the control's width (passed at open), and
  `cap = max(SELECT_MAX_W, min_width)` so a wide control is never clipped.
  Widest label is measured with makepad's own text engine
  (`prepare_single_line_run`), exactly as `MenuPopup::draw` does.
- **Per-item lead render.** `SelectLead::Icon` → `IconSet::draw` tinted (idle /
  hovered-accent / disabled-dim); `SelectLead::Badge` → colored rounded square +
  centered letter; `None` → no gutter glyph.
- **Selected row** gets a trailing check glyph and a subtle persistent fill,
  distinct from the transient hover wash.

Opens via `open_select(cx, anchor, min_width, items)`. Implements the `Popup`
trait (`handle`/`reset`) with the same shape as `MenuPopup`'s popup-mode branch.

`SELECT_GAP` (≈ 2 lpx, tight, no horizontal indent) replaces `MenuPopup`'s
`MENU_GAP`/`MENU_INDENT_X` for this kind — the card sits flush-left just under
the control.

### Unit 3 — `PopupRoot` wiring (`popup/root.rs`, `app.rs`)

Mirror the existing `Menu`/`Radial` seams exactly:

- `body` DSL gains `select := SelectFlyout{ width: Fill height: Fill }`.
- `enum ActiveKind { Menu, Radial, Select }`.
- `enum PopupSpec { …, Select { tag, anchor: DVec2, min_width: f64, bounds: Rect, items: Vec<SelectItem> } }`. No open-mode field — a combo is always
  click-latched (`MarkingCore::begin_popup`); there is no marking-open variant.
- `show_at` gains a `Select` arm (`Presenter::place` clamps on-screen with
  `size = (max(min_width, cap), rows_height)`), plus `reset` and `route` arms for
  `ActiveKind::Select` calling `SelectFlyout`.

### Unit 4 — `SelectBox` control widget (`select_box.rs`)

Reusable closed control:

- Renders the box: HUD `AccentFrame{field_bg}` + optional leading `SelectLead`
  for the selected row + selected label + trailing caret glyph.
- State: `items: Vec<SelectItem>`, `selected: Option<usize>`, `open: bool`.
- Click anywhere on the box → emits
  `SelectBoxAction::OpenRequested { anchor: <own screen rect>, min_width: <own width>, items }`. It cannot open the surface itself (authority) — `App` relays.
- `open == true` → draws **active**: an accent stroke over the box (source-bright
  Atlas tint), the visual link to the open list.
- API: `set_items(cx, Vec<SelectItem>)`, `set_selected(cx, Option<usize>)`,
  `picked(actions) -> Option<LiveId>`, `open_request(actions) -> Option<(Rect, f64, Vec<SelectItem>)>`, `on_closed(cx, PopupResult)` (clears `open`; on
  `Invoked(id)` updates `selected`).

### Unit 5 — inspector consumer (`inspector_panel.rs`, `app.rs`)

- Drop the hand-drawn picker field: `picker_field_rect`, the field click branch,
  the `OpenPicker`/`open_picker_request` action path, and `picker_items()`'s
  `PopupItem` construction. Keep `picker_ids` (id→index resolution) and
  `apply_pick`.
- Host a `SelectBox` in the picker bar, fed `SelectItem`s built from the diagram
  elements: node rows `SelectLead::Badge{ bucket_color(kind), letter }`, edge
  rows `SelectLead::Icon(Icon::Spline)` with `edge_target` labels, `selected` =
  the current subject's row, `enabled = matches!(kind, Node)`.
- `App` relays the box's `OpenRequested` → `PopupSpec::Select` (tag
  `element_picker`, `min_width` from the request), and on `picker_closed`
  (`Invoked` **or** `Dismissed`) calls `SelectBox::on_closed`, then `apply_pick`
  on an `Invoked(id)`.

## Data flow (open → pick → close)

1. User clicks the inspector's `SelectBox`. Box sets `open = true`, redraws
   active, emits `OpenRequested { anchor, min_width, items }`.
2. `App::handle_actions` reads the request, calls
   `PopupRoot::show_at(PopupSpec::Select{ tag: element_picker, … })`.
3. `PopupRoot` supersedes any open popup, opens `SelectFlyout` in popup mode,
   records `active = (Select, element_picker)`.
4. Pointer events flow through `PopupRoot::route` → `SelectFlyout::handle`
   (hover-arm, commit, outside-dismiss).
5. On commit/dismiss `PopupRoot` emits `Closed { tag: element_picker, result }`.
6. `App` filters it (`pr.closed(actions, element_picker)`), calls
   `SelectBox::on_closed` (clears `open`, updates `selected` on `Invoked`), then
   `inspector.apply_pick` on `Invoked(id)`.

## Error / edge handling

- Empty item list → box still opens a zero-row card that dismisses on any outside
  press (matches menu behavior with no items); the caret still toggles active.
- Superseding: opening any other popup while the flyout is open closes it
  `Dismissed` via the existing `show_at` supersede path — `App` still routes that
  `Closed` into `on_closed`, so the box never sticks "active".
- Off-screen clamp: `Presenter::place` keeps the card in `bounds`; a control near
  the window bottom slides the card up, unchanged from the menu path.
- Disabled rows (edges today) draw dimmed and never arm or commit — same as
  `MenuPopup`'s `enabled` gate.

## Testing

- **`LinearGeom` width clamp** (unit, pure): `width = max(hug, min_width)`; a
  `min_width` wider than the hug drives `panel_rect`/`row_rect`/`row_at` edges
  (extends the existing `set_width_drives_panel_and_hit_edges` test).
- **`SelectBox` state** (unit): click sets `open`; `on_closed(Dismissed)` clears
  `open` without touching `selected`; `on_closed(Invoked(id))` clears `open` and
  updates `selected`; `picked`/`open_request` action readers round-trip.
- **`decide()` reuse**: existing `PopupRoot` decision tests already cover
  commit/dismiss/keep — `Select` routes through the same `decide`.
- **Rendering** (`SelectLead` badges/splines, min-width, selected mark, active
  box) is UI-driven: verify via `/run` on a diagram fixture.

## File touch list

- `crates/waml-editor/src/popup/select.rs` — **new**: `SelectItem`, `SelectLead`,
  `SelectFlyout`.
- `crates/waml-editor/src/select_box.rs` — **new**: `SelectBox`, `SelectBoxAction`.
- `crates/waml-editor/src/popup/mod.rs` — register `select`.
- `crates/waml-editor/src/popup/root.rs` — `ActiveKind::Select`,
  `PopupSpec::Select`, `select :=` child, `show_at`/`route`/`reset` arms.
- `crates/waml-editor/src/app.rs` — relay `OpenRequested` → `Select`, feed
  `on_closed`.
- `crates/waml-editor/src/inspector_panel.rs` — drop hand-drawn field, host
  `SelectBox`, build `SelectItem`s.
- `crates/waml-editor/src/main.rs` / module registration — declare the new mods.
