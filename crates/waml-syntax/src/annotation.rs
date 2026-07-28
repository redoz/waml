use crate::{
    GreenElement, GreenFactory, GreenNode, RewriteError, SyntaxLanguage, SyntaxLocator, SyntaxNode,
    SyntaxTree,
};
use std::{num::NonZeroU64, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SyntaxAnnotation {
    id: NonZeroU64,
    kind: Arc<str>,
    data: Option<Arc<str>>,
}
impl SyntaxAnnotation {
    pub fn new(id: NonZeroU64, kind: impl Into<Arc<str>>, data: Option<Arc<str>>) -> Self {
        Self {
            id,
            kind: kind.into(),
            data,
        }
    }
    pub fn id(&self) -> NonZeroU64 {
        self.id
    }
    pub fn kind(&self) -> &str {
        &self.kind
    }
    pub fn data(&self) -> Option<&str> {
        self.data.as_deref()
    }
}
pub fn annotate_occurrence<L: SyntaxLanguage>(
    tree: &SyntaxTree<L>,
    locator: &SyntaxLocator<L>,
    annotation: SyntaxAnnotation,
) -> Result<GreenNode<L>, RewriteError<L::Kind>> {
    let _ = tree.resolve(locator)?;
    Ok(rebuild(
        &tree.root_green,
        locator.path().as_slice(),
        annotation,
    ))
}
fn rebuild<L: SyntaxLanguage>(
    node: &GreenNode<L>,
    path: &[u32],
    annotation: SyntaxAnnotation,
) -> GreenNode<L> {
    if path.is_empty() {
        let mut a = node.annotations().to_vec();
        a.push(annotation);
        return GreenFactory::new()
            .node_with_annotations(node.kind(), node.children().iter().cloned(), a.into())
            .unwrap();
    }
    let i = path[0] as usize;
    let children = node
        .children()
        .iter()
        .enumerate()
        .map(|(n, e)| match (n == i, e, path.len()) {
            (true, GreenElement::Node(child), _) => {
                GreenElement::Node(rebuild(child, &path[1..], annotation.clone()))
            }
            (true, GreenElement::Token(token), 1) => {
                let mut annotations = token.syntax_annotations().to_vec();
                annotations.push(annotation.clone());
                GreenElement::Token(
                    GreenFactory::new().token_with_syntax_annotations(token, annotations.into()),
                )
            }
            _ => e.clone(),
        });
    GreenFactory::new()
        .node_with_annotations(node.kind(), children, node.annotations().into())
        .unwrap()
}
pub fn find_annotation<L: SyntaxLanguage>(
    tree: &SyntaxTree<L>,
    id: NonZeroU64,
) -> Vec<SyntaxNode<L>> {
    let mut out = Vec::new();
    fn go<L: SyntaxLanguage>(n: SyntaxNode<L>, id: NonZeroU64, out: &mut Vec<SyntaxNode<L>>) {
        if n.green().annotations().iter().any(|a| a.id() == id) {
            out.push(n.clone())
        }
        for e in n.children() {
            if let Some(c) = e.into_node() {
                go(c, id, out)
            }
        }
    }
    go(tree.root(), id, &mut out);
    out
}
