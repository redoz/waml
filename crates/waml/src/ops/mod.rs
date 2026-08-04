use crate::layout::Direction;
use crate::model::{ElementType, RelEnd, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::source::SourceBundle;

pub type Bundle = Vec<(String, String)>;

pub type OpError = crate::edit::EditError;

pub use crate::uml::{DiagramDisplaySet, FieldEdit, NameSpec};

/// One mutation. One variant per sugar command; grows task by task.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    AttrAdd {
        node: String,
        name: String,
        ty_token: String,
        multiplicity: Option<Multiplicity>,
        visibility: Option<Visibility>,
    },
    AttrSet {
        node: String,
        name: String,
        ty_token: Option<String>,
        multiplicity: FieldEdit<Multiplicity>,
        visibility: Option<Visibility>,
        rename: Option<String>,
    },
    AttrRm {
        node: String,
        name: String,
    },
    ValueAdd {
        node: String,
        literal: String,
    },
    ValueRm {
        node: String,
        literal: String,
    },
    RelAdd {
        source: String,
        kind: RelationshipKind,
        target: String,
        name: Option<NameSpec>,
        ends: Option<(RelEnd, RelEnd)>,
    },
    RelSet {
        selector: Selector,
        ends: Option<(RelEnd, RelEnd)>,
        name: Option<NameSpec>,
    },
    RelRm {
        selector: Selector,
    },
    NodeNew {
        slug: String,
        /// Target package directory ("" = root). File written at `<dir>/<slug>.md`.
        dir: String,
        ty: ElementType,
        title: String,
        stereotype: Vec<String>,
        description: Option<String>,
        abstract_: bool,
    },
    NodeSet {
        slug: String,
        title: Option<String>,
        description: Option<String>,
        stereotype: Option<Vec<String>>,
        abstract_: Option<bool>,
        ty: Option<ElementType>,
    },
    NodeRm {
        slug: String,
        cascade: bool,
    },
    NodeRename {
        from: String,
        to: String,
    },
    PkgMove {
        slug: String,
        to_dir: String,
    },
    PkgRename {
        from: String,
        to: String,
    },
    PkgDelete {
        path: String,
        cascade: bool,
    },
    PkgReorder {
        path: String,
        order: Vec<String>,
    },
    PkgSort {
        path: String,
    },
    PkgRetitle {
        path: String,
        title: String,
    },
    PkgInsert {
        parent_path: String,
        name: String,
        docs: Vec<(String, String)>,
    },
    DiagramSet {
        key: String,                        // diagram doc id (full-path or bare slug)
        title: Option<String>,              // None = leave unchanged
        description: Option<String>,        // None = leave unchanged
        clear_description: bool,            // true = remove authored description
        display: Option<DiagramDisplaySet>, // None = leave display untouched
    },
    PlaceSet {
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        directions: Vec<Direction>,
    },
    PlaceRm {
        diagram: String,
        subject_slug: String,
        reference_slug: String,
    },
}

pub fn apply(bundle: &[(String, String)], ops: &[Op]) -> Result<Bundle, OpError> {
    let source = SourceBundle::try_from_pairs(bundle.iter().cloned())
        .map_err(|error| OpError::at("bundle", error.to_string()))?;
    apply_source(&source, ops).map(|bundle| bundle.to_pairs())
}

pub fn apply_source(bundle: &SourceBundle, ops: &[Op]) -> Result<SourceBundle, OpError> {
    let mut steps = Vec::with_capacity(ops.len());
    for (index, op) in ops.iter().cloned().enumerate() {
        let step = crate::compat::step_from_legacy(op).map_err(|mut error| {
            error.index = index;
            error
        })?;
        steps.push(step);
    }
    let batch = crate::compat::Batch::new(steps);
    crate::compat::apply(bundle, &batch)
}

pub fn referrers(bundle: &Bundle, slug: &str) -> Vec<String> {
    crate::uml::lower::referrers(bundle, slug)
}

pub mod selector {
    pub use crate::uml::selector::*;
}
pub use selector::{parse_selector, render_selector, RelBy, Selector};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{parse_ends, CardinalityVisibility, ElementType, RelationshipKind};
    use crate::ops::selector::{RelBy, Selector};
    use crate::uml::lower::slug_of;

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

    fn attr_add(node: &str, name: &str, ty: &str) -> Op {
        Op::AttrAdd {
            node: node.into(),
            name: name.into(),
            ty_token: ty.into(),
            multiplicity: None,
            visibility: None,
        }
    }

    #[test]
    fn node_new_writes_frontmatter_and_title_and_refuses_dup() {
        let b: Bundle = vec![];
        let out = apply(
            &b,
            &[Op::NodeNew {
                slug: "order".into(),
                dir: String::new(),
                ty: ElementType::parse("uml.Class"),
                title: "Order".into(),
                stereotype: vec!["entity".into()],
                description: Some("An order.".into()),
                abstract_: false,
            }],
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, "order.md");
        assert!(out[0].1.contains("type: uml.Class"));
        assert!(out[0].1.contains("title: Order"));
        assert!(out[0].1.contains("# Order"));
        let dup = apply(
            &out,
            &[Op::NodeNew {
                slug: "order".into(),
                dir: String::new(),
                ty: ElementType::parse("uml.Class"),
                title: "X".into(),
                stereotype: vec![],
                description: None,
                abstract_: false,
            }],
        )
        .unwrap_err();
        assert!(dup.reason.contains("already exists"));
    }

    #[test]
    fn node_new_writes_into_target_directory() {
        let out = apply(
            &[],
            &[Op::NodeNew {
                slug: "order".into(),
                dir: "sales".into(),
                ty: ElementType::parse("uml.Class"),
                title: "Order".into(),
                stereotype: vec![],
                description: None,
                abstract_: false,
            }],
        )
        .unwrap();
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
            &[Op::NodeSet {
                slug: "order".into(),
                title: Some("Sales Order".into()),
                description: None,
                stereotype: Some(vec!["aggregateRoot".into()]),
                abstract_: None,
                ty: None,
            }],
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
            &[Op::NodeRm {
                slug: "money".into(),
                cascade: false,
            }],
        )
        .unwrap_err();
        assert!(err.reason.contains("referenced by"));
        assert!(err.reason.contains("order"));
        let out = apply(
            &b,
            &[Op::NodeRm {
                slug: "money".into(),
                cascade: true,
            }],
        )
        .unwrap();
        assert!(out.iter().all(|(p, _)| slug_of(p) != "money"));
    }

    #[test]
    fn node_rm_deletes_unreferenced() {
        let b = vec![(
            "a/lonely.md".to_string(),
            "---\ntype: uml.Class\ntitle: Lonely\n---\n# Lonely\n".to_string(),
        )];
        let out = apply(
            &b,
            &[Op::NodeRm {
                slug: "lonely".into(),
                cascade: false,
            }],
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
        let refs = referrers(&b, "order");
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
        let refs = referrers(&b, "order");
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
        let out = apply(&b, &[attr_add("shop/order", "total", "Money")]).unwrap();
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
        let out = apply(&b, &[attr_add("a/order", "total", "a/money")]).unwrap();
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
            &[Op::NodeSet {
                slug: "shop/order".into(),
                title: Some("Sales Order".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }],
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
            &[Op::NodeRm {
                slug: "shop/money".into(),
                cascade: false,
            }],
        )
        .unwrap_err();
        assert!(err.reason.contains("referenced by"));
        assert!(err.reason.contains("order"));
        let out = apply(
            &b,
            &[Op::NodeRm {
                slug: "shop/money".into(),
                cascade: true,
            }],
        )
        .unwrap();
        assert!(out.iter().all(|(p, _)| slug_of(p) != "money"));
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
        let out = apply(
            &b,
            &[Op::NodeNew {
                slug: "order".into(),
                dir: "billing".into(),
                ty: ElementType::parse("uml.Class"),
                title: "Order".into(),
                stereotype: vec![],
                description: None,
                abstract_: false,
            }],
        )
        .unwrap();
        assert!(out.iter().any(|(p, _)| p == "billing/order.md"));
        // same directory + same basename must still collide
        let dup = apply(
            &b,
            &[Op::NodeNew {
                slug: "order".into(),
                dir: "shop".into(),
                ty: ElementType::parse("uml.Class"),
                title: "X".into(),
                stereotype: vec![],
                description: None,
                abstract_: false,
            }],
        )
        .unwrap_err();
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
            &[Op::RelSet {
                selector: sel,
                ends: Some(ends),
                name: None,
            }],
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
        let out = apply(&b, &[Op::RelRm { selector: sel }]).unwrap();
        let order = &out.iter().find(|(p, _)| p == "shop/order.md").unwrap().1;
        assert!(
            !order.contains("associates"),
            "endpoint addressed by full-path id must resolve for removal: {order}"
        );
    }

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

    #[test]
    fn diagram_set_writes_title_and_note() {
        let out = apply(
            &diagram_doc(),
            &[Op::DiagramSet {
                key: "dia".into(),
                title: Some("Order lifecycle".into()),
                description: Some("Notes for reviewers".into()),
                clear_description: false,
                display: None,
            }],
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
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: Some("First line\nSecond line".into()),
                clear_description: false,
                display: None,
            }],
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
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: Some("Notes for reviewers".into()),
                clear_description: false,
                display: None,
            }],
        )
        .unwrap();

        let cleared = apply(
            &described,
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: None,
                clear_description: true,
                display: None,
            }],
        )
        .unwrap();

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
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: None,
                clear_description: false,
                display: Some(DiagramDisplaySet {
                    cardinality: CardinalityVisibility::Explicit,
                    show_cardinality: false,
                    ..full_display()
                }),
            }],
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
                &[Op::DiagramSet {
                    key: "dia".into(),
                    title: None,
                    description: None,
                    clear_description: false,
                    display: Some(DiagramDisplaySet {
                        cardinality,
                        ..full_display()
                    }),
                }],
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
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: None,
                clear_description: false,
                display: Some(full_display()),
            }],
        )
        .unwrap();
        assert!(set[0].1.contains("showAttributes: false"));
        assert!(set[0].1.contains("maxAttributes: 6"));
        assert!(set[0].1.contains("stereotypeFilter: [entity]"));

        // A second DiagramSet with a display that omits maxAttributes/stereotypeFilter
        // must drop those stale keys entirely (whole-block replace).
        let cleared = apply(
            &set,
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: None,
                clear_description: false,
                display: Some(DiagramDisplaySet {
                    max_attributes: None,
                    stereotype_filter: None,
                    stereotype_colors: vec![],
                    ..full_display()
                }),
            }],
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
            &[Op::DiagramSet {
                key: "ghost".into(),
                title: Some("X".into()),
                description: None,
                clear_description: false,
                display: None,
            }],
        )
        .unwrap_err();
        assert!(err.reason.contains("no document 'ghost'"));
    }

    #[test]
    fn diagram_set_leaves_untouched_fields_alone() {
        let out = apply(
            &diagram_doc(),
            &[Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: None,
                clear_description: false,
                display: None,
            }],
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
            &[Op::DiagramSet {
                key: "shop/dia".into(),
                title: Some("D2".into()),
                description: None,
                clear_description: false,
                display: None,
            }],
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

    fn placeset(subject: (&str, &str), reference: (&str, &str), directions: Vec<Direction>) -> Op {
        Op::PlaceSet {
            diagram: "dia".into(),
            subject_title: subject.0.into(),
            subject_slug: subject.1.into(),
            reference_title: reference.0.into(),
            reference_slug: reference.1.into(),
            directions,
        }
    }

    #[test]
    fn place_set_adds_a_left_of_placement() {
        let b = layout_diagram("- [Customer](./customer.md) below [Order](./order.md)\n");
        let out = apply(
            &b,
            &[placeset(
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
            &[placeset(
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
        let b = layout_diagram(
            "- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n",
        );
        let out = apply(
            &b,
            &[placeset(
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
        let b = layout_diagram(
            "- [PaymentGateway](./payment-gateway.md) left of [Order](./order.md)\n",
        );
        let out = apply(
            &b,
            &[placeset(
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
        let b =
            layout_diagram("- [Order](./order.md) above [PaymentGateway](./payment-gateway.md)\n");
        let out = apply(
            &b,
            &[placeset(
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
            &[placeset(
                ("Order", "order"),
                ("PaymentGateway", "payment-gateway"),
                vec![Direction::AboveLeft],
            )],
        )
        .unwrap();
        assert!(
            out[0].1.contains(
                "- [Order](./order.md) above left of [PaymentGateway](./payment-gateway.md)"
            ),
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
            &[placeset(
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

    fn placerm(subject_slug: &str, reference_slug: &str) -> Op {
        Op::PlaceRm {
            diagram: "dia".into(),
            subject_slug: subject_slug.into(),
            reference_slug: reference_slug.into(),
        }
    }

    #[test]
    fn place_rm_removes_a_matching_placement() {
        let b = layout_diagram(
            "- [Order](./order.md) left of [PaymentGateway](./payment-gateway.md)\n",
        );
        let out = apply(&b, &[placerm("order", "payment-gateway")]).unwrap();
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
        let b = layout_diagram(
            "- [PaymentGateway](./payment-gateway.md) left of [Order](./order.md)\n",
        );
        let out = apply(&b, &[placerm("order", "payment-gateway")]).unwrap();
        assert!(
            !out[0].1.contains("left of"),
            "reversed-pair placement removed: {}",
            out[0].1
        );
    }

    #[test]
    fn place_rm_is_a_noop_when_absent() {
        let b = layout_diagram("- [Customer](./customer.md) below [Order](./order.md)\n");
        let out = apply(&b, &[placerm("order", "payment-gateway")]).unwrap();
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
        let out = apply(&diagram_no_layout(), &[placerm("order", "payment-gateway")]).unwrap();
        assert!(
            !out[0].1.contains("- ["),
            "no placement introduced by a no-op removal on a docless-Layout diagram: {}",
            out[0].1
        );
    }
}
