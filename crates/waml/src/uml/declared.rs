use super::syntax::{
    AttributeSyntax, BindingSyntax, FlowTraceSyntax, GateSyntax, InlineInstanceSyntax,
    InteractionUseSyntax, MemberSyntax, MessageSyntax, RelationshipSyntax, SequenceFragmentSyntax,
    SequenceOperandSyntax, SlotSyntax, UmlLanguage, ValueSyntax,
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
    /// A handle the author invents: a lifeline or interaction-use alias.
    Alias,
    /// The name of an inline instance.
    InstanceName,
    /// The name half of a `## Slots` entry.
    SlotName,
    /// The value half of a `## Slots` entry.
    SlotValue,
    /// A message call id, declared after `as` or referenced after `for`.
    CallId,
}

/// The one table mapping a typed slot to what the grammar expects in it.
/// The analysis reads it to label an `Incomplete` field and `uml::complete`
/// reads the same slot kinds to pick a candidate provider -- neither reads the
/// other. An unlisted slot falls back to `MessageTarget`, which is what every
/// unlabelled site reported before this table existed, so adding a slot kind
/// can never turn an unfinished document into a panic.
pub fn expected_for_slot(slot: super::syntax::UmlSyntaxKind) -> ExpectedSyntax {
    use super::syntax::UmlSyntaxKind as K;
    match slot {
        K::LifelineAlias | K::InteractionUseAlias => ExpectedSyntax::Alias,
        K::MessageCallId | K::MessageReturnCall => ExpectedSyntax::CallId,
        K::InlineInstance => ExpectedSyntax::InstanceName,
        K::Slot => ExpectedSyntax::SlotName,
        K::FlowTarget => ExpectedSyntax::FlowTarget,
        K::TypeReference => ExpectedSyntax::TypeReference,
        K::Multiplicity => ExpectedSyntax::ValidMultiplicity,
        K::Link => ExpectedSyntax::LinkTarget,
        _ => ExpectedSyntax::MessageTarget,
    }
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
    pub target: DeclaredField<UmlLanguage, crate::layout::FlowTargetRef>,
    pub carries: DeclaredField<UmlLanguage, String>,
    pub effect: DeclaredField<UmlLanguage, String>,
    pub traces: Arc<[DeclaredFlowTrace]>,
}

pub struct DeclaredFlowTrace {
    pub syntax: FlowTraceSyntax,
    pub label: DeclaredField<UmlLanguage, String>,
    pub href: DeclaredField<UmlLanguage, String>,
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
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredEndpointRef {
    Lifeline(String),
    Outside,
    LocalGate(String),
    UseGate { use_alias: String, gate: String },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclaredMessageKind {
    SyncCall,
    AsyncCall,
    AsyncSignal,
    Reply,
    Create,
    Delete,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclaredOperandSpec {
    Guard(String),
    Else,
    Branch { label: Option<String> },
}
pub struct DeclaredMessage {
    pub syntax: MessageSyntax,
    pub source: DeclaredField<UmlLanguage, DeclaredEndpointRef>,
    pub kind: DeclaredField<UmlLanguage, DeclaredMessageKind>,
    pub target: DeclaredField<UmlLanguage, DeclaredEndpointRef>,
    pub value: DeclaredField<UmlLanguage, String>,
    pub call_id: DeclaredField<UmlLanguage, String>,
    pub return_to: DeclaredField<UmlLanguage, DeclaredEndpointRef>,
    pub return_for: DeclaredField<UmlLanguage, String>,
    pub depth: usize,
}
pub struct DeclaredGate {
    pub syntax: GateSyntax,
    pub name: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredFragment {
    pub syntax: SequenceFragmentSyntax,
    pub kind: DeclaredField<UmlLanguage, crate::model::FragmentKind>,
    pub depth: usize,
}
pub struct DeclaredOperand {
    pub syntax: SequenceOperandSyntax,
    pub spec: DeclaredField<UmlLanguage, DeclaredOperandSpec>,
    pub depth: usize,
}
pub struct DeclaredBinding {
    pub syntax: BindingSyntax,
    pub local: DeclaredField<UmlLanguage, String>,
    pub target: DeclaredField<UmlLanguage, String>,
}
pub struct DeclaredInteractionUse {
    pub syntax: InteractionUseSyntax,
    pub link: DeclaredField<UmlLanguage, String>,
    pub alias: DeclaredField<UmlLanguage, String>,
    pub bindings: Arc<[DeclaredBinding]>,
    pub depth: usize,
}
pub enum DeclaredLayoutStatement {
    Placement {
        operands: Arc<[DeclaredField<UmlLanguage, crate::layout::Operand>]>,
        directions: Arc<[DeclaredField<UmlLanguage, crate::layout::Direction>]>,
    },
    Alignment {
        left: DeclaredField<UmlLanguage, crate::layout::Anchored>,
        right: DeclaredField<UmlLanguage, crate::layout::Anchored>,
    },
    Standalone(DeclaredField<UmlLanguage, crate::layout::Operand>),
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
    pub gates: Arc<[DeclaredGate]>,
    pub messages: Arc<[DeclaredMessage]>,
    pub fragments: Arc<[DeclaredFragment]>,
    pub operands: Arc<[DeclaredOperand]>,
    pub interaction_uses: Arc<[DeclaredInteractionUse]>,
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
