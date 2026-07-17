# Task 4: Reshape `model::Edge` → substrate `{ source, target, kind }`

**Load:** `README.md` + this file. Read `crates/waml/src/model.rs` (the `Edge` struct), `crates/waml/src/parse.rs` (`build_edges` ~854–922), `crates/waml/src/wire.rs` (`wire_edge`), and `crates/waml/tests/golden.rs` before starting.

**Deliverable:** The object-model `Edge` becomes `{ source, target, kind: EdgeKind }` with the association payload behind `EdgeKind::Uml(UmlEdge::Relationship(Relationship))`. Accessors keep native readers mechanical. `build_wire` re-flattens to the unchanged `WireEdge`, so the wire JSON is byte-identical. Slice 1 only ever constructs `Relationship` edges. `Node` (Task 3) and `Diagram` (Task 5) are untouched.

**Files:**
- Modify: `crates/waml/src/model.rs` — reshape `Edge`; add accessors.
- Modify: `crates/waml/src/parse.rs` — rewrite `build_edges` to emit `EdgeKind::Uml(UmlEdge::Relationship(..))`.
- Modify: `crates/waml/src/wire.rs` — `wire_edge` reads via accessors.
- Modify: `crates/waml-editor/src/scene.rs`, `crates/waml-cli/src/main.rs` — read edge `kind` via the accessor.
- Modify: `crates/waml/tests/golden.rs` — edge reads via accessors.

**Interfaces:**
- Consumes: `EdgeKind`/`uml::{UmlEdge, Relationship}` (Task 1).
- Produces: `pub struct Edge { pub source: String, pub target: String, pub kind: EdgeKind }` (serde `source`→`"from"`, `target`→`"to"`); accessors `Edge::{relationship, rel_kind, name, from_end, to_end, bidirectional}`.

---

## Steps

- [ ] **4.1 Write the failing edge wire + object-model lock test.** In `crates/waml/tests/serde_shape.rs`, append:
```rust
#[test]
fn edge_reshape_keeps_wire_flat_and_accessors_work() {
    use waml::model::RelationshipKind;
    let m = build_model(&bundle()); // order composes line (see bundle())
    let e = m.edges.iter().find(|e| e.rel_kind() == Some(RelationshipKind::Composes)).unwrap();
    assert_eq!(e.source, "m/order");
    assert_eq!(e.target, "m/line");
    assert!(e.relationship().is_some());
    // Wire stays flat: from/to/kind unchanged.
    let v = serde_json::to_value(waml::wire::build_wire(&m)).unwrap();
    let we = v["edges"].as_array().unwrap().iter().find(|e| e["kind"] == "composes").unwrap();
    assert_eq!(we["from"], "m/order");
    assert_eq!(we["to"], "m/line");
    assert_eq!(we["bidirectional"], false);
}
```

- [ ] **4.2 Reshape `Edge` + add accessors.** In `crates/waml/src/model.rs`, replace the whole `pub struct Edge { … }` (lines ~220–243) with:
```rust
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Edge {
    #[cfg_attr(feature = "serde", serde(rename = "from"))]
    pub source: String,
    #[cfg_attr(feature = "serde", serde(rename = "to"))]
    pub target: String,
    pub kind: EdgeKind,
}

impl Edge {
    pub fn relationship(&self) -> Option<&crate::uml::Relationship> {
        match &self.kind {
            EdgeKind::Uml(crate::uml::UmlEdge::Relationship(r)) => Some(r),
            _ => None,
        }
    }
    pub fn rel_kind(&self) -> Option<RelationshipKind> {
        self.relationship().map(|r| r.kind)
    }
    pub fn name(&self) -> Option<&AssocName> {
        self.relationship().and_then(|r| r.name.as_ref())
    }
    pub fn from_end(&self) -> Option<&RelEnd> {
        self.relationship().map(|r| &r.from_end)
    }
    pub fn to_end(&self) -> Option<&RelEnd> {
        self.relationship().map(|r| &r.to_end)
    }
    pub fn bidirectional(&self) -> bool {
        self.relationship().map(|r| r.bidirectional).unwrap_or(false)
    }
}
```
(`AssocName` and its hand-written serde impls stay in `model.rs` unchanged — reused by `uml::Relationship`.)

- [ ] **4.3 Run the lock test.** Run:
  ```
  cargo test -p waml --features serde --test serde_shape edge_reshape
  ```
  Expected: PASS once `build_edges` (4.4) compiles. (It fails to compile until `build_edges` emits the new shape.)

- [ ] **4.4 Rewrite `build_edges` to emit `EdgeKind::Uml(UmlEdge::Relationship(..))`.** In `crates/waml/src/parse.rs`:
  - At the `use crate::model::{AssocName, RelationshipKind};` line (~851), add `EdgeKind` and a `use crate::uml::{Relationship, UmlEdge};`.
  - Replace the reciprocal-associate mutation block (~881–890):
```rust
                    if let Some(&idx) = assoc_pair.get(&key) {
                        if let EdgeKind::Uml(UmlEdge::Relationship(r)) = &mut edges[idx].kind {
                            r.bidirectional = true;
                            r.from_end.navigable = Some(true);
                            r.to_end.navigable = Some(true);
                            if r.name.is_none() && name.is_some() {
                                r.name = name;
                            }
                        }
                        continue;
                    }
```
  - Replace the first `edges.push(Edge { … })` (associates branch, ~893–901):
```rust
                    let mut to_end = r.to_end.clone();
                    to_end.navigable = Some(true);
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: EdgeKind::Uml(UmlEdge::Relationship(Relationship {
                            kind: RelationshipKind::Associates,
                            name,
                            from_end: r.from_end.clone(),
                            to_end,
                            bidirectional: false,
                        })),
                    });
```
  - Replace the second `edges.push(Edge { … })` (non-associates branch, ~908–916):
```rust
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: EdgeKind::Uml(UmlEdge::Relationship(Relationship {
                            kind: r.kind,
                            name,
                            from_end: r.from_end.clone(),
                            to_end: r.to_end.clone(),
                            bidirectional: false,
                        })),
                    });
```

- [ ] **4.5 Rewrite `wire_edge` via accessors.** In `crates/waml/src/wire.rs`, replace `wire_edge`:
```rust
fn wire_edge(e: &Edge) -> WireEdge {
    let r = e.relationship();
    WireEdge {
        source: e.source.clone(),
        target: e.target.clone(),
        kind: e.rel_kind().unwrap_or(RelationshipKind::Associates),
        name: r.and_then(|r| r.name.clone()),
        from_end: r.map(|r| r.from_end.clone()).unwrap_or_default(),
        to_end: r.map(|r| r.to_end.clone()).unwrap_or_default(),
        bidirectional: e.bidirectional(),
    }
}
```
(`RelationshipKind` and `RelEnd` are already imported in `wire.rs`.)

- [ ] **4.6 Update native edge readers.** In `crates/waml-editor/src/scene.rs:62`, the `SceneEdge` construction reads the model edge's kind: change `kind: e.kind` to `kind: e.rel_kind().unwrap_or(RelationshipKind::Associates)` (ensure `RelationshipKind` is in scope in `scene.rs`; it is already used at `:117`). `e.source`/`e.target` at `:60` are unchanged (still fields). In `crates/waml-cli/src/main.rs:507`, change `e.kind.as_str()` to `e.rel_kind().map(|k| k.as_str()).unwrap_or("")`; `e.source`/`e.target` at `:506–507` are unchanged.

- [ ] **4.7 Update `golden.rs` edge reads.** In `crates/waml/tests/golden.rs` `orders_domain_builds_the_expected_model`, add `use waml::model::RelationshipKind;` at the top, then:
  - `:18` — `m.edges.iter().map(|e| e.kind.as_str())` → `m.edges.iter().map(|e| e.rel_kind().unwrap().as_str())`.
  - `:23` — `.find(|e| e.kind.as_str() == "composes")` → `.find(|e| e.rel_kind() == Some(RelationshipKind::Composes))`.
  - `:26` — `comp.to_end.role.as_deref()` → `comp.to_end().unwrap().role.as_deref()`.
  - `:30` — `.find(|e| e.kind.as_str() == "associates")` → `.find(|e| e.rel_kind() == Some(RelationshipKind::Associates))`.
  - `:33` — `assoc.from_end.role.as_deref()` → `assoc.from_end().unwrap().role.as_deref()`.
  - `:34` — `assoc.to_end.role.as_deref()` → `assoc.to_end().unwrap().role.as_deref()`.
  (`comp.source`/`comp.target`/`assoc.source`/`assoc.target` are unchanged.)

- [ ] **4.8 Run the `waml` gate slice.** Run:
  ```
  cargo fmt
  cargo test -p waml -p waml-editor -p waml-cli
  ```
  Expected: green.

- [ ] **4.9 Full gate + commit.** Wire output unchanged (WireEdge byte-identical), so bindings regenerate identical. Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/wire.rs crates/waml-editor/src/scene.rs crates/waml-cli/src/main.rs crates/waml/tests/golden.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/wasm-inline.ts
  git commit -m "refactor(model): reshape Edge onto the substrate + EdgeKind seam"
  ```
  Expected: green, no wire change for edges.
