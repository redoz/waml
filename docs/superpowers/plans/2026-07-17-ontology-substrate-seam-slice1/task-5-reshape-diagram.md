# Task 5: Reshape `model::Diagram` → substrate `{ key, label, kind }`

**Load:** `README.md` + this file. Read `crates/waml/src/model.rs` (the `Diagram` struct ~747–764), `crates/waml/src/parse.rs` (`build_diagrams` ~940–991 + its unit tests ~1016–1062, 1371–1383), `crates/waml/src/solve/resolve.rs` (~254–258), and `crates/waml/src/wire.rs` (`wire_diagram`) before starting.

**Deliverable:** The object-model `Diagram` becomes `{ key, label, kind: DiagramKind }` with the render payload behind `DiagramKind::Uml(UmlDiagram { flavor: Class, profile, description, groups, display, layout })`. Accessors keep internal readers (the solver, index generation, ops helpers, tests) mechanical. `build_wire` re-flattens to the unchanged `WireDiagram` (flat `members`, `title`, etc.). **Wire JSON unchanged.** This is the last object-model reshape — after it, the substrate (Node/Edge/Diagram) is fully behind the seam.

**Files:**
- Modify: `crates/waml/src/model.rs` — reshape `Diagram`; add accessors.
- Modify: `crates/waml/src/parse.rs` — rewrite `build_diagrams` construction; update its unit tests to accessors.
- Modify: `crates/waml/src/wire.rs` — `wire_diagram` via accessors; `title` from `label`.
- Modify: `crates/waml/src/solve/resolve.rs` — read `groups`/`layout` via accessors.
- Modify: `crates/waml/src/index_md.rs`, `crates/waml/src/ops/pkg.rs`, `crates/waml/src/seed.rs` — read the diagram title via `label`.
- Modify: `crates/waml/tests/{golden.rs, layout_serde_roundtrip.rs}` — read `groups`/`layout` via accessors.

**Interfaces:**
- Consumes: `DiagramKind`/`uml::{UmlDiagram, UmlDiagramFlavor}` (Task 1).
- Produces: `pub struct Diagram { pub key: String, pub label: String, pub kind: DiagramKind }`; accessors `Diagram::{flavor, profile, description, groups, display, layout}`.

---

## Steps

- [ ] **5.1 Write the failing diagram lock test.** In `crates/waml/tests/serde_shape.rs`, append:
```rust
#[test]
fn diagram_reshape_keeps_wire_flat_and_accessors_work() {
    let b = vec![
        ("x.md".to_string(), "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string()),
        (
            "d.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [X](./x.md)\n".to_string(),
        ),
    ];
    let m = build_model(&b);
    let d = m.diagrams.iter().find(|d| d.key == "d").unwrap();
    assert_eq!(d.label, "D");
    assert_eq!(d.profile(), "uml-domain");
    assert_eq!(d.flavor(), Some(waml::uml::UmlDiagramFlavor::Class));
    assert_eq!(d.groups().iter().map(|g| g.members.len()).sum::<usize>(), 1);
    // Wire stays flat: title/profile/members.
    let v = serde_json::to_value(waml::wire::build_wire(&m)).unwrap();
    let wd = &v["diagrams"][0];
    assert_eq!(wd["title"], "D");
    assert_eq!(wd["profile"], "uml-domain");
    assert_eq!(wd["members"][0], "x");
    assert!(wd.get("groups").is_none());
}
```

- [ ] **5.2 Reshape `Diagram` + add accessors.** In `crates/waml/src/model.rs`, replace the whole `pub struct Diagram { … }` (lines ~747–764) with:
```rust
/// A substrate diagram: identity + display name + ontology payload (design spec
/// §2/§3.3). `label` is the render title (not `title` — that is OKF storage). All
/// UML render concerns (flavor/profile/groups/display/layout) live behind
/// `DiagramKind::Uml`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "wasm", derive(tsify_next::Tsify))]
#[cfg_attr(feature = "wasm", tsify(into_wasm_abi, from_wasm_abi))]
pub struct Diagram {
    pub key: String,
    pub label: String,
    pub kind: DiagramKind,
}

impl Diagram {
    fn uml(&self) -> Option<&crate::uml::UmlDiagram> {
        match &self.kind {
            DiagramKind::Uml(u) => Some(u),
            DiagramKind::Unknown(_) => None,
        }
    }
    pub fn flavor(&self) -> Option<crate::uml::UmlDiagramFlavor> {
        self.uml().map(|u| u.flavor)
    }
    pub fn profile(&self) -> &str {
        self.uml().map(|u| u.profile.as_str()).unwrap_or("")
    }
    pub fn description(&self) -> Option<&str> {
        self.uml().and_then(|u| u.description.as_deref())
    }
    pub fn groups(&self) -> &[DiagramGroup] {
        self.uml().map(|u| u.groups.as_slice()).unwrap_or(&[])
    }
    pub fn display(&self) -> Option<&DiagramDisplay> {
        self.uml().map(|u| &u.display)
    }
    pub fn layout(&self) -> &[crate::syntax::LayoutStatement] {
        self.uml().map(|u| u.layout.as_slice()).unwrap_or(&[])
    }
}
```

- [ ] **5.3 Rewrite the `build_diagrams` construction.** In `crates/waml/src/parse.rs`, replace the final `out.push(Diagram { … })` (~988) with:
```rust
        out.push(Diagram {
            key: p.id.clone(),
            label: title,
            kind: crate::model::DiagramKind::Uml(crate::uml::UmlDiagram {
                flavor: crate::uml::UmlDiagramFlavor::Class,
                profile,
                description,
                groups,
                display,
                layout,
            }),
        });
```

- [ ] **5.4 Update `build_diagrams`' own unit tests to accessors.** In `crates/waml/src/parse.rs` `#[cfg(test)] mod tests` (the diagram tests ~1016–1062 and ~1371–1383), change every read of a `model::Diagram`'s reshaped field:
  - `let x = &d.display;` (~1016) → `let x = d.display().unwrap();`.
  - Each `<...>.diagrams[0].display.<field>` (~1033, 1035, 1041, 1043, 1045, 1048, 1053, 1054, 1055) → `<...>.diagrams[0].display().unwrap().<field>`.
  - `m.diagrams[0].display.is_empty()` (~1062) → `m.diagrams[0].display().unwrap().is_empty()`.
  - `d.groups.len()` / `d.groups[0].name` / `d.groups[0].members` (~1379–1381) → `d.groups().len()` / `d.groups()[0].name` / `d.groups()[0].members`.
  - `d.layout.len()` / `d.layout[0]` (~1382–1383) → `d.layout().len()` / `d.layout()[0]`.

- [ ] **5.5 Rewrite `wire_diagram` via accessors.** In `crates/waml/src/wire.rs`, replace `wire_diagram`:
```rust
fn wire_diagram(d: &Diagram) -> WireDiagram {
    WireDiagram {
        key: d.key.clone(),
        title: d.label.clone(),
        profile: d.profile().to_string(),
        description: d.description().map(str::to_string),
        members: flatten_groups(d.groups()),
        display: d.display().cloned().unwrap_or_default(),
        layout: d.layout().to_vec(),
    }
}
```
(`flatten_groups` is unchanged. `DiagramDisplay` derives `Default`, so `unwrap_or_default` is valid.)

- [ ] **5.6 Update the solver.** In `crates/waml/src/solve/resolve.rs`:
  - `:254` — `for g in &diagram.groups {` → `for g in diagram.groups() {`.
  - `:257` — `for stmt in &diagram.layout {` → `for stmt in diagram.layout() {`.
  (`diagram.key` at `:255`/`:258` is unchanged.)

- [ ] **5.7 Update the diagram-title readers.** These read a built `model::Diagram`'s title (now `label`):
  - `crates/waml/src/index_md.rs:62` — `d.title.clone()` → `d.label.clone()`.
  - `crates/waml/src/ops/pkg.rs:77` — `.map(|d| d.title.clone())` → `.map(|d| d.label.clone())`.
  - `crates/waml/src/ops/pkg.rs:126` — `.map(|d| (d.title.clone(), false, None))` → `.map(|d| (d.label.clone(), false, None))`.
  (These read the substrate `label` field only — no `uml::*` dependency, so the "ops speak tokens" constraint holds.)

- [ ] **5.8 Update the remaining tests.** 
  - `crates/waml/src/seed.rs:88` — `d.title == "My Domain"` → `d.label == "My Domain"`.
  - `crates/waml/tests/golden.rs:14` — `m.diagrams[0].groups.iter()` → `m.diagrams[0].groups().iter()`.
  - `crates/waml/tests/layout_serde_roundtrip.rs:23` — `diagram.layout.is_empty()` → `diagram.layout().is_empty()`; `:28` — `assert_eq!(back.layout, diagram.layout, …)` → `assert_eq!(back.layout(), diagram.layout(), …)`. (The `serde_json::to_string(diagram)` / `from_str::<Diagram>` round-trip at `:25–26` is unchanged — the reshaped `Diagram` serde is symmetric.)

- [ ] **5.9 Run the `waml` gate slice.** Run:
  ```
  cargo fmt
  cargo test -p waml
  ```
  Expected: green. A `no field 'groups' on type '&Diagram'` error means a missed accessor swap.

- [ ] **5.10 Full gate + commit.** Wire output unchanged (WireDiagram byte-identical to Task 2). Run in order:
  ```
  cargo test --workspace
  pnpm build:wasm
  pnpm lint
  pnpm build
  pnpm -r test
  cargo fmt
  git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/wire.rs crates/waml/src/solve/resolve.rs crates/waml/src/index_md.rs crates/waml/src/ops/pkg.rs crates/waml/src/seed.rs crates/waml/tests/golden.rs crates/waml/tests/layout_serde_roundtrip.rs crates/waml/tests/serde_shape.rs packages/wasm/src/generated/waml_wasm.d.ts packages/wasm/src/generated/wasm-inline.ts
  git commit -m "refactor(model): reshape Diagram onto the substrate + DiagramKind seam"
  ```
  Expected: green, no wire change for diagrams. The substrate + seam are now complete: `Node`/`Edge`/`Diagram` are `{ identity, label/endpoints, kind }` with all UML behind `*Kind::Uml`, `is_classifier` is a `NodeKind` variant check, Concept lives on `Model.concepts`, and Rust owns the flat wire projection.
