use super::{UmlLanguage, UmlSyntaxKind};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug)]
pub struct AttributeSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct TypeReferenceSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MultiplicitySyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct ValueSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct SlotSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct RelationshipSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct RelationshipEndSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MemberSyntax(pub(crate) SyntaxNode<UmlLanguage>);
/// A single bullet in a confirmed diagram `Members` section.  This is named
/// separately from the older semantic `Member` model so parser consumers can
/// express that they are operating on the lossless line syntax.
pub type MemberLineSyntax = MemberSyntax;
#[derive(Clone, Debug)]
pub struct MemberGroupSyntax(pub(crate) SyntaxNode<UmlLanguage>);
/// The confirmed `## Members` island.  Its direct children are the ordered
/// `MemberGroupSyntax` forest; it is deliberately distinct from arbitrary
/// Markdown headed "Members" outside the shell-confirmed section map.
#[derive(Clone, Debug)]
pub struct DiagramMembersSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct InlineInstanceSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct LayoutSectionSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct LayoutStatementSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct LayoutPlacementSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct LayoutAlignmentSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct LayoutStandaloneSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct OperandSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct AnchoredSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct DirectionClauseSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct HintClauseSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct EdgeSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct AxisSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct OperandRefSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct NameRefSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct HintSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct ShapeSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MarginSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct FlagSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct FlowNodeSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct FlowTransitionSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct FlowBlockSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct FlowInternalSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct LifelineSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MessageSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct SequenceOperandSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct SequenceFragmentSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MessagesBlockSyntax(pub(crate) SyntaxNode<UmlLanguage>);
/// A typed, lossless leaf view over an authoritative fixed-slot layout tree.
/// Consumers may inspect source order and ranges through this compatibility
/// API; semantic lowering reads the typed nodes directly.
#[derive(Clone, Debug)]
pub enum LayoutAtomSyntax {
    Word(SyntaxToken<UmlLanguage>),
    Link(SyntaxToken<UmlLanguage>),
    Quote(SyntaxToken<UmlLanguage>),
    OpenParen(SyntaxToken<UmlLanguage>),
    CloseParen(SyntaxToken<UmlLanguage>),
    Comma(SyntaxToken<UmlLanguage>),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotValueKind {
    Bare,
    Quoted,
    Link,
    Missing,
    Invalid,
}
impl AttributeSyntax {
    pub fn visibility_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.token(UmlSyntaxKind::VisibilityToken)
    }
    pub fn name_token(&self) -> SyntaxToken<UmlLanguage> {
        self.token(UmlSyntaxKind::IdentifierToken)
            .expect("attribute has fixed name slot")
    }
    pub fn colon_token(&self) -> SyntaxToken<UmlLanguage> {
        self.token(UmlSyntaxKind::ColonToken)
            .expect("attribute has fixed colon slot")
    }
    pub fn type_syntax(&self) -> Option<TypeReferenceSyntax> {
        self.node(UmlSyntaxKind::TypeReference)
            .map(TypeReferenceSyntax)
    }
    pub fn multiplicity(&self) -> Option<MultiplicitySyntax> {
        self.node(UmlSyntaxKind::Multiplicity)
            .map(MultiplicitySyntax)
    }
    pub fn recovery(&self) -> impl Iterator<Item = SyntaxElement<UmlLanguage>> + '_ {
        self.0.children().filter(|e| {
            matches!(
                e.kind(),
                UmlSyntaxKind::SkippedTokensSyntax | UmlSyntaxKind::BadToken
            )
        })
    }
    fn token(&self, kind: UmlSyntaxKind) -> Option<SyntaxToken<UmlLanguage>> {
        self.0
            .children()
            .find(|e| e.kind() == kind)
            .and_then(SyntaxElement::into_token)
    }
    fn node(&self, kind: UmlSyntaxKind) -> Option<SyntaxNode<UmlLanguage>> {
        self.0
            .children()
            .find(|e| e.kind() == kind)
            .and_then(SyntaxElement::into_node)
    }
}
impl TypeReferenceSyntax {
    pub fn type_token(&self) -> SyntaxToken<UmlLanguage> {
        self.0
            .children()
            .find(|element| element.kind() == UmlSyntaxKind::TypeToken)
            .and_then(SyntaxElement::into_token)
            .expect("type reference has fixed type token")
    }
    pub fn link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        self.0
            .children()
            .find(|element| element.kind() == UmlSyntaxKind::Link)
            .and_then(SyntaxElement::into_node)
    }
    pub fn link_text_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.link()?
            .children()
            .find(|element| element.kind() == UmlSyntaxKind::LinkTextToken)
            .and_then(SyntaxElement::into_token)
    }
    pub fn link_target_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.link()?
            .children()
            .find(|element| element.kind() == UmlSyntaxKind::LinkTargetToken)
            .and_then(SyntaxElement::into_token)
    }
}
impl MultiplicitySyntax {
    pub fn open_token(&self) -> SyntaxToken<UmlLanguage> {
        self.token(UmlSyntaxKind::OpenBracketToken)
    }
    pub fn value_token(&self) -> SyntaxToken<UmlLanguage> {
        self.token(UmlSyntaxKind::IdentifierToken)
    }
    pub fn close_token(&self) -> SyntaxToken<UmlLanguage> {
        self.token(UmlSyntaxKind::CloseBracketToken)
    }
    fn token(&self, kind: UmlSyntaxKind) -> SyntaxToken<UmlLanguage> {
        self.0
            .children()
            .find(|element| element.kind() == kind)
            .and_then(SyntaxElement::into_token)
            .expect("multiplicity has fixed token slots")
    }
}
impl AstNode<UmlLanguage> for AttributeSyntax {
    fn can_cast(k: UmlSyntaxKind) -> bool {
        k == UmlSyntaxKind::Attribute
    }
    fn cast(n: SyntaxNode<UmlLanguage>) -> Option<Self> {
        Self::can_cast(n.kind()).then_some(Self(n))
    }
    fn syntax(&self) -> &SyntaxNode<UmlLanguage> {
        &self.0
    }
}
impl AstNode<UmlLanguage> for TypeReferenceSyntax {
    fn can_cast(kind: UmlSyntaxKind) -> bool {
        kind == UmlSyntaxKind::TypeReference
    }
    fn cast(node: SyntaxNode<UmlLanguage>) -> Option<Self> {
        Self::can_cast(node.kind()).then_some(Self(node))
    }
    fn syntax(&self) -> &SyntaxNode<UmlLanguage> {
        &self.0
    }
}
impl AstNode<UmlLanguage> for MultiplicitySyntax {
    fn can_cast(kind: UmlSyntaxKind) -> bool {
        kind == UmlSyntaxKind::Multiplicity
    }
    fn cast(node: SyntaxNode<UmlLanguage>) -> Option<Self> {
        Self::can_cast(node.kind()).then_some(Self(node))
    }
    fn syntax(&self) -> &SyntaxNode<UmlLanguage> {
        &self.0
    }
}

macro_rules! simple_ast {
    ($name:ident, $kind:ident) => {
        impl AstNode<UmlLanguage> for $name {
            fn can_cast(kind: UmlSyntaxKind) -> bool {
                kind == UmlSyntaxKind::$kind
            }
            fn cast(node: SyntaxNode<UmlLanguage>) -> Option<Self> {
                Self::can_cast(node.kind()).then_some(Self(node))
            }
            fn syntax(&self) -> &SyntaxNode<UmlLanguage> {
                &self.0
            }
        }
    };
}
simple_ast!(ValueSyntax, Value);
simple_ast!(SlotSyntax, Slot);
simple_ast!(RelationshipSyntax, Relationship);
simple_ast!(RelationshipEndSyntax, RelationshipEnd);
simple_ast!(MemberSyntax, Member);
simple_ast!(MemberGroupSyntax, MemberGroup);
simple_ast!(DiagramMembersSyntax, MembersSection);
simple_ast!(InlineInstanceSyntax, InlineInstance);
simple_ast!(LayoutSectionSyntax, LayoutSection);
simple_ast!(LayoutStatementSyntax, LayoutStatement);
simple_ast!(LayoutPlacementSyntax, LayoutPlacement);
simple_ast!(LayoutAlignmentSyntax, LayoutAlignment);
simple_ast!(LayoutStandaloneSyntax, LayoutStandalone);
simple_ast!(OperandSyntax, Operand);
simple_ast!(AnchoredSyntax, Anchored);
simple_ast!(DirectionClauseSyntax, DirectionClause);
simple_ast!(HintClauseSyntax, HintClause);
simple_ast!(EdgeSyntax, Edge);
simple_ast!(AxisSyntax, Axis);
simple_ast!(OperandRefSyntax, OperandRef);
simple_ast!(NameRefSyntax, NameRef);
simple_ast!(HintSyntax, Hint);
simple_ast!(ShapeSyntax, Shape);
simple_ast!(MarginSyntax, Margin);
simple_ast!(FlagSyntax, Flag);
simple_ast!(FlowNodeSyntax, FlowNode);
simple_ast!(FlowTransitionSyntax, FlowTransition);
simple_ast!(FlowBlockSyntax, FlowBlock);
simple_ast!(FlowInternalSyntax, FlowInternal);
simple_ast!(LifelineSyntax, Lifeline);
simple_ast!(MessageSyntax, Message);
simple_ast!(SequenceOperandSyntax, SequenceOperand);
simple_ast!(SequenceFragmentSyntax, SequenceFragment);
simple_ast!(MessagesBlockSyntax, MessagesSection);

macro_rules! behavior_syntax {
    ($name:ident, $recovery:expr) => {
        impl $name {
            pub fn recovery(&self) -> impl Iterator<Item = SyntaxElement<UmlLanguage>> + '_ {
                recovery_at(&self.0, $recovery).into_iter()
            }
        }
    };
}

impl FlowNodeSyntax {
    pub const HEADING_SLOT: usize = 0;
    pub const KIND_SLOT: usize = 1;
    pub const IDENTITY_SLOT: usize = 2;
    pub const RECOVERY_SLOT: usize = 3;
    pub const NEWLINE_SLOT: usize = 4;
    pub const BODY_START: usize = 5;

    pub fn kind_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::KIND_SLOT, 0)
    }
    pub fn identity_token(&self) -> SyntaxToken<UmlLanguage> {
        let identity =
            node_at(&self.0, Self::IDENTITY_SLOT).expect("flow node has fixed identity occurrence");
        match identity
            .child_at(0)
            .expect("flow identity has fixed token-or-link slot")
        {
            SyntaxElement::Token(token) => token,
            SyntaxElement::Node(link) => required_token_at(&link, 1),
        }
    }
    pub fn object_link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        node_at(&self.0, Self::IDENTITY_SLOT).and_then(|identity| node_at(&identity, 0))
    }
    pub fn internal_blocks(&self) -> impl Iterator<Item = SyntaxNode<UmlLanguage>> + '_ {
        self.0
            .children()
            .filter(|element| element.kind() == UmlSyntaxKind::FlowInternal)
            .filter_map(SyntaxElement::into_node)
    }
}
behavior_syntax!(FlowNodeSyntax, FlowNodeSyntax::RECOVERY_SLOT);

impl FlowTransitionSyntax {
    pub const BULLET_SLOT: usize = 0;
    pub const TRIGGER_SLOT: usize = 1;
    pub const GUARD_SLOT: usize = 2;
    pub const TRANSITIONS_KEYWORD_SLOT: usize = 3;
    pub const TO_SLOT: usize = 4;
    pub const TARGET_SLOT: usize = 5;
    pub const CARRIES_SLOT: usize = 6;
    pub const EFFECT_SLOT: usize = 7;
    pub const RECOVERY_SLOT: usize = 8;
    pub const NEWLINE_SLOT: usize = 9;

    pub fn trigger_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::TRIGGER_SLOT, 1)
    }
    pub fn guard_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::GUARD_SLOT, 1)
    }
    pub fn target_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        let target = node_at(&self.0, Self::TARGET_SLOT)?;
        match target.child_at(0)? {
            SyntaxElement::Token(token) => Some(token),
            SyntaxElement::Node(link) => Some(required_token_at(&link, 4)),
        }
    }
    pub fn target_link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        node_at(&self.0, Self::TARGET_SLOT).and_then(|target| node_at(&target, 0))
    }
    pub fn carries_link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        node_at(&self.0, Self::CARRIES_SLOT)
            .and_then(|carries| node_at(&carries, 1))
            .filter(|link| present_token_at(link, 4).is_some())
    }
    pub fn effect_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::EFFECT_SLOT, 1)
    }
    pub fn else_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::GUARD_SLOT, 0)
            .filter(|token| token.kind() == UmlSyntaxKind::ElseToken)
    }
}
behavior_syntax!(FlowTransitionSyntax, FlowTransitionSyntax::RECOVERY_SLOT);

impl FlowBlockSyntax {
    pub fn nodes(&self) -> impl Iterator<Item = FlowNodeSyntax> + '_ {
        self.0
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter_map(FlowNodeSyntax::cast)
    }
}
impl FlowInternalSyntax {
    pub const BULLET_SLOT: usize = 0;
    pub const KEYWORD_SLOT: usize = 1;
    pub const COLON_SLOT: usize = 2;
    pub const VALUE_SLOT: usize = 3;
    pub const LINK_SLOT: usize = 4;
    pub const RECOVERY_SLOT: usize = 5;
    pub const NEWLINE_SLOT: usize = 6;

    pub fn keyword_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        direct_token_at(&self.0, Self::KEYWORD_SLOT)
    }
    pub fn value_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::VALUE_SLOT, 0)
    }
    pub fn link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        node_at(&self.0, Self::LINK_SLOT).filter(|link| present_token_at(link, 4).is_some())
    }
}
behavior_syntax!(FlowInternalSyntax, FlowInternalSyntax::RECOVERY_SLOT);
impl LifelineSyntax {
    pub const BULLET_SLOT: usize = 0;
    pub const LINK_SLOT: usize = 1;
    pub const AS_SLOT: usize = 2;
    pub const ALIAS_SLOT: usize = 3;
    pub const RECOVERY_SLOT: usize = 4;
    pub const NEWLINE_SLOT: usize = 5;

    pub fn link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        node_at(&self.0, Self::LINK_SLOT)
    }
    pub fn alias_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::ALIAS_SLOT, 0)
    }
}
behavior_syntax!(LifelineSyntax, LifelineSyntax::RECOVERY_SLOT);

impl MessageSyntax {
    pub const BULLET_SLOT: usize = 0;
    pub const SOURCE_SLOT: usize = 1;
    pub const VERB_SLOT: usize = 2;
    pub const TARGET_SLOT: usize = 3;
    pub const ASYNC_SLOT: usize = 4;
    pub const VALUE_SLOT: usize = 5;
    pub const AS_SLOT: usize = 6;
    pub const CALL_ID_SLOT: usize = 7;
    pub const TO_SLOT: usize = 8;
    pub const RETURN_TARGET_SLOT: usize = 9;
    pub const FOR_SLOT: usize = 10;
    pub const RETURN_CALL_SLOT: usize = 11;
    pub const COLON_SLOT: usize = 12;
    pub const RECOVERY_SLOT: usize = 13;
    pub const NEWLINE_SLOT: usize = 14;

    pub fn source_token(&self) -> SyntaxToken<UmlLanguage> {
        required_slot_token_at(&self.0, Self::SOURCE_SLOT, 0)
    }
    pub fn verb_token(&self) -> SyntaxToken<UmlLanguage> {
        required_slot_token_at(&self.0, Self::VERB_SLOT, 0)
    }
    pub fn target_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::TARGET_SLOT, 0)
    }
    pub fn async_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::ASYNC_SLOT, 0)
    }
    pub fn value_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::VALUE_SLOT, 0)
    }
    pub fn as_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_token_at(&self.0, Self::AS_SLOT)
    }
    pub fn call_id_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::CALL_ID_SLOT, 0)
    }
    pub fn to_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_token_at(&self.0, Self::TO_SLOT)
    }
    pub fn return_target_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::RETURN_TARGET_SLOT, 0)
    }
    pub fn for_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_token_at(&self.0, Self::FOR_SLOT)
    }
    pub fn return_call_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::RETURN_CALL_SLOT, 0)
    }
    pub fn colon_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_token_at(&self.0, Self::COLON_SLOT)
    }
}
behavior_syntax!(MessageSyntax, MessageSyntax::RECOVERY_SLOT);

impl SequenceFragmentSyntax {
    pub const BULLET_SLOT: usize = 0;
    pub const KIND_SLOT: usize = 1;
    pub const RECOVERY_SLOT: usize = 2;
    pub const NEWLINE_SLOT: usize = 3;

    pub fn kind_token(&self) -> SyntaxToken<UmlLanguage> {
        required_slot_token_at(&self.0, Self::KIND_SLOT, 0)
    }
}
behavior_syntax!(
    SequenceFragmentSyntax,
    SequenceFragmentSyntax::RECOVERY_SLOT
);

impl SequenceOperandSyntax {
    pub const BULLET_SLOT: usize = 0;
    pub const KEYWORD_SLOT: usize = 1;
    pub const GUARD_SLOT: usize = 2;
    pub const RECOVERY_SLOT: usize = 3;
    pub const NEWLINE_SLOT: usize = 4;

    pub fn keyword_token(&self) -> SyntaxToken<UmlLanguage> {
        required_token_at(&self.0, Self::KEYWORD_SLOT)
    }
    pub fn guard_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        present_slot_token_at(&self.0, Self::GUARD_SLOT, 0)
    }
}
behavior_syntax!(SequenceOperandSyntax, SequenceOperandSyntax::RECOVERY_SLOT);

fn node_at(node: &SyntaxNode<UmlLanguage>, index: usize) -> Option<SyntaxNode<UmlLanguage>> {
    node.child_at(index).and_then(SyntaxElement::into_node)
}

fn direct_token_at(
    node: &SyntaxNode<UmlLanguage>,
    index: usize,
) -> Option<SyntaxToken<UmlLanguage>> {
    node.child_at(index).and_then(SyntaxElement::into_token)
}

fn required_token_at(node: &SyntaxNode<UmlLanguage>, index: usize) -> SyntaxToken<UmlLanguage> {
    direct_token_at(node, index).expect("required token has a fixed direct index")
}

fn present_token_at(
    node: &SyntaxNode<UmlLanguage>,
    index: usize,
) -> Option<SyntaxToken<UmlLanguage>> {
    direct_token_at(node, index).filter(|token| !token.flags().is_missing())
}

fn required_slot_token_at(
    node: &SyntaxNode<UmlLanguage>,
    slot_index: usize,
    token_index: usize,
) -> SyntaxToken<UmlLanguage> {
    let slot = node_at(node, slot_index).expect("required occurrence has a fixed direct index");
    required_token_at(&slot, token_index)
}

fn present_slot_token_at(
    node: &SyntaxNode<UmlLanguage>,
    slot_index: usize,
    token_index: usize,
) -> Option<SyntaxToken<UmlLanguage>> {
    node_at(node, slot_index).and_then(|slot| present_token_at(&slot, token_index))
}

fn recovery_at(
    node: &SyntaxNode<UmlLanguage>,
    slot_index: usize,
) -> Option<SyntaxElement<UmlLanguage>> {
    let recovery = node_at(node, slot_index)?;
    let element = recovery.child_at(0)?;
    match &element {
        SyntaxElement::Node(node) if node.kind() == UmlSyntaxKind::SkippedTokensSyntax => {
            Some(element)
        }
        SyntaxElement::Token(token) if token.flags().is_bad() && !token.flags().is_missing() => {
            Some(element)
        }
        _ => None,
    }
}
impl LayoutStatementSyntax {
    pub fn placement(&self) -> Option<LayoutPlacementSyntax> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::LayoutPlacement)
            .and_then(SyntaxElement::into_node)
            .map(LayoutPlacementSyntax)
    }
    pub fn alignment(&self) -> Option<LayoutAlignmentSyntax> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::LayoutAlignment)
            .and_then(SyntaxElement::into_node)
            .map(LayoutAlignmentSyntax)
    }
    pub fn standalone(&self) -> Option<LayoutStandaloneSyntax> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::LayoutStandalone)
            .and_then(SyntaxElement::into_node)
            .map(LayoutStandaloneSyntax)
    }
    /// Lossless layout leaves in authored order, projected from the fixed
    /// typed-node tree without reconstructing source or grammar state.
    pub fn atoms(&self) -> impl Iterator<Item = SyntaxToken<UmlLanguage>> + '_ {
        fn collect(node: &SyntaxNode<UmlLanguage>, out: &mut Vec<SyntaxToken<UmlLanguage>>) {
            for element in node.children() {
                if let Some(token) = element.clone().into_token() {
                    if matches!(
                        token.kind(),
                        UmlSyntaxKind::LayoutWordToken
                            | UmlSyntaxKind::LayoutLinkToken
                            | UmlSyntaxKind::LayoutQuoteToken
                            | UmlSyntaxKind::LayoutOpenParenToken
                            | UmlSyntaxKind::LayoutCloseParenToken
                            | UmlSyntaxKind::LayoutCommaToken
                    ) {
                        out.push(token);
                    }
                } else if let Some(child) = element.into_node() {
                    collect(&child, out);
                }
            }
        }
        let mut atoms = Vec::new();
        collect(&self.0, &mut atoms);
        atoms.into_iter()
    }
    pub fn typed_atoms(&self) -> impl Iterator<Item = LayoutAtomSyntax> + '_ {
        self.atoms().filter_map(|token| match token.kind() {
            UmlSyntaxKind::LayoutWordToken => Some(LayoutAtomSyntax::Word(token)),
            UmlSyntaxKind::LayoutLinkToken => Some(LayoutAtomSyntax::Link(token)),
            UmlSyntaxKind::LayoutQuoteToken => Some(LayoutAtomSyntax::Quote(token)),
            UmlSyntaxKind::LayoutOpenParenToken => Some(LayoutAtomSyntax::OpenParen(token)),
            UmlSyntaxKind::LayoutCloseParenToken => Some(LayoutAtomSyntax::CloseParen(token)),
            UmlSyntaxKind::LayoutCommaToken => Some(LayoutAtomSyntax::Comma(token)),
            _ => None,
        })
    }
}
impl LayoutPlacementSyntax {
    pub fn operands(&self) -> impl Iterator<Item = OperandSyntax> + '_ {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::Operand)
            .filter_map(SyntaxElement::into_node)
            .map(OperandSyntax)
    }
    pub fn directions(&self) -> impl Iterator<Item = DirectionClauseSyntax> + '_ {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::DirectionClause)
            .filter_map(SyntaxElement::into_node)
            .map(DirectionClauseSyntax)
    }
}
impl LayoutAlignmentSyntax {
    pub fn anchored(&self) -> impl Iterator<Item = AnchoredSyntax> + '_ {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::Anchored)
            .filter_map(SyntaxElement::into_node)
            .map(AnchoredSyntax)
    }
    pub fn left(&self) -> Option<AnchoredSyntax> {
        self.anchored().next()
    }
    pub fn right(&self) -> Option<AnchoredSyntax> {
        self.anchored().nth(1)
    }
}
impl LayoutStandaloneSyntax {
    pub fn operand(&self) -> Option<OperandSyntax> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::Operand)
            .and_then(SyntaxElement::into_node)
            .map(OperandSyntax)
    }
}
impl OperandSyntax {
    pub fn reference(&self) -> Option<OperandRefSyntax> {
        direct_node(&self.0, UmlSyntaxKind::OperandRef).map(OperandRefSyntax)
    }
    pub fn axis(&self) -> Option<AxisSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Axis).map(AxisSyntax)
    }
    pub fn hint_clause(&self) -> Option<HintClauseSyntax> {
        direct_node(&self.0, UmlSyntaxKind::HintClause).map(HintClauseSyntax)
    }
    pub fn hints(&self) -> impl Iterator<Item = HintSyntax> + '_ {
        self.hint_clause().into_iter().flat_map(|clause| {
            clause
                .0
                .children()
                .filter(|element| element.kind() == UmlSyntaxKind::Hint)
                .filter_map(SyntaxElement::into_node)
                .map(HintSyntax)
                .collect::<Vec<_>>()
        })
    }

    /// Decode the already-shaped operand slots without re-reading the
    /// statement's authored atom sequence.
    pub fn value(&self) -> Option<crate::layout::Operand> {
        Some(crate::layout::Operand {
            ref_: self.reference()?.value()?,
            axis: match self.axis() {
                Some(axis) => Some(axis.value()?),
                None => None,
            },
            hints: self
                .hints()
                .map(|hint| hint.value())
                .collect::<Option<Vec<_>>>()?,
        })
    }
}
impl AnchoredSyntax {
    pub fn edge(&self) -> Option<EdgeSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Edge).map(EdgeSyntax)
    }
    pub fn operand(&self) -> Option<OperandSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Operand).map(OperandSyntax)
    }

    pub fn value(&self) -> Option<crate::layout::Anchored> {
        Some(crate::layout::Anchored {
            edge: match self.edge() {
                Some(edge) => Some(edge.value()?),
                None => None,
            },
            operand: self.operand()?.value()?,
        })
    }
}
impl OperandRefSyntax {
    pub fn name(&self) -> Option<NameRefSyntax> {
        direct_node(&self.0, UmlSyntaxKind::NameRef).map(NameRefSyntax)
    }
    pub fn group_axis(&self) -> Option<AxisSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Axis).map(AxisSyntax)
    }
    pub fn items(&self) -> impl Iterator<Item = OperandSyntax> + '_ {
        self.0
            .children()
            .filter(|element| element.kind() == UmlSyntaxKind::Operand)
            .filter_map(SyntaxElement::into_node)
            .map(OperandSyntax)
    }

    pub fn value(&self) -> Option<crate::layout::OperandRef> {
        if let Some(name) = self.name() {
            return Some(crate::layout::OperandRef::Name(name.value()?));
        }
        if let Some(axis) = self.group_axis() {
            return Some(crate::layout::OperandRef::InlineGroup {
                axis: axis.value()?,
                items: self
                    .items()
                    .map(|item| item.value())
                    .collect::<Option<Vec<_>>>()?,
            });
        }
        let mut items = self.items();
        let inner = items.next()?.value()?;
        items
            .next()
            .is_none()
            .then_some(crate::layout::OperandRef::Paren(Box::new(inner)))
    }
}
impl HintSyntax {
    pub fn shape(&self) -> Option<ShapeSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Shape).map(ShapeSyntax)
    }
    pub fn margin(&self) -> Option<MarginSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Margin).map(MarginSyntax)
    }
    pub fn flag(&self) -> Option<FlagSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Flag).map(FlagSyntax)
    }

    pub fn value(&self) -> Option<crate::layout::Hint> {
        if let Some(shape) = self.shape() {
            return Some(crate::layout::Hint::Shape(shape.value()?));
        }
        if let Some(margin) = self.margin() {
            return Some(crate::layout::Hint::Margin(margin.value()?));
        }
        self.flag()
            .map(|flag| flag.value().map(crate::layout::Hint::Flag))?
    }
}

impl DirectionClauseSyntax {
    pub fn value(&self) -> Option<crate::layout::Direction> {
        use crate::layout::Direction;
        match direct_layout_words(&self.0).as_slice() {
            [first] if first == "above" => Some(Direction::Above),
            [first] if first == "below" => Some(Direction::Below),
            [first, of] if first == "left" && of == "of" => Some(Direction::LeftOf),
            [first, of] if first == "right" && of == "of" => Some(Direction::RightOf),
            [first, diagonal, of] if first == "above" && diagonal == "left" && of == "of" => {
                Some(Direction::AboveLeft)
            }
            [first, diagonal, of] if first == "above" && diagonal == "right" && of == "of" => {
                Some(Direction::AboveRight)
            }
            [first, diagonal, of] if first == "below" && diagonal == "left" && of == "of" => {
                Some(Direction::BelowLeft)
            }
            [first, diagonal, of] if first == "below" && diagonal == "right" && of == "of" => {
                Some(Direction::BelowRight)
            }
            _ => None,
        }
    }
}

impl AxisSyntax {
    pub fn value(&self) -> Option<crate::layout::Axis> {
        match direct_layout_words(&self.0).last().map(String::as_str) {
            Some("row") => Some(crate::layout::Axis::Row),
            Some("column") => Some(crate::layout::Axis::Column),
            _ => None,
        }
    }
}

impl EdgeSyntax {
    pub fn value(&self) -> Option<crate::layout::Edge> {
        match direct_layout_words(&self.0).first().map(String::as_str) {
            Some("top") => Some(crate::layout::Edge::Top),
            Some("bottom") => Some(crate::layout::Edge::Bottom),
            Some("left") => Some(crate::layout::Edge::Left),
            Some("right") => Some(crate::layout::Edge::Right),
            Some("center") => Some(crate::layout::Edge::Center),
            _ => None,
        }
    }
}

impl NameRefSyntax {
    pub fn value(&self) -> Option<crate::layout::NameRef> {
        let token = self
            .0
            .children()
            .find_map(SyntaxElement::into_token)
            .filter(|token| !token.flags().is_missing())?;
        let authored = token.text().write_to_string();
        let authored = authored.trim();
        match token.kind() {
            UmlSyntaxKind::LayoutLinkToken => {
                let (title, target) = authored.strip_prefix('[')?.split_once("](")?;
                Some(crate::layout::NameRef::Link {
                    title: title.to_string(),
                    slug: target.strip_suffix(')')?.to_string(),
                })
            }
            UmlSyntaxKind::LayoutQuoteToken => Some(crate::layout::NameRef::Bare(
                authored.strip_prefix('"')?.strip_suffix('"')?.to_string(),
            )),
            UmlSyntaxKind::LayoutWordToken => {
                Some(crate::layout::NameRef::Bare(authored.to_string()))
            }
            _ => None,
        }
    }
}

impl ShapeSyntax {
    pub fn value(&self) -> Option<crate::layout::Shape> {
        match direct_layout_words(&self.0).first().map(String::as_str) {
            Some("frame") => Some(crate::layout::Shape::Frame),
            Some("box") => Some(crate::layout::Shape::Box),
            Some("shrink") => Some(crate::layout::Shape::Shrink),
            _ => None,
        }
    }
}

impl MarginSyntax {
    pub fn value(&self) -> Option<crate::layout::Margin> {
        match direct_layout_words(&self.0).first().map(String::as_str) {
            Some("no") => Some(crate::layout::Margin::No),
            Some("small") => Some(crate::layout::Margin::Small),
            Some("medium") => Some(crate::layout::Margin::Medium),
            Some("large") => Some(crate::layout::Margin::Large),
            _ => None,
        }
    }
}

impl FlagSyntax {
    pub fn value(&self) -> Option<crate::layout::Flag> {
        match direct_layout_words(&self.0).first().map(String::as_str) {
            Some("emphasized") => Some(crate::layout::Flag::Emphasized),
            Some("collapsed") => Some(crate::layout::Flag::Collapsed),
            _ => None,
        }
    }
}

fn direct_layout_words(node: &SyntaxNode<UmlLanguage>) -> Vec<String> {
    node.children()
        .filter_map(SyntaxElement::into_token)
        .filter(|token| {
            token.kind() == UmlSyntaxKind::LayoutWordToken && !token.flags().is_missing()
        })
        .map(|token| token.text().write_to_string().trim().to_ascii_lowercase())
        .collect()
}

fn direct_node(
    node: &SyntaxNode<UmlLanguage>,
    kind: UmlSyntaxKind,
) -> Option<SyntaxNode<UmlLanguage>> {
    node.children()
        .find(|element| element.kind() == kind)
        .and_then(SyntaxElement::into_node)
}

macro_rules! direct_token {
    ($name:ident, $method:ident, $kind:ident) => {
        impl $name {
            pub fn $method(&self) -> Option<SyntaxToken<UmlLanguage>> {
                self.0
                    .children()
                    .find(|e| e.kind() == UmlSyntaxKind::$kind)
                    .and_then(SyntaxElement::into_token)
            }
        }
    };
}
direct_token!(ValueSyntax, value_token, IdentifierToken);
direct_token!(SlotSyntax, name_token, IdentifierToken);
direct_token!(SlotSyntax, colon_token, ColonToken);
direct_token!(RelationshipSyntax, kind_token, RelationshipKindToken);
macro_rules! direct_link {
    ($name:ident) => {
        impl $name {
            pub fn link(&self) -> Option<SyntaxNode<UmlLanguage>> {
                self.0
                    .children()
                    .find(|e| e.kind() == UmlSyntaxKind::Link)
                    .and_then(SyntaxElement::into_node)
            }
        }
    };
}
direct_link!(SlotSyntax);
direct_link!(RelationshipSyntax);
direct_link!(MemberSyntax);
direct_link!(InlineInstanceSyntax);
impl MemberSyntax {
    pub fn target_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.link()?
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::LinkTargetToken)
            .and_then(SyntaxElement::into_token)
    }
}
impl SlotSyntax {
    pub fn value_kind(&self) -> SlotValueKind {
        let Some(colon) = self.colon_token() else {
            return SlotValueKind::Missing;
        };
        if colon.flags().is_missing() {
            return SlotValueKind::Missing;
        }
        let mut after_colon = false;
        for child in self.0.children() {
            if child.kind() == UmlSyntaxKind::ColonToken {
                after_colon = true;
                continue;
            }
            if !after_colon {
                continue;
            }
            return match child.kind() {
                UmlSyntaxKind::Link => SlotValueKind::Link,
                UmlSyntaxKind::TypeToken => SlotValueKind::Quoted,
                UmlSyntaxKind::IdentifierToken => SlotValueKind::Bare,
                UmlSyntaxKind::BadToken | UmlSyntaxKind::SkippedTokensSyntax => {
                    SlotValueKind::Invalid
                }
                _ => continue,
            };
        }
        SlotValueKind::Missing
    }
    pub fn value_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        let mut after_colon = false;
        for child in self.0.children() {
            if child.kind() == UmlSyntaxKind::ColonToken {
                after_colon = true;
                continue;
            }
            if after_colon {
                if let Some(token) = child.clone().into_token() {
                    return Some(token);
                }
                if child.kind() == UmlSyntaxKind::Link {
                    return child
                        .into_node()?
                        .children()
                        .find(|e| e.kind() == UmlSyntaxKind::LinkTargetToken)
                        .and_then(SyntaxElement::into_token);
                }
            }
        }
        None
    }
}
impl RelationshipSyntax {
    pub fn recovery(&self) -> impl Iterator<Item = SyntaxElement<UmlLanguage>> + '_ {
        self.0.children().filter(|e| {
            matches!(
                e.kind(),
                UmlSyntaxKind::SkippedTokensSyntax | UmlSyntaxKind::BadToken
            )
        })
    }
    pub fn name_label_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::RelationshipName)
            .and_then(|e| e.into_node())?
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::TypeToken)
            .and_then(SyntaxElement::into_token)
    }
    pub fn name_link(&self) -> Option<SyntaxNode<UmlLanguage>> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::RelationshipName)
            .and_then(|e| e.into_node())?
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::Link)
            .and_then(SyntaxElement::into_node)
    }
    pub fn colon_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::ColonToken)
            .and_then(SyntaxElement::into_token)
    }
    pub fn source_end(&self) -> Option<RelationshipEndSyntax> {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::RelationshipEnd)
            .nth(0)
            .and_then(SyntaxElement::into_node)
            .map(RelationshipEndSyntax)
    }
    pub fn to_end(&self) -> Option<RelationshipEndSyntax> {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::RelationshipEnd)
            .nth(1)
            .and_then(SyntaxElement::into_node)
            .map(RelationshipEndSyntax)
    }
    pub fn target_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.link()?
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::LinkTargetToken)
            .and_then(SyntaxElement::into_token)
    }
}
impl RelationshipEndSyntax {
    pub fn multiplicity_token(&self) -> SyntaxToken<UmlLanguage> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::IdentifierToken)
            .and_then(|e| e.clone().into_token())
            .expect("relationship end has fixed multiplicity slot")
    }
    pub fn role_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::IdentifierToken)
            .nth(1)
            .and_then(SyntaxElement::into_token)
    }
}
impl InlineInstanceSyntax {
    pub fn name_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        let mut after_as = false;
        for child in self.0.children() {
            if child.kind() == UmlSyntaxKind::AsToken {
                after_as = true;
                continue;
            }
            if after_as && child.kind() == UmlSyntaxKind::IdentifierToken {
                return child.into_token();
            }
        }
        None
    }
    pub fn slots(&self) -> impl Iterator<Item = SlotSyntax> + '_ {
        self.0
            .children()
            .filter(|e| e.kind() == UmlSyntaxKind::InlineSlot)
            .filter_map(|e| e.into_node())
            .map(SlotSyntax)
    }
    pub fn classifier_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.link()?
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::LinkTargetToken)
            .and_then(SyntaxElement::into_token)
    }
}
impl MemberGroupSyntax {
    pub fn heading_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
        self.0
            .children()
            .find(|e| e.kind() == UmlSyntaxKind::IdentifierToken)
            .and_then(|e| e.into_token())
    }
}
