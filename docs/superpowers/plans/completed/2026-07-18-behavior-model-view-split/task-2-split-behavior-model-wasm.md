# Behavior Model/View Split — Task 2: Split behavior model from view (Rust + wasm bindings)

> **Segment 2 of 4** of the **Behavior Model/View Split** plan. See [`README.md`](README.md) for the plan Goal, Architecture, Tech Stack, File Structure, and Notes/risks; full original monolithic plan preserved verbatim as [`_source.md`](_source.md).
> **REQUIRED SUB-SKILL:** superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

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
