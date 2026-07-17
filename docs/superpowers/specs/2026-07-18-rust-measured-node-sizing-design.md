# Rust measured node sizing — one sizing authority for both frontends

**Date:** 2026-07-18
**Status:** Design
**Product:** WAML (`crates/waml/src/solve`, `crates/waml-wasm`, `crates/waml-editor`, `packages/web`)
**Scope:** Replace the crude, triplicated node-size *estimate* with a single
text-*measured* sizing function in the `waml` core crate, consumed by both the
web (via wasm) and Makepad frontends. Supersedes the "measure the DOM in the web
app" idea from the Spec 1 handoff — sizes are now computed in Rust, up front.

## Why

Node sizing is currently implemented **three times**, none of which measures text:

- `packages/core/src/canvas/layoutSize.ts` — the web estimate. Its height model
  is wrong: `rows = max(min(total, 4), 1)` forces a phantom attribute row at zero
  attributes, and `ERD_HEADER = 66` overshoots the ~40px header that actually
  renders. This is the source of the "frame hull too tall / bottom-heavy" symptom
  (see memory `spec2-frame-oversize-symptom`).
- `crates/waml-editor/src/sizing.rs` — an independent Rust reimplementation for
  the Makepad viewer. Its height model is already *correct* (compact when there
  are no attributes; `ERD_HEADER 44`, `ERD_ROW 22`).
- The solver's own `Size { w: 100, h: 40 }` fallback for unsized keys.

Three sources drift. The web one is buggy. And the solver — which positions boxes
in pixel space — is fed a size estimate, so hulls and gaps are only as good as the
estimate. The fix is to make sizing **one measured function** the solver's callers
share.

This also serves the strategic direction: the Makepad viewer (`crates/waml-editor`,
already built) is intended to eventually replace the web app. Sizing logic that
lives in Rust and is consumed by both frontends is not throwaway; a web-only DOM
measurement path would be.

## Goals

- One **classifier** node-sizing authority: `waml::solve::sizing`, measured (not
  estimated), consumed by web (via a wasm binding) and Makepad alike. (Flow and
  sequence sizing are future siblings — see "Diagram-kind scope" below.)
- Correct height model (no phantom row; real header/row heights) — kills the
  frame-oversize symptom on web by construction.
- Text-measured **width**: nodes size to the wider of their title and their
  visible attribute/value rows, measured against a bundled **IBM Plex Sans** TTF
  with `ttf-parser` — no more fixed 250/220 widths.
- Delete the duplicates: `layoutSize.ts`'s sizing role and
  `waml-editor/src/sizing.rs`.
- Unify the web renderer onto IBM Plex Sans so drawn text matches what was
  measured.

## Non-goals

- Extracting the solver into its own crate. Sizing lives in `waml::solve::sizing`
  beside the mechanical solver; a crate split, if ever wanted, is a separate
  mechanical refactor.
- Pixel-perfect hulls. Measurement uses IBM Plex Sans regular metrics; real
  rendering adds hinting/subpixel/weight nuance, so hulls are pixel-*close*, not
  bit-exact. The reported symptom is vertical and is fully fixed regardless.
- Any change to the solver's positioning geometry or its `SizeMap` input contract.
- Flow/Sequence (behavior) view sizing. Those keep their current web layout
  (`flowGraph.ts` / `sequenceLayout.ts`); moving them to Rust siblings of this
  sizer is future work (see "Diagram-kind scope").

## Design

### 1. Core sizer — `crates/waml/src/solve/sizing.rs` (new)

The mechanical solver stays exactly as it is: it consumes a caller-supplied
`SizeMap` and does not measure. A new sibling submodule owns all measurement:

```
waml::solve::sizing
    size_of(node: &Node, display: &DiagramDisplay) -> Size
    size_map(model: &Model, diagram: &Diagram) -> SizeMap
    pub const COLLAPSED_ROW_CAP: u32          // was ERD_COLLAPSED_ROWS (web)
```

- **Height** — the (correct) `waml-editor/sizing.rs` model, moved here: compact
  `Size` when attributes are hidden or the node has none; otherwise
  `header + rows * row_h`, `rows = visible.len().min(cap)`. No phantom row.
- **Width** — measured. `rows = if node.values is non-empty { values } else
  { attribute labels }` (mirrors the web `values ?? attributes` rule). Node width
  = `max(text_width(title), max_r text_width(row_r)) + horizontal padding`,
  clamped to a sane min. Text width comes from `ttf-parser` glyph advances
  (`glyph_index` then `glyph_hor_advance`, scaled by `font_size / units_per_em`)
  over a bundled IBM Plex Sans TTF (`include_bytes!`), parsed once (a
  `OnceLock<Face>`).
- Pure and deterministic. `ttf-parser` is the crates.io crate (pure Rust,
  wasm-clean) — **not** makepad's vendored fork; `waml` must not depend on
  makepad.

`waml` gains `ttf-parser` as a dependency and embeds the font bytes. `waml-cli`
and the LSP pull these transitively though they never size; accepted. Future lever
if it bites: a default-on cargo `sizing` feature the CLI can disable. Not built
now.

### Diagram-kind scope

The model keeps three distinct kinds in separate collections:
`Model.diagrams: Vec<Diagram>` (classifier — `## Members`/`## Layout`),
`Model.flows: Vec<FlowDoc>` (activity, `FlowNodeKind`), and interactions
(sequence, `Lifeline`/`SeqItem`). Only classifier `Diagram`s go through
`waml::solve`; flows and sequences are laid out by `flowGraph.ts` /
`sequenceLayout.ts` on the web today.

So `size_of(&Node)` / `size_map(&Diagram)` are **classifier-scoped by their
argument types** — a flow node or lifeline cannot reach them — and the web caller
already routes by kind (`isRealDiagram` / `behaviorViews` / `sequenceViews`). No
runtime `DiagramKind` discriminator is added; that would be dead branching (the
function only ever receives a classifier input).

When flow/sequence layout eventually moves to Rust (needed for Makepad parity once
web is retired), their sizers join this same `sizing` module as siblings
(`size_of_flow_node(&FlowNode, …)`, `size_of_lifeline(&Lifeline, …)`), keyed on
their own model types. The module is organized **per diagram kind**, not as one
polymorphic function. That move is out of scope here.

### 2. wasm binding — `crates/waml-wasm`

`solve()` keeps its caller-supplied `sizes` contract unchanged. Add one export so
the web can obtain measured sizes:

```
sizeMap(bundle: [path, md][], diagramKey: string) -> Record<string, {w, h}>
```

plus the `COLLAPSED_ROW_CAP` constant (so the web renderer shows exactly as many
rows as the measured height reserves).

### 3. Web — `packages/web` (the largest change)

`erdAwareNodeSize` is called today both at **layout** time and synchronously at
**render** time (`edges.ts:52` edge-border geometry; node rendering). With
variable, text-hugged widths, render-time callers can no longer recompute a
fixed size. So:

- Every layout pass — the solve branch **and** the dagre branch of
  `layoutActiveView` — calls wasm `sizeMap(bundle, key)` once and **stashes the
  returned `SizeMap` in canvas state**. `runSolveLayout` receives that map (as
  today); `runDagreLayout` uses it in place of `erdAwareNodeSize`.
- `edges.ts` and node rendering read node sizes from the stashed map, not from a
  recomputed estimate.
- `packages/core/src/canvas/layoutSize.ts` is deleted. `ERD_COLLAPSED_ROWS`
  re-exports from the wasm `COLLAPSED_ROW_CAP` (`RowsCompartment.svelte`).
- The web node/canvas rendering switches to **IBM Plex Sans** (`@font-face` plus
  the node text CSS) so hulls hug what is drawn.
- The Spec 1 handoff's DOM-measure / two-pass / reactive-re-solve-loop plan is
  **dropped**: sizes now arrive computed, up front, inside the existing single
  imperative solve pass. No new reactive effect, no loop-guard.

### 4. Makepad — `crates/waml-editor`

Delete `src/sizing.rs`; `scene.rs` calls `waml::solve::sizing::size_map`. The
viewer already renders IBM Plex Sans (makepad's default `THEME_FONT`), so its
hulls become pixel-close immediately. Its sizing unit tests move into the core
module.

## Components touched

| Unit | Change |
|------|--------|
| `crates/waml/src/solve/sizing.rs` | **new** — measured `size_of` / `size_map` / `COLLAPSED_ROW_CAP`; font + `ttf-parser` |
| `crates/waml/Cargo.toml` | add `ttf-parser`; embed IBM Plex Sans TTF |
| `crates/waml/src/solve/mod.rs` | expose `sizing` submodule |
| `crates/waml-wasm/src/lib.rs` | add `sizeMap` export + `COLLAPSED_ROW_CAP` |
| `crates/waml-editor/src/sizing.rs` | **deleted** — `scene.rs` calls core |
| `packages/web/src/canvas/layout.ts` | dagre + solve consume the wasm SizeMap |
| `packages/web/src/components/canvas/CanvasInner.svelte` | stash SizeMap in state; feed layout; no two-pass |
| `packages/web/src/components/canvas/edges.ts` | read stashed size, drop `erdAwareNodeSize` |
| `packages/web/src/components/canvas/nodes/RowsCompartment.svelte` | cap from wasm const |
| `packages/core/src/canvas/layoutSize.ts` | **deleted** |
| web fonts / node CSS | `@font-face` IBM Plex Sans; node text uses it |

## Sequencing

Each layer is independently green (`cargo test --workspace`, then `pnpm -r test &&
pnpm lint && pnpm build`):

1. **Core** — `waml::solve::sizing` with height model + `ttf-parser` width
   measurement + bundled font; unit tests (glyph-width cases + the moved
   compact/ERD cases).
2. **wasm** — `sizeMap` export + `COLLAPSED_ROW_CAP`; parity test.
3. **Makepad** — delete `waml-editor/sizing.rs`; `scene.rs` calls core; move tests.
4. **Web** — stash + thread the wasm SizeMap; delete `layoutSize.ts`; font swap;
   recalibrate fixtures.

## Testing

- **Core (`solve::sizing`):** `size_of` height for hidden/shown/zero/capped
  attributes (moved from `waml-editor`); width measurement asserts a wider title
  yields a wider box and a longer row widens the box, against known IBM Plex Sans
  advances. Determinism (same input → same `Size`).
- **wasm:** `sizeMap(bundle, key)` returns a map covering every member with `w/h`
  matching the core `size_map`.
- **Makepad:** existing `scene.rs` / sizing assertions updated to the core sizer's
  values.
- **Web:** `Canvas.solve.test.ts` member positions **shift** because the sizes fed
  to `solve()` change (e.g. Order's `x:314` moves) — recalibrate to the new
  deterministic values, deliberately. Dagre-view fixtures likewise. `edges.ts`
  geometry reads the stashed size.
- Rust `solver_golden.rs` uses its own synthetic `SizeMap` and is **unaffected**.

## Known ceiling / accepted consequences

- Measurement font (IBM Plex Sans regular) is not every rendered weight (titles
  may render semibold, slightly wider). Hulls are pixel-close, not exact. Fixing
  this fully means measuring per-weight; deferred.
- `waml-cli`/LSP transitively pull `ttf-parser` + the font blob. Accepted; a cargo
  `sizing` feature is the escape hatch if needed.
- One overlay position per node across diagrams (Spec 1's accepted model) is
  unchanged here.

## Open questions

- Exact horizontal padding + min-width constants — pick during implementation to
  match the current node chrome (measure the rendered box padding once).
- Whether titles should be measured with a semibold face to tighten the width
  ceiling — start with regular; revisit if titles visibly overflow.
