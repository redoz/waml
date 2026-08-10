use super::{RelationshipSelector, Selector};
use crate::edit::{EditBatch, EditContext, EditError};
use crate::layout::Direction;
use crate::model::{CardinalityVisibility, ElementType, RelEnd, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::okf::DirectoryAddress;
use crate::source::SourceBundle;

/// How a relationship's name is given on an op (a `Ref`'s title is resolved at apply time).
#[derive(Debug, Clone, PartialEq)]
pub enum NameSpec {
    Label(String),
    Ref(String), // target slug
}

/// Intent for editing an optional authored field.
///
/// `Unchanged` preserves the current value, `Clear` removes it, and `Set`
/// replaces it. On serde wire boundaries an omitted field defaults to
/// `Unchanged`, an explicit `null` is `Clear`, and a value is `Set`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FieldEdit<T> {
    #[default]
    Unchanged,
    Clear,
    Set(T),
}

impl<T> FieldEdit<T> {
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}

#[cfg(feature = "serde")]
impl<T: serde::Serialize> serde::Serialize for FieldEdit<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Unchanged | Self::Clear => serializer.serialize_none(),
            Self::Set(value) => value.serialize(serializer),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, T: serde::Deserialize<'de>> serde::Deserialize<'de> for FieldEdit<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(
            match <Option<T> as serde::Deserialize>::deserialize(deserializer)? {
                Some(value) => Self::Set(value),
                None => Self::Clear,
            },
        )
    }
}

/// A fully-specified display block. The panel always holds a resolved
/// display, so every non-nullable field is present; nullable fields use
/// their own absent state (`None` ⇒ omit the key).
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramDisplaySet {
    pub show_attributes: bool,
    pub show_type: bool,
    pub show_attribute_visibility: bool,
    pub cardinality: CardinalityVisibility,
    pub max_attributes: Option<u32>,
    pub show_roles: bool,
    pub show_cardinality: bool,
    pub show_labels: bool,
    pub show_stereotype: bool,
    pub stereotype_filter: Option<Vec<String>>,
    pub stereotype_colors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionSelector {
    pub behavior: String,
    pub source_node: String,
    pub occurrence: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceSpec {
    pub label: String,
    pub href: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceEdit {
    Insert {
        index: usize,
        label: String,
        href: String,
    },
    Update {
        index: usize,
        label: String,
        href: String,
    },
    Remove {
        index: usize,
    },
    Move {
        from: usize,
        to: usize,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    AttributeAdd {
        node: String,
        name: String,
        ty_token: String,
        multiplicity: Option<Multiplicity>,
        visibility: Option<Visibility>,
    },
    AttributeSet {
        node: String,
        name: String,
        ty_token: Option<String>,
        multiplicity: FieldEdit<Multiplicity>,
        visibility: Option<Visibility>,
        rename: Option<String>,
    },
    AttributeRemove {
        node: String,
        name: String,
    },
    ValueAdd {
        node: String,
        literal: String,
    },
    ValueRemove {
        node: String,
        literal: String,
    },
    RelationshipAdd {
        source: String,
        kind: RelationshipKind,
        target: String,
        name: Option<NameSpec>,
        ends: Option<(RelEnd, RelEnd)>,
    },
    RelationshipSet {
        selector: RelationshipSelector,
        ends: Option<(RelEnd, RelEnd)>,
        name: Option<NameSpec>,
    },
    RelationshipRemove {
        selector: RelationshipSelector,
    },
    ClassifierNew {
        slug: String,
        directory: DirectoryAddress,
        ty: ElementType,
        title: String,
        stereotype: Vec<String>,
        description: Option<String>,
        abstract_: bool,
    },
    ClassifierSet {
        id: String,
        title: Option<String>,
        description: Option<String>,
        stereotype: Option<Vec<String>>,
        abstract_: Option<bool>,
        ty: Option<ElementType>,
    },
    ClassifierRemove {
        id: String,
        cascade: bool,
    },
    ClassifierRename {
        from: String,
        to: String,
    },
    DiagramSet {
        key: String,
        title: Option<String>,
        description: Option<String>,
        clear_description: bool,
        display: Option<DiagramDisplaySet>,
    },
    PlacementSet {
        diagram: String,
        subject_title: String,
        subject_slug: String,
        reference_title: String,
        reference_slug: String,
        directions: Vec<Direction>,
    },
    PlacementRemove {
        diagram: String,
        subject_slug: String,
        reference_slug: String,
    },
    EditTransitionTraces {
        selector: TransitionSelector,
        edit: TraceEdit,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch(pub Vec<Op>);

impl crate::edit::sealed::Sealed for Batch {}

impl EditBatch for Batch {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        let mut cursor = super::lower::UmlLoweringCursor::new(context);
        for (index, op) in self.0.iter().enumerate() {
            cursor.apply(index, op)?;
        }
        Ok(cursor.finish())
    }
}

fn require_claimed(
    state: &super::lower::UmlLoweringState,
    work: &SourceBundle,
    target: &str,
    op: &str,
) -> Result<(), EditError> {
    if state.path(target).is_some() {
        return Ok(());
    }
    if super::lower::resolve_index(work, target).is_none() {
        Err(EditError::at(op, format!("no document '{target}'")))
    } else {
        Err(EditError::at(
            op,
            format!("'{target}' is not claimed by the UML projection"),
        ))
    }
}

pub(crate) fn lower_one_with_state(
    work: &mut SourceBundle,
    state: &mut super::lower::UmlLoweringState,
    op: &Op,
) -> Result<(), EditError> {
    match op {
        Op::AttributeAdd {
            node,
            name,
            ty_token,
            multiplicity,
            visibility,
        } => {
            require_claimed(state, work, node, "attr.add")?;
            super::lower::op_attr_add(work, state, node, name, ty_token, multiplicity, *visibility)
        }
        Op::AttributeSet {
            node,
            name,
            ty_token,
            multiplicity,
            visibility,
            rename,
        } => {
            require_claimed(state, work, node, "attr.set")?;
            super::lower::op_attr_set(
                work,
                state,
                node,
                name,
                ty_token,
                multiplicity,
                *visibility,
                rename,
            )
        }
        Op::AttributeRemove { node, name } => {
            require_claimed(state, work, node, "attr.rm")?;
            super::lower::op_attr_rm(work, state, node, name)
        }
        Op::ValueAdd { node, literal } => {
            require_claimed(state, work, node, "value.add")?;
            super::lower::op_value_add(work, state, node, literal)
        }
        Op::ValueRemove { node, literal } => {
            require_claimed(state, work, node, "value.rm")?;
            super::lower::op_value_rm(work, state, node, literal)
        }
        Op::RelationshipAdd {
            source,
            kind,
            target,
            name,
            ends,
        } => {
            require_claimed(state, work, source, "rel.add")?;
            require_claimed(state, work, target, "rel.add")?;
            super::lower::op_rel_add(work, state, source, *kind, target, name, ends)
        }
        Op::RelationshipSet {
            selector,
            ends,
            name,
        } => {
            let selector = Selector::from(selector.clone());
            require_claimed(state, work, selector.source(), "rel.set")?;
            super::lower::op_rel_set(work, state, &selector, ends, name)
        }
        Op::RelationshipRemove { selector } => {
            let selector = Selector::from(selector.clone());
            require_claimed(state, work, selector.source(), "rel.rm")?;
            super::lower::op_rel_rm(work, state, &selector)
        }
        Op::ClassifierNew {
            slug,
            directory,
            ty,
            title,
            stereotype,
            description,
            abstract_,
        } => {
            if !crate::uml::recognizes_type(ty) {
                return Err(EditError::at("node.new", "type is not claimed by UML"));
            }
            super::lower::op_node_new(
                work,
                slug,
                &crate::okf::ops::legacy_path(directory),
                ty,
                title,
                stereotype,
                description,
                *abstract_,
            )
        }
        Op::ClassifierSet {
            id,
            title,
            description,
            stereotype,
            abstract_,
            ty,
        } => {
            require_claimed(state, work, id, "node.set")?;
            super::lower::op_node_set(
                work,
                state,
                id,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            )
        }
        Op::ClassifierRemove { id, cascade } => {
            require_claimed(state, work, id, "node.rm")?;
            super::lower::op_node_rm(work, state, id, *cascade)
        }
        Op::ClassifierRename { from, to } => {
            require_claimed(state, work, from, "node.rename")?;
            super::rename::op_node_rename(work, state, from, to)
        }
        Op::DiagramSet {
            key,
            title,
            description,
            clear_description,
            display,
        } => {
            require_claimed(state, work, key, "diagram.set")?;
            super::lower::op_diagram_set(
                work,
                state,
                key,
                title,
                description,
                *clear_description,
                display,
            )
        }
        Op::PlacementSet {
            diagram,
            subject_title,
            subject_slug,
            reference_title,
            reference_slug,
            directions,
        } => {
            require_claimed(state, work, diagram, "place.set")?;
            super::lower::op_place_set(
                work,
                state,
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            )
        }
        Op::PlacementRemove {
            diagram,
            subject_slug,
            reference_slug,
        } => {
            require_claimed(state, work, diagram, "place.rm")?;
            super::lower::op_place_rm(work, state, diagram, subject_slug, reference_slug)
        }
        Op::EditTransitionTraces { selector, edit } => {
            super::lower::op_transition_traces_edit(work, state, selector, edit)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::{AppliedEdit, EditBatch};
    use crate::uml::selector::RelBy;

    fn apply_reversible(batch: &impl EditBatch, source: &SourceBundle) -> AppliedEdit {
        batch.apply_reversible(context(source)).unwrap()
    }

    fn assert_reversible(
        source: SourceBundle,
        batch: Batch,
        assert_forward: impl FnOnce(&SourceBundle),
    ) {
        let applied = apply_reversible(&batch, &source);
        assert_forward(&applied.source);
        let forward = applied.source.clone();

        let restored = apply_reversible(&applied.inverse, &applied.source);
        assert_eq!(restored.source, source);

        let redone = apply_reversible(&restored.inverse, &restored.source);
        assert_eq!(redone.source, forward);
    }

    fn context(source: &SourceBundle) -> EditContext<'_> {
        let okf_analysis = Box::leak(Box::new(
            crate::analysis::analyze_okf(source, None, 0).unwrap(),
        ));
        let uml = Box::leak(Box::new(
            crate::uml::analyze(
                crate::analysis::DomainAnalysisContext {
                    source,
                    catalog: &okf_analysis.catalog,
                    markdown: &okf_analysis.markdown,
                    okf: &okf_analysis.bundle,
                    session_revision: 0,
                },
                None,
            )
            .unwrap(),
        ));
        EditContext {
            source,
            okf_analysis,
            session_revision: 0,
            uml,
        }
    }

    #[test]
    fn ordered_batch_is_atomic_and_copy_on_write() {
        let source = SourceBundle::try_from_pairs([
            ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
            ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
        ])
        .unwrap();
        let batch = Batch(vec![
            Op::ClassifierSet {
                id: "a".into(),
                title: Some("Changed".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            },
            Op::AttributeRemove {
                node: "missing".into(),
                name: "x".into(),
            },
        ]);
        assert!(batch.apply_reversible(context(&source)).is_err());
        assert!(source.documents()[0].text().contains("title: A"));

        let changed = Batch(vec![Op::ClassifierSet {
            id: "a".into(),
            title: Some("Changed".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        }])
        .lower(context(&source))
        .unwrap();
        assert!(!changed.shares_text_with(&source, "a.md"));
        assert!(changed.shares_text_with(&source, "b.md"));
    }

    #[test]
    fn uml_lowerer_rejects_unclaimed_okf_concepts() {
        let source = SourceBundle::try_from_pairs([(
            "vendor.md",
            "---\ntype: vendor.Custom\ntitle: Vendor\n---\n# Vendor\n",
        )])
        .unwrap();
        let result = Batch(vec![Op::ClassifierSet {
            id: "vendor".into(),
            title: Some("Changed".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        }])
        .lower(context(&source));
        assert!(result.is_err());
        assert!(source.documents()[0].text().contains("title: Vendor"));
    }

    #[test]
    fn ordered_batch_validates_against_the_evolving_candidate() {
        let source = SourceBundle::default();
        let changed = Batch(vec![
            Op::ClassifierNew {
                slug: "invoice".into(),
                directory: DirectoryAddress::parse("/").unwrap(),
                ty: ElementType::parse("uml.Class"),
                title: "Invoice".into(),
                stereotype: vec![],
                description: None,
                abstract_: false,
            },
            Op::AttributeAdd {
                node: "invoice".into(),
                name: "id".into(),
                ty_token: "InvoiceId".into(),
                multiplicity: None,
                visibility: None,
            },
        ])
        .lower(context(&source))
        .unwrap();
        assert!(changed.documents()[0].text().contains("id: InvoiceId"));
    }

    #[test]
    fn relationship_add_rejects_unclaimed_target() {
        let source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("vendor.md", "---\ntype: vendor.Custom\n---\n# Vendor\n"),
        ])
        .unwrap();
        let result = Batch(vec![Op::RelationshipAdd {
            source: "order".into(),
            kind: RelationshipKind::Depends,
            target: "vendor".into(),
            name: None,
            ends: None,
        }])
        .lower(context(&source));
        assert!(result.is_err());
    }

    #[test]
    fn relationship_add_accepts_target_created_earlier_in_batch() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let changed = Batch(vec![
            Op::ClassifierNew {
                slug: "invoice".into(),
                directory: DirectoryAddress::parse("/").unwrap(),
                ty: ElementType::parse("uml.Class"),
                title: "Invoice".into(),
                stereotype: vec![],
                description: None,
                abstract_: false,
            },
            Op::RelationshipAdd {
                source: "order".into(),
                kind: RelationshipKind::Depends,
                target: "invoice".into(),
                name: None,
                ends: None,
            },
        ])
        .lower(context(&source))
        .unwrap();
        assert!(changed
            .document_by_concept_id("order")
            .unwrap()
            .text()
            .contains("invoice.md"));
    }

    #[test]
    fn every_uml_operation_round_trips_authored_source() {
        let class = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n";
        let class_with_attr =
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n";
        let class_with_attrs = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId\n- total: Money\n";
        let enum_one = "---\ntype: uml.Enum\ntitle: Status\n---\n# Status\n\n## Values\n- DRAFT\n";
        let enum_two =
            "---\ntype: uml.Enum\ntitle: Status\n---\n# Status\n\n## Values\n- DRAFT\n- PLACED\n";
        let customer = "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n";
        let relationship = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n";
        let diagram =
            "---\ntype: uml.ClassDiagram\ntitle: Domain\nprofile: uml-domain\n---\n# Domain\n";
        let placed = "---\ntype: uml.ClassDiagram\ntitle: Domain\nprofile: uml-domain\n---\n# Domain\n\n## Layout\n- [Order](./order.md) left of [Customer](./customer.md)\n";

        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", class)]).unwrap(),
            Batch(vec![Op::AttributeAdd {
                node: "order".into(),
                name: "total".into(),
                ty_token: "Money".into(),
                multiplicity: None,
                visibility: None,
            }]),
            |source| assert!(source.documents()[0].text().contains("- total: Money")),
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", class_with_attr)]).unwrap(),
            Batch(vec![Op::AttributeSet {
                node: "order".into(),
                name: "id".into(),
                ty_token: Some("String".into()),
                multiplicity: FieldEdit::Set(Multiplicity::parse("0..1").unwrap()),
                visibility: Some(Visibility::Private),
                rename: Some("customer_id".into()),
            }]),
            |source| {
                let text = source.documents()[0].text();
                assert!(text.contains("customer_id"));
                assert!(text.contains("String {0..1}"));
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", class_with_attrs)]).unwrap(),
            Batch(vec![Op::AttributeRemove {
                node: "order".into(),
                name: "total".into(),
            }]),
            |source| {
                assert!(!source.documents()[0].text().contains("total"));
                assert!(source.documents()[0].text().contains("- id: OrderId"));
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("status.md", enum_one)]).unwrap(),
            Batch(vec![Op::ValueAdd {
                node: "status".into(),
                literal: "PLACED".into(),
            }]),
            |source| assert!(source.documents()[0].text().contains("- PLACED")),
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("status.md", enum_two)]).unwrap(),
            Batch(vec![Op::ValueRemove {
                node: "status".into(),
                literal: "DRAFT".into(),
            }]),
            |source| {
                assert!(!source.documents()[0].text().contains("DRAFT"));
                assert!(source.documents()[0].text().contains("- PLACED"));
            },
        );

        let relationship_source = || {
            SourceBundle::try_from_pairs([("order.md", class), ("customer.md", customer)]).unwrap()
        };
        let selector = || RelationshipSelector {
            source: "order".into(),
            by: RelBy::Endpoint {
                kind: RelationshipKind::Depends,
                target: "customer".into(),
            },
        };
        assert_reversible(
            relationship_source(),
            Batch(vec![Op::RelationshipAdd {
                source: "order".into(),
                kind: RelationshipKind::Depends,
                target: "customer".into(),
                name: None,
                ends: None,
            }]),
            |source| {
                assert!(source.documents()[0]
                    .text()
                    .contains("- depends [Customer](./customer.md)"))
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", relationship), ("customer.md", customer)])
                .unwrap(),
            Batch(vec![Op::RelationshipSet {
                selector: selector(),
                ends: None,
                name: Some(NameSpec::Label("buyer".into())),
            }]),
            |source| assert!(source.documents()[0].text().contains("as \"buyer\"")),
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", relationship), ("customer.md", customer)])
                .unwrap(),
            Batch(vec![Op::RelationshipRemove {
                selector: selector(),
            }]),
            |source| assert!(!source.documents()[0].text().contains("depends")),
        );

        assert_reversible(
            SourceBundle::default(),
            Batch(vec![Op::ClassifierNew {
                slug: "invoice".into(),
                directory: DirectoryAddress::parse("/sales").unwrap(),
                ty: ElementType::parse("uml.Class"),
                title: "Invoice".into(),
                stereotype: vec!["entity".into()],
                description: Some("An invoice.".into()),
                abstract_: false,
            }]),
            |source| {
                assert_eq!(source.documents()[0].path().as_str(), "sales/invoice.md");
                assert!(source.documents()[0].text().contains("title: Invoice"));
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", class)]).unwrap(),
            Batch(vec![Op::ClassifierSet {
                id: "order".into(),
                title: Some("Sales Order".into()),
                description: Some("Placed by a customer.".into()),
                stereotype: Some(vec!["aggregateRoot".into()]),
                abstract_: Some(true),
                ty: None,
            }]),
            |source| {
                let text = source.documents()[0].text();
                assert!(text.contains("title: Sales Order"));
                assert!(text.contains("# Sales Order"));
                assert!(text.contains("abstract: true"));
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("order.md", class)]).unwrap(),
            Batch(vec![Op::ClassifierRemove {
                id: "order".into(),
                cascade: false,
            }]),
            |source| assert!(source.is_empty()),
        );
        assert_reversible(
            SourceBundle::try_from_pairs([
                ("order.md", class),
                (
                    "customer.md",
                    "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n\n## Relationships\n- depends [Order](./order.md)\n",
                ),
            ])
            .unwrap(),
            Batch(vec![Op::ClassifierRename {
                from: "order".into(),
                to: "purchase-order".into(),
            }]),
            |source| {
                assert!(source
                    .documents()
                    .iter()
                    .any(|doc| doc.path().as_str() == "purchase-order.md"));
                assert!(source
                    .documents()
                    .iter()
                    .any(|doc| doc.text().contains("./purchase-order.md")));
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("dia.md", diagram)]).unwrap(),
            Batch(vec![Op::DiagramSet {
                key: "dia".into(),
                title: Some("Order lifecycle".into()),
                description: Some("Notes for reviewers".into()),
                clear_description: false,
                display: None,
            }]),
            |source| {
                let text = source.documents()[0].text();
                assert!(text.contains("title: Order lifecycle"));
                assert!(text.contains("# Order lifecycle"));
                assert!(text.contains("description: Notes for reviewers"));
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("dia.md", diagram)]).unwrap(),
            Batch(vec![Op::PlacementSet {
                diagram: "dia".into(),
                subject_title: "Order".into(),
                subject_slug: "order".into(),
                reference_title: "Customer".into(),
                reference_slug: "customer".into(),
                directions: vec![Direction::LeftOf],
            }]),
            |source| {
                assert!(source.documents()[0]
                    .text()
                    .contains("- [Order](./order.md) left of [Customer](./customer.md)"))
            },
        );
        assert_reversible(
            SourceBundle::try_from_pairs([("dia.md", placed)]).unwrap(),
            Batch(vec![Op::PlacementRemove {
                diagram: "dia".into(),
                subject_slug: "order".into(),
                reference_slug: "customer".into(),
            }]),
            |source| assert!(!source.documents()[0].text().contains("left of")),
        );
    }

    #[test]
    fn evolving_multi_step_uml_batch_round_trips_as_one_transaction() {
        assert_reversible(
            SourceBundle::default(),
            Batch(vec![
                Op::ClassifierNew {
                    slug: "invoice".into(),
                    directory: DirectoryAddress::parse("/").unwrap(),
                    ty: ElementType::parse("uml.Class"),
                    title: "Invoice".into(),
                    stereotype: vec![],
                    description: None,
                    abstract_: false,
                },
                Op::AttributeAdd {
                    node: "invoice".into(),
                    name: "id".into(),
                    ty_token: "InvoiceId".into(),
                    multiplicity: None,
                    visibility: None,
                },
            ]),
            |source| {
                assert_eq!(source.len(), 1);
                assert!(source.documents()[0].text().contains("- id: InvoiceId"));
            },
        );
    }
}
