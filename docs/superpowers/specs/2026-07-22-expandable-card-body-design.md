# Expandable card body — design

## Goal

A node card with many members should not dominate the diagram. Render at most
`MAX_BODY_ROWS = 4` member rows in the card body, then a footer row (`▾ N more`).
Clicking the footer expands the card to show every member (footer becomes
`▴ show less`); clicking again collapses it back. Collapsed is the default and
is the size the solver lays out, so a big classifier stays compact until the
user opens it.

Non-goal: this is a view-only interaction, not document data. Expansion state is
never written back to the model.

## Terminology

`SceneNode` already carries a `collapsed: bool`, but that is an unrelated
*authored* directive (`- [Node](./x.md) with collapsed` in a diagram's `##
Layout`, which sizes the node as a small chip via `geometry.rs`). This feature is
distinct and is named **expand / member-overflow** throughout to avoid collision.
No existing `collapsed` behavior changes.

## Decisions (locked)

1. **Scope** — whole body. Attributes and operations are counted together as one
   ordered member list; one shared limit; one footer at the card bottom.
2. **N** — fixed constant `MAX_BODY_ROWS = 4` in the card layout module. Not
   styleable yet (YAGNI; promote to `StyleSheet` later if needed).
3. **State** — ephemeral view state: an `App`-owned `HashSet<String>` of expanded
   node keys. Cleared when the open diagram changes; survives edit-rebuilds
   (keyed by stable node key); never persisted to the model.
4. **Footer click** — toggles expand only. The click is consumed; node selection
   / inspector target is unchanged.
5. **Expansion re-solves** — the collapsed (4 + footer) size is the canonical
   measured size fed to the solver, so the default diagram is compact and edges
   route around compact rects. Toggling a node rebuilds the scene and re-solves,
   so an expanded node grows and its edges reroute correctly. (Diagrams are
   small; a full re-solve per toggle is cheap.)

## Architecture

Sizing pipeline today: `model` → `sizing::size_map` (measures each node's full
card hull) → solver → `SceneNode.rect` → edges route around rects → `draw_card`
fills the rect. Expansion must influence the *measured* size, so the expanded set
is threaded from `App` down through scene build, and mirrored onto each
`SceneNode` so the draw path and hit-test agree with what was solved.

### 1. State + threading

- `App` gains `expanded: HashSet<String>` (node keys). Cleared whenever the open
  diagram changes (same points that call `set_scene` for a new diagram); left
  intact across edit-driven rebuilds of the same diagram.
- `SceneNode` gains `expanded: bool`, defaulting `false` like `emphasized` /
  `collapsed`.
- `build_scene(model, diagram, &expanded)` threads the set:
  - `size_map(model, diagram, &expanded)` → `size_of(model, node, expanded)` sets
    the projected `SceneNode.expanded` before calling `card_size`, so the
    measured hull is the effective (collapsed-or-expanded) card.
  - Each final `SceneNode.expanded` is set from the same set, so `draw_card`
    (which re-runs `class_shape`) and the footer hit-test see the same shape the
    solver measured.
- `build_focus_scene` passes `expanded = false` (single-node focus view; overflow
  behavior there is out of scope and unchanged — focus cards show the full
  classifier as today).

### 2. Truncation in `card::class_shape`

- `class_shape` reads `node.expanded`. Combine members into one ordered list,
  attributes first then operations; call its length `M`.
- `M <= MAX_BODY_ROWS` → build the full card exactly as today; no footer.
- `M > MAX_BODY_ROWS` and `!expanded` → keep the first `MAX_BODY_ROWS` members,
  **regroup** them back into an Attributes compartment and an Operations
  compartment (preserving each compartment's styling and the inter-compartment
  divider — a truncation that lands mid-list simply yields a shorter or absent
  Operations compartment), then append a footer row `▾ {M - MAX_BODY_ROWS} more`.
- `expanded` → all `M` members in their normal compartments, then a footer row
  `▴ show less`.
- Footer styling: accent-color mono, its own row, so it reads as a control.
  (Marker glyph `▾` / `▴` prefix + accent color; adjustable during spec review.)
- `measure` captures the footer's laid-out rect via a new `Block::Footer`
  `PlacedBlock`, reusing the existing role-tagged block-capture path (same
  mechanism as `Header` / `Attributes` / `Operations`). `card_size` is unchanged
  — it measures whatever `class_shape` produced.

### 3. Interaction (canvas `FingerUp`)

- Existing `is_click` gate + `node_at` are unchanged. On a click that lands on
  node `i`: measure that node's card (`node.expanded` already set), and if the
  card has a `Block::Footer`, translate its rect to screen space
  (`screen.pos + rect * zoom`). If the release point is inside the footer rect →
  emit `GraphCanvasAction::ToggleExpand { key }` and **consume** (skip the
  select/deselect path). Otherwise fall through to today's `NodeSelect` /
  `NodeDeselect`.
- A small pure helper `footer_screen_rect(node, screen, zoom) -> Option<Rect>`
  keeps the hit math unit-testable without a GPU, mirroring `node_at` / `is_click`.
- `App` handles `ToggleExpand { key }`: flip the key in `expanded`, rebuild the
  scene for the current diagram with the updated set, and hand it to the canvas
  via **`update_scene`** (below).

### 4. `update_scene` — preserve camera + selection

`set_scene` resets `fitted`, `focus_mode`, and `selected` — correct for opening a
different diagram, wrong for a same-diagram re-solve (it would refit the camera
and drop the inspector selection on every toggle). Add:

- `GraphCanvas::update_scene(cx, scene)` — swaps the scene **without** clearing
  `fitted` (camera stays put) and re-resolves the current selection by key.
- The canvas tracks the selected **key** (not only the index), so after the swap
  it can find the new index of the same node (or clear if that node vanished).
  `NodeSelect` already carries the key; the canvas stores it when selecting.

Rendering re-runs `draw_card`, which rebuilds `class_shape` from
`node.expanded` — no change needed there beyond the new footer row it now emits.

## Testing

- **card** (`card/mod.rs`):
  - `M <= 4` → no `Block::Footer`, full card unchanged.
  - `M > 4`, collapsed → exactly 4 member rows drawn + a `▾ N more` footer;
    `N == M - 4`.
  - expanded → all `M` rows + a `▴ show less` footer.
  - footer `PlacedBlock` rect is captured and sits below the last compartment.
  - combined attributes-then-operations ordering; a truncation splitting
    mid-list regroups the kept rows into the right compartments.
- **sizing** (`sizing.rs`):
  - collapsed hull is shorter than the expanded hull for the same many-member
    node; hull still matches the drawn card in both states.
- **canvas** (`canvas.rs`):
  - pure `footer_screen_rect` / footer-hit test: a point in the footer band
    yields a toggle, a point elsewhere in the node yields a select.
  - `update_scene` re-resolves selection by key and leaves `fitted` untouched
    (camera held), versus `set_scene` which clears both.

## Out of scope

- Persisting expansion to the model.
- Per-compartment independent limits (whole-body was chosen).
- Making `MAX_BODY_ROWS` styleable.
- Focus-view (`build_focus_scene`) overflow.
- Animation of the expand/collapse transition.
