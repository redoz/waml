# OKF expand step 1 — add `concept` to `model::Node` (additive)

> **Rigor:** tdd-per-task

## Context

First **expand** step of the OKF two-tier migration (expand/contract, never red-CI).
`model::Node` today is flat: it duplicates OKF fields (`title`/`description`/…) and
**drops** the non-UML OKF fields (`tags`/`resource`/`timestamp`/`links`/`citations`/
`role`/`extra`) for every document. The `okf` tier already lands on `origin/main`
(`crates/uaml/src/okf.rs`: `Concept`, `Bundle`, `okf::project`, `okf::build_bundle`).

This step nests the lossless `okf::Concept` onto every `Node` **additively** — every
existing flat field stays exactly as-is, so no Rust/TS/Svelte consumer breaks — and
regenerates the TS wire (`packages/okf/src/types.ts`) + the prebuilt wasm blob so the
new `concept` field reaches the front-end. Later migrate (readers flat → `concept.*`)
and contract (delete flat fields) steps ride on top; NOT in scope here.

**Out of scope (do NOT do):** `uml.*` → `uaml.*` rename (dropped by user); full-path
node keying (`key` stays the slug); deleting any flat `Node` field; migrating any
front-end reader.

### Gate — FULL, must stay green

`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`

`packages/web` AND `packages/core` are LIVE and gated — do NOT relax to
`--filter @uaml/okf`. Because the change is additive (a new optional-on-the-wire
`concept` field, existing fields untouched), web/core compile unchanged; only
snapshot/golden assertions that pin the exact `Node` JSON need the new field added.

### Gotchas (read before starting)

- **serde is NOT a default cargo feature.** `cargo test -p uaml` silently skips serde
  goldens; only `cargo test --workspace` (uaml-wasm turns serde on) exercises the wire.
- **wasm blob is prebuilt.** `packages/okf/src/generated/wasm-inline.ts` is regenerated
  only by `node scripts/build-wasm.mjs` (root `build:wasm`). Regen it so the new
  `concept` field reaches TS; `pnpm build` alone does not rebuild the blob.
- **types.ts is tsify/hand-maintained.** `ModelNode` in `packages/okf/src/types.ts`
  must GAIN `concept: Concept` (the `Concept` type already exists there, added in
  `c5e8395`) while keeping every existing flat field. Purely additive.
- `RTK_DISABLE=1` on gate commands and any git you reason about (RTK mangles large output).

---

### Task 1: Nest `okf::Concept` onto `model::Node` additively + regen TS/wasm wire

Add a `concept: okf::Concept` field to `model::Node`, populate it from the existing
`okf::project` (single source — do NOT re-derive OKF fields by hand), keep ALL current
flat fields byte-identical, then propagate the additive wire change to TS + the wasm blob.

- **`crates/uaml/src/model.rs`** (`Node`, ~line 338): add `pub concept: crate::okf::Concept`
  as the first field. Keep every existing field (`key`, `ty`, `title`, `stereotypes`,
  `abstract_`, `description`, `attributes`, `values`, `body`, `annotates`) unchanged.
  Under `feature = "serde"` it serializes as a nested `concept` object (no rename needed).
- **`crates/uaml/src/parse.rs`** (`ParsedDoc` ~360, `parse_bundle` ~366, `build_node` ~387):
  add `concept: crate::okf::Concept` to `ParsedDoc`; in `parse_bundle` compute it via
  `crate::okf::project(path, text)` (both are in scope in the map closure); in `build_node`
  set `concept: p.concept.clone()`. Do NOT change slug keying, `keyset`, or any flat field.
- **`packages/okf/src/types.ts`** (`ModelNode`, ~line 138+): add `concept: Concept;` to
  `ModelNode`, keeping every existing flat field. `Concept` already declared in this file.
- **`packages/okf/src/generated/wasm-inline.ts`**: regenerate via `node scripts/build-wasm.mjs`
  (root `build:wasm`) so the Rust wire change reaches TS. Commit the regenerated blob.
- Update any golden/snapshot that pins the exact `Node`/`ModelNode` JSON to include the
  new `concept` field — VERIFY each diff is exactly "gains a nested `concept` mirroring the
  doc's OKF projection", never a change to an existing field:
  - Rust: `crates/uaml/tests/serde_shape.rs`, `crates/uaml/tests/golden.rs`,
    `crates/uaml/tests/ops_golden.rs`, `crates/uaml/tests/solver_golden.rs`,
    `crates/uaml-wasm/tests/native.rs` (only those that assert full `Node` JSON).
  - TS: any `packages/core` / `packages/web` snapshot asserting a full model-node object.
- **Headline check (must hold):** build a bundle with a non-UML `Playbook` doc (tags,
  resource, timestamp, links, citations); its `Node` (if UML-classified) — and the parallel
  `okf::project` — carry every field on `concept`. Existing UML nodes keep their flat fields.

**Files:** crates/uaml/src/model.rs, crates/uaml/src/parse.rs, packages/okf/src/types.ts, packages/okf/src/generated/wasm-inline.ts, crates/uaml/tests/serde_shape.rs, crates/uaml/tests/golden.rs, crates/uaml/tests/ops_golden.rs, crates/uaml/tests/solver_golden.rs, crates/uaml-wasm/tests/native.rs
