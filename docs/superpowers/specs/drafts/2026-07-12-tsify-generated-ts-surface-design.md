# Generated TypeScript Surface via Tsify (DRAFT)

**Date:** 2026-07-12
**Status:** DRAFT — incomplete, resume later.
**Product:** UAML / Model Canvas (`crates/uaml`, `crates/uaml-ops-dto`, `crates/uaml-wasm`, `packages/`)
**Scope:** Auto-generate the WASM boundary's TypeScript types from Rust with
`tsify`, replacing hand-written TS interfaces. Cross-cuts the Phase 2 solver
bridge spec (`2026-07-12-diagram-solver-wasm-bridge-design.md`).

## Problem

The Phase 2 bridge spec hand-writes every boundary type in TS
(`Size`, `Rect`, `FlagSet`, `SolvedGroup`, `Solved`, `SolveConfig`,
`Diagnostic`, `SolveResult` — that spec's lines 135-169). Current marshalling is
`serde-wasm-bindgen` + `serde_json`, which moves data but emits **no `.d.ts`
shapes** — the TS side sees the exports as `any` (e.g. `validate()` is typed
`any` today). Every boundary type is therefore duplicated: one serde struct in
Rust, one hand-kept interface in TS. They drift silently; the spec's own "Open
questions" flag `SolveConfig` field-name drift (`margin_px`/`chip`) as a risk.

## Idea

`tsify` derives real TS interfaces straight from the serde structs. One source of
truth in Rust; TS types generated at wasm-build time. Reads existing serde attrs
(`rename_all`, `tag`, `rename`) so generated TS matches the JSON exactly —
kills the drift class entirely.

```rust
#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct LayoutResult { /* ... */ }
```

## Crate layout — already correct

The 3-crate split we need is already in place:

- `uaml` — core; serde behind optional `serde` feature; no wasm deps.
- `uaml-ops-dto` — serde wire contract (`OpDto`); depends on `uaml`; serde only.
- `uaml-wasm` — `cdylib` + `rlib`; `wasm-bindgen` + `serde-wasm-bindgen`.

So the seam for gating tsify is the **DTO crate**, not the wasm crate — the
boundary types live in `uaml-ops-dto` (and, for the solver, output types in
`uaml`). Native CLI (`uaml-cli`) depends on `uaml-ops-dto` with no features and
stays plain serde; `uaml-wasm` flips the feature on.

## Feature gating (optional-for-wasm)

`tsify` pulls `wasm-bindgen` transitively, so it MUST stay off the native build.

```toml
# uaml-ops-dto/Cargo.toml
[features]
wasm = ["dep:tsify"]

[dependencies]
serde = { workspace = true }
tsify = { version = "0.4", optional = true }
```

```toml
# uaml-wasm/Cargo.toml
uaml-ops-dto = { path = "../uaml-ops-dto", features = ["wasm"] }
```

Same `#[cfg_attr(feature = "wasm", ...)]` pattern already used for serde gating
in `crates/uaml/src/model.rs`. Both the `derive(Tsify)` and the `#[tsify(...)]`
attr must be gated on the same feature (the attr only exists when the derive is
present).

**Guard:** confirm `uaml-cli` does NOT transitively enable the `wasm` feature —
native build must stay wasm-free.

## What this changes in the Phase 2 spec

- Delete the hand-written TS interface block (bridge spec lines 135-169). The
  wrapper in `packages/wasm/src/index.ts` imports generated types instead of
  declaring them.
- The solver output types that live in `crates/uaml` (`Solved`, `Rect`,
  `SolvedGroup`, `FlagSet`, `SolveConfig`, `Diagnostic`) also need tsify derives
  — so `uaml` itself grows an optional tsify path, OR those output types get
  re-declared as tsify DTOs in `uaml-wasm`. **UNDECIDED — see open questions.**
- "Open questions" re `SolveConfig`/`Diagnostic` field-name drift become moot —
  generated types can't drift from serde.

## Open questions (resume here)

1. **Where do solver OUTPUT types get their tsify derive?** They live in
   `crates/uaml/src/solve/`, not the DTO crate. Options:
   a) add an optional `wasm`/`tsify` feature to `uaml` core and gate derives
      there (mirrors existing serde gating, but grows core's feature surface);
   b) mirror them as thin tsify DTOs in `uaml-wasm` and map at the boundary
      (keeps core clean, adds a mapping layer + a second type set — the drift we
      are trying to kill).
   Leaning (a). Decide before impl.
2. **`serde(flatten)` support.** tsify's `flatten` handling is spotty. Audit the
   boundary types for `#[serde(flatten)]`; if any, verify generated output or
   restructure.
3. **tsify `.d.ts` wiring into the build.** How generated `.d.ts` flows through
   `scripts/build-wasm.mjs` + `copy-wasm-glue.mjs` into `packages/wasm` — tsify
   emits into the wasm-bindgen output; confirm the copy step carries it.
4. **tsify version / maintenance.** Confirm current `tsify` (or `tsify-next`)
   version + wasm-bindgen 0.2 compatibility before committing.
5. **Does this land in Phase 2 or a follow-up?** Phase 2 could ship hand-typed
   and this converts later, OR Phase 2 adopts tsify from the start (avoids
   writing throwaway hand-types). Leaning: adopt in Phase 2.

## Notes / decisions so far

- Layout split is already what we want — no restructuring, just add the feature.
- Serde already on the core types — tsify rides existing derives, low cost.
- Hot-path caveat (general, not blocking): don't serde-marshal huge graphs every
  call; keep opaque handles for stateful objects, tsify only the small in/out
  DTOs. The solver's `solve()` is call-and-return with bounded payloads, so this
  is fine here.
