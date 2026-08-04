//! Legacy `waml::ops::Op` test suite, ported 1:1 onto [`Step`]/[`Batch`]/
//! [`crate::edit::apply`]. See the porting table in the retire-compat plan
//! for the mapping used to translate each `Op` literal.

use super::{Batch, EditError, Step};
use crate::model::{parse_ends, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::source::SourceBundle;
use crate::uml::selector::{RelBy, Selector};
use crate::uml::{self, FieldEdit, NameSpec, RelationshipSelector};

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

#[allow(dead_code)]
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
