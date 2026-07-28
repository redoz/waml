use super::{UmlLanguage, UmlSyntaxKind};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode, SyntaxToken};

#[derive(Clone, Debug)]
pub struct AttributeSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct TypeReferenceSyntax(pub(crate) SyntaxNode<UmlLanguage>);
#[derive(Clone, Debug)]
pub struct MultiplicitySyntax(pub(crate) SyntaxNode<UmlLanguage>);
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
