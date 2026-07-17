# Ontology Substrate + Seam (Slice 1) — Plan Index

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Each task lives in its own file (`task-N-*.md`). Load **this README + the one task file you are executing** — nothing else. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce the ontology-agnostic substrate (`Node { key, label, kind }`, `Edge { source, target, kind }`, `Diagram { key, label, kind }`) with the `NodeKind`/`EdgeKind`/`DiagramKind` seam (`Uml(uml::…) | Unknown(String)`), moving all UML vocabulary — plus the OKF `Concept` off the node — behind the `Uml(..)` arm in a new `uml` module, and have **Rust own the wire projection** so TypeScript is a thin render skin.

**Architecture:** Two Rust tiers. (1) The **object model** (`model::{Node,Edge,Diagram}`) is skeleton + identity only (`key`/`label`/endpoints) plus an ontology discriminator (`kind`); all UML payload — classifier/structural fields, association ends, diagram render fields — lives behind `NodeKind::Uml(uml::UmlNode)` etc. Per spec §2 the OKF `Concept` does **not** ride on the object-model node. (2) A **Rust-owned wire projection** (`wire` module) joins the pure object model with the parse-time OKF projection into flat DTOs (`WireNode`/`WireEdge`/`WireDiagram`) that cross the wasm boundary. All flattening/defaulting/joining logic that today lives in TS (`overlay.ts`) moves into Rust incrementally; the TS bridge collapses to canvas-only overlay merge (position/handles/id).

**Tech Stack:** Rust (`waml`, `waml-editor`, `waml-cli`, `waml-wasm`, `waml-ops-dto` crates; `cargo`), TypeScript monorepo (`packages/`, `pnpm`), serde (externally-tagged enums) + tsify/wasm-bindgen codegen, egui native editor, Svelte frontend.

---

## Global Constraints

Every task's requirements implicitly include this section.

- **Rust owns the logic; TS is a skin.** No domain logic in TypeScript. The wire shape is a Rust-owned projection (`wire` module). TS reads flat DTOs and merges only canvas-only overlay state (node position, edge handles, synthetic id) — data that genuinely lives in TS and never touches the bundle. The hand-written `packages/okf/src/types.ts` app types are **not** a frozen contract — they follow the Rust wire shape. (Reverses the earlier draft's "app contract unchanged / only overlay.ts changes" constraint.)
- **`Concept` is off the object-model node (spec §2).** `okf::Concept` is a parse-time projection of storage, not object-model identity. The object-model `Node` holds only `key`/`label`/`kind`. `label` is the UML render title extracted at parse time (deliberately **not** `title`). The OKF-generic fields the UI needs (`description`/`tags`/`body`/`links`/`citations`/`role`/`extra`) reach the app via the **Rust wire projection**, never re-sourced in TS.
- **UML is a closed enum set with an `Unknown` escape hatch (spec §3, §8).** No property-bag, profile registry, or data-driven UML. `Unknown(String)` per axis is the only dynamism, at the ontology boundary. Exhaustive matches over metaclass/classifier sets stay written-out (no `_ =>` catch-all) so adding a variant forces a decision.
- **Frontmatter `type` tokens unchanged.** `ElementType::parse`/`as_str` still emit/accept `"uml.Class"`…`"uml.Sequence"`, `"Diagram"`, and opaque `Unknown` tokens verbatim. On-disk OKF markdown is untouched — `parse` is the only projection boundary. `ElementType` survives as the parse-time recognizer.
- **Ops are untouched (storage tier).** `waml::ops::Op` carries `ElementType`/`RelationshipKind` **tokens** and rewrites markdown storage. Ops never see `Node`/`Edge`/`NodeKind`/`uml::*`. The object-model reshape does not touch `crates/waml/src/ops/`.
- **Behavior + package coexistence.** `FlowDoc`/`SequenceDoc` (`Model.flows`/`Model.interactions`) and the `Model.packages` vec **stay on `Model` unchanged** for slice 1 — NOT collapsed into nodes/edges/diagrams (spec §9 slices 2–4). `Model.packages` reshapes to `Vec<Node>` (the new `Node`) but stays a separate vec; its wire projection stays a separate `WireNode[]`.
- **`is_classifier` is a `NodeKind` predicate.** Relocate from `ElementType` (delete the old method + its unit test) to `NodeKind::is_classifier` as a variant check mapping spec §3.1. Zero non-test callers today.
- **Idiomatic Rust.** `cargo fmt` on touched files before every commit; no new `cargo clippy` warnings on touched crates.
- **Full gate (from `.github/workflows/ci.yml`), in this order:** `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`. Each task ends green on the full gate.
- **Do NOT edit `docs/`** (specs + completed plans reference old shapes historically). Do NOT commit anything a task does not list. This plan directory stays uncommitted.
- **Windows/PowerShell env.** The Bash tool is Git Bash (POSIX). Use absolute paths.

---

## Shared Type Reference

Task files reference these by name. Full derive attributes + field-level serde live in the task that constructs the type (Task 1 for the `uml` module + seam enums; Tasks 2–4 for the reshaped substrate; Task 5 for the wire DTOs).

### `crate::uml` module (Task 1) — object-model UML payload

```rust
// Nodes (spec §3.1). UmlNode is an ENUM — Concept is NOT a field on it.
enum  UmlNode { Classifier(Classifier), Structural(Structural), Behavior(BehaviorElement), Lifeline(Lifeline) }
enum  ClassifierKind { Class, Interface, Enum, DataType, Association, Actor, UseCase }  // parse/name
struct Classifier { kind: ClassifierKind, stereotypes: Vec<String>, abstract_: bool, attributes: Vec<Attribute>, values: Vec<String> }
enum  Structural { Package { members: Vec<String> }, Note { body: Option<String>, annotates: Vec<NoteAnchor> } }
struct FlowBody { partition, entry, do_, exit, refines: Option<String> }              // slice 2
enum  PseudostateKind { Initial, Final, Decision, Merge, Fork, Join }                  // slice 2
enum  BehaviorElement { Action(FlowBody), State(FlowBody), Pseudostate(PseudostateKind), ObjectNode { object_ref }, Fragment { kind: FragmentKind }, Operand { guard } }  // slices 2–3
struct Lifeline { ref_: Option<String>, alias: Option<String> }                       // slice 3

// Edges (spec §3.2).
enum  UmlEdge { Relationship(Relationship), Transition(Transition), Message(Message), Containment }
struct Relationship { kind: RelationshipKind, name: Option<AssocName>, from_end: RelEnd, to_end: RelEnd, bidirectional: bool }
struct Transition { trigger, guard, is_else, effect, carries, to_ref }                 // slice 2
struct Message { verb: MessageVerb, signature: Option<String>, seq: u32 }              // slice 3

// Diagrams (spec §3.3).
enum  UmlDiagramFlavor { Class, Activity, StateMachine, Sequence, UseCase }
struct UmlDiagram { flavor: UmlDiagramFlavor, profile: String, description: Option<String>, groups: Vec<DiagramGroup>, display: DiagramDisplay, layout: Vec<syntax::LayoutStatement> }
```

Reused leaf types stay in `model.rs` unchanged: `Attribute`, `TypeRef`, `RelEnd`, `RelationshipKind`, `AssocName`, `NoteAnchor`, `FragmentKind`, `MessageVerb`, `DiagramGroup`, `DiagramDisplay`. `Concept`/`Link`/`Citation`/`ConceptRole` stay in `okf.rs`. `LayoutStatement` stays in `syntax`.

Slice 1 only ever **constructs** `Classifier`, `Structural::{Package,Note}`, `UmlEdge::Relationship(Relationship)`, and `UmlDiagram { flavor: Class }`. The `Behavior*`/`Lifeline`/`Transition`/`Message`/`Containment` variants are **defined** (so the seam is complete per spec §3) but constructed by later slices.

### `crate::model` seam enums (Task 1)

```rust
enum NodeKind    { Uml(uml::UmlNode),    Unknown(String) }   // + fn is_classifier(&self) -> bool
enum EdgeKind    { Uml(uml::UmlEdge),    Unknown(String) }
enum DiagramKind { Uml(uml::UmlDiagram), Unknown(String) }
```

### Reshaped object-model substrate (Tasks 2–4)

```rust
struct Node    { key: String, label: String, kind: NodeKind }        // Task 2 — Concept OFF
struct Edge    { source: String, target: String, kind: EdgeKind }    // Task 3
struct Diagram { key: String, label: String, kind: DiagramKind }     // Task 4
// Model keeps: nodes, edges, diagrams, path, packages (Vec<Node>), flows, interactions.
```

Object-model accessors (added alongside each reshape) let native readers stay mechanical: `Node::classifier()/attributes()/stereotypes()/values()/is_abstract()/members()/note_body()/annotates()`, `Edge::relationship()/rel_kind()/name()/from_end()/to_end()/bidirectional()`, `Diagram::flavor()/profile()/description()/groups()/display()/layout()`.

### Rust-owned wire projection (`crate::wire`, Task 5) — what crosses to TS

The object model reshapes to the nested §2/§3 shape (Tasks 2–4). But the **wasm wire is a Rust-owned FLAT projection**, deliberately field-compatible with today's JSON so the skin barely moves. Rust does the flatten (from the reshaped internal model + the retained OKF `Concept`); TS reads flat fields. Evolving the wire toward the nested honest shape later is a **Rust-only** change — the seam is already owned by Rust.

```rust
struct WireGraph { nodes: Vec<WireNode>, edges: Vec<WireEdge>, diagrams: Vec<WireDiagram>, path: String, packages: Vec<WireNode>, flows: Vec<FlowDoc>, interactions: Vec<SequenceDoc> }

// WireNode == today's ModelNode minus `position` (position is TS overlay state).
// serde field names match today's Node exactly (see Task 5 for the full derive).
struct WireNode {
    concept: Concept,          // the off-node OKF projection, re-attached on the wire (spec §2)
    key: String,
    ty: ElementType,           // serde rename "type" — flat token string, drives skin dispatch
    stereotypes: Vec<String>,
    abstract_: bool,           // serde rename "abstract"
    attributes: Vec<Attribute>,
    values: Vec<String>,
    note_body: Option<String>,
    annotates: Vec<NoteAnchor>,
    members: Vec<String>,
}

// WireEdge == today's Edge wire shape (flat, pre-reshape).
struct WireEdge { source: String /*"from"*/, target: String /*"to"*/, kind: RelationshipKind, name: Option<AssocName>, from_end: RelEnd /*"fromEnd"*/, to_end: RelEnd /*"toEnd"*/, bidirectional: bool }

// WireDiagram == today's Diagram wire shape, EXCEPT `members` is flattened from `groups` IN RUST
// (drops `flattenGroups` from overlay.ts — the one bit of TS logic that moves to Rust this slice).
struct WireDiagram { key: String, title: String, profile: String, description: Option<String>, members: Vec<String>, display: DiagramDisplay, layout: Vec<syntax::LayoutStatement> }
```

Rust builds each `Wire*` from the object model via the accessors (`Node::attributes()`, `Edge::rel_kind()`, `Diagram::flavor()`, etc.) plus the **re-joined `Concept`**: `build_wire(model: &Model, bundle: &okf::Bundle) -> WireGraph` looks up each node's concept by `bundle.concept(&node.key)` (invariant: `node.key == concept.id`, both = bundle path minus `.md`). Rust owns this join — TS never re-sources. Every `Wire*` field maps 1:1 to a field the scouted skin already reads — so no TS consumer changes its field access; only the imported wire *type names* change (`Model`→`WireGraph`, `Node`→`WireNode`, …) and `overlay.ts` drops `flattenGroups` (its members now arrive flat).

**Sequencing constraint (wire-first).** `waml-wasm::build_model` currently returns the object-model `Model` directly, so reshaping `Node`/`Edge`/`Diagram` would break the wire + TS *before* the wire module exists. Therefore the wire indirection is introduced **first** (Task 2), projecting today's unchanged `Model`; then Tasks 3–5 reshape the object model *behind* the stable wire, updating only `build_wire`'s internal projection each time. Every task keeps the wire JSON stable, so the TS gate stays green throughout and the only TS edit lands once in Task 2.

---

## Task Index

Dependency order (**wire-first** — see the sequencing constraint above). Each task ends green on the full gate and is an independently reviewable deliverable.

1. **`task-1-uml-module-and-seam.md`** — Create the `uml` module + `NodeKind`/`EdgeKind`/`DiagramKind` seam enums + `NodeKind::is_classifier`; delete `ElementType::is_classifier`. Standalone types, not yet wired. Rust-only. *(Foundation for 2–5.)*
2. **`task-2-wire-projection.md`** — Add the `wire` module (`WireGraph`/`WireNode`/`WireEdge`/`WireDiagram` + `build_wire(model, bundle)`) projecting today's **unchanged** `Model` into the flat wire; point `waml-wasm::build_model` at it; regenerate bindings; collapse `overlay.ts` (drop `flattenGroups`, read flat `diagram.members`). Wire JSON byte-identical to today **except** `diagram.members` flattened in Rust. *(Depends 1 for the seam types that later tasks will reshape behind this wire. Object model untouched.)*
3. **`task-3-reshape-node.md`** — Reshape `model::Node` → `{key,label,kind}`, Concept off; add accessors; rewrite `build_node` + package `Node` construction; update native readers (`waml-editor`, `waml-cli`) + `build_wire`'s `WireNode` projection (concept re-joined from bundle). Wire output unchanged. *(Depends 1, 2.)*
4. **`task-4-reshape-edge.md`** — Reshape `model::Edge` → `{source,target,kind}`; add accessors; rewrite `build_edges`; update native readers + `build_wire`'s `WireEdge` projection. Wire output unchanged. *(Depends 1, 2.)*
5. **`task-5-reshape-diagram.md`** — Reshape `model::Diagram` → `{key,label,kind}`; add accessors; rewrite `build_diagrams`; update native readers + `build_wire`'s `WireDiagram` projection. Wire output unchanged. *(Depends 1, 2.)*

Tasks 3–5 each reshape one object-model type **behind** the stable wire and keep `build_wire` emitting identical JSON, so `cargo test --workspace` and the TS gate stay green throughout. The lone intentional TS edit lands once, in Task 2.

---

## Execution Handoff

After the plan pieces are complete, execute via **subagent-driven-development** (fresh subagent per task, two-stage review) — each subagent loads this README + one task file. Alternatively **executing-plans** inline with checkpoints.
