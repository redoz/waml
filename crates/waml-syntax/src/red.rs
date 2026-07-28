use crate::{
    write_green_to, GreenElement, GreenFactory, GreenNode, GreenToken, MarkdownDialect,
    SyntaxLanguage, TextError, TextRange, TextSize,
};
use std::{
    fmt,
    hash::{Hash, Hasher},
    num::NonZeroU64,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
static NEXT_TREE: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TreeInstanceId(NonZeroU64);
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SyntaxPath(Arc<[u32]>);
impl SyntaxPath {
    pub fn from_indices(v: impl IntoIterator<Item = u32>) -> Self {
        Self(v.into_iter().collect())
    }
    pub(crate) fn as_slice(&self) -> &[u32] {
        &self.0
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SyntaxSeverity {
    Error,
    Warning,
    Info,
}
#[derive(Clone, Debug)]
pub struct TreeDiagnostic<C> {
    pub code: C,
    pub severity: SyntaxSeverity,
    pub message: Arc<str>,
    pub range: TextRange,
}
#[derive(Clone, Debug)]
struct RedContext {
    tree: TreeInstanceId,
}
#[derive(Clone, Debug)]
struct RedNodeData<L: SyntaxLanguage> {
    green: GreenNode<L>,
    context: Arc<RedContext>,
    path: SyntaxPath,
    parent: Option<SyntaxNode<L>>,
    range: TextRange,
}
#[derive(Clone, Debug)]
struct RedTokenData<L: SyntaxLanguage> {
    green: GreenToken<L>,
    context: Arc<RedContext>,
    path: SyntaxPath,
    parent: SyntaxNode<L>,
    range: TextRange,
}
#[derive(Debug)]
pub struct SyntaxNode<L: SyntaxLanguage>(Arc<RedNodeData<L>>);
#[derive(Debug)]
pub struct SyntaxToken<L: SyntaxLanguage>(Arc<RedTokenData<L>>);
#[derive(Debug)]
pub enum SyntaxElement<L: SyntaxLanguage> {
    Node(SyntaxNode<L>),
    Token(SyntaxToken<L>),
}
impl<L: SyntaxLanguage> Clone for SyntaxNode<L> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<L: SyntaxLanguage> Clone for SyntaxToken<L> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<L: SyntaxLanguage> Clone for SyntaxElement<L> {
    fn clone(&self) -> Self {
        match self {
            Self::Node(n) => Self::Node(n.clone()),
            Self::Token(t) => Self::Token(t.clone()),
        }
    }
}
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct SyntaxLocator<L: SyntaxLanguage> {
    tree: TreeInstanceId,
    path: SyntaxPath,
    expected_kind: L::Kind,
}
#[derive(Debug)]
pub enum RewriteError<K> {
    WrongTree {
        expected: TreeInstanceId,
        actual: TreeInstanceId,
    },
    InvalidPath {
        depth: usize,
        child_index: u32,
    },
    ExpectedNode {
        path: SyntaxPath,
    },
    ExpectedToken {
        path: SyntaxPath,
    },
    KindMismatch {
        expected: K,
        actual: K,
    },
    Text(TextError),
}
impl<K: fmt::Debug + Send + Sync + 'static> fmt::Display for RewriteError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "syntax rewrite error: {:?}", self)
    }
}
impl<K: fmt::Debug + Send + Sync + 'static> std::error::Error for RewriteError<K> {}
impl<K> From<TextError> for RewriteError<K> {
    fn from(v: TextError) -> Self {
        Self::Text(v)
    }
}
pub struct SyntaxTree<L: SyntaxLanguage> {
    pub(crate) root_green: GreenNode<L>,
    context: Arc<RedContext>,
    diagnostics: Arc<[TreeDiagnostic<L::DiagnosticCode>]>,
    dialect: MarkdownDialect,
}
impl<L: SyntaxLanguage> SyntaxTree<L> {
    pub fn new(
        root_green: GreenNode<L>,
        diagnostics: Arc<[TreeDiagnostic<L::DiagnosticCode>]>,
        dialect: MarkdownDialect,
    ) -> Self {
        let id = NonZeroU64::new(NEXT_TREE.fetch_add(1, Ordering::Relaxed)).unwrap();
        Self {
            root_green,
            context: Arc::new(RedContext {
                tree: TreeInstanceId(id),
            }),
            diagnostics,
            dialect,
        }
    }
    pub fn root(&self) -> SyntaxNode<L> {
        node(
            self.root_green.clone(),
            self.context.clone(),
            SyntaxPath::from_indices([]),
            None,
            zero(),
        )
    }
    pub fn root_green(&self) -> &GreenNode<L> {
        &self.root_green
    }
    pub fn diagnostics(&self) -> &[TreeDiagnostic<L::DiagnosticCode>] {
        &self.diagnostics
    }
    pub fn write_to_string(&self) -> String {
        let mut s = String::new();
        write_green_to(&self.root_green, &mut s).expect("String");
        s
    }
    pub fn resolve(&self, l: &SyntaxLocator<L>) -> Result<SyntaxElement<L>, RewriteError<L::Kind>> {
        if self.context.tree != l.tree {
            return Err(RewriteError::WrongTree {
                expected: self.context.tree,
                actual: l.tree,
            });
        }
        let mut e = GreenElement::Node(self.root_green.clone());
        for (d, &i) in l.path.as_slice().iter().enumerate() {
            let GreenElement::Node(n) = e else {
                return Err(RewriteError::InvalidPath {
                    depth: d,
                    child_index: i,
                });
            };
            e = n
                .children()
                .get(i as usize)
                .cloned()
                .ok_or(RewriteError::InvalidPath {
                    depth: d,
                    child_index: i,
                })?;
        }
        let actual = match &e {
            GreenElement::Node(n) => n.kind(),
            GreenElement::Token(t) => t.kind(),
        };
        if actual != l.expected_kind {
            return Err(RewriteError::KindMismatch {
                expected: l.expected_kind,
                actual,
            });
        }
        Ok(match e {
            GreenElement::Node(_) => self
                .element_at(l.path.as_slice())
                .into_node()
                .unwrap()
                .into(),
            GreenElement::Token(_) => self
                .element_at(l.path.as_slice())
                .into_token()
                .unwrap()
                .into(),
        })
    }
    fn element_at(&self, path: &[u32]) -> SyntaxElement<L> {
        let mut n = self.root();
        for (i, &p) in path.iter().enumerate() {
            let e = n.children().nth(p as usize).unwrap();
            if i + 1 == path.len() {
                return e;
            }
            n = e.into_node().unwrap()
        }
        n.into()
    }
    pub fn rewrite(&self, r: &mut impl crate::SyntaxRewriter<L>) -> Self {
        fn same<L: SyntaxLanguage>(a: &GreenElement<L>, b: &GreenElement<L>) -> bool {
            match (a, b) {
                (GreenElement::Node(x), GreenElement::Node(y)) => Arc::ptr_eq(x, y),
                (GreenElement::Token(x), GreenElement::Token(y)) => Arc::ptr_eq(x, y),
                _ => false,
            }
        }
        fn go<L: SyntaxLanguage>(
            n: &GreenNode<L>,
            r: &mut impl crate::SyntaxRewriter<L>,
        ) -> GreenNode<L> {
            let c: Vec<_> = n
                .children()
                .iter()
                .map(|e| match e {
                    GreenElement::Node(x) => GreenElement::Node(go(x, r)),
                    GreenElement::Token(x) => GreenElement::Token(r.rewrite_token(x)),
                })
                .collect();
            if c.iter().zip(n.children()).all(|(a, b)| same(a, b)) {
                return r.rewrite_node(n);
            }
            let x = GreenFactory::new()
                .node_with_annotations(n.kind(), c, n.annotations().into())
                .unwrap();
            r.rewrite_node(&x)
        }
        Self::new(
            go(&self.root_green, r),
            self.diagnostics.clone(),
            self.dialect,
        )
    }
}
fn zero() -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(0).unwrap(),
        TextSize::try_from_usize(0).unwrap(),
    )
    .unwrap()
}
fn node<L: SyntaxLanguage>(
    g: GreenNode<L>,
    c: Arc<RedContext>,
    p: SyntaxPath,
    parent: Option<SyntaxNode<L>>,
    range: TextRange,
) -> SyntaxNode<L> {
    SyntaxNode(Arc::new(RedNodeData {
        green: g,
        context: c,
        path: p,
        parent,
        range,
    }))
}
impl<L: SyntaxLanguage> SyntaxNode<L> {
    pub fn kind(&self) -> L::Kind {
        self.0.green.kind()
    }
    pub(crate) fn green(&self) -> &GreenNode<L> {
        &self.0.green
    }
    pub fn range(&self) -> TextRange {
        self.0.range
    }
    pub fn parent(&self) -> Option<SyntaxNode<L>> {
        self.0.parent.clone()
    }
    pub fn children(&self) -> impl Iterator<Item = SyntaxElement<L>> + '_ {
        let mut start = self.0.range.start();
        let me = self.clone();
        self.0
            .green
            .children()
            .iter()
            .enumerate()
            .map(move |(i, e)| {
                let end = start
                    .checked_add(match e {
                        GreenElement::Node(n) => n.width(),
                        GreenElement::Token(t) => t.width(),
                    })
                    .unwrap();
                let range = TextRange::new(start, end).unwrap();
                start = end;
                let mut p = me.0.path.0.to_vec();
                p.push(i as u32);
                match e {
                    GreenElement::Node(n) => SyntaxElement::Node(node(
                        n.clone(),
                        me.0.context.clone(),
                        SyntaxPath::from_indices(p),
                        Some(me.clone()),
                        range,
                    )),
                    GreenElement::Token(t) => {
                        SyntaxElement::Token(SyntaxToken(Arc::new(RedTokenData {
                            green: t.clone(),
                            context: me.0.context.clone(),
                            path: SyntaxPath::from_indices(p),
                            parent: me.clone(),
                            range,
                        })))
                    }
                }
            })
    }
    pub fn child_at(&self, i: usize) -> Option<SyntaxElement<L>> {
        self.children().nth(i)
    }
    pub fn locator(&self) -> SyntaxLocator<L> {
        SyntaxLocator {
            tree: self.0.context.tree,
            path: self.0.path.clone(),
            expected_kind: self.kind(),
        }
    }
    pub fn walk(&self, v: &mut impl crate::SyntaxVisitor<L>) {
        v.visit(self.clone().into());
        for e in self.children() {
            match e {
                SyntaxElement::Node(n) => n.walk(v),
                x => v.visit(x),
            }
        }
    }
}
impl<L: SyntaxLanguage> SyntaxToken<L> {
    pub fn kind(&self) -> L::Kind {
        self.0.green.kind()
    }
    pub fn range(&self) -> TextRange {
        self.0.range
    }
    pub fn parent(&self) -> Option<SyntaxNode<L>> {
        Some(self.0.parent.clone())
    }
    pub fn locator(&self) -> SyntaxLocator<L> {
        SyntaxLocator {
            tree: self.0.context.tree,
            path: self.0.path.clone(),
            expected_kind: self.kind(),
        }
    }
    pub fn same_green(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0.green, &o.0.green)
    }
    pub fn previous_sibling(&self) -> Option<SyntaxElement<L>> {
        let i = *self.0.path.as_slice().last()? as usize;
        self.0.parent.child_at(i.checked_sub(1)?)
    }
}
impl<L: SyntaxLanguage> SyntaxElement<L> {
    pub fn kind(&self) -> L::Kind {
        match self {
            Self::Node(n) => n.kind(),
            Self::Token(t) => t.kind(),
        }
    }
    pub fn into_node(self) -> Option<SyntaxNode<L>> {
        if let Self::Node(x) = self {
            Some(x)
        } else {
            None
        }
    }
    pub fn into_token(self) -> Option<SyntaxToken<L>> {
        if let Self::Token(x) = self {
            Some(x)
        } else {
            None
        }
    }
    pub fn locator(&self) -> SyntaxLocator<L> {
        match self {
            Self::Node(n) => n.locator(),
            Self::Token(t) => t.locator(),
        }
    }
}
impl<L: SyntaxLanguage> From<SyntaxNode<L>> for SyntaxElement<L> {
    fn from(v: SyntaxNode<L>) -> Self {
        Self::Node(v)
    }
}
impl<L: SyntaxLanguage> From<SyntaxToken<L>> for SyntaxElement<L> {
    fn from(v: SyntaxToken<L>) -> Self {
        Self::Token(v)
    }
}
impl<L: SyntaxLanguage> PartialEq for SyntaxNode<L> {
    fn eq(&self, o: &Self) -> bool {
        self.0.context.tree == o.0.context.tree && self.0.path == o.0.path
    }
}
impl<L: SyntaxLanguage> Eq for SyntaxNode<L> {}
impl<L: SyntaxLanguage> Hash for SyntaxNode<L> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        self.0.context.tree.hash(s);
        self.0.path.hash(s)
    }
}
impl<L: SyntaxLanguage> PartialEq for SyntaxToken<L> {
    fn eq(&self, o: &Self) -> bool {
        self.0.context.tree == o.0.context.tree && self.0.path == o.0.path
    }
}
impl<L: SyntaxLanguage> Eq for SyntaxToken<L> {}
impl<L: SyntaxLanguage> Hash for SyntaxToken<L> {
    fn hash<H: Hasher>(&self, s: &mut H) {
        self.0.context.tree.hash(s);
        self.0.path.hash(s)
    }
}
impl<L: SyntaxLanguage> SyntaxLocator<L> {
    pub fn tree_id(&self) -> TreeInstanceId {
        self.tree
    }
    pub fn path(&self) -> &SyntaxPath {
        &self.path
    }
    pub fn expected_kind(&self) -> L::Kind {
        self.expected_kind
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forged_expected_kind_is_rejected() {}
}
