//! Selective UML projection over the domain-agnostic OKF bundle.

pub mod analysis;
pub mod declared;
mod format;
pub(crate) mod lower;
pub mod ops;
pub(crate) mod rename;
mod repair;
pub mod selector;
pub mod syntax;
pub use crate::ops::{DiagramDisplaySet, FieldEdit, NameSpec};
pub use format::{ActionContext, FormatError, Formatter};
pub use ops::{Batch, Op};
pub use repair::repair_actions;
pub use selector::{RelationshipSelector, Selector};

pub type Projection = crate::model::Model;
pub use analysis::{analyze, Analysis};
pub use declared::{
    DeclaredAttribute, DeclaredBundle, DeclaredConcept, DeclaredField, DeclaredFlowNode,
    DeclaredFlowTransition, DeclaredInlineInstance, DeclaredLayoutStatement, DeclaredLifeline,
    DeclaredMember, DeclaredMemberGroup, DeclaredMessage, DeclaredRelationship,
    DeclaredSequenceOperand, DeclaredSlot, DeclaredValue, ExpectedSyntax,
};
pub use syntax::{
    AnchoredSyntax, AxisSyntax, DiagramMembersSyntax, DirectionClauseSyntax, EdgeSyntax,
    FlagSyntax, FlowBlockSyntax, FlowInternalSyntax, FlowNodeSyntax, FlowTransitionSyntax,
    HintClauseSyntax, HintSyntax, InlineInstanceSyntax, LayoutAlignmentSyntax, LayoutAtomSyntax,
    LayoutPlacementSyntax, LayoutSectionSyntax, LayoutStandaloneSyntax, LayoutStatementSyntax,
    LifelineSyntax, MarginSyntax, MemberGroupSyntax, MemberSyntax, MessageSyntax,
    MessagesBlockSyntax, NameRefSyntax, OperandRefSyntax, OperandSyntax, RelationshipEndSyntax,
    RelationshipSyntax, SequenceFragmentSyntax, SequenceOperandSyntax, ShapeSyntax, SlotSyntax,
    SlotValueKind, ValueSyntax,
};

pub(crate) fn recognizes_type(element_type: &crate::model::ElementType) -> bool {
    matches!(
        element_type,
        crate::model::ElementType::Uml(_)
            | crate::model::ElementType::Behavior(_)
            | crate::model::ElementType::Diagram
    )
}

pub fn recognizes(concept: &crate::okf::Concept) -> bool {
    recognizes_type(&crate::model::ElementType::parse(&concept.ty))
}

pub fn project(bundle: &crate::okf::Bundle) -> Projection {
    use crate::frontmatter::{FmValue, Frontmatter};

    let documents = bundle.concepts().iter().map(|concept| {
        let mut frontmatter = concept.extra.clone();
        let mut known = vec![("type".to_owned(), FmValue::Str(concept.ty.clone()))];
        for (key, value) in [
            ("title", concept.title.as_ref()),
            ("description", concept.description.as_ref()),
            ("resource", concept.resource.as_ref()),
            ("timestamp", concept.timestamp.as_ref()),
        ] {
            if let Some(value) = value {
                known.push((key.to_owned(), FmValue::Str(value.clone())));
            }
        }
        if !concept.tags.is_empty() {
            known.push((
                "tags".to_owned(),
                FmValue::List(concept.tags.iter().cloned().map(FmValue::Str).collect()),
            ));
        }
        known.append(&mut frontmatter.entries);
        let rendered = crate::frontmatter::render_frontmatter(&Frontmatter { entries: known });
        (
            format!("{}.md", concept.id),
            format!("---\n{rendered}\n---\n{}", concept.body.as_ref()),
        )
    });
    let source = crate::source::SourceBundle::try_from_pairs(documents)
        .expect("OKF concept IDs form valid unique document paths");
    crate::analysis::prepare_candidate(source, None, 0)
        .expect("an existing OKF bundle remains analyzable")
        .uml()
        .projection
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceBundle;

    fn concept(ty: &str) -> crate::okf::Concept {
        crate::okf::project("x.md", &format!("---\ntype: {ty}\n---\n# X\n"))
    }

    #[test]
    fn recognizer_claims_only_supported_uml_behavior_and_diagram_types() {
        for ty in [
            "uml.Class",
            "uml.Package",
            "uml.Activity",
            "uml.StateMachine",
            "uml.Sequence",
            "Diagram",
        ] {
            assert!(recognizes(&concept(ty)), "{ty}");
        }
        for ty in ["", "Playbook", "bpmn.Task", "uml.FutureThing"] {
            assert!(!recognizes(&concept(ty)), "{ty}");
        }
    }

    #[test]
    fn projection_keeps_unclaimed_concepts_in_okf_only() {
        let source = SourceBundle::try_from_pairs([
            ("plain.md", "# Plain\n"),
            ("vendor.md", "---\ntype: vendor.Custom\n---\n# Vendor\n"),
            ("future.md", "---\ntype: uml.FutureThing\n---\n# Future\n"),
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ])
        .unwrap();
        let bundle = crate::okf::Bundle::parse(&source).unwrap();

        let projection = project(&bundle);

        assert_eq!(
            projection
                .nodes
                .iter()
                .map(|node| node.key.as_str())
                .collect::<Vec<_>>(),
            ["order"]
        );
        assert!(projection.diagrams.is_empty());
        assert!(projection.packages.is_empty());
        assert!(projection.path.is_empty());
        assert!(projection.contains_concept("order"));
        assert!(!projection.contains_concept("plain"));
        assert!(bundle.concept("plain").is_some());
        assert!(bundle.concept("vendor").is_some());
        assert!(bundle.concept("future").is_some());
    }

    #[test]
    fn relationships_and_diagram_members_resolve_only_claimed_concepts() {
        let source = SourceBundle::try_from_pairs([
            (
                "order.md",
                "---\ntype: uml.Class\n---\n# Order\n\n## Relationships\n- depends [Customer](./customer.md)\n- depends [Vendor](./vendor.md)\n- depends [Future](./future.md)\n",
            ),
            (
                "customer.md",
                "---\ntype: uml.Class\n---\n# Customer\n",
            ),
            ("vendor.md", "---\ntype: vendor.Custom\n---\n# Vendor\n"),
            (
                "future.md",
                "---\ntype: uml.FutureThing\n---\n# Future\n",
            ),
            (
                "diagram.md",
                "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [Order](./order.md)\n- [Customer](./customer.md)\n- [Vendor](./vendor.md)\n- [Future](./future.md)\n",
            ),
        ])
        .unwrap();
        let bundle = crate::okf::Bundle::parse(&source).unwrap();

        let projection = project(&bundle);

        assert_eq!(projection.edges.len(), 1);
        assert_eq!(projection.edges[0].target, "customer");
        assert_eq!(
            projection.diagrams[0].groups[0].members,
            ["order", "customer"]
        );
        assert!(projection.node("vendor").is_none());
        assert!(projection.node("future").is_none());
    }
}
