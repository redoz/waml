# Instances + Object Diagrams — Design (UML element-model slice 3)

> **Slice:** sequel slice 3 of the UML Element Model
> ([`2026-07-17-uml-element-model-domain-design.md`](./2026-07-17-uml-element-model-domain-design.md) §5, §7.3).
> **Predecessors landed:** slice 1 (element-pool rename, `is_classifier()`),
> slice 2 (behavior model/view split), sequence-flat-model. This slice assumes
> all of them are on `main`.
> **Scope:** model + validation + generated bindings. **No renderer.**

## 1. Motivation

`InstanceSpecification` is the UML metaclass behind an "object" on an object
diagram: an entity that is an *instance of* a classifier, carrying **slot**
values rather than owning attributes/operations. A **link** is the object-diagram
counterpart to an Association — an edge that is "an instance of an Association,"
connecting two instances. Sparx EA keeps objects in `t_object` alongside classes;
waml keeps them as one more element kind in the same pool (spec §5).

This slice lands the whole data model + conformance validation so that later work
(slice 4, instances-as-lifelines) and a future object-diagram renderer have a
stable shape to build on. It deliberately stops short of rendering.

## 2. Scope

**In scope**
- Rust model: `InstanceSpecification` metaclass, `Slot`, `RelationshipKind::{InstanceOf, Links}`.
- Two authoring front-doors (standalone doc + inline-in-diagram), both producing
  pool `InstanceSpecification` nodes.
- Grammar parse + serialize (deterministic round-trip) for the new syntax.
- Slot-conformance + classifier-target validation — **warn-only**.
- Regenerated wasm binding and the TS consumers coupled to it.

**Out of scope (guards)**
- **No renderer.** Object diagrams parse into the model; canvas rendering is a
  separate follow-up slice.
- **§7.4 instances-as-lifelines stays out.** `Lifeline.ref_` is untouched and
  remains classifier-only. Do NOT widen its target to instances here.
- **Inline is instances-only.** Inlining an arbitrary element (a whole class,
  etc.) into a diagram is a broader, separate idea — noted as future, not built.
- **No storage-format redesign of untouched layers** (spec §9). Existing syntax
  and round-trip for classes/behaviors/sequences are unchanged; only additive
  new productions are introduced.

## 3. Model

`crates/waml/src/model.rs`.

### 3.1 Metaclass

- Add `UmlMetaclass::InstanceSpecification`. Token: `uml.InstanceSpecification`.
- `is_classifier()` returns `false` for it — an instance is NOT a classifier
  (spec §3.1). The explicit (no `_ =>`) match forces the decision at compile time.

### 3.2 Slot

```rust
pub struct Slot {
    pub name: String,
    pub value: String,
    /// Set when the slot value resolves to another element (e.g. an
    /// instance-valued slot). A display token otherwise.
    pub ref_: Option<String>,
}
```
serde + tsify derived like `Attribute`. `ref_` serialized as `ref`,
`skip_serializing_if = "Option::is_none"`.

### 3.3 Node

`Node` gains:
```rust
pub slots: Vec<Slot>,   // empty for non-instances; skip_serializing_if empty
```
The classifier reference is NOT a Node field — it lands as an `InstanceOf` edge
(§3.4), keeping the "every relationship is a typed edge" substrate uniform
(spec §3).

### 3.4 Edges

`RelationshipKind` gains two variants (with `as_str`/parse arms; explicit matches
updated):
- `InstanceOf` — `source` = instance key, `target` = classifier key.
- `Links` — `source`/`target` = instance keys; `name: Option<AssocName>` carries
  the Association it instantiates (reuses the existing `{ ref }`-capable
  `AssocName`).

Both are ordinary `Edge`s in `Model.edges`. No separate pool, no separate type.

## 4. Syntax

### 4.1 Standalone instance doc

```markdown
---
type: uml.InstanceSpecification
title: order42
---
# order42

## Relationships
- instance of [Order](./order.md)

## Slots
- id: "ORD-42"
- status: PLACED
```

- `## Relationships` accepts the new `instance of [ref]` verb line (and `links`).
- New `## Slots` section: `- name: value` lines, parsed to `Slot`.

### 4.2 Inline instance (in a Diagram's `## Members`)

```markdown
## Members
- instance of [Order](./order.md) as order42 with id set to "ORD-42" and status set to PLACED
```

- Promoted to a pool `Node` keyed `{diagram}#order42` (same mechanism
  `build_flows` uses for inline flow nodes), auto-added to that diagram's members.
- Slots parsed from the `with … set to … and …` clause.

### 4.3 Link

```markdown
## Relationships
- links [order42-line](./order42-line.md) as [Order→OrderLine](./order-orderline-assoc.md)
```
Authored on the source instance doc (mirrors class `## Relationships`). The
`as [Assoc]` names which Association is instantiated (optional).

### 4.4 Controlled-prose parse rule

Reads as prose to an LLM, parses deterministically:
- Reserved tokens: `instance of`, `with`, `set to`, `and`, `as`, `links`.
- A slot **value** is one of: a quoted string (`"ORD-42"`), a bare
  identifier/number (`PLACED`, `3`), or a `[Label](ref)` link (instance-valued
  slot → `Slot.ref_`).
- A bare value may not contain a reserved word; quote it if it must.
- **Canonical serialize** joins slots with ` and ` (one canonical form) so
  parse → serialize is byte-identical.

`crates/waml/src/syntax.rs`, `grammar.rs`, `parse.rs`, `serialize.rs`.

## 5. Validation

`crates/waml/src/validate.rs`. All three are **warnings** (uniform with waml's
tolerant `Unknown`-type / OKF §9 posture — object diagrams are often sketched
before their classifiers' attributes settle). New `DiagCode`s:

- `SlotUnknownAttribute` — slot name is not an attribute of the referenced
  classifier.
- `InstanceOfNonClassifier` — the `instance of` target's `is_classifier()` is
  false (includes pointing at another instance — "you do not instantiate an
  instance," spec §5).
- `InstanceOfUnresolved` — the classifier ref is dangling.

## 6. Generated bindings + TS

- `pnpm build:wasm` regenerates `packages/wasm/src/generated/waml_wasm.d.ts`
  (+ `.js`); `Slot`, the `InstanceSpecification` token, and the new edge kinds
  surface through Tsify. **No hand-edits to generated files.**
- `ModelGraph` / `toModelGraph` (TS) carry `slots` on nodes; instance/link edges
  flow through the existing edge plumbing **additively** — no renames, no
  collisions with behavior/sequence pools.
- `packages/okf` / `packages/core` re-export regions extended additively.

## 7. Atomicity

One green vertical: the Rust runtime shape, the regenerated wasm binding it
produces, and the TS consumers that read it are coupled through
`waml_wasm.d.ts`, so they land in one green commit — same shape as the behavior
and sequence slices.

## 8. Testing

- **serde-shape round-trip** for a standalone instance doc and an inline-in-diagram object.
- **grammar round-trip** (parse → serialize identity) for `## Slots`, the inline
  `with … set to …` clause, and `links`.
- **validation** tests exercising each of the three warn codes.
- **TS type-surface** test asserting `Slot` and the new edge kinds appear in the
  generated binding.

## 9. Cross-slice contract

- Flows are pooled, the interaction stays inline, and instances are pooled (this
  slice) — all consistent with the substrate (spec §3, §4, §5). `is_classifier()`
  (slice 1) is the single classifier predicate; `InstanceSpecification` is a pool
  member for which it returns `false`.
- `Lifeline.ref_` is untouched (classifier-only); widening it to instances is
  slice 4 (§7.4), explicitly deferred.
