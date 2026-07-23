# Inspector: Groups & Edges Selectable Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a diagram's groups and edges selectable in the native `waml-editor` inspector's element-picker, projecting a group into a new MEMBERS compartment and an edge into a titled view.

**Architecture:** The inspector splits into a pure projection layer (`crates/waml-editor/src/inspector.rs`: `Subject` → `InspectorView`, plus the picker-row model) and the makepad widget (`crates/waml-editor/src/inspector_panel.rs`) that hand-draws the view. This change adds two `Subject` variants, an `ElementKind::Group` picker row, a `members` field on the view, and the panel wiring that constructs/renders them. Groups live on `Diagram.groups: Vec<DiagramGroup>` (a recursive `{ name, members, children }` tree); the implicit top-level group has `name == ""` and is skipped.

**Tech Stack:** Rust (workspace), makepad (native GPU widget toolkit, forked), pnpm/vitest (web — untouched here).

## Global Constraints

- Native `waml-editor` inspector ONLY. No web (`ElementPicker.svelte`) changes, no `card/mod.rs` changes.
- No new catalog icon. Reuse existing `Icon::SquareDashedTopSolid` (verified present: `crates/waml-editor/src/icons.rs:3547`) for groups and existing `Icon::Spline` (`crates/waml-editor/src/icons.rs:3485`) for edges.
- Group `Subject` resolution is by name across all diagrams' group trees, **first match wins** (no diagram context in `build_view`). This is an accepted limitation for this iteration.
- Diagram and Placeholder picker rows stay disabled / no-op (no `Subject::Diagram` view in scope).
- The gate EVERY task must pass on its own (leaving the tree green): `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. Clippy runs with `-D warnings`, so **dead_code is a hard error** — `waml-editor` is a **binary crate** (no `[lib]` in `crates/waml-editor/Cargo.toml`), so `pub` does NOT suppress dead_code: every added enum variant needs a non-test **constructor** and every added struct field needs a non-test **reader** within the same task.
- The source `Model` is never mutated; title/description edits stay in the panel's in-memory `overrides` map (existing behavior).

## Tasks

Three tasks. Task 1 adds a named group to the shared `mini` fixture (needed for the group tests) and guards it. Task 2 adds the `ElementKind::Group` picker row (variant constructed in non-test `diagram_elements`). Task 3 lands the two `Subject` variants, the `members` field, `build_view`/`subject_to_index`, AND the panel wiring in one unit — the variants and field are only dead_code-clean when their non-test constructor (`apply_pick`) and reader (MEMBERS draw) land alongside them.

---

### Task 1: Add a named group to the `mini` fixture

**Files:**
- Modify: `crates/waml-editor/tests/fixtures/mini/orders-diagram.md`
- Test: `crates/waml-editor/src/inspector.rs` (append one test to the existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing new.
- Produces: the `mini` fixture's `Orders` diagram (key `"orders-diagram"`) now has a named group `"Sales"` with members `Order` + `Customer`, and an implicit (`""`) group holding `PaymentGateway`. All three nodes remain diagram members (so `size_map` / scene tests keeping their counts stay green). Later tasks rely on `"Sales"` being a resolvable group name and on all three nodes staying present.

**Why this shape:** The parser (`crates/waml/src/grammar.rs:parse_members_block`) routes flat bullets under `## Members` into an implicit group (`name == ""`, inserted at `groups[0]`) and each `### Heading` into a named sibling group. Keeping `PaymentGateway` flat and wrapping `Order`+`Customer` under `### Sales` yields `groups == [implicit(""), Sales]` while all three stay members — this is what lets Task 2 test that the implicit group is skipped and Task 3 project the named group.

- [ ] **Step 1: Rewrite the `## Members` section of the fixture**

Edit `crates/waml-editor/tests/fixtures/mini/orders-diagram.md`. Replace the current `## Members` block:

```markdown
## Members
- [Order](./order.md)
- [Customer](./customer.md)
- [PaymentGateway](./payment-gateway.md)
```

with (leave the frontmatter, `# Orders` heading, and the whole `## Layout` section exactly as they are):

```markdown
## Members
- [PaymentGateway](./payment-gateway.md)

### Sales
- [Order](./order.md)
- [Customer](./customer.md)
```

- [ ] **Step 2: Write the fixture-guard test**

Append to the `tests` module in `crates/waml-editor/src/inspector.rs` (after the last test, before the closing `}` of `mod tests`):

```rust
    #[test]
    fn mini_fixture_exposes_a_named_group_and_keeps_all_members() {
        let model = mini();
        let diagram = model
            .diagrams
            .iter()
            .find(|d| d.key == "orders-diagram")
            .expect("mini has the orders-diagram");
        // The named "Sales" group holds Order + Customer.
        let sales = diagram
            .groups
            .iter()
            .find(|g| g.name == "Sales")
            .expect("Sales group present");
        assert_eq!(sales.members.len(), 2, "Sales holds Order + Customer");
        // The implicit ("") group is still present (holds PaymentGateway).
        assert!(
            diagram.groups.iter().any(|g| g.name.is_empty()),
            "implicit unnamed group present"
        );
        // All three classifiers remain diagram members: three nodes total.
        assert_eq!(model.nodes.len(), 3);
    }
```

- [ ] **Step 3: Run the new test to verify it passes**

Run: `cargo test -p waml-editor --lib mini_fixture_exposes_a_named_group -- --nocapture`
Expected: PASS (the fixture parses into the intended group shape).

- [ ] **Step 4: Run the full workspace suite to confirm nothing regressed**

Run: `cargo test --workspace`
Expected: PASS. In particular `size_map_covers_every_resolved_member_with_positive_sizes` (`crates/waml-editor/src/sizing.rs`) still sees 3 members, and the scene layout tests (`crates/waml-editor/src/scene.rs`) still resolve `order`/`customer`/`payment-gateway`.

- [ ] **Step 5: Run the rest of the gate**

Run: `pnpm -r test && pnpm lint && pnpm build`
Expected: PASS (no Rust involvement in these; they must stay green).

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/tests/fixtures/mini/orders-diagram.md crates/waml-editor/src/inspector.rs
git commit -m "test(inspector): add named group to mini fixture with guard"
```

---

### Task 2: Add `ElementKind::Group` and emit group picker rows

**Files:**
- Modify: `crates/waml-editor/src/inspector.rs` (import, `ElementKind` enum, `diagram_elements`)
- Test: `crates/waml-editor/src/inspector.rs` (`tests` module)

**Interfaces:**
- Consumes: `Diagram.groups: Vec<DiagramGroup>` (`waml::model::DiagramGroup { name: String, members: Vec<String>, children: Vec<DiagramGroup> }`), looked up via `model.diagrams.iter().find(|d| d.key == diagram_key)`.
- Produces: `ElementKind::Group` variant; `diagram_elements` now emits one `ElementRow { key: <group name>, label: <group name>, kind: ElementKind::Group }` per named group, depth-first (parent then children), flat (no indent), inserted **after** the diagram row and **before** the first node row. The implicit (`""`) group is skipped. `diagram_elements`' signature is unchanged. Task 3 relies on `ElementKind::Group` and on the row `key`/`label` being the group name.

**Dead_code note:** `ElementKind::Group` is constructed in non-test `diagram_elements`, so it is live even though `apply_pick`'s `matches!(row.kind, ElementKind::Node)` guard still makes selecting it a no-op until Task 3. `build_select_items`' `match row.kind` has a `_ =>` fallback arm, so adding the variant does not break exhaustiveness.

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `crates/waml-editor/src/inspector.rs`:

```rust
    #[test]
    fn picker_lists_named_groups_after_diagram_before_nodes() {
        let model = mini();
        // Pass the REAL diagram key so groups resolve off the model.
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));

        // Row 0 = placeholder, row 1 = diagram, row 2 = first (only) named group.
        assert_eq!(rows[1].kind, ElementKind::Diagram);
        assert_eq!(rows[2].kind, ElementKind::Group);
        assert_eq!(rows[2].key, "Sales");
        assert_eq!(rows[2].label, "Sales");

        // Groups precede nodes.
        let first_group = rows
            .iter()
            .position(|r| r.kind == ElementKind::Group)
            .expect("a group row");
        let first_node = rows
            .iter()
            .position(|r| r.kind == ElementKind::Node)
            .expect("a node row");
        assert!(first_group < first_node, "group rows come before node rows");

        // Exactly one named group; the implicit "" group is skipped.
        let group_rows: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == ElementKind::Group)
            .collect();
        assert_eq!(group_rows.len(), 1);
        assert!(
            group_rows.iter().all(|r| !r.key.is_empty()),
            "the implicit unnamed group must be skipped"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor --lib picker_lists_named_groups -- --nocapture`
Expected: FAIL to COMPILE — `no variant named `Group` found for enum `ElementKind``.

- [ ] **Step 3: Add the `Group` variant and the `DiagramGroup` import**

In `crates/waml-editor/src/inspector.rs`, change the import at the top:

```rust
use waml::model::{ElementType, Model, RelationshipKind};
```

to:

```rust
use waml::model::{DiagramGroup, ElementType, Model, RelationshipKind};
```

Then add `Group` to `ElementKind`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    /// Index-0 sentinel shown when nothing is selected.
    Placeholder,
    Diagram,
    Group,
    Node,
    Edge,
}
```

- [ ] **Step 4: Emit group rows in `diagram_elements`**

In `crates/waml-editor/src/inspector.rs`, add this free function just above `diagram_elements`:

```rust
/// Depth-first (parent, then children) flatten of a group tree into flat picker
/// rows. The implicit top-level group (`name == ""`) is skipped; every named
/// group emits one row keyed/labelled by its name, no indent.
fn push_group_rows(groups: &[DiagramGroup], rows: &mut Vec<ElementRow>) {
    for g in groups {
        if !g.name.is_empty() {
            rows.push(ElementRow {
                key: g.name.clone(),
                label: g.name.clone(),
                kind: ElementKind::Group,
            });
        }
        push_group_rows(&g.children, rows);
    }
}
```

Then, inside `diagram_elements`, immediately AFTER the diagram row is pushed (the `rows.push(ElementRow { ... kind: ElementKind::Diagram })` block) and BEFORE the `for nk in node_keys {` loop, insert:

```rust
    // Group rows, flat and depth-first, after the diagram and before the nodes.
    if let Some(diagram) = model.diagrams.iter().find(|d| d.key == diagram_key) {
        push_group_rows(&diagram.groups, &mut rows);
    }
```

- [ ] **Step 5: Run the new test to verify it passes**

Run: `cargo test -p waml-editor --lib picker_lists_named_groups -- --nocapture`
Expected: PASS.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS. The existing `diagram_elements` tests that pass the fake key `"d1"` still emit no group rows (`find` returns `None`), so `picker_rows_lead_with_placeholder_then_diagram`, `picker_rows_list_every_node`, and `picker_nests_edge_after_its_source_node` are unaffected.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/inspector.rs
git commit -m "feat(inspector): list diagram groups as picker rows"
```

---

### Task 3: Select groups & edges — `Subject` variants, `members`, projection, and panel wiring

**Files:**
- Modify: `crates/waml-editor/src/inspector.rs` (`Subject`, `InspectorView`, `subject_to_index`, `build_view`)
- Modify: `crates/waml-editor/src/inspector_panel.rs` (`build_select_items`, `apply_pick`, `subject_key`, `draw_walk`)
- Test: `crates/waml-editor/src/inspector.rs` (`tests` module)

**Interfaces:**
- Consumes: `ElementKind::Group` (Task 2); `Diagram.groups`; `Model::edges` (`Edge { source: String, target: String, kind: RelationshipKind, .. }`, with `edge.kind.as_str()`); `Icon::SquareDashedTopSolid`, `Icon::Spline` (`crate::icons`).
- Produces:
  - `Subject::Group(String)` (group name) and `Subject::Edge(String)` (synthetic `"src->tgt"` key).
  - `InspectorView.members: Vec<String>` (group member display labels; empty for every other subject).
  - `build_view` projects Group (`title = name`, `kind_label = "Group"`, `members` = direct member titles, everything else empty/None) and Edge (`title = "<srcTitle> \u{2192} <tgtTitle>"`, `kind_label = edge.kind.as_str()`, everything else empty/None).
  - `subject_to_index` resolves Group and Edge rows by key.
  - Panel: `apply_pick` maps `Node→Classifier`, `Group→Group`, `Edge→Edge`, `Diagram`/`Placeholder→None`; `build_select_items` renders Group (enabled, `Icon::SquareDashedTopSolid`) and Edge (enabled, `Icon::Spline`) rows; `subject_key` returns the inner key for all three keyed variants; `draw_walk` paints a MEMBERS compartment.

**Dead_code note (why this is one unit):** `Subject::Group`/`Subject::Edge` get their only non-test constructor from `apply_pick`; `InspectorView.members` gets its only non-test reader from the MEMBERS draw. `subject_key` in `inspector_panel.rs` is the only **exhaustive** `match &self.subject` in the codebase (`context_items` ignores its `subject` arg), so adding the variants without updating it is a compile error. Therefore the pure changes and the panel changes must land together.

- [ ] **Step 1: Write the failing pure tests**

Append to the `tests` module in `crates/waml-editor/src/inspector.rs`:

```rust
    #[test]
    fn group_projects_name_kind_and_members() {
        let model = mini();
        let view = build_view(&model, &Subject::Group("Sales".into())).unwrap();
        assert_eq!(view.title, "Sales");
        assert_eq!(view.kind_label, "Group");
        // Members are the group's direct members, mapped to node titles.
        assert_eq!(view.members, vec!["Order".to_string(), "Customer".to_string()]);
        assert!(view.attributes.is_empty());
        assert!(view.associations.is_empty());
        assert!(view.description.is_none());
    }

    #[test]
    fn unknown_group_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Group("Nope".into())).is_none());
    }

    #[test]
    fn edge_projects_endpoint_titles_and_kind() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let id = format!("{order}->{customer}");
        let view = build_view(&model, &Subject::Edge(id)).unwrap();
        // Title carries both endpoint titles.
        assert!(view.title.contains("Order"), "title has source: {}", view.title);
        assert!(
            view.title.contains("Customer"),
            "title has target: {}",
            view.title
        );
        // Kind is the relationship kind string.
        assert_eq!(view.kind_label, "associates");
        assert!(view.members.is_empty());
    }

    #[test]
    fn unknown_edge_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Edge("a->b".into())).is_none());
    }

    #[test]
    fn classifier_has_empty_members() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(view.members.is_empty());
    }

    #[test]
    fn subject_to_index_resolves_group_row() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        let idx = subject_to_index(&rows, &Subject::Group("Sales".into()));
        assert_eq!(rows[idx].kind, ElementKind::Group);
        assert_eq!(rows[idx].key, "Sales");
    }

    #[test]
    fn subject_to_index_resolves_edge_row() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        let edge_key = format!("{}->{}", key_for(&model, "Order"), key_for(&model, "Customer"));
        let idx = subject_to_index(&rows, &Subject::Edge(edge_key.clone()));
        assert_eq!(rows[idx].kind, ElementKind::Edge);
        assert_eq!(rows[idx].key, edge_key);
    }

    #[test]
    fn subject_to_index_unknown_group_and_edge_fall_back_to_placeholder() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        assert_eq!(subject_to_index(&rows, &Subject::Group("Nope".into())), 0);
        assert_eq!(subject_to_index(&rows, &Subject::Edge("x->y".into())), 0);
    }
```

- [ ] **Step 2: Run the pure tests to verify they fail**

Run: `cargo test -p waml-editor --lib group_projects_name_kind_and_members -- --nocapture`
Expected: FAIL to COMPILE — `no variant named `Group` found for enum `Subject`` (and `members` field missing). The whole crate will not build until Steps 3–8 land; that is expected — this task commits once, at the end.

- [ ] **Step 3: Add the `Subject` variants**

In `crates/waml-editor/src/inspector.rs`, extend `Subject`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Subject {
    #[default]
    None,
    Classifier(String),
    /// Group name (diagram-scoped; resolved by name, first match wins).
    Group(String),
    /// Synthetic `"src->tgt"` id (the Edge picker row's key).
    Edge(String),
}
```

- [ ] **Step 4: Add the `members` field to `InspectorView`**

In `crates/waml-editor/src/inspector.rs`, add `members` to the struct (between `attributes` and `associations`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorView {
    pub title: String,
    pub kind_label: String,
    pub abstract_flag: bool,
    pub stereotypes: Vec<String>,
    pub description: Option<String>,
    pub attributes: Vec<AttrRow>,
    /// Group member display labels; empty for every non-group subject.
    pub members: Vec<String>,
    pub associations: Vec<AssocRow>,
}
```

- [ ] **Step 5: Rewrite `subject_to_index` to resolve all keyed variants**

In `crates/waml-editor/src/inspector.rs`, replace the whole `subject_to_index` function:

```rust
/// The picker index for `subject`: 0 (placeholder) for `None` or a key with no
/// matching row of the right kind, else the row whose kind+key matches.
pub fn subject_to_index(rows: &[ElementRow], subject: &Subject) -> usize {
    let (kind, key) = match subject {
        Subject::Classifier(k) => (ElementKind::Node, k),
        Subject::Group(k) => (ElementKind::Group, k),
        Subject::Edge(k) => (ElementKind::Edge, k),
        Subject::None => return 0,
    };
    rows.iter()
        .position(|r| r.kind == kind && &r.key == key)
        .unwrap_or(0)
}
```

- [ ] **Step 6: Refactor `build_view` into per-variant builders**

In `crates/waml-editor/src/inspector.rs`, replace the entire `build_view` function (from `pub fn build_view` through its closing `}`) with a dispatcher plus three helpers. First, a shared title helper:

```rust
/// A node's display title, falling back to its key.
fn node_title(model: &Model, key: &str) -> String {
    model
        .nodes
        .iter()
        .find(|n| n.key == key)
        .and_then(|n| n.concept.title.clone())
        .unwrap_or_else(|| key.to_string())
}

/// Project `subject` against `model`. Returns `None` for `Subject::None` and for
/// any key that resolves to nothing (all render the empty state).
pub fn build_view(model: &Model, subject: &Subject) -> Option<InspectorView> {
    match subject {
        Subject::None => None,
        Subject::Classifier(key) => build_classifier_view(model, key),
        Subject::Group(name) => build_group_view(model, name),
        Subject::Edge(id) => build_edge_view(model, id),
    }
}

fn build_classifier_view(model: &Model, key: &str) -> Option<InspectorView> {
    let node = model.nodes.iter().find(|n| n.key == key)?;

    let attributes = node
        .attributes
        .iter()
        .map(|a| AttrRow {
            name: a.name.clone(),
            ty: a.ty.name.clone(),
            multiplicity: a.multiplicity.as_str().to_string(),
            visibility: a
                .visibility
                .map(|v| v.marker().to_string())
                .unwrap_or_default(),
        })
        .collect();

    let mut associations = Vec::new();
    for edge in &model.edges {
        // uml.Note anchor, not a real relationship (mirrors the web skip).
        if edge.kind == RelationshipKind::Annotates {
            continue;
        }
        let outgoing = edge.source == key;
        let incoming = edge.target == key;
        if !outgoing && !incoming {
            continue;
        }
        let dir = if edge.bidirectional
            || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))
        {
            AssocDir::Bi
        } else if outgoing {
            AssocDir::Out
        } else {
            AssocDir::In
        };
        // Role + multiplicity read from the FAR end.
        let far_end = if outgoing {
            &edge.to_end
        } else {
            &edge.from_end
        };
        let far_key = if outgoing { &edge.target } else { &edge.source };
        let role = far_end.role.clone().unwrap_or_default();
        // Hide a bare "1" like the attribute rows do.
        let multiplicity = match &far_end.multiplicity {
            Some(m) if m.as_str() != "1" => m.as_str().to_string(),
            _ => String::new(),
        };
        associations.push(AssocRow {
            kind: edge.kind.as_str().to_string(),
            dir,
            other_label: node_title(model, far_key),
            role,
            multiplicity,
        });
    }

    Some(InspectorView {
        title: node
            .concept
            .title
            .clone()
            .unwrap_or_else(|| node.key.clone()),
        kind_label: kind_label(&node.ty),
        abstract_flag: node.abstract_,
        stereotypes: node.stereotypes.clone(),
        description: node.concept.description.clone(),
        attributes,
        members: Vec::new(),
        associations,
    })
}

fn build_group_view(model: &Model, name: &str) -> Option<InspectorView> {
    fn find<'a>(groups: &'a [DiagramGroup], name: &str) -> Option<&'a DiagramGroup> {
        for g in groups {
            if g.name == name {
                return Some(g);
            }
            if let Some(found) = find(&g.children, name) {
                return Some(found);
            }
        }
        None
    }
    // First match wins across every diagram's group tree (see Global Constraints).
    let group = model.diagrams.iter().find_map(|d| find(&d.groups, name))?;
    let members = group
        .members
        .iter()
        .map(|k| node_title(model, k))
        .collect();
    Some(InspectorView {
        title: name.to_string(),
        kind_label: "Group".to_string(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members,
        associations: Vec::new(),
    })
}

fn build_edge_view(model: &Model, id: &str) -> Option<InspectorView> {
    let (src, tgt) = id.split_once("->")?;
    let edge = model
        .edges
        .iter()
        .find(|e| e.source == src && e.target == tgt)?;
    Some(InspectorView {
        title: format!("{} \u{2192} {}", node_title(model, src), node_title(model, tgt)),
        kind_label: edge.kind.as_str().to_string(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members: Vec::new(),
        associations: Vec::new(),
    })
}
```

Note: the old `build_view` defined a `node_label` closure and used `&edge.source == key` (reference comparison). The rewrite uses the shared `node_title` free function and `edge.source == key` (`String == &str` via `PartialEq`), which is why the `far_key` binding is `&edge.target` / `&edge.source` and passed to `node_title(model, far_key)`.

- [ ] **Step 7: Update the panel's `subject_key` (required to keep the crate compiling)**

In `crates/waml-editor/src/inspector_panel.rs`, replace `subject_key`:

```rust
    fn subject_key(&self) -> Option<String> {
        match &self.subject {
            Subject::Classifier(key) | Subject::Group(key) | Subject::Edge(key) => Some(key.clone()),
            Subject::None => None,
        }
    }
```

- [ ] **Step 8: Wire `apply_pick` to construct the new variants (non-test constructor)**

In `crates/waml-editor/src/inspector_panel.rs`, replace the body of `apply_pick` (drop the `Node`-only guard):

```rust
    pub fn apply_pick(&mut self, cx: &mut Cx, model: &Model, id: LiveId) -> Option<Subject> {
        let idx = self
            .picker_ids
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, x)| *x)?;
        let row = self.elements.get(idx)?;
        let subject = match row.kind {
            ElementKind::Node => Subject::Classifier(row.key.clone()),
            ElementKind::Group => Subject::Group(row.key.clone()),
            ElementKind::Edge => Subject::Edge(row.key.clone()),
            ElementKind::Diagram | ElementKind::Placeholder => return None,
        };
        self.set_subject(cx, model, subject.clone());
        Some(subject)
    }
```

- [ ] **Step 9: Enable Group & Edge rows in `build_select_items`**

In `crates/waml-editor/src/inspector_panel.rs`, in the `match row.kind` inside `build_select_items`, add a `Group` arm and flip the `Edge` arm's `enabled` to `true`. Replace the existing `ElementKind::Edge` arm and add the `ElementKind::Group` arm so the block reads:

```rust
                ElementKind::Group => (
                    // Dashed box reads as a group frame — distinct from the
                    // diagram's solid `Frame` and any node's catalog icon.
                    SelectLead::Icon(Icon::SquareDashedTopSolid),
                    row.label.clone(),
                    true,
                ),
                ElementKind::Edge => (
                    SelectLead::Icon(Icon::Spline),
                    edge_target(&row.label).to_string(),
                    true,
                ),
                // The root diagram row leads with the `Frame` glyph -- distinct
                // from any node's catalog icon, marking it as the container.
                ElementKind::Diagram => (SelectLead::Icon(Icon::Frame), row.label.clone(), false),
                _ => (SelectLead::None, row.label.clone(), false),
```

(The `ElementKind::Node` arm above it is unchanged.)

- [ ] **Step 10: Add the MEMBERS compartment to `draw_walk` (non-test reader for `members`)**

In `crates/waml-editor/src/inspector_panel.rs`, in `draw_walk`, insert this block AFTER the `ATTRIBUTES` block (the one ending `y += GAP;` just before the `// Relationships:` comment) and BEFORE the `if !view.associations.is_empty() {` block:

```rust
        // Members: a group's direct members, one dim row each. Mirrors the
        // ATTRIBUTES compartment; only groups populate `view.members`.
        if !view.members.is_empty() {
            self.draw_dim.draw_abs(cx, dvec2(x, y), "MEMBERS");
            y += ROW_H;
            for m in &view.members {
                self.draw_label.draw_abs(cx, dvec2(x, y), m);
                y += ROW_H;
            }
            y += GAP;
        }
```

- [ ] **Step 11: Run the pure tests to verify they pass**

Run: `cargo test -p waml-editor --lib -- group_projects unknown_group edge_projects unknown_edge classifier_has_empty_members subject_to_index_resolves_group subject_to_index_resolves_edge subject_to_index_unknown_group`
Expected: PASS (all Task 3 tests green).

- [ ] **Step 12: Run the full gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: PASS. Confirm no clippy dead_code warnings: `Subject::Group`/`Subject::Edge` are constructed in `apply_pick`, matched in `build_view`/`subject_to_index`/`subject_key`; `InspectorView.members` is written in `build_view` and read in `draw_walk`.

- [ ] **Step 13: Commit**

```bash
git add crates/waml-editor/src/inspector.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "feat(inspector): make groups and edges selectable with MEMBERS compartment"
```

---

## Self-Review

**Spec coverage (checked against `docs/superpowers/specs/2026-07-24-inspector-groups-edges-selectable-design.md`):**
- §1 `Subject::Group` + `Subject::Edge` — Task 3 Step 3.
- §2 `ElementKind::Group` + group rows flat after diagram, before nodes, depth-first, skip `""` — Task 2 Steps 3–4.
- §3 `InspectorView.members` — Task 3 Step 4.
- §4 `build_view` projects Classifier/Group/Edge — Task 3 Step 6.
- §5 `subject_to_index` resolves all variants — Task 3 Step 5.
- §6 panel `build_select_items` (Group enabled + Edge enabled), `apply_pick` (drop Node guard, map all kinds), `subject_key` (inner key for Group/Edge) — Task 3 Steps 7–9.
- §7 MEMBERS compartment in `draw_walk` after ATTRIBUTES, before RELATIONSHIPS, gated on `!members.is_empty()` — Task 3 Step 10.
- Tests section (group rows / `""` skip / group projection / edge projection / index resolution) — Task 2 Step 1, Task 3 Step 1. Fixture-with-group need — Task 1.
- Non-goals (no web, no new icon, no `card/mod.rs`) — honored; both icons already exist.

**Icon verification:** `Icon::SquareDashedTopSolid` (`crates/waml-editor/src/icons.rs:3547`) and `Icon::Spline` (`:3485`) both exist — no substitution needed, no new catalog glyph. Spec's assumption confirmed.

**Placeholder scan:** No TBD/TODO; every code step shows complete code; every command has an expected result.

**Type consistency:** `ElementKind::Group` (Task 2) is the same variant matched in Task 3's `apply_pick`/`subject_to_index`. `Subject::Group`/`Subject::Edge`, `InspectorView.members`, `node_title`, `push_group_rows`, `build_classifier_view`/`build_group_view`/`build_edge_view` names are consistent across tasks. Diagram key `"orders-diagram"` is used verbatim in every group/edge test; existing tests keep their fake `"d1"` key deliberately (no group resolution).

## Execution Handoff

Not applicable in this context — this plan is written to be consumed by the `implement-plan` workflow (H3 `### Task N` headings, one committable green unit each).
