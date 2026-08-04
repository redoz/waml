use crate::{GreenNode, GreenToken, SyntaxElement, SyntaxLanguage, SyntaxNode, SyntaxToken};
use std::ops::Range;

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

pub struct AstSlots<'a, L: SyntaxLanguage> {
    syntax: &'a SyntaxNode<L>,
}
impl<'a, L: SyntaxLanguage> AstSlots<'a, L> {
    pub fn new(syntax: &'a SyntaxNode<L>) -> Self {
        Self { syntax }
    }
    pub fn len(&self) -> usize {
        self.syntax.children().count()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn required_node(&self, index: usize) -> Option<SyntaxNode<L>> {
        self.syntax.child_at(index)?.into_node()
    }
    pub fn required_token(&self, index: usize) -> Option<SyntaxToken<L>> {
        self.syntax.child_at(index)?.into_token()
    }
    pub fn list(&self, indices: Range<usize>) -> Vec<SyntaxElement<L>> {
        let start = indices.start;
        self.syntax
            .children()
            .skip(start)
            .take(indices.len())
            .collect()
    }
}
