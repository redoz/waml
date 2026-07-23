# Inspector: groups & edges selectable

**Date:** 2026-07-24
**Scope:** native `waml-editor` inspector only. No web (`ElementPicker.svelte`) changes.

## Goal

The inspector's element-picker dropdown lists a diagram's contents. Today it
shows the placeholder, the diagram row, and one row per node (each with its
source-edges nested after). **Groups are absent entirely**, and **Diagram / Edge
rows are listed-but-disabled** (no-op on select — no `Subject` variant projects
them).

This change:

1. Lists a diagram's **groups** in the picker (flat rows — the "B" shape, not
   nested-under-member).
2. Makes **groups AND edges selectable**: picking one repoints the inspector at
   a real projection. Adds `Subject::Group` and `Subject::Edge`.
3. Groups project their **members** into a new `MEMBERS` compartment in the
   panel body.

Diagram rows stay disabled (a `Subject::Diagram` view is out of scope here).

## Background — the seam

Pure projection lives in `crates/waml-editor/src/inspector.rs`; the makepad
widget in `inspector_panel.rs` renders it. The contract:

`set_subject(model, subject)` → `build_view(model, &subject) -> Option<InspectorView>`
→ the panel's hand-drawn `draw_walk` paints title / kind / stereotypes /
`ATTRIBUTES` / `RELATIONSHIPS` / `DESCRIPTION` as absolute-positioned rows at an
incrementing `y` (`inspector_panel.rs:529-597`). No taffy, no turtle children —
adding a compartment = one more `draw_abs` block.

Groups live on `Diagram.groups: Vec<DiagramGroup>` (`waml/src/model.rs:906`), a
**recursive** tree: `{ name: String, members: Vec<String>, children:
Vec<DiagramGroup> }`. A group's properties, for inspector purposes, are its
**name** and its **member set**.

## Changes

### 1. `inspector.rs` — `Subject`

```rust
pub enum Subject {
    #[default] None,
    Classifier(String),
    Group(String),  // group name (diagram-scoped; see limitation)
    Edge(String),   // synthetic "src->tgt" id (already the Edge row's key)
}
```

### 2. `inspector.rs` — `ElementKind::Group` + group rows

Add `Group` to `ElementKind`. `diagram_elements` emits group rows **flat, right
after the diagram row, before the nodes**. Signature is unchanged — it already
receives `model` + `diagram_key`, so it looks up
`model.diagrams.iter().find(|d| d.key == diagram_key)` and reads `.groups`.

`DiagramGroup` is recursive; flatten **depth-first (parent, then its children)**,
emitting one row per group, all names, no indent. Skip the implicit top-level
group whose `name` is `""` (it is not a user-authored group). Row `key` = group
`name`, `label` = `name`.

Resulting order: `[Placeholder, Diagram, Group*, (Node (+ its Edges))*]`.

### 3. `inspector.rs` — `InspectorView` gains `members`

```rust
pub struct InspectorView {
    pub title: String,
    pub kind_label: String,
    pub abstract_flag: bool,
    pub stereotypes: Vec<String>,
    pub description: Option<String>,
    pub attributes: Vec<AttrRow>,
    pub members: Vec<String>,       // NEW — group member labels; empty otherwise
    pub associations: Vec<AssocRow>,
}
```

`members` holds display labels (member key → node title, falling back to the
key). Empty for every non-group subject.

### 4. `inspector.rs` — `build_view` projects all variants

- `None` → `None` (unchanged).
- `Classifier(key)` → as today; `members` empty.
- `Group(name)` → find the group by name in the model's diagram group trees
  (recursive search, **first match wins** — see limitation). `title = name`,
  `kind_label = "Group"`, `members` = the group's direct member labels,
  `attributes`/`associations` empty, `description = None`, `abstract_flag =
  false`, `stereotypes = []`.
- `Edge("src->tgt")` → split on `"->"`, find the edge in `model.edges` by
  `source` + `target`. `title = "<srcTitle> \u{2192} <tgtTitle>"`, `kind_label`
  = the relationship kind (`edge.kind.as_str()`), everything else empty/None.
  (Endpoint role/multiplicity detail is a fast-follow — see Deferred.)

### 5. `inspector.rs` — `subject_to_index`

Extend to resolve the selected row for every variant:

- `Classifier(key)` → the `Node` row with that key (unchanged).
- `Group(name)` → the `Group` row with that key.
- `Edge(key)` → the `Edge` row with that key.
- `None` / no match → `0`.

### 6. `inspector_panel.rs` — enable rows + wire selection

- **`build_select_items`** (`:662`): add a `Group` arm — enabled, leads with
  `Icon::SquareDashedTopSolid` (a dashed box; reads as a group frame, distinct
  from the diagram's solid `Frame` and any node icon; no new catalog glyph). Flip
  the `Edge` arm to `enabled = true` (keeps `Icon::Spline`).
- **`apply_pick`** (`:744`): drop the `matches!(row.kind, ElementKind::Node)`
  guard. Map `Node → Subject::Classifier`, `Group → Subject::Group`, `Edge →
  Subject::Edge`; `Diagram`/`Placeholder` → `None` (still no-op).
- **`subject_key`** (`:759`): return the inner key for `Group` and `Edge` too, so
  title/description inline edits attach an override to them like classifiers.

### 7. `inspector_panel.rs` — `MEMBERS` compartment

In `draw_walk`, after the `ATTRIBUTES` block and before `RELATIONSHIPS`
(`:548`), add a block mirroring `ATTRIBUTES`:

```rust
if !view.members.is_empty() {
    self.draw_dim.draw_abs(cx, dvec2(x, y), "MEMBERS");
    y += ROW_H;
    for m in &view.members {
        self.draw_label.draw_abs(cx, dvec2(x, y), m);
        y += ROW_H;
    }
    y += GAP;
}
```

## Data flow

`class_diagram_view.rs` (`sync_inspector_elements`) is **unchanged** — it already
passes `model` + `diagram_key`; `diagram_elements` now reads groups off the model
itself. Canvas / tree selection paths that already call `set_subject` gain
group/edge targets for free once `apply_pick` and `subject_to_index` handle them.

## Known limitation

`build_view` is pure `(model, subject)` with no diagram context, so
`Subject::Group(name)` resolves by **name across all diagrams' group trees,
first match wins**. Two diagrams with a same-named group could cross-resolve.
Acceptable for this iteration (groups are picked from a diagram-scoped picker, so
the name in hand belongs to the active diagram); a `(diagram_key, name)` key is
the clean fix when it bites.

## Deferred (fast-follow)

- **Edge endpoint detail** — role / multiplicity / direction rows for the two
  ends of a selected edge (reuse `AssocRow` or a dedicated endpoints compartment).
- **Group nesting in the picker** — the "A" shape (members indented under group
  headers) instead of flat group rows.
- **`Subject::Diagram`** — a diagram-level projection so the diagram row selects.
- **Persist edits to the model** — title/description overrides on groups/edges
  live in the panel's override map only, same as classifiers today.

## Tests (pure, in `inspector.rs`)

Extend the existing `tests` module (fixture-driven via `mini()`; a fixture with a
named group may be needed — add one if `mini` has none):

- `diagram_elements` emits `Group` rows after the diagram row and before nodes;
  the implicit `""` group is skipped.
- `build_view(Subject::Group(name))` → `title == name`, `kind_label == "Group"`,
  `members` populated, other lists empty.
- `build_view(Subject::Edge("a->b"))` → title carries both endpoint titles,
  `kind_label` == the relationship kind.
- `subject_to_index` resolves a `Group` row and an `Edge` row by key; unknown
  falls back to `0`.

## Non-goals

- No web frontend changes.
- No new catalog icon.
- No `card/mod.rs` (canvas node card) changes.
