//! Legacy `waml::ops::Op` test suite, ported 1:1 onto [`Step`]/[`Batch`]/
//! [`crate::edit::apply`]. See the porting table in the retire-compat plan
//! for the mapping used to translate each `Op` literal.

use super::{Batch, EditError, Step};
use crate::layout::Direction;
use crate::model::{parse_ends, CardinalityVisibility, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::source::SourceBundle;
use crate::uml::selector::{RelBy, Selector};
use crate::uml::{self, DiagramDisplaySet, FieldEdit, NameSpec, RelationshipSelector};

type Bundle = Vec<(String, String)>;

fn apply(bundle: &[(String, String)], steps: Vec<Step>) -> Result<Bundle, EditError> {
    let source = SourceBundle::try_from_pairs(bundle.iter().cloned())
        .map_err(|error| EditError::at("bundle", error.to_string()))?;
    crate::edit::apply(&source, &Batch::new(steps)).map(|bundle| bundle.to_pairs())
}

fn projection(bundle: &Bundle) -> crate::uml::Projection {
    let source = SourceBundle::try_from_pairs(bundle.iter().cloned()).unwrap();
    crate::analysis::prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone()
}

fn layout_statement_count(bundle: &Bundle) -> usize {
    projection(bundle)
        .diagrams
        .first()
        .map_or(0, |diagram| diagram.layout.len())
}

fn attr_add(node: &str, name: &str, ty: &str) -> Step {
    Step::Uml(uml::Op::AttributeAdd {
        node: node.into(),
        name: name.into(),
        ty_token: ty.into(),
        multiplicity: None,
        visibility: None,
    })
}

fn rel_selector(selector: Selector) -> RelationshipSelector {
    RelationshipSelector::try_from(selector).unwrap()
}

#[test]
fn retitle_changes_index_content_without_changing_child_paths() {
    let before = vec![(
        "sales/order.md".into(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into(),
    )];
    let after = apply(
        &before,
        vec![Step::Okf(crate::okf::Op::IndexRetitle {
            directory: crate::okf::DirectoryAddress::parse("/sales").unwrap(),
            title: "Sales Domain".into(),
        })],
    )
    .unwrap();

    assert!(after
        .iter()
        .any(|(path, text)| path == "sales/index.md" && text.contains("# Sales Domain")));
    assert!(after.iter().any(|(path, _)| path == "sales/order.md"));
}

#[test]
fn attr_add_appends_a_bare_attribute() {
    let b = vec![(
        "shop/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n"
            .to_string(),
    )];
    let out = apply(&b, vec![attr_add("order", "total", "Money")]).unwrap();
    assert!(out[0].1.contains("- total: Money"));
    assert!(out[0].1.contains("- id: OrderId"), "existing attr kept");
}

#[test]
fn attr_add_links_a_known_slug() {
    let b = vec![
        (
            "a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "a/money.md".to_string(),
            "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string(),
        ),
    ];
    let out = apply(&b, vec![attr_add("order", "total", "money")]).unwrap();
    assert!(
        out[0].1.contains("- total: [Money](./money.md)"),
        "known slug links with target title"
    );
}

#[test]
fn attr_add_refuses_a_duplicate_name() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n"
            .to_string(),
    )];
    let err = apply(&b, vec![attr_add("order", "id", "X")]).unwrap_err();
    assert_eq!(err.index, 0);
    assert_eq!(err.op, "attr.add");
    assert!(err.reason.contains("already exists"));
}

#[test]
fn attr_add_on_missing_node_errors() {
    let b: Bundle = vec![];
    let err = apply(&b, vec![attr_add("ghost", "x", "Y")]).unwrap_err();
    assert!(err.reason.contains("no document 'ghost'"));
}

#[test]
fn apply_is_atomic_on_a_later_failure() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n"
            .to_string(),
    )];
    let steps = vec![
        attr_add("order", "total", "Money"),
        attr_add("order", "id", "X"),
    ]; // 2nd is a dup
    let err = apply(&b, steps).unwrap_err();
    assert_eq!(err.index, 1, "failing op index reported");
    assert!(
        !b[0].1.contains("total"),
        "input bundle untouched; caller writes nothing"
    );
}

#[test]
fn attr_set_changes_type_and_multiplicity() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n"
            .to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeSet {
            node: "order".into(),
            name: "id".into(),
            ty_token: Some("String".into()),
            multiplicity: FieldEdit::Set(Multiplicity::parse("0..1").unwrap()),
            visibility: Some(Visibility::Private),
            rename: None,
        })],
    )
    .unwrap();
    assert!(out[0].1.contains("- id: String {0..1}"));
    let model = projection(&out);
    let id = model
        .nodes
        .iter()
        .flat_map(|node| &node.attributes)
        .find(|a| a.name == "id")
        .expect("id attribute present");
    assert_eq!(id.visibility, Some(Visibility::Private));
}

#[test]
fn attr_set_omitting_multiplicity_preserves_authored_value() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {0..*}\n"
            .to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeSet {
            node: "order".into(),
            name: "id".into(),
            ty_token: Some("String".into()),
            multiplicity: FieldEdit::Unchanged,
            visibility: None,
            rename: None,
        })],
    )
    .unwrap();
    assert!(out[0].1.contains("- id: String {0..*}\n"));
}

#[test]
fn attr_set_explicitly_clears_authored_multiplicity() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n"
            .to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeSet {
            node: "order".into(),
            name: "id".into(),
            ty_token: Some("String".into()),
            multiplicity: FieldEdit::Clear,
            visibility: None,
            rename: None,
        })],
    )
    .unwrap();
    assert!(out[0].1.contains("- id: String\n"));
    assert!(!out[0].1.contains("{1}"));
}

#[test]
fn attr_set_renames_and_refuses_collision() {
    let b = vec![("a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n".to_string())];
    let ok = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeSet {
            node: "order".into(),
            name: "id".into(),
            ty_token: None,
            multiplicity: FieldEdit::Unchanged,
            visibility: None,
            rename: Some("orderId".into()),
        })],
    )
    .unwrap();
    assert!(ok[0].1.contains("- orderId: OrderId"));
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeSet {
            node: "order".into(),
            name: "id".into(),
            ty_token: None,
            multiplicity: FieldEdit::Unchanged,
            visibility: None,
            rename: Some("total".into()),
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("already exists"));
}

#[test]
fn attr_set_on_missing_attr_errors() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeSet {
            node: "order".into(),
            name: "ghost".into(),
            ty_token: Some("X".into()),
            multiplicity: FieldEdit::Unchanged,
            visibility: None,
            rename: None,
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("no attribute 'ghost'"));
}

#[test]
fn attr_rm_removes_and_refuses_missing() {
    let b = vec![("a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n".to_string())];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeRemove {
            node: "order".into(),
            name: "total".into(),
        })],
    )
    .unwrap();
    assert!(!out[0].1.contains("total"));
    assert!(out[0].1.contains("- id: OrderId"));
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::AttributeRemove {
            node: "order".into(),
            name: "ghost".into(),
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("no attribute 'ghost'"));
}

#[test]
fn value_add_appends_and_refuses_duplicate() {
    let b = vec![(
        "a/order-status.md".to_string(),
        "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n"
            .to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ValueAdd {
            node: "order-status".into(),
            literal: "PLACED".into(),
        })],
    )
    .unwrap();
    assert!(out[0].1.contains("- DRAFT"));
    assert!(out[0].1.contains("- PLACED"));
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::ValueAdd {
            node: "order-status".into(),
            literal: "DRAFT".into(),
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("already"));
}

#[test]
fn value_rm_removes_and_refuses_missing() {
    let b = vec![("a/order-status.md".to_string(),
        "---\ntype: uml.Enum\ntitle: OrderStatus\n---\n# OrderStatus\n\n## Values\n- DRAFT\n- PLACED\n".to_string())];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ValueRemove {
            node: "order-status".into(),
            literal: "DRAFT".into(),
        })],
    )
    .unwrap();
    assert!(!out[0].1.contains("DRAFT"));
    assert!(out[0].1.contains("- PLACED"));
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::ValueRemove {
            node: "order-status".into(),
            literal: "GONE".into(),
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("no value 'GONE'"));
}

#[test]
fn rel_add_composes_with_ends() {
    let b = vec![
        (
            "a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "a/order-line.md".to_string(),
            "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string(),
        ),
    ];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Composes,
            target: "order-line".into(),
            name: None,
            ends: parse_ends("1 to 1..* lines"),
        })],
    )
    .unwrap();
    assert!(out[0]
        .1
        .contains("- composes [OrderLine](./order-line.md): 1 to 1..* lines"));
}

#[test]
fn authored_links_are_relative_to_the_mutated_document() {
    let bundle = vec![
        (
            "shop/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "types/money.md".to_string(),
            "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string(),
        ),
    ];
    let with_attribute = apply(
        &bundle,
        vec![attr_add("shop/order", "total", "types/money")],
    )
    .unwrap();
    assert!(with_attribute[0]
        .1
        .contains("- total: [Money](../types/money.md)"));

    let with_relationship = apply(
        &with_attribute,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "shop/order".into(),
            kind: RelationshipKind::Depends,
            target: "types/money".into(),
            name: None,
            ends: None,
        })],
    )
    .unwrap();
    assert!(with_relationship[0]
        .1
        .contains("- depends [Money](../types/money.md)"));
}

#[test]
fn rel_add_enforces_ends_xor_verb() {
    let b = vec![
        (
            "a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "a/x.md".to_string(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n".to_string(),
        ),
    ];
    // composes requires ends
    let e1 = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Composes,
            target: "x".into(),
            name: None,
            ends: None,
        })],
    )
    .unwrap_err();
    assert!(e1.reason.contains("requires ends"));
    // depends forbids ends
    let e2 = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Depends,
            target: "x".into(),
            name: None,
            ends: parse_ends("1 to 1"),
        })],
    )
    .unwrap_err();
    assert!(e2.reason.contains("does not take ends"));
}

#[test]
fn rel_add_requires_claimed_target_and_refuses_duplicate() {
    let b = vec![
        (
            "a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "a/customer.md".to_string(),
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
        ),
    ];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Depends,
            target: "customer".into(),
            name: None,
            ends: None,
        })],
    )
    .unwrap();
    assert!(out[0].1.contains("- depends [Customer](./customer.md)"));
    let dup = apply(
        &out,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Depends,
            target: "customer".into(),
            name: None,
            ends: None,
        })],
    )
    .unwrap_err();
    assert!(dup.reason.contains("already exists"));
}

#[test]
fn rel_set_updates_ends_and_rel_rm_removes() {
    let b = vec![
        ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md): 1 to 1..* lines\n".to_string()),
        ("a/order-line.md".to_string(), "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string()),
    ];
    let sel = Selector::Rel {
        source: "order".into(),
        by: RelBy::Endpoint {
            kind: RelationshipKind::Composes,
            target: "order-line".into(),
        },
    };
    let set = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipSet {
            selector: rel_selector(sel.clone()),
            ends: parse_ends("1 to *"),
            name: None,
        })],
    )
    .unwrap();
    assert!(set[0].1.contains(": 1 to *"));
    let rm = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipRemove {
            selector: rel_selector(sel),
        })],
    )
    .unwrap();
    assert!(!rm[0].1.contains("composes"));
}

#[test]
fn rel_rm_on_missing_rel_errors() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let sel = Selector::Rel {
        source: "order".into(),
        by: RelBy::Named("nope".into()),
    };
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipRemove {
            selector: rel_selector(sel),
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("no relationship"));
    assert!(err.selector.is_some());
}

#[test]
fn rel_set_on_missing_rel_errors() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let sel = Selector::Rel {
        source: "order".into(),
        by: RelBy::Named("nope".into()),
    };
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipSet {
            selector: rel_selector(sel),
            ends: None,
            name: None,
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("no relationship"));
    assert!(err.selector.is_some());
}

#[test]
fn rel_matches_ref_named_selector() {
    let b = vec![
        (
            "a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "a/customer.md".to_string(),
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string(),
        ),
        (
            "a/order-line.md".to_string(),
            "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".to_string(),
        ),
    ];
    let added = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Depends,
            target: "order-line".into(),
            name: Some(NameSpec::Ref("customer".into())),
            ends: None,
        })],
    )
    .unwrap();
    let sel = Selector::Rel {
        source: "order".into(),
        by: RelBy::Named("Customer".into()),
    };
    let rm = apply(
        &added,
        vec![Step::Uml(uml::Op::RelationshipRemove {
            selector: rel_selector(sel),
        })],
    )
    .unwrap();
    assert!(
        !rm[0].1.contains("depends"),
        "Ref-named relationship must be reachable via RelBy::Named on its resolved title"
    );
}

fn dir(path: &str) -> crate::okf::DirectoryAddress {
    let path = path.trim_matches('/');
    crate::okf::DirectoryAddress::parse(if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    })
    .unwrap()
}

fn node_new(slug: &str, dir_path: &str, title: &str, stereotype: Vec<&str>) -> Step {
    Step::Uml(uml::Op::ClassifierNew {
        slug: slug.into(),
        directory: dir(dir_path),
        ty: crate::model::ElementType::parse("uml.Class"),
        title: title.into(),
        stereotype: stereotype.into_iter().map(String::from).collect(),
        description: None,
        abstract_: false,
    })
}

#[test]
fn node_new_writes_frontmatter_and_title_and_refuses_dup() {
    let b: Bundle = vec![];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierNew {
            slug: "order".into(),
            directory: dir(""),
            ty: crate::model::ElementType::parse("uml.Class"),
            title: "Order".into(),
            stereotype: vec!["entity".into()],
            description: Some("An order.".into()),
            abstract_: false,
        })],
    )
    .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, "order.md");
    assert!(out[0].1.contains("type: uml.Class"));
    assert!(out[0].1.contains("title: Order"));
    assert!(out[0].1.contains("# Order"));
    let dup = apply(&out, vec![node_new("order", "", "X", vec![])]).unwrap_err();
    assert!(dup.reason.contains("already exists"));
}

#[test]
fn node_new_writes_into_target_directory() {
    let out = apply(&[], vec![node_new("order", "sales", "Order", vec![])]).unwrap();
    assert_eq!(out[0].0, "sales/order.md");
}

#[test]
fn node_set_updates_title_frontmatter_in_place() {
    let b = vec![(
        "a/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierSet {
            id: "order".into(),
            title: Some("Sales Order".into()),
            description: None,
            stereotype: Some(vec!["aggregateRoot".into()]),
            abstract_: None,
            ty: None,
        })],
    )
    .unwrap();
    assert_eq!(out[0].0, "a/order.md", "node.set never moves the file");
    assert!(out[0].1.contains("title: Sales Order"));
    assert!(out[0].1.contains("# Sales Order"));
    assert!(out[0].1.contains("stereotype: [aggregateRoot]"));
}

#[test]
fn node_rm_refuses_referenced_then_allows_cascade() {
    let b = vec![
        ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Money](./money.md)\n".to_string()),
        ("a/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
    ];
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierRemove {
            id: "money".into(),
            cascade: false,
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("referenced by"));
    assert!(err.reason.contains("order"));
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierRemove {
            id: "money".into(),
            cascade: true,
        })],
    )
    .unwrap();
    assert!(out
        .iter()
        .all(|(p, _)| crate::uml::lower::slug_of(p) != "money"));
}

#[test]
fn node_rm_deletes_unreferenced() {
    let b = vec![(
        "a/lonely.md".to_string(),
        "---\ntype: uml.Class\ntitle: Lonely\n---\n# Lonely\n".to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierRemove {
            id: "lonely".into(),
            cascade: false,
        })],
    )
    .unwrap();
    assert!(out.is_empty());
}

#[test]
fn referrers_includes_layout_link_reference() {
    let b = vec![
        ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("a/diagram.md".to_string(),
         "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- [Order](./order.md) with collapsed\n".to_string()),
    ];
    let refs = crate::uml::lower::referrers(&b, "order");
    assert!(
        refs.contains(&"diagram".to_string()),
        "diagram referencing 'order' only via a Layout link must be reported: {refs:?}"
    );
}

#[test]
fn referrers_includes_layout_bare_reference() {
    let b = vec![
        ("a/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string()),
        ("a/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
        ("a/diagram.md".to_string(),
         "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- order left of customer\n".to_string()),
    ];
    let refs = crate::uml::lower::referrers(&b, "order");
    assert!(
        refs.contains(&"diagram".to_string()),
        "diagram referencing 'order' only via a bare Layout operand must be reported: {refs:?}"
    );
}

// ---- full bundle-path id resolution (matches the parse/graph layer's
// `okf::id_of` keying, not just a bare same-directory basename) ----

#[test]
fn find_doc_resolves_full_path_id_for_a_nested_doc() {
    let b = vec![(
        "shop/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n"
            .to_string(),
    )];
    let out = apply(&b, vec![attr_add("shop/order", "total", "Money")]).unwrap();
    assert!(
        out[0].1.contains("- total: Money"),
        "op.node addressed by full-path id must resolve: {:?}",
        out[0].1
    );
}

#[test]
fn attr_add_links_a_known_slug_addressed_by_full_path_id() {
    let b = vec![
        (
            "a/order.md".to_string(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
        ),
        (
            "a/money.md".to_string(),
            "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string(),
        ),
    ];
    // both the node being edited and the type token are passed as full-path ids
    let out = apply(&b, vec![attr_add("a/order", "total", "a/money")]).unwrap();
    assert!(
        out[0].1.contains("- total: [Money](./money.md)"),
        "type token resolved by full-path id must still emit a bare same-directory href: {:?}",
        out[0].1
    );
}

#[test]
fn node_set_resolves_nested_doc_by_full_path_id() {
    let b = vec![(
        "shop/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierSet {
            id: "shop/order".into(),
            title: Some("Sales Order".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        })],
    )
    .unwrap();
    assert_eq!(out[0].0, "shop/order.md");
    assert!(out[0].1.contains("title: Sales Order"));
}

#[test]
fn node_rm_resolves_nested_doc_by_full_path_id_and_referrers_stay_bare() {
    let b = vec![
        ("shop/order.md".to_string(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Money](./money.md)\n".to_string()),
        ("shop/money.md".to_string(), "---\ntype: uml.DataType\ntitle: Money\n---\n# Money\n".to_string()),
    ];
    let err = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierRemove {
            id: "shop/money".into(),
            cascade: false,
        })],
    )
    .unwrap_err();
    assert!(err.reason.contains("referenced by"));
    assert!(err.reason.contains("order"));
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::ClassifierRemove {
            id: "shop/money".into(),
            cascade: true,
        })],
    )
    .unwrap();
    assert!(out
        .iter()
        .all(|(p, _)| crate::uml::lower::slug_of(p) != "money"));
}

#[test]
fn node_new_collision_check_is_scoped_to_the_destination_path_not_global() {
    // A same-basename doc already exists in a different directory — this
    // must NOT collide (full-path keying is what allows same-basename
    // docs to coexist across directories in the first place).
    let b = vec![(
        "shop/order.md".to_string(),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".to_string(),
    )];
    let out = apply(&b, vec![node_new("order", "billing", "Order", vec![])]).unwrap();
    assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
    // same directory + same basename must still collide
    let dup = apply(&b, vec![node_new("order", "shop", "X", vec![])]).unwrap_err();
    assert!(dup.reason.contains("already exists"));
}

#[test]
fn rel_set_resolves_endpoint_target_addressed_by_full_path_id() {
    let b = vec![
        ("shop/order.md".to_string(),
         "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n".to_string()),
        ("shop/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
    ];
    let sel = Selector::Rel {
        source: "shop/order".into(),
        by: RelBy::Endpoint {
            kind: RelationshipKind::Associates,
            target: "shop/customer".into(),
        },
    };
    let ends = parse_ends("1 to 1..* customers").unwrap();
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipSet {
            selector: rel_selector(sel),
            ends: Some(ends),
            name: None,
        })],
    )
    .unwrap();
    let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
    assert!(
        order.contains("1..* customers"),
        "endpoint addressed by full-path id must resolve: {order}"
    );
}

#[test]
fn rel_rm_resolves_endpoint_target_addressed_by_full_path_id() {
    let b = vec![
        ("shop/order.md".to_string(),
         "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n".to_string()),
        ("shop/customer.md".to_string(), "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".to_string()),
    ];
    let sel = Selector::Rel {
        source: "shop/order".into(),
        by: RelBy::Endpoint {
            kind: RelationshipKind::Associates,
            target: "shop/customer".into(),
        },
    };
    let out = apply(
        &b,
        vec![Step::Uml(uml::Op::RelationshipRemove {
            selector: rel_selector(sel),
        })],
    )
    .unwrap();
    let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
    assert!(
        !order.contains("associates"),
        "endpoint addressed by full-path id must resolve for removal: {order}"
    );
}

// ---- Op::DiagramSet / Op::PlaceSet / Op::PlaceRm ----

fn diagram_doc() -> Bundle {
    vec![(
        "shop/dia.md".to_string(),
        "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n".to_string(),
    )]
}

fn full_display() -> DiagramDisplaySet {
    DiagramDisplaySet {
        show_attributes: false,
        show_type: false,
        show_attribute_visibility: false,
        cardinality: CardinalityVisibility::Off,
        max_attributes: Some(6),
        show_roles: false,
        show_cardinality: true,
        show_labels: true,
        show_stereotype: false,
        stereotype_filter: Some(vec!["entity".into()]),
        stereotype_colors: vec!["entity:#ffedd5".into()],
    }
}

fn diagram_set(
    key: &str,
    title: Option<&str>,
    description: Option<&str>,
    clear_description: bool,
    display: Option<DiagramDisplaySet>,
) -> Step {
    Step::Uml(uml::Op::DiagramSet {
        key: key.into(),
        title: title.map(String::from),
        description: description.map(String::from),
        clear_description,
        display,
    })
}

#[test]
fn diagram_set_writes_title_and_note() {
    let out = apply(
        &diagram_doc(),
        vec![diagram_set(
            "dia",
            Some("Order lifecycle"),
            Some("Notes for reviewers"),
            false,
            None,
        )],
    )
    .unwrap();
    assert!(out[0].1.contains("title: Order lifecycle"));
    assert!(out[0].1.contains("# Order lifecycle"), "H1 kept in sync");
    assert!(out[0].1.contains("description: Notes for reviewers"));
}

#[test]
fn diagram_set_round_trips_multiline_description_through_reopen() {
    let out = apply(
        &diagram_doc(),
        vec![diagram_set(
            "dia",
            None,
            Some("First line\nSecond line"),
            false,
            None,
        )],
    )
    .unwrap();

    assert!(
        out[0]
            .1
            .contains(r#"description: "First line\nSecond line""#),
        "serialized diagram must escape the embedded line break: {}",
        out[0].1
    );
    let reopened = projection(&out);
    assert_eq!(
        reopened.diagrams[0].description.as_deref(),
        Some("First line\nSecond line")
    );
}

#[test]
fn diagram_set_explicitly_clears_an_authored_description() {
    let described = apply(
        &diagram_doc(),
        vec![diagram_set(
            "dia",
            None,
            Some("Notes for reviewers"),
            false,
            None,
        )],
    )
    .unwrap();

    let cleared = apply(&described, vec![diagram_set("dia", None, None, true, None)]).unwrap();

    assert!(
        !cleared[0].1.contains("description:"),
        "explicit clear must remove the frontmatter key: {}",
        cleared[0].1
    );
    let model = projection(&cleared);
    assert_eq!(model.diagrams[0].description, None);
}

#[test]
fn diagram_set_writes_cardinality_mode() {
    let out = apply(
        &diagram_doc(),
        vec![diagram_set(
            "dia",
            None,
            None,
            false,
            Some(DiagramDisplaySet {
                cardinality: CardinalityVisibility::Explicit,
                show_cardinality: false,
                ..full_display()
            }),
        )],
    )
    .unwrap();
    assert!(out[0].1.contains("cardinality: explicit"));
    assert!(out[0].1.contains("showCardinality: false"));
}

#[test]
fn diagram_set_legacy_attribute_gate_tracks_cardinality() {
    for (cardinality, expected_gate) in [
        (CardinalityVisibility::Off, false),
        (CardinalityVisibility::Explicit, true),
        (CardinalityVisibility::All, true),
    ] {
        let out = apply(
            &diagram_doc(),
            vec![diagram_set(
                "dia",
                None,
                None,
                false,
                Some(DiagramDisplaySet {
                    cardinality,
                    ..full_display()
                }),
            )],
        )
        .unwrap();

        assert!(
            out[0]
                .1
                .contains(&format!("showAttributeMultiplicity: {expected_gate}")),
            "{cardinality:?} must author the compatible legacy gate: {}",
            out[0].1
        );
    }
}

#[test]
fn diagram_set_replaces_display_block_and_drops_stale_keys() {
    let set = apply(
        &diagram_doc(),
        vec![diagram_set("dia", None, None, false, Some(full_display()))],
    )
    .unwrap();
    assert!(set[0].1.contains("showAttributes: false"));
    assert!(set[0].1.contains("maxAttributes: 6"));
    assert!(set[0].1.contains("stereotypeFilter: [entity]"));

    // A second DiagramSet with a display that omits maxAttributes/stereotypeFilter
    // must drop those stale keys entirely (whole-block replace).
    let cleared = apply(
        &set,
        vec![diagram_set(
            "dia",
            None,
            None,
            false,
            Some(DiagramDisplaySet {
                max_attributes: None,
                stereotype_filter: None,
                stereotype_colors: vec![],
                ..full_display()
            }),
        )],
    )
    .unwrap();
    assert!(
        !cleared[0].1.contains("maxAttributes"),
        "stale key must be dropped: {}",
        cleared[0].1
    );
    assert!(
        !cleared[0].1.contains("stereotypeFilter"),
        "stale key must be dropped: {}",
        cleared[0].1
    );
    assert!(
        !cleared[0].1.contains("stereotypeColors"),
        "stale key must be dropped: {}",
        cleared[0].1
    );
}

#[test]
fn diagram_set_on_missing_diagram_errors() {
    let err = apply(
        &diagram_doc(),
        vec![diagram_set("ghost", Some("X"), None, false, None)],
    )
    .unwrap_err();
    assert!(err.reason.contains("no document 'ghost'"));
}

#[test]
fn diagram_set_leaves_untouched_fields_alone() {
    let out = apply(
        &diagram_doc(),
        vec![diagram_set("dia", None, None, false, None)],
    )
    .unwrap();
    // A no-op syntax-native DiagramSet preserves all authored bytes. Domain
    // lowering is not a hidden canonical-formatting boundary.
    let original = &diagram_doc()[0].1;
    assert_eq!(
        &out[0].1, original,
        "no-op DiagramSet preserves authored source exactly"
    );
}

#[test]
fn diagram_set_resolves_nested_doc_by_full_path_id() {
    let out = apply(
        &diagram_doc(),
        vec![diagram_set("shop/dia", Some("D2"), None, false, None)],
    )
    .unwrap();
    assert_eq!(out[0].0, "shop/dia.md");
    assert!(out[0].1.contains("title: D2"));
}

// ---- Op::PlaceSet (## Layout write-back, Phase A) ----

/// A `## Layout` diagram doc whose Layout body is `layout_body` (may be "").
fn layout_diagram(layout_body: &str) -> Bundle {
    vec![(
        "shop/dia.md".to_string(),
        format!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n{layout_body}"
        ),
    )]
}

/// A diagram doc with NO `## Layout` section.
fn diagram_no_layout() -> Bundle {
    vec![(
        "shop/dia.md".to_string(),
        "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n".to_string(),
    )]
}

fn placeset(subject: (&str, &str), reference: (&str, &str), directions: Vec<Direction>) -> Step {
    Step::Uml(uml::Op::PlacementSet {
        diagram: "dia".into(),
        subject_title: subject.0.into(),
        subject_slug: subject.1.into(),
        reference_title: reference.0.into(),
        reference_slug: reference.1.into(),
        directions,
    })
}

#[test]
fn place_set_adds_a_left_of_placement() {
    let b = layout_diagram("- [Customer](./customer.md) below [Order](./order.md)\n");
    let out = apply(
        &b,
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::LeftOf],
        )],
    )
    .unwrap();
    assert!(
        out[0]
            .1
            .contains("- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)"),
        "authored placement present: {}",
        out[0].1
    );
    assert!(
        out[0]
            .1
            .contains("- [Customer](./customer.md) below [Order](./order.md)"),
        "existing layout line kept: {}",
        out[0].1
    );
}

#[test]
fn place_set_creates_layout_section_when_absent() {
    let out = apply(
        &diagram_no_layout(),
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::LeftOf],
        )],
    )
    .unwrap();
    assert!(
        out[0].1.contains("## Layout"),
        "Layout section created when absent: {}",
        out[0].1
    );
    assert!(out[0]
        .1
        .contains("- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)"));
}

#[test]
fn place_set_replaces_same_axis_placement() {
    let b =
        layout_diagram("- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n");
    let out = apply(
        &b,
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::RightOf],
        )],
    )
    .unwrap();
    assert!(
        out[0]
            .1
            .contains("- [Order](./order.md) right of [PaymentGateway](./payment-gateway.md)"),
        "new horizontal placement present: {}",
        out[0].1
    );
    assert!(
        !out[0].1.contains("left of"),
        "prior same-pair placement replaced, not duplicated: {}",
        out[0].1
    );
}

#[test]
fn place_set_replaces_a_reversed_pair_placement() {
    // One relation per UNORDERED pair. A prior drag authored
    // `PaymentGateway left of Order` (subject=PaymentGateway). Re-dragging
    // Order against PaymentGateway -- the SAME pair with subject/reference
    // swapped -- must REWRITE that relation, not stack a second conflicting
    // one. The retain check is pair-symmetric, so operand order can't hide
    // the existing line.
    let b =
        layout_diagram("- [PaymentGateway](./payment-gateway.md) left of [Order](./order.md)\n");
    let out = apply(
        &b,
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::RightOf],
        )],
    )
    .unwrap();
    assert!(
        out[0]
            .1
            .contains("- [Order](./order.md) right of [PaymentGateway](./payment-gateway.md)"),
        "new placement present: {}",
        out[0].1
    );
    assert_eq!(
        layout_statement_count(&out),
        1,
        "reversed-pair placement replaced, not stacked: {}",
        out[0].1
    );
}

#[test]
fn place_set_rewrites_a_different_axis_placement() {
    // One relation per pair: authoring a horizontal placement REWRITES an
    // existing vertical one for the same pair. The solver can't hold both --
    // each direction center-aligns the cross axis, so `left of` + `above` on
    // one pair mutually conflict and both get dropped. So a re-drag onto a
    // target already related replaces the relation wholesale.
    let b = layout_diagram("- [Order](./order.md) above [PaymentGateway](./payment-gateway.md)\n");
    let out = apply(
        &b,
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::LeftOf],
        )],
    )
    .unwrap();
    assert!(
        out[0].1.contains("left of"),
        "new horizontal placement present: {}",
        out[0].1
    );
    assert!(
        !out[0].1.contains("above"),
        "prior placement on the other axis rewritten, not kept: {}",
        out[0].1
    );
}

#[test]
fn place_set_diagonal_replaces_a_cardinal_for_the_same_pair() {
    // A single diagonal Direction is ONE placement. Re-dragging Order onto a
    // corner of PaymentGateway rewrites the prior cardinal for that ordered
    // pair, not stacking a conflicting second relation.
    let b = layout_diagram(
        "- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)
",
    );
    let out = apply(
        &b,
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::AboveLeft],
        )],
    )
    .unwrap();
    assert!(
        out[0]
            .1
            .contains("- [Order](./order.md) above left of [PaymentGateway](./payment-gateway.md)"),
        "authored diagonal present: {}",
        out[0].1
    );
    // The prior bare `... .md) left of ...` line is gone. (The diagonal line
    // reads `... .md) above left of ...`, so `.md) left of` distinguishes it.)
    assert!(
        !out[0].1.contains("md) left of"),
        "prior cardinal replaced, not kept: {}",
        out[0].1
    );
}

#[test]
fn place_set_corner_authors_two_statements() {
    let out = apply(
        &diagram_no_layout(),
        vec![placeset(
            ("Order", "order"),
            ("PaymentGateway", "payment-gateway"),
            vec![Direction::LeftOf, Direction::Above],
        )],
    )
    .unwrap();
    assert!(
        out[0].1.contains("left of"),
        "horizontal statement: {}",
        out[0].1
    );
    assert!(
        out[0].1.contains("above"),
        "vertical statement: {}",
        out[0].1
    );
    // Two separate 2-operand placement bullets
    // (invariant: directions.len() == operands.len() - 1).
    assert_eq!(
        layout_statement_count(&out),
        2,
        "corner drop authored two statements"
    );
}

// ---- Op::PlaceRm (## Layout removal) ----

fn placerm(subject_slug: &str, reference_slug: &str) -> Step {
    Step::Uml(uml::Op::PlacementRemove {
        diagram: "dia".into(),
        subject_slug: subject_slug.into(),
        reference_slug: reference_slug.into(),
    })
}

#[test]
fn place_rm_removes_a_matching_placement() {
    let b =
        layout_diagram("- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n");
    let out = apply(&b, vec![placerm("order", "payment-gateway")]).unwrap();
    assert!(
        !out[0].1.contains("left of"),
        "matching placement removed: {}",
        out[0].1
    );
}

#[test]
fn place_rm_removes_a_reversed_pair_placement() {
    // Stored order is PaymentGateway left of Order; remove with the operand
    // order swapped (subject=order, reference=payment-gateway) -- pair
    // symmetry means it still matches and is removed.
    let b =
        layout_diagram("- [PaymentGateway](./payment-gateway.md) left of [Order](./order.md)\n");
    let out = apply(&b, vec![placerm("order", "payment-gateway")]).unwrap();
    assert!(
        !out[0].1.contains("left of"),
        "reversed-pair placement removed: {}",
        out[0].1
    );
}

#[test]
fn place_rm_is_a_noop_when_absent() {
    let b = layout_diagram("- [Customer](./customer.md) below [Order](./order.md)\n");
    let out = apply(&b, vec![placerm("order", "payment-gateway")]).unwrap();
    assert!(
        out[0]
            .1
            .contains("- [Customer](./customer.md) below [Order](./order.md)"),
        "unrelated placement kept: {}",
        out[0].1
    );
}

#[test]
fn place_rm_is_a_noop_without_a_layout_section() {
    let out = apply(
        &diagram_no_layout(),
        vec![placerm("order", "payment-gateway")],
    )
    .unwrap();
    assert!(
        !out[0].1.contains("- ["),
        "no placement introduced by a no-op removal on a docless-Layout diagram: {}",
        out[0].1
    );
}
