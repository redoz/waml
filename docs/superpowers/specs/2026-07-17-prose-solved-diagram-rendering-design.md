# Prose-solved Diagram Rendering

**Date:** 2026-07-17
**Status:** Design
**Product:** WAML / Model Canvas (`packages/web`, `crates/waml/src/solve`)
**Scope:** Render the diagram layout solver's output on the real canvas for
**Diagram** views — the first half of wiring the prose layout language into the
app. Drag stays live (free override). The prose *round-trip* (drag → written
relation) is a separate follow-on, **Spec 2**, out of scope here.

## Why

The `## Layout` prose language, its parser, and its solver all exist and are
green (`crates/waml/src/layout.rs`, `crates/waml/src/solve`, `packages/wasm`
`solve()`), but nothing renders solved output: `packages/web/src/canvas/layout.ts`
is still dagre-only. So today you cannot write layout prose and see the picture.
Until you can, the central product question — *is describing a diagram in
natural-sounding prose actually pleasant, or annoying?* — is unfalsifiable.

This spec builds the fastest surface that answers it: real canvas, real solver
output, hand-written prose. Author by editing the diagram markdown and reloading
(the existing import/load path); no bespoke prose editor is built.

## Goals

- Diagram views position nodes from `solve()` output instead of dagre.
- Render group hulls: a titled box for `frame`; `box`/`shrink` shape the layout
  but draw nothing.
- Reflect solver flags: `collapsed` renders a node as a chip; `emphasize`
  best-effort.
- Surface solver diagnostics (conflicts, unresolved refs) inline so mistakes are
  visible immediately.
- Keep drag live everywhere — nothing becomes read-only.

## Non-goals

- **The prose round-trip** (drag infers and writes a relation). That is Spec 2.
  Here, drag is a *free override*: it pins a transient position that lasts until
  the next solve, writing no prose.
- A prose-editing panel. Authoring is edit-the-`.md`-and-reload.
- The `diagram.layout` write op, alignment inference, conflict precedence,
  measured node sizes, group dragging — all Spec 2 or later.
- Making Flow/Sequence behavior views editable — unrelated, its own project.

## Design

### Solve is a drop-in for the imperative dagre pass

dagre already runs **imperatively** on explicit events — load, import,
new-package, and the "layout" tool button (`CanvasInner.svelte`
`layoutAll` / `handleToolChange` / `loadBundleWithLayout` / `handleNewPackageAdd`)
— and writes positions into the store overlay via `store.updateNode`. The OKF
format persists no coordinates, so this overlay is already ephemeral, recomputed
per session.

Solve slots into that exact shape. It is **not** run from a reactive `$effect`
(that risks a solve → `updateNode` → `$model` change → solve loop); it runs on
the same imperative triggers dagre does, plus one more (Diagram-view
activation, below).

Because drag is a free override, the existing drag write-back
(`onNodeDragStop` → `store.updateNode({ position })`) needs **no change**. A
dragged node keeps its dropped position in the overlay until the next solve
trigger overwrites it. Nothing is read-only.

### The layout-pass branch

```
layoutActiveView():
    if activeDiagram is a real Diagram (key ≠ ALL_DIAGRAM_KEY, has a doc in the bundle):
        result = runSolveLayout(bundle, activeDiagram.key, sizes)
        write result.positions → store overlay (existing updateNode path)
        solveResult = result            // $state, drives group/flag/diagnostic rendering
    else:                               // implicit "All" / freeform / behavior views
        runDagreLayout(...)             // unchanged
        solveResult = null
```

- `runSolveLayout(bundle, diagramKey, sizes)` is a new function in
  `packages/web/src/canvas/layout.ts` alongside `runDagreLayout`. It calls the
  `@waml/wasm` `solve()` bridge and returns
  `{ positions: Map<string,{x,y}>, groups: SolvedGroup[], flags: Record<string,FlagSet>, diagnostics: Diagnostic[] }`.
  `positions` is `solved.nodes` reduced to top-left `{x,y}` (a `Rect`'s `x,y` is
  already the top-left, matching how the canvas positions nodes — no centering
  fix-up like dagre needs).
- `sizes` is `{ key → erdAwareNodeSize(node, display) }` mapped `{width,height}`
  → `{w,h}` — the same static estimate dagre feeds today. Measured sizes are a
  Spec 2 refinement.
- The **All / synthesized view has no backing doc**, so `solve()` (which throws
  on an unknown diagram key) is never called for it — the guard on
  `key ≠ ALL_DIAGRAM_KEY` makes the split clean. A defensive `try/catch` around
  `runSolveLayout` falls back to leaving positions untouched and surfaces the
  error as a diagnostic rather than throwing out of a handler.

### Trigger points

`layoutActiveView()` replaces the direct `runDagreLayout` calls at every
existing trigger (`layoutAll`, `loadBundleWithLayout`, `applyMergeWithLayout`,
`handleNewPackageAdd`, the `"layout"` tool button). **One new trigger:**
activating a Diagram view (a change in `activeDiagram.key` to a real Diagram)
runs `layoutActiveView()` so the newly-shown diagram's prose takes effect.

This one-per-node-position model has a known consequence: the same node in two
solved diagrams shares a single overlay position, and re-solving on view entry
recomputes any drag override. Acceptable for a probe; a per-view position model
is Spec 2 territory. Recorded, not fixed.

### Rendering group hulls

`solve()` returns `groups: SolvedGroup[]` — each `{ rect, shape, title, depth }`.
A new `group-frame` node type renders them:

- Registered in `flowTypes.ts` `nodeTypes` as `group-frame` → a new
  `GroupFrame.svelte` (a titled bordered box sized to `rect`).
- Only `shape === "frame"` draws chrome (border + title). `box` and `shrink`
  produce a `SolvedGroup` that shaped the surrounding layout but render **no**
  node — consistent with the language spec (both are invisible).
- Pseudo-nodes: `id = "__group__" + index`, `type: "group-frame"`,
  `position: { x: rect.x, y: rect.y }`, sized via `width`/`height` style,
  `selectable: false`, `draggable: false`, `deletable: false`, `zIndex` below
  members and ordered by `depth` (outer/shallower behind inner/deeper). Their
  ids never collide with model node keys, so selection/drag/delete handlers
  ignore them without change.

The `rfNodes` `$effect` builds member nodes as it does now, then appends the
group-frame pseudo-nodes from `solveResult?.groups ?? []`.

### Flags

- `collapsed`: read from `solveResult.flags[key]?.collapsed` and pass as
  `toRFNode(..., collapsed)` (already threaded through as `_collapsed`, and the
  solver already sizes a collapsed node to the chip). This supersedes the current
  `diag.hints?.collapse?.includes(key)` source on solved views.
- `emphasize`: best-effort — apply `OkfNode`'s emphasis styling if it already
  exists; otherwise defer to Spec 2. Not a blocker.

### Diagnostics

`solveResult.diagnostics` (e.g. `LayoutConflict`,
`unresolved-layout-ref`) render in a lightweight inline banner scoped to the
active diagram — a small dismissible strip listing each warning's message. This
makes "did the prose do something dumb / did I mistype a name" visible the moment
you reload, which is core to the probe.

## Components touched

| Unit | Change |
|------|--------|
| `packages/web/src/canvas/layout.ts` | add `runSolveLayout`; `runDagreLayout` unchanged |
| `packages/web/src/components/canvas/CanvasInner.svelte` | `layoutActiveView()` branch; `solveResult` `$state`; new Diagram-activation trigger; append group nodes; collapse from flags; diagnostics banner |
| `packages/web/src/components/canvas/flow*`/`nodes` | new `GroupFrame.svelte` + `nodeTypes` registration |
| `packages/web/src/components/canvas/toRFNode.ts` | unchanged (already accepts `collapsed`) |

No Rust or wasm changes — the `solve()` bridge is already built and parity-tested
(`packages/wasm/src/solve.test.ts`, `crates/waml/tests/solver_golden.rs`).

## Testing

- **Unit (`layout.ts`):** `runSolveLayout` over a small bundle returns the golden
  positions + a `frame` group + a `collapsed` flag, mirroring the existing
  `solve.test.ts` fixture. Node keys map to `{w,h}` correctly.
- **Component:** a Diagram view with a `## Layout` prose section renders member
  nodes at solved positions and a titled frame behind a `with frame` group;
  switching to the All view falls back to dagre and renders no frame.
- **Diagnostics:** a diagram whose prose references a non-member name surfaces the
  `unresolved-layout-ref` banner.
- **Drag:** dragging a node on a solved view moves it and it stays until a
  re-solve trigger (proves free override, not read-only).

## Open questions

- Banner placement/style — top strip vs a corner toast. Cosmetic; pick during
  implementation.
- Whether Diagram-view activation should re-solve unconditionally or skip when the
  overlay already holds a solve for that diagram (to preserve drag overrides
  across a round-trip of views). Defaulting to unconditional for simplicity;
  revisit if it annoys during dogfooding.
