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
/// Tree-bound location of one syntax occurrence.
///
/// The fields are deliberately private, so external code cannot forge or
/// mutate occurrence identity.
///
/// ```compile_fail
/// use waml_syntax::{SyntaxLanguage, SyntaxLocator, SyntaxPath};
/// # #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)] enum Kind { Root }
/// # #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)] enum Code { Error }
/// # struct Lang;
/// # impl SyntaxLanguage for Lang { type Kind = Kind; type DiagnosticCode = Code; }
/// let locator = SyntaxLocator::<Lang> {
///     tree: unsafe { std::mem::zeroed() },
///     path: SyntaxPath::from_indices([]),
///     expected_kind: Kind::Root,
/// };
/// ```
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
        let range = TextRange::new(
            TextSize::try_from_usize(0).expect("zero is representable"),
            self.root_green.width(),
        )
        .expect("root range is ordered");
        node(
            self.root_green.clone(),
            self.context.clone(),
            SyntaxPath::from_indices([]),
            None,
            range,
        )
    }
    pub fn root_green(&self) -> &GreenNode<L> {
        &self.root_green
    }
    pub fn diagnostics(&self) -> &[TreeDiagnostic<L::DiagnosticCode>] {
        &self.diagnostics
    }
    pub(crate) fn dialect(&self) -> MarkdownDialect {
        self.dialect
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
/// Total authored width of a run of trivia.  Trivia that cannot report a
/// width contributes nothing, which keeps a trimmed range inside the untrimmed
/// one rather than failing the caller.
fn trivia_width(trivia: &[crate::GreenTrivia]) -> usize {
    trivia
        .iter()
        .filter_map(|piece| piece.width().ok())
        .map(TextSize::to_usize)
        .sum()
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
    /// The node's range with the leading trivia of its first token and the
    /// trailing trivia of its last token excluded.  A node range covers the
    /// trivia its edge tokens own, so anything that paints over a node --
    /// a diagnostic squiggle, a selection -- needs the trimmed range to start
    /// on authored text instead of the whitespace in front of it.  Falls back
    /// to the full range for a node that owns no tokens.
    pub fn trimmed_range(&self) -> TextRange {
        let (Some(first), Some(last)) = (self.first_token(), self.last_token()) else {
            return self.0.range;
        };
        let lead = trivia_width(first.leading_trivia());
        let trail = trivia_width(last.trailing_trivia());
        let start = TextSize::new(first.range().start().to_usize().saturating_add(lead) as u32);
        let end = TextSize::new(last.range().end().to_usize().saturating_sub(trail) as u32);
        TextRange::new(start, end).unwrap_or(self.0.range)
    }
    /// The first token in document order beneath this node, if it owns one.
    pub fn first_token(&self) -> Option<SyntaxToken<L>> {
        self.children().find_map(|element| match element {
            SyntaxElement::Token(token) => Some(token),
            SyntaxElement::Node(node) => node.first_token(),
        })
    }
    /// The last token in document order beneath this node, if it owns one.
    pub fn last_token(&self) -> Option<SyntaxToken<L>> {
        self.children()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find_map(|element| match element {
                SyntaxElement::Token(token) => Some(token),
                SyntaxElement::Node(node) => node.last_token(),
            })
    }
    pub fn same_green(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0.green, &other.0.green)
    }
    pub fn syntax_annotations(&self) -> &[crate::SyntaxAnnotation] {
        self.0.green.annotations()
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
    /// The token's range with its own leading and trailing trivia excluded, so
    /// it covers exactly the bytes `text()` reports.
    pub fn trimmed_range(&self) -> TextRange {
        let lead = trivia_width(self.leading_trivia());
        let trail = trivia_width(self.trailing_trivia());
        let start = TextSize::new(self.0.range.start().to_usize().saturating_add(lead) as u32);
        let end = TextSize::new(self.0.range.end().to_usize().saturating_sub(trail) as u32);
        TextRange::new(start, end).unwrap_or(self.0.range)
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
    pub fn flags(&self) -> crate::TokenFlags {
        self.0.green.flags()
    }
    pub fn text(&self) -> &crate::GreenText {
        self.0.green.text()
    }
    pub fn leading_trivia(&self) -> &[crate::GreenTrivia] {
        self.0.green.leading_trivia()
    }
    pub fn trailing_trivia(&self) -> &[crate::GreenTrivia] {
        self.0.green.trailing_trivia()
    }
    pub fn syntax_annotations(&self) -> &[crate::SyntaxAnnotation] {
        self.0.green.syntax_annotations()
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
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    enum Kind {
        Root,
        Token,
        Other,
    }
    #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
    struct Code;
    struct Lang;
    impl SyntaxLanguage for Lang {
        type Kind = Kind;
        type DiagnosticCode = Code;
    }
    #[test]
    fn forged_expected_kind_is_rejected() {
        let factory = GreenFactory::<Lang>::new();
        let token = factory.missing_token(Kind::Token);
        let root = factory
            .node(Kind::Root, [GreenElement::Token(token)])
            .unwrap();
        let tree = SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
        let valid = tree.root().child_at(0).unwrap().locator();
        let forged = SyntaxLocator {
            tree: valid.tree,
            path: valid.path,
            expected_kind: Kind::Other,
        };
        assert!(matches!(
            tree.resolve(&forged),
            Err(RewriteError::KindMismatch {
                expected: Kind::Other,
                actual: Kind::Token
            })
        ));
    }

    /// `  ab  cd  ` as one node of two tokens, each owning the whitespace on
    /// either side of it as trivia.
    fn padded_tree() -> SyntaxTree<Lang> {
        let factory = GreenFactory::<Lang>::new();
        let space = |text: &'static str| {
            factory
                .trivia(
                    crate::TriviaKind::Whitespace,
                    crate::GreenText::Static(text),
                )
                .unwrap()
        };
        let word = |text: &'static str, lead: Vec<crate::GreenTrivia>, trail: Vec<_>| {
            GreenElement::Token(
                factory
                    .token(Kind::Token, crate::GreenText::Static(text), lead, trail)
                    .unwrap(),
            )
        };
        let root = factory
            .node(
                Kind::Root,
                [
                    word("ab", vec![space("  ")], vec![]),
                    word("cd", vec![space("  ")], vec![space("  ")]),
                ],
            )
            .unwrap();
        SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT)
    }

    #[test]
    fn trimmed_range_drops_edge_trivia_but_keeps_interior_gaps() {
        let root = padded_tree();
        let root = root.root();
        assert_eq!(root.range().start().to_usize(), 0);
        assert_eq!(root.range().end().to_usize(), 10);
        // Leading "  " and trailing "  " fall away; the gap between the two
        // words stays, because it is interior to the node.
        let trimmed = root.trimmed_range();
        assert_eq!(trimmed.start().to_usize(), 2);
        assert_eq!(trimmed.end().to_usize(), 8);
    }

    #[test]
    fn trimmed_range_of_a_token_covers_exactly_its_text() {
        let root = padded_tree();
        for element in root.root().children() {
            let token = element.into_token().expect("token child");
            let trimmed = token.trimmed_range();
            assert_eq!(
                trimmed.len().to_usize(),
                token.text().write_to_string().len()
            );
        }
    }

    #[test]
    fn trimmed_range_falls_back_to_the_full_range_without_tokens() {
        let factory = GreenFactory::<Lang>::new();
        let root = factory.node(Kind::Root, []).unwrap();
        let tree = SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
        assert_eq!(tree.root().trimmed_range(), tree.root().range());
    }
}
