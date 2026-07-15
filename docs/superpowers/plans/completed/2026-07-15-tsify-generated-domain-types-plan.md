# Tsify-generated domain types Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Rust (`crates/waml`, `crates/waml-ops-dto`) the single source of truth for every domain type that crosses the wasm boundary, so the hand-mirrored TypeScript copies in `packages/core/src/state/overlay.ts`, `packages/core/src/state/ops-adapter.ts`, and `packages/okf/src/types.ts` are replaced by Tsify-generated declarations re-exported from `@waml/wasm`.

**Architecture:** Add `#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]` + `#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]` to the domain structs/enums (exactly the pattern already proven on `Diagnostic`/`Severity`/`DiagCode` in `crates/waml/src/diagnostic.rs:1-75` and `Size`/`Rect`/`Solved` in `crates/waml/src/solve/mod.rs:22-69`). Flip the `#[wasm_bindgen]` exports in `crates/waml-wasm/src/lib.rs` from `JsValue`/`String` returns to the concrete Tsify types. Regenerate `packages/wasm/src/generated/*` via `node scripts/build-wasm.mjs`, then delete the hand-written TS mirrors and re-export the generated ones under their existing public names. The migration follows the spec's risk-ascending rollout, gated at each step by the same wire-shape pinning tests in `crates/waml/tests/serde_shape.rs`, which stay green because only the *derivation mechanism* changes, never the JSON shape.

**Tech Stack:** Rust (serde, `tsify-next` 0.5.6, `wasm-bindgen`, `serde-wasm-bindgen`, `wasm-bindgen-utils`), `wasm-pack` (invoked by `scripts/build-wasm.mjs`); TypeScript pnpm workspaces (`@waml/wasm`, `@waml/okf`, `@waml/core`, `@waml/web`), Vitest.

## Global Constraints

- **`tsify-next` is already a dependency at version `0.5.6` with `features = ["js"]`.** `crates/waml/Cargo.toml:15` has it `optional = true` behind the `wasm` feature (`crates/waml/Cargo.toml:20`); `crates/waml-wasm/Cargo.toml:20` has it non-optional. Do NOT bump or add any new crate dependency other than enabling `tsify-next` on `crates/waml-ops-dto` (Task 8). `wasm-bindgen = 0.2.126` (waml) / `0.2` (waml-wasm); `serde-wasm-bindgen = 0.6`; `wasm-bindgen-utils = 0.1.3`.
- **serde is NOT a default cargo feature.** `cargo test -p waml` alone compiles WITHOUT serde and silently skips `serde_shape.rs`. `cargo test --workspace` unifies features (via `waml-wasm`'s `waml = { features = ["wasm"] }`), turning on `serde` + `wasm` + the Tsify derives on a native target. **The Rust gate is always `cargo test --workspace`.**
- **The wasm blob and `.d.ts` are PREBUILT committed artifacts.** `packages/wasm/src/generated/waml_wasm.{js,d.ts}` and `packages/wasm/src/generated/wasm-inline.ts` are regenerated ONLY by `node scripts/build-wasm.mjs` (which runs `wasm-pack build crates/waml-wasm --target web`, `scripts/build-wasm.mjs:15-24`). Any Rust wire/type change is invisible to TypeScript until this is rerun. Every task that changes Rust types MUST rerun it before the TS gate.
- **The TSIFY BLOCK.** Unless a type needs a `type` override (see below), "add the TSIFY BLOCK" means inserting these two lines immediately below the existing `#[cfg_attr(feature = "serde", derive(...))]` / serde attribute lines and above `pub struct`/`pub enum`:
  ```rust
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  ```
- **Field-level `type` overrides** use the codebase's proven form (see `crates/waml/src/solve/mod.rs:64` — `#[cfg_attr(feature = "wasm", tsify(type = "Record<string, FlagSet>"))]`). A field-level override is placed as an extra `#[cfg_attr]` line directly above the field, alongside any existing serde field attribute.
- **`#[tsify(optional)]` on omittable fields** (fields with `skip_serializing_if` / `default`) is used to make the generated key optional (`field?: T`) so it matches the hand-written `field?: T`. Whether `optional` is accepted by tsify-next 0.5.6, and whether `#[serde(default)]` alone already produces an optional key, is answered by **Task 1 (Spike A)**. If Spike A reports `optional` is unsupported, drop the `optional` token everywhere in this plan and accept the tsify default rendering (`field: T | undefined`); the wire shape is unaffected either way (it is a `.d.ts`-only difference).
- **`serde(rename)` / `serde(rename_all)` are honored by tsify** (proven: `Severity`/`DiagCode` render as their kebab/lowercase string unions in the current `.d.ts`). Do not add TS overrides for enums that already serialize as strings via `rename_all` (`RelationshipKind`, `FlowFlavor`, `FlowNodeKind`, `MessageVerb`, `FragmentKind`, `ConceptRole`, and the internally/untagged enums `SeqItem`, `NoteAnchor`, `FmValue`).
- **Stringy newtypes/enums with custom serde (`Visibility`, `Multiplicity`, `AssocName`, `ClassifierType`) do NOT get a Tsify derive.** They serialize to a bare string / union via hand-written or `into`/`from = "String"` serde impls that tsify cannot introspect. Each is handled by a field-level `type` override at its single (or, for `Multiplicity`, two) use site(s). This means there is NO generated `Visibility`/`Multiplicity`/`AssocName`/`ClassifierType` type; `@waml/okf` keeps its hand-written `Visibility` alias (Task 7). *(Decision: the spec named three overrides — `Visibility`, `AssocName`, `ClassifierType` — but grounding in real code shows `Multiplicity` (`crates/waml/src/multiplicity.rs:11-14`, `serde(into/try_from = "String")`) and `Diagram.layout: Vec<LayoutStatement>` (`crates/waml/src/model.rs:688`; `LayoutStatement` is not Tsify, `crates/waml/src/syntax.rs:121-123`) need overrides too. Both are added below.)*
- **The full green gate for any task that touches Rust types or TS:** `cargo test --workspace` → `node scripts/build-wasm.mjs` → `pnpm -r test` → `pnpm build` → `pnpm lint`, all from the repo root. Tasks that touch only Rust wasm-export signatures still need the wasm rebuild + `pnpm build` so the regenerated `.d.ts` is committed and type-checks.
- **No new tooling, no restructuring of existing files.** Follow the established one-attribute-per-type pattern.

---

### Task 1: Spike A — does tsify-next 0.5.6 mark `#[serde(default)]` fields optional? (Open Risk #1)

This spike answers whether `OpDto.v: u32` (`#[serde(default = "one")]`, `crates/waml-ops-dto/src/lib.rs:16-17`) will emit as required (`v: number`) or optional (`v?: number`) in the generated `.d.ts`, and whether `#[tsify(optional)]` is a supported attribute in this tsify-next version. Task 4 and Task 8 depend on the answer. Must run before Task 4.

**Files:**
- Modify (throwaway, reverted at end): `crates/waml-wasm/src/lib.rs`
- Create: `docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md`

**Interfaces:**
- Consumes: nothing.
- Produces: a committed findings note recording (a) the exact generated `.d.ts` line for a `#[serde(default)]` field with no tsify attr, (b) whether `#[tsify(optional)]` compiles and what it emits. Tasks 4/8 read "SPIKE-A-RESULT" from this note.

- [ ] **Step 1: Add a throwaway Tsify struct** to the bottom of `crates/waml-wasm/src/lib.rs`:
  ```rust
  // SPIKE-A THROWAWAY — remove before commit of Task 1.
  #[derive(serde::Serialize, serde::Deserialize, tsify_next::Tsify)]
  #[tsify(into_wasm_abi, from_wasm_abi)]
  pub struct SpikeDefault {
      #[serde(default)]
      pub v: u32,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub opt_plain: Option<String>,
      #[cfg_attr(all(), tsify(optional))]
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub opt_marked: Option<String>,
  }
  ```
- [ ] **Step 2: Build wasm and inspect the generated declaration**
  Run: `node scripts/build-wasm.mjs`
  Then read `packages/wasm/src/generated/waml_wasm.d.ts` and locate the `SpikeDefault` interface (search for `SpikeDefault`).
  Expected: either the build fails on `tsify(optional)` (⇒ unsupported) OR it succeeds and prints an interface. Record the exact three field lines for `v`, `opt_plain`, `opt_marked`.
- [ ] **Step 3: Record the finding** in `docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md`:
  ```markdown
  # Tsify spike findings (2026-07-15)

  ## SPIKE-A-RESULT (Open Risk #1: #[serde(default)] optionality)
  - `#[serde(default)]` `v: u32` with no tsify attr generated as: `<paste exact line>`
  - `Option<T>` + skip, no tsify attr, generated as: `<paste exact line>`
  - `#[tsify(optional)]` on `Option<T>` + skip: <supported? paste exact line, OR "compile error: <msg>">
  - DECISION: <"use #[tsify(optional)] on omittable fields" | "omit optional token; tsify renders Option as `field?: T` already" | "omit optional token; accept `field: T | undefined`">
  ```
- [ ] **Step 4: Revert the throwaway** — delete the `SpikeDefault` block from `crates/waml-wasm/src/lib.rs`, and rerun `node scripts/build-wasm.mjs` so the committed `.d.ts` no longer contains `SpikeDefault`.
  Run: `cargo test --workspace`
  Expected: PASS (unchanged; the throwaway is gone).
- [ ] **Step 5: Commit**
  ```bash
  git add docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md packages/wasm/src/generated crates/waml-wasm/src/lib.rs
  git commit -m "chore(spike): record tsify #[serde(default)] optionality behavior (Open Risk #1)"
  ```

---

### Task 2: Spike B — does tsify's `into_wasm_abi` render a map field as a plain object? (Open Risk #2)

`Concept.extra` is a `Frontmatter` (`crates/waml/src/okf.rs:106-110`; `Frontmatter` = `Vec<(String, FmValue)>` with a hand-written `serialize_map` impl, `crates/waml/src/frontmatter.rs:23-38`). Today `build_bundle` crosses it as a plain object via `serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true)` (`crates/waml-wasm/src/lib.rs:113`). The smoke test `packages/wasm/src/smoke.test.ts:38` reads `c.extra?.owner` as an OBJECT property — a JS `Map` would make that `undefined`. This spike determines whether flipping `build_bundle` to a Tsify `Bundle` return (Task 7) would regress object → Map, mirroring the `Solved.nodes` handling in `crates/waml/src/solve/mod.rs:58-64`. Must run before Task 7.

**Files:**
- Modify (throwaway, reverted at end): `crates/waml-wasm/src/lib.rs`
- Modify: `docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md`

**Interfaces:**
- Consumes: nothing.
- Produces: appends "SPIKE-B-RESULT" to the findings note; Task 7 reads it to decide whether to keep the custom serializer or flip `build_bundle` to return `Bundle`.

- [ ] **Step 1: Add a throwaway export** that returns a map both ways, at the bottom of `crates/waml-wasm/src/lib.rs`:
  ```rust
  // SPIKE-B THROWAWAY — remove before commit of Task 2.
  #[derive(serde::Serialize, serde::Deserialize, tsify_next::Tsify)]
  #[tsify(into_wasm_abi, from_wasm_abi)]
  pub struct SpikeMap {
      pub m: std::collections::BTreeMap<String, u32>,
  }
  #[wasm_bindgen]
  pub fn spike_map_tsify() -> SpikeMap {
      let mut m = std::collections::BTreeMap::new();
      m.insert("owner".to_string(), 7u32);
      SpikeMap { m }
  }
  ```
- [ ] **Step 2: Write a throwaway probe test** at `packages/wasm/src/spike-b.test.ts`:
  ```ts
  import { test, expect } from "vitest";
  import { initWasm } from "./index";
  // @ts-expect-error throwaway export not in index barrel
  import { spike_map_tsify } from "./generated/waml_wasm.js";

  test("SPIKE-B: does tsify render a BTreeMap as a plain object?", async () => {
    await initWasm();
    const out: any = spike_map_tsify();
    console.log("SPIKE-B is Map:", out.m instanceof Map, "obj access:", out.m.owner, "map access:", out.m instanceof Map ? out.m.get("owner") : undefined);
    expect(out).toBeDefined();
  });
  ```
- [ ] **Step 3: Build wasm and run the probe**
  Run: `node scripts/build-wasm.mjs && pnpm --filter @waml/wasm test spike-b`
  Read the `SPIKE-B is Map:` console line.
- [ ] **Step 4: Record the finding** — append to `docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md`:
  ```markdown
  ## SPIKE-B-RESULT (Open Risk #2: map field rendering through tsify into_wasm_abi)
  - `out.m instanceof Map`: <true|false>
  - object access `out.m.owner`: <7|undefined>
  - DECISION for build_bundle (Task 7): <"tsify emits plain object → flip build_bundle to return Bundle and DELETE the serialize_maps_as_objects call" | "tsify emits a Map → KEEP build_bundle returning JsValue with serialize_maps_as_objects; add serialize_btreemap_as_object-style override only if flipping later">
  ```
- [ ] **Step 5: Revert throwaways** — delete the `SpikeMap`/`spike_map_tsify` block from `crates/waml-wasm/src/lib.rs`, delete `packages/wasm/src/spike-b.test.ts`, and rerun `node scripts/build-wasm.mjs`.
  Run: `cargo test --workspace && pnpm -r test`
  Expected: PASS (throwaways gone; `smoke.test.ts` still green).
- [ ] **Step 6: Commit**
  ```bash
  git add docs/superpowers/plans/notes/2026-07-15-tsify-spike-findings.md packages/wasm/src/generated crates/waml-wasm/src/lib.rs
  git commit -m "chore(spike): record tsify map-field rendering vs serialize_maps_as_objects (Open Risk #2)"
  ```

---

### Task 3: (Rollout step 1) Flip `validate()` to a typed `Diagnostic[]` return

Zero Rust type work — `Diagnostic` is already tsify'd (`crates/waml/src/diagnostic.rs:63-75`). This proves the pipeline (Rust signature → wasm rebuild → generated `.d.ts` → `@waml/wasm` re-export) end-to-end before any struct work.

**Files:**
- Modify: `crates/waml-wasm/src/lib.rs:118-123`
- Modify (regenerated): `packages/wasm/src/generated/waml_wasm.{js,d.ts}`, `packages/wasm/src/generated/wasm-inline.ts`
- Test: `crates/waml/tests/serde_shape.rs` (already green — `diagnostic_serializes_with_kebab_code_and_lowercase_severity`, lines 165-179), `packages/wasm/src/smoke.test.ts`

**Interfaces:**
- Consumes: `waml::validate::validate(&b) -> Vec<waml::diagnostic::Diagnostic>` (unchanged core).
- Produces: `validate(bundle: any): Diagnostic[]` in the generated `.d.ts` (was `validate(bundle: any): any`, `packages/wasm/src/generated/waml_wasm.d.ts:114`).

- [ ] **Step 1: Add a failing type-level assertion** in a new test file `packages/wasm/src/validate-typed.test.ts`:
  ```ts
  import { test, expect } from "vitest";
  import { initWasm, validate } from "./index";
  import type { Diagnostic } from "./index";

  test("validate returns a typed Diagnostic[]", async () => {
    await initWasm();
    const bundle: [string, string][] = [["a.md", "---\ntype: uml.Class\n---\n# A\n\n## Relationships\n- composes [Gone](./gone.md)\n"]];
    const diags: Diagnostic[] = validate(bundle);
    // If `validate` were still `any`, `.severity` access would be untyped; this pins the shape.
    expect(diags.every((d) => d.severity === "error" || d.severity === "warning")).toBe(true);
  });
  ```
- [ ] **Step 2: Run it to verify it fails at build time**
  Run: `pnpm build`
  Expected: FAIL — `packages/wasm` `tsc` errors because `validate` currently returns `any` and `Diagnostic[]` assignment from `any` succeeds at runtime but the generated `.d.ts` still declares `any` (the test compiles but does not yet exercise a typed return). If `tsc` passes, proceed — the meaningful gate is Step 4's regenerated `.d.ts`.
- [ ] **Step 3: Flip the signature.** Replace `crates/waml-wasm/src/lib.rs:118-123` with:
  ```rust
  /// `bundle`: a `[path, markdown][]`. Returns a `Diagnostic[]`.
  #[wasm_bindgen]
  pub fn validate(bundle: JsValue) -> Result<Vec<waml::diagnostic::Diagnostic>, JsValue> {
      let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
      Ok(waml::validate::validate(&b))
  }
  ```
- [ ] **Step 4: Rebuild wasm and confirm the generated declaration changed**
  Run: `node scripts/build-wasm.mjs`
  Then confirm `packages/wasm/src/generated/waml_wasm.d.ts` now declares `export function validate(bundle: any): Diagnostic[];` (was `: any`).
- [ ] **Step 5: Run the full gate**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS — `serde_shape.rs` unchanged and green, `smoke.test.ts` green, `validate-typed.test.ts` green, all four packages `tsc`-build.
- [ ] **Step 6: Commit**
  ```bash
  git add crates/waml-wasm/src/lib.rs packages/wasm/src/generated packages/wasm/src/validate-typed.test.ts
  git commit -m "feat(wasm): return typed Diagnostic[] from validate()"
  ```

---

### Task 4: (Rollout step 2, Rust half) Tsify the Model closure and flip `build_model` to a typed `Model`

`build_model` returns `Model`, and `Model` transitively embeds every domain type: `Node.concept` is an OKF `Concept`; `Model.flows`/`Model.interactions` are `FlowDoc`/`SequenceDoc`. For the regenerated `.d.ts` to be self-consistent, **all of these must derive Tsify together in this task** — the generated `Model` type names them, so an undefined name would break every consumer of `@waml/wasm`. *(Decision: this is why the OKF leaves `Concept`/`Link`/`Citation`/`FmValue`/`ConceptRole` and the entire Flow/Sequence closure are derived HERE, in step 2, not later — exactly the "a type needed by an earlier step is established there and merely reused later" case the rollout calls out. Steps 3 and 4 then only do TS-side re-export dedup and `Bundle`/`build_bundle`.)* Depends on Task 1 (Spike A) for the `optional` decision.

**Files:**
- Modify: `crates/waml/src/model.rs` — add TSIFY BLOCK + overrides to: `TypeRef` (58-67), `Attribute` (69-86), `RelationshipKind` (88-101), `RelEnd` (142-160), `Edge` (209-230), `FlowNodeKind` (304-316), `FlowFlavor` (~346-352), `FlowNode` (355-377), `FlowEdge` (380-401), `FlowDoc` (404-415), `MessageVerb` (418-427), `FragmentKind` (452-477), `Lifeline` (480-495), `SeqOperand` (498-507), `SeqItem` (510-528), `SequenceDoc` (531-543), `NoteAnchor` (601-622), `Node` (624-666), `DiagramGroup` (669-675), `Diagram` (677-691), `DiagramDisplay` (696-722), `Model` (730-748)
- Modify: `crates/waml/src/okf.rs` — add TSIFY BLOCK to `ConceptRole` (19-26), `Link` (35-40), `Citation` (44-49), `Concept` (55-111) + override on `extra`
- Modify: `crates/waml/src/frontmatter.rs` — add TSIFY BLOCK to `FmValue` (9-19)
- Modify: `crates/waml-wasm/src/lib.rs:96-101` (`build_model`)
- Modify (regenerated): `packages/wasm/src/generated/*`
- Test: `crates/waml/tests/serde_shape.rs` (all tests, unchanged, must stay green)

**Interfaces:**
- Consumes: Spike A decision (whether to write `#[tsify(optional)]`).
- Produces (generated `.d.ts` type names other tasks re-export): `Model`, `Node`, `Edge`, `RelEnd`, `TypeRef`, `Attribute`, `RelationshipKind`, `NoteAnchor`, `DiagramGroup`, `DiagramDisplay`, `Diagram`, `FlowDoc`, `FlowNode`, `FlowEdge`, `FlowFlavor`, `FlowNodeKind`, `SequenceDoc`, `Lifeline`, `SeqItem`, `SeqOperand`, `MessageVerb`, `FragmentKind`, `Concept`, `Link`, `Citation`, `FmValue`, `ConceptRole`. `build_model(bundle: any): Model`.

- [ ] **Step 1: Confirm the shape-pinning tests fail-safe.** These already exist and must stay green; run them first as the baseline.
  Run: `cargo test --workspace -- serde_shape`
  Expected: PASS (baseline before any change).
- [ ] **Step 2: Add the TSIFY BLOCK to every plain enum/struct listed above.** For each, insert the two-line TSIFY BLOCK directly beneath its existing `#[cfg_attr(feature = "serde", derive(...))]`/serde attributes. Example — `RelationshipKind` (`crates/waml/src/model.rs:88-101`) becomes:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub enum RelationshipKind {
  ```
  And `TypeRef` (`crates/waml/src/model.rs:58-67`) becomes:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub struct TypeRef {
      pub name: String,
      #[cfg_attr(
          feature = "serde",
          serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
      )]
      pub ref_: Option<String>,
  }
  ```
  Apply the identical two-line block to: `Attribute`, `RelEnd`, `Edge`, `FlowNodeKind`, `FlowFlavor`, `FlowNode`, `FlowEdge`, `FlowDoc`, `MessageVerb`, `FragmentKind`, `Lifeline`, `SeqOperand`, `SeqItem`, `SequenceDoc`, `NoteAnchor`, `Node`, `DiagramGroup`, `Diagram`, `DiagramDisplay`, `Model` (model.rs), and `ConceptRole`, `Link`, `Citation`, `Concept` (okf.rs), and `FmValue` (frontmatter.rs). Do NOT add the block to `Visibility`, `Multiplicity`, `AssocName`, `ClassifierType`, `Frontmatter`, or `LayoutStatement` (handled by overrides below).
- [ ] **Step 3: Add the field-level `type` overrides.** Insert each extra `#[cfg_attr]` line directly above its field (keep the existing serde line). If Spike A said `optional` is unsupported, delete the `optional,` token from these lines.
  - `Attribute.multiplicity` (`crates/waml/src/model.rs:75`):
    ```rust
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub multiplicity: Multiplicity,
    ```
  - `Attribute.visibility` (`crates/waml/src/model.rs:76-80`):
    ```rust
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    #[cfg_attr(feature = "wasm", tsify(optional, type = "\"+\" | \"-\" | \"#\" | \"~\""))]
    pub visibility: Option<Visibility>,
    ```
  - `RelEnd.multiplicity` (`crates/waml/src/model.rs:145-149`): add above `pub multiplicity`:
    ```rust
    #[cfg_attr(feature = "wasm", tsify(optional, type = "string"))]
    ```
  - `Edge.name` (`crates/waml/src/model.rs:217-221`): add above `pub name`:
    ```rust
    #[cfg_attr(feature = "wasm", tsify(optional, type = "string | { ref: string }"))]
    ```
  - `Node.ty` (`crates/waml/src/model.rs:634-635`): add above `pub ty`, keeping the serde rename:
    ```rust
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub ty: ClassifierType,
    ```
  - `Diagram.layout` (`crates/waml/src/model.rs:688`): add above `pub layout` (*decision: `LayoutStatement` is not Tsify and no TS code reads `layout` yet — `overlay.ts`'s `RustDiagram` omits it entirely; type it opaquely rather than deriving the whole `syntax` AST*):
    ```rust
    #[cfg_attr(feature = "wasm", tsify(type = "unknown[]"))]
    pub layout: Vec<crate::syntax::LayoutStatement>,
    ```
  - `Concept.extra` (`crates/waml/src/okf.rs:106-110`): add above `pub extra`:
    ```rust
    #[cfg_attr(feature = "wasm", tsify(optional, type = "Record<string, FmValue>"))]
    pub extra: Frontmatter,
    ```
- [ ] **Step 4: Flip `build_model`.** Replace `crates/waml-wasm/src/lib.rs:96-101` with:
  ```rust
  /// `bundle`: a `[path, markdown][]` (array of pairs). Returns the resolved `Model`.
  #[wasm_bindgen]
  pub fn build_model(bundle: JsValue) -> Result<waml::model::Model, JsValue> {
      let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(&bundle)
          .map_err(JsValue::from)?;
      Ok(waml::parse::build_model(&b))
  }
  ```
  (If `serde_wasm_bindgen::from_value` does not accept `&bundle` in this version, keep `from_value(bundle)?` as in the current code — the only required change is the `-> Result<waml::model::Model, JsValue>` return and dropping the `serde_wasm_bindgen::to_value(&model)?` wrapper.)
- [ ] **Step 5: Compile-check Rust before rebuilding wasm**
  Run: `cargo test --workspace`
  Expected: PASS — `serde_shape.rs` still green (JSON shape unchanged; only derives added), all native `waml`/`waml-wasm` tests green.
- [ ] **Step 6: Rebuild wasm and eyeball the generated types**
  Run: `node scripts/build-wasm.mjs`
  Confirm in `packages/wasm/src/generated/waml_wasm.d.ts`: `build_model(bundle: any): Model;`, an `interface Model`, `interface Node { ... type: string; ... concept: Concept; ... }`, `interface Edge { ... name?: string | { ref: string }; fromEnd: RelEnd; toEnd: RelEnd }`, `interface Attribute { ... multiplicity: string; visibility?: "+" | "-" | "#" | "~"; ... }`, `interface Diagram { ... layout: unknown[]; ... }`, `interface Concept { ... extra?: Record<string, FmValue>; ... }`, and the flow/sequence interfaces.
- [ ] **Step 7: Run the full gate**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS. `overlay.ts`'s hand-written `RustModel`/`RustNode`/etc. are still in place and structurally identical to the new generated types, so `@waml/core` still builds. (The overlay swap is Task 5.)
- [ ] **Step 8: Commit**
  ```bash
  git add crates/waml/src/model.rs crates/waml/src/okf.rs crates/waml/src/frontmatter.rs crates/waml-wasm/src/lib.rs packages/wasm/src/generated
  git commit -m "feat(wasm): tsify the Model closure and return a typed Model from build_model"
  ```

---

### Task 5: (Rollout step 2, TS half) Replace `overlay.ts` hand-written `Rust*` mirrors with generated imports

`overlay.ts` (`packages/core/src/state/overlay.ts:22-94`) hand-declares `RustNode`/`RustEdge`/`RustDiagramGroup`/`RustDiagramDisplay`/`RustDiagram`/`RustModel`, which are now redundant with the generated `Node`/`Edge`/`DiagramGroup`/`DiagramDisplay`/`Diagram`/`Model`. `@waml/core` already depends on `@waml/wasm` (`packages/core/package.json:18`), so this is a pure import swap; `toModelGraph`'s body (which uses the `RustX` names throughout) is untouched.

**Files:**
- Modify: `packages/core/src/state/overlay.ts:22-94`
- Test: `packages/core/src/state/overlay.test.ts`, `packages/core/src/state/model.test.ts`

**Interfaces:**
- Consumes: generated `Model`/`Node`/`Edge`/`Diagram`/`DiagramGroup`/`DiagramDisplay` from `@waml/wasm` (Task 4).
- Produces: `overlay.ts` no longer declares any `Rust*` interface; `toModelGraph` signature unchanged.

- [ ] **Step 1: Run the existing overlay tests as the baseline**
  Run: `pnpm --filter @waml/core test overlay`
  Expected: PASS (baseline before the swap).
- [ ] **Step 2: Delete the hand-written block and add aliased imports.** Remove the entire `// ── Rust `Model` JSON shape …` block (`packages/core/src/state/overlay.ts:22-94`, i.e. the six `export interface RustNode/RustEdge/RustDiagramGroup/RustDiagramDisplay/RustDiagram/RustModel` declarations). In their place, extend the existing top-of-file import so the generated types are aliased to the `RustX` names the rest of the file already uses:
  ```ts
  import type {
    Model as RustModel,
    Node as RustNode,
    Edge as RustEdge,
    Diagram as RustDiagram,
    DiagramGroup as RustDiagramGroup,
    DiagramDisplay as RustDiagramDisplay,
  } from "@waml/wasm";
  ```
  Keep the existing `NodeOverlay`/`Overlay`/`ModelGraph` machinery and the `import type { ... } from "@waml/okf"` line intact. *(Decision: `RustDiagramGroup` had no separate generated equivalent name mismatch — the generated `DiagramGroup` is the recursive wire forest, matching the deleted interface. `RustDiagram` now additionally carries `layout: unknown[]`; `toModelGraph` never reads it, so no code change.)*
- [ ] **Step 3: Type-check and run overlay/model tests**
  Run: `pnpm --filter @waml/core build && pnpm --filter @waml/core test overlay model`
  Expected: PASS — the generated types are structurally identical to the deleted interfaces, so `toModelGraph` type-checks unchanged.
- [ ] **Step 4: Run the full gate**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS.
- [ ] **Step 5: Commit**
  ```bash
  git add packages/core/src/state/overlay.ts
  git commit -m "refactor(core): consume generated Model wire types in overlay.ts, drop Rust* mirrors"
  ```

---

### Task 6: (Rollout step 3) Dedup Flow/Sequence + Model-leaf types in `@waml/okf` via re-export

`@waml/okf` hand-writes the Flow/Sequence types and the Model-leaf types (`TypeRef`/`Attribute`/`RelEnd`/`RelationshipKind`/`NoteAnchor`) that `overlay.ts` and `ops-adapter.ts` import from it (`packages/okf/src/types.ts:1-286`). They are now generated in `@waml/wasm` (Task 4). Replace the hand-written declarations with type-only re-exports so nothing importing them from `@waml/okf` needs to change. This task also introduces the `@waml/wasm` dependency into `@waml/okf`.

**Files:**
- Modify: `packages/okf/package.json` (add `@waml/wasm` dependency)
- Modify: `packages/okf/src/types.ts` (delete hand-written wire mirrors 7-34 + 168-261 + 276-286; add re-export block)
- Test: `packages/core/src/state/overlay.test.ts`, `packages/core/src/state/model.test.ts`, `packages/core/src/templates/templates.test.ts`

**Interfaces:**
- Consumes: generated `TypeRef`, `Attribute`, `RelEnd`, `RelationshipKind`, `NoteAnchor`, `FlowDoc`, `FlowNode`, `FlowEdge`, `FlowFlavor`, `FlowNodeKind`, `SequenceDoc`, `Lifeline`, `SeqItem`, `SeqOperand`, `MessageVerb`, `FragmentKind` from `@waml/wasm`.
- Produces: `@waml/okf` re-exports those names unchanged; keeps hand-written `Visibility`, `ModelNode`, `ModelEdge`, `DiagramHints`, `DiagramDisplay` (resolved), `DEFAULT_DISPLAY`, `resolveDisplay`, `Diagram` (UI), `ModelGraph`, `splitType`, `RELATIONSHIP_KINDS`, `ENDED_KINDS`.

- [ ] **Step 1: Baseline the okf-dependent tests**
  Run: `pnpm --filter @waml/core test`
  Expected: PASS (baseline).
- [ ] **Step 2: Add the dependency.** In `packages/okf/package.json`, add `@waml/wasm` to a `dependencies` block (the file currently has none):
  ```json
  { "name": "@waml/okf", "version": "0.0.0", "type": "module", "license": "Apache-2.0",
    "main": "dist/index.js", "types": "dist/index.d.ts",
    "scripts": { "build": "tsc -p tsconfig.json", "test": "vitest run --passWithNoTests" },
    "dependencies": { "@waml/wasm": "workspace:*" },
    "devDependencies": { "vitest": "^2.1.0", "typescript": "^5.6.0" } }
  ```
  Then run `pnpm install` so the workspace link is created.
- [ ] **Step 3: Delete the hand-written Model-leaf + Flow/Sequence types and re-export.** In `packages/okf/src/types.ts`:
  - Delete `TypeRef` (8), `Attribute` (10-17), `RelEnd` (28), `NoteAnchor` (31-34).
  - Delete the `export type RelationshipKind = (typeof RELATIONSHIP_KINDS)[number];` line (21) but KEEP `RELATIONSHIP_KINDS` (20) and `ENDED_KINDS` (26) — they are runtime values. *(Decision: the generated `RelationshipKind` union has identical members to `RELATIONSHIP_KINDS`, so `ReadonlySet<RelationshipKind>` on line 26 still type-checks against the const's element strings.)*
  - Delete the entire Flow substrate block `FlowFlavor` (169) through `SequenceDoc` (261) — i.e. `FlowFlavor`, `FlowNodeKind`, `FlowNode`, `FlowEdge`, `FlowDoc`, `MessageVerb`, `FragmentKind`, `Lifeline`, `SeqOperand`, `SeqItem`, `SequenceDoc`. KEEP `splitType` (264-267).
  - Delete the hand-written `FmValue` (277), `ConceptRole` (280), `Link` (283), `Citation` (286) — these move in Task 7; leave the `Concept`/`Bundle` hand-written for now (Task 7 removes them) so this task stays compiling. *(To keep Task 6 self-contained, do the OKF-leaf move in Task 7; here only Model-leaf + Flow/Sequence move.)*
  - Add, near the top of the file, one re-export block:
    ```ts
    export type {
      TypeRef,
      Attribute,
      RelEnd,
      RelationshipKind,
      NoteAnchor,
      FlowFlavor,
      FlowNodeKind,
      FlowNode,
      FlowEdge,
      FlowDoc,
      MessageVerb,
      FragmentKind,
      Lifeline,
      SeqOperand,
      SeqItem,
      SequenceDoc,
    } from "@waml/wasm";
    ```
  - Keep `export type Visibility = "+" | "-" | "#" | "~";` (5) hand-written — it has no generated equivalent (Global Constraints).
  - The still-hand-written `Concept`/`Bundle` (291-314) reference `Link`/`Citation`/`FmValue`/`ConceptRole`; since those are deleted here, temporarily keep local `Link`/`Citation`/`FmValue`/`ConceptRole` declarations OR (cleaner) move the OKF-leaf re-export into THIS task too. *(Decision: to avoid a broken intermediate, fold the OKF-leaf re-export forward — add `FmValue`, `ConceptRole`, `Link`, `Citation` to the re-export block above and delete their hand-written forms now; Task 7 then only handles `Concept`/`Bundle` + `build_bundle`. This keeps every commit compiling.)* Final re-export block therefore also includes `FmValue, ConceptRole, Link, Citation`.
- [ ] **Step 4: Type-check okf and its consumers**
  Run: `pnpm --filter @waml/okf build && pnpm --filter @waml/core build`
  Expected: PASS — consumers importing `FlowDoc`/`Attribute`/`RelEnd`/etc. from `@waml/okf` resolve to the re-exported generated types.
- [ ] **Step 5: Run the full gate**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS — `overlay.test.ts`, `model.test.ts`, `templates.test.ts` green.
- [ ] **Step 6: Commit**
  ```bash
  git add packages/okf/package.json packages/okf/src/types.ts pnpm-lock.yaml
  git commit -m "refactor(okf): re-export generated Flow/Sequence + model-leaf + OKF-leaf types from @waml/wasm"
  ```

---

### Task 7: (Rollout step 4) Tsify `Bundle`, dedup `Concept`/`Bundle` in `@waml/okf`, and settle `build_bundle`

`Concept`/`Link`/`Citation`/`FmValue`/`ConceptRole` are already tsify'd (Task 4); only `Bundle` (`crates/waml/src/okf.rs:114-118`) remains. This task derives `Bundle`, replaces the last hand-written OKF wire types (`Concept`, `Bundle`) in `@waml/okf` with re-exports, and applies the Spike B decision to `build_bundle`. Depends on Task 2 (Spike B).

**Files:**
- Modify: `crates/waml/src/okf.rs:114-118` (`Bundle`)
- Modify (Spike-B-gated): `crates/waml-wasm/src/lib.rs:108-115` (`build_bundle`)
- Modify (regenerated): `packages/wasm/src/generated/*`
- Modify: `packages/okf/src/types.ts` (delete `Concept` 291-309, `Bundle` 312-314; add to re-export)
- Test: `packages/wasm/src/smoke.test.ts` (the `c.extra?.owner` object guard, line 38), `crates/waml/src/okf.rs` serde tests

**Interfaces:**
- Consumes: Spike B decision; generated `Concept`/`Bundle` from `@waml/wasm`.
- Produces: `@waml/okf` re-exports `Concept`, `Bundle`; `build_bundle` either typed `Bundle` or unchanged `any` per Spike B.

- [ ] **Step 1: Baseline the bundle smoke test**
  Run: `pnpm --filter @waml/wasm test smoke`
  Expected: PASS (`c.extra?.owner === "data-team"` — proves object semantics today).
- [ ] **Step 2: Derive Tsify on `Bundle`.** `crates/waml/src/okf.rs:114-118` becomes:
  ```rust
  #[derive(Debug, Clone, Default, PartialEq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub struct Bundle {
      pub concepts: Vec<Concept>,
  }
  ```
- [ ] **Step 3: Apply the Spike B decision to `build_bundle`.**
  - **If SPIKE-B-RESULT = "plain object"** — flip the return type and delete the custom serializer. Replace `crates/waml-wasm/src/lib.rs:108-115` with:
    ```rust
    /// `bundle`: a `[path, markdown][]`. Returns the resolved OKF `Bundle` (one
    /// `Concept` per document). Additive to [`build_model`]; the UML surface is
    /// untouched. `Concept.extra` (frontmatter) crosses as a plain JS object.
    #[wasm_bindgen]
    pub fn build_bundle(bundle: JsValue) -> Result<waml::okf::Bundle, JsValue> {
        let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
        Ok(waml::okf::build_bundle(&b))
    }
    ```
  - **If SPIKE-B-RESULT = "Map"** — do NOT flip; keep `build_bundle` exactly as `crates/waml-wasm/src/lib.rs:108-115` today (returning `JsValue` via `serialize_maps_as_objects(true)`), so the smoke test's object access stays valid. The `Bundle`/`Concept` TS types are still generated by their derives and re-exported below; only the *typed return* is deferred. Leave a one-line comment above `build_bundle` noting the Spike-B rationale.
- [ ] **Step 4: Rebuild wasm**
  Run: `node scripts/build-wasm.mjs`
  If Step 3 flipped the return, confirm `build_bundle(bundle: any): Bundle;` in `packages/wasm/src/generated/waml_wasm.d.ts`; otherwise it stays `: any`.
- [ ] **Step 5: Re-export `Concept`/`Bundle` from `@waml/okf`.** In `packages/okf/src/types.ts`, delete the hand-written `Concept` (291-309) and `Bundle` (312-314), and add `Concept, Bundle` to the `export type { ... } from "@waml/wasm"` block created in Task 6. Delete the now-orphaned OKF-tier comment banner (269-274) if it only documented the removed types.
- [ ] **Step 6: Run the full gate — the smoke test is the critical guard**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS. Specifically `packages/wasm/src/smoke.test.ts` line 38 (`c.extra?.owner === "data-team"`) MUST stay green — if it fails, Step 3 chose the wrong branch (Map crossed where object expected); revert Step 3 to the "Map" branch.
- [ ] **Step 7: Commit**
  ```bash
  git add crates/waml/src/okf.rs crates/waml-wasm/src/lib.rs packages/wasm/src/generated packages/okf/src/types.ts
  git commit -m "feat(okf): tsify Bundle and re-export generated Concept/Bundle from @waml/wasm"
  ```

---

### Task 8: (Rollout step 5) Tsify `OpDto` and wire `apply_ops` to the generated type

`OpDto` (`crates/waml-ops-dto/src/lib.rs:11-201`) and its nested `DisplayDto` (204-220) are hand-mirrored in `packages/core/src/state/ops-adapter.ts:37-80` (which deliberately omits the `v` field and the `diagram.set` variant). This is the highest-risk step because of the `v: u32` `#[serde(default = "one")]` field on every variant (`crates/waml-ops-dto/src/lib.rs:16-17`) — Spike A (Task 1) determines whether the generated `v` is optional. `waml-ops-dto` has no `wasm`/tsify feature today, so this task also wires that up. Depends on Task 1.

**Files:**
- Modify: `crates/waml-ops-dto/Cargo.toml` (add optional `tsify-next` + `wasm-bindgen` + `wasm` feature)
- Modify: `crates/waml-wasm/Cargo.toml:14` (enable the `wasm` feature on the `waml-ops-dto` dependency)
- Modify: `crates/waml-ops-dto/src/lib.rs:11-13` (`OpDto` derive/attrs) and `:204-206` (`DisplayDto` derive/attrs)
- Modify (Spike-A-gated): `crates/waml-wasm/src/lib.rs:126-134` (`apply_ops`)
- Modify (regenerated): `packages/wasm/src/generated/*`
- Modify: `packages/core/src/state/ops-adapter.ts:37-80` (replace hand-written `OpDto` with a re-export)
- Test: `packages/core/src/state/ops-adapter.test.ts`, `crates/waml-ops-dto` round-trip tests (`crates/waml-ops-dto/src/lib.rs:527-716`)

**Interfaces:**
- Consumes: Spike A decision (whether `v` is optional in generated `OpDto`).
- Produces: generated `OpDto` + `DisplayDto` in `@waml/wasm`; `@waml/okf`- or `@waml/wasm`-sourced `OpDto` in `ops-adapter.ts`; `apply_ops(bundle: any, ops: OpDto[]): [string, string][]` (if flipped).

- [ ] **Step 1: Baseline the ops round-trip + adapter tests**
  Run: `cargo test --workspace -p waml-ops-dto && pnpm --filter @waml/core test ops-adapter`
  Expected: PASS (baseline; wire shape unchanged throughout this task).
- [ ] **Step 2: Add tsify to `waml-ops-dto`'s Cargo.** Replace `crates/waml-ops-dto/Cargo.toml` dependency/feature section with:
  ```toml
  [dependencies]
  waml = { path = "../waml" }
  serde = { workspace = true }
  tsify-next = { version = "0.5.6", features = ["js"], optional = true }
  wasm-bindgen = { version = "0.2", optional = true }

  [features]
  default = []
  wasm = ["dep:tsify-next", "dep:wasm-bindgen"]

  [dev-dependencies]
  serde_json = { workspace = true }
  ```
- [ ] **Step 3: Enable the feature from `waml-wasm`.** Change `crates/waml-wasm/Cargo.toml:14` from:
  ```toml
  waml-ops-dto = { path = "../waml-ops-dto" }
  ```
  to:
  ```toml
  waml-ops-dto = { path = "../waml-ops-dto", features = ["wasm"] }
  ```
- [ ] **Step 4: Derive Tsify on `OpDto` and `DisplayDto`.** `crates/waml-ops-dto/src/lib.rs:11-13`:
  ```rust
  #[derive(Debug, Serialize, Deserialize)]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  #[serde(tag = "op")]
  pub enum OpDto {
  ```
  If Spike A reported `v` is emitted as required and `#[tsify(optional)]` IS supported, add `#[cfg_attr(feature = "wasm", tsify(optional))]` above each `v: u32` field so the generated variants read `v?: number` (matching the adapter which never sends `v`). If Spike A reported `v` is already optional (or `optional` unsupported and `v` renders required), leave the fields as-is and rely on the adapter constructing objects without `v` (callers pass `any`-compatible literals; a required `v?` mismatch surfaces in Step 8's `tsc`). And `DisplayDto` at `:204-206`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  #[serde(rename_all = "camelCase")]
  pub struct DisplayDto {
  ```
- [ ] **Step 5: Compile-check Rust**
  Run: `cargo test --workspace`
  Expected: PASS — the `waml-ops-dto` round-trip tests (`crates/waml-ops-dto/src/lib.rs:537-716`) stay green (only derives added; serde shape unchanged).
- [ ] **Step 6: Apply the Spike A decision to `apply_ops`.** If Spike A confirmed the generated `OpDto` is usable from TS without `v` (optional `v`), flip `crates/waml-wasm/src/lib.rs:126-134` to a typed input:
  ```rust
  /// `bundle`: a `[path, markdown][]`; `ops`: an `OpDto[]`. Returns the edited bundle.
  #[wasm_bindgen]
  pub fn apply_ops(bundle: JsValue, ops: Vec<waml_ops_dto::OpDto>) -> Result<JsValue, JsValue> {
      let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
      let parsed = dtos_to_ops(ops).map_err(|e| JsValue::from_str(&e))?;
      let out = waml::ops::apply(&b, &parsed)
          .map_err(|e| JsValue::from_str(&format!("op {}: {}", e.index, e.reason)))?;
      Ok(serde_wasm_bindgen::to_value(&out)?)
  }
  ```
  (`Vec<OpDto>` as a `#[wasm_bindgen]` parameter works because `OpDto` now has `from_wasm_abi`.) If Spike A showed the generated `OpDto` would force callers to supply `v` (breaking the adapter's `v`-less literals) and `optional` is unsupported, leave `apply_ops` unchanged (`ops: JsValue`) — the generated `OpDto` type is still emitted and re-exported for callers that want it, and Step 7 imports it type-only.
- [ ] **Step 7: Rebuild wasm**
  Run: `node scripts/build-wasm.mjs`
  Confirm `packages/wasm/src/generated/waml_wasm.d.ts` contains an `OpDto` discriminated-union type (with the `diagram.set` variant and `DisplayDto`), and — per Spike A — `v?: number` on each variant.
- [ ] **Step 8: Re-export `OpDto` in `ops-adapter.ts`.** In `packages/core/src/state/ops-adapter.ts`, delete the hand-written `export type OpDto = ...` union (`:37-80`) and replace it with a re-export plus the local `EdgeName` alias it still needs:
  ```ts
  import type { OpDto } from "@waml/wasm";
  export type { OpDto };
  ```
  Keep every op-constructor function (`nodeNewOps`, `attrDiffOps`, `edgeSetOps`, …) unchanged; they build object literals that satisfy the generated union (with `v` omitted, now legal via `v?`). If `tsc` flags any literal because a generated field is stricter than the hand-written one (e.g. `pkg.delete` `cascade` — generated `cascade?: boolean` vs the hand-written required `cascade: boolean`), adjust only the offending literal, not the generated type. *(Note: the generated union is a SUPERSET of the old hand-written one — it adds the `diagram.set` variant and `v?` — so existing narrowings by `op` tag remain valid.)*
- [ ] **Step 9: Run the full gate**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS — `ops-adapter.test.ts` green (adapter still emits identical wire JSON), all packages build.
- [ ] **Step 10: Commit**
  ```bash
  git add crates/waml-ops-dto/Cargo.toml crates/waml-wasm/Cargo.toml crates/waml-ops-dto/src/lib.rs crates/waml-wasm/src/lib.rs packages/wasm/src/generated packages/core/src/state/ops-adapter.ts
  git commit -m "feat(ops): tsify OpDto/DisplayDto and consume the generated OpDto in ops-adapter"
  ```

---

### Task 9: Final consistency sweep — update `@waml/wasm` barrel and confirm no stale mirrors remain

The generated domain types should be exported from the `@waml/wasm` barrel (`packages/wasm/src/index.ts:17-29`) alongside the solver types, so consumers can import them directly and the "single source of truth" comment holds. Also verify no hand-written wire mirror survived.

**Files:**
- Modify: `packages/wasm/src/index.ts:17-29` (extend the generated-type re-export block)
- Test: `pnpm build`, `pnpm -r test`

**Interfaces:**
- Consumes: all generated type names from Tasks 4/7/8.
- Produces: `@waml/wasm` publicly re-exports the domain types (not just solver types).

- [ ] **Step 1: Extend the barrel re-export.** In `packages/wasm/src/index.ts`, add the domain types to the existing `export type { ... } from "./generated/waml_wasm.js";` block (lines 17-29):
  ```ts
  export type {
    Size,
    Rect,
    FlagSet,
    Shape,
    SolvedGroup,
    Solved,
    SolveConfig,
    Diagnostic,
    DiagCode,
    Severity,
    SolveResult,
    Model,
    Node,
    Edge,
    RelEnd,
    TypeRef,
    Attribute,
    RelationshipKind,
    NoteAnchor,
    DiagramGroup,
    DiagramDisplay,
    Diagram,
    FlowDoc,
    FlowNode,
    FlowEdge,
    FlowFlavor,
    FlowNodeKind,
    SequenceDoc,
    Lifeline,
    SeqItem,
    SeqOperand,
    MessageVerb,
    FragmentKind,
    Concept,
    Bundle,
    Link,
    Citation,
    FmValue,
    ConceptRole,
    OpDto,
  } from "./generated/waml_wasm.js";
  ```
- [ ] **Step 2: Grep for surviving hand-written mirrors** to confirm the migration is complete.
  Run: `git grep -nE "interface Rust(Node|Edge|Model|Diagram|DiagramGroup|DiagramDisplay)" -- packages` and `git grep -nE "export (type|interface) (FlowDoc|SequenceDoc|Concept|Bundle|Attribute|RelEnd|OpDto)\b" -- packages/okf packages/core`
  Expected: the first returns nothing; the second returns only re-export lines (`export type { ... } from "@waml/wasm"`), no standalone declarations.
- [ ] **Step 3: Run the full gate**
  Run: `cargo test --workspace && pnpm -r test && pnpm build && pnpm lint`
  Expected: PASS.
- [ ] **Step 4: Commit**
  ```bash
  git add packages/wasm/src/index.ts
  git commit -m "feat(wasm): re-export generated domain types from the @waml/wasm barrel"
  ```

---

## Self-Review

**1. Spec coverage** — every spec requirement mapped to a task:
- "flip `validate()` to `Result<Vec<Diagnostic>, JsValue>`" → Task 3.
- "Model surface: `Node`/`Edge`/`RelEnd`/`TypeRef`/`Attribute`/`RelationshipKind`/`NoteAnchor`/`DiagramGroup`/`DiagramDisplay`/`Diagram`/`Model` + `build_model`" → Task 4.
- "three `#[tsify(type=...)]` overrides (`Visibility`, `AssocName`, `ClassifierType`)" → Task 4 Step 3 (plus the two extra overrides — `Multiplicity`, `Diagram.layout` — that grounding in real code revealed; flagged in Global Constraints + Task 4).
- "Flow/Sequence pass-through riding on `build_model`" → derives in Task 4 (Model transitively needs them), TS dedup in Task 6.
- "OKF `Concept`/`Bundle`/`Link`/`Citation`/`FmValue`/`ConceptRole` + `HashMap<String, FmValue>` handling" → `Concept`/leaves derived in Task 4, `Bundle` + build_bundle + re-export in Task 7; the map is `Concept.extra: Frontmatter` handled by the `Record<string, FmValue>` override (Task 4 Step 3) and the Spike-B-gated serializer decision (Task 7).
- "`OpDto`" → Task 8.
- "`overlay.ts` import flip deleting `RustModel`/…" → Task 5, matching the spec's proposed aliased-import block.
- "`@waml/okf` re-export + new `@waml/wasm` dependency, keep `ModelNode`/`ModelEdge`/`ModelGraph`/`DiagramHints`/resolved `DiagramDisplay`/`DEFAULT_DISPLAY`/`resolveDisplay`/UI `Diagram` hand-written" → Tasks 6 & 7 (delete list + keep list enumerated explicitly).
- "`serde_shape.rs` stays green throughout" → asserted as a gate step in Tasks 3, 4, and every `cargo test --workspace` gate.
- Open Risk #1 spike → Task 1; Open Risk #2 spike → Task 2; both upfront, before their dependent steps (Task 4/8 depend on Task 1; Task 7 depends on Task 2).
- "no new dependency beyond `tsify-next`" → only added to `waml-ops-dto` (Task 8), matching the spec.

**2. Placeholder scan** — no "TBD"/"handle edge cases"/"similar to Task N". Every code step shows the exact `#[cfg_attr]` lines, exact signatures, exact TS blocks, and exact commands with expected results. The two spike-gated branches (Task 7 Step 3, Task 8 Steps 4/6/8) show BOTH concrete branches with the decision criterion, not a deferral. Fixed during review: I initially left Task 6 with a broken intermediate (deleting `Link`/`Citation`/`FmValue`/`ConceptRole` while `Concept`/`Bundle` still referenced them) — corrected in Task 6 Step 3 by folding the OKF-leaf re-export forward so every commit compiles.

**3. Type consistency** — names checked across tasks: the generated type names produced in Task 4's "Produces" (`Model`, `Node`, `Edge`, `RelEnd`, `TypeRef`, `Attribute`, `RelationshipKind`, `NoteAnchor`, `DiagramGroup`, `DiagramDisplay`, `Diagram`, `FlowDoc`+closure, `Concept`+leaves) are exactly the names consumed by Task 5 (overlay aliases), Task 6 (okf re-export), Task 7 (`Concept`/`Bundle`), and Task 9 (barrel). The override field types (`multiplicity: string`, `visibility?: "+" | "-" | "#" | "~"`, `name?: string | { ref: string }`, `type: string`, `layout: unknown[]`, `extra?: Record<string, FmValue>`) match the hand-written shapes in `packages/okf/src/types.ts` and `packages/core/src/state/overlay.ts` they replace. `RELATIONSHIP_KINDS`/`ENDED_KINDS` kept as runtime values while `RelationshipKind` becomes the re-exported type — verified the const's members equal the generated union so `ReadonlySet<RelationshipKind>` still type-checks. `build_model`/`validate`/`build_bundle`/`apply_ops` signatures are quoted verbatim from `crates/waml-wasm/src/lib.rs` with only the return/param type changed.
