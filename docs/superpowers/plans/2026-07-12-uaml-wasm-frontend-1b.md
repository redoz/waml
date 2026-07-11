# Stage 1b: invert the frontend onto the WASM core (bundle-as-truth)

**Goal:** Make the bundle the in-memory source of truth. The `Model` becomes a
derived, read-only view (`build_model`); editing becomes ops on the bundle
(`apply_ops`) then re-derive. Retire the TS parse/serialize/migrate bodies. The
web app keeps rendering + dagre layout only. Layout-from-rules and drag→rule
editing are Stage 1c (out of scope).

**Design spec:** `docs/superpowers/specs/2026-07-12-uaml-wasm-frontend-1b-design.md`
**Parent spec:** `docs/superpowers/specs/2026-07-11-uaml-wasm-frontend-design.md`
**Stage 1a plan (done):** `docs/superpowers/plans/2026-07-11-uaml-wasm-ops-editing.md`

## Global Constraints

- **TDD.** Every behavioral task writes a failing test first, then implements to
  green. The ops adapter (Task 2) is the riskiest module — its tests must cover
  every mutation and every array-diff case.
- **Green at every commit.** Sequence so `pnpm -r test && pnpm build && pnpm lint`
  passes after each task's commit. Additive units (adapter, overlay, template
  bundle) land before the atomic store/bootstrap swap; TS deletions land last.
- **WASM call shape (verified in `crates/uaml-wasm/src/lib.rs` +
  `packages/okf/src/wasm/smoke.test.ts`):** `bundle` is a `[path, markdown][]`
  array of PAIRS; `ops` is an `OpDto[]`, each internally tagged `{op:"..."}`.
  `initWasm()` (`packages/okf/src/wasm/index.ts`) is async + memoized;
  `build_model`/`apply_ops`/`validate`/`fmt`/`split_bundle` are **sync** after
  init.
- **Ops available (the ONLY ones — `crates/uaml/src/ops/mod.rs` `enum Op`):**
  `AttrAdd`, `AttrSet`, `AttrRm`, `ValueAdd`, `ValueRm`, `RelAdd`, `RelSet`,
  `RelRm`, `NodeNew`, `NodeSet`, `NodeRm`, `NodeRename`. **No diagram / position /
  membership ops** — diagrams stay read-derived, mutations remain ~no-ops.
- **No back-compat.** Nothing released: no localStorage migration, no legacy
  share decode, delete `migrate.ts`/`migrateGraph`.
- **Regenerate wasm** after any Rust change: `pnpm build:wasm` (idempotent;
  commit the generated dir). This stage should need NO Rust change — flag it in
  review if a task appears to require one.
- **pnpm 10.12.4 works directly** (ignore corepack EPERM on Windows).
- **Commits:** Conventional Commits, terse. Do NOT add any Co-Authored-By /
  "Generated with Claude Code" trailer.

## Task overview

1. `Overlay` type + `toModelGraph(model, overlay)` adapter (pure, tested).
2. Ops adapter: store change/patch → `OpDto[]`; array diffing (pure, tested).
3. Invert `createModelStore` onto bundle-as-truth (uses Tasks 1–2).
4. Template → committed `.okf` bundle via throwaway codegen; drop the other 22.
5. Bundle-native ingest: bootstrap (await initWasm), share `url.ts`,
   `persist.ts`. Atomic app-wiring swap.
6. Retire TS `parse.ts`, `serialize.ts`, `migrate.ts`; fix `@uaml/okf` exports
   and fallout.
7. Manual end-to-end + final gate.

---

## Task 1: `Overlay` + `toModelGraph(model, overlay)`

The single function that fuses the Rust `Model` (nested `groups`, no positions)
with canvas-only overlay data into the flat `ModelGraph` the canvas already
renders. Quarantines the Rust↔TS shape gap.

**New file:** `packages/core/src/state/overlay.ts`

- [ ] **Step 1: Write the failing test** `packages/core/src/state/overlay.test.ts`

  Cover:
  - Rust `Model.diagrams[].groups` (a forest) flattens to flat
    `Diagram.members: string[]` in declared order (nested children appended
    depth-first).
  - Each node gets `position` injected from the overlay; missing → `{x:0,y:0}`.
  - Edge `sourceHandle`/`targetHandle` and synthetic `n#`/`e#` ids come from the
    overlay.
  - Empty `diagrams` ⇒ ModelGraph with `diagrams: []` (canvas shows the implicit
    all-node view, per `types.ts` contract).

- [ ] **Step 2: Define the `Overlay` type + `emptyOverlay()`**

  ```ts
  // slug-keyed canvas-only data; NONE of this persists to the bundle in 1b.
  export interface NodeOverlay { position?: { x: number; y: number }; id?: string }
  export interface EdgeOverlay { id?: string; sourceHandle?: string | null; targetHandle?: string | null }
  export interface Overlay {
    nodes: Map<string, NodeOverlay>;   // key = node slug/key
    edges: Map<string, EdgeOverlay>;   // key = stable edge key (see Task 2 note)
  }
  ```

  Note the Rust `Model` type as consumed from wasm: import the serialized shape
  from `@uaml/okf` (Task 6 re-exports the derived `Model` JSON shape) or type it
  locally against `crates/uaml/src/model.rs` (`Node` has no `position`;
  `Diagram` has `groups`, `edges` use `from`/`to`).

- [ ] **Step 3: Implement `toModelGraph(model, overlay): ModelGraph`**

  Flatten groups, inject overlay data, produce `ModelNode`/`ModelEdge`/`Diagram`
  in the `types.ts` shape. Derive `bidirectional` etc. straight from the Rust
  edge (it already carries it).

- [ ] **Step 4: Green + commit**

  ```
  pnpm --filter @uaml/core test
  git commit -m "feat(core): toModelGraph adapter + Overlay type"
  ```

---

## Task 2: ops adapter (store change → `OpDto[]`)

Pure module translating a requested store change into the ops that realize it,
then leaving `apply_ops` to the store. This is where the array diffing lives.

**New file:** `packages/core/src/state/ops-adapter.ts`

- [ ] **Step 1: Write the failing test** `packages/core/src/state/ops-adapter.test.ts`

  For each case, assert the emitted `OpDto[]`, then assert
  `apply_ops(bundle, ops)` + `build_model` yields the expected `Model`
  (round-trip through wasm; `await initWasm()` in `beforeAll`). Cases:
  - **Node scalar edit** (title / description / stereotypes / abstract / type) →
    a single `{op:"node.set", slug, ...}` with only the changed fields.
  - **Add node** → `{op:"node.new", ...}`. **Remove node** →
    `{op:"node.rm", slug, cascade}`. **Rename** → `{op:"node.rename", from, to}`.
  - **Attribute array diff** (old vs new `attributes[]`): additions →
    `attr.add`; removals → `attr.rm`; a changed field on a kept attribute →
    `attr.set` (incl. rename when the name changes but identity is matched by
    position/prior name — pick and document the matching rule).
  - **Value array diff** → `value.add` / `value.rm`.
  - **Add edge** → `{op:"rel.add", source, kind, target, ...}`. **Edit edge**
    (kind/ends/name) → `{op:"rel.set", selector, ...}`. **Remove edge** →
    `{op:"rel.rm", selector}`.
  - **Handles-only / position-only change** → emits `[]` (no op; the store keeps
    these in the overlay). Assert empty.

- [ ] **Step 2: Confirm the `OpDto` wire tags**

  Read `crates/uaml-ops-dto/src/lib.rs` and match the exact `#[serde(tag="op")]`
  tag strings and `rename`s (e.g. `abstract`) so the emitted DTOs deserialize.
  Add a comment listing the tag→variant map used.

- [ ] **Step 3: Implement the adapter**

  Functions per mutation (`nodeSetOps(prev, patch)`, `attrDiffOps(slug, prev, next)`,
  `valueDiffOps(...)`, `edgeOps(...)`, …) each returning `OpDto[]`. No wasm calls
  here — pure translation.

- [ ] **Step 4: Green + commit**

  ```
  pnpm --filter @uaml/core test
  git commit -m "feat(core): ops adapter — store change to OpDto[] with array diffing"
  ```

---

## Task 3: invert `createModelStore` onto bundle-as-truth

`packages/core/src/state/model.ts` — hold the bundle as truth, derive the Model,
run the edit loop through the ops adapter. **Keep every method signature** so the
~13 `store.*` call sites in `CanvasInner.svelte` and the details panel
(`onUpdateNode`/`onUpdateEdge`) do not change.

- [ ] **Step 1: Write the failing store test**
  `packages/core/src/state/model.test.ts` (extend existing).

  `await initWasm()` in `beforeAll`. Construct the store from a small bundle.
  Assert:
  - `store.get()` returns the derived `ModelGraph` (via `toModelGraph`).
  - `addNode` / `updateNode(scalar)` / `addEdge` / `removeNode` mutate the
    underlying bundle (re-derivable) and `emit` fires.
  - `updateNode({position})` and edge handle changes update the overlay only —
    the bundle text is byte-identical before/after.
  - A failing op (e.g. rename to an existing slug) is **rejected**: the bundle
    and derived graph are unchanged, no partial mutation.

- [ ] **Step 2: Rebuild the store internals**

  New shape (private): `{ bundle: [string,string][]; model: Model; overlay: Overlay }`.
  - Constructor takes an initial **bundle** (not a `ModelGraph`). Derive
    `model = build_model(bundle)`; seed `overlay` (positions left empty → dagre
    fills at the web layer).
  - `get()` → `toModelGraph(model, overlay)`.
  - Helper `run(ops: OpDto[])`: `const next = apply_ops(bundle, ops)` in a
    try/catch; on success replace `bundle`, recompute `model`, `emit`; on error
    keep prior state and surface the error (return it / callback — see Step 3).
  - Each mutation method: build ops via the Task 2 adapter, call `run`; for
    canvas-only changes, mutate `overlay` and `emit` without `run`.
  - The id counter / `uid` logic is superseded: node identity is the slug in the
    bundle; synthetic `n#`/`e#` live in the overlay (generate on demand in
    `toModelGraph`). Remove the now-dead counter code.

- [ ] **Step 3: Decide the error surface**

  `apply_ops` failure must not throw out of a Svelte event handler. Return a
  `{ ok: false, error } | { ok: true }` from mutators, or accept an
  `onError` callback the web layer wires to a toast. Keep it minimal; document
  the choice in the file header.

- [ ] **Step 4: Green + commit**

  ```
  pnpm --filter @uaml/core test
  git commit -m "feat(core): bundle-as-truth store — edit via apply_ops, derive via build_model"
  ```

  NOTE: `bootstrap.ts` still passes a `ModelGraph` at this point and will not
  typecheck against the app build yet — that is Task 5, which lands the ingest
  swap atomically. Keep this task's commit scoped to `@uaml/core` (its own tests
  green); the workspace `pnpm build` goes green at the end of Task 5.
  If keeping the tree fully green per-commit is required, fold Tasks 3+4+5 into a
  single commit — they form the atomic app-wiring swap.

---

## Task 4: one template as a committed `.okf` bundle

Keep exactly one template — **Orders Domain** (`orders-domain`, id
`uml_orders_domain`, "Orders Domain (UML)"). Drop the other 22.

- [ ] **Step 1: Throwaway codegen script** `scripts/gen-template-bundle.mjs`

  Import `ordersDomain` + today's `serializeBundle`, emit the bundle as a
  committed artifact the app imports as data — e.g.
  `packages/core/src/templates/orders-domain.bundle.ts`:
  ```ts
  // GENERATED by scripts/gen-template-bundle.mjs — do not edit by hand.
  export const ordersDomainBundle: [string, string][] = [ /* [path, md] pairs */ ];
  ```
  (A committed `.ts` module of pairs avoids a bundler raw-import step; the script
  is deleted after use or kept under `scripts/` clearly marked throwaway.)

- [ ] **Step 2: Run it, commit the generated bundle**

  Verify the generated markdown re-derives: `build_model(ordersDomainBundle)`
  matches the old `parseBundle(serializeBundle(ordersDomain))` Model shape (a
  one-off assertion test, then delete it).

- [ ] **Step 3: Delete the other 22 templates + registry**

  Remove their `.ts` files; reduce `packages/core/src/templates/index.ts` to the
  single entry (keep `Template` type + `?template=uml_orders_domain` id stable).
  Update `templates.test.ts`.

- [ ] **Step 4: Green + commit**

  ```
  pnpm --filter @uaml/core test
  git commit -m "feat(core): ship Orders Domain as a committed .okf bundle; drop other templates"
  ```

  (`serializeBundle` still exists here — it is deleted in Task 6, after nothing
  runtime uses it.)

---

## Task 5: bundle-native ingest + app wiring (atomic swap)

Convert every load/save path to bundles and wire the inverted store into the app.
Lands together so `pnpm build` goes green.

- [ ] **Step 1: `bootstrap.ts` — await initWasm, feed a bundle**

  `await initWasm()` before constructing the store (block first render; on
  failure show a hard load error — no TS fallback). Precedence unchanged
  (`?template=` → `#m=` share → localStorage → empty). Each source now yields a
  **bundle**:
  - template → `ordersDomainBundle`;
  - share → `readSharedModel()` returns a bundle (Step 2);
  - persisted → `loadPersistedBundle()` (Step 3);
  - empty → `[]` (empty bundle).
  Dagre still runs at the web layer on the derived graph (unchanged
  `runDagreLayout`), feeding node positions into the overlay.

- [ ] **Step 2: `share/url.ts` — encode/decode the bundle**

  `encodeModel(bundle)`: concat docs into one multi-doc string (the
  `split_bundle` input format — HTML-comment path markers, see
  `crates/uaml/src/parse.rs::split_bundle`), gzip + b64url as today.
  `decodeModel(payload)`: gunzip → `split_bundle(text)` → `[path,md][]`; null on
  error. Drop `sanitize`/`migrateGraph`. **Size check:** log the compressed
  length of `ordersDomainBundle` and assert it fits a comfortable URL-hash
  ceiling (add a test with a headroom bound; note the ceiling in a comment).

- [ ] **Step 3: `persist.ts` — store the bundle**

  New key (`mc.bundle.v1`); `persistBundle`/`loadPersistedBundle` store the
  bundle as JSON (or the concatenated string). No v1 migration.

- [ ] **Step 4: web wiring**

  Update `bootstrap.ts` exports and any consumer of `encodeModel`/persist to the
  bundle types. Wire the store error surface (Task 3 Step 3) to a toast. The
  reactive bridge (`model.svelte.ts`) is unchanged (still exposes `ModelGraph`).

- [ ] **Step 5: Full build + green + commit**

  ```
  pnpm build:wasm && pnpm build && pnpm -r test && pnpm lint
  git commit -m "feat(web): bundle-native ingest — bootstrap/share/persist on the WASM store"
  ```

---

## Task 6: retire the TS parse/serialize/migrate bodies

- [ ] **Step 1: Delete + re-export**

  Remove `packages/okf/src/parse.ts`, `serialize.ts`, `migrate.ts`. Update
  `packages/okf/src/index.ts`: drop `parseBundle`/`serializeBundle`/`OkfBundle`/
  `migrateGraph` exports; add the wasm-derived `Model` JSON types + the wasm
  entry points that consumers need (`buildModel`, `applyOps`, `validate`, `fmt`,
  `splitBundle`, `initWasm`). Keep `types.ts` (the `ModelGraph` shape the canvas
  still uses).

- [ ] **Step 2: Delete the throwaway codegen script** (`scripts/gen-template-bundle.mjs`)
  and any remaining `ordersDomain` `ModelGraph` object now that the bundle is
  committed.

- [ ] **Step 3: Sweep call sites + dead tests**

  Grep the 60+ `@uaml/okf` import sites; fix any still importing removed
  symbols. Delete/rewrite tests that exercised the deleted TS bodies
  (`parse`/`serialize`/`migrate` unit tests; `okf/io.*`). `exportImage.ts` and
  URL/share tests updated to the bundle path.

- [ ] **Step 4: Full gate + commit**

  ```
  cargo test --workspace && pnpm build:wasm && pnpm build && pnpm -r test && pnpm lint
  git commit -m "refactor(okf): retire TS parse/serialize/migrate — WASM core is the only source"
  ```

---

## Task 7: manual end-to-end + final gate

- [ ] **Step 1: Run the app** (`packages/web`, `pnpm dev`).

  Load Orders Domain. Then, confirming each renders via
  `apply_ops → build_model → toModelGraph`:
  - add a node; edit its title; add / rename / remove an attribute;
  - add an edge between two nodes; change its kind/ends; remove it;
  - remove a node (cascade removes its edges);
  - drag a node — position changes, **no bundle mutation** (overlay only);
  - reload the page — the model round-trips from localStorage (bundle);
  - build + open a share link — it reopens the same model.

- [ ] **Step 2: Final gate**

  ```
  cargo test --workspace && pnpm build:wasm && pnpm build && pnpm -r test && pnpm lint
  ```

---

## Self-review checklist (run before handing off)

- [ ] Store method signatures unchanged — `CanvasInner.svelte` + details panel
  untouched except the error-surface wiring.
- [ ] Ops adapter covers every mutation and every array-diff case; each asserted
  against a real `apply_ops` + `build_model` round-trip.
- [ ] Drag + edge-handle changes emit **no ops** and leave the bundle
  byte-identical (assert in tests).
- [ ] A failing `apply_ops` leaves bundle + derived graph unchanged (no partial
  edit); error surfaced, not thrown.
- [ ] `toModelGraph` flattens `groups` in declared order and injects overlay
  positions/handles/ids; empty-diagrams implicit view preserved.
- [ ] Share `encodeModel`→`decodeModel` is identity on the bundle; compressed
  Orders-Domain payload within the URL-hash ceiling (test asserts it).
- [ ] `parseBundle`, `serializeBundle`, `migrate.ts`/`migrateGraph`, and the 22
  dropped templates are gone; no dangling imports; `@uaml/okf` exports updated.
- [ ] `initWasm()` awaited before first render; hard error on init failure (no TS
  fallback).
- [ ] **No Rust change required** by this stage (flag in review if any task
  needed one — that would signal a missing op and a scope change).
- [ ] No Co-Authored-By / Claude trailer on any commit.

