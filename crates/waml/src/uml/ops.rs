use super::{DiagramDisplaySet, FieldEdit, NameSpec, RelationshipSelector, Selector};
use crate::edit::{EditBatch, EditContext, EditError};
use crate::model::{ElementType, RelEnd, RelationshipKind, Visibility};
use crate::multiplicity::Multiplicity;
use crate::okf::DirectoryAddress;
use crate::source::SourceBundle;
use crate::syntax::Direction;

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
            super::rename::op_node_rename(work, from, to)
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(source: &SourceBundle) -> EditContext<'_> {
        let okf_analysis = Box::leak(Box::new(
            crate::analysis::analyze_okf(source, None, 0).unwrap(),
        ));
        let uml = Box::leak(Box::new(
            crate::uml::analyze(
                crate::analysis::DomainAnalysisContext {
                    source,
                    catalog: &okf_analysis.catalog,
                    shell: &okf_analysis.shell,
                    structures: &okf_analysis.structures,
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
        assert!(batch.lower(context(&source)).is_err());
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
}
