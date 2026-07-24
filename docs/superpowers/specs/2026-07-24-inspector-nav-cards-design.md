# Inspector navigable reference cards — design

**Date:** 2026-07-24
**Scope:** waml-editor inspector panel, diagram/picker view (`show_picker == true`), native only. Web/Svelte untouched. The `!show_picker` single-item preview body stays immediate-mode verbatim.

## Problem

Post the inspector-turtle-widgets + inspector-groups-edges-selectable landings, the inspector body renders three section kinds inconsistently:

- **ATTRIBUTES** — `AttrRowView` FlatList (uniform rows).
- **ASSOCIATIONS** — `RelationshipCardView` FlatList: bordered, **rounded** two-line cards, roomy padding.
- **MEMBERS** — a single dim `Label` with member names joined by `\n` (`inspector_panel.rs:920`). Not a widget list, not clickable.

User-reported defects on the live panel (Group "Connectors"):

1. Vertical spacing between sections/rows is too large.
2. Members and associations lack a **uniform** presentation.
3. Member/association rows are **not navigable** — clicking does nothing; you can't shift focus to the referenced element.
4. The Group **icon is wrong** (`Icon::SquareDashedTopSolid`) — both in the panel header lead and the collapsed select-box.
5. The selected name sits **too low** in the collapsed select-box.

Root causes: MEMBERS renders as a joined Label rather than a widget list; `members: Vec<String>` (`inspector.rs:111`) and `AssocRow` carry **no element reference**, so nothing can navigate; the association card is rounded + roomy, clashing with the Atlas HUD language and the spacing complaint.

## Approach

One shared, compact, **square-cornered** bordered reference card — clickable — backing both MEMBERS and ASSOCIATIONS via their existing per-section FlatLists. The read model gains element references so a click can repoint the inspector **and** select the node on the canvas.

## Components

### 1. `RefCardView` widget (replaces `RelationshipCardView`)

A pure-view Turtle widget, `recent_row.rs`-templated:

- **Border:** thin stroke, **square corners** — drawn with `sdf.rect` (NOT `sdf.box(..,0)`, which degenerates/floods — see the fork shader gotchas). Atlas HUD styling, no rounding.
- **Compactness:** tighter than the current `RelationshipCardView` — reduced inner padding and line-height; single-line minimum height when there's no meta.
- **Line 1:** element-kind lead icon + name.
- **Line 2 (optional, dim):** meta run. Associations show the direction glyph + role + multiplicity (`meta_line`); members omit line 2 entirely.
- **Interaction:** a hit on the card emits a navigate action carrying the target `(key, kind)`. A subtle press/hover background is out of scope for v1 (rows still clearly clickable via the border + cursor); may be added later.

`RelationshipCardView` is removed; ASSOCIATIONS re-point their FlatList at `RefCardView`.

### 2. Read model carries element references (`inspector.rs`)

- New `pub struct ElementRef { pub key: String, pub kind: ElementKind, pub label: String }`.
- `InspectorView.members: Vec<String>` → `Vec<ElementRef>`.
- `AssocRow` gains `pub target_key: String` and `pub target_kind: ElementKind` (kind is `Node` or `Edge`).
- `build_view` / the group-member builder populate both from the source model — edges already carry `source`/`target`; group members already resolve to element keys.

### 3. Navigation wiring (`app.rs` + `canvas.rs`)

- `RefCardView` click → `InspectorPanel` surfaces a `navigate(key, kind)` action (same shape the picker uses).
- `App` handles it: `inspector.set_subject(cx, model, subject_from(key, kind))` to repoint the panel, **and** `canvas.select_by_key(cx, &key)` to select/focus the node on the canvas.
- New `Canvas::select_by_key(&mut self, cx, key)` — a public setter over the existing private `selected_key` field, re-resolving `selected` via the existing `selection_index` (`canvas.rs:634,1979`). Edge keys select nothing on canvas (no node) but still repoint the inspector.

### 4. Body spacing (`inspector_panel.rs`)

Reduce the declarative Turtle body column's section gaps and the card padding/line-height constants so the panel reads tight. Tune against the live panel; no structural change to the column.

### 5. Group icon — new catalog glyph (`icons.rs`)

Add the Lucide **`group`** glyph to the icon catalog (hand-authored SDF at the 18-unit size, per the catalog convention). Respect the full `Icon` invariant: enum variant == draw field == DSL registration == `draw()` match arm == `name()` arm == `ALL` list, all in the same order, with the count assertions bumped (see keep-unused-catalog-icons). Then map `ElementKind::Group` to `Icon::Group` at `inspector_panel.rs:1048`, replacing `SquareDashedTopSolid`. This drives both the panel header lead and the collapsed select-box.

### 6. Collapsed select-box baseline (`select_box.rs`)

The selected label draws at `cy - 8.0` (`select_box.rs:175`) with 14px bold. Nudge up (~`cy - 9.5`) so the cap height centers in the 32px box. Numeric-only tweak; verify against the live collapsed box.

## Data flow

```
model ──build_view──> InspectorView{ members:Vec<ElementRef>, associations:Vec<AssocRow{target_key,target_kind}> }
                          │
              per-section FlatList ──> RefCardView (border + icon + name + optional meta)
                          │ click
              InspectorPanel::navigate(key,kind) action
                          │
   App ── inspector.set_subject(subject_from(key,kind))   (repoint panel)
       └─ canvas.select_by_key(key)                       (select node)
```

## Testing

- `inspector.rs` unit tests: `build_view` populates `members` as `ElementRef`s with correct key/kind/label; `AssocRow.target_key`/`target_kind` resolve to the far endpoint; edge vs node kinds correct.
- Pure formatting helpers (`meta_line`, any ref-label fallback) unit-tested as today.
- Visual verify (native, pid-safe, per the run scripts): open the Orders diagram, click a Group node → compact square-cornered member cards, click a member card → inspector repoints + that node selects on canvas; Lucide group icon in header + collapsed box; name vertically centered in collapsed box; tighter section spacing; no regression vs `scratchpad/typescale-08.png` for attrs.

## Out of scope / non-goals

- Hover/press row background (deferred; border + cursor suffice for v1).
- The `!show_picker` preview body (untouched).
- Web/Svelte frontend.
- Any change to the picker popup or the top select-box list behavior beyond the baseline nudge and the group-icon swap.
