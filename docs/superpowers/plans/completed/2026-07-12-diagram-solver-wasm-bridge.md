# Diagram Solver WASM Bridge (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the Phase-1 Rust diagram solver over WASM via a new `solve()` entry, extract the Rust→JS bridge into a standalone `@uaml/wasm` package, and un-skip `Diagram.layout` serde.

**Architecture:** Un-skip `Diagram.layout` serde in `crates/uaml`; give the solver IO + diagnostic types **tsify-next** (`Tsify`) derives behind a new, wasm-only `wasm` feature so the TypeScript definitions are generated from the Rust structs (single source of truth) — the pure `default` build stays wasm-free; add a bundle-in `solve` core + `#[wasm_bindgen]` shell in `crates/uaml-wasm` returning a `Tsify` `SolveResult`; move the wasm glue/wrapper out of `@uaml/okf` into a new `@uaml/wasm` package and flip all consumers. No canvas integration — a parity vitest against the Rust golden numbers is the phase gate.

**Tech Stack:** Rust (serde, tsify-next, serde-wasm-bindgen, wasm-bindgen, wasm-bindgen-utils, wasm-pack), TypeScript, pnpm workspaces, vitest.

**Spec:** `docs/superpowers/specs/2026-07-12-diagram-solver-wasm-bridge-design.md`

## Global Constraints

- Rust features in `crates/uaml`:
  - `serde = ["dep:serde"]` — existing; plain serde derives are `#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]`.
  - `wasm = ["serde", "dep:tsify-next", "dep:wasm-bindgen", "dep:wasm-bindgen-utils"]` — NEW; adds tsify/wasm-bindgen ONLY for wasm builds. `tsify-next` uses `features = ["js"]` (serde-wasm-bindgen ABI, JS objects not JSON strings). The `default` feature set stays `[]` — a plain `cargo build -p uaml` pulls neither serde nor wasm-bindgen.
  - Tsify derive pattern: `#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]` + `#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]`, stacked ON TOP of the serde derives (Tsify requires `Serialize`/`Deserialize`).
- `crates/uaml-wasm` builds `uaml` with `features = ["wasm"]` (was `["serde"]`).
- Determinism: solver output uses `BTreeMap` — never reorder or switch to `HashMap`.
- Map ABI: `BTreeMap<String, _>` fields crossing the wasm boundary get `#[cfg_attr(feature = "wasm", tsify(type = "Record<string, T>"))]` + `#[cfg_attr(all(feature = "wasm", target_family = "wasm"), serde(serialize_with = "wasm_bindgen_utils::serialize_btreemap_as_object"))]` so JS receives a plain object, not a `Map`.
- No coordinate persistence: solved pixels are render-time only.
- No canvas changes: `packages/web/src/canvas/layout.ts` stays dagre-only this phase.
- Package naming: new package is `@uaml/wasm` at `packages/wasm/`.
- Commit style: Conventional Commits. Do NOT add a Claude/Co-Authored-By trailer.
- Test commands: `cargo test -p uaml`, `cargo test -p uaml-wasm`, `pnpm -r test`, `pnpm lint`, `pnpm build`.

---

### Task 1: Un-skip `Diagram.layout` + serde on the 12 layout-AST types

**Files:**
- Modify: `crates/uaml/src/syntax.rs` (add serde derives to 12 types)
- Modify: `crates/uaml/src/model.rs:381-394` (drop `#[serde(skip)]` on `Diagram.layout`, fix comment)
- Test: `crates/uaml/tests/layout_serde_roundtrip.rs` (new)

**Interfaces:**
- Produces: `model::Diagram.layout` now serializes/deserializes; the layout AST (`LayoutStatement` and descendants) is serde-capable under the `serde` feature.

- [ ] **Step 1: Write the failing round-trip test**

Create `crates/uaml/tests/layout_serde_roundtrip.rs`:

```rust
//! Guards that Diagram.layout survives a serde round-trip now that it is no
//! longer `#[serde(skip)]`. Requires the `serde` feature (dev-deps enable it via
//! serde_json; the crate's own `serde` feature must be on for the derives).
#![cfg(feature = "serde")]

use uaml::parse::build_model;

fn bundle() -> Vec<(String, String)> {
    vec![
        ("shop/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ("shop/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
        (
            "shop/orders-domain.md".into(),
            "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n".into(),
        ),
    ]
}

#[test]
fn diagram_layout_survives_serde_roundtrip() {
    let model = build_model(&bundle());
    let diagram = &model.diagrams[0];
    assert!(!diagram.layout.is_empty(), "fixture must have layout statements");

    let json = serde_json::to_string(diagram).unwrap();
    let back: uaml::model::Diagram = serde_json::from_str(&json).unwrap();

    assert_eq!(back.layout, diagram.layout, "layout must round-trip byte-equal");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p uaml --features serde --test layout_serde_roundtrip`
Expected: FAIL — compile error (`LayoutStatement` does not implement `Serialize`) or, once `#[serde(skip)]` is still present, `back.layout` is empty and the `assert_eq!` fails.

- [ ] **Step 3: Add serde derives to the 12 layout-AST types in `syntax.rs`**

For EACH of these 12 type definitions in `crates/uaml/src/syntax.rs`, add the derive line directly above it if not already present:

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
```

The 12 types: `LayoutStatement`, `Operand`, `OperandRef`, `NameRef`, `Direction`, `Anchored`, `Edge`, `Axis`, `Hint`, `Shape`, `Margin`, `Flag`.

Notes:
- `OperandRef::Paren(Box<Operand>)` — `Box<T>` serializes transparently; no extra work.
- Do NOT add serde to `Line`, `LayoutItem`, `ErrorNode`, `Section`, `Document`, `MembersBlock` — they are not in `Diagram.layout`'s subtree.
- If any of the 12 already carries the `cfg_attr` derive, leave it.

- [ ] **Step 4: Un-skip `Diagram.layout` in `model.rs`**

In `crates/uaml/src/model.rs`, the `Diagram` struct currently has:

```rust
    // `layout` carries the raw layout AST (`syntax::LayoutStatement`), which the
    // frontend does not yet consume (dagre handles placement). Skipped from the
    // wire until layout-in-Rust (Stage 2), so the whole `syntax` layout tree need
    // not implement serde.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub layout: Vec<crate::syntax::LayoutStatement>,
```

Replace with (drop the `skip`, update the comment):

```rust
    // `layout` carries the raw layout AST (`syntax::LayoutStatement`). Serialized
    // end to end (Phase 2) so the frontend can read the layout relations.
    pub layout: Vec<crate::syntax::LayoutStatement>,
```

- [ ] **Step 5: Run the round-trip test — verify it passes**

Run: `cargo test -p uaml --features serde --test layout_serde_roundtrip`
Expected: PASS.

- [ ] **Step 6: Run the full uaml suite — verify nothing regressed**

Run: `cargo test -p uaml --features serde && cargo test -p uaml`
Expected: PASS (both with and without the feature; the crate must still compile serde-off).

- [ ] **Step 7: Commit**

```bash
git add crates/uaml/src/syntax.rs crates/uaml/src/model.rs crates/uaml/tests/layout_serde_roundtrip.rs
git commit -m "feat(uaml): serialize Diagram.layout — serde on the layout AST"
```

---

### Task 2: `wasm` feature + Tsify/serde on the solver IO + diagnostic types

**Files:**
- Modify: `crates/uaml/Cargo.toml` (add `wasm` feature + optional deps)
- Modify: `crates/uaml/src/solve/mod.rs` (serde + Tsify derives on IO types; map ABI attrs)
- Modify: `crates/uaml/src/diagnostic.rs` (serde + Tsify derives on `Diagnostic`/`DiagCode`/`Severity`)
- Test: add a serde unit test inside `crates/uaml/src/solve/mod.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `Shape` serde from Task 1 (used by `SolvedGroup.shape`; `Shape` already got serde in Task 1 — Tsify for `Shape` is added here too).
- Produces: under the `wasm` feature, `Solved`, `SolvedGroup`, `Rect`, `FlagSet`, `Size`, `SolveConfig`, `Diagnostic`, `DiagCode`, `Severity` (and `Shape`) all derive `Tsify` with `into_wasm_abi`/`from_wasm_abi` and emit TypeScript definitions. Under `serde` alone they are `Serialize`/`Deserialize`. These are the wire types.

- [ ] **Step 1: Add the `wasm` feature + optional deps to `crates/uaml/Cargo.toml`**

Run (resolves current versions; keeps them optional):

```bash
cargo add --package uaml --optional wasm-bindgen
cargo add --package uaml --optional wasm-bindgen-utils
cargo add --package uaml --optional tsify-next --features js
```

Then edit the `[features]` table so it reads:

```toml
[features]
default = []
serde = ["dep:serde"]
wasm = ["serde", "dep:tsify-next", "dep:wasm-bindgen", "dep:wasm-bindgen-utils"]
```

Verify `[dependencies]` now has `tsify-next` with `features = ["js"]` and `optional = true`; if `cargo add` did not set the feature, edit it to:

```toml
tsify-next = { version = "0.5", features = ["js"], optional = true }
```

- [ ] **Step 2: Write the failing serde round-trip test**

Add to the `#[cfg(test)] mod tests` block in `crates/uaml/src/solve/mod.rs` (this exercises the plain-serde path, which the `wasm` feature also relies on):

```rust
    #[test]
    #[cfg(feature = "serde")]
    fn solve_io_types_serde_roundtrip() {
        // Inputs deserialize from a JS-shaped object.
        let cfg: SolveConfig =
            serde_json::from_str(r#"{"margin_px":[0,8,16,32],"chip":{"w":96,"h":28}}"#).unwrap();
        assert_eq!(cfg, SolveConfig::default());

        let sizes: SizeMap = serde_json::from_str(r#"{"a":{"w":200,"h":90}}"#).unwrap();
        assert_eq!(sizes["a"], Size { w: 200.0, h: 90.0 });

        // Output serializes with maps as JSON objects (serde_json default).
        let mut nodes = BTreeMap::new();
        nodes.insert("a".to_string(), Rect { x: 1.0, y: 2.0, w: 3.0, h: 4.0 });
        let solved = Solved { nodes, groups: vec![], flags: BTreeMap::new() };
        let v: serde_json::Value = serde_json::to_value(&solved).unwrap();
        assert_eq!(v["nodes"]["a"]["x"], 1.0);
        assert_eq!(v["nodes"]["a"]["w"], 3.0);
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p uaml --features serde solve_io_types_serde_roundtrip`
Expected: FAIL — compile error (`SolveConfig`/`Solved` do not implement `Deserialize`/`Serialize`).

- [ ] **Step 4: Add serde + Tsify derives to the solver IO types**

In `crates/uaml/src/solve/mod.rs`, above each of `Size`, `Rect`, `SolveConfig`, `FlagSet`, `SolvedGroup`, `Solved`, stack these two derive lines (below the existing `#[derive(Debug, ...)]`):

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
```

Then annotate the two `BTreeMap` fields on `Solved` so they cross as JS objects. `Solved` becomes:

```rust
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Solved {
    #[cfg_attr(feature = "wasm", tsify(type = "Record<string, Rect>"))]
    #[cfg_attr(
        all(feature = "wasm", target_family = "wasm"),
        serde(serialize_with = "wasm_bindgen_utils::serialize_btreemap_as_object")
    )]
    pub nodes: BTreeMap<String, Rect>,
    pub groups: Vec<SolvedGroup>,
    #[cfg_attr(feature = "wasm", tsify(type = "Record<string, FlagSet>"))]
    #[cfg_attr(
        all(feature = "wasm", target_family = "wasm"),
        serde(serialize_with = "wasm_bindgen_utils::serialize_btreemap_as_object")
    )]
    pub flags: BTreeMap<String, FlagSet>,
}
```

Do NOT add any derives to `BoxId`, `BoxKind`, `Box`, `Constraint`, `Scene` — internal IR, never on the wire.

- [ ] **Step 5: Add serde + Tsify derives to the diagnostic types**

In `crates/uaml/src/diagnostic.rs`, add the Tsify lines to `Severity`, `DiagCode`, and `Diagnostic` (they already carry the serde `cfg_attr`). For each, the attribute stack becomes:

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
```

Keep the existing `#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]` on `Severity` and `#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]` on `DiagCode`. `Diagnostic.span: Option<(usize, usize)>` needs no extra attribute — Tsify types a tuple as `[number, number]`.

- [ ] **Step 6: Add the same Tsify stack to `Shape` in `syntax.rs`**

`Shape` got the serde `cfg_attr` in Task 1. Add the two Tsify lines above it as well (it is a field type of `SolvedGroup`, so it must cross the boundary):

```rust
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
```

(The other 11 layout-AST types from Task 1 do NOT need Tsify — they are not returned by `solve`; `Diagram.layout` rides `build_model`'s plain-serde path.)

- [ ] **Step 7: Run the serde test — verify it passes**

Run: `cargo test -p uaml --features serde solve_io_types_serde_roundtrip`
Expected: PASS.

- [ ] **Step 8: Verify all three build modes compile**

```bash
cargo build -p uaml                    # default: pure, no serde, no wasm-bindgen
cargo build -p uaml --features serde   # serde only
cargo build -p uaml --features wasm    # tsify + wasm-bindgen (compiles on host target)
cargo test -p uaml --features serde && cargo test -p uaml
```
Expected: all PASS. The `--features wasm` build proves the Tsify derives + map attrs are well-formed.

- [ ] **Step 9: Commit**

```bash
git add crates/uaml/Cargo.toml crates/uaml/src/solve/mod.rs crates/uaml/src/diagnostic.rs crates/uaml/src/syntax.rs Cargo.lock
git commit -m "feat(uaml): wasm feature — Tsify/serde on solver IO + diagnostic types"
```

---

### Task 3: `solve` core + `#[wasm_bindgen]` shell in `crates/uaml-wasm`

**Files:**
- Modify: `crates/uaml-wasm/src/lib.rs` (add `SolveResult`, `solve_bundle` core, `solve` shell)
- Modify: `crates/uaml-wasm/Cargo.toml` (add `serde` dep for the `SolveResult` derive + `Serializer` use)
- Test: `crates/uaml-wasm/tests/native.rs` (append solve tests)

**Interfaces:**
- Consumes: `uaml::solve::{solve_diagram, Solved, Size, SizeMap, SolveConfig, Rect}`, `uaml::parse::build_model`, `uaml::diagnostic::Diagnostic`.
- Produces:
  - `pub fn solve_bundle(bundle: &[(String, String)], diagram_key: &str, sizes: SizeMap, cfg: SolveConfig) -> Result<SolveResult, String>`
  - `pub struct SolveResult { pub solved: Solved, pub diagnostics: Vec<Diagnostic> }`
  - `#[wasm_bindgen] pub fn solve(bundle: JsValue, diagram_key: String, sizes: JsValue, cfg: JsValue) -> Result<JsValue, JsValue>`

- [ ] **Step 1: Point `crates/uaml-wasm/Cargo.toml` at the `wasm` feature + add tsify**

Change the `uaml` dependency line from `features = ["serde"]` to `features = ["wasm"]`:

```toml
uaml = { path = "../uaml", features = ["wasm"] }
```

Add tsify + serde so `SolveResult` (defined here) can derive `Tsify`:

```bash
cargo add --package uaml-wasm tsify-next --features js
cargo add --package uaml-wasm serde --features derive
```

(`uaml-wasm` is always a wasm crate, so these are non-optional here.)

- [ ] **Step 2: Write the failing native tests**

Append to `crates/uaml-wasm/tests/native.rs`. First, extend the top `use` line to include `solve_bundle`:

```rust
use uaml_wasm::{apply_ops_bundle, build_model_json, fmt_bundle, solve_bundle, validate_json};
use std::collections::BTreeMap;
use uaml::solve::{Rect, Size, SizeMap, SolveConfig};
```

Then add:

```rust
fn layout_bundle() -> Vec<(String, String)> {
    vec![
        ("shop/customer.md".into(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into()),
        ("shop/account.md".into(), "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n".into()),
        ("shop/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
        (
            "shop/orders-domain.md".into(),
            "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n".into(),
        ),
    ]
}

fn sizes_200x90() -> SizeMap {
    let mut s: SizeMap = BTreeMap::new();
    for k in ["customer", "account", "order"] {
        s.insert(k.into(), Size { w: 200.0, h: 90.0 });
    }
    s
}

#[test]
fn solve_bundle_matches_golden_rects() {
    let r = solve_bundle(&layout_bundle(), "orders", sizes_200x90(), SolveConfig::default()).unwrap();
    assert!(r.diagnostics.is_empty(), "expected no diagnostics, got: {:?}", r.diagnostics);
    assert_eq!(r.solved.nodes["customer"], Rect { x: 16.0, y: 16.0, w: 200.0, h: 90.0 });
    assert_eq!(r.solved.nodes["account"], Rect { x: 16.0, y: 122.0, w: 200.0, h: 90.0 });
    assert_eq!(r.solved.nodes["order"], Rect { x: 264.0, y: 69.0, w: 200.0, h: 90.0 });
    // Two groups: framed "Users" and shrink "Orders".
    assert_eq!(r.solved.groups.len(), 2);
}

#[test]
fn solve_bundle_unknown_key_errs() {
    let err = solve_bundle(&layout_bundle(), "nope", sizes_200x90(), SolveConfig::default()).unwrap_err();
    assert!(err.contains("nope"), "error should name the missing key, got: {err}");
}

#[test]
fn solve_bundle_surfaces_unresolved_operand_diagnostic() {
    let mut b = layout_bundle();
    // Append a layout line referencing a non-existent operand.
    let diagram = b.last_mut().unwrap();
    diagram.1.push_str("- Ghosts left of Orders\n");
    let r = solve_bundle(&b, "orders", sizes_200x90(), SolveConfig::default()).unwrap();
    assert!(
        r.diagnostics.iter().any(|d| d.code == uaml::diagnostic::DiagCode::UnresolvedLayoutRef),
        "expected an unresolved-layout-ref diagnostic, got: {:?}", r.diagnostics
    );
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p uaml-wasm --test native solve_bundle`
Expected: FAIL — `solve_bundle` not found.

- [ ] **Step 4: Implement `SolveResult`, `solve_bundle`, and the `solve` shell**

In `crates/uaml-wasm/src/lib.rs`, add the core (in the "Pure, natively-testable cores" section):

```rust
/// Result of solving one diagram: absolute rects + any layout diagnostics.
/// Tsify emits its TypeScript type; under `wasm` it crosses the boundary as a
/// plain JS object.
#[derive(serde::Serialize, serde::Deserialize, tsify_next::Tsify)]
#[tsify(into_wasm_abi)]
pub struct SolveResult {
    pub solved: uaml::solve::Solved,
    pub diagnostics: Vec<uaml::diagnostic::Diagnostic>,
}

/// Build the model from `bundle`, pick the diagram whose `key == diagram_key`,
/// and solve it with the caller-supplied `sizes` + `cfg`. Errors if no diagram
/// matches the key (a caller bug, distinct from in-diagram graceful degradation).
pub fn solve_bundle(
    bundle: &[(String, String)],
    diagram_key: &str,
    sizes: uaml::solve::SizeMap,
    cfg: uaml::solve::SolveConfig,
) -> Result<SolveResult, String> {
    let model = uaml::parse::build_model(bundle);
    let diagram = model
        .diagrams
        .iter()
        .find(|d| d.key == diagram_key)
        .ok_or_else(|| format!("no diagram with key '{diagram_key}'"))?;
    let (solved, diagnostics) = uaml::solve::solve_diagram(diagram, &sizes, &cfg);
    Ok(SolveResult { solved, diagnostics })
}
```

Add the wasm shell (in the `#[wasm_bindgen]` surface section). The return type is the `Tsify` `SolveResult` — wasm-bindgen converts it to a plain JS object via `into_wasm_abi` (so `solved.nodes`/`solved.flags` are objects, per the map attrs from Task 2). Inputs stay `JsValue` and deserialize into the `uaml` types:

```rust
/// `bundle`: `[path, markdown][]`; `diagram_key`: which diagram to solve;
/// `sizes`: `Record<string, {w, h}>`; `cfg`: `SolveConfig | null | undefined`.
/// Returns `{ solved, diagnostics }`.
#[wasm_bindgen]
pub fn solve(
    bundle: JsValue,
    diagram_key: String,
    sizes: JsValue,
    cfg: JsValue,
) -> Result<SolveResult, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let sizes: uaml::solve::SizeMap = serde_wasm_bindgen::from_value(sizes)?;
    let cfg: uaml::solve::SolveConfig = if cfg.is_null() || cfg.is_undefined() {
        uaml::solve::SolveConfig::default()
    } else {
        serde_wasm_bindgen::from_value(cfg)?
    };
    solve_bundle(&b, &diagram_key, sizes, cfg).map_err(|e| JsValue::from_str(&e))
}
```

- [ ] **Step 5: Run tests — verify they pass**

Run: `cargo test -p uaml-wasm --test native solve_bundle`
Expected: PASS (all three solve tests).

- [ ] **Step 6: Run the full uaml-wasm suite + a wasm-target typecheck**

Run: `cargo test -p uaml-wasm`
Expected: PASS.

Then verify it still compiles to wasm:
Run: `cargo build -p uaml-wasm --target wasm32-unknown-unknown`
Expected: builds clean (if the target is not installed, note it and rely on the `build:wasm` run in Task 4 to catch wasm-specific issues).

- [ ] **Step 7: Commit**

```bash
git add crates/uaml-wasm/Cargo.toml crates/uaml-wasm/src/lib.rs crates/uaml-wasm/tests/native.rs Cargo.lock
git commit -m "feat(uaml-wasm): solve() entry — bundle-in, returns Tsify {solved, diagnostics}"
```

---

### Task 4: Create `@uaml/wasm` package; move the bridge out of `@uaml/okf`

**Files:**
- Create: `packages/wasm/package.json`
- Create: `packages/wasm/tsconfig.json`
- Create: `packages/wasm/scripts/copy-wasm-glue.mjs`
- Create: `packages/wasm/src/index.ts` (moved wrapper + new `solve` wrapper + types)
- Modify: `scripts/build-wasm.mjs` (retarget `outDir`)
- Regenerate: `packages/wasm/src/generated/*` (via `build:wasm`)
- Delete: `packages/okf/src/wasm/index.ts`, `packages/okf/src/generated/*`, `packages/okf/scripts/copy-wasm-glue.mjs`
- Modify: `packages/okf/src/index.ts` (drop wasm re-export), `packages/okf/package.json` (drop copy-glue build step)
- Modify: root `package.json` (build order)

**Interfaces:**
- Consumes: the `solve` wasm export from Task 3.
- Produces: `@uaml/wasm` exporting `initWasm, apply_ops, build_model, fmt, split_bundle, validate, solve` plus the TS types `Size, Rect, FlagSet, SolvedGroup, Solved, SolveConfig, Diagnostic, SolveResult`.

- [ ] **Step 1: Retarget the wasm build output**

In `scripts/build-wasm.mjs`, change the `outDir` line:

```js
const outDir = join(root, "packages", "okf", "src", "generated");
```

to:

```js
const outDir = join(root, "packages", "wasm", "src", "generated");
```

- [ ] **Step 2: Scaffold the package manifest + tsconfig + copy-glue script**

Create `packages/wasm/package.json`:

```json
{
  "name": "@uaml/wasm",
  "version": "0.0.0",
  "type": "module",
  "license": "Apache-2.0",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "scripts": {
    "build": "tsc -p tsconfig.json && node scripts/copy-wasm-glue.mjs",
    "test": "vitest run --passWithNoTests"
  },
  "devDependencies": {
    "vitest": "^2.1.0",
    "typescript": "^5.6.0"
  }
}
```

Create `packages/wasm/tsconfig.json`:

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "outDir": "dist"
  },
  "include": ["src"]
}
```

Create `packages/wasm/scripts/copy-wasm-glue.mjs` (same as okf's, comment updated):

```js
// tsc compiles src/generated/wasm-inline.ts but does NOT copy the plain-JS
// wasm-bindgen glue (uaml_wasm.js) or its .d.ts. Copy them into dist/generated so
// the published package is self-contained for cross-package imports (core/web
// build + tests resolve @uaml/wasm → dist and follow dist/index → dist/generated).
import { copyFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const pkg = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(pkg, "src", "generated");
const dist = join(pkg, "dist", "generated");

mkdirSync(dist, { recursive: true });
for (const f of ["uaml_wasm.js", "uaml_wasm.d.ts"]) {
  copyFileSync(join(src, f), join(dist, f));
}
```

- [ ] **Step 3: Regenerate the wasm glue into the new package**

Run: `pnpm build:wasm`
Expected: prints `wrote .../packages/wasm/src/generated/wasm-inline.ts (... KB base64)` and creates `packages/wasm/src/generated/{uaml_wasm.js,uaml_wasm.d.ts,wasm-inline.ts}`. Confirm `uaml_wasm.d.ts` now contains a `solve` declaration.

- [ ] **Step 4: Write `packages/wasm/src/index.ts`**

The solver types are now **generated** by Tsify into `src/generated/uaml_wasm.d.ts` (`Size`, `Rect`, `FlagSet`, `SolvedGroup`, `Solved`, `SolveConfig`, `Diagnostic`, `DiagCode`, `Severity`, `SolveResult`) along with a typed `solve(...)`. The wrapper re-exports those types and adds a small `initWasm` + typed `solve` convenience over the raw `solve`:

```ts
// Frontend-facing entry to the Rust UAML core, compiled to wasm and inlined
// (no runtime .wasm fetch). Call `initWasm()` once before any other export.
import init, {
  apply_ops,
  build_model,
  fmt,
  init_panic_hook,
  solve as solveRaw,
  split_bundle,
  validate,
} from "./generated/uaml_wasm.js";
import { wasmBytes } from "./generated/wasm-inline";

// Types generated from the Rust structs by Tsify (single source of truth).
export type {
  Size,
  Rect,
  FlagSet,
  SolvedGroup,
  Solved,
  SolveConfig,
  Diagnostic,
  DiagCode,
  Severity,
  SolveResult,
} from "./generated/uaml_wasm.js";

import type { Size, SolveConfig, SolveResult } from "./generated/uaml_wasm.js";

let ready: Promise<void> | undefined;

/** Instantiate the inlined wasm exactly once. Safe to await repeatedly. */
export function initWasm(): Promise<void> {
  if (!ready) {
    ready = init({ module_or_path: wasmBytes() }).then(() => {
      init_panic_hook();
    });
  }
  return ready;
}

export { apply_ops, build_model, fmt, split_bundle, validate };

/**
 * Solve one diagram's layout. `bundle` is the OKF bundle, `diagramKey` the
 * `Diagram.key`, `sizes` maps node key → intrinsic size, `cfg` is optional.
 * Throws if `diagramKey` matches no diagram.
 */
export function solve(
  bundle: [string, string][],
  diagramKey: string,
  sizes: Record<string, Size>,
  cfg?: SolveConfig,
): SolveResult {
  return solveRaw(bundle, diagramKey, sizes, cfg ?? undefined);
}
```

Note: confirm the generated `uaml_wasm.d.ts` exports exactly these type names and that `solve` is typed `(bundle: [string, string][], diagram_key: string, sizes: any, cfg: any) => SolveResult` (wasm-bindgen types the two `JsValue` params as `any`). If any generated name differs (e.g. Tsify emits `SolveResult` under a different casing), align the re-export list to the actual `.d.ts`. Do NOT hand-write these interfaces — they come from Tsify.

- [ ] **Step 5: Strip the wasm bridge from `@uaml/okf`**

Delete these paths:
- `packages/okf/src/wasm/` (the whole dir — `index.ts`)
- `packages/okf/src/generated/` (the whole dir)
- `packages/okf/scripts/copy-wasm-glue.mjs`

```bash
git rm -r packages/okf/src/wasm packages/okf/src/generated packages/okf/scripts/copy-wasm-glue.mjs
```

In `packages/okf/src/index.ts`, delete the wasm re-export block (the final `export { initWasm, apply_ops, build_model, fmt, split_bundle, validate } from "./wasm/index";` and its comment). Keep the `types`/`slug`/`grammar` exports.

In `packages/okf/package.json`, change the build script from:

```json
    "build": "tsc -p tsconfig.json && node scripts/copy-wasm-glue.mjs",
```

to:

```json
    "build": "tsc -p tsconfig.json",
```

- [ ] **Step 6: Set the root build order**

In root `package.json`, change the `build` script from:

```json
    "build": "pnpm --filter @uaml/okf build && pnpm --filter @uaml/core build && pnpm --filter @uaml/web build",
```

to (build `@uaml/wasm` first):

```json
    "build": "pnpm --filter @uaml/wasm build && pnpm --filter @uaml/okf build && pnpm --filter @uaml/core build && pnpm --filter @uaml/web build",
```

- [ ] **Step 7: Build the new package in isolation**

Run: `pnpm install` (links the new workspace package), then `pnpm --filter @uaml/wasm build`
Expected: `dist/index.js`, `dist/index.d.ts`, and `dist/generated/*` produced with no TS errors.

- [ ] **Step 8: Commit**

```bash
git add packages/wasm scripts/build-wasm.mjs packages/okf/src/index.ts packages/okf/package.json package.json
git add -A packages/okf
git commit -m "refactor(wasm): extract the WASM bridge into @uaml/wasm; okf stops hosting wasm"
```

---

### Task 5: Flip all consumers off `@uaml/okf` wasm onto `@uaml/wasm`

**Files (add `@uaml/wasm` dep):**
- Modify: `packages/core/package.json`, `packages/web/package.json`

**Files (flip imports — wasm symbols only: `initWasm, apply_ops, build_model, fmt, split_bundle, validate`):**
- `packages/core/src/share/url.ts`, `packages/core/src/share/url.test.ts`
- `packages/core/src/state/model.ts`, `packages/core/src/state/model.test.ts`
- `packages/core/src/state/ops-adapter.test.ts`
- `packages/core/src/templates/templates.test.ts`
- `packages/web/src/canvas/layout.test.ts`
- `packages/web/src/components/canvas/edges.test.ts`
- `packages/web/src/components/canvas/selection.test.ts`
- `packages/web/src/components/canvas/toRFNode.test.ts`
- `packages/web/src/components/ImportDialog.svelte`
- `packages/web/src/components/LibraryDialog.svelte`
- `packages/web/src/components/TemplateApplyDialog.svelte`
- `packages/web/src/components/TemplateApplyDialog.test.ts`
- `packages/web/src/components/WelcomeDialog.svelte`
- `packages/web/src/main.ts`
- `packages/web/src/state/bootstrap.test.ts`
- `packages/web/src/state/model.svelte.test.ts`
- `packages/web/src/test/setup.ts`

**Interfaces:**
- Consumes: `@uaml/wasm` from Task 4.
- Rule: move ONLY the six wasm symbols to `@uaml/wasm`. Any okf **types/consts** on the same import line (`DiagramDisplay`, `DEFAULT_DISPLAY`, `RELATIONSHIP_KINDS`, `resolveDisplay`, `splitType`, etc.) stay on a `@uaml/okf` import. `@uaml/core` and `@uaml/web` keep their `@uaml/okf` dependency.

- [ ] **Step 1: Add the workspace dependency**

In `packages/core/package.json` and `packages/web/package.json`, add to `dependencies` (alongside `"@uaml/okf": "workspace:*"`):

```json
    "@uaml/wasm": "workspace:*",
```

Then run: `pnpm install`

- [ ] **Step 2: Flip the pure-wasm import lines**

For files importing ONLY wasm symbols from okf, change `from "@uaml/okf"` → `from "@uaml/wasm"`. These are:

- `packages/core/src/share/url.ts` — `import { split_bundle } from "@uaml/wasm";`
- `packages/core/src/share/url.test.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/core/src/state/model.ts` — `import { build_model, apply_ops } from "@uaml/wasm";`
- `packages/core/src/state/model.test.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/core/src/state/ops-adapter.test.ts` — `import { initWasm, apply_ops, build_model } from "@uaml/wasm";`
- `packages/web/src/canvas/layout.test.ts` — split: `initWasm` from wasm, `DEFAULT_DISPLAY` from okf (see Step 3).
- `packages/web/src/components/canvas/selection.test.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/web/src/components/ImportDialog.svelte` — `import { build_model } from "@uaml/wasm";`
- `packages/web/src/components/LibraryDialog.svelte` — `import { build_model } from "@uaml/wasm";`
- `packages/web/src/components/TemplateApplyDialog.svelte` — `import { build_model } from "@uaml/wasm";`
- `packages/web/src/components/TemplateApplyDialog.test.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/web/src/components/WelcomeDialog.svelte` — `import { build_model } from "@uaml/wasm";`
- `packages/web/src/main.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/web/src/state/bootstrap.test.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/web/src/state/model.svelte.test.ts` — `import { initWasm } from "@uaml/wasm";`
- `packages/web/src/test/setup.ts` — `import { initWasm } from "@uaml/wasm";`

- [ ] **Step 3: Split the mixed import lines (wasm symbol + okf type/const)**

Three files mix a wasm symbol with an okf type/const on one line. Replace each single line with two import lines:

`packages/core/src/templates/templates.test.ts` — from:
```ts
import { initWasm, build_model, RELATIONSHIP_KINDS } from "@uaml/okf";
```
to:
```ts
import { initWasm, build_model } from "@uaml/wasm";
import { RELATIONSHIP_KINDS } from "@uaml/okf";
```

`packages/web/src/canvas/layout.test.ts` — from:
```ts
import { DEFAULT_DISPLAY, initWasm } from "@uaml/okf";
```
to:
```ts
import { initWasm } from "@uaml/wasm";
import { DEFAULT_DISPLAY } from "@uaml/okf";
```

`packages/web/src/components/canvas/edges.test.ts` — from:
```ts
import { DEFAULT_DISPLAY, initWasm, type DiagramDisplay } from "@uaml/okf";
```
to:
```ts
import { initWasm } from "@uaml/wasm";
import { DEFAULT_DISPLAY, type DiagramDisplay } from "@uaml/okf";
```

`packages/web/src/components/canvas/toRFNode.test.ts` — from:
```ts
import { DEFAULT_DISPLAY, initWasm } from "@uaml/okf";
```
to:
```ts
import { initWasm } from "@uaml/wasm";
import { DEFAULT_DISPLAY } from "@uaml/okf";
```

- [ ] **Step 4: Verify no wasm symbol is still imported from okf**

Run: `grep -rEn "import[^;]*\b(initWasm|apply_ops|build_model|fmt|split_bundle|validate)\b[^;]*from \"@uaml/okf\"" packages --include=*.ts --include=*.svelte | grep -v dist/`
Expected: NO output (empty). Every wasm symbol now comes from `@uaml/wasm`.

- [ ] **Step 5: Full workspace gate**

Run: `pnpm -r test && pnpm lint && pnpm build`
Expected: PASS. All existing suites green, lint clean, full build succeeds with the new package + import graph.

- [ ] **Step 6: Commit**

```bash
git add packages/core packages/web
git commit -m "refactor(web,core): import wasm entry points from @uaml/wasm"
```

---

### Task 6: `solve` parity vitest in `@uaml/wasm`

**Files:**
- Test: `packages/wasm/src/solve.test.ts` (new)

**Interfaces:**
- Consumes: `initWasm`, `solve` from `@uaml/wasm` (this package's own `src/index.ts`).

- [ ] **Step 1: Write the parity test**

Create `packages/wasm/src/solve.test.ts`:

```ts
import { beforeAll, describe, expect, it } from "vitest";
import { initWasm, solve } from "./index";

const bundle: [string, string][] = [
  ["shop/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"],
  ["shop/account.md", "---\ntype: uml.Class\ntitle: Account\n---\n# Account\n"],
  ["shop/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
  [
    "shop/orders-domain.md",
    "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Members\n\n### Users\n- [Customer](./customer.md)\n- [Account](./account.md)\n\n### Orders\n- [Order](./order.md)\n\n## Layout\n- Users as column with frame\n- Users left of Orders\n",
  ],
];

const sizes = {
  customer: { w: 200, h: 90 },
  account: { w: 200, h: 90 },
  order: { w: 200, h: 90 },
};

describe("solve() over wasm", () => {
  beforeAll(async () => {
    await initWasm();
  });

  it("returns the golden rects as plain objects", () => {
    const { solved, diagnostics } = solve(bundle, "orders", sizes);
    expect(diagnostics).toEqual([]);
    // Plain object, not a Map.
    expect(solved.nodes.customer).toEqual({ x: 16, y: 16, w: 200, h: 90 });
    expect(solved.nodes.account).toEqual({ x: 16, y: 122, w: 200, h: 90 });
    expect(solved.nodes.order).toEqual({ x: 264, y: 69, w: 200, h: 90 });
    expect(solved.groups).toHaveLength(2);
    // The framed "Users" group renders with a title.
    expect(solved.groups.some((g) => g.title === "Users")).toBe(true);
  });

  it("throws when the diagram key is unknown", () => {
    expect(() => solve(bundle, "nope", sizes)).toThrow(/nope/);
  });

  it("surfaces an unresolved-operand diagnostic", () => {
    const bad = bundle.map(
      ([p, t]) =>
        (p === "shop/orders-domain.md"
          ? [p, t + "- Ghosts left of Orders\n"]
          : [p, t]) as [string, string],
    );
    const { diagnostics } = solve(bad, "orders", sizes);
    expect(diagnostics.some((d) => d.code === "unresolved-layout-ref")).toBe(true);
  });
});
```

- [ ] **Step 2: Run the parity test**

Run: `pnpm --filter @uaml/wasm test`
Expected: PASS. If `solved.groups[*].shape` is asserted anywhere and fails on case, align the TS `SolvedGroup.shape` union in `src/index.ts` to the actual string and re-run.

- [ ] **Step 3: Final full gate**

Run: `pnpm -r test && pnpm lint && pnpm build && cargo test -p uaml && cargo test -p uaml-wasm`
Expected: PASS across the board.

- [ ] **Step 4: Commit**

```bash
git add packages/wasm/src/solve.test.ts
git commit -m "test(wasm): solve() parity vs Rust golden layout"
```

---

## Notes for the implementer

- **The golden numbers** (`customer @ 16,16`, `account @ 16,122`, `order @ 264,69`, groups `232x228` / `232x122`) come from `crates/uaml/tests/solver_golden.rs`. The bundle in Tasks 3 and 6 is the markdown equivalent of that hand-built diagram: `### Users` (Customer, Account) + `### Orders` (Order) groups, `Users as column with frame`, `Users left of Orders`, all nodes 200×90. If the solver's numbers ever change, update the golden first, then these two tests to match.
- **Tsify owns the TS types.** `Solved`/`Rect`/`Diagnostic`/… TypeScript is generated from the Rust structs into `uaml_wasm.d.ts`; never hand-write or duplicate them in `index.ts`. The `#[tsify(type = "Record<string, T>")]` + `serialize_btreemap_as_object` pair (Task 2) is what makes `nodes`/`flags` arrive as JS objects, not `Map`s — the other wasm exports (`build_model`, etc.) are unaffected and keep their existing `serde_wasm_bindgen::to_value` path.
- **`wasm` is a wasm-only feature.** The pure `cargo build -p uaml` (default features) must stay serde-free and wasm-bindgen-free. Only `crates/uaml-wasm` (and explicit `--features wasm`) turns it on. If a native `cargo test -p uaml-wasm` fails to compile a Tsify `into_wasm_abi` impl, that is the signal to double-check the tsify-next version resolved by `cargo add`.
- **okf keeps its type exports** (`Diagram`, `ModelGraph`, `DiagramDisplay`, …). Only the six wasm function symbols moved. `@uaml/core` and `@uaml/web` still depend on `@uaml/okf`.
- **Diagram key** is the diagram doc's slug (`"orders"` for title "Orders"). If `solve_bundle(..., "orders", ...)` errors with "no diagram", assert `model.diagrams[0].key` in a scratch test to find the real slug.
```
