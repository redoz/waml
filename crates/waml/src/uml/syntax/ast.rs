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
pub struct LifelineSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MessageSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct SequenceOperandSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MessagesBlockSyntax(pub(crate) SyntaxNode<UmlLanguage>);
/// A typed leaf in a layout statement.  The source range belongs to this
/// token directly, keeping diagnostics on links/quotes/delimiters precise.
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
simple_ast!(LifelineSyntax, Lifeline);
simple_ast!(MessageSyntax, Message);
simple_ast!(SequenceOperandSyntax, SequenceOperand);
simple_ast!(MessagesBlockSyntax, MessagesSection);

macro_rules! behavior_syntax {
    ($name:ident) => {
        impl $name {
            pub fn tokens(&self) -> impl Iterator<Item = SyntaxToken<UmlLanguage>> + '_ {
                self.0.children().filter_map(SyntaxElement::into_token)
            }
            pub fn recovery(&self) -> impl Iterator<Item = SyntaxElement<UmlLanguage>> + '_ {
                self.0.children().filter(|element| {
                    matches!(
                        element.kind(),
                        UmlSyntaxKind::SkippedTokensSyntax | UmlSyntaxKind::BadToken
                    )
                })
            }
        }
    };
}
behavior_syntax!(FlowNodeSyntax);
behavior_syntax!(FlowTransitionSyntax);
behavior_syntax!(LifelineSyntax);
behavior_syntax!(MessageSyntax);
behavior_syntax!(SequenceOperandSyntax);
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
    /// Direct, lossless layout atoms in authored order.  Consumers must not
    /// descend through arbitrary Markdown nodes or reconstruct the bullet.
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
}
impl AnchoredSyntax {
    pub fn edge(&self) -> Option<EdgeSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Edge).map(EdgeSyntax)
    }
    pub fn operand(&self) -> Option<OperandSyntax> {
        direct_node(&self.0, UmlSyntaxKind::Operand).map(OperandSyntax)
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
    pub fn from_end(&self) -> Option<RelationshipEndSyntax> {
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
