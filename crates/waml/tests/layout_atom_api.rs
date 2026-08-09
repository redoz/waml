use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    source::{BundlePath, SourceBundle},
    uml::{self, LayoutAtomSyntax, LayoutStatementSyntax},
};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode, SyntaxToken};

fn external_consumer(statement: &LayoutStatementSyntax) -> Vec<LayoutAtomSyntax> {
    statement.typed_atoms().collect()
}

fn find_statement(node: SyntaxNode<uml::syntax::UmlLanguage>) -> Option<LayoutStatementSyntax> {
    LayoutStatementSyntax::cast(node.clone()).or_else(|| {
        node.children()
            .filter_map(SyntaxElement::into_node)
            .find_map(find_statement)
    })
}

fn token_of(atom: &LayoutAtomSyntax) -> (&'static str, &SyntaxToken<uml::syntax::UmlLanguage>) {
    match atom {
        LayoutAtomSyntax::Word(token) => ("word", token),
        LayoutAtomSyntax::Link(token) => ("link", token),
        LayoutAtomSyntax::Quote(token) => ("quote", token),
        LayoutAtomSyntax::OpenParen(token) => ("open", token),
        LayoutAtomSyntax::CloseParen(token) => ("close", token),
        LayoutAtomSyntax::Comma(token) => ("comma", token),
    }
}

#[test]
fn public_layout_atom_views_preserve_leaf_kind_range_and_authored_order() {
    let diagram = "---\r\ntype: Diagram\r\n---\r\n# D\r\n\r\n## Layout\r\n- column of \"Café\", ([Order](../domain/order.md)) as row with frame, small margins left of Ghost\r\n";
    let source = SourceBundle::try_from_pairs([
        ("views/d.md", diagram),
        ("domain/order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ])
    .unwrap();
    let okf = analyze_okf(&source, None, 7).unwrap();
    let analysis = uml::analyze(
        DomainAnalysisContext {
            source: &source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 7,
        },
        None,
    )
    .unwrap();
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&BundlePath::parse("views/d.md").unwrap())
        .unwrap();
    let statement = find_statement(analysis.syntax.document(id).unwrap().syntax().root())
        .expect("typed layout statement");

    let atoms = statement.atoms().collect::<Vec<_>>();
    let typed = external_consumer(&statement);
    assert_eq!(atoms.len(), typed.len());
    let expected = [
        ("word", "column"),
        ("word", "of"),
        ("quote", "\"Café\""),
        ("comma", ","),
        ("open", "("),
        ("link", "[Order](../domain/order.md)"),
        ("close", ")"),
        ("word", "as"),
        ("word", "row"),
        ("word", "with"),
        ("word", "frame"),
        ("comma", ","),
        ("word", "small"),
        ("word", "margins"),
        ("word", "left"),
        ("word", "of"),
        ("word", "Ghost"),
    ];
    assert_eq!(typed.len(), expected.len());

    let mut previous_end = statement.syntax().range().start();
    for ((expected_kind, expected_text), (atom, untyped)) in
        expected.iter().zip(typed.iter().zip(&atoms))
    {
        let (kind, token) = token_of(atom);
        assert_eq!(kind, *expected_kind);
        assert_eq!(token.kind(), untyped.kind());
        assert_eq!(token.range(), untyped.range());
        assert!(token.range().start() >= previous_end);
        assert!(token.range().end() > token.range().start());
        assert!(token.range().end() <= statement.syntax().range().end());
        // The gap in front of an atom is leading trivia, so the trimmed range
        // covers exactly the bytes `text()` reports and nothing else.
        let trimmed = token.trimmed_range();
        let authored = &diagram[trimmed.start().to_usize()..trimmed.end().to_usize()];
        assert_eq!(token.text().write_to_string(), authored);
        assert_eq!(authored, *expected_text);
        previous_end = token.range().end();
    }
}
