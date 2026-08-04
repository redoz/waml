use std::{num::NonZeroU64, sync::Arc};

use waml_syntax::{
    annotate_occurrence, find_annotation, AstNode, AstSlots, GreenElement, GreenFactory, GreenText,
    MarkdownDialect, SyntaxAnnotation, SyntaxElement, SyntaxLanguage, SyntaxRewriter, SyntaxTree,
    SyntaxVisitor, TextRange, TextSize,
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
struct Code;
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
    SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT)
}

#[test]
fn shared_zero_width_occurrences_have_distinct_red_identity_and_locators() {
    let tree = tree();
    let root = tree.root();
    let first = root.children().next().unwrap().into_node().unwrap();
    let second = root.children().nth(1).unwrap().into_node().unwrap();
    assert!(first.same_green(&tree.root().children().next().unwrap().into_node().unwrap()));
    assert!(!first.same_green(&second));
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
fn root_range_covers_full_width_and_empty_tree() {
    let tree = tree();
    assert_eq!(
        tree.root().range(),
        TextRange::new(
            TextSize::try_from_usize(0).unwrap(),
            TextSize::try_from_usize(4).unwrap()
        )
        .unwrap()
    );
    let empty = GreenFactory::<Lang>::new().node(Kind::Root, []).unwrap();
    let empty = SyntaxTree::new(empty, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
    assert_eq!(
        empty.root().range().len(),
        TextSize::try_from_usize(0).unwrap()
    );
}

#[test]
fn token_annotations_target_only_the_resolved_shared_occurrence() {
    let tree = tree();
    let left = tree
        .root()
        .child_at(0)
        .unwrap()
        .into_node()
        .unwrap()
        .child_at(1)
        .unwrap()
        .into_token()
        .unwrap();
    let _right = tree
        .root()
        .child_at(1)
        .unwrap()
        .into_node()
        .unwrap()
        .child_at(0)
        .unwrap()
        .into_token()
        .unwrap();
    let green = annotate_occurrence(
        &tree,
        &left.locator(),
        SyntaxAnnotation::new(NonZeroU64::new(11).unwrap(), "token", None),
    )
    .unwrap();
    let updated = SyntaxTree::new(green, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
    let left = updated
        .root()
        .child_at(0)
        .unwrap()
        .into_node()
        .unwrap()
        .child_at(1)
        .unwrap()
        .into_token()
        .unwrap();
    let right = updated
        .root()
        .child_at(1)
        .unwrap()
        .into_node()
        .unwrap()
        .child_at(0)
        .unwrap()
        .into_token()
        .unwrap();
    assert_eq!(
        left.syntax_annotations()[0].id(),
        NonZeroU64::new(11).unwrap()
    );
    assert!(right.syntax_annotations().is_empty());
    assert!(!left.same_green(&right));
}

#[test]
fn ast_slots_use_declared_indices_for_all_slot_categories() {
    struct DeclaredPair(waml_syntax::SyntaxNode<Lang>);
    impl DeclaredPair {
        const NAME: usize = 0;
        const COLON: usize = 1;
        const VALUE: usize = 2;
        const ITEMS: std::ops::Range<usize> = 3..4;
        const RECOVERY: usize = 4;
        const TRAILING: usize = 5;
        fn slots(&self) -> AstSlots<'_, Lang> {
            AstSlots::new(&self.0)
        }
        fn name(&self) -> waml_syntax::SyntaxToken<Lang> {
            self.slots().required_token(Self::NAME).unwrap()
        }
        fn colon(&self) -> waml_syntax::SyntaxToken<Lang> {
            self.slots().required_token(Self::COLON).unwrap()
        }
        fn value(&self) -> Option<waml_syntax::SyntaxToken<Lang>> {
            self.slots().required_token(Self::VALUE)
        }
        fn items(&self) -> Vec<SyntaxElement<Lang>> {
            self.slots().list(Self::ITEMS)
        }
        fn recovery(&self) -> waml_syntax::SyntaxNode<Lang> {
            self.slots().required_node(Self::RECOVERY).unwrap()
        }
        fn trailing(&self) -> waml_syntax::SyntaxToken<Lang> {
            self.slots().required_token(Self::TRAILING).unwrap()
        }
    }
    fn declared_pair(skipped_count: usize) -> DeclaredPair {
        let f = GreenFactory::<Lang>::new();
        let token = |kind, text: &str| {
            GreenElement::Token(
                f.token(kind, GreenText::Owned(Arc::from(text)), [], [])
                    .unwrap(),
            )
        };
        let list = f.node(Kind::Value, [token(Kind::Value, "item")]).unwrap();
        let recovery = f
            .node(
                Kind::Recovery,
                (0..skipped_count).map(|_| token(Kind::Value, "?")),
            )
            .unwrap();
        let pair = f
            .node(
                Kind::Pair,
                [
                    token(Kind::Name, "name"),
                    token(Kind::Colon, ":"),
                    token(Kind::Value, "value"),
                    GreenElement::Node(list),
                    GreenElement::Node(recovery),
                    token(Kind::Name, "trailing"),
                ],
            )
            .unwrap();
        let root = f.node(Kind::Root, [GreenElement::Node(pair)]).unwrap();
        let tree = SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
        DeclaredPair(tree.root().child_at(0).unwrap().into_node().unwrap())
    }
    let zero = declared_pair(0);
    let multiple = declared_pair(3);
    for pair in [&zero, &multiple] {
        assert_eq!(pair.slots().len(), 6);
        assert_eq!(
            pair.name().locator().path(),
            pair.0
                .child_at(DeclaredPair::NAME)
                .unwrap()
                .locator()
                .path()
        );
        assert_eq!(
            pair.colon().locator().path(),
            pair.0
                .child_at(DeclaredPair::COLON)
                .unwrap()
                .locator()
                .path()
        );
        assert_eq!(
            pair.value().unwrap().locator().path(),
            pair.0
                .child_at(DeclaredPair::VALUE)
                .unwrap()
                .locator()
                .path()
        );
        assert_eq!(
            pair.items()[0].locator().path(),
            pair.0
                .child_at(DeclaredPair::ITEMS.start)
                .unwrap()
                .locator()
                .path()
        );
        assert_eq!(
            pair.recovery().locator().path(),
            pair.0
                .child_at(DeclaredPair::RECOVERY)
                .unwrap()
                .locator()
                .path()
        );
        assert_eq!(
            pair.trailing().locator().path(),
            pair.0
                .child_at(DeclaredPair::TRAILING)
                .unwrap()
                .locator()
                .path()
        );
    }
    assert_eq!(zero.recovery().children().count(), 0);
    assert_eq!(multiple.recovery().children().count(), 3);
    assert_eq!(zero.trailing().kind(), multiple.trailing().kind());
}

#[test]
fn ast_slots_list_truncates_past_the_end_and_is_empty_for_an_empty_range() {
    let tree = tree();
    let root = tree.root();
    let slots = AstSlots::new(&root);
    assert_eq!(slots.len(), 2);
    let all = slots.list(0..2);
    let past_end = slots.list(0..10);
    assert_eq!(all.len(), 2);
    assert_eq!(past_end.len(), 2);
    for (a, b) in all.iter().zip(past_end.iter()) {
        assert_eq!(a.locator().path(), b.locator().path());
    }
    assert!(slots.list(2..2).is_empty());
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
        MarkdownDialect::WAML_DEFAULT,
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
    let annotated_tree = SyntaxTree::new(annotated, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
    assert_eq!(
        find_annotation(&annotated_tree, NonZeroU64::new(8).unwrap()).len(),
        1
    );
}

#[test]
fn one_token_rewrite_rebuilds_ancestors_and_retains_sibling_annotation_and_green() {
    let tree = tree();
    let sibling = tree.root().child_at(1).unwrap().into_node().unwrap();
    let annotated = annotate_occurrence(
        &tree,
        &sibling.locator(),
        SyntaxAnnotation::new(NonZeroU64::new(12).unwrap(), "retained", None),
    )
    .unwrap();
    let tree = SyntaxTree::new(annotated, Arc::from([]), MarkdownDialect::WAML_DEFAULT);
    let old_root = tree.root();
    let old_changed = old_root.child_at(0).unwrap().into_node().unwrap();
    let old_sibling = old_root.child_at(1).unwrap().into_node().unwrap();
    struct ReplaceName;
    impl SyntaxRewriter<Lang> for ReplaceName {
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
            if token.kind() == Kind::Name {
                GreenFactory::new()
                    .token(Kind::Name, GreenText::Static("NAME"), [], [])
                    .unwrap()
            } else {
                token.clone()
            }
        }
    }
    let rewritten = tree.rewrite(&mut ReplaceName);
    let new_root = rewritten.root();
    let new_changed = new_root.child_at(0).unwrap().into_node().unwrap();
    let new_sibling = new_root.child_at(1).unwrap().into_node().unwrap();
    assert!(!old_root.same_green(&new_root));
    assert!(!old_changed.same_green(&new_changed));
    assert!(old_sibling.same_green(&new_sibling));
    assert_eq!(
        find_annotation(&rewritten, NonZeroU64::new(12).unwrap()).len(),
        1
    );
}
