# Task 2: Rust-owned wire projection (`wire` module)

**Load:** `README.md` + this file. Read `crates/waml-wasm/src/lib.rs` (the wasm boundary), `packages/core/src/state/overlay.ts`, and `crates/waml/tests/serde_shape.rs` before starting.

**Deliverable:** A new `crate::wire` module projects today's **unchanged** object-model `Model` into flat wire DTOs (`WireGraph`/`WireNode`/`WireEdge`/`WireDiagram`). `waml-wasm::build_model` returns `WireGraph`. The wire JSON is byte-identical to today's **except** `diagram.members` is flattened in Rust (was `diagram.groups`), so `overlay.ts` drops `flattenGroups`. This is the indirection Tasks 3–5 reshape the object model behind. **The object model is untouched in this task.**

**Files:**
- Create: `crates/waml/src/wire.rs`
- Modify: `crates/waml/src/lib.rs` — add `pub mod wire;` (alphabetical, after `pub mod uml;`).
- Modify: `crates/waml-wasm/src/lib.rs` — `build_model_json` + the `#[wasm_bindgen] build_model` return the wire.
- Modify: `crates/waml/tests/serde_shape.rs` — add a wire-shape lock test.
- Modify: `packages/core/src/state/overlay.ts` — drop `flattenGroups`; read flat `d.members`; retype `RustDiagram`.
- Modify: `packages/core/src/state/overlay.test.ts` — diagram inputs use `members: []` not `groups: []`.
- REGENERATED: `packages/wasm/src/generated/{waml_wasm.d.ts, wasm-inline.ts}` via `pnpm build:wasm`.

**Interfaces:**
- Produces: `pub struct WireGraph`/`WireNode`/`WireEdge`/`WireDiagram` (README "Rust-owned wire projection"); `pub fn build_wire(model: &crate::model::Model) -> WireGraph`. **Signature `&Model` is stable for all later tasks** — Task 3 adds `Model.concepts` and reads it inside `build_wire` without changing this signature or the wasm caller.
- Consumes: `crate::model::{Model, Node, Edge, Diagram, DiagramGroup, DiagramDisplay, ElementType, RelationshipKind, AssocName, RelEnd, Attribute, NoteAnchor, FlowDoc, SequenceDoc}`, `crate::okf::Concept`, `crate::syntax::LayoutStatement`.

---

## Steps

- [ ] **2.1 Create `crates/waml/src/wire.rs`.** The `Wire*` types mirror today's `Model`/`Node`/`Edge`/`Diagram` serde field names EXACTLY (so the wire JSON is stable) — except `WireDiagram` has flat `members: Vec<String>` instead of `groups`. Write:

```rust
//! The Rust-owned wire projection: flat DTOs that cross the wasm boundary to the
//! TypeScript skin. The object model (`crate::model`) is the internal source of
//! truth; `build_wire` projects it into this shape. Rust owns every flatten/join
//! here so TS stays a render skin (no domain logic). See the plan README.
//!
//! Slice 1: field-compatible with the pre-reshape wire, EXCEPT `WireDiagram.members`
//! is flattened from the object model's `groups` here in Rust (was flattened in
//! `overlay.ts::flattenGroups`).

use crate::model::{
    Attribute, Diagram, DiagramDisplay, DiagramGroup, Edge, ElementType, FlowDoc, Model, Node,
    NoteAnchor, RelEnd, RelationshipKind, SequenceDoc,
};
use crate::model::AssocName;
use crate::okf::Concept;
use crate::syntax::LayoutStatement;

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireGraph {
    pub nodes: Vec<WireNode>,
    pub edges: Vec<WireEdge>,
    pub diagrams: Vec<WireDiagram>,
    #[cfg_attr(feature = "serde", serde(default))]
    pub path: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub packages: Vec<WireNode>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub flows: Vec<FlowDoc>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub interactions: Vec<SequenceDoc>,
}

/// Flat node DTO == today's `ModelNode` minus `position` (position is TS overlay
/// state). Field names/serde match the pre-reshape `Node` exactly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireNode {
    pub concept: Concept,
    pub key: String,
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    #[cfg_attr(feature = "wasm", tsify(type = "string"))]
    pub ty: ElementType,
    pub stereotypes: Vec<String>,
    #[cfg_attr(feature = "serde", serde(rename = "abstract", default, skip_serializing_if = "crate::model::is_false"))]
    pub abstract_: bool,
    pub attributes: Vec<Attribute>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub values: Vec<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub note_body: Option<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub annotates: Vec<NoteAnchor>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireEdge {
    #[cfg_attr(feature = "serde", serde(rename = "from"))]
    pub source: String,
    #[cfg_attr(feature = "serde", serde(rename = "to"))]
    pub target: String,
    pub kind: RelationshipKind,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    #[cfg_attr(feature = "wasm", tsify(type = "string | { ref: string }"))]
    pub name: Option<AssocName>,
    #[cfg_attr(feature = "serde", serde(rename = "fromEnd"))]
    pub from_end: RelEnd,
    #[cfg_attr(feature = "serde", serde(rename = "toEnd"))]
    pub to_end: RelEnd,
    pub bidirectional: bool,
}

/// Flat diagram DTO. `members` is flattened from the object model's `groups` in
/// Rust (moves `overlay.ts::flattenGroups` into Rust). `display`/`layout` unchanged.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct WireDiagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub description: Option<String>,
    pub members: Vec<String>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "DiagramDisplay::is_empty"))]
    pub display: DiagramDisplay,
    #[cfg_attr(feature = "wasm", tsify(type = "unknown[]"))]
    pub layout: Vec<LayoutStatement>,
}

/// Project the object model into the flat wire. Rust owns every flatten/clone.
/// Signature is stable across the whole slice: Task 3 sources `concept` from
/// `Model.concepts` here without changing this signature or the wasm caller.
pub fn build_wire(model: &Model) -> WireGraph {
    WireGraph {
        nodes: model.nodes.iter().map(wire_node).collect(),
        edges: model.edges.iter().map(wire_edge).collect(),
        diagrams: model.diagrams.iter().map(wire_diagram).collect(),
        path: model.path.clone(),
        packages: model.packages.iter().map(wire_node).collect(),
        flows: model.flows.clone(),
        interactions: model.interactions.clone(),
    }
}

// Task 2: the object-model `Node` still carries `concept` + flat UML fields, so
// this copies them straight across. Task 3 rewrites the body to read `concept`
// from `Model.concepts` and the UML fields via `Node` accessors.
fn wire_node(n: &Node) -> WireNode {
    WireNode {
        concept: n.concept.clone(),
        key: n.key.clone(),
        ty: n.ty.clone(),
        stereotypes: n.stereotypes.clone(),
        abstract_: n.abstract_,
        attributes: n.attributes.clone(),
        values: n.values.clone(),
        note_body: n.note_body.clone(),
        annotates: n.annotates.clone(),
        members: n.members.clone(),
    }
}

fn wire_edge(e: &Edge) -> WireEdge {
    WireEdge {
        source: e.source.clone(),
        target: e.target.clone(),
        kind: e.kind,
        name: e.name.clone(),
        from_end: e.from_end.clone(),
        to_end: e.to_end.clone(),
        bidirectional: e.bidirectional,
    }
}

fn wire_diagram(d: &Diagram) -> WireDiagram {
    WireDiagram {
        key: d.key.clone(),
        title: d.title.clone(),
        profile: d.profile.clone(),
        description: d.description.clone(),
        members: flatten_groups(&d.groups),
        display: d.display.clone(),
        layout: d.layout.clone(),
    }
}

/// Depth-first flatten (mirrors the retired `overlay.ts::flattenGroups`): each
/// group contributes its own members, then its children recursively.
fn flatten_groups(groups: &[DiagramGroup]) -> Vec<String> {
    fn walk(g: &DiagramGroup, out: &mut Vec<String>) {
        out.extend(g.members.iter().cloned());
        for c in &g.children {
            walk(c, out);
        }
    }
    let mut out = Vec::new();
    for g in groups {
        walk(g, &mut out);
    }
    out
}
```

- [ ] **2.2 Register the module.** In `crates/waml/src/lib.rs`, add `pub mod wire;` (alphabetical). Run `cargo check -p waml`. Expected: success.

- [ ] **2.3 Point `waml-wasm` at the wire.** In `crates/waml-wasm/src/lib.rs`:
  - `build_model_json`:
```rust
pub fn build_model_json(bundle: &[(String, String)]) -> String {
    serde_json::to_string(&waml::wire::build_wire(&waml::parse::build_model(bundle))).unwrap()
}
```
  - The `#[wasm_bindgen] build_model`:
```rust
/// `bundle`: a `[path, markdown][]` (array of pairs). Returns the resolved `WireGraph`.
#[wasm_bindgen]
pub fn build_model(bundle: JsValue) -> Result<waml::wire::WireGraph, JsValue> {
    let b: Vec<(String, String)> = serde_wasm_bindgen::from_value(bundle)?;
    Ok(waml::wire::build_wire(&waml::parse::build_model(&b)))
}
```

- [ ] **2.4 Write the failing wire-shape lock test.** In `crates/waml/tests/serde_shape.rs`, append (it exercises `build_wire`, pinning the flat wire shape incl. the flattened `members`):
```rust
#[test]
fn wire_json_matches_ts_field_names() {
    let model = build_model(&bundle());
    let wire = waml::wire::build_wire(&model);
    let v = serde_json::to_value(&wire).unwrap();

    let node = &v["nodes"][0];
    assert_eq!(node["type"], "uml.Class");
    assert_eq!(node["key"], "m/order");
    assert_eq!(node["concept"]["id"], "m/order");
    assert_eq!(node["concept"]["title"], "Order");
    assert_eq!(node["attributes"][0]["name"], "id");
    assert_eq!(node["attributes"][0]["type"]["name"], "OrderId");

    let edge = &v["edges"][0];
    assert_eq!(edge["kind"], "composes");
    assert_eq!(edge["from"], "m/order");
    assert_eq!(edge["to"], "m/line");
}

#[test]
fn wire_diagram_members_are_flattened_in_rust() {
    // A diagram with a nested group forest must surface a FLAT `members` list.
    let b = vec![(
        "d.md".to_string(),
        "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n\n### Group A\n- [X](./x.md)\n\n#### Sub\n- [Y](./y.md)\n".to_string(),
    ),
        ("x.md".to_string(), "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string()),
        ("y.md".to_string(), "---\ntype: uml.Class\ntitle: Y\n---\n# Y\n".to_string()),
    ];
    let wire = waml::wire::build_wire(&build_model(&b));
    let v = serde_json::to_value(&wire).unwrap();
    let members = v["diagrams"][0]["members"].as_array().unwrap();
    assert!(members.iter().any(|m| m == "x"), "flat members must include x: {members:?}");
    assert!(members.iter().any(|m| m == "y"), "flat nested members must include y: {members:?}");
    assert!(v["diagrams"][0].get("groups").is_none(), "wire diagram has no groups: {}", v["diagrams"][0]);
}
```
  Run:
  ```
  cargo test -p waml --features serde --test serde_shape wire_
  ```
  Expected: both pass. (`model_json_matches_ts_field_names` also still passes — the object model is unchanged this task.)

- [ ] **2.5 Rewrite the diagram map in the TS bridge.** In `packages/core/src/state/overlay.ts`:
  - Change the `RustDiagram` import usage: the wire diagram now carries flat `members` and no `groups`. Remove the `flattenGroups` function (lines ~55–65) and the `RustDiagramGroup` import (it becomes unused).
  - In `toModelGraph`, the diagram map changes `members: flattenGroups(d.groups)` to `members: d.members`:
```typescript
  const diagrams: Diagram[] = model.diagrams.map((d) => ({
    key: d.key,
    title: d.title,
    profile: d.profile,
    members: d.members,
    ...(d.description !== undefined ? { description: d.description } : {}),
    ...(d.display ? { display: partialDisplayFromWire(d.display) } : {}),
  }));
```
  (`partialDisplayFromWire` and the node/edge maps are unchanged — the wire node/edge shapes are byte-identical to before.)

- [ ] **2.6 Update `overlay.test.ts` diagram inputs.** In `packages/core/src/state/overlay.test.ts`, every `RustDiagramInput`/`modelWith` diagram literal that sets `groups: [...]` must set `members: [...]` instead (the wire diagram has no `groups`). Grep the file for `groups:` and replace each diagram-construction `groups: <X>` with `members: <flattened X>`. For the two bare cases (`groups: []`), use `members: []`. Any test that asserted flattening behavior of nested groups moves its expectation to the flat `members` the wire now provides.

- [ ] **2.7 Regenerate bindings + inspect.** Run:
  ```
  pnpm build:wasm
  git diff packages/wasm/src/generated/waml_wasm.d.ts
  ```
  Expected: `build_model` now returns `WireGraph`; `WireGraph`/`WireNode`/`WireEdge`/`WireDiagram` exported. `WireNode` fields match the old `Node` (`concept`, `key`, `type`, `stereotypes`, `attributes`, …). `WireDiagram` has `members: string[]`, no `groups`. The `Model`/`Node`/`Edge`/`Diagram` wasm types may disappear from the bindings if no longer reachable from a `#[wasm_bindgen]` entry — that is expected (the object model is internal now).

- [ ] **2.8 Fix the `overlay.ts` wasm-type imports.** If `pnpm build:wasm` removed `Model`/`Node`/`Edge`/`Diagram` from the generated bindings, update the `overlay.ts` import (and the `export type { … }` re-export) to alias the wire types:
```typescript
import type {
  WireGraph as RustModel,
  WireNode as RustNode,
  WireEdge as RustEdge,
  WireDiagram as RustDiagram,
  DiagramDisplay as RustDiagramDisplay,
} from "@waml/wasm";
export type { RustModel, RustNode, RustEdge, RustDiagram, RustDiagramDisplay };
```
  (Drop `RustDiagramGroup`.) The `RustModel`/`RustNode`/… alias names stay, so `model.ts`, `overlay.test.ts`, `templates.test.ts`, `ops-adapter.test.ts` — which import these aliases — need no further change.

- [ ] **2.9 Full gate + format + commit.** Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  cargo fmt
  git add crates/waml/src/wire.rs crates/waml/src/lib.rs crates/waml-wasm/src/lib.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/wasm-inline.ts packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
  git commit -m "feat(wire): add Rust-owned wire projection; flatten diagram members in Rust"
  ```
  Expected: all green. The only intentional wire change is `diagram.members` (flat); everything else is byte-identical.
