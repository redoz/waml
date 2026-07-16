# Makepad diagram viewer — design

**Date:** 2026-07-16
**Branch:** `makepad-viewer`
**Status:** approved design, pre-implementation

## 1. Summary

A native, GPU-rendered, **read-only** viewer for WAML/OKF diagrams, built on the
[makepad](../../../../vendor/makepad) UI engine. It loads an OKF markdown directory,
runs the existing Rust layout solver (`waml::solve`), and draws the resulting classifier
diagram — nodes, membership groups, and relationship edges — on a pan/zoom canvas.

This is the first slice of a larger idea: rebuild the WAML editor's *view* layer as a
single native Rust binary instead of the Svelte/WASM web app. The entire non-view
pipeline (`okf` parse → `model` → `solve`) is already Rust and is reused verbatim; only
the rendering, camera, and app shell are new.

### Motivation

- Native performance and headroom for large diagrams (the browser SvelteFlow/dagre path
  degrades on big graphs).
- A single native binary that opens an OKF directory directly — no browser, no node
  toolchain at runtime.
- Reuse makepad's mature primitives (GPU quad/text draw, font atlas, immediate-mode
  layout, later its `code_editor`) instead of rebuilding them.
- Move toward one Rust codebase end to end.
- It is a genuinely fun, well-bounded first target.

## 2. Scope

### In (MVP)

- Load one OKF directory into a `waml::model::Model`.
- Select one `Diagram` (CLI arg by title/key, else the first diagram).
- Compute node sizes (`SizeMap`) for that diagram's members.
- Solve to absolute rects via `waml::solve::solve_diagram`.
- Render on a GPU canvas: membership groups, relationship edges, classifier nodes.
- Pan (drag) and zoom-to-cursor (scroll).

### Out (explicitly deferred)

Editing of any kind; inspector/property panel; markdown `code_editor`; writing back to
disk; export/share; templates; automatic (dagre-style) layout; multi-diagram navigation;
file-open dialog; flow (`uml.Activity`/`StateMachine`) and sequence diagrams; attribute
row rendering beyond the node title (fast-follow, see §6).

## 3. Topology

A new binary crate `crates/waml-editor` in the `waml` repo.

```toml
# crates/waml-editor/Cargo.toml
[dependencies]
waml = { path = "../waml", features = ["serde"] }   # NOT the "wasm" feature
makepad-widgets = { path = "../../../vendor/makepad/widgets" }
```

- The `waml` core is depended on as a normal path crate with the `serde` feature only.
  The `wasm`/`tsify` machinery stays off for the native build.
- "Ripping makepad down" means **not depending** on `studio`, `xr`, `box3d`, `gamemaker`,
  or the examples. Cargo compiles only the `widgets → draw → platform → shader` subtree
  that `makepad-widgets` pulls in. A literal trimmed/vendored copy of makepad is a
  possible later optimization, not part of this work.
- The existing pnpm/WASM web app is untouched and continues to coexist; it can be retired
  later once the native viewer reaches parity.

## 4. Data pipeline (all reused `waml` API)

```
OKF dir
  └─ walk *.md → Vec<(rel_path: String, source: String)>          (new: dir loader)
       └─ waml::parse::build_model(&bundle) -> waml::model::Model  (reused)
            ├─ Model.diagrams : Vec<Diagram>   → pick one          (new: selection)
            ├─ Model.nodes    : Vec<Node>      → size the members  (new: sizing, §6)
            └─ Model.edges    : Vec<Edge>      → edge list         (reused data)
       └─ waml::solve::solve_diagram(&diagram, &sizes, &cfg)       (reused)
            -> (Solved { nodes: BTreeMap<String,Rect>,
                         groups: Vec<SolvedGroup>,
                         flags:  BTreeMap<String,FlagSet> }, Vec<Diagnostic>)
```

Reused public surface (confirmed against source, 2026-07-16):

- `waml::parse::build_model(bundle: &[(String, String)]) -> Model`
- `waml::model::{Model, Node, Diagram, DiagramGroup, Edge, RelationshipKind, DiagramDisplay}`
  - `Edge { source: String, target: String, kind: RelationshipKind, name, from_end, to_end, bidirectional }`
    — endpoints are node `key`s, matching `Solved.nodes` keys.
- `waml::solve::{solve_diagram, Solved, SolvedGroup, Rect, Size, SizeMap, SolveConfig, FlagSet}`
  - `SizeMap = BTreeMap<String, Size>`; `Size { w, h }`; `Rect { x, y, w, h }`.
  - `SolveConfig::default()` = `{ margin_px: [0, 8, 16, 32], chip: {96, 28} }`.

The solver is DSL-driven: it positions boxes according to the diagram's `## Layout`
statements (`Diagram.layout: Vec<LayoutStatement>`). It does **not** auto-place unarranged
nodes and does **not** measure node sizes — sizing is the caller's job (§6). Diagrams
without a `## Layout` section will solve to a degenerate arrangement; that is acceptable
for the MVP (we render whatever the solver returns).

## 5. Canvas widget

A custom makepad `Widget` (`GraphCanvas`) drawing in immediate mode.

- **Camera:** `{ pan: DVec2, zoom: f64 }`. Each frame, world rects (solver output, in
  diagram pixel space) are transformed to screen space in Rust
  (`screen = (world - pan) * zoom + origin`) and drawn with `draw_abs`. Explicit camera
  math — no reliance on a makepad view transform — for full control and simple hit-free
  rendering.
- **Draw order (back to front):**
  1. `Solved.groups` — one framed/boxed rect per membership group, styled by
     `SolvedGroup.shape` and `depth` (nested packages darker/indented). `DrawQuad`.
  2. Edges — for each `Model.edge` whose `source` and `target` both appear in
     `Solved.nodes`, a straight segment between the two node rects, clipped to their
     borders (border-intersection of the center-to-center line). `DrawQuad` (thin
     rotated quad) or a small line shader. Relationship *kind* styling (arrowheads,
     dashing) is minimal for MVP: a single arrow triangle at the target end; richer
     `RelationshipKind` glyphs are fast-follow.
  3. `Solved.nodes` — one rect per node: `DrawQuad` background + `DrawText` title
     (`Node.concept.title`, falling back to `Node.key`). Emphasis/collapsed from
     `Solved.flags`. Attribute rows deferred (§6).
- **Input:** left-drag pans; scroll wheel zooms toward the cursor. No selection, no
  editing (read-only).
- **Fit:** on load, compute the bounding box of all solved rects and set the initial
  camera to fit it in the viewport with padding.

## 6. Node sizing

The solver needs a `Size` per node up front. The web app measures the DOM; we replace
that with a Rust sizing function ported from `packages/core/src/canvas/layoutSize.ts`:

- MVP: fixed **compact** size `200 × 90` for every node; ERD-style sizing (header +
  capped attribute rows) for entities when the diagram's `DiagramDisplay.show_attributes`
  is set — porting the `COMPACT` / `ERD_*` / `ERD_COLLAPSED_ROWS` constants directly.
- The node renderer draws only the title in the MVP even when the ERD size reserves room
  for rows; attribute-row rendering and true text-measured sizing (via the makepad font
  atlas) are the first fast-follow, and they change only the sizing function + node
  renderer, not the pipeline.

This keeps sizing as one small, independently testable unit: `fn size_of(node, display) -> Size`.

## 7. App shell

- makepad `Window` → `Root` containing a single `GraphCanvas`.
- Directory chosen by CLI argument for the MVP: `waml-editor <okf-dir> [--diagram <title>]`.
  A file-open dialog is deferred.
- Run via makepad's normal app entry (`app_main!`). Per the makepad `AGENTS.md`, UI runs
  during development go through the Studio bridge runnable item, not raw `cargo run`;
  release builds for any perf check.

## 8. Module boundaries

Small, single-purpose units:

| Unit | Responsibility | Depends on |
|---|---|---|
| `load.rs` | walk a dir → `Vec<(path, src)>` → `Model` | `std::fs`, `waml::parse` |
| `sizing.rs` | `size_of(&Node, &DiagramDisplay) -> Size`; build `SizeMap` | `waml::model`, `waml::solve` |
| `scene.rs` | pick diagram + solve + filter edges → a plain `Scene { nodes, groups, edges }` render model | `waml::solve`, `waml::model` |
| `camera.rs` | pan/zoom state + world↔screen transform + fit | (pure math) |
| `canvas.rs` | `GraphCanvas` widget: draw scene under camera, handle input | makepad, `scene`, `camera` |
| `app.rs` | window/root shell, CLI arg, wiring | makepad, all above |

`scene.rs` is the seam: everything above it is engine-agnostic plain data; everything
below it is makepad drawing. This keeps the solver-facing logic testable without a GPU.

## 9. Testing

- Reuse `waml::solve`'s existing unit tests (unchanged).
- `sizing.rs`: unit tests asserting compact vs ERD sizes against the ported constants.
- `scene.rs`: a fixture OKF diagram → assert the built `Scene` (node keys, group count,
  edge endpoints) and cross-check node rects against `waml::solve::pretty()` output. No
  GPU needed.
- `camera.rs`: unit tests for world↔screen round-trip and fit-bbox math.
- Visual: one makepad **headless** render of the fixture diagram to PNG for eyeball and
  later regression (makepad ships a headless CPU renderer).

## 10. Risks / unknowns

- **Degenerate layout without `## Layout`.** The solver only arranges what the DSL
  specifies. Acceptable for MVP (render as-is); auto-layout is a separate future project.
  Choose a fixture diagram that *has* a `## Layout` section so the MVP looks right.
- **Coordinate/unit parity.** The solver emits pixel-space rects sized to match the web
  renderer's node sizes; our sizing must match closely enough that `## Layout` gaps look
  right. Ported constants mitigate this.
- **makepad path-dep from a foreign workspace.** `makepad-widgets` pulls a large subtree;
  first build is slow but self-contained. No API instability expected for draw/text/widget.
- **Edge border-clipping** on rotated/odd rects is a small geometry detail; center-to-
  center with rect-border intersection is sufficient for MVP.

## 11. Sequencing

1. Crate skeleton + `load.rs` + `sizing.rs` + `scene.rs` with tests (no GPU).
2. `camera.rs` + `canvas.rs`: draw nodes only, pan/zoom, fit.
3. Add groups, then edges.
4. Headless PNG fixture check.

Fast-follows (post-MVP, not in this spec): attribute-row rendering + text-measured
sizing; richer relationship-kind edge styling; file-open dialog; live re-parse; inspector.
