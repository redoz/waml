use crate::{GreenNode, GreenToken, SyntaxElement, SyntaxLanguage, SyntaxNode};

pub trait AstNode<L: SyntaxLanguage>: Sized {
    fn can_cast(kind: L::Kind) -> bool;
    fn cast(node: SyntaxNode<L>) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode<L>;
}

pub trait SyntaxVisitor<L: SyntaxLanguage> {
    fn visit(&mut self, element: SyntaxElement<L>);
}
pub trait SyntaxRewriter<L: SyntaxLanguage> {
    fn rewrite_node(&mut self, node: &GreenNode<L>) -> GreenNode<L>;
    fn rewrite_token(&mut self, token: &GreenToken<L>) -> GreenToken<L>;
}
