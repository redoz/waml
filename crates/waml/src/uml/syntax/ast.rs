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
pub struct MemberSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MemberGroupSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct InlineInstanceSyntax(pub(crate) SyntaxNode<UmlLanguage>);
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
simple_ast!(MemberSyntax, Member);
simple_ast!(MemberGroupSyntax, MemberGroup);
simple_ast!(InlineInstanceSyntax, InlineInstance);

macro_rules! raw_slot {
    ($name:ident) => {
        impl $name {
            pub fn content_token(&self) -> Option<SyntaxToken<UmlLanguage>> {
                self.0
                    .children()
                    .find(|e| e.kind() == UmlSyntaxKind::RawMarkdownToken)
                    .and_then(SyntaxElement::into_token)
            }
        }
    };
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
impl InlineInstanceSyntax {
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
            .find_map(SyntaxElement::into_node)
            .and_then(|n| {
                n.children()
                    .find(|e| e.kind() == UmlSyntaxKind::RawMarkdownToken)
                    .and_then(SyntaxElement::into_token)
            })
    }
}
