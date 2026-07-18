# Behavior Model/View Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reverse today's "behavior doc IS model AND view." Activity/state-machine flow *elements* become reusable model-level pool members (`ActivityNode`); transitions become typed model-level pool edges (`FlowEdge`, discriminated `ControlFlow` / `ObjectFlow`); and `FlowDoc` becomes a **view** that references those pool members by key — exactly as a class `Diagram` references pooled classifiers by `members`.

**Architecture:** Today `Model.flows: Vec<FlowDoc>` and each `FlowDoc` owns its `nodes: Vec<FlowNode>` / `edges: Vec<FlowEdge>` inline (model AND view). This plan introduces two model-level pools — `Model.activity_nodes: Vec<ActivityNode>` and `Model.flow_edges: Vec<FlowEdge>` — populated during `build_model`. `FlowDoc.nodes` / `FlowDoc.edges` become `Vec<String>` pool keys (`"{behavior}#{localId}"` for nodes, `"{behavior}#e{n}"` for edges). Cross-document transition semantics (`to_ref` = target behavior key, unresolved link labels not drawn) are preserved verbatim. The markdown storage format (grammar/syntax/serialize) is UNTOUCHED — this is a runtime-model reshape only (design spec §9: storage and runtime need not be 1:1). The two node-pool filters that special-cased `!Behavior(_)` are unified behind one honest predicate, `ElementType::is_view()`. The TypeScript renderers rebuild against the new pool/view shape in the same series.

**Tech Stack:** Rust (`waml` crate, `cargo`, serde, tsify-next/wasm-bindgen codegen), TypeScript monorepo (`packages/`, `pnpm`, `vitest`), `@dagrejs/dagre` + `@xyflow/svelte` flow renderer.

## Global Constraints

- **Storage format is frozen.** Do NOT touch `crates/waml/src/grammar.rs`, `crates/waml/src/syntax.rs`, or `crates/waml/src/serialize.rs`. The markdown ↔ AST round-trip (the `flow_document_serialize_is_a_semantic_fixpoint` test) must stay byte-identical. This slice reshapes only the RESOLVED runtime `Model` (design spec §9).
- **Do NOT touch the sequence/interaction substrate.** `SequenceDoc`, `Lifeline`, `SeqItem`, `SeqOperand`, `MessageVerb`, `FragmentKind`, `build_interactions`, and `Model.interactions` are a SEPARATE plan being written in parallel. Leave them exactly as they are.
- **`ops` is out of scope for flow.** `crates/waml/src/ops/mod.rs` has zero flow references (verified) — activity nodes/edges are derived-only, never mutated by `Op`s. Do not add flow ops.
- **`FlowEdgeKind` and `is_view()` matches MUST be exhaustive and explicit** — no `_ =>` catch-all where a metaclass/kind decision should be forced at compile time.
- **`is_view()` replaces both `!= Diagram && !Behavior(_)` filters** (in `parse.rs::build_model` and `validate.rs::link`). `is_view()` returns `true` for `Diagram` and every `Behavior(_)`, so the filter behavior is identical — this is a pure clarity refactor. Do NOT confuse `is_view()` with `is_classifier()` (which returns `true` for behaviors).
- **Wire naming:** new multi-word wire fields use camelCase via `#[serde(rename = ...)]` — `activityNodes`, `flowEdges`, `objectRef`, `toRef`, `controlFlow` / `objectFlow`. This matches the existing camelCase wire conventions (`objectRef`, `toRef` already exist on the old shapes).
- Idiomatic Rust: run `cargo fmt` on touched files before every commit; introduce no new `cargo clippy` warnings on the `waml` crate.
- Full CI gate (from `.github/workflows/ci.yml`), in order: `cargo test --workspace` → `pnpm build:wasm` → `pnpm lint` → `pnpm build` → `pnpm -r test`.
- **Cross-language atomicity note:** the runtime-model wire SHAPE changes, so Rust and TypeScript cannot both be green on the *full* gate at every intermediate commit. Each task runs the per-language gate that covers its own change (stated per task); the FULL gate is green only at the end of **Task 4**. This is expected for a feature branch and mirrors how a shape change lands.
- Do NOT edit files under `docs/` (historical specs/plans reference the old shapes — leave them).
- Frequent commits, one deliverable per task.

---

## File Structure

Modified files:

- `crates/waml/src/model.rs` — **Task 1**: add `ElementType::is_view()` + unit test. **Task 2**: add `ActivityNode` struct, `FlowEdgeKind` enum, reshape `FlowEdge` (add `key`/`kind`/`behavior`; `from`/`to` become pool keys), reshape `FlowDoc` (`nodes`/`edges` → `Vec<String>`), add `Model.activity_nodes` / `Model.flow_edges`.
- `crates/waml/src/parse.rs` — **Task 1**: unify the `build_model` classifier filter behind `is_view()`. **Task 2**: rewrite `build_flows` to return `(Vec<FlowDoc>, Vec<ActivityNode>, Vec<FlowEdge>)` and distribute into the new `Model` fields; rewrite the `builds_flow_doc_with_resolved_links_and_edges` test.
- `crates/waml/src/validate.rs` — **Task 1**: unify the `link` keyset filter behind `is_view()`.
- `packages/wasm/src/generated/waml_wasm.d.ts` — **Task 2**: REGENERATED by `pnpm build:wasm` (never hand-edited).
- `packages/wasm/src/index.ts` — **Task 2**: re-export list: `FlowNode` → `ActivityNode`, add `FlowEdgeKind`.
- `packages/okf/src/types.ts` — **Task 3**: re-export `ActivityNode`/`FlowEdgeKind` (drop `FlowNode`); add `activityNodes?`/`flowEdges?` to `ModelGraph`.
- `packages/core/src/state/overlay.ts` — **Task 3**: `toModelGraph` passes the two new pools through.
- `packages/core/src/state/overlay.test.ts` — **Task 3**: update the flow-passthrough fixture to the new view+pool shape.
- `packages/web/src/canvas/flowGraph.ts` — **Task 4**: `flowNodeSize`/`transitionLabel`/`KIND_TO_TYPE` retype to `ActivityNode`; add `resolveFlow(doc, graph)`; `flowToRf` takes a resolved view and keys nodes by pool key.
- `packages/web/src/canvas/flowGraph.test.ts` — **Task 4**: rebuild against the resolved-view shape; add a `resolveFlow` test.
- `packages/web/src/components/canvas/flow/FlowView.svelte` — **Task 4**: take a `graph` prop; resolve then render.
- `packages/web/src/components/canvas/flow/FlowView.test.ts` — **Task 4**: pass `graph`.
- `packages/web/src/components/canvas/flow/FlowStepNode.svelte`, `FlowControlNode.svelte`, `FlowObjectNode.svelte` — **Task 4**: `FlowNode` type → `ActivityNode`.
- `packages/web/src/components/canvas/CanvasInner.svelte` — **Task 4**: pass `graph={$model}` to `FlowView`.

No files are created. No files are deleted.

---

## Task 1: Unify the node-pool filter behind `ElementType::is_view()`

Add the honest predicate the "behavior split" slice was waiting on, and route both `!= Diagram && !Behavior(_)` filters through it. Behavior is unchanged (`is_view()` ≡ `Diagram | Behavior(_)`), so this is a pure clarity refactor and the full gate stays green.

**Files:**
- Modify: `crates/waml/src/model.rs` — add `is_view()` inside `impl ElementType` (after `is_classifier`, ~line 661); add a unit test in `mod tests`.
- Modify: `crates/waml/src/parse.rs` — `build_model` filter (~lines 641-646).
- Modify: `crates/waml/src/validate.rs` — `link` keyset filter (~line 160).

**Interfaces:**
- Produces: `pub fn is_view(&self) -> bool` on `enum ElementType`.
- Consumes: existing `ElementType::{Diagram, Behavior}`, `BehaviorKind`, `UmlMetaclass`.

Steps:

- [ ] **1.1 Write the failing predicate test.** In `crates/waml/src/model.rs`, inside `#[cfg(test)] mod tests`, immediately after the `is_classifier_matches_spec_table` test (after its closing `}`), add:
  ```rust
      #[test]
      fn is_view_flags_diagrams_and_behaviors() {
          // Views / notation — never pooled classifiers, never link targets.
          assert!(ElementType::Diagram.is_view());
          assert!(ElementType::Behavior(BehaviorKind::Activity).is_view());
          assert!(ElementType::Behavior(BehaviorKind::StateMachine).is_view());
          assert!(ElementType::Behavior(BehaviorKind::Sequence).is_view());
          // Pool members (classifiers, notes, unknowns) are not views.
          assert!(!ElementType::Uml(UmlMetaclass::Class).is_view());
          assert!(!ElementType::Uml(UmlMetaclass::Note).is_view());
          assert!(!ElementType::Uml(UmlMetaclass::Package).is_view());
          assert!(!ElementType::Unknown("bpmn.Task".to_string()).is_view());
      }
  ```

- [ ] **1.2 Run it, verify it fails.** Run:
  ```
  cargo test -p waml is_view
  ```
  Expected: compile error `no method named `is_view` found for enum `ElementType``.

- [ ] **1.3 Implement `is_view()`.** In `crates/waml/src/model.rs`, inside `impl ElementType { ... }`, immediately after the `is_classifier` method's closing brace (~line 661) and before the impl block's closing brace, add:
  ```rust
      /// True for element types that are **views / notation**, not pooled
      /// classifiers: `Diagram` (a class-diagram view) and every behavior kind
      /// (activity / state machine / interaction — each a view over pooled
      /// behavior elements, design spec §4). A view never contributes a
      /// classifier `Node` to `Model.nodes` and is never a relationship/link
      /// target. Distinct from `is_classifier()`, which is `true` for behaviors.
      pub fn is_view(&self) -> bool {
          matches!(self, ElementType::Diagram | ElementType::Behavior(_))
      }
  ```

- [ ] **1.4 Run the predicate test, verify pass.** Run:
  ```
  cargo test -p waml is_view
  ```
  Expected: `test model::tests::is_view_flags_diagrams_and_behaviors ... ok`.

- [ ] **1.5 Route the `build_model` classifier filter through `is_view()`.** In `crates/waml/src/parse.rs`, replace the filter closure in `build_model` (currently):
  ```rust
          .filter(|p| {
              p.ty != ElementType::Diagram
                  && !matches!(p.ty, ElementType::Behavior(_))
                  && p.slug != "index"
                  && p.slug != "log"
          })
  ```
  with:
  ```rust
          .filter(|p| !p.ty.is_view() && p.slug != "index" && p.slug != "log")
  ```

- [ ] **1.6 Route the `link` keyset filter through `is_view()`.** In `crates/waml/src/validate.rs`, in `pub fn link`, replace:
  ```rust
          if *ty != ElementType::Diagram && !matches!(ty, ElementType::Behavior(_)) {
              keyset.insert(slug);
          }
  ```
  with:
  ```rust
          if !ty.is_view() {
              keyset.insert(slug);
          }
  ```

- [ ] **1.7 Run the full gate, verify green.** Run in order and confirm each passes (behavior is unchanged, so every existing test — including the flow/interaction resolution and validation tests — stays green):
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green, no wasm/TS diff.

- [ ] **1.8 Format, then commit.** Run:
  ```
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/validate.rs
  git commit -m "refactor(model): unify node-pool filter behind ElementType::is_view"
  ```

---

## Task 2: Split behavior model from view (Rust + wasm bindings)

Reshape the model types so activity nodes and flow edges live in model-level pools and `FlowDoc` is a pure view; rewrite `build_flows` to populate the pools; regenerate the wasm bindings and update the wasm package's re-exports. This is atomic: the Rust type change, the `build_flows` rewrite, the test rewrite, and the wasm re-export edit must land together or the `waml` crate / wasm package will not compile.

**Files:**
- Modify: `crates/waml/src/model.rs` — replace `FlowNode` with `ActivityNode`; add `FlowEdgeKind`; reshape `FlowEdge`; reshape `FlowDoc`; add `Model.activity_nodes` / `Model.flow_edges`.
- Modify: `crates/waml/src/parse.rs` — rewrite `build_flows`; update `build_model`'s `Model { .. }` literal; rewrite the flow-build test.
- Modify: `packages/wasm/src/index.ts` — re-export list.
- Regenerate: `packages/wasm/src/generated/waml_wasm.d.ts`.

**Interfaces:**
- Produces (Rust):
  - `pub struct ActivityNode { key: String, id: String, behavior: String, kind: FlowNodeKind, object_ref: Option<String>, partition: Option<String>, entry: Option<String>, do_: Option<String>, exit: Option<String>, refines: Option<String>, notes: Vec<String> }`
  - `pub enum FlowEdgeKind { ControlFlow, ObjectFlow }`
  - `pub struct FlowEdge { key: String, kind: FlowEdgeKind, behavior: String, from: String, to: String, to_ref: Option<String>, trigger: Option<String>, guard: Option<String>, is_else: bool, effect: Option<String>, carries: Option<String> }`
  - `pub struct FlowDoc { key: String, title: String, flavor: FlowFlavor, describes: Option<String>, nodes: Vec<String>, edges: Vec<String> }`
  - `Model.activity_nodes: Vec<ActivityNode>` (wire `activityNodes`), `Model.flow_edges: Vec<FlowEdge>` (wire `flowEdges`)
  - `fn build_flows(parsed, keyset) -> (Vec<FlowDoc>, Vec<ActivityNode>, Vec<FlowEdge>)`
- Produces (TS, generated): `interface ActivityNode`, `type FlowEdgeKind = "controlFlow" | "objectFlow"`, reshaped `FlowEdge` / `FlowDoc`, `Model.activityNodes?` / `Model.flowEdges?`.
- Consumes: `FlowNodeKind`, `FlowFlavor`, `BehaviorKind`, `is_false` helper — all unchanged.

Steps:

- [ ] **2.1 Rewrite the flow-build test to the new pool/view shape (RED).** In `crates/waml/src/parse.rs`, replace the whole `builds_flow_doc_with_resolved_links_and_edges` test (currently ~lines 1386-1412) with:
  ```rust
      #[test]
      fn builds_flow_view_and_pools_with_resolved_links_and_edges() {
          use crate::model::{FlowEdgeKind, FlowFlavor, FlowNodeKind};
          let b = vec![
              ("m/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
              ("m/sub.md".into(), "---\ntype: uml.Activity\ntitle: Sub\n---\n# Sub\n\n## Nodes\n\n### initial\n- transitions to final\n\n### final\n".into()),
              ("m/lifecycle.md".into(),
               "---\ntype: uml.StateMachine\ntitle: Order Lifecycle\ndescribes: [Order](./order.md)\n---\n# Order Lifecycle\n\n## Nodes\n\n### initial\n- transitions to Draft\n\n### Draft\n- on `place` when `items > 0` transitions to Placed: `reserve`\n- partition: Sales\n\n### Placed\n- entry: `reserveStock`\n- refines [Sub](./sub.md)\n- transitions to Ship carries [Order](./order.md)\n\n### Ship\n- transitions to final\n\n### final\n".into()),
          ];
          let m = build_model(&b);
          // Two behavior VIEWS.
          assert_eq!(m.flows.len(), 2);
          let f = m.flows.iter().find(|f| f.key == "m/lifecycle").unwrap();
          assert_eq!(f.flavor, FlowFlavor::StateMachine);
          assert_eq!(f.describes.as_deref(), Some("m/order"));
          // The view references pooled nodes/edges by key (no inline objects).
          assert_eq!(f.nodes.len(), 5);
          assert_eq!(f.nodes[0], "m/lifecycle#initial");
          assert_eq!(f.edges.len(), 4);
          // Activity nodes live in the model-level pool.
          let node = |key: &str| m.activity_nodes.iter().find(|n| n.key == key).unwrap();
          assert_eq!(node("m/lifecycle#initial").kind, FlowNodeKind::Initial);
          assert_eq!(node("m/lifecycle#initial").behavior, "m/lifecycle");
          assert_eq!(node("m/lifecycle#Draft").partition.as_deref(), Some("Sales"));
          assert_eq!(node("m/lifecycle#Placed").entry.as_deref(), Some("reserveStock"));
          assert_eq!(node("m/lifecycle#Placed").refines.as_deref(), Some("m/sub"));
          // Flow edges live in the typed model-level pool.
          let edges: Vec<_> = m.flow_edges.iter().filter(|e| e.behavior == "m/lifecycle").collect();
          assert_eq!(edges.len(), 4);
          let placed = edges.iter().find(|e| e.to == "m/lifecycle#Placed").unwrap();
          assert_eq!(placed.from, "m/lifecycle#Draft");
          assert_eq!(placed.trigger.as_deref(), Some("place"));
          assert_eq!(placed.guard.as_deref(), Some("items > 0"));
          assert_eq!(placed.effect.as_deref(), Some("reserve"));
          assert_eq!(placed.kind, FlowEdgeKind::ControlFlow);
          let ship = edges.iter().find(|e| e.to == "m/lifecycle#Ship").unwrap();
          assert_eq!(ship.carries.as_deref(), Some("m/order"));
          assert_eq!(ship.kind, FlowEdgeKind::ObjectFlow);
          // The pool spans both behaviors (the `Sub` activity's nodes are here too).
          assert!(m.activity_nodes.iter().any(|n| n.key == "m/sub#initial"));
      }
  ```

- [ ] **2.2 Run it, verify it fails to compile.** Run:
  ```
  cargo test -p waml builds_flow_view_and_pools
  ```
  Expected: compile errors — `no field `activity_nodes` on type `Model``, `FlowEdgeKind` not found, `ActivityNode`/`behavior` not found. This proves the target shape does not yet exist.

- [ ] **2.3 Replace `FlowNode` with `ActivityNode` and add `FlowEdgeKind`.** In `crates/waml/src/model.rs`, replace the entire `FlowNode` struct (the doc comment `/// A resolved node of a flow document.` plus the struct, ~lines 372-397) with the pool-member `ActivityNode` and the typed-edge kind:
  ```rust
  /// A behavior flow element in the shared model-level pool (design spec §3): an
  /// `Element`, NOT a classifier. Each activity/state-machine node lives here and
  /// is referenced from its owning behavior's view (`FlowDoc.nodes`) by `key` —
  /// exactly as a class `Diagram` references pooled classifiers by `members`.
  #[derive(Debug, Clone, PartialEq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub struct ActivityNode {
      /// Global pool identity: `"{behavior}#{id}"` (unique across the model).
      pub key: String,
      /// Local heading identity (unique within the owning behavior): the display
      /// name and the name local transitions resolve against.
      pub id: String,
      /// Owning behavior document key.
      pub behavior: String,
      pub kind: FlowNodeKind,
      /// Resolved key of an `object` node's typing classifier.
      #[cfg_attr(feature = "serde", serde(rename = "objectRef", default, skip_serializing_if = "Option::is_none"))]
      pub object_ref: Option<String>,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub partition: Option<String>,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub entry: Option<String>,
      #[cfg_attr(feature = "serde", serde(rename = "do", default, skip_serializing_if = "Option::is_none"))]
      pub do_: Option<String>,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub exit: Option<String>,
      /// Resolved key of the flow document this composite/call-behavior refines.
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub refines: Option<String>,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
      pub notes: Vec<String>,
  }

  /// The kind of a pooled activity edge (design spec §3). Not flattened into
  /// `Association`; each kind keeps its own semantics.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub enum FlowEdgeKind {
      /// Plain sequencing between activity nodes.
      ControlFlow,
      /// Carries an object token (an `object`-node endpoint, or a `carries` type).
      ObjectFlow,
  }
  ```

- [ ] **2.4 Reshape `FlowEdge` into a typed pool edge.** In `crates/waml/src/model.rs`, replace the entire `FlowEdge` struct (the doc comment `/// A resolved transition (flow edge)...` plus the struct, ~lines 399-423) with:
  ```rust
  /// A typed control/object flow edge (design spec §3): a model-level pool member,
  /// referenced from its owning behavior's view (`FlowDoc.edges`) by `key`.
  #[derive(Debug, Clone, PartialEq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub struct FlowEdge {
      /// Global pool identity: `"{behavior}#e{n}"`.
      pub key: String,
      pub kind: FlowEdgeKind,
      /// Owning behavior document key.
      pub behavior: String,
      /// Source activity-node pool key (always a node in `behavior`).
      pub from: String,
      /// Target activity-node pool key for a LOCAL target; the link title for a
      /// cross-document target (matches no local node key → not drawn, mirroring
      /// the class-diagram edge rule).
      pub to: String,
      /// Resolved key of the target *behavior document* when the target was a
      /// cross-document link.
      #[cfg_attr(feature = "serde", serde(rename = "toRef", default, skip_serializing_if = "Option::is_none"))]
      pub to_ref: Option<String>,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub trigger: Option<String>,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub guard: Option<String>,
      /// Decision default branch (`else transitions to …`).
      #[cfg_attr(feature = "serde", serde(rename = "else", default, skip_serializing_if = "is_false"))]
      pub is_else: bool,
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub effect: Option<String>,
      /// Resolved key of the carried object type (`carries <link>` object flow).
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub carries: Option<String>,
  }
  ```

- [ ] **2.5 Reshape `FlowDoc` into a pure view.** In `crates/waml/src/model.rs`, replace the entire `FlowDoc` struct (the doc comment `/// One flow document: one self-rendering directed graph (model AND view).` plus the struct, ~lines 425-439) with:
  ```rust
  /// One behavior document as a **view** (design spec §4): it no longer owns its
  /// nodes/edges inline — it references pooled `ActivityNode`s and `FlowEdge`s by
  /// key, exactly as a class `Diagram` references pooled classifiers by `members`.
  #[derive(Debug, Clone, PartialEq)]
  #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
  #[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
  #[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
  pub struct FlowDoc {
      pub key: String,
      pub title: String,
      pub flavor: FlowFlavor,
      /// Resolved key of the entity this behavior describes (frontmatter link).
      #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
      pub describes: Option<String>,
      /// Pool keys of this behavior's `ActivityNode`s (view → pool reference).
      pub nodes: Vec<String>,
      /// Pool keys of this behavior's `FlowEdge`s (view → pool reference).
      pub edges: Vec<String>,
  }
  ```

- [ ] **2.6 Add the two model-level pools to `Model`.** In `crates/waml/src/model.rs`, in `pub struct Model`, immediately after the `pub flows: Vec<FlowDoc>,` field (and its doc comment / attributes, ~line 824) and before the `interactions` field, add:
  ```rust
      /// Model-level pool of behavior flow elements (activity/state-machine nodes),
      /// referenced by `FlowDoc.nodes`. Design spec §3/§4.
      #[cfg_attr(feature = "serde", serde(rename = "activityNodes", default, skip_serializing_if = "Vec::is_empty"))]
      pub activity_nodes: Vec<ActivityNode>,
      /// Model-level pool of typed control/object flow edges, referenced by
      /// `FlowDoc.edges`. Design spec §3/§4.
      #[cfg_attr(feature = "serde", serde(rename = "flowEdges", default, skip_serializing_if = "Vec::is_empty"))]
      pub flow_edges: Vec<FlowEdge>,
  ```

- [ ] **2.7 Rewrite `build_flows` to populate the pools + view.** In `crates/waml/src/parse.rs`, replace the entire `build_flows` function (the doc comment `/// Scan all parsed docs for behavior docs...` plus the function body, ~lines 684-771) with:
  ```rust
  /// Scan all parsed docs for behavior docs in the flow substrate
  /// (`uml.Activity` / `uml.StateMachine` — NOT `uml.Sequence`) and resolve their
  /// `## Nodes` block into a `FlowDoc` **view** plus its `ActivityNode` and
  /// `FlowEdge` **pool** members. Pool keys are `"{behavior}#{id}"` (nodes) and
  /// `"{behavior}#e{n}"` (edges). A behavior with no `## Nodes` section yields an
  /// empty view and no pool members — never a panic.
  fn build_flows(
      parsed: &[ParsedDoc],
      keyset: &HashSet<&str>,
  ) -> (Vec<FlowDoc>, Vec<ActivityNode>, Vec<FlowEdge>) {
      use crate::syntax::{FlowBullet, FlowTargetRef};
      let flow_keys: HashSet<String> = parsed
          .iter()
          .filter(|p| matches!(p.ty, ElementType::Behavior(BehaviorKind::Activity | BehaviorKind::StateMachine)))
          .map(|p| p.id.clone())
          .collect();
      let mut views = Vec::new();
      let mut pool_nodes = Vec::new();
      let mut pool_edges = Vec::new();
      for p in parsed {
          let flavor = match p.ty {
              ElementType::Behavior(BehaviorKind::Activity) => FlowFlavor::Activity,
              ElementType::Behavior(BehaviorKind::StateMachine) => FlowFlavor::StateMachine,
              _ => continue,
          };
          let behavior = p.id.clone();
          // First pass: local identity -> kind, for object-flow classification.
          let mut kinds: HashMap<&str, crate::model::FlowNodeKind> = HashMap::new();
          for s in &p.doc.sections {
              let Section::Nodes(block) = s else { continue };
              for n in &block.nodes {
                  kinds.entry(n.identity.as_str()).or_insert(n.kind);
              }
          }
          let mut node_keys = Vec::new();
          let mut edge_keys = Vec::new();
          let mut edge_n = 0usize;
          for s in &p.doc.sections {
              let Section::Nodes(block) = s else { continue };
              for n in &block.nodes {
                  let node_key = format!("{behavior}#{}", n.identity);
                  let mut anode = ActivityNode {
                      key: node_key.clone(),
                      id: n.identity.clone(),
                      behavior: behavior.clone(),
                      kind: n.kind,
                      object_ref: n
                          .object_ref
                          .as_ref()
                          .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
                          .filter(|k| keyset.contains(k.as_str())),
                      partition: None,
                      entry: None,
                      do_: None,
                      exit: None,
                      refines: None,
                      notes: n.notes.iter().filter_map(Line::parsed).cloned().collect(),
                  };
                  for b in n.bullets.iter().filter_map(Line::parsed) {
                      match b {
                          FlowBullet::Transition(t) => {
                              let (to, to_ref, target_is_object) = match &t.target {
                                  FlowTargetRef::Local(name) => (
                                      format!("{behavior}#{name}"),
                                      None,
                                      kinds.get(name.as_str()) == Some(&crate::model::FlowNodeKind::Object),
                                  ),
                                  FlowTargetRef::Link(l) => {
                                      let r = crate::okf::resolve_href(&p.path, &l.slug);
                                      (l.title.clone(), flow_keys.contains(&r).then_some(r), false)
                                  }
                              };
                              let carries = t
                                  .carries
                                  .as_ref()
                                  .map(|l| crate::okf::resolve_href(&p.path, &l.slug))
                                  .filter(|k| keyset.contains(k.as_str()));
                              // ObjectFlow iff it carries a type, or an endpoint is an object node.
                              let kind = if carries.is_some()
                                  || n.kind == crate::model::FlowNodeKind::Object
                                  || target_is_object
                              {
                                  FlowEdgeKind::ObjectFlow
                              } else {
                                  FlowEdgeKind::ControlFlow
                              };
                              let edge_key = format!("{behavior}#e{edge_n}");
                              edge_n += 1;
                              edge_keys.push(edge_key.clone());
                              pool_edges.push(FlowEdge {
                                  key: edge_key,
                                  kind,
                                  behavior: behavior.clone(),
                                  from: node_key.clone(),
                                  to,
                                  to_ref,
                                  trigger: t.trigger.clone(),
                                  guard: t.guard.clone(),
                                  is_else: t.is_else,
                                  effect: t.effect.clone(),
                                  carries,
                              });
                          }
                          FlowBullet::Entry(e) => anode.entry = Some(e.clone()),
                          FlowBullet::Do(e) => anode.do_ = Some(e.clone()),
                          FlowBullet::Exit(e) => anode.exit = Some(e.clone()),
                          FlowBullet::Refines(l) => {
                              let r = crate::okf::resolve_href(&p.path, &l.slug);
                              anode.refines = flow_keys.contains(&r).then_some(r);
                          }
                          FlowBullet::Partition(name) => anode.partition = Some(name.clone()),
                      }
                  }
                  node_keys.push(node_key);
                  pool_nodes.push(anode);
              }
          }
          views.push(FlowDoc {
              key: p.id.clone(),
              title: doc_title(p),
              flavor,
              describes: resolve_describes(p, keyset),
              nodes: node_keys,
              edges: edge_keys,
          });
      }
      (views, pool_nodes, pool_edges)
  }
  ```
  (`HashMap` and `HashSet` are already imported at the top of `parse.rs` — line 10, `use std::collections::{HashMap, HashSet};` — so no new import is needed.)

- [ ] **2.8 Distribute the pools into the `Model` literal.** In `crates/waml/src/parse.rs`, in `build_model`, replace:
  ```rust
      let flows = build_flows(&parsed, &keyset);
      let interactions = build_interactions(&parsed, &keyset);

      Model { nodes, edges, diagrams, path, packages, flows, interactions }
  ```
  with:
  ```rust
      let (flows, activity_nodes, flow_edges) = build_flows(&parsed, &keyset);
      let interactions = build_interactions(&parsed, &keyset);

      Model { nodes, edges, diagrams, path, packages, flows, activity_nodes, flow_edges, interactions }
  ```

- [ ] **2.9 Update the `parse.rs` imports.** In `crates/waml/src/parse.rs`, in the `use crate::model::{ ... }` block (~lines 13-14), rename the imported flow item `FlowNode` to `ActivityNode` and add `FlowEdgeKind`. The block becomes (keep the other names as-is):
  ```rust
  use crate::model::{
      ActivityNode, Attribute, BehaviorKind, Diagram, DiagramDisplay, DiagramGroup, Edge, ElementType,
      FlowDoc, FlowEdge, FlowEdgeKind, FlowFlavor, Lifeline, Model, Node, SeqItem, SeqOperand,
      SequenceDoc,
  };
  ```

- [ ] **2.10 Run the crate tests, verify green.** Run:
  ```
  cargo test -p waml
  ```
  Expected: all pass, including `builds_flow_view_and_pools_with_resolved_links_and_edges`, `excludes_sequence_docs_from_flows` (unchanged — it only asserts on `m.flows[..].key`), and `flow_document_serialize_is_a_semantic_fixpoint` (storage untouched). If `parses_flow_nodes_section` / `malformed_flow_bullet_is_preserved_and_diagnosed` fail, they should NOT — they operate on `Section::Nodes` AST via `parse_document`, not on `Model`; leave them alone.

- [ ] **2.11 Run the whole workspace, verify green.** Run:
  ```
  cargo test --workspace
  ```
  Expected: all green. (If any other crate constructs `FlowNode`/`FlowEdge`/`FlowDoc`/`Model` literally, the compiler will name the file+line; the only known construction site is the `build_flows`/test pair just rewritten.)

- [ ] **2.12 Format + clippy the Rust change.** Run:
  ```
  cargo fmt
  cargo clippy -p waml --all-targets
  ```
  Expected: `cargo fmt` normalizes the new code; clippy reports no new warnings.

- [ ] **2.13 Regenerate the wasm bindings.** Run:
  ```
  pnpm build:wasm
  ```
  Then inspect the generated flow types:
  ```
  git diff packages/wasm/src/generated/waml_wasm.d.ts
  ```
  Expected diff: `FlowNode` interface → replaced by `ActivityNode` (with `key` / `id` / `behavior`); `FlowEdge` gains `key` / `kind` / `behavior` and `from` / `to` are documented as pool keys; `FlowDoc.nodes` / `FlowDoc.edges` become `string[]`; a new `FlowEdgeKind = "controlFlow" | "objectFlow"`; the `Model` interface gains `activityNodes?: ActivityNode[]` and `flowEdges?: FlowEdge[]`.

- [ ] **2.14 Update the wasm package re-exports.** In `packages/wasm/src/index.ts`, in the `export type { ... } from "./generated/waml_wasm.js";` block, replace the line `  FlowNode,` with `  ActivityNode,` and add `  FlowEdgeKind,` immediately after `  FlowEdge,`. The flow region becomes:
  ```ts
    FlowDoc,
    ActivityNode,
    FlowEdge,
    FlowEdgeKind,
    FlowFlavor,
    FlowNodeKind,
  ```

- [ ] **2.15 Typecheck the wasm package, verify green.** Run:
  ```
  pnpm --filter @waml/wasm build
  ```
  Expected: `tsc` succeeds — `ActivityNode` / `FlowEdgeKind` resolve in the regenerated `.d.ts` and the re-export list is consistent. (Downstream `@waml/okf` / web are updated in Tasks 3-4, so do NOT run the full `pnpm build` here — it is expected to fail on the stale `FlowNode` type reference until then.)

- [ ] **2.16 Commit.** Run:
  ```
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/src/parse.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/index.ts
  git commit -m "feat(model): split behavior flow elements into model-level pools; FlowDoc becomes a view"
  ```

---

## Task 3: Reshape `@waml/okf` types + core overlay plumbing

Propagate the new pool/view shape through the type re-export package and the `Rust Model → ModelGraph` fusion so the two pools reach the frontend graph.

**Files:**
- Modify: `packages/okf/src/types.ts` — re-export list + `ModelGraph` interface.
- Modify: `packages/core/src/state/overlay.ts` — `toModelGraph`.
- Modify: `packages/core/src/state/overlay.test.ts` — flow-passthrough fixture.

**Interfaces:**
- Consumes: `ActivityNode`, `FlowEdge`, `FlowEdgeKind`, `FlowDoc` from `@waml/wasm` (Task 2).
- Produces: `ModelGraph.activityNodes?: ActivityNode[]`, `ModelGraph.flowEdges?: FlowEdge[]`; `toModelGraph` fills both from `model.activityNodes` / `model.flowEdges`.

Steps:

- [ ] **3.1 Update the `@waml/okf` re-exports.** In `packages/okf/src/types.ts`, in the `export type { ... } from "@waml/wasm";` block (~lines 7-30), replace the line `  FlowNode,` with `  ActivityNode,` and add `  FlowEdgeKind,` immediately after `  FlowEdge,`. Then in the `import type { ... } from "@waml/wasm";` block just below it (~lines 32-40), add `  ActivityNode,` and `  FlowEdge,` to the imported names (they are needed to type the new `ModelGraph` pool fields).

- [ ] **3.2 Add the two pools to `ModelGraph`.** In `packages/okf/src/types.ts`, in `export interface ModelGraph`, immediately after the `flows?: FlowDoc[];` field (and its doc comment, ~line 174) add:
  ```ts
    /** Model-level pool of behavior flow elements, referenced by `FlowDoc.nodes` (design spec §3/§4). */
    activityNodes?: ActivityNode[];
    /** Model-level pool of typed control/object flow edges, referenced by `FlowDoc.edges`. */
    flowEdges?: FlowEdge[];
  ```

- [ ] **3.3 Update the overlay flow-passthrough test (RED).** In `packages/core/src/state/overlay.test.ts`, replace the `"passes flow docs through to the ModelGraph"` test body (~lines 180-192) with the new view+pool shape:
  ```ts
    it("passes flow views and their pools through to the ModelGraph", () => {
      const flow: FlowDoc = {
        key: "m/lifecycle",
        title: "Order Lifecycle",
        flavor: "stateMachine",
        describes: "m/order",
        nodes: ["m/lifecycle#initial", "m/lifecycle#Draft"],
        edges: ["m/lifecycle#e0"],
      };
      const activityNodes = [
        { key: "m/lifecycle#initial", id: "initial", behavior: "m/lifecycle", kind: "initial" },
        { key: "m/lifecycle#Draft", id: "Draft", behavior: "m/lifecycle", kind: "plain", entry: "reserveStock" },
      ];
      const flowEdges = [
        { key: "m/lifecycle#e0", kind: "controlFlow", behavior: "m/lifecycle", from: "m/lifecycle#initial", to: "m/lifecycle#Draft" },
      ];
      const rust = { nodes: [], edges: [], diagrams: [], path: "", packages: [], flows: [flow], activityNodes, flowEdges };
      const g = toModelGraph(rust as never, emptyOverlay());
      expect(g.flows).toEqual([flow]);
      expect(g.activityNodes).toEqual(activityNodes);
      expect(g.flowEdges).toEqual(flowEdges);
    });
  ```

- [ ] **3.4 Run it, verify it fails.** Run:
  ```
  pnpm --filter @waml/core test -- overlay
  ```
  Expected: FAIL — `g.activityNodes` / `g.flowEdges` are `undefined` (`toModelGraph` does not yet forward them).

- [ ] **3.5 Forward the pools in `toModelGraph`.** In `packages/core/src/state/overlay.ts`, in the object returned by `toModelGraph` (~lines 136-144), immediately after `flows: model.flows ?? [],` add:
  ```ts
      activityNodes: model.activityNodes ?? [],
      flowEdges: model.flowEdges ?? [],
  ```
  (`RustModel` is the generated `Model` type from `@waml/wasm`, which now carries `activityNodes?` / `flowEdges?` — no manual `RustModel` edit is needed.)

- [ ] **3.6 Run the core tests, verify green.** Run:
  ```
  pnpm --filter @waml/okf build
  pnpm --filter @waml/core test
  ```
  Expected: `@waml/okf` builds (types resolve); the overlay test passes; the `nav/tree.test.ts` and `state/diagrams.test.ts` fixtures — which build `FlowDoc`s with empty `nodes: []` / `edges: []` — still typecheck (empty arrays satisfy `string[]`) and pass.

- [ ] **3.7 Commit.** Run:
  ```
  git add packages/okf/src/types.ts packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
  git commit -m "feat(core): thread activity-node + flow-edge pools through ModelGraph"
  ```

---

## Task 4: Rebuild the flow renderer against the pool/view model (full gate)

Reshape the web renderer to resolve a `FlowDoc` view against the model pools, retype the flow node components, and bring the whole gate green.

**Files:**
- Modify: `packages/web/src/canvas/flowGraph.ts` — retype to `ActivityNode`; add `resolveFlow`; key layout by pool key.
- Modify: `packages/web/src/canvas/flowGraph.test.ts` — resolved-view fixtures + `resolveFlow` test.
- Modify: `packages/web/src/components/canvas/flow/FlowView.svelte` — `graph` prop + resolve.
- Modify: `packages/web/src/components/canvas/flow/FlowView.test.ts` — pass `graph`.
- Modify: `packages/web/src/components/canvas/flow/FlowStepNode.svelte`, `FlowControlNode.svelte`, `FlowObjectNode.svelte` — `FlowNode` type → `ActivityNode`.
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` — pass `graph={$model}`.

**Interfaces:**
- Consumes: `ActivityNode`, `FlowEdge`, `FlowFlavor`, `FlowDoc`, `ModelGraph` from `@waml/okf`.
- Produces: `resolveFlow(doc: FlowDoc, graph: ModelGraph): { flavor: FlowFlavor; nodes: ActivityNode[]; edges: FlowEdge[] }`; `flowToRf(view: { flavor: FlowFlavor; nodes: ActivityNode[]; edges: FlowEdge[] }): { nodes: Node[]; edges: Edge[] }`.

Steps:

- [ ] **4.1 Rewrite `flowGraph.test.ts` against the resolved-view shape (RED).** Replace the whole file `packages/web/src/canvas/flowGraph.test.ts` with:
  ```ts
  import { describe, expect, it } from "vitest";
  import type { ActivityNode, FlowDoc, FlowEdge, FlowFlavor, ModelGraph } from "@waml/okf";
  import { flowToRf, resolveFlow, transitionLabel } from "./flowGraph";

  const B = "m/lifecycle";
  const k = (id: string) => `${B}#${id}`;
  const nodes: ActivityNode[] = [
    { key: k("initial"), id: "initial", behavior: B, kind: "initial" },
    { key: k("Draft"), id: "Draft", behavior: B, kind: "plain" },
    { key: k("Ready to ship?"), id: "Ready to ship?", behavior: B, kind: "decision" },
    { key: k("final"), id: "final", behavior: B, kind: "final" },
  ];
  const edges: FlowEdge[] = [
    { key: k("e0"), kind: "controlFlow", behavior: B, from: k("initial"), to: k("Draft") },
    { key: k("e1"), kind: "controlFlow", behavior: B, from: k("Draft"), to: k("Ready to ship?"), trigger: "place", guard: "items > 0", effect: "reserve" },
    { key: k("e2"), kind: "controlFlow", behavior: B, from: k("Ready to ship?"), to: k("final"), else: true },
    { key: k("e3"), kind: "controlFlow", behavior: B, from: k("Draft"), to: k("Missing") }, // unresolved target: not drawn, never errors
  ];
  const view = { flavor: "stateMachine" as FlowFlavor, nodes, edges };

  describe("transitionLabel", () => {
    it("renders UML 'trigger [guard] / effect' labels", () => {
      expect(transitionLabel(edges[1])).toBe("place [items > 0] / reserve");
      expect(transitionLabel(edges[2])).toBe("[else]");
      expect(transitionLabel(edges[0])).toBe("");
    });
  });

  describe("resolveFlow", () => {
    it("dereferences a view's node/edge keys against the model pools", () => {
      const graph = { activityNodes: nodes, flowEdges: edges } as unknown as ModelGraph;
      const doc: FlowDoc = { key: B, title: "T", flavor: "stateMachine", nodes: nodes.map((n) => n.key), edges: edges.map((e) => e.key) };
      const r = resolveFlow(doc, graph);
      expect(r.flavor).toBe("stateMachine");
      expect(r.nodes.map((n) => n.id)).toEqual(["initial", "Draft", "Ready to ship?", "final"]);
      expect(r.edges).toHaveLength(4);
    });
  });

  describe("flowToRf", () => {
    it("lays out every node and maps kinds to component types", () => {
      const { nodes: rf, edges: rfEdges } = flowToRf(view);
      expect(rf).toHaveLength(4);
      expect(rf.map((n) => n.type)).toEqual(["flowControl", "flowStep", "flowControl", "flowControl"]);
      // React node ids are pool keys; dagre TB puts initial above final.
      const y = (key: string) => rf.find((n) => n.id === key)!.position.y;
      expect(y(k("initial"))).toBeLessThan(y(k("final")));
      // the edge to a missing node is dropped, the rest are transitions
      expect(rfEdges).toHaveLength(3);
      expect(rfEdges.every((e) => e.type === "transition")).toBe(true);
    });

    it("carries the flavor and the source node's kind on each edge", () => {
      const { edges: rfEdges } = flowToRf(view);
      const data = (i: number) => rfEdges[i].data as { flavor: string; fromKind: string };
      expect(rfEdges.every((e) => (e.data as { flavor: string }).flavor === "stateMachine")).toBe(true);
      expect(data(0).fromKind).toBe("initial");
      expect(data(2).fromKind).toBe("decision");
    });
  });
  ```

- [ ] **4.2 Run it, verify it fails.** Run:
  ```
  pnpm --filter web test -- flowGraph
  ```
  Expected: FAIL — `resolveFlow` is not exported and `flowToRf` still expects a `FlowDoc` with inline `nodes`/`edges`.

- [ ] **4.3 Rewrite `flowGraph.ts`.** Replace the whole file `packages/web/src/canvas/flowGraph.ts` with:
  ```ts
  import dagre from "@dagrejs/dagre";
  import type { Edge, Node } from "@xyflow/svelte";
  import type { ActivityNode, FlowDoc, FlowEdge, FlowFlavor, ModelGraph } from "@waml/okf";

  // ── Flow substrate rendering (behavior model/view split) ─────────────────────
  // A behavior document is a VIEW: it references pooled activity nodes / flow
  // edges by key. `resolveFlow` dereferences those against the model pools; the
  // resolved graph is laid out at render time by dagre (relational, never stored).

  export function flowNodeSize(n: ActivityNode): { width: number; height: number } {
    switch (n.kind) {
      case "initial":
      case "final":
        return { width: 36, height: 36 };
      case "decision":
      case "merge":
        return { width: 56, height: 56 };
      case "fork":
      case "join":
        return { width: 120, height: 10 };
      case "object":
        return { width: 160, height: 48 };
      default: {
        const internals = [n.entry, n.do, n.exit].filter(Boolean).length;
        return { width: 180, height: 48 + internals * 18 + (n.refines ? 18 : 0) };
      }
    }
  }

  /** UML edge label: `trigger [guard] / effect`; a decision default is `[else]`. */
  export function transitionLabel(e: FlowEdge): string {
    const head = [e.trigger, e.guard ? `[${e.guard}]` : e.else ? "[else]" : undefined]
      .filter(Boolean)
      .join(" ");
    const eff = e.effect ? `/ ${e.effect}` : "";
    return [head, eff].filter(Boolean).join(" ").trim();
  }

  const KIND_TO_TYPE: Record<ActivityNode["kind"], string> = {
    plain: "flowStep",
    object: "flowObject",
    initial: "flowControl",
    final: "flowControl",
    decision: "flowControl",
    merge: "flowControl",
    fork: "flowControl",
    join: "flowControl",
  };

  export interface ResolvedFlow {
    flavor: FlowFlavor;
    nodes: ActivityNode[];
    edges: FlowEdge[];
  }

  /** Dereference a behavior VIEW's node/edge keys against the model-level pools. */
  export function resolveFlow(doc: FlowDoc, graph: ModelGraph): ResolvedFlow {
    const nodeByKey = new Map((graph.activityNodes ?? []).map((n) => [n.key, n]));
    const edgeByKey = new Map((graph.flowEdges ?? []).map((e) => [e.key, e]));
    const nodes = doc.nodes.map((key) => nodeByKey.get(key)).filter((n): n is ActivityNode => n != null);
    const edges = doc.edges.map((key) => edgeByKey.get(key)).filter((e): e is FlowEdge => e != null);
    return { flavor: doc.flavor, nodes, edges };
  }

  export function flowToRf(view: ResolvedFlow): { nodes: Node[]; edges: Edge[] } {
    const g = new dagre.graphlib.Graph();
    g.setDefaultEdgeLabel(() => ({}));
    g.setGraph({ rankdir: "TB", nodesep: 50, ranksep: 70 });
    for (const n of view.nodes) {
      const s = flowNodeSize(n);
      g.setNode(n.key, { width: s.width, height: s.height });
    }
    const local = new Set(view.nodes.map((n) => n.key));
    for (const e of view.edges) if (local.has(e.from) && local.has(e.to)) g.setEdge(e.from, e.to);
    dagre.layout(g);

    const nodes: Node[] = view.nodes.map((n) => {
      const s = flowNodeSize(n);
      const pos = g.node(n.key);
      return {
        id: n.key,
        type: KIND_TO_TYPE[n.kind],
        position: { x: (pos?.x ?? 0) - s.width / 2, y: (pos?.y ?? 0) - s.height / 2 },
        data: { node: n, flavor: view.flavor } as unknown as Record<string, unknown>,
        draggable: false,
        connectable: false,
        selectable: false,
      };
    });
    const kindByKey = new Map(view.nodes.map((n) => [n.key, n.kind]));
    const edges: Edge[] = view.edges
      .filter((e) => local.has(e.from) && local.has(e.to))
      .map((e) => ({
        id: e.key,
        source: e.from,
        target: e.to,
        type: "transition",
        // flavor picks the path shape; fromKind lets a decision source snap to a tip.
        data: { label: transitionLabel(e), carries: e.carries, flavor: view.flavor, fromKind: kindByKey.get(e.from) } as unknown as Record<string, unknown>,
        selectable: false,
      }));
    return { nodes, edges };
  }
  ```

- [ ] **4.4 Run the renderer test, verify green.** Run:
  ```
  pnpm --filter web test -- flowGraph
  ```
  Expected: `transitionLabel`, `resolveFlow`, and `flowToRf` suites all pass.

- [ ] **4.5 Retype the three flow node components.** In each of `packages/web/src/components/canvas/flow/FlowStepNode.svelte`, `FlowControlNode.svelte`, and `FlowObjectNode.svelte`, change the import line
  ```ts
    import type { FlowFlavor, FlowNode } from "@waml/okf";
  ```
  to
  ```ts
    import type { ActivityNode, FlowFlavor } from "@waml/okf";
  ```
  and change the props line
  ```ts
    let { data }: { data: { node: FlowNode; flavor: FlowFlavor } } = $props();
  ```
  to
  ```ts
    let { data }: { data: { node: ActivityNode; flavor: FlowFlavor } } = $props();
  ```
  (The component bodies read `n.id` / `n.entry` / `n.do` / `n.exit` / `n.refines` / `n.partition`, all of which exist on `ActivityNode` — no body changes.)

- [ ] **4.6 Update `FlowView.svelte` to resolve the view.** Replace the whole `<script>` block of `packages/web/src/components/canvas/flow/FlowView.svelte` with:
  ```svelte
  <script lang="ts">
    import { SvelteFlow, SvelteFlowProvider, Background, BackgroundVariant, Controls, type Edge, type Node } from "@xyflow/svelte";
    import type { FlowDoc, ModelGraph } from "@waml/okf";
    import { flowToRf, resolveFlow } from "../../../canvas/flowGraph";
    import FlowStepNode from "./FlowStepNode.svelte";
    import FlowControlNode from "./FlowControlNode.svelte";
    import FlowObjectNode from "./FlowObjectNode.svelte";
    import TransitionEdge from "./TransitionEdge.svelte";

    let { doc, graph }: { doc: FlowDoc; graph: ModelGraph } = $props();

    const nodeTypes = { flowStep: FlowStepNode, flowControl: FlowControlNode, flowObject: FlowObjectNode };
    const edgeTypes = { transition: TransitionEdge };

    let nodes = $state<Node[]>([]);
    let edges = $state<Edge[]>([]);
    $effect(() => {
      const rf = flowToRf(resolveFlow(doc, graph));
      nodes = rf.nodes;
      edges = rf.edges;
    });
  </script>
  ```
  (Leave the markup below `</script>` unchanged.)

- [ ] **4.7 Pass the graph from `CanvasInner.svelte`.** In `packages/web/src/components/canvas/CanvasInner.svelte`, change the `FlowView` usage (~line 729) from:
  ```svelte
          <FlowView doc={activeFlow} />
  ```
  to:
  ```svelte
          <FlowView doc={activeFlow} graph={$model} />
  ```

- [ ] **4.8 Update the `FlowView.test.ts` fixture to the view+pool shape.** Replace the whole file `packages/web/src/components/canvas/flow/FlowView.test.ts` with:
  ```ts
  import { describe, expect, it } from "vitest";
  import { render } from "@testing-library/svelte";
  import type { FlowDoc, ModelGraph } from "@waml/okf";
  import FlowView from "./FlowView.svelte";

  const B = "m/lifecycle";
  const DOC: FlowDoc = {
    key: B,
    title: "Order Lifecycle",
    flavor: "stateMachine",
    nodes: [`${B}#initial`, `${B}#Placed`, `${B}#final`],
    edges: [`${B}#e0`, `${B}#e1`],
  };
  const GRAPH = {
    activityNodes: [
      { key: `${B}#initial`, id: "initial", behavior: B, kind: "initial" },
      { key: `${B}#Placed`, id: "Placed", behavior: B, kind: "plain", entry: "reserveStock" },
      { key: `${B}#final`, id: "final", behavior: B, kind: "final" },
    ],
    flowEdges: [
      { key: `${B}#e0`, kind: "controlFlow", behavior: B, from: `${B}#initial`, to: `${B}#Placed` },
      { key: `${B}#e1`, kind: "controlFlow", behavior: B, from: `${B}#Placed`, to: `${B}#final`, trigger: "deliver" },
    ],
  } as unknown as ModelGraph;

  describe("FlowView", () => {
    it("renders every flow node with its internals", () => {
      const { getByText } = render(FlowView, { props: { doc: DOC, graph: GRAPH } });
      expect(getByText("Placed")).toBeTruthy();
      expect(getByText("entry / reserveStock")).toBeTruthy();
    });

    it("gives every node a source and target handle so edges survive", () => {
      // SvelteFlow drops an edge unless the source node has a source handle and
      // the target node a target handle. jsdom never lays the graph out, so we
      // assert the invariant that was missing and caused every flow edge to vanish.
      const { container } = render(FlowView, { props: { doc: DOC, graph: GRAPH } });
      expect(container.querySelectorAll(".svelte-flow__handle.source").length).toBe(DOC.nodes.length);
      expect(container.querySelectorAll(".svelte-flow__handle.target").length).toBe(DOC.nodes.length);
    });
  });
  ```

- [ ] **4.9 Run the full CI gate, verify green.** Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  ```
  Expected: all green end-to-end — Rust pools resolve, the wasm bindings match, `@waml/okf` and web typecheck against `ActivityNode` / `FlowEdgeKind`, and every flow test passes on the new shape.

- [ ] **4.10 Commit.** Run:
  ```
  git add packages/web/src/canvas/flowGraph.ts packages/web/src/canvas/flowGraph.test.ts packages/web/src/components/canvas/flow/FlowView.svelte packages/web/src/components/canvas/flow/FlowView.test.ts packages/web/src/components/canvas/flow/FlowStepNode.svelte packages/web/src/components/canvas/flow/FlowControlNode.svelte packages/web/src/components/canvas/flow/FlowObjectNode.svelte packages/web/src/components/canvas/CanvasInner.svelte
  git commit -m "feat(web): render behavior views by resolving activity-node + flow-edge pools"
  ```

---

## Notes / risks

- **Object-flow classification is derived, not authored.** `FlowEdgeKind` is computed at build time: `ObjectFlow` iff the edge `carries` a resolved type OR either endpoint is an `object` node; else `ControlFlow`. This matches design spec §3 (ObjectFlow carries a type; object nodes are its endpoints) without adding grammar. Cross-document targets can't be inspected for kind, so a cross-doc transition is `ObjectFlow` only when it `carries` or its source is an object node — acceptable, and the renderer drops cross-doc edges anyway.
- **Cross-document transitions are unchanged.** `to_ref` still resolves to the target *behavior document* key; the local `to` fallback (the link title) still fails the renderer's `local.has(e.to)` check and is not drawn — byte-for-byte the same visible behavior as today.
- **Pool keys use `#` deliberately.** Document slugs derive from file paths (slashes, no `#`), so `"{behavior}#{id}"` / `"{behavior}#e{n}"` can never collide with a classifier / package / diagram key. The sequence-slice plan must use the same `#` convention if it pools any interaction elements.
- **Storage is untouched.** No grammar/syntax/serialize change; the markdown fixpoint test proves it. The wire *runtime* model changes shape, which is explicitly allowed (design spec §9: storage and runtime need not be 1:1).
- **`is_view()` vs `is_classifier()`.** They are different predicates and both now exist on `ElementType`. `is_view()` (Diagram + behaviors) gates the classifier node pool; `is_classifier()` (structural + behavior classifiers) is the spec §3.1 predicate. Do not merge them.
</content>
</invoke>
