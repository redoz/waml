use super::syntax::{
    AttributeSyntax, InlineInstanceSyntax, MemberSyntax, RelationshipSyntax, SlotSyntax,
    UmlLanguage, ValueSyntax,
};
use crate::{
    model::{TypeRef, Visibility},
    multiplicity::Multiplicity,
};
use std::{collections::BTreeMap, sync::Arc};
use waml_syntax::{SyntaxLanguage, SyntaxNode};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpectedSyntax {
    ColonToken,
    TypeReference,
    ValidMultiplicity,
    LinkTarget,
    RelationshipTarget,
    LayoutOperand,
    FlowTarget,
    MessageTarget,
}
#[derive(Clone)]
pub enum DeclaredField<L: SyntaxLanguage, T> {
    Absent,
    Valid {
        value: T,
        syntax: SyntaxNode<L>,
    },
    Incomplete {
        syntax: SyntaxNode<L>,
        expected: ExpectedSyntax,
    },
    Invalid {
        syntax: SyntaxNode<L>,
        diagnostics: Arc<[crate::diagnostic::DiagCode]>,
    },
}
pub struct DeclaredAttribute {
    pub syntax: AttributeSyntax,
    pub visibility: DeclaredField<UmlLanguage, Visibility>,
    pub name: DeclaredField<UmlLanguage, String>,
    pub ty: DeclaredField<UmlLanguage, TypeRef>,
    pub multiplicity: DeclaredField<UmlLanguage, Multiplicity>,
}
pub struct DeclaredValue {
    pub syntax: ValueSyntax,
    pub value: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredSlot {
    pub syntax: SlotSyntax,
    pub name: DeclaredField<UmlLanguage, String>,
    pub value: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredRelationship {
    pub syntax: RelationshipSyntax,
    pub kind: DeclaredField<UmlLanguage, crate::model::RelationshipKind>,
    pub target: DeclaredField<UmlLanguage, String>,
    pub name: DeclaredField<UmlLanguage, crate::model::AssocName>,
    pub from_end: DeclaredField<UmlLanguage, crate::model::RelEnd>,
    pub to_end: DeclaredField<UmlLanguage, crate::model::RelEnd>,
}
pub struct DeclaredMember {
    pub syntax: MemberSyntax,
    pub target: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredMemberGroup {
    pub syntax: super::syntax::MemberGroupSyntax,
    pub name: DeclaredField<UmlLanguage, String>,
    pub members: Arc<[DeclaredMember]>,
    pub inline_instances: Arc<[DeclaredInlineInstance]>,
    pub children: Arc<[DeclaredMemberGroup]>,
}
pub struct DeclaredInlineInstance {
    pub syntax: InlineInstanceSyntax,
    pub classifier: DeclaredField<UmlLanguage, String>,
    pub name: DeclaredField<UmlLanguage, String>,
    pub slots: Arc<[DeclaredSlot]>,
}
pub struct DeclaredFlowTransition {
    pub syntax: super::syntax::FlowTransitionSyntax,
    pub trigger: DeclaredField<UmlLanguage, String>,
    pub guard: DeclaredField<UmlLanguage, String>,
    pub is_else: bool,
    pub target: DeclaredField<UmlLanguage, crate::syntax::FlowTargetRef>,
    pub carries: DeclaredField<UmlLanguage, String>,
    pub effect: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredFlowNode {
    pub syntax: super::syntax::FlowNodeSyntax,
    pub kind: DeclaredField<UmlLanguage, crate::model::FlowNodeKind>,
    pub identity: DeclaredField<UmlLanguage, String>,
    pub object_ref: DeclaredField<UmlLanguage, String>,
    pub entry: DeclaredField<UmlLanguage, String>,
    pub do_: DeclaredField<UmlLanguage, String>,
    pub exit: DeclaredField<UmlLanguage, String>,
    pub refines: DeclaredField<UmlLanguage, String>,
    pub partition: DeclaredField<UmlLanguage, String>,
    pub notes: Arc<[DeclaredField<UmlLanguage, String>]>,
    pub transitions: Arc<[DeclaredFlowTransition]>,
}
pub struct DeclaredLifeline {
    pub syntax: super::syntax::LifelineSyntax,
    pub target: DeclaredField<UmlLanguage, String>,
    pub title: DeclaredField<UmlLanguage, String>,
    pub alias: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredMessage {
    pub syntax: super::syntax::MessageSyntax,
    pub from: DeclaredField<UmlLanguage, String>,
    pub verb: DeclaredField<UmlLanguage, crate::model::MessageVerb>,
    pub to: DeclaredField<UmlLanguage, String>,
    pub signature: DeclaredField<UmlLanguage, String>,
    pub depth: usize,
}
pub struct DeclaredSequenceOperand {
    pub syntax: super::syntax::SequenceOperandSyntax,
    pub fragment: DeclaredField<UmlLanguage, crate::model::FragmentKind>,
    pub guard: DeclaredField<UmlLanguage, String>,
    pub is_else: bool,
    pub depth: usize,
}
pub enum DeclaredLayoutStatement {
    Placement {
        operands: Arc<[DeclaredField<UmlLanguage, crate::syntax::Operand>]>,
        directions: Arc<[DeclaredField<UmlLanguage, crate::syntax::Direction>]>,
    },
    Alignment {
        left: DeclaredField<UmlLanguage, crate::syntax::Anchored>,
        right: DeclaredField<UmlLanguage, crate::syntax::Anchored>,
    },
    Standalone(DeclaredField<UmlLanguage, crate::syntax::Operand>),
}
pub struct DeclaredConcept {
    pub concept_id: String,
    pub attributes: Arc<[DeclaredAttribute]>,
    pub values: Arc<[DeclaredValue]>,
    pub slots: Arc<[DeclaredSlot]>,
    pub relationships: Arc<[DeclaredRelationship]>,
    pub members: Arc<[DeclaredMember]>,
    pub member_groups: Arc<[DeclaredMemberGroup]>,
    pub inline_instances: Arc<[DeclaredInlineInstance]>,
    pub layout: Arc<[DeclaredField<UmlLanguage, DeclaredLayoutStatement>]>,
    pub flow_nodes: Arc<[DeclaredFlowNode]>,
    pub lifelines: Arc<[DeclaredLifeline]>,
    pub messages: Arc<[DeclaredMessage]>,
    pub sequence_operands: Arc<[DeclaredSequenceOperand]>,
}
#[derive(Default)]
pub struct DeclaredBundle {
    pub(crate) concepts: BTreeMap<String, DeclaredConcept>,
}
impl DeclaredBundle {
    pub fn concept(&self, id: &str) -> Option<&DeclaredConcept> {
        self.concepts.get(id)
    }
    pub fn concepts(&self) -> impl Iterator<Item = &DeclaredConcept> {
        self.concepts.values()
    }
}

impl DeclaredAttribute {
    /// Produce the semantic attribute only when every required authored slot is
    /// valid and each optional slot is absent or valid.
    pub(crate) fn validated(&self) -> Option<crate::model::Attribute> {
        if self.syntax.colon_token().flags().is_missing() {
            return None;
        }
        let DeclaredField::Valid { value: name, .. } = &self.name else {
            return None;
        };
        let DeclaredField::Valid { value: ty, .. } = &self.ty else {
            return None;
        };
        let visibility = match &self.visibility {
            DeclaredField::Absent => None,
            DeclaredField::Valid { value, .. } => Some(*value),
            DeclaredField::Incomplete { .. } | DeclaredField::Invalid { .. } => return None,
        };
        let multiplicity = match &self.multiplicity {
            DeclaredField::Absent => None,
            DeclaredField::Valid { value, .. } => Some(value.clone()),
            DeclaredField::Incomplete { .. } | DeclaredField::Invalid { .. } => return None,
        };
        Some(crate::model::Attribute {
            name: name.clone(),
            ty: ty.clone(),
            multiplicity,
            visibility,
            description: None,
        })
    }
}
