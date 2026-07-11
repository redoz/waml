# UAML WASM frontend — Stage 1b design spec

**Status:** design locked, ready for implementation plan.
**Parent spec:** `docs/superpowers/specs/2026-07-11-uaml-wasm-frontend-design.md`
**Stage 1a plan (done):** `docs/superpowers/plans/2026-07-11-uaml-wasm-ops-editing.md`
**Precedes:** Stage 1c (layout-from-rules engine + drag→rule editing).

## Terminology

**bundle** — the set of `.md` docs that make up one model, carried as a
`[path, markdown][]` array. Codebase jargon, **not** an OKF format term (the
format defines individual markdown docs). Distinct from `uml.Package`, which is
a grouping classifier *inside* a model. In code: TS `OkfBundle`, Rust
`type Bundle = Vec<(String, String)>`, wasm `[path, md][]`.

## Problem

Stage 1a shipped an inlined WASM build of `crates/uaml` (`build_model`,
`validate`, `apply_ops`, `fmt`, `split_bundle`) proven callable from JS. But
nothing in `core`/`web` imports it yet — the app still runs the hand-maintained
TypeScript port (`@uaml/okf`: `parse.ts`, `serialize.ts`, `grammar.ts`,
`migrate.ts`) with `ModelGraph` as its in-memory source of truth. Every parse /
serialize / edit rule lives twice and must be kept in sync.

## Goal

Invert the frontend onto the Rust core:

- **Bundle is the source of truth.** The `Model` (nodes/edges/diagrams) is a
  **derived, read-only** view (`build_model`). Editing is **ops on the bundle**
  (`apply_ops`), then re-derive. No `Model → bundle` regenerator (the parent
  spec bans it as lossy).
- Retire the TS parse/serialize/migrate bodies. The web app keeps **rendering**
  and **dagre layout** only.

Layout stays on dagre this stage. Rule-driven layout and drag→rule editing are
**Stage 1c** (see "Deferred").

## Key findings that shaped this design

These are load-bearing facts discovered while scoping; they explain why the
design looks as it does.

1. **The Rust core has no position concept.** `MemberLine` is `{ title, slug }`;
   `MEMBER_RE` (grammar.rs) *rejects* any `- [X](./x.md) at 100,200` line. So
   feeding a positioned bundle to `build_model` would drop the position **and
   the whole membership**, and `fmt`/`apply_ops` strip `at x,y` on round-trip.
   → Positions cannot ride the bundle. They are **ephemeral** (dagre-on-load),
   held in the overlay, not persisted this stage.

2. **Placement is meant to be rule-driven, not coordinate-driven.** Node
   placement is expressed by declarative `## Layout` rules (`A left of B`,
   alignments) parsed into `Diagram.layout` (currently `#[serde(skip)]`). A
   layout engine turns rules into geometry. That engine is Stage 1c; until then
   dagre stands in and the rules are carried but not honored.

3. **The ops surface is thin.** `Op` = attr / value / rel / node only. **No
   diagram ops, no membership ops, no position ops.** Diagram editing in the app
   is already nascent — `CanvasInner.svelte` notes `updateDiagram` is "a no-op
   … no real diagram in the model yet." → Diagrams stay **read-derived**; their
   mutations remain ~no-ops. No new Rust ops this stage.

4. **The Rust `Model` shape differs from the TS `ModelGraph`.** Rust
   `Diagram` exposes nested `groups {name, members, children}`; TS `Diagram` is
   flat `members: string[]` (+ `hints`, `display`). Rust `Node` has no
   `position`. → A single `toModelGraph(model, overlay)` adapter absorbs the gap
   so canvas components stay unchanged.

5. **Nothing is released.** No back-compat is required — no localStorage
   migration, no legacy share-link decode, no `migrateGraph`.

## Architecture

### Store: bundle-as-truth

Today `createModelStore` holds a `ModelGraph` and mutates it directly. The new
store holds:

```
store = {
  bundle:  [path, markdown][]     // SOURCE OF TRUTH
  model:   Model                  // derived: build_model(bundle)
  overlay: Overlay                // canvas-only, slug-keyed (see below)
}
```

`initWasm()` is async and MUST be awaited **once** at bootstrap before the store
is constructed and before first render. Every wasm entry point used afterward
(`build_model`, `apply_ops`, `validate`, `fmt`) is **synchronous**, so the
store's mutation API stays synchronous and the reactive bridge
(`model.svelte.ts`) is unaffected.

The public reactive view (`$model`) continues to expose a `ModelGraph`, produced
by `toModelGraph(store.model, store.overlay)`.

### Edit loop: an ops adapter behind stable signatures

The store keeps its existing method signatures (`addNode`, `updateNode`,
`removeNode`, `addEdge`, `updateEdge`, `removeEdge`, …) so the ~13 call sites in
`CanvasInner.svelte` and the details panel do not change. Each method's body
becomes:

```
change → build op(s) → apply_ops(bundle) → build_model → toModelGraph → emit
```

- **Scalar node/edge fields** (title, type, stereotypes, abstract, description;
  edge kind, ends, name) map directly to `NodeSet` / `RelSet` from the patch —
  the patch already names what changed, no diff needed.
- **Attribute / value arrays** (a `Partial<ModelNode>` patch replaces the whole
  array) are **diffed** old-vs-new into `AttrAdd` / `AttrSet` / `AttrRm` /
  `ValueAdd` / `ValueRm`.
- **Add/remove node** → `NodeNew` / `NodeRm`. **Add/remove edge** →
  `RelAdd` / `RelRm`. **Rename** → `NodeRename`.
- **Pure-canvas edits** — drag (`updateNode({position})`) and edge handle
  changes — touch the **overlay only**. No op, no re-derive of the bundle.

The ops adapter is the single riskiest module; it is the one place the diff
logic lives and must be unit-tested against `apply_ops` round-trips.

### Overlay

Slug-keyed, canvas-only, **not persisted to the bundle** this stage:

```
Overlay = Map<slug, {
  position?:     { x, y }          // ephemeral, seeded by dagre, updated on drag
  nodeId?:       string            // synthetic n# for xyflow
  edgeIds?:      ...               // synthetic e#
  handles?:      { source?, target? }   // per-edge sourceHandle/targetHandle
}>
```

Positions are recomputed by dagre on load and whenever the graph's structure
changes; drags update the overlay in place. Handles and synthetic ids exist to
satisfy xyflow and are regenerated as needed. Persisting any of this into the
format is Stage 3 work.

### toModelGraph(model, overlay)

Assembles the flat `ModelGraph` the canvas already renders:

- Rust `Diagram.groups` → flat TS `Diagram.members: string[]` (flatten the group
  forest, preserving order).
- Inject `position` onto each node from the overlay (default `{0,0}` → dagre
  fills it).
- Attach synthetic `n#`/`e#` ids and edge `sourceHandle`/`targetHandle` from the
  overlay.

Keeping the flat shape quarantines the Rust↔TS divergence in this one function.
Canvas components migrate onto the raw `Model` shape in Stage 3, when the overlay
dissolves.

## Ingest / egress — all bundle-native

Every path that loads or saves a model deals in **bundles**, not `ModelGraph`.

- **Template.** Keep exactly one template — **Orders Domain** (`orders-domain`,
  id `uml_orders_domain`). Convert it to a committed `.okf` bundle via a
  **throwaway codegen script** that runs today's `serializeBundle` once; commit
  the bundle; then delete `serializeBundle` and all template `.ts`. Drop the
  other 22 templates and their registry entries.
- **Share link.** `url.ts` encodes the **bundle** through the existing
  gzip + base64url path (fflate level 9). The natural codec reuses the wasm
  primitives: encode = concatenate the docs into one multi-doc string; decode =
  `split_bundle` back to `[path, md][]` (a small JSON envelope is an acceptable
  alternative). **Size check:** the
  compressed Orders-Domain bundle must fit comfortably in a URL hash; verify
  during implementation (markdown compresses well, but confirm, and note the
  ceiling).
- **localStorage.** Persist the **bundle** string under a fresh key. No v1
  migration (nothing released).

**Retired this stage:** `parseBundle`, `serializeBundle` (after codegen),
`migrate.ts` / `migrateGraph`, the 22 dropped templates, and any
`ModelGraph`-as-transport assumptions in `url.ts` / `persist.ts` / `bootstrap.ts`.

## Components and boundaries

| Unit | Responsibility | Depends on |
|---|---|---|
| `initWasm()` (okf/wasm) | memoized async init; must resolve before first render | inlined wasm module |
| ops adapter (core/state) | change → op(s); diff arrays; call `apply_ops` | wasm `apply_ops`, `OpDto` |
| `createModelStore` (core/state) | hold bundle+model+overlay; run edit loop; emit | ops adapter, `build_model`, `toModelGraph` |
| `toModelGraph` (core/state) | fuse `Model` + overlay → flat `ModelGraph` | `Model`, `Overlay` |
| overlay (core/state) | slug-keyed canvas data; dagre positions, handles, ids | — |
| ingest (core/share, core/state/persist, web/bootstrap) | template/share/localStorage ⇄ bundle | wasm, `url.ts` codec |

## Data flow

```
load:   bundle ─build_model→ Model ─toModelGraph(+overlay)→ ModelGraph ─$model→ canvas
edit:   canvas → store.method(patch)
          ├─ semantic → op(s) → apply_ops(bundle) → new bundle → build_model → toModelGraph → emit
          └─ canvas-only (drag/handle) → overlay update → toModelGraph → emit
save:   bundle → localStorage / share-url codec   (no Model→bundle; bundle is already truth)
```

## Error handling

- **`initWasm()` failure** — surface a hard load error; the app cannot run
  without the core. No silent TS fallback (the TS port is being removed).
- **`apply_ops` error** — returns `op {index}: {reason}`. The store must NOT
  mutate the bundle on a failed op; the edit is rejected and the prior state
  stands. Surface a non-fatal notice; log the op + reason.
- **Malformed share payload** — `decodeModel` returns null (as today); the app
  opens empty rather than throwing.

## Testing

- **Ops adapter (unit).** For each store mutation, assert the emitted op(s) and
  that `apply_ops` + `build_model` yields the expected `Model`. Cover the array
  diff cases (attribute add/rename/retype/remove; value add/remove).
- **toModelGraph (unit).** Group flatten order; overlay injection of positions,
  handles, synthetic ids; empty-diagram implicit-view behavior.
- **Round-trip (integration).** load bundle → edit via store → the resulting
  bundle re-derives to the expected `Model`; `fmt` idempotence holds.
- **Ingest.** Template bundle loads; share encode→decode is identity on the
  bundle; localStorage persist→rehydrate is identity.
- **End-to-end (manual, required).** Run `packages/web`: load Orders Domain,
  add/edit/remove a node, an attribute, and an edge; confirm each renders via
  `apply_ops → build_model → toModelGraph`; drag a node (overlay only); reload
  and confirm the bundle round-trips.
- **Gate:** `cargo test --workspace && pnpm build:wasm && pnpm build &&
  pnpm -r test && pnpm lint`.

## Deferred (not this stage)

- **Stage 1c** — layout-from-rules engine (honor `## Layout`), drag → infer a
  layout constraint → edit the rule → re-derive. This is the headline feature;
  it needs a rules→geometry engine, which dagre is not.
- **Stage 2** — full Rust layout engine (`crates/uaml-layout`), swap web off
  dagre.
- **Stage 3** — fold overlay data (positions via diagram members, edge-handle
  overrides encoded in the format, drop synthetic ids for slug keys); canvas
  components migrate onto the raw `Model` shape; delete the overlay and remaining
  `@uaml/okf`.
- **Diagram editing ops** — membership / display / create-edit in Rust; arrives
  when diagrams become load-bearing.
- **Format-preserving edits** — Stage 4 (parent spec).

## Non-goals

- `Model → bundle` regenerator — never.
- Byte-exact preservation — canonical `fmt` normalizes until Stage 4.
- Back-compat / migration — nothing is released.
- Porting rendering or dagre into Rust this stage.
