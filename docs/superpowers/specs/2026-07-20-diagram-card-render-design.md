# Diagram card render

## Problem

The waml-editor diagram view (e.g. the Orders Domain Model) renders every node as
a plain 200×90 box carrying only its title. The rich card render — «stereotype»
eyebrow, bold title, and a compartment of `<vis> <name> : <Type> [<mult>]`
attribute rows — already exists and works, but is wired **only** to the
classifier focus tab (reached by clicking a classifier). Diagram nodes never
reach it.

Two gates cause this:

1. **Sizing** (`sizing::size_of`) returns fixed `COMPACT` (200×90) or `ERD`
   boxes, not the card's measured hull. It only grows for attributes when the
   diagram's `DiagramDisplay.show_attributes` is set — the Orders diagram does
   not set it.
2. **Renderer** (`canvas.rs` draw loop) draws the full card (`draw_focus_card`)
   only when `focus_mode` is true. In the diagram (`set_scene`, `focus_mode =
   false`) it takes a title-only overview branch.

The node data needed for the card (attributes, declared stereotypes, element
type) is already projected onto every diagram `SceneNode` — only the size and
the draw path are missing.

## Goal

Make diagram nodes render the full card now, with everything turned on: every
node draws the complete card (eyebrow + title + all attribute rows), sized to the
real card hull. No per-diagram configuration and no properties panel in this
change — those come later. `DiagramDisplay`-driven gating (show/hide, row caps,
attribute detail) is explicitly out of scope.

## Design

### Shared scene-node projection (`scene.rs`)

Extract the per-key derivation currently inlined in `build_scene` into a helper:

```rust
/// Project model `node` into a SceneNode with a zeroed rect. The rect is filled
/// later — from the solver in `build_scene`, or measured to the card hull in
/// `sizing`. One place derives title / element_type / stereotypes / attributes
/// so measurement and drawing never diverge.
pub fn project_scene_node(model: &Model, node: &Node) -> SceneNode
```

`build_scene` calls it and then overwrites `rect` with the solved rect.
`sizing::size_map` calls it to obtain the SceneNode it measures.

### Sizing on the card hull (`sizing.rs`)

`size_of` measures the projected node's card:

```rust
pub fn size_of(model: &Model, node: &Node) -> Size {
    let scene_node = crate::scene::project_scene_node(model, node);
    let (w, h) = crate::card::card_size(&scene_node, &crate::card::mono_sheet());
    Size { w, h }
}
```

- Drops the `DiagramDisplay` parameter and the `COMPACT_*` / `ERD_*` constants
  and cap logic (everything on = all rows, full card).
- `size_map(model, diagram)` keeps its signature and member-key walk but no
  longer reads `diagram.display`.

Node rects the solver lays out now equal the card hull, so the drawn card text
lands exactly inside its box — the same invariant `build_focus_scene` already
relies on.

### Renderer (`canvas.rs`)

- Rename `draw_focus_card` → `draw_card`.
- In the node draw loop, call `draw_card` for **every** node, unconditionally.
  Delete the title-only overview branch (the `stereotype_label` eyebrow + title
  path).
- `focus_mode` keeps only its camera role: zoom 1.0 + center on the focus node
  vs. fit-to-view for the whole diagram. It no longer selects a render path.
- `node_style::stereotype_label` may become unused by `canvas.rs`; the card path
  uses `scene::focus_eyebrow` (declared stereotypes, else the metaclass label),
  so declared stereotypes now appear in the diagram (e.g. `order` →
  «AGGREGATEROOT, ENTITY»). Leave `stereotype_label` in place — `focus_eyebrow`
  still calls it.

### Unchanged

- `build_focus_scene` already sizes a single node to the card hull and renders
  the card; it stays and is now consistent with the diagram path.
- `card::class_shape` / `card::card_texts` already render all attributes with no
  cap and full `name : Type` detail — no change needed for "everything on".
- Nodes with no attributes render an eyebrow + title card (degenerate but
  correct).

## Testing

- **`sizing.rs`** — existing tests assert the fixed `COMPACT`/`ERD` constants and
  the `DiagramDisplay` gating; those are removed. Replace with card-hull
  behavior tests mirroring `card`'s: hull grows taller with more attribute rows,
  wider with a longer type; a no-attribute node still yields a positive hull;
  `size_map` covers every resolved member. Assert against measured `card_size`,
  not magic numbers.
- **`scene.rs`** — add a test that `project_scene_node` carries title /
  element_type / stereotypes / attributes; existing `build_scene` tests
  (titles, attribute rows, element types, edges, layout, bbox) stay green.
- **`canvas.rs`** — pure-function tests (`border_point`, `node_at`,
  `node_command_for`) are unaffected. The draw path has no unit coverage today;
  verification of the visual result is by running the editor on the Orders
  Domain Model diagram and confirming cards render.

## Out of scope

- Per-diagram properties panel / editing UI.
- `DiagramDisplay`-driven render modes (show/hide attributes, row caps,
  `attributeDetail`, compact vs full).
- Edge/relationship rendering changes.
