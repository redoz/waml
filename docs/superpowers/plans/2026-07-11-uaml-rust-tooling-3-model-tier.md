# UAML Rust Tooling — Plan 3: Model Tier (bundle → resolved graph)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `build_model`, which walks a whole bundle of parsed `Document`s and produces the resolved `Model` graph: classifier nodes (with attribute type-refs resolved), relationship edges (with reciprocal `associates` collapsed to one bidirectional edge), and diagram views (members + render hints). Prove it against a committed golden fixture — the `okf-format.md` orders-domain example.

**Architecture:** `build_model(bundle: &[(String, String)]) -> Model` parses each doc with `parse_document` (Plan 2), partitions classifier vs `Diagram` docs, builds a slug→node key set (key = filename slug), then runs node / edge / diagram passes mirroring `packages/okf/src/parse.ts` `parseBundle` — minus all legacy and OWOX paths.

**Tech Stack:** Rust 2021. Builds on Plans 1–2 (`model`, `syntax`, `parse`, `grammar`, `serialize`).

## Global Constraints

- All Global Constraints from Plans 1–2 apply.
- **Key = filename slug.** No `owox.key`, no `owox.position`. Positions live only on diagram `Member`s.
- **Scope for the first cut:** classifier docs (`uml.Class/Interface/Enum/DataType/Package` and `Unknown`) and `Diagram` docs. **`uml.Note`, `annotates` anchors, and `## Notes` sugar are DEFERRED.** A note's `## Body` still round-trips (Plan 2), but `Node.annotates` is left empty and no note desugaring/collapse happens. `fmt` (Plan 5) refuses to rewrite any file the validator (Plan 4) flags, so deferred/unsupported constructs are never silently lost.
- **Edge merge rules** (from `parse.ts:202-229`): reciprocal `associates` (declared from both ends) collapse to one edge with `bidirectional = true` and both ends navigable; the first declaration wins ends + name. Other kinds dedupe by `(kind, from, to)`. A target that doesn't resolve, or equals the source, is skipped (the validator reports unresolved targets separately).
- Attribute type-refs that don't resolve **degrade silently** to a bare token (ref dropped) — matching `grammar.ts:35-40`. Not an error.

---

### Task 1: `build_model` — classifier nodes

**Files:**
- Modify: `crates/uaml/src/parse.rs`

**Interfaces:**
- Consumes: `crate::parse::parse_document`, `crate::syntax::Section`, `crate::model::*`.
- Produces: `pub fn build_model(bundle: &[(String, String)]) -> Model` (nodes populated; edges/diagrams empty for now), plus private `doc_slug`, `build_node`.

- [ ] **Step 1: Write the failing tests**

Add a new test module at the end of `crates/uaml/src/parse.rs`:
```rust
#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::model::{ClassifierType, UmlMetaclass};

    fn bundle() -> Vec<(String, String)> {
        vec![
            ("shop/order.md".into(),
             "---\ntype: uml.Class\nstereotype: [aggregateRoot, entity]\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- status: [OrderStatus](./order-status.md) [0..1]\n- ghost: [Missing](./missing.md)\n".into()),
            ("shop/order-status.md".into(),
             "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n".into()),
        ]
    }

    #[test]
    fn builds_classifier_nodes() {
        let m = build_model(&bundle());
        assert_eq!(m.nodes.len(), 2);
        let order = m.node("order").unwrap();
        assert_eq!(order.title, "Order");
        assert_eq!(order.ty, ClassifierType::Uml(UmlMetaclass::Class));
        assert_eq!(order.stereotypes, vec!["aggregateRoot", "entity"]);
        assert_eq!(order.attributes.len(), 3);
    }

    #[test]
    fn resolves_and_degrades_attribute_refs() {
        let m = build_model(&bundle());
        let order = m.node("order").unwrap();
        // resolvable link keeps its ref
        assert_eq!(order.attributes[1].ty.ref_.as_deref(), Some("order-status"));
        // unresolvable link degrades to a bare token (ref dropped), name preserved
        assert_eq!(order.attributes[2].ty.name, "Missing");
        assert_eq!(order.attributes[2].ty.ref_, None);
    }

    #[test]
    fn collects_enum_values() {
        let m = build_model(&bundle());
        assert_eq!(m.node("order-status").unwrap().values, vec!["DRAFT", "PLACED"]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml model_tests`
Expected: FAIL — `build_model` not found.

- [ ] **Step 3: Implement `build_model` (nodes pass)**

Add to `crates/uaml/src/parse.rs` (above the test modules). Add the imports shown to the existing `use` lines at the top:
```rust
use std::collections::{HashMap, HashSet};

use crate::model::{
    Attribute, ClassifierType, Diagram, Edge, Member, Model, Node, RenderHints,
};
use crate::syntax::Document;

/// A classifier's filename slug (the node key): last path segment, `.md` stripped.
fn doc_slug(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

struct ParsedDoc {
    slug: String,
    ty: ClassifierType,
    doc: Document,
}

fn parse_bundle(bundle: &[(String, String)]) -> Vec<ParsedDoc> {
    bundle
        .iter()
        .map(|(path, text)| {
            let doc = parse_document(text);
            let ty = ClassifierType::parse(doc.frontmatter.get_str("type").unwrap_or("uml.Class"));
            ParsedDoc { slug: doc_slug(path), ty, doc }
        })
        .collect()
}

fn resolve_attr(attr: &Attribute, keyset: &HashSet<&str>) -> Attribute {
    let mut a = attr.clone();
    if let Some(slug) = &a.ty.ref_ {
        if !keyset.contains(slug.as_str()) {
            a.ty.ref_ = None; // degrade to a bare token
        }
    }
    a
}

fn build_node(p: &ParsedDoc, keyset: &HashSet<&str>) -> Node {
    let fm = &p.doc.frontmatter;
    let title = fm.get_str("title").map(String::from).unwrap_or_else(|| {
        if p.doc.title.is_empty() { "Untitled".to_string() } else { p.doc.title.clone() }
    });
    let mut attributes = Vec::new();
    let mut values = Vec::new();
    let mut body = None;
    for s in &p.doc.sections {
        match s {
            Section::Attributes(a) => attributes = a.iter().map(|x| resolve_attr(x, keyset)).collect(),
            Section::Values(v) => values = v.clone(),
            Section::Body(b) => body = Some(b.clone()),
            _ => {}
        }
    }
    Node {
        key: p.slug.clone(),
        ty: p.ty.clone(),
        title,
        stereotypes: fm.get_string_list("stereotype"),
        abstract_: fm.get_bool("abstract") == Some(true),
        description: fm.get_str("description").map(String::from),
        attributes,
        values,
        body,
        annotates: Vec::new(), // deferred: uml.Note anchors
    }
}

pub fn build_model(bundle: &[(String, String)]) -> Model {
    let parsed = parse_bundle(bundle);
    let classifiers: Vec<&ParsedDoc> =
        parsed.iter().filter(|p| p.ty != ClassifierType::Diagram).collect();
    let keyset: HashSet<&str> = classifiers.iter().map(|p| p.slug.as_str()).collect();

    let nodes = classifiers.iter().map(|p| build_node(p, &keyset)).collect();
    let edges: Vec<Edge> = build_edges(&classifiers, &keyset);
    let diagrams: Vec<Diagram> = build_diagrams(&parsed, &keyset);

    Model { nodes, edges, diagrams }
}

// Filled in by Tasks 2 and 3; stubs keep the crate compiling now.
fn build_edges(_classifiers: &[&ParsedDoc], _keyset: &HashSet<&str>) -> Vec<Edge> {
    Vec::new()
}
fn build_diagrams(_parsed: &[ParsedDoc], _keyset: &HashSet<&str>) -> Vec<Diagram> {
    Vec::new()
}
```

Note: the unused imports `HashMap`, `Member`, `RenderHints` are consumed in Tasks 2–3; add `#[allow(unused_imports)]` above the `use` block if the compiler warns, and remove it in Task 3.

- [ ] **Step 4: Run to verify passing**

Run: `cargo test -p uaml model_tests`
Expected: PASS — `3 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): build_model node pass (classifiers, attributes, values)"
```

---

### Task 2: `build_model` — relationship edges

**Files:**
- Modify: `crates/uaml/src/parse.rs`

**Interfaces:**
- Produces: real `build_edges(&[&ParsedDoc], &HashSet<&str>) -> Vec<Edge>`.

Reciprocal `associates` collapse to one bidirectional edge (first declaration wins ends + name); other kinds dedupe by `(kind, from, to)`; unresolved or self targets are skipped.

- [ ] **Step 1: Write the failing tests**

Add to the `model_tests` module in `crates/uaml/src/parse.rs`:
```rust
    fn rel_bundle() -> Vec<(String, String)> {
        vec![
            ("a/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n- associates [Customer](./customer.md): 1 to 1\n".into()),
            ("a/order-line.md".into(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".into()),
            ("a/customer.md".into(),
             "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Relationships\n- associates [Order](./order.md): 1 to 1\n".into()),
        ]
    }

    #[test]
    fn builds_composition_edge() {
        let m = build_model(&rel_bundle());
        let comp = m.edges.iter().find(|e| e.kind == crate::model::RelationshipKind::Composes).unwrap();
        assert_eq!(comp.source, "order");
        assert_eq!(comp.target, "order-line");
        assert_eq!(comp.to_end.role.as_deref(), Some("lines"));
        assert!(!comp.bidirectional);
    }

    #[test]
    fn reciprocal_associates_collapse_to_one_bidirectional_edge() {
        let m = build_model(&rel_bundle());
        let assocs: Vec<_> = m.edges.iter().filter(|e| e.kind == crate::model::RelationshipKind::Associates).collect();
        assert_eq!(assocs.len(), 1, "reciprocal associates must collapse to one edge");
        assert!(assocs[0].bidirectional);
        assert_eq!(assocs[0].from_end.navigable, Some(true));
        assert_eq!(assocs[0].to_end.navigable, Some(true));
    }

    #[test]
    fn skips_unresolved_targets() {
        let b = vec![("x/a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let m = build_model(&b);
        assert!(m.edges.is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml model_tests`
Expected: FAIL — reciprocal/composition assertions fail (stub returns no edges).

- [ ] **Step 3: Implement `build_edges`**

Replace the `build_edges` stub in `crates/uaml/src/parse.rs` with:
```rust
use crate::model::{AssocName, RelationshipKind};
use crate::syntax::ParsedName;

fn build_edges(classifiers: &[&ParsedDoc], keyset: &HashSet<&str>) -> Vec<Edge> {
    let mut edges: Vec<Edge> = Vec::new();
    let mut assoc_pair: HashMap<(String, String), usize> = HashMap::new();
    let mut seen_other: HashSet<(String, String, String)> = HashSet::new();

    for p in classifiers {
        let from = &p.slug;
        for s in &p.doc.sections {
            let Section::Relationships(rels) = s else { continue };
            for r in rels {
                let to = &r.target_slug;
                if !keyset.contains(to.as_str()) || to == from {
                    continue;
                }
                let name = match &r.name {
                    None => None,
                    Some(ParsedName::Label(l)) => Some(AssocName::Label(l.clone())),
                    Some(ParsedName::Ref { slug, .. }) => {
                        keyset.contains(slug.as_str()).then(|| AssocName::Assoc(slug.clone()))
                    }
                };

                if r.kind == RelationshipKind::Associates {
                    let mut pair = [from.clone(), to.clone()];
                    pair.sort();
                    let key = (pair[0].clone(), pair[1].clone());
                    if let Some(&idx) = assoc_pair.get(&key) {
                        let e = &mut edges[idx];
                        e.bidirectional = true;
                        e.from_end.navigable = Some(true);
                        e.to_end.navigable = Some(true);
                        if e.name.is_none() && name.is_some() {
                            e.name = name;
                        }
                        continue;
                    }
                    let mut to_end = r.to_end.clone();
                    to_end.navigable = Some(true);
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: RelationshipKind::Associates,
                        name,
                        from_end: r.from_end.clone(),
                        to_end,
                        bidirectional: false,
                    });
                    assoc_pair.insert(key, edges.len() - 1);
                } else {
                    let dedup = (r.kind.as_str().to_string(), from.clone(), to.clone());
                    if !seen_other.insert(dedup) {
                        continue;
                    }
                    edges.push(Edge {
                        source: from.clone(),
                        target: to.clone(),
                        kind: r.kind,
                        name,
                        from_end: r.from_end.clone(),
                        to_end: r.to_end.clone(),
                        bidirectional: false,
                    });
                }
            }
        }
    }
    edges
}
```

- [ ] **Step 4: Run to verify passing**

Run: `cargo test -p uaml model_tests`
Expected: PASS — node + edge tests green.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): build_model edge pass with bidirectional associate merge"
```

---

### Task 3: `build_model` — diagrams

**Files:**
- Modify: `crates/uaml/src/parse.rs`

**Interfaces:**
- Produces: real `build_diagrams(&[ParsedDoc], &HashSet<&str>) -> Vec<Diagram>`.

- [ ] **Step 1: Write the failing tests**

Add to the `model_tests` module in `crates/uaml/src/parse.rs`:
```rust
    fn diagram_bundle() -> Vec<(String, String)> {
        vec![
            ("d/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("d/pricing.md".into(), "---\ntype: uml.Class\ntitle: Pricing\n---\n# Pricing\n".into()),
            ("d/orders-domain.md".into(),
             "---\ntype: Diagram\ntitle: Orders Domain\nprofile: uml-domain\n---\n# Orders Domain\n\n## Members\n- [Order](./order.md) at 40,80\n- [Pricing](./pricing.md)\n- [Ghost](./ghost.md)\n\n## Render hints\n- emphasize: order\n- collapse [Pricing](./pricing.md)\n".into()),
        ]
    }

    #[test]
    fn builds_diagram_with_members_and_hints() {
        let m = build_model(&diagram_bundle());
        assert_eq!(m.nodes.len(), 2, "diagram doc is not a node");
        assert_eq!(m.diagrams.len(), 1);
        let d = &m.diagrams[0];
        assert_eq!(d.title, "Orders Domain");
        assert_eq!(d.profile, "uml-domain");
        // resolvable members only; ghost is dropped
        assert_eq!(d.members.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), vec!["order", "pricing"]);
        assert_eq!(d.members[0].position, Some((40.0, 80.0)));
        assert_eq!(d.hints.emphasize, vec!["order"]);
        assert_eq!(d.hints.collapse, vec!["pricing"]);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml model_tests::builds_diagram_with_members_and_hints`
Expected: FAIL — stub returns no diagrams.

- [ ] **Step 3: Implement `build_diagrams`**

Replace the `build_diagrams` stub in `crates/uaml/src/parse.rs` with:
```rust
use crate::syntax::HintLine;

fn build_diagrams(parsed: &[ParsedDoc], keyset: &HashSet<&str>) -> Vec<Diagram> {
    let mut out = Vec::new();
    for p in parsed.iter().filter(|p| p.ty == ClassifierType::Diagram) {
        let fm = &p.doc.frontmatter;
        let title = fm.get_str("title").map(String::from).unwrap_or_else(|| "Untitled diagram".to_string());
        let profile = fm
            .get_str("profile")
            .filter(|s| !s.is_empty())
            .unwrap_or("uml-domain")
            .to_string();

        let mut members = Vec::new();
        let mut hints = RenderHints::default();
        for s in &p.doc.sections {
            match s {
                Section::Members(ms) => {
                    for mem in ms {
                        if keyset.contains(mem.slug.as_str()) {
                            members.push(Member { key: mem.slug.clone(), position: mem.position });
                        }
                    }
                }
                Section::RenderHints(hs) => {
                    for h in hs {
                        match h {
                            HintLine::Emphasize(list) => hints.emphasize = list.clone(),
                            HintLine::Collapse { slug, .. } => {
                                if keyset.contains(slug.as_str()) {
                                    hints.collapse.push(slug.clone());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        out.push(Diagram { key: p.slug.clone(), title, profile, members, hints });
    }
    out
}
```

If a `#[allow(unused_imports)]` was added in Task 1, remove it now — all of `HashMap`, `Member`, `RenderHints` are used.

- [ ] **Step 4: Run to verify passing**

Run: `cargo test -p uaml model_tests`
Expected: PASS — all model tests green.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/parse.rs
git commit -m "feat(uaml): build_model diagram pass (members + render hints)"
```

---

### Task 4: golden fixture — the orders-domain example

**Files:**
- Create: `crates/uaml/tests/fixtures/orders-domain.md`
- Create: `crates/uaml/tests/golden.rs`

**Interfaces:**
- Consumes: `uaml::parse::{split_bundle, build_model, parse_document}`, `uaml::serialize::serialize_document`.

The fixture is the copy-paste worked example from `packages/web/public/okf-format.md` (five classifiers + one diagram, as a single blob with `<!-- shop/*.md -->` markers). This is the same artifact the TS `guideExample.test.ts` guards, pinning both implementations to one spec example.

- [ ] **Step 1: Create the fixture**

`crates/uaml/tests/fixtures/orders-domain.md` (verbatim from the okf-format.md worked example):
```markdown
<!-- shop/order.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Order
description: "A customer's placed order."
---
# Order

## Attributes
- id: OrderId
- status: [OrderStatus](./order-status.md)

## Relationships
- composes [OrderLine](./order-line.md): 1 to 1..* lines
- associates [Customer](./customer.md): 1 to 1

<!-- shop/order-line.md -->
---
type: uml.Class
stereotype: entity
title: OrderLine
---
# OrderLine

## Attributes
- quantity: Int
- unitPrice: [Money](./money.md)

<!-- shop/customer.md -->
---
type: uml.Class
stereotype: [aggregateRoot, entity]
title: Customer
---
# Customer

## Attributes
- id: CustomerId
- name: String

<!-- shop/order-status.md -->
---
type: uml.Enum
title: OrderStatus
---
# OrderStatus

## Values
- DRAFT
- PLACED
- SHIPPED
- CANCELLED

<!-- shop/money.md -->
---
type: uml.DataType
stereotype: valueObject
title: Money
---
# Money

## Attributes
- amount: Decimal
- currency: CurrencyCode

<!-- shop/orders-domain.md -->
---
type: Diagram
title: Orders Domain
profile: uml-domain
---
# Orders Domain

## Members
- [Order](./order.md)
- [OrderLine](./order-line.md)
- [Customer](./customer.md)
- [OrderStatus](./order-status.md)
- [Money](./money.md)
```

- [ ] **Step 2: Write the failing test**

`crates/uaml/tests/golden.rs`:
```rust
use uaml::parse::{build_model, parse_document, split_bundle};
use uaml::serialize::serialize_document;

const FIXTURE: &str = include_str!("fixtures/orders-domain.md");

#[test]
fn orders_domain_builds_the_expected_model() {
    let bundle = split_bundle(FIXTURE);
    let m = build_model(&bundle);

    // Five classifiers, one diagram.
    assert_eq!(m.nodes.len(), 5);
    assert_eq!(m.diagrams.len(), 1);
    assert_eq!(m.diagrams[0].members.len(), 5);

    // Two edges: composes + associates.
    assert_eq!(m.edges.len(), 2);
    let kinds: Vec<_> = m.edges.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"composes"));
    assert!(kinds.contains(&"associates"));

    // The composition target resolves and carries the far role.
    let comp = m.edges.iter().find(|e| e.kind.as_str() == "composes").unwrap();
    assert_eq!(comp.source, "order");
    assert_eq!(comp.target, "order-line");
    assert_eq!(comp.to_end.role.as_deref(), Some("lines"));

    // The Money value-object's attribute types are bare tokens (no matching docs).
    let money = m.node("money").unwrap();
    assert_eq!(money.attributes[0].ty.name, "Decimal");
    assert_eq!(money.attributes[0].ty.ref_, None);
}

#[test]
fn every_doc_is_a_serialize_fixpoint() {
    for (_path, text) in split_bundle(FIXTURE) {
        let once = serialize_document(&parse_document(&text));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice, "serialize must be idempotent per document");
    }
}
```

- [ ] **Step 3: Run to verify failure, then confirm it passes**

Run: `cargo test -p uaml --test golden`
Expected: initially the fixture/paths resolve and the test compiles; it should PASS if Tasks 1–3 are correct. If `orders_domain_builds_the_expected_model` fails, the failure pinpoints a resolution bug to fix before continuing.

- [ ] **Step 4: Run the full suite**

Run: `cargo test -p uaml`
Expected: PASS — all unit tests plus the golden integration test.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/tests/fixtures/orders-domain.md crates/uaml/tests/golden.rs
git commit -m "test(uaml): golden orders-domain fixture pins build_model to the spec example"
```

---

## Self-Review

- **Spec coverage (this plan's slice):** `Model` tier / `ModelGraph` analogue built ✔ (Tasks 1–3); attribute ref resolution with silent degradation ✔ (Task 1); bidirectional `associates` merge + other-kind dedup ✔ (Task 2); diagrams with members + hints, positions on members ✔ (Task 3); golden fixture pinned to the same spec artifact as the TS guide test ✔ (Task 4). Validation and the CLI are Plans 4–5. Notes/annotates deferred (documented).
- **Placeholder scan:** none — the Task 1 `build_edges`/`build_diagrams` stubs are explicitly replaced with full implementations in Tasks 2–3, each with tests; no step ships a stub as final.
- **Type consistency:** `build_model`/`parse_document`/`split_bundle`/`serialize_document` signatures match Plan 2. `Node`/`Edge`/`Diagram`/`Member`/`RenderHints`/`AssocName`/`RelationshipKind`/`ClassifierType` fields and methods match Plan 1 exactly (incl. the `Edge.bidirectional` field added by the Plan 1 patch). `build_edges`/`build_diagrams` are declared as stubs in Task 1 with the exact signatures Tasks 2–3 implement.
