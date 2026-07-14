# Expose OKF tier — additive wasm + TS surface (CI-safe)

> **Rigor:** tdd-per-task

## Context

The OKF foundation (`okf::Concept`/`Bundle`, `okf::project`, `okf::build_bundle`) already landed on origin/main (commit `2f38282`), but it is only reachable from Rust — nothing exposes it through the wasm surface or the TS wire, so no downstream consumer can read a `Bundle`.

This plan does ONLY the **additive** part of the wire work: expose `build_bundle` through wasm and add the OKF types to the TS package. It **does NOT** touch the existing `model::Node`/`build_model` shape, node keys, or the `uml.*` token. Nothing existing changes, so `packages/web` + `packages/core` keep compiling and passing — CI stays green.

The **breaking** units (`uml.*`→`uaml.*` rename, Node-wraps-Concept, full-path keying, and the destructive `types.ts` regen that drops legacy fields) are explicitly OUT OF SCOPE here — they wait for the front-end (web-svelte) rewrite track that can absorb the wire change.

### Gate (full — this slice must keep everything green)

`cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. Because the work is additive, the FULL gate (web + core included) must stay green; that is the proof nothing broke.

### Gotchas

- **serde is not a default cargo feature** — `cargo test --workspace` (not `-p uaml`) turns it on via `uaml-wasm`; always gate with `--workspace`.
- **The wasm blob is prebuilt.** `packages/okf/src/generated/wasm-inline.ts` is a committed artifact rebuilt only by `node scripts/build-wasm.mjs` (root `build:wasm`). Task 2 must regenerate it so the new `build_bundle` export is actually callable from TS — an additive change to the blob (adds an export, removes nothing).
- **RTK proxy mangles large tool outputs** — set `$env:RTK_DISABLE=1` for gate commands; read files in <200-line ranges.

---

### Task 1: Expose `build_bundle` through the wasm crate (Rust-only, additive)

- `crates/uaml-wasm/src/lib.rs`: add a `build_bundle` wasm-bindgen entry mirroring the existing `build_model` — take the raw bundle input, call `uaml::okf::build_bundle`, serialize the `okf::Bundle` to the wire. Add a native `build_bundle_json` mirroring `build_model_json` for native tests. Do NOT modify `build_model`/`build_model_json` or any existing export.
- Test (`crates/uaml-wasm/tests/native.rs`, additive): feed a mixed bundle — a `uaml.Class` doc plus a non-UML `Playbook` doc carrying `tags`/`resource`/`timestamp`/citations/links — through `build_bundle_json`; assert every `Concept` field survives (the headline OKF round-trip guarantee) and the existing `build_model_json` output is unchanged.
- Verify: `cargo test --workspace` green.

**Files:** crates/uaml-wasm/src/lib.rs, crates/uaml-wasm/tests/native.rs

### Task 2: Add OKF types to the TS wire + regenerate the wasm blob (additive)

- `packages/okf/src/types.ts`: ADD `Concept`, `Link`, `Citation`, `ConceptRole`, `Bundle` type declarations mirroring the Rust `okf` shapes. Do NOT modify or remove the existing `Node`/`Model`/related types — the current front-end must keep compiling.
- Expose `build_bundle` in the TS wrapper/entry additively (alongside the existing `build_model` wrapper) if there is one.
- Regenerate the wasm blob so the new export is live: run `node scripts/build-wasm.mjs`, refreshing `packages/okf/src/generated/wasm-inline.ts` and the glue (additive — adds the `build_bundle` export, removes nothing).
- Verify: full gate green — `pnpm -r test && pnpm lint && pnpm build` all pass with `packages/web` + `packages/core` untouched and compiling.

**Files:** packages/okf/src/types.ts, packages/okf/src/generated/wasm-inline.ts, packages/okf/src/index.ts (or the existing wasm wrapper), packages/okf/src/wasm/smoke.test.ts

## Verification

- Each task: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build` green before commit — the entire tree, including web/core, stays green (this is the additive guarantee).
- Headline: a non-UML `Playbook` doc → `build_bundle` (Rust + wasm) → every `Concept` field (`tags`/`resource`/`timestamp`/citations/links/body) survives.
- Existing `build_model` wire output is byte-unchanged (no consumer sees a difference).
