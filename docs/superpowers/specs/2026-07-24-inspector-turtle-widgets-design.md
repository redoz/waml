# Inspector body → reusable Turtle child widgets

**Date:** 2026-07-24
**Branch:** `inspector-typescale` (off `origin/main`)
**Scope:** diagram/picker inspector view ONLY (native, Makepad). Web frontend unchanged.

## Problem

The inspector panel's body (attributes, relationships, description) is drawn
immediate-mode in `Inspector::draw_walk` (`inspector_panel.rs:530-670`):
hand-computed `x`/`y`, fixed pixel line advances (`ROW_H` / `HEAD_H` / `CARD_H` /
`CARD_LINE_H`), zero text measuring. Consequences:

- Rows can't self-size. A long attribute name or relationship label clips or
  collides with the next line — the advance is a constant, not the drawn extent.
- Spacing is imperative arithmetic (`y += …`), not declarative layout.
- Nothing is reusable. The same card/row/heading shapes will be wanted for node
  cards, drag overlays, and the node editor, but they're welded into one
  `draw_walk`.

## Goal

Replace the immediate-mode body with real Turtle-laid-out **reusable child
widgets** so rows self-size, spacing is declarative, and the card/row/heading
widgets can be reused elsewhere. The `inspector.rs` projection
(`InspectorView` / `AttrRow` / `AssocRow`) is the data source and stays
untouched; the new widgets are pure-view — the parent pushes values per row.

## Out of scope

- The single-item tree-preview panel (the `show_picker == false` path whose top
  overlaps the title + selectbox). Separate unfinished path — leave as-is.
- Web (Svelte) frontend.
- Inline-edit mechanics beyond the description field: title/description
  click-to-edit keeps its current hand-rolled rect+keyboard approach.

## Architecture

### New widgets

Each is a `#[derive(Script, ScriptHook, Widget)]` struct with `#[deref] View`,
a `script_mod!` DSL block declaring the Turtle layout, and granular per-line
setters the parent calls per row — mirroring `recent_row.rs` exactly (Fill/Fit,
`flow`, `align`, `padding`, `spacing`, no y-offsets, no text measuring).

| Widget | File | Layout | Content |
|---|---|---|---|
| `SectionHeading` | `section_heading.rs` | `width:Fill height:Fit` | one eyebrow label — SemiBold 10, `atlas.text_dim`. `set_text` for ATTRIBUTES / RELATIONSHIPS / DESCRIPTION. |
| `AttrRowView` | `attr_row.rs` | `flow:Right width:Fill height:Fit align:{y:0.5}` | separate labels: optional visibility + Mono-11 `name`, literal `": "`, accent-11 `ty`, dim-11 `[mult]`. Real alignment, NOT string-concat. |
| `RelationshipCardView` | `relationship_card.rs` | `flow:Down width:Fill height:Fit` bordered (`draw_bg` = card fill+accent ring, the `sdf.box(..,6.0)` idiom from the current `draw_card`) | Row(accent-13 direction glyph + SemiBold-12 name) over dim-11 meta line. |

Type scale (ported from the uncommitted exploratory pass, which is a value
reference only — its host `draw_walk` gets deleted):

- section heading: SemiBold 10, `atlas.text_dim`
- kind line: `atlas.accent` 11 — a plain `Label` in the Turtle column (not a new
  custom widget)
- attr name: IBM Plex Mono 11, `atlas.text`
- attr type: Mono 11 `atlas.accent`; attr `[mult]`: Mono/Sans 11 `atlas.text_dim`
- relationship name: SemiBold 12, `atlas.text`
- relationship glyph: 13, `atlas.accent`
- relationship meta: Regular 11, `atlas.text_dim`
- divider: `atlas.surface_border`

### Body composition

`Inspector`'s container stays a `View` deref-ing widget. The body becomes a
declared Turtle `flow:Down` column (inside the existing container), populated at
`draw_walk` time via the FlatList interpose idiom from `start_screen.rs:314-345`.

Column order (diagram/picker view):

```
element_bar (existing SelectBox child + fold/pin IconButtons)
divider (full-width hairline, atlas.surface_border)
kind line (accent 11)
stereotype chips (dim, existing, when present)
SectionHeading "ATTRIBUTES"      + FlatList<AttrRowView>          (when non-empty)
SectionHeading "RELATIONSHIPS"   + FlatList<RelationshipCardView> (when non-empty)
SectionHeading "DESCRIPTION"     + editable description field (existing hand-rolled edit)
```

Variable-length attribute and relationship sections each use their own
`FlatList` (proven scroll + virtualize path), NOT a plain child-vec. Per the
`start_screen` idiom: walk the tree, and when each FlatList step surfaces,
`list.item(cx, item_id, id!(Row))` per data row, push values through the row's
`Ref` setters, `row.draw_all`. Stable per-row `item_id` (attr name / assoc key).

The description field keeps its current click-to-edit rect + keyboard handling;
only its heading becomes a `SectionHeading`.

### Registration (dead-node trap)

`app.rs` registers `script_mod(vm)` in dependency order. A custom widget mounted
as a DSL child is a dead/invisible node unless its module registered BEFORE the
consuming module. The three new widgets are consumed by `inspector_panel`, so
they register **before** `crate::inspector_panel::script_mod(vm)` (`app.rs:1599`),
after `select_box` (`:1598`). Their own deps (`atlas`, `text`) are already
registered far above. Add a comment mirroring the existing IconButton/select_box
ordering notes.

### SelectBox flat-header fix (folded first task)

The uncommitted pass gave `SelectBox` a bold web-header name (`theme.font_bold`
14) and stopped drawing the boxed field frame. Keepers:

- Keep the bold-14 name.
- Make flatness intentional: the `draw_frame` field + its `AccentFrame` DSL
  (`select_box.rs:39,80`) are now dead (never `draw_abs`'d). Remove them so no
  reader thinks a frame still paints. The open-state `draw_active` accent ring
  stays (drawn only while the list is open).
- The "border still renders" the exploratory note flagged is the **Inspector
  panel's own** AccentFrame edge (`inspector_panel.rs:60-80`), not the SelectBox
  — confirm in visual verification; it is intended and stays.

## Data flow

Unchanged upstream: `App` → `Inspector::set_subject` / `set_diagram_elements`
build `InspectorView` via `inspector.rs::build_view`. New: `draw_walk` reads
`self.proj` and, per section, feeds each FlatList row widget through its `Ref`
setters. No widget holds model state; all values pushed per draw.

## Testing / verification

- `cargo test --workspace` green. New widgets get the same kind of pure setter /
  formatting unit tests the existing helpers have where there's logic worth
  pinning (e.g. attr line parts, meta-line join stays in `inspector_panel.rs`).
- Visual verify (native, pid-safe launch + capture-by-pid, `realclick-pid.ps1`):
  launch, click a canvas node in the Orders diagram, capture. Inspector shows:
  bold selectbox name, full-width divider, accent kind, Mono aligned attrs,
  bordered relationship card, roomy spacing — all self-sizing. Resize panel /
  long name must not clip or misalign. No regression vs `typescale-08.png`
  reference in scratchpad.

## Gotchas (carried from handoff / memory)

- ALWAYS work in `.worktrees/inspector-typescale`; never edit the main checkout.
- Screenshot/verify by SPECIFIC pid only — never by process name, never
  `Stop-Process` by name (kills the user's own running editor). Makepad ignores
  synthetic `PostMessage` clicks; use `realclick-pid.ps1`. Fresh launch does not
  auto-select a node.
- Only `live_design!`/DSL value overrides hot-reload; new struct fields, consts,
  and Rust logic need a full rebuild. `run-native.ps1` builds its own
  `$PSScriptRoot` dir.
- Native-only. `web` frontend untouched.
