# Diagram Solver — WASM Bridge (Phase 2)

**Date:** 2026-07-12
**Product:** UAML / Model Canvas (`crates/uaml`, `crates/uaml-wasm`, `packages/`)
**Scope (this spec):** **Phase 2** — expose the Rust diagram layout solver over
WASM and give the Rust→JS bridge its own workspace package. No canvas
integration; that is Phase 3 with its own spec.

## Context

Phase 1 (`docs/superpowers/specs/2026-07-12-diagram-layout-solver-design.md`,
plan archived under `completed/`) landed the headless Rust solver in
`crates/uaml/src/solve/`. Its top-level entry is:

```rust
pub fn solve_diagram(diagram: &model::Diagram, sizes: &SizeMap, cfg: &SolveConfig)
    -> (Solved, Vec<Diagnostic>);
```

`Solved` carries `nodes: BTreeMap<String, Rect>`, a `groups: Vec<SolvedGroup>`
draw list, and per-node `flags`. The solver is pure, deterministic, and
golden-tested. Nothing calls it from JS yet.

Today the Rust→JS bridge lives inside `@uaml/okf`. `scripts/build-wasm.mjs`
compiles `crates/uaml-wasm` and inlines the bytes into
`packages/okf/src/generated/`; `packages/okf/src/wasm/index.ts` wraps `init` +
the exports; `okf/src/index.ts` re-exports them. `@uaml/core` and `@uaml/web`
import `build_model` / `apply_ops` / `split_bundle` / `validate` / `initWasm`
`from "@uaml/okf"`.

Two things this phase changes:

1. **The bridge outgrows okf.** "okf" is the file-format package. Phase 2 adds a
   *geometry/solver* entry point, which has nothing to do with the OKF format.
   The wasm bridge moves to a dedicated `@uaml/wasm` package.
2. **`Diagram.layout` becomes real data.** `model::Diagram.layout` is currently
   `#[serde(skip)]` (`crates/uaml/src/model.rs`) — a deliberate "for later"
   marker so the `syntax` layout AST need not implement serde. It is now later:
   the field is un-skipped so the frontend can read the layout AST (navigator,
   future drag→relation inference).

## Goals

- A `#[wasm_bindgen] solve(...)` entry that turns a bundle + node sizes into the
  solved layout, running the solver **entirely in Rust**.
- A standalone `@uaml/wasm` package owning the bridge; okf stops hosting wasm.
- `Diagram.layout` serialized end to end, guarded by a round-trip test.
- Downstream (`@uaml/core`, `@uaml/web`) keeps compiling; imports flip cleanly.

## Non-goals

- **No canvas integration.** `packages/web/src/canvas/layout.ts` stays
  dagre-only. Consuming `Solved` on the canvas (swap dagre → solved positions,
  draw group frames, drag→relation inference) is **Phase 3**, its own spec.
- **No new solver behavior.** Phase 2 only exposes what Phase 1 built.
- **No model-in solver signature.** `solve` takes the bundle and rebuilds the
  model in Rust; it does not accept a JS-side `Model` (see Decisions).
- **No coordinate persistence.** Solved pixels are render-time only, never
  written back to the bundle (unchanged from Phase 1).

## Why `Diagram.layout` must be un-skipped — and why the solver does not need it

These are **orthogonal**. The solver does *not* need layout on the wire: `solve`
runs in Rust from the bundle, where `build_model` populates `layout` in memory
regardless of the serde attribute. Un-skipping is a separate, standalone
improvement whose payoff is **frontend visibility** — the navigator and Phase 3
drag→relation inference want to read the layout AST. It rides along in this
phase because it is small and the moment is right, not because `solve` depends
on it.

The resolved `model::Diagram.layout` is `Vec<syntax::LayoutStatement>`, so the
serde subtree is bounded to these **12 types** in `crates/uaml/src/syntax.rs`:

`LayoutStatement`, `Operand`, `OperandRef`, `NameRef`, `Direction`, `Anchored`,
`Edge`, `Axis`, `Hint`, `Shape`, `Margin`, `Flag`.

`LayoutItem`, `Line`, `Section`, `Document`, `MembersBlock` are **not** in the
subtree — the resolved diagram carries statements, not the source-line wrappers.
Each of the 12 gets `#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]`,
matching the existing feature-gated pattern on `model.rs`. `Box<Operand>` in
`OperandRef::Paren` serializes transparently.

Then in `model.rs`, `Diagram.layout` drops `#[serde(skip)]` and becomes a
serialized field. Update the field comment (it currently explains the skip).

## Architecture

### 1. `solve` WASM entry (`crates/uaml-wasm/src/lib.rs`)

```rust
#[wasm_bindgen]
pub fn solve(
    bundle: JsValue,       // [(path, text)] — same shape as build_model
    diagram_key: String,   // which Diagram to solve (model::Diagram.key)
    sizes: JsValue,        // Record<string, { w: f64, h: f64 }>
    cfg: JsValue,          // SolveConfig | null | undefined
) -> Result<JsValue, JsValue>;
```

Steps:

1. `from_value` the bundle → `Vec<(String, String)>`; build the model in Rust
   (same path `build_model` uses). `layout` is populated in memory here.
2. Find the diagram with `key == diagram_key`. **Missing key → `Err`** (a caller
   bug, distinct from the solver's graceful in-diagram degradation).
3. `from_value` `sizes` → `SizeMap` (`BTreeMap<String, Size>`).
4. `cfg`: `null`/`undefined` → `SolveConfig::default()`; otherwise `from_value`.
5. Call `solve::solve_diagram(&diagram, &sizes, &cfg)` → `(Solved, Vec<Diagnostic>)`.
6. Serialize `{ solved, diagnostics }` back to `JsValue` using a
   `serde_wasm_bindgen::Serializer` with `serialize_maps_as_objects(true)`, so
   `Solved.nodes` and `Solved.flags` (both `BTreeMap<String, _>`) arrive as JS
   **objects**, not `Map`s. Diagnostics reuse the existing `Diagnostic` serde
   shape (whatever `validate` returns) for consistency.

`Size`, `SolveConfig`, `Solved`, `Rect`, `SolvedGroup`, `FlagSet`, `Shape`, and
`Diagnostic` need `serde` derives sufficient for the directions used
(`Deserialize` for the inputs `Size`/`SolveConfig`; `Serialize` for the outputs).
Add whichever are missing, feature-gated like the rest of the crate.

### 2. `@uaml/wasm` package

New `packages/wasm/`:

- **Moves in:** `packages/okf/src/generated/*` (wasm-bindgen glue + inlined
  bytes), `packages/okf/src/wasm/index.ts` → `packages/wasm/src/index.ts`, and
  okf's `scripts/copy-wasm-glue.mjs`.
- **`scripts/build-wasm.mjs`:** retarget `outDir` from
  `packages/okf/src/generated` → `packages/wasm/src/generated`.
- **`packages/wasm/package.json`:** `name: "@uaml/wasm"`, mirrors okf's build
  (`tsc` + copy-glue), `main`/`types` → `dist`.
- **Add the `solve` wrapper** to `packages/wasm/src/index.ts` alongside the
  re-exported `initWasm`/`apply_ops`/`build_model`/`fmt`/`split_bundle`/`validate`:

```ts
export interface Size { w: number; h: number }
export interface Rect { x: number; y: number; w: number; h: number }
export interface FlagSet { emphasized: boolean; collapsed: boolean }
export interface SolvedGroup {
  rect: Rect;
  shape: "Frame" | "Box" | "Shrink";
  title: string | null;
  depth: number;
}
export interface Solved {
  nodes: Record<string, Rect>;
  groups: SolvedGroup[];
  flags: Record<string, FlagSet>;
}
export interface SolveConfig { margin_px: [number, number, number, number]; chip: Size }

// Mirrors crates/uaml::Diagnostic serde output. No TS Diagnostic type exists
// today (validate() is typed `any`); this wrapper introduces it.
export interface Diagnostic {
  severity: "error" | "warning";   // serde rename_all = "lowercase"
  code: string;                    // kebab-case, e.g. "unresolved-layout-ref"
  message: string;
  file: string;
  line: number;
  span: [number, number] | null;
}
export interface SolveResult { solved: Solved; diagnostics: Diagnostic[] }

export function solve(
  bundle: [string, string][],
  diagramKey: string,
  sizes: Record<string, Size>,
  cfg?: SolveConfig,
): SolveResult;
```

The wrapper hand-types the generated `solve(bundle, diagramKey, sizes, cfg ?? null)`.

### 3. Flip consumers off okf's wasm

- **`okf/src/index.ts`:** drop the
  `export { initWasm, apply_ops, build_model, fmt, split_bundle, validate } from "./wasm/index"`
  line. okf keeps exporting **types/slug/grammar only** (`Diagram`, `ModelGraph`,
  etc. stay in okf — those are not wasm).
- **Flip 4 import sites** `from "@uaml/okf"` → `from "@uaml/wasm"`:
  - `packages/core/src/share/url.ts` (`split_bundle`)
  - `packages/core/src/state/model.ts` (`build_model`, `apply_ops`)
  - `packages/web/src/main.ts` (`initWasm`)
  - `packages/web/src/test/setup.ts` (`initWasm`)
  - plus any `*.test.ts` importing these symbols from okf.
- **Deps:** add `"@uaml/wasm": "workspace:*"` to `packages/core` and
  `packages/web`. Drop `@uaml/okf` from a package only if nothing else in it
  still imports okf types (core still uses okf types → keep; check web).
- **Build order:** root `package.json` `build` and `build:wasm` must build
  `@uaml/wasm` before `@uaml/core`/`@uaml/web`.

## Data flow

```
bundle [(path,text)]  ─┐
diagramKey             ├─►  @uaml/wasm solve()  ─►  crates/uaml-wasm::solve
sizes Record<k,{w,h}>  │        (JS wrapper)            │ build_model (layout in mem)
cfg? SolveConfig      ─┘                                │ pick diagram by key
                                                        │ solve_diagram()
                                                        ▼
                          { solved: {nodes,groups,flags}, diagnostics: [...] }
                          (maps-as-objects)  ◄───────────┘
```

The web caller (Phase 3) already holds the bundle via `store.getBundle()` and
supplies sizes from `erdAwareNodeSize`. Phase 2 ships the mechanism only.

## Decisions

- **bundle-in, not model-in.** `solve` takes the bundle and rebuilds the model in
  Rust. Rejected model-in (passing the JS-held `Model` back) because it needs a
  new `getRawModel()` store accessor and a `from_value::<Model>` round-trip for
  no real gain — `build_model` already runs on every edit, so one more call is
  free, and bundle-in keeps `solve` self-contained.
- **`{ solved, diagnostics }`, not bare `Solved`.** Diagnostics (unknown operand,
  cycle, alignment conflict) are how authors learn why a relation didn't take;
  dropping them on the floor at the bridge would waste Phase 1's work.
- **maps-as-objects.** `Record<string, Rect>` is far more ergonomic in JS/TS than
  a `Map`; the serializer flag is a one-liner and only affects the `solve` return.
- **Missing `diagram_key` is an `Err`, not graceful.** In-diagram degradation
  (unknown operands, cycles) is the solver's job and stays warn+drop; asking to
  solve a diagram that isn't there is a caller mistake.

## Testing

- **Rust round-trip (un-skip guard):** a `crates/uaml` test that builds a model
  from a bundle containing a `## Layout` section, serializes the model to JSON
  (serde), deserializes it back, and asserts `diagram.layout` survives equal.
  Proves the 12 syntax types round-trip and the field is no longer skipped.
- **`solve` parity (`packages/wasm`):** a vitest that feeds a fixed `## Layout`
  bundle + a fixed `sizes` map through `solve()` and asserts the returned rects
  match the values the Rust golden already pins (same fixture, same numbers).
  This is the phase gate — it proves the bridge is faithful without any canvas.
- **Diagnostics surfaced:** a case with an unresolved layout operand asserts a
  warning (`code: "unresolved-layout-ref"`, the real `DiagCode::UnresolvedLayoutRef`)
  appears in `result.diagnostics`.
- **Existing suites stay green** after the package move: `pnpm -r test`, `pnpm
  lint`, `pnpm build`, and `cargo test -p uaml -p uaml-wasm`. The import flip and
  build-order change must not break `@uaml/core`/`@uaml/web`.

## Open questions

- **`SolveConfig` from JS:** confirm the field names serialize as `margin_px` /
  `chip` (snake) — the TS interface must match serde output. If it drifts, add
  `#[serde(rename_all = ...)]` or align the TS names during impl.
- **`web`'s okf dependency:** verify whether `packages/web` still imports any okf
  *type* after the flip; if not, drop the `@uaml/okf` dep from web.

## Phase 3 (outline — separate spec)

`packages/web` canvas consumes `Solved` for Diagram documents: replace the dagre
path with solved positions, draw group hulls/frames, and (further out)
drag→relation inference writing sentences back into `## Layout`. Not in this spec.
