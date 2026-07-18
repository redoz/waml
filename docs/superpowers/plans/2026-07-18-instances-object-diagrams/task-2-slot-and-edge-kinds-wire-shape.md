# Instances + Object Diagrams — Task 2: `Slot`, `Node.slots`, and `InstanceOf`/`Links` edge kinds (wire shape)

> **Segment 2 of 6** of the **Instances + Object Diagrams** plan (slug `instances-object-diagrams`). See [`README.md`](README.md) for Goal, Global Constraints, File Structure, and the full plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

### Task 2: Slot, Node.slots, and InstanceOf/Links edge kinds (wire shape + binding + TS)

Add every **tsify-visible runtime type** this slice needs, in ONE green commit: the runtime `Slot` struct, the `Node.slots` pool, and the two new `RelationshipKind` variants. The checked-in `packages/wasm/src/generated/waml_wasm.d.ts` couples the Rust shape to its TS consumers, so a Rust-only intermediate would leave the committed binding stale and fail `pnpm build`. This task therefore lands Rust + the regenerated binding + the TS barrels + the `ModelGraph` mapping together. Nothing populates `slots` or emits the new edge kinds yet (Tasks 3–5 do); this task only fixes the SHAPE.

**Why one task (not independently splittable):** the generated binding is the coupling seam. This is the single "wire-shape" commit of the slice; Tasks 3–5 are pure Rust logic against this shape (no further binding regen), and Task 6 does the one other binding regen (for the new `DiagCode`s).

**Guardrails (from the design spec):**
- `Slot` mirrors `Attribute`: serde + tsify derived; `ref_` serialized as `ref` with `skip_serializing_if = "Option::is_none"`.
- `Node.slots: Vec<Slot>` with `skip_serializing_if = "Vec::is_empty"` (empty for non-instances) — mirrors `Node.values`.
- The classifier reference is NOT a `Node` field — it lands as an `InstanceOf` edge (Task 4). Do not add a `type`/`classifier` field to `Node`.
- `RelationshipKind::{InstanceOf, Links}` are ordinary edge kinds in `Model.edges`. `is_ended()` is `false` for both (they take no `: near to far` ends). `Links` carries its optional Association via the existing `name: Option<AssocName>` — do NOT add a new field.
- Do NOT modify the TS class-relationship authoring lists `RELATIONSHIP_KINDS` / `ENDED_KINDS` in `packages/okf/src/types.ts`, and do NOT touch `packages/okf/src/grammar.ts`. `instanceof`/`links` surface additively through the generated `RelationshipKind` union; the class-diagram authoring UI is a separate (future) concern. This keeps `packages/core/src/templates/templates.test.ts` (which asserts every template edge kind is in `RELATIONSHIP_KINDS`) green — templates carry no instance edges.
- Do NOT hand-edit the generated files.

**Files:**
- Modify: `crates/waml/src/model.rs` — add `struct Slot`; add `Node.slots`; add `RelationshipKind::{InstanceOf, Links}` with `as_str`/`parse` arms; extend the `relationship_kind_round_trips` test.
- Modify: `crates/waml/src/parse.rs` — add `slots: Vec::new()` to the `build_node` (`fn build_node`) and `build_packages` `Node { … }` literals.
- Test: `crates/waml/tests/serde_shape.rs` — add `slots: vec![]` to the two `Node { … }` literals (`package_node_and_model_path`), and add wire-shape tests for `Slot` and the new kinds.
- Modify (regenerated, do NOT hand-edit): `packages/wasm/src/generated/waml_wasm.d.ts` (+ `.js`) — via `pnpm build:wasm`.
- Modify: `packages/wasm/src/index.ts` — re-export `Slot`.
- Modify: `packages/okf/src/types.ts` — re-export `Slot`; add `ModelNode.slots?: Slot[]`.
- Modify: `packages/core/src/state/overlay.ts` — map `n.slots` in `toModelGraph`'s `toNode`.

**Interfaces:**
- Produces (Rust, `crate::model`):
  - `pub struct Slot { pub name: String, pub value: String, pub ref_: Option<String> }` (serde `ref`, skip-if-none; tsify).
  - `Node.slots: Vec<Slot>` (skip-if-empty).
  - `RelationshipKind::InstanceOf` (`as_str() == "instance of"`, wire `"instanceof"`), `RelationshipKind::Links` (`as_str() == "links"`, wire `"links"`); both `is_ended() == false`.
- Produces (TS): `Slot = { name: string; value: string; ref?: string }`; `Node.slots?: Slot[]`; `RelationshipKind` union gains `"instanceof" | "links"`; `ModelNode.slots?: Slot[]`.
- Consumes: existing `Attribute` (as the mirror), `AssocName` (reused by `Links`).

### Phase A — Rust runtime shape (TDD)

- [ ] **Step 2.1: Write the failing wire-shape tests.** In `crates/waml/tests/serde_shape.rs`, add (import `Slot` and `RelationshipKind` into the `use waml::model::{…}` line):

```rust
#[test]
fn slot_serializes_with_ref_key_and_skips_none() {
    use waml::model::Slot;
    let bare = Slot { name: "id".into(), value: "ORD-42".into(), ref_: None };
    let v = serde_json::to_value(&bare).unwrap();
    assert_eq!(v["name"], "id");
    assert_eq!(v["value"], "ORD-42");
    assert!(v.get("ref").is_none(), "None ref must be omitted: {v}");

    let linked = Slot { name: "customer".into(), value: "Ann".into(), ref_: Some("m/ann".into()) };
    assert_eq!(serde_json::to_value(&linked).unwrap()["ref"], "m/ann");
}

#[test]
fn instance_edge_kinds_serialize_lowercase() {
    use waml::model::RelationshipKind;
    assert_eq!(
        serde_json::to_value(RelationshipKind::InstanceOf).unwrap(),
        serde_json::json!("instanceof")
    );
    assert_eq!(
        serde_json::to_value(RelationshipKind::Links).unwrap(),
        serde_json::json!("links")
    );
    // Markdown verb (as_str) keeps the authored spelling.
    assert_eq!(RelationshipKind::InstanceOf.as_str(), "instance of");
    assert_eq!(RelationshipKind::Links.as_str(), "links");
    assert!(!RelationshipKind::InstanceOf.is_ended());
    assert!(!RelationshipKind::Links.is_ended());
}

#[test]
fn classifier_node_omits_empty_slots() {
    // A plain class must omit `slots` entirely (skip-if-empty, mirrors values).
    let m = build_model(&bundle());
    let v = serde_json::to_value(&m).unwrap();
    assert!(v["nodes"][0].get("slots").is_none(), "empty slots must be omitted: {}", v["nodes"][0]);
}
```

- [ ] **Step 2.2: Run them, verify they fail to compile.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape
  ```
  Expected: FAIL to compile — `Slot`, `RelationshipKind::InstanceOf`/`Links` do not exist yet.

- [ ] **Step 2.3: Add the `Slot` struct.** In `crates/waml/src/model.rs`, immediately after the `Attribute` struct, add:

```rust
/// A slot value on an `InstanceSpecification` (design spec §3.2): a named value
/// that stands in for a classifier attribute, rather than declaring one. Mirrors
/// `Attribute` for serde/tsify.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Slot {
    pub name: String,
    pub value: String,
    /// Set when the slot value resolves to another pool element (an
    /// instance-valued slot); a display token otherwise.
    #[cfg_attr(
        feature = "serde",
        serde(rename = "ref", default, skip_serializing_if = "Option::is_none")
    )]
    pub ref_: Option<String>,
}
```

- [ ] **Step 2.4: Add `Node.slots`.** In `pub struct Node`, add after the `members` field:

```rust
    /// Slot values on an `InstanceSpecification` node (design spec §3.3). Empty
    /// on every non-instance node.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub slots: Vec<Slot>,
```

- [ ] **Step 2.5: Add the two `RelationshipKind` variants.** In `pub enum RelationshipKind`, add after `Extends`:

```rust
    InstanceOf,
    Links,
```

  In `RelationshipKind::as_str`, add (markdown verbs — `InstanceOf` keeps its authored two-word spelling):

```rust
            RelationshipKind::InstanceOf => "instance of",
            RelationshipKind::Links => "links",
```

  In `RelationshipKind::parse`, add:

```rust
            "instance of" => Some(RelationshipKind::InstanceOf),
            "links" => Some(RelationshipKind::Links),
```

  `is_ended()` is a closed `matches!(… Associates | Aggregates | Composes)`, so `InstanceOf`/`Links` are already `false` — leave it unchanged.

- [ ] **Step 2.6: Extend `relationship_kind_round_trips` (model.rs test).** Add `RelationshipKind::InstanceOf,` and `RelationshipKind::Links,` to the array iterated by the test so `parse(as_str())` round-trips both.

- [ ] **Step 2.7: Add `slots` to every `Node { … }` literal.** Add `slots: Vec::new(),` to `build_node` (`crates/waml/src/parse.rs`, `fn build_node`) and `slots: vec![],` to `build_packages`'s `Node { … }` literal. Add `slots: vec![],` to the two literals in `crates/waml/tests/serde_shape.rs` (`package_node_and_model_path`) and the one in `crates/waml/src/model.rs` tests (search: `grep -rn "Node {" crates/waml/src crates/waml/tests` — 5 literals total; do NOT touch `ErrorNode`/`ActivityNode`/`SeqNode`/`SceneNode`).

- [ ] **Step 2.8: Run Rust tests + workspace, verify green.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape
  cargo test --workspace
  cargo clippy -p waml --all-targets
  cargo fmt
  ```
  Expected: all green; the three new serde_shape tests pass; existing round-trip/validate tests unaffected (no parser/grammar change yet).

### Phase B — Regenerate the wasm binding

- [ ] **Step 2.9: Regenerate the binding.** Run:
  ```
  pnpm build:wasm
  git diff packages/wasm/src/generated/waml_wasm.d.ts
  ```
  Expected new/changed exports:
  ```ts
  export interface Slot { name: string; value: string; ref?: string }
  export interface Node { …; members?: string[]; slots?: Slot[] }
  export type RelationshipKind = "associates" | … | "extends" | "instanceof" | "links";
  ```
  Do NOT hand-edit; if wrong, fix Phase A and rerun.

### Phase C — TS type barrels + ModelGraph

- [ ] **Step 2.10: Re-export `Slot` from `@waml/wasm`.** In `packages/wasm/src/index.ts`, in the `export type { … } from "./generated/waml_wasm.js";` block, add `Slot,` next to `Attribute,`.

- [ ] **Step 2.11: Re-export `Slot` from `@waml/okf` and add `ModelNode.slots`.** In `packages/okf/src/types.ts`:
  - In the `export type { … } from "@waml/wasm";` block, add `Slot,` next to `Attribute,`.
  - In the `import type { … } from "@waml/wasm";` block, add `Slot,` next to `Attribute,`.
  - In `interface ModelNode`, add after `attributes: Attribute[];`:
    ```ts
      /** Slot values on a uml.InstanceSpecification node (design spec §3.3). Absent on non-instances. */
      slots?: Slot[];
    ```

- [ ] **Step 2.12: Map `slots` in `toModelGraph`.** In `packages/core/src/state/overlay.ts`, in `toNode`, add after the `attributes` line, mirroring the `values` guard:

```ts
    ...(n.slots && n.slots.length ? { slots: n.slots } : {}),
```

### Phase D — Full gate + commit

- [ ] **Step 2.13: Run the rest of the gate, verify green.** Run in order:
  ```
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green. `pnpm lint` (tsc + eslint) passes — `ModelNode.slots?` and the `Slot` re-exports type-check; `pnpm build` compiles `@waml/okf`; `pnpm -r test` passes, including `templates.test.ts` (no instance edges in templates ⇒ `RELATIONSHIP_KINDS` unchanged is fine).

- [ ] **Step 2.14: Commit.** Run:
  ```
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/waml_wasm.js packages/wasm/src/index.ts packages/okf/src/types.ts packages/core/src/state/overlay.ts
  git commit -F - <<'EOF'
  feat(model): add Slot, Node.slots, and instance-of/links edge kinds

  Plan: instances-object-diagrams
  Plan-Tasks: Task 2
  EOF
  ```
  (Add any other `packages/wasm/src/generated/*` files `pnpm build:wasm` regenerated — e.g. `wasm-inline` — as reported by `git status`.)
