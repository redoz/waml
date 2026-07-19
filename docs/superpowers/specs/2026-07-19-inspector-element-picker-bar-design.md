# Inspector element-picker bar

**Date:** 2026-07-19
**Branch:** ui-frame
**Component:** `crates/waml-editor` — `inspector_panel.rs`, `inspector.rs`, `app.rs`

## Goal

Add a control bar to the top of the right-side Inspector: a makepad `DropDown`
listing the current diagram's contents, plus a square "pin" toggle to its right.
Lets the user pick an element to inspect without clicking the canvas. Matches the
supplied mock (white field, chevron, blue pin box).

## Scope

- **List** every element of the current diagram: diagram title, each node, each
  edge. **Inspect** only node rows — diagram and edge rows are listed but selecting
  them is a no-op for now (`Subject::Diagram` / `Subject::Edge` views are a later
  task). Rows all render normally (not visually disabled).
- Pin is **visual-only** this cut: wire a local `pinned: bool`, render
  active(blue)/inactive per mock. Its real purpose — keep the panel opaque when it
  loses focus — is deferred; the panel-goes-translucent-on-blur behavior does not
  exist yet, so no opacity effect here.

## List contents & order

Two-level feel via source-anchored edges:

```
<Diagram title>
<Node A>
  <Node A -> target>     (edges whose source is Node A, listed right after it)
<Node B>
  <Node B -> target>
...
```

- Diagram title row first.
- Then each node, and immediately after each node the edges it is the **source**
  of (source end), giving a shallow 2-level hierarchy in the flat dropdown.
- Placeholder text `"Select an element…"` when nothing is selected
  (`Subject::None`).
- Edge row label format: `"<source> -> <target>"` (e.g. `"Order -> Customer"`).

## Interaction

- Picking a **node** row is inspector-local: `set_subject(Classifier(key))`. Does
  NOT open a tab, does NOT move or highlight the canvas.
- Picking a **diagram** or **edge** row: no-op.
- Two-way sync: when the subject changes from elsewhere (canvas click, classifier
  tab), the dropdown's selected row updates to match; `Subject::None` resets the
  dropdown to the placeholder.
- Pin toggles `pinned` and repaints its blue/inactive look.

## Data flow

The Inspector holds no `Model`, so it cannot resolve a pick itself:

- Pick emits `InspectorAction::ElementPicked(key)`.
- `App` handles it by calling `inspector.set_subject(cx, &model,
  Subject::Classifier(key))` — the same path a canvas/tab selection takes.
- `App` feeds the option list via a new
  `Inspector::set_diagram_elements(cx, &[(key, label, kind)])`, called whenever the
  current diagram changes: startup, `switch_diagram`, and the Diagram branch of
  `sync_active_tab`.
- The `kind` tag distinguishes diagram / node / edge rows so the picker knows which
  rows are inspectable (node) vs no-op (diagram, edge).

## Layout

- Bar sits at the **top** of `draw_walk`, ~44px tall. Existing inspector content
  shifts down by the bar height.
- Bar = `DropDown` (fills, minus the pin) + square pin toggle at the right.
- Themed to Atlas (white field, chevron; blue active pin). Sharp corners — sdf
  radius must be > 0 on this fork.

## Implementation notes

**Container flip (decided):** the Inspector today is a manual immediate-mode
widget with no child widgets. Rather than bolt a sub-View on and entrench a
throwaway shell, flip the whole widget to a `View`-deref container — the correct
home for the form of real controls the inspector will grow into. This is a
*container* refactor, not a body rewrite:

- `#[deref] view: View`. The HudFrame material is inlined onto `View`'s `DrawQuad`
  (a `DrawColor` field can't swap onto it) — copy the exact shader from
  `tree_panel.rs` / `draw_hud.rs`.
- The **bar** (dropdown + pin) is the first real DSL child. `element_bar := View`
  holds `element_picker := DropDown{…}`; the pin is drawn manually in the bar's
  reserved right-gap (no `pin.svg` exists).
- The **body stays manual `draw_abs`** for now, drawn inside `draw_walk` below the
  bar. Precedent: `ProjectTree` derefs `View` yet still does manual immediate
  draws in its `draw_walk`. Hit-test the body against `self.view.area()` (not the
  old `draw_bg.area()`); key focus likewise moves to `self.view.area()`.
- Future field controls replace manual body pieces one at a time — each is just
  another DSL child. No re-architecting when they land.

**DropDown theming:** the fork's `DropDown` already draws its own chevron and has
a `border_radius` (keep it > 0 — a 0 radius floods on this fork). Theme the
visible field to Atlas (white `field_bg`, `frame_hi/lo` border, `text` arrow +
label, IBM Plex). The popup is a separate `PopupMenuFlat`; leave its styling at
the fork default for this cut (Atlas popup theming is a follow-up). Runtime API:
`set_labels`, `set_selected_item` (no action), `changed(actions) -> Option<usize>`
— all on `DropDownRef`, reached via `self.view.drop_down(cx, ids!(element_picker))`.
The fork uses a `script_mod!` DSL, not stock `live_design!`.

## Tests

Pure-logic unit tests (no `Cx`):
- diagram -> ordered options list (diagram, then node + its source-edges, ...).
- subject <-> selected-index sync (Classifier(key) resolves to the right row;
  `None` -> placeholder index).

`cargo test -p waml-editor` green, `cargo build` clean.

## Deferred / out of scope

- `Subject::Diagram` / `Subject::Edge` inspector views.
- Pin's opacity-on-blur (panel translucency when it loses focus).
- Driving canvas selection or opening tabs from a dropdown pick.
