# UAML WASM ops-editing — Stage 1a: callable WASM core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a WASM build of `crates/uaml` that the frontend can call for
`build_model`, `validate`, `apply_ops`, `fmt`, and `split_bundle` — inlined (no
runtime fetch) and proven callable from a JS test. (Frontend rewire onto it is
Stage 1b, a separate plan.)

**Architecture:** Feature-gate `serde` on the core crate with renames pinned to the
TS `packages/okf/src/types.ts` field names. Move the existing `OpDto` op wire
contract into a shared `crates/uaml-ops-dto` crate. Add a `crates/uaml-wasm`
(`cdylib`+`rlib`) crate exposing thin `serde-wasm-bindgen` wrappers over the
existing `uaml` functions (`build_model`, `validate`, `ops::apply`,
`serialize`/`fmt`, `split_bundle`). A Node build script base64-inlines the `.wasm`.

**Tech Stack:** Rust (wasm-bindgen, serde, serde-wasm-bindgen, wasm-pack), Node
(build script), pnpm workspace, Vitest.

## Global Constraints

- Rust edition 2021, `rust-version = 1.80` (workspace `Cargo.toml`).
- `serde` on `crates/uaml` MUST be **optional + feature-gated** (`[features] serde`);
  the crate builds with zero serde by default (native CLI path unaffected).
- serde field names MUST match `packages/okf/src/types.ts` exactly (see Task 1).
- The core `uaml` crate MUST stay WASM-compatible: no filesystem / OS / threads in
  parse/serialize/validate/ops/model (already true — keep it true).
- No OWOX branding in any new output. `owox.*` frontmatter handling: read-only in
  this stage (do not rename fields yet — that lands with Stage 1b/3 to avoid
  breaking the live TS parser mid-stage).
- pnpm 10.12.4 (`packageManager`); Node 22 (`.node-version`).
- Commit after every green task.

---

### Task 1: serde feature + derives on core model/diagnostic types

**Files:**
- Modify: `crates/uaml/Cargo.toml` (add optional serde + feature)
- Modify: `crates/uaml/src/model.rs` (derives + renames)
- Modify: `crates/uaml/src/multiplicity.rs`, `crates/uaml/src/diagnostic.rs`
- Test: `crates/uaml/tests/serde_shape.rs` (new)

**Interfaces:**
- Produces: `uaml` compiles with `--features serde`; `Model`, `Diagnostic` and all
  reachable types implement `serde::{Serialize, Deserialize}` under that feature,
  serializing to JSON that matches `packages/okf/src/types.ts` field names.

- [ ] **Step 1: Add the feature to `crates/uaml/Cargo.toml`**

```toml
[dependencies]
regex.workspace = true
pulldown-cmark.workspace = true
serde = { workspace = true, optional = true }

[features]
default = []
serde = ["dep:serde"]

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 2: Write the failing round-trip/shape test**

`crates/uaml/tests/serde_shape.rs`:

```rust
#![cfg(feature = "serde")]
use uaml::parse::build_model;

fn bundle() -> Vec<(String, String)> {
    vec![
        ("m/order.md".into(),
         "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId [1]\n\n## Relationships\n- composes [Line](./line.md): 1 to 1..*\n".into()),
        ("m/line.md".into(),
         "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n".into()),
    ]
}

#[test]
fn model_json_matches_ts_field_names() {
    let model = build_model(&bundle());
    let v = serde_json::to_value(&model).unwrap();
    let node = &v["nodes"][0];
    // TS ModelNode uses `type` and `key`, not `ty`.
    assert_eq!(node["type"], "uml.Class");
    assert_eq!(node["key"], "order");
    // Attribute.multiplicity is the canonical string form.
    assert_eq!(node["attributes"][0]["multiplicity"], "1");
    // TS ModelEdge uses `from`/`to`, kind lowercase string.
    let edge = &v["edges"][0];
    assert_eq!(edge["from"], "order");
    assert_eq!(edge["to"], "line");
    assert_eq!(edge["kind"], "composes");
}

#[test]
fn model_round_trips_through_json() {
    let model = build_model(&bundle());
    let json = serde_json::to_string(&model).unwrap();
    let back: uaml::model::Model = serde_json::from_str(&json).unwrap();
    assert_eq!(model, back);
}
```

- [ ] **Step 3: Run it, verify it fails to compile (no derives yet)**

Run: `cargo test -p uaml --features serde --test serde_shape`
Expected: FAIL — `Model` does not implement `Serialize`.

- [ ] **Step 4: Add gated derives + renames in `model.rs`**

For each struct/enum, add above the existing `#[derive(...)]`:

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
```

Field/variant renames (apply exactly):
- `Node`: `#[cfg_attr(feature="serde", serde(rename_all = "camelCase"))]` is NOT
  needed (fields already match) EXCEPT `ty` → `#[serde(rename = "type")]` and
  `abstract_` → `#[serde(rename = "abstract")]`.
- `Edge`: `source` → `#[serde(rename = "from")]`, `target` → `#[serde(rename = "to")]`.
- `TypeRef`: `ref_` → `#[serde(rename = "ref")]`.
- `RelationshipKind`: add `#[cfg_attr(feature="serde", serde(rename_all = "lowercase"))]`
  (unit enum → serializes as `"associates"`, `"composes"`, …).
- `Visibility`: represent as the marker string. Add:
  `#[cfg_attr(feature="serde", serde(into = "String", try_from = "String"))]`
  and impls `From<Visibility> for String` (via `marker()`), `TryFrom<String> for
  Visibility` (via `from_marker`, error on unknown).
- `ClassifierType`: flat string `"uml.Class"` / `"Diagram"` / opaque. Add
  `#[cfg_attr(feature="serde", serde(into = "String", try_from = "String"))]`
  with `From<ClassifierType> for String` (existing `as_str()`), `TryFrom<String>`
  (existing `parse`).
- `AssocName`: TS is `string | { ref: string }`. Add
  `#[cfg_attr(feature="serde", serde(untagged))]`; `Label(String)` → bare string,
  `Assoc(String)` → wrap in a `{ ref }` newtype variant shape via a helper (define a
  `#[serde] struct AssocRef { r#ref: String }` mapping, or a manual impl). Pin with
  a test asserting `Assoc("x")` → `{"ref":"x"}`.
- `NoteAnchor`: `#[serde(untagged)]` matching the three TS shapes in `types.ts:29-32`
  (`{targetKey}`, `{sourceKey,name}`, `{sourceKey,kind,targetKey}`); rename
  `target_key`→`targetKey`, `source_key`→`sourceKey`.
- `Member`: keep `key`, `position` (serializes as `[x,y]` array — acceptable; the TS
  overlay reads positions separately in Stage 1b).
- `Diagram`, `RenderHints`, `Attribute`, `RelEnd`, `UmlMetaclass`: derives only,
  field names already match (`RelEnd.navigable/role/multiplicity` ok).

- [ ] **Step 5: Gate `Multiplicity` in `multiplicity.rs`**

```rust
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "String", try_from = "String"))]
pub struct Multiplicity(String);
```

Add `From<Multiplicity> for String` and `TryFrom<String> for Multiplicity` (reuse
the existing `Multiplicity::parse`, error when it returns `None`).

- [ ] **Step 6: Gate `Diagnostic`, `Severity`, `DiagCode` in `diagnostic.rs`**

Add the gated derive to all three. `Severity` → `#[serde(rename_all = "lowercase")]`
(`"error"`/`"warning"`). `DiagCode` → serialize as its kebab slug via
`#[serde(into = "String")]` using the existing `as_str()` (Deserialize: `try_from`
mapping slug back, or derive if a slug round-trip fn exists; if not, a `TryFrom` that
matches `as_str()` values).

- [ ] **Step 7: Run the tests to green**

Run: `cargo test -p uaml --features serde --test serde_shape`
Expected: PASS (both tests).
Also: `cargo test -p uaml` (no features) still 161 pass; `cargo build` clean.

- [ ] **Step 8: Commit**

```bash
git add crates/uaml/Cargo.toml crates/uaml/src crates/uaml/tests/serde_shape.rs
git commit -m "feat(uaml): feature-gated serde with TS-matching JSON shape"
```

---

### Task 2: extract `OpDto` into a shared `uaml-ops-dto` crate

**Files:**
- Create: `crates/uaml-ops-dto/Cargo.toml`, `crates/uaml-ops-dto/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members`)
- Modify: `crates/uaml-cli/Cargo.toml` (depend on new crate), `crates/uaml-cli/src/ops_dto.rs`
  (become a re-export), `crates/uaml-cli/src/main.rs` (import path if needed)

**Interfaces:**
- Produces: `uaml_ops_dto::{OpDto, to_op, from_op}` — the `#[serde(tag="op")]` op wire
  enum, moved verbatim; `OpDto::to_op(&self) -> Result<uaml::ops::Op, _>` (or the
  existing conversion fn name) available to CLI and WASM.

- [ ] **Step 1: Create the crate `Cargo.toml`**

`crates/uaml-ops-dto/Cargo.toml`:

```toml
[package]
name = "uaml-ops-dto"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "Serde wire contract (OpDto) for UAML editing operations."

[dependencies]
uaml = { path = "../uaml" }
serde = { workspace = true }

[dev-dependencies]
serde_json = { workspace = true }
```

- [ ] **Step 2: Move the module**

Move the entire contents of `crates/uaml-cli/src/ops_dto.rs` (the `OpDto` enum, the
`one()` helper, all `to_op`/`from_op` conversions, and its `#[cfg(test)]` round-trip
tests) into `crates/uaml-ops-dto/src/lib.rs` verbatim. Keep the same public item
names.

- [ ] **Step 3: Re-export from the CLI to keep call sites working**

Replace `crates/uaml-cli/src/ops_dto.rs` contents with:

```rust
pub use uaml_ops_dto::*;
```

Add to `crates/uaml-cli/Cargo.toml` `[dependencies]`:

```toml
uaml-ops-dto = { path = "../uaml-ops-dto" }
```

- [ ] **Step 4: Register the crate in the workspace**

`Cargo.toml` (root): `members = ["crates/uaml", "crates/uaml-ops-dto", "crates/uaml-cli"]`

- [ ] **Step 5: Run tests to green**

Run: `cargo test -p uaml-ops-dto` (moved round-trip tests pass)
Run: `cargo test -p uaml-cli` (CLI still green via the re-export)
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml-ops-dto Cargo.toml crates/uaml-cli
git commit -m "refactor(ops): extract OpDto into shared uaml-ops-dto crate"
```

---

### Task 3: `uaml-wasm` crate — `build_model` + `validate` bindings

**Files:**
- Create: `crates/uaml-wasm/Cargo.toml`, `crates/uaml-wasm/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members`)
- Test: `crates/uaml-wasm/tests/native.rs` (new — native rlib tests, no browser)

**Interfaces:**
- Produces (wasm-bindgen exports): `build_model(bundle: JsValue) -> Result<JsValue,
  JsValue>` and `validate(bundle: JsValue) -> Result<JsValue, JsValue>`, where
  `bundle` is a `Record<string,string>` (path→markdown). Internally: bundle is
  received as `Vec<(String, String)>` via serde.

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "uaml-wasm"
version = "0.0.0"
edition.workspace = true
rust-version.workspace = true
license.workspace = true
description = "WASM bindings for the UAML core (parse/model/validate/ops)."

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
uaml = { path = "../uaml", features = ["serde"] }
uaml-ops-dto = { path = "../uaml-ops-dto" }
wasm-bindgen = "0.2"
serde-wasm-bindgen = "0.6"
serde = { workspace = true }
console_error_panic_hook = "0.1"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
```

Add `crates/uaml-wasm` to root `Cargo.toml` `members`.

- [ ] **Step 2: Write the failing native test**

`crates/uaml-wasm/tests/native.rs` (exercises the pure helper the binding wraps —
keep binding bodies one line so logic is testable natively):

```rust
use uaml_wasm::{build_model_json, validate_json};

fn b() -> Vec<(String, String)> {
    vec![("m/a.md".into(), "---\ntype: uml.Class\ntitle: A\n---\n# A\n".into())]
}

#[test]
fn build_model_returns_nodes() {
    let json = build_model_json(&b());
    assert!(json.contains("\"key\":\"a\""));
    assert!(json.contains("\"type\":\"uml.Class\""));
}

#[test]
fn validate_returns_array() {
    let json = validate_json(&b());
    assert!(json.starts_with('['));
}
```

- [ ] **Step 3: Run it, verify failure**

Run: `cargo test -p uaml-wasm --test native`
Expected: FAIL — `uaml_wasm` has no `build_model_json`.

- [ ] **Step 4: Implement `src/lib.rs`**

```rust
use wasm_bindgen::prelude::*;

// Pure, natively-testable cores.
pub fn build_model_json(bundle: &[(String, String)]) -> String {
    let model = uaml::parse::build_model(bundle);
    serde_json::to_string(&model).unwrap()
}

pub fn validate_json(bundle: &[(String, String)]) -> String {
    let diags = uaml::validate::validate(bundle);
    serde_json::to_string(&diags).unwrap()
}

// wasm-bindgen surface (serde-wasm-bindgen for structured JS objects).
#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn build_model(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let model = uaml::parse::build_model(&b);
    Ok(serde_wasm_bindgen::to_value(&model)?)
}

#[wasm_bindgen]
pub fn validate(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let diags = uaml::validate::validate(&b);
    Ok(serde_wasm_bindgen::to_value(&diags)?)
}
```

Add `serde_json` to `[dependencies]` (workspace) since the `*_json` helpers use it.

- [ ] **Step 5: Run tests to green**

Run: `cargo test -p uaml-wasm --test native`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/uaml-wasm Cargo.toml
git commit -m "feat(wasm): uaml-wasm crate with build_model + validate bindings"
```

---

### Task 4: `apply_ops` binding (bundle + OpDto[] → bundle)

**Files:**
- Modify: `crates/uaml-wasm/src/lib.rs`
- Test: `crates/uaml-wasm/tests/native.rs`

**Interfaces:**
- Produces: `apply_ops(bundle: JsValue, ops: JsValue) -> Result<JsValue, JsValue>` —
  `bundle` is `Record<string,string>`, `ops` is `OpDto[]`; returns the new bundle
  map. On op error returns a `JsValue` string. Native core: `apply_ops_bundle(bundle,
  ops_json: &str) -> Result<Vec<(String,String)>, String>`.

- [ ] **Step 1: Write the failing native test**

Append to `crates/uaml-wasm/tests/native.rs`:

```rust
#[test]
fn apply_ops_adds_attribute() {
    let bundle = vec![("m/a.md".into(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n".to_string())];
    let ops = r#"[{"op":"attr.add","node":"a","name":"id","ty":"AId"}]"#;
    let out = uaml_wasm::apply_ops_bundle(&bundle, ops).unwrap();
    let a = &out.iter().find(|(p, _)| p == "m/a.md").unwrap().1;
    assert!(a.contains("## Attributes"));
    assert!(a.contains("- id: AId"));
}
```

- [ ] **Step 2: Run it, verify failure**

Run: `cargo test -p uaml-wasm --test native apply_ops_adds_attribute`
Expected: FAIL — no `apply_ops_bundle`.

- [ ] **Step 3: Implement**

Add to `src/lib.rs`:

```rust
pub fn apply_ops_bundle(
    bundle: &[(String, String)],
    ops_json: &str,
) -> Result<Vec<(String, String)>, String> {
    let dtos: Vec<uaml_ops_dto::OpDto> =
        serde_json::from_str(ops_json).map_err(|e| e.to_string())?;
    let ops: Vec<uaml::ops::Op> = dtos
        .into_iter()
        .map(|d| d.to_op())          // use the actual conversion fn from ops_dto
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    uaml::ops::apply(bundle, &ops)
        .map_err(|e| format!("op {}: {}", e.index, e.reason))
}

#[wasm_bindgen]
pub fn apply_ops(bundle: JsValue, ops: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    let dtos: Vec<uaml_ops_dto::OpDto> = serde_wasm_bindgen::from_value(ops)?;
    let parsed: Vec<uaml::ops::Op> = dtos
        .into_iter()
        .map(|d| d.to_op())
        .collect::<Result<_, _>>()
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let out = uaml::ops::apply(&b, &parsed)
        .map_err(|e| JsValue::from_str(&format!("op {}: {}", e.index, e.reason)))?;
    Ok(serde_wasm_bindgen::to_value(&out)?)
}
```

(If the `OpDto`→`Op` conversion in `uaml-ops-dto` has a different name/signature than
`to_op()`, match it exactly — check `crates/uaml-ops-dto/src/lib.rs`.)

- [ ] **Step 4: Run tests to green**

Run: `cargo test -p uaml-wasm`
Expected: PASS (all native tests).

- [ ] **Step 5: Commit**

```bash
git add crates/uaml-wasm/src/lib.rs crates/uaml-wasm/tests/native.rs
git commit -m "feat(wasm): apply_ops binding over ops::apply"
```

---

### Task 5: `fmt` + `split_bundle` bindings

**Files:**
- Modify: `crates/uaml-wasm/src/lib.rs`, `crates/uaml-wasm/tests/native.rs`

**Interfaces:**
- Produces: `fmt(bundle) -> bundle` (canonical serialize per file via
  `serialize_document(parse_document(..))`), `split_bundle(text) -> bundle`
  (wraps `uaml::parse::split_bundle`).

- [ ] **Step 1: Failing test**

```rust
#[test]
fn fmt_is_idempotent() {
    let b = vec![("m/a.md".into(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n- id: AId\n".to_string())];
    let once = uaml_wasm::fmt_bundle(&b);
    let twice = uaml_wasm::fmt_bundle(&once);
    assert_eq!(once, twice);
}
```

- [ ] **Step 2: Run, verify fail.** `cargo test -p uaml-wasm fmt_is_idempotent` → FAIL.

- [ ] **Step 3: Implement**

```rust
pub fn fmt_bundle(bundle: &[(String, String)]) -> Vec<(String, String)> {
    bundle.iter().map(|(p, t)| {
        (p.clone(), uaml::serialize::serialize_document(&uaml::parse::parse_document(t)))
    }).collect()
}

#[wasm_bindgen]
pub fn fmt(bundle: JsValue) -> Result<JsValue, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    Ok(serde_wasm_bindgen::to_value(&fmt_bundle(&b))?)
}

#[wasm_bindgen]
pub fn split_bundle(text: &str) -> Result<JsValue, JsValue> {
    Ok(serde_wasm_bindgen::to_value(&uaml::parse::split_bundle(text))?)
}
```

- [ ] **Step 4: Run to green.** `cargo test -p uaml-wasm` → PASS.

- [ ] **Step 5: Commit**

```bash
git commit -am "feat(wasm): fmt + split_bundle bindings"
```

---

### Task 6: inline WASM build script

**Files:**
- Create: `scripts/build-wasm.mjs`
- Create: `crates/uaml-wasm/.gitignore` (ignore `pkg/`)
- Modify: root `package.json` (add a `build:wasm` script)

**Interfaces:**
- Produces: `packages/okf/src/generated/uaml_wasm.js` (wasm-bindgen glue),
  `uaml_wasm.d.ts`, and `wasm-inline.ts` (`export const WASM_BASE64`, `wasmBytes()`),
  regenerated idempotently. (Output dir is a script constant so the package-layout
  decision can move it later.)

- [ ] **Step 1: Verify the toolchain is available**

Run: `rustup target add wasm32-unknown-unknown && wasm-pack --version`
(If `wasm-pack` is missing: `cargo install wasm-pack`.)
Expected: prints a version.

- [ ] **Step 2: Write `scripts/build-wasm.mjs`**

```js
// Builds crates/uaml-wasm to WASM and emits an inlined base64 TS module.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const pkg = join(root, "crates/uaml-wasm/pkg");
const outDir = join(root, "packages/okf/src/generated");

execFileSync(
  "wasm-pack",
  ["build", "crates/uaml-wasm", "--target", "web", "--release", "--out-dir", "pkg"],
  { cwd: root, stdio: "inherit" },
);

mkdirSync(outDir, { recursive: true });
copyFileSync(join(pkg, "uaml_wasm.js"), join(outDir, "uaml_wasm.js"));
copyFileSync(join(pkg, "uaml_wasm.d.ts"), join(outDir, "uaml_wasm.d.ts"));

const wasm = readFileSync(join(pkg, "uaml_wasm_bg.wasm"));
const b64 = wasm.toString("base64");
writeFileSync(
  join(outDir, "wasm-inline.ts"),
  `// AUTO-GENERATED by scripts/build-wasm.mjs — do not edit.\n` +
  `export const WASM_BASE64 = "${b64}";\n` +
  `export function wasmBytes(): Uint8Array {\n` +
  `  const bin = atob(WASM_BASE64);\n` +
  `  const out = new Uint8Array(bin.length);\n` +
  `  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);\n` +
  `  return out;\n}\n`,
);
console.log(`wrote ${outDir}/wasm-inline.ts (${(b64.length / 1024).toFixed(0)} KB base64)`);
```

- [ ] **Step 3: Add the npm script + gitignore**

Root `package.json` `scripts`: add `"build:wasm": "node scripts/build-wasm.mjs"`.
`crates/uaml-wasm/.gitignore`: `pkg/`.

- [ ] **Step 4: Run it, verify outputs exist**

Run: `pnpm build:wasm`
Expected: creates `packages/okf/src/generated/{uaml_wasm.js,uaml_wasm.d.ts,wasm-inline.ts}`.
Run again — second run overwrites identically (idempotent).

- [ ] **Step 5: Commit** (commit the script + generated dir per the default
  "commit artifact" lean; the Render/staleness decision is finalized in Stage 1b/CI)

```bash
git add scripts/build-wasm.mjs crates/uaml-wasm/.gitignore package.json packages/okf/src/generated
git commit -m "build(wasm): inline base64 wasm build script + generated module"
```

---

### Task 7: JS smoke test — prove the inlined WASM is callable

**Files:**
- Create: `packages/okf/src/wasm/index.ts` (async init boundary)
- Test: `packages/okf/src/wasm/wasm.test.ts` (Vitest, node env)

**Interfaces:**
- Consumes: `packages/okf/src/generated/{uaml_wasm.js, wasm-inline.ts}` from Task 6.
- Produces: `initWasm(): Promise<void>` (memoized) and typed re-exports
  `build_model`, `validate`, `apply_ops`, `fmt`, `split_bundle`.

- [ ] **Step 1: Write the init boundary**

`packages/okf/src/wasm/index.ts`:

```ts
import init, * as bindings from "../generated/uaml_wasm.js";
import { wasmBytes } from "../generated/wasm-inline.js";

let ready: Promise<void> | null = null;
export function initWasm(): Promise<void> {
  return (ready ??= init(wasmBytes()).then(() => bindings.init_panic_hook()));
}
export const { build_model, validate, apply_ops, fmt, split_bundle } = bindings;
```

- [ ] **Step 2: Write the failing smoke test**

`packages/okf/src/wasm/wasm.test.ts`:

```ts
import { test, expect } from "vitest";
import { initWasm, build_model, apply_ops } from "./index";

test("apply_ops then build_model round-trips through wasm", async () => {
  await initWasm();
  const bundle = [["m/a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"]];
  const out = apply_ops(bundle, [{ op: "attr.add", node: "a", name: "id", ty: "AId" }]);
  const model = build_model(out);
  const node = model.nodes.find((n: any) => n.key === "a");
  expect(node.type).toBe("uml.Class");
  expect(node.attributes[0].name).toBe("id");
});
```

- [ ] **Step 3: Run it, verify it fails first (before `build:wasm` / wiring)**

Run: `pnpm --filter @uaml/okf test`
Expected: FAIL if generated module absent — run `pnpm build:wasm` first, then it
should be runnable. (`init(bytes)` works headless in Node ≥ 20 via `WebAssembly`.)

- [ ] **Step 4: Make it green**

Run: `pnpm build:wasm && pnpm --filter @uaml/okf test`
Expected: PASS. If Vitest can't load the ESM wasm glue in jsdom, set this test file
to the `node` environment (`// @vitest-environment node` at the top).

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/wasm
git commit -m "test(okf): JS smoke test proving inlined wasm is callable"
```

---

### Task 8: CI — build the WASM before the JS build

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:** Produces a green CI run that builds the wasm and runs `cargo test`
for the new crates on both matrix legs.

- [ ] **Step 1: Add a cargo test + wasm build step before `pnpm build`**

In `.github/workflows/ci.yml`, after "Install dependencies" and before "Lint",
add (keep it on both `ubuntu-latest` and `windows-latest` for now — revisit the
build-once-share-artifact optimization in Stage 1b):

```yaml
      - name: Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - name: Rust tests
        run: cargo test --workspace

      - name: Install wasm-pack
        run: cargo install wasm-pack --locked

      - name: Build inlined WASM
        run: pnpm build:wasm
```

- [ ] **Step 2: Verify locally that the sequence works**

Run: `cargo test --workspace && pnpm build:wasm && pnpm build && pnpm -r test`
Expected: all green.

- [ ] **Step 3: Commit + push the branch**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: build inlined wasm and run cargo tests"
```

---

## Self-review checklist (run before handing off)

- [ ] Every serde rename in Task 1 matches `packages/okf/src/types.ts` — verified by
  `serde_shape.rs` assertions on `type`/`key`/`from`/`to`/`kind`.
- [ ] `to_op()` in Tasks 4 matches the real conversion fn name/signature in
  `crates/uaml-ops-dto/src/lib.rs` (read it; fix if different).
- [ ] `build_model`, `validate`, `split_bundle` signatures match `crates/uaml/src/`
  (`build_model(&[(String,String)]) -> Model`, `validate(&[(String,String)]) ->
  Vec<Diagnostic>`, `split_bundle(&str) -> Vec<(String,String)>`).
- [ ] No OWOX renames performed in this stage (deferred).
- [ ] `cargo test -p uaml` (no features) still passes — serde is truly optional.

## Not in this plan (Stage 1b — next)

Rewire `packages/core/src/state/model.ts` (`createModelStore`) and
`packages/core/src/sync/merge.ts` onto `apply_ops` + a new
`packages/core/src/state/overlay.ts` (slug-keyed canvas data: position, `n#`/`e#`,
handles); `toModelGraph(model, overlay)`; swap `@uaml/okf` `parseBundle`/
`serializeBundle` bodies to wasm; retire the dead TS bodies; finalize the Render
delivery + package-layout decisions; `main.ts` async `initWasm().then(mount)`.
