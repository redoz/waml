use std::{num::NonZeroU64, sync::Arc};

use waml_syntax::{
    annotate_occurrence, find_annotation, AstNode, GreenElement, GreenFactory, GreenText,
    MarkdownDialect, SyntaxAnnotation, SyntaxElement, SyntaxLanguage, SyntaxRewriter, SyntaxTree,
    SyntaxVisitor, TextSize,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Kind {
    Root,
    Pair,
    Name,
    Colon,
    Value,
    Recovery,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Code {
    Bad,
}
#[derive(Debug, Eq, PartialEq)]
struct Lang;
impl SyntaxLanguage for Lang {
    type Kind = Kind;
    type DiagnosticCode = Code;
}

struct Pair(waml_syntax::SyntaxNode<Lang>);
impl AstNode<Lang> for Pair {
    fn can_cast(kind: Kind) -> bool {
        kind == Kind::Pair
    }
    fn cast(node: waml_syntax::SyntaxNode<Lang>) -> Option<Self> {
        Self::can_cast(node.kind()).then_some(Self(node))
    }
    fn syntax(&self) -> &waml_syntax::SyntaxNode<Lang> {
        &self.0
    }
}

fn tree() -> SyntaxTree<Lang> {
    let f = GreenFactory::<Lang>::new();
    let name = f
        .token(Kind::Name, GreenText::Static("name"), [], [])
        .unwrap();
    let colon = f.missing_token(Kind::Colon);
    let pair = f
        .node(
            Kind::Pair,
            [
                GreenElement::Token(name),
                GreenElement::Token(colon.clone()),
            ],
        )
        .unwrap();
    let sibling = f.node(Kind::Pair, [GreenElement::Token(colon)]).unwrap();
    let root = f
        .node(
            Kind::Root,
            [GreenElement::Node(pair), GreenElement::Node(sibling)],
        )
        .unwrap();
    SyntaxTree::new(root, Arc::from([]), MarkdownDialect::CommonMarkCurrent)
}

#[test]
fn shared_zero_width_occurrences_have_distinct_red_identity_and_locators() {
    let tree = tree();
    let root = tree.root();
    let first = root.children().next().unwrap().into_node().unwrap();
    let second = root.children().nth(1).unwrap().into_node().unwrap();
    let first_colon = first.children().nth(1).unwrap().into_token().unwrap();
    let second_colon = second.children().next().unwrap().into_token().unwrap();
    assert_eq!(
        first_colon.range().start(),
        TextSize::try_from_usize(4).unwrap()
    );
    assert_eq!(first_colon.range(), second_colon.range());
    assert_ne!(first_colon, second_colon);
    assert!(first_colon.same_green(&second_colon));
    assert_eq!(first_colon.parent().unwrap(), first);
    assert_eq!(
        first_colon
            .previous_sibling()
            .unwrap()
            .into_token()
            .unwrap()
            .kind(),
        Kind::Name
    );
    let left = first_colon.locator();
    let right = second_colon.locator();
    assert_eq!(left.tree_id(), right.tree_id());
    assert_eq!(left.expected_kind(), right.expected_kind());
    assert_ne!(left.path(), right.path());
    assert_ne!(left, right);
    assert_eq!(
        tree.resolve(&left).unwrap().into_token().unwrap(),
        first_colon
    );
    assert_eq!(
        tree.resolve(&right).unwrap().into_token().unwrap(),
        second_colon
    );
}

#[test]
fn locators_are_tree_bound_for_resolution_and_annotations() {
    let tree = tree();
    let locator = tree
        .root()
        .children()
        .next()
        .unwrap()
        .into_node()
        .unwrap()
        .locator();
    let other = SyntaxTree::new(
        tree.root_green().clone(),
        Arc::from([]),
        MarkdownDialect::CommonMarkCurrent,
    );
    assert!(matches!(
        other.resolve(&locator),
        Err(waml_syntax::RewriteError::WrongTree { .. })
    ));
    let annotation = SyntaxAnnotation::new(NonZeroU64::new(7).unwrap(), "tracked", None);
    assert!(matches!(
        annotate_occurrence(&other, &locator, annotation),
        Err(waml_syntax::RewriteError::WrongTree { .. })
    ));
}

#[test]
fn visitor_rewriter_slots_and_annotations_use_declared_occurrences() {
    let tree = tree();
    let pair = Pair::cast(tree.root().children().next().unwrap().into_node().unwrap()).unwrap();
    assert_eq!(
        pair.syntax()
            .child_at(0)
            .unwrap()
            .into_token()
            .unwrap()
            .kind(),
        Kind::Name
    );
    assert_eq!(
        pair.syntax()
            .child_at(1)
            .unwrap()
            .into_token()
            .unwrap()
            .kind(),
        Kind::Colon
    );
    let mut kinds = Vec::new();
    struct Visitor<'a>(&'a mut Vec<Kind>);
    impl SyntaxVisitor<Lang> for Visitor<'_> {
        fn visit(&mut self, e: SyntaxElement<Lang>) {
            self.0.push(e.kind());
        }
    }
    tree.root().walk(&mut Visitor(&mut kinds));
    assert_eq!(
        kinds,
        vec![
            Kind::Root,
            Kind::Pair,
            Kind::Name,
            Kind::Colon,
            Kind::Pair,
            Kind::Colon
        ]
    );
    struct Replace;
    impl SyntaxRewriter<Lang> for Replace {
        fn rewrite_node(
            &mut self,
            node: &waml_syntax::GreenNode<Lang>,
        ) -> waml_syntax::GreenNode<Lang> {
            node.clone()
        }
        fn rewrite_token(
            &mut self,
            token: &waml_syntax::GreenToken<Lang>,
        ) -> waml_syntax::GreenToken<Lang> {
            token.clone()
        }
    }
    let rewritten = tree.rewrite(&mut Replace);
    assert!(Arc::ptr_eq(tree.root_green(), rewritten.root_green()));
    let annotation = SyntaxAnnotation::new(
        NonZeroU64::new(8).unwrap(),
        "tracked",
        Some(Arc::from("data")),
    );
    let annotated = annotate_occurrence(&tree, &pair.syntax().locator(), annotation).unwrap();
    let annotated_tree =
        SyntaxTree::new(annotated, Arc::from([]), MarkdownDialect::CommonMarkCurrent);
    assert_eq!(
        find_annotation(&annotated_tree, NonZeroU64::new(8).unwrap()).len(),
        1
    );
}
