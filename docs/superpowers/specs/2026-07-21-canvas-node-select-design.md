# Canvas node selection — design

## Goal

Primary-click a node in the diagram canvas to **select** it: draw a selection
highlight on the node and point the inspector panel at that classifier.
Clicking empty canvas deselects. This mirrors the web viewer's click-to-focus,
which the native canvas never had (primary-click only pans today).

## Non-goals

- No node move/drag (the model has no layout-edit path yet).
- No multi-select.
- No change to the right-click radial menu, pan, zoom, or fit behavior.
- No new tab, no camera move on select (inspector-only repoint).

## Interaction

The canvas primary pointer gesture is currently pan-only (`canvas.rs:267`).
Add click-vs-drag discrimination on top of it:

- On `FingerDown` (primary): record the down position (already stored as
  `drag_start_abs`) and begin the pan as today.
- On `FingerUp` (primary): if the pointer moved less than a small threshold
  (`SELECT_SLOP = 4.0` screen px) since down, treat it as a **click**;
  otherwise it was a pan — do nothing extra.
- On a click: hit-test with the existing `node_at(rects, camera, view_rect, abs)`
  helper (already used for the right-click radial).
  - Node hit → store `selected` and emit `NodeSelect { key }`.
  - No hit → clear `selected` and emit `NodeDeselect`.

Drag always pans and never selects, even when the press starts on a node.

## Selection state & highlight

`GraphCanvas` gains `selected: Option<usize>` (scene node index). The draw loop
(`canvas.rs:386`) renders the selected node with emphasis:

- **Thicker accent stroke:** add a `selected` uniform (`0.0`/`1.0`) to
  `AccentFrame` (`frame.rs`), pushed per-node via `set_uniform` before
  `draw_abs`, exactly like the existing `zoom` uniform. When `selected == 1.0`
  the shader widens the border inset/stroke (roughly 1.5× the base).
- **Drop shadow (optional):** a soft shadow quad drawn behind the selected card
  (a separate blurred `DrawColor` at an inflated rect). If the SDF shadow proves
  fiddly, **skip it** — the thicker stroke alone is the required highlight;
  the shadow is a nice-to-have.

Unselected nodes render exactly as today (thin frame, no shadow), so the common
path is visually unchanged.

`selected` is an index into the current scene, so it MUST be reset to `None`
whenever the scene is replaced (`set_scene` / `set_focus`), or a stale index
could highlight the wrong node.

## Inspector wiring

`App::match_action` (`app.rs`) already extracts canvas actions via
`canvas_action(actions)` and handles `NodeMenu`. Add handling for the two new
variants, reusing the element-picker path (`app.rs:1092`, inspector-only
repoint — no tab, no camera move):

- `NodeSelect { key }` → `inspector.set_subject(cx, &model, Subject::Classifier(key))`.
- `NodeDeselect` → `inspector.set_subject(cx, &model, Subject::None)`.

## New action variants

```rust
pub enum GraphCanvasAction {
    None,
    NodeMenu { abs: DVec2, node: usize },   // unchanged
    NodeSelect { key: String },             // new
    NodeDeselect,                            // new
}
```

The canvas emits the classifier `key` (`SceneNode::key`) directly, so `App`
never re-maps an index.

## Testing

- Unit: click-vs-drag threshold logic and `node_at` hit mapping to the right key
  (pure fns, no GPU) — assert a sub-slop up selects the node under the point and
  an over-slop up selects nothing.
- Unit: scene replacement resets `selected` to `None`.
- Manual: click node → highlight + inspector shows that classifier; drag → pans,
  no selection; click empty → deselect + inspector empty; right-click → radial
  still opens.

## Files touched

- `crates/waml-editor/src/canvas.rs` — click/drag discrimination, `selected`
  state, highlight draw, new action variants, reset on scene swap.
- `crates/waml-editor/src/frame.rs` — `selected` uniform + thicker stroke branch.
- `crates/waml-editor/src/app.rs` — handle `NodeSelect` / `NodeDeselect`.
