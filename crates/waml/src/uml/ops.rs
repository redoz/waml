use super::{DiagramDisplaySet, FieldEdit, NameSpec, Selector};
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
        selector: Selector,
        ends: Option<(RelEnd, RelEnd)>,
        name: Option<NameSpec>,
    },
    RelationshipRemove {
        selector: Selector,
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
        let mut candidate = context.source.clone();
        for (index, op) in self.0.iter().enumerate() {
            lower_one(&mut candidate, op).map_err(|mut error| {
                error.index = index;
                error
            })?;
        }
        Ok(candidate)
    }
}

fn require_claimed(work: &SourceBundle, target: &str, op: &str) -> Result<(), EditError> {
    let index = crate::ops::resolve_index(work, target)
        .ok_or_else(|| EditError::at(op, format!("no document '{target}'")))?;
    let document = work.document_at(index).expect("resolved document index");
    let parsed = crate::parse::parse_document(document.text());
    let ty = ElementType::parse(parsed.frontmatter.get_str("type").unwrap_or(""));
    if crate::uml::recognizes_type(&ty) {
        Ok(())
    } else {
        Err(EditError::at(
            op,
            format!("'{target}' is not claimed by the UML projection"),
        ))
    }
}

pub(crate) fn lower_one(work: &mut SourceBundle, op: &Op) -> Result<(), EditError> {
    match op {
        Op::AttributeAdd {
            node,
            name,
            ty_token,
            multiplicity,
            visibility,
        } => {
            require_claimed(work, node, "attr.add")?;
            crate::ops::op_attr_add(work, node, name, ty_token, multiplicity, *visibility)
        }
        Op::AttributeSet {
            node,
            name,
            ty_token,
            multiplicity,
            visibility,
            rename,
        } => {
            require_claimed(work, node, "attr.set")?;
            crate::ops::op_attr_set(
                work,
                node,
                name,
                ty_token,
                multiplicity,
                *visibility,
                rename,
            )
        }
        Op::AttributeRemove { node, name } => {
            require_claimed(work, node, "attr.rm")?;
            crate::ops::op_attr_rm(work, node, name)
        }
        Op::ValueAdd { node, literal } => {
            require_claimed(work, node, "value.add")?;
            crate::ops::op_value_add(work, node, literal)
        }
        Op::ValueRemove { node, literal } => {
            require_claimed(work, node, "value.rm")?;
            crate::ops::op_value_rm(work, node, literal)
        }
        Op::RelationshipAdd {
            source,
            kind,
            target,
            name,
            ends,
        } => {
            require_claimed(work, source, "rel.add")?;
            crate::ops::op_rel_add(work, source, *kind, target, name, ends)
        }
        Op::RelationshipSet {
            selector,
            ends,
            name,
        } => crate::ops::op_rel_set(work, selector, ends, name),
        Op::RelationshipRemove { selector } => crate::ops::op_rel_rm(work, selector),
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
            crate::ops::op_node_new(
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
            require_claimed(work, id, "node.set")?;
            crate::ops::op_node_set(work, id, title, description, stereotype, abstract_, ty)
        }
        Op::ClassifierRemove { id, cascade } => {
            require_claimed(work, id, "node.rm")?;
            crate::ops::op_node_rm(work, id, *cascade)
        }
        Op::ClassifierRename { from, to } => {
            require_claimed(work, from, "node.rename")?;
            crate::ops::rename::op_node_rename(work, from, to)
        }
        Op::DiagramSet {
            key,
            title,
            description,
            clear_description,
            display,
        } => {
            require_claimed(work, key, "diagram.set")?;
            crate::ops::op_diagram_set(work, key, title, description, *clear_description, display)
        }
        Op::PlacementSet {
            diagram,
            subject_title,
            subject_slug,
            reference_title,
            reference_slug,
            directions,
        } => {
            require_claimed(work, diagram, "place.set")?;
            crate::ops::op_place_set(
                work,
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
            require_claimed(work, diagram, "place.rm")?;
            crate::ops::op_place_rm(work, diagram, subject_slug, reference_slug)
        }
    }
}

impl From<Op> for crate::ops::Op {
    fn from(op: Op) -> Self {
        match op {
            Op::AttributeAdd {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
            } => Self::AttrAdd {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
            },
            Op::AttributeSet {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
                rename,
            } => Self::AttrSet {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
                rename,
            },
            Op::AttributeRemove { node, name } => Self::AttrRm { node, name },
            Op::ValueAdd { node, literal } => Self::ValueAdd { node, literal },
            Op::ValueRemove { node, literal } => Self::ValueRm { node, literal },
            Op::RelationshipAdd {
                source,
                kind,
                target,
                name,
                ends,
            } => Self::RelAdd {
                source,
                kind,
                target,
                name,
                ends,
            },
            Op::RelationshipSet {
                selector,
                ends,
                name,
            } => Self::RelSet {
                selector,
                ends,
                name,
            },
            Op::RelationshipRemove { selector } => Self::RelRm { selector },
            Op::ClassifierNew {
                slug,
                directory,
                ty,
                title,
                stereotype,
                description,
                abstract_,
            } => Self::NodeNew {
                slug,
                dir: crate::okf::ops::legacy_path(&directory),
                ty,
                title,
                stereotype,
                description,
                abstract_,
            },
            Op::ClassifierSet {
                id,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            } => Self::NodeSet {
                slug: id,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            },
            Op::ClassifierRemove { id, cascade } => Self::NodeRm { slug: id, cascade },
            Op::ClassifierRename { from, to } => Self::NodeRename { from, to },
            Op::DiagramSet {
                key,
                title,
                description,
                clear_description,
                display,
            } => Self::DiagramSet {
                key,
                title,
                description,
                clear_description,
                display,
            },
            Op::PlacementSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            } => Self::PlaceSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            },
            Op::PlacementRemove {
                diagram,
                subject_slug,
                reference_slug,
            } => Self::PlaceRm {
                diagram,
                subject_slug,
                reference_slug,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordered_batch_is_atomic_and_copy_on_write() {
        let source = SourceBundle::try_from_pairs([
            ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
            ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
        ])
        .unwrap();
        let okf = crate::okf::Bundle::parse(&source).unwrap();
        let uml = crate::uml::project(&okf);
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
        assert!(batch
            .lower(EditContext {
                source: &source,
                okf: &okf,
                uml: &uml
            })
            .is_err());
        assert!(source.documents()[0].text().contains("title: A"));

        let changed = Batch(vec![Op::ClassifierSet {
            id: "a".into(),
            title: Some("Changed".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        }])
        .lower(EditContext {
            source: &source,
            okf: &okf,
            uml: &uml,
        })
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
        let okf = crate::okf::Bundle::parse(&source).unwrap();
        let uml = crate::uml::project(&okf);
        let result = Batch(vec![Op::ClassifierSet {
            id: "vendor".into(),
            title: Some("Changed".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        }])
        .lower(EditContext {
            source: &source,
            okf: &okf,
            uml: &uml,
        });
        assert!(result.is_err());
        assert!(source.documents()[0].text().contains("title: Vendor"));
    }

    #[test]
    fn ordered_batch_validates_against_the_evolving_candidate() {
        let source = SourceBundle::default();
        let okf = crate::okf::Bundle::parse(&source).unwrap();
        let uml = crate::uml::project(&okf);
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
        .lower(EditContext {
            source: &source,
            okf: &okf,
            uml: &uml,
        })
        .unwrap();
        assert!(changed.documents()[0].text().contains("id: InvoiceId"));
    }
}
