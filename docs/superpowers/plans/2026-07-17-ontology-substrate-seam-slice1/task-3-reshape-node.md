# Task 3: Reshape `model::Node` → substrate `{ key, label, kind }`

**Load:** `README.md` + this file. Read `crates/waml/src/model.rs` (the `Node` struct + `Model` + tests), `crates/waml/src/parse.rs` (`build_node` ~442, `build_packages` ~534, `build_model` ~634), `crates/waml/src/wire.rs` (from Task 2), and `crates/waml/tests/{golden.rs, serde_shape.rs}` before starting.

**Deliverable:** The object-model `Node` becomes `{ key, label, kind: NodeKind }` (Concept off, spec §2). The parse-time OKF projection moves to `Model.concepts` (a `HashMap<node_key, Concept>`). `build_wire` re-joins concepts from that map. Native readers (`waml-editor`, `waml-cli`) read via `label` + accessors. **Wire JSON output is unchanged** — the reshape is invisible past `build_wire`. `Edge`/`Diagram` are untouched (Tasks 4–5), so `golden.rs`'s edge/diagram lines still compile.

**Files:**
- Modify: `crates/waml/src/model.rs` — reshape `Node`; add accessors + `Node::ty()`; add `Model.concepts` + `Model::concept`; update the `model_looks_up_nodes_by_key` test.
- Modify: `crates/waml/src/parse.rs` — `build_node` returns `(Node, Concept)` and maps `ElementType` → `NodeKind`; package `Node` construction in `build_packages` returns `(Node, Concept)`; `build_model` assembles `Model.concepts`.
- Modify: `crates/waml/src/wire.rs` — `wire_node` sources `concept` from `Model.concepts` + reads UML fields via accessors.
- Modify: `crates/waml-editor/src/{scene.rs, sizing.rs, tree.rs}` — read node title via `label`, `ty` via `ty()`, fields via accessors.
- Modify: `crates/waml-cli/src/main.rs` — same, at the inspect/list sites.
- Modify: `crates/waml/tests/{golden.rs, serde_shape.rs}` — update node reads/literals; retire the object-model `model_json` shape test.

**Interfaces:**
- Consumes: `NodeKind`/`uml::{UmlNode, Classifier, ClassifierKind, Structural}` (Task 1); `build_wire(&Model)` (Task 2).
- Produces: `pub struct Node { pub key: String, pub label: String, pub kind: NodeKind }`; accessors `Node::{classifier, attributes, stereotypes, values, is_abstract, members, note_body, annotates, ty}`; `Model.concepts: HashMap<String, okf::Concept>` + `Model::concept(&str) -> Option<&okf::Concept>`; `build_node(&ParsedDoc, &HashSet<&str>) -> (Node, okf::Concept)`; `build_packages(...) -> (String, Vec<(Node, okf::Concept)>)`.

---

## Steps

- [ ] **3.1 Write the failing node wire-shape lock test.** In `crates/waml/tests/serde_shape.rs`, append (proves the wire is unchanged after the reshape, and the accessors work). It builds via `build_model` + `build_wire`:
```rust
#[test]
fn node_reshape_keeps_wire_flat_and_accessors_work() {
    let m = build_model(&[(
        "shop/order.md".to_string(),
        "---\ntype: uml.Class\nstereotype: [entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n".to_string(),
    )]);
    // Object model: Concept is OFF the node; it lives in Model.concepts.
    let n = m.nodes.iter().find(|n| n.key == "shop/order").unwrap();
    assert_eq!(n.label, "Order");
    assert!(n.kind.is_classifier());
    assert_eq!(n.attributes().len(), 1);
    assert_eq!(n.stereotypes(), ["entity".to_string()]);
    assert_eq!(m.concept("shop/order").and_then(|c| c.title.as_deref()), Some("Order"));
    // Wire is still flat + carries the re-joined concept.
    let v = serde_json::to_value(waml::wire::build_wire(&m)).unwrap();
    let wn = &v["nodes"][0];
    assert_eq!(wn["type"], "uml.Class");
    assert_eq!(wn["key"], "shop/order");
    assert_eq!(wn["concept"]["title"], "Order");
    assert_eq!(wn["stereotypes"][0], "entity");
    assert_eq!(wn["attributes"][0]["name"], "id");
}
```

- [ ] **3.2 Reshape `Node` + add accessors.** In `crates/waml/src/model.rs`, replace the whole `pub struct Node { … }` (the block ending ~line 734) with:
```rust
/// A substrate node: skeleton + shared identity only (design spec §2). `key` is
/// identity; `label` is the ontology-agnostic display name (the UML render title,
/// deliberately NOT `title`). All UML payload lives behind `NodeKind::Uml`; the
/// OKF `Concept` projection lives on `Model.concepts`, not here.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Node {
    pub key: String,
    pub label: String,
    pub kind: NodeKind,
}

impl Node {
    fn uml(&self) -> Option<&crate::uml::UmlNode> {
        match &self.kind {
            NodeKind::Uml(u) => Some(u),
            NodeKind::Unknown(_) => None,
        }
    }
    pub fn classifier(&self) -> Option<&crate::uml::Classifier> {
        match self.uml() {
            Some(crate::uml::UmlNode::Classifier(c)) => Some(c),
            _ => None,
        }
    }
    pub fn attributes(&self) -> &[Attribute] {
        self.classifier().map(|c| c.attributes.as_slice()).unwrap_or(&[])
    }
    pub fn stereotypes(&self) -> &[String] {
        self.classifier().map(|c| c.stereotypes.as_slice()).unwrap_or(&[])
    }
    pub fn values(&self) -> &[String] {
        self.classifier().map(|c| c.values.as_slice()).unwrap_or(&[])
    }
    pub fn is_abstract(&self) -> bool {
        self.classifier().map(|c| c.abstract_).unwrap_or(false)
    }
    pub fn members(&self) -> &[String] {
        match self.uml() {
            Some(crate::uml::UmlNode::Structural(crate::uml::Structural::Package { members })) => {
                members.as_slice()
            }
            _ => &[],
        }
    }
    pub fn note_body(&self) -> Option<&str> {
        match self.uml() {
            Some(crate::uml::UmlNode::Structural(crate::uml::Structural::Note { body, .. })) => {
                body.as_deref()
            }
            _ => None,
        }
    }
    pub fn annotates(&self) -> &[NoteAnchor] {
        match self.uml() {
            Some(crate::uml::UmlNode::Structural(crate::uml::Structural::Note { annotates, .. })) => {
                annotates.as_slice()
            }
            _ => &[],
        }
    }
    /// Recompute the parse-time `ElementType` token for legacy/native readers and
    /// the wire (`WireNode.ty`). Slice 1 nodes are only Classifier/Structural/Unknown;
    /// Behavior/Lifeline nodes are constructed by later slices.
    pub fn ty(&self) -> ElementType {
        match &self.kind {
            NodeKind::Uml(crate::uml::UmlNode::Classifier(c)) => {
                ElementType::parse(&format!("uml.{}", c.kind.name()))
            }
            NodeKind::Uml(crate::uml::UmlNode::Structural(crate::uml::Structural::Package { .. })) => {
                ElementType::Uml(UmlMetaclass::Package)
            }
            NodeKind::Uml(crate::uml::UmlNode::Structural(crate::uml::Structural::Note { .. })) => {
                ElementType::Uml(UmlMetaclass::Note)
            }
            NodeKind::Uml(crate::uml::UmlNode::Behavior(_))
            | NodeKind::Uml(crate::uml::UmlNode::Lifeline(_)) => {
                unreachable!("behavior/lifeline nodes are constructed in slices 2–3")
            }
            NodeKind::Unknown(s) => ElementType::Unknown(s.clone()),
        }
    }
}
```

- [ ] **3.3 Add `Model.concepts` + `Model::concept`.** In `crates/waml/src/model.rs`, add a field to `pub struct Model` (after `interactions`) and a helper. First add `use std::collections::HashMap;` at the top of `model.rs` if not present (`use crate::multiplicity::Multiplicity;` is line 1 — add the `HashMap` use alongside). Then the field:
```rust
    /// Parse-time OKF projection of each node's source document (design spec §2:
    /// Concept is a projection of storage, off the object-model `Node`). Keyed by
    /// `node.key` (packages key by their dir; the `Concept.id` inside keeps its
    /// natural path). `build_wire` re-joins it onto the wire; native readers use
    /// `Model::concept`. Not part of the wire — the wire re-flattens it.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "HashMap::is_empty"))]
    #[cfg_attr(feature = "wasm", tsify(type = "Record<string, Concept>"))]
    pub concepts: HashMap<String, crate::okf::Concept>,
```
And in `impl Model`, next to `node`:
```rust
    pub fn concept(&self, key: &str) -> Option<&crate::okf::Concept> {
        self.concepts.get(key)
    }
```

- [ ] **3.4 Update the `model_looks_up_nodes_by_key` test.** In `crates/waml/src/model.rs` `mod tests`, replace that test (~lines 961–981) with the new-shape version:
```rust
    #[test]
    fn model_looks_up_nodes_by_key() {
        use crate::uml::{Classifier, ClassifierKind, UmlNode};
        let node = Node {
            key: "order".to_string(),
            label: "Order".to_string(),
            kind: NodeKind::Uml(UmlNode::Classifier(Classifier {
                kind: ClassifierKind::Class,
                stereotypes: vec![],
                abstract_: false,
                attributes: vec![],
                values: vec![],
            })),
        };
        let model = Model { nodes: vec![node], ..Default::default() };
        assert_eq!(model.node("order").map(|n| n.label.as_str()), Some("Order"));
        assert!(model.node("missing").is_none());
    }
```

- [ ] **3.5 Rewrite `build_node` to return `(Node, Concept)` + map to `NodeKind`.** In `crates/waml/src/parse.rs`, replace `build_node` (~442–469) with:
```rust
fn build_node(p: &ParsedDoc, keyset: &HashSet<&str>) -> (Node, crate::okf::Concept) {
    use crate::uml::{Classifier, ClassifierKind, Structural, UmlNode};
    let fm = &p.doc.frontmatter;
    let mut attributes = Vec::new();
    let mut values = Vec::new();
    let mut body = None;
    for s in &p.doc.sections {
        match s {
            Section::Attributes(a) => {
                attributes = a.iter().filter_map(Line::parsed).map(|x| resolve_attr(x, &p.path, keyset)).collect()
            }
            Section::Values(v) => values = v.iter().filter_map(Line::parsed).cloned().collect(),
            Section::Body(b) => body = Some(b.clone()),
            _ => {}
        }
    }
    let stereotypes = fm.get_string_list("stereotype");
    let abstract_ = fm.get_bool("abstract") == Some(true);

    // Map the parse-time ElementType onto the ontology seam. `classifiers` in
    // `build_model` already excludes Diagram/Behavior/index/log; Package is built
    // in `build_packages`. So only Classifier metaclasses, Note, and Unknown reach here.
    let kind = match &p.ty {
        ElementType::Uml(mc) => match ClassifierKind::parse(mc.name()) {
            Some(ck) => NodeKind::Uml(UmlNode::Classifier(Classifier {
                kind: ck,
                stereotypes,
                abstract_,
                attributes,
                values,
            })),
            // Only `Note` has no ClassifierKind here (Package is elsewhere).
            None => NodeKind::Uml(UmlNode::Structural(Structural::Note {
                body,
                annotates: Vec::new(), // deferred: uml.Note anchors
            })),
        },
        ElementType::Unknown(s) => NodeKind::Unknown(s.clone()),
        ElementType::Diagram | ElementType::Behavior(_) => {
            unreachable!("Diagram/Behavior docs are not built as nodes")
        }
    };

    let node = Node { key: p.id.clone(), label: doc_title(p), kind };
    (node, p.concept.clone())
}
```

- [ ] **3.6 Make package `Node` construction return `(Node, Concept)`.** In `crates/waml/src/parse.rs` `build_packages`, change the return type to `(String, Vec<(Node, crate::okf::Concept)>)` and the `.map(|d| { … Node { … } })` closure to build the new-shape package node and return the pair. Replace the `Node { concept, key: d.clone(), ty: …Package, …, members }` construction (~611–622) with:
```rust
            let node = Node {
                key: d.clone(),
                label: title.clone(),
                kind: crate::model::NodeKind::Uml(crate::uml::UmlNode::Structural(
                    crate::uml::Structural::Package { members },
                )),
            };
            (node, concept)
```
(The synthesized `concept` — with its pinned title + intro description — is returned unchanged as the pair's second element. Its `Concept.id` stays the index path, e.g. `"sales/index"`; the pair is keyed into `Model.concepts` by `node.key` in `build_model`.) The function's final `(path, packages)` return becomes `(path, packages)` where `packages: Vec<(Node, Concept)>`.

- [ ] **3.7 Assemble `Model.concepts` in `build_model`.** In `crates/waml/src/parse.rs` `build_model` (~634–672), collect the concepts from both node sources. Replace the `nodes`/`packages`/`Model { … }` region with:
```rust
    let node_pairs: Vec<(Node, crate::okf::Concept)> =
        classifiers.iter().map(|p| build_node(p, &keyset)).collect();
    let edges: Vec<Edge> = build_edges(&classifiers, &keyset);
    let diagrams: Vec<Diagram> = build_diagrams(&parsed, &keyset);

    let docs: Vec<(String, String, String)> = parsed
        .iter()
        .filter(|p| p.slug != "index" && p.slug != "log")
        .map(|p| (p.path.clone(), p.id.clone(), doc_title(p)))
        .collect();
    let indexes: std::collections::BTreeMap<String, String> = bundle
        .iter()
        .filter(|(path, _)| doc_slug(path) == "index")
        .map(|(path, text)| (dir_of(path), text.clone()))
        .collect();
    let (path, package_pairs) = build_packages(&docs, &indexes);

    let flows = build_flows(&parsed, &keyset);
    let interactions = build_interactions(&parsed, &keyset);

    let mut concepts: std::collections::HashMap<String, crate::okf::Concept> =
        std::collections::HashMap::new();
    let mut nodes = Vec::with_capacity(node_pairs.len());
    for (n, c) in node_pairs {
        concepts.insert(n.key.clone(), c);
        nodes.push(n);
    }
    let mut packages = Vec::with_capacity(package_pairs.len());
    for (n, c) in package_pairs {
        concepts.insert(n.key.clone(), c);
        packages.push(n);
    }

    Model { nodes, edges, diagrams, path, packages, flows, interactions, concepts }
```

- [ ] **3.8 Switch `wire_node` to the concept map + accessors.** In `crates/waml/src/wire.rs`, replace `wire_node` and its call sites in `build_wire`. Add `use std::collections::HashMap;` to `wire.rs`. New `wire_node`:
```rust
fn wire_node(n: &Node, concepts: &HashMap<String, Concept>) -> WireNode {
    WireNode {
        concept: concepts
            .get(&n.key)
            .cloned()
            .expect("every wire node has a Concept in Model.concepts (build_model invariant)"),
        key: n.key.clone(),
        ty: n.ty(),
        stereotypes: n.stereotypes().to_vec(),
        abstract_: n.is_abstract(),
        attributes: n.attributes().to_vec(),
        values: n.values().to_vec(),
        note_body: n.note_body().map(str::to_string),
        annotates: n.annotates().to_vec(),
        members: n.members().to_vec(),
    }
}
```
And in `build_wire`, the two node maps pass the concept map:
```rust
        nodes: model.nodes.iter().map(|n| wire_node(n, &model.concepts)).collect(),
        // …
        packages: model.packages.iter().map(|n| wire_node(n, &model.concepts)).collect(),
```
(`build_wire`'s `&Model` signature and the `waml-wasm` caller are unchanged.)

- [ ] **3.9 Run the Rust crate tests.** Run:
  ```
  cargo test -p waml
  ```
  Expected: compile errors in `golden.rs`, `serde_shape.rs`, and the native crates (`waml-editor`, `waml-cli`) at node-field read sites — fix them in 3.10–3.12. The `waml` library + its `serde_shape` wire tests should be reachable once those compile.

- [ ] **3.10 Update native readers via `label`/accessors.** In `crates/waml-editor/src`:
  - `scene.rs:41` — `n.concept.title.clone().unwrap_or_else(|| n.key.clone())` → `n.label.clone()`.
  - `tree.rs:57` — `n.concept.title.clone().unwrap_or_else(|| n.key.clone())` → `n.label.clone()`.
  - `tree.rs:58` — `kind_of(&n.ty)` → `kind_of(&n.ty())`.
  - `tree.rs:64` — `p.concept.title.clone().unwrap_or_else(|| p.key.clone())` → `p.label.clone()`.
  - `tree.rs:85` — `build_children(&root_pkg.members, …)` → `build_children(root_pkg.members(), …)`.
  - `tree.rs:127` — `.map(|p| build_children(&p.members, …))` → `.map(|p| build_children(p.members(), …))`.
  - `sizing.rs:20,22` — `node.attributes` → `node.attributes()` (both reads).

  In `crates/waml-cli/src/main.rs`:
  - `:499` — `node.concept.title.as_deref().unwrap_or("Untitled")` → `&node.label`; `node.ty.as_str()` → `node.ty().as_str()`.
  - `:500` — `for a in &node.attributes` → `for a in node.attributes()`.
  - `:503` — `for v in &node.values` → `for v in node.values()`.
  - `:514` — `"title": node.concept.title.as_deref().unwrap_or("Untitled")` → `"title": node.label`; `node.ty.as_str()` → `node.ty().as_str()`.
  - `:515` — `node.attributes.iter()` → `node.attributes().iter()`.
  - `:518` — `"values": node.values` → `"values": node.values()`.
  - `:592` — `n.ty.as_str()` → `n.ty().as_str()`.
  - `:594` — `n.ty.as_str()` → `n.ty().as_str()`; `n.concept.title.as_deref().unwrap_or("Untitled")` → `&n.label`.
  - `:596` — same two substitutions as `:594`.

  Note (cosmetic behavior change): when a doc has no frontmatter `title`, the CLI printed `"Untitled"`; it now prints `label` (frontmatter title → H1 → slug, never empty). If a `waml-cli` test asserts the literal `"Untitled"`, update it. Grep to confirm: `grep -rn Untitled crates/waml-cli`.

- [ ] **3.11 Update `golden.rs` node reads.** In `crates/waml/tests/golden.rs`, in `orders_domain_builds_the_expected_model`:
  - `:38–39` — `money.attributes[0]` → `money.attributes()[0]` (both `.ty.name` / `.ty.ref_` reads).
  - `:44` — `assert_eq!(order.concept.title.as_deref(), Some("Order"));` → `assert_eq!(order.label, "Order");`.
  - `:45` — `order.attributes.len()` → `order.attributes().len()`.
  - `:46` — `order.attributes.iter()` → `order.attributes().iter()`.
  (Leave the edge lines `:18,23–34` and the diagram line `:14` — those use the still-old `Edge`/`Diagram` and are updated in Tasks 4/5.)

- [ ] **3.12 Update `serde_shape.rs` literals + retire the object-model shape test.** In `crates/waml/tests/serde_shape.rs`:
  - Delete `model_json_matches_ts_field_names` (~lines 22–47): it pinned the *object-model* `Model` JSON to TS names; that job now belongs to the wire tests (`wire_json_matches_ts_field_names`, `node_reshape_keeps_wire_flat_and_accessors_work`). The object model is internal and intentionally nested now.
  - Rewrite `package_node_and_model_path` (~76–116) `Node` literals to the new shape (drop `concept`; the test only asserts `path`/`members` on the serialized `Model`, which still contains the `"members":["order","customer"]` substring nested under `kind`):
```rust
#[test]
fn package_node_and_model_path() {
    use waml::model::NodeKind;
    use waml::uml::{Classifier, ClassifierKind, Structural, UmlNode};
    let pkg = Node {
        key: "sales".into(),
        label: "sales".into(),
        kind: NodeKind::Uml(UmlNode::Structural(Structural::Package {
            members: vec!["order".into(), "customer".into()],
        })),
    };
    let model = Model {
        nodes: vec![],
        edges: vec![],
        diagrams: vec![],
        path: "acme-model".into(),
        packages: vec![pkg],
        ..Default::default()
    };
    let json = serde_json::to_string(&model).unwrap();
    assert!(json.contains("\"path\":\"acme-model\""));
    assert!(json.contains("\"members\":[\"order\",\"customer\"]"));
    let bare = Node {
        key: "order".into(),
        label: "Order".into(),
        kind: NodeKind::Uml(UmlNode::Classifier(Classifier {
            kind: ClassifierKind::Class,
            stereotypes: vec![],
            abstract_: false,
            attributes: vec![],
            values: vec![],
        })),
    };
    let bj = serde_json::to_string(&bare).unwrap();
    assert!(!bj.contains("members"), "a classifier has no members field: {bj}");
}
```
  - Fix the top-of-file `use` list: `Node` is still used; add nothing there (the new imports are function-local `use` above). Remove `UmlMetaclass` from the top import only if it becomes unused — it is still used by `classifier_type_wire_strings_are_stable`, so keep it. Keep `ElementType`, `BehaviorKind`, `AssocName`, `Visibility`.

- [ ] **3.13 Run the `waml` gate slice.** Run:
  ```
  cargo fmt
  cargo test -p waml -p waml-editor -p waml-cli
  ```
  Expected: green. A `no field 'attributes' on type '&Node'` error means a missed accessor swap (3.10/3.11).

- [ ] **3.14 Full gate + commit.** The wire output is unchanged, so `pnpm build:wasm` regenerates identical bindings (only the internal object-model types differ, and they are not on the wire). Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/wire.rs crates/waml-editor/src crates/waml-cli/src/main.rs crates/waml/tests/golden.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/wasm-inline.ts
  git commit -m "refactor(model): reshape Node onto the substrate; Concept off-node via Model.concepts"
  ```
  Expected: green. `git diff` on the generated bindings should show **no wire change** for nodes (WireNode is byte-identical); if `Node`/`Model` vanished from the bindings that is expected (internal-only now).
