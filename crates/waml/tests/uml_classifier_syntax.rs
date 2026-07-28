use waml::{analysis::analyze_okf, source::SourceBundle, uml};
use waml_syntax::AstNode;

fn contains<T: AstNode<waml::uml::syntax::UmlLanguage>>(
    node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
) -> bool {
    T::cast(node.clone()).is_some()
        || node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
            .any(contains::<T>)
}

fn analyze(source: &SourceBundle) -> uml::Analysis {
    let okf = analyze_okf(source, None, 1).unwrap();
    uml::analyze(
        waml::analysis::DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            shell: &okf.shell,
            structures: &okf.structures,
            okf: &okf.bundle,
            session_revision: 1,
        },
        None,
    )
    .unwrap()
}

#[test]
fn classifier_sections_are_lossless_and_expose_fixed_typed_slots() {
    let authored = "---\r\ntype: uml.Class\r\n---\r\n# Café\r\n\r\n## Values\r\n- OPEN\r\n\r\n## Slots\r\n- status: \"OPEN\"\r\n\r\n## Relationships\r\n- depends [Customer](./customer.md)\r\n\r\n## Members\r\n### People\r\n- [Customer](./customer.md)\r\n- instance of [Customer](./customer.md) as primary with status set to OPEN\r\n\r\n## Operations\r\n- must remain Markdown\r\n";
    let source = SourceBundle::try_from_pairs([
        ("cafe.md", authored),
        ("customer.md", "---\ntype: uml.Class\n---\n# Customer\n"),
    ])
    .unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("cafe.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    assert!(contains::<uml::ValueSyntax>(root.clone()));
    assert!(contains::<uml::SlotSyntax>(root.clone()));
    assert!(contains::<uml::RelationshipSyntax>(root.clone()));
    assert!(contains::<uml::MemberSyntax>(root.clone()));
    assert!(contains::<uml::InlineInstanceSyntax>(root));
    assert_eq!(
        analysis
            .syntax
            .document(id)
            .unwrap()
            .syntax()
            .write_to_string(),
        authored
    );
}

#[test]
fn classifier_items_do_not_hide_authored_grammar_in_raw_markdown_tokens() {
    let source = SourceBundle::try_from_pairs([
        ("class.md", "---\ntype: uml.Class\n---\n# Class\n\n## Values\n- READY\n\n## Slots\n- state: \"ready\"\n\n## Relationships\n- depends [Other](./other.md)\n\n## Members\n- [Other](./other.md)\n"),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ]).unwrap();
    let analysis = analyze(&source);
    let id = analysis
        .syntax
        .catalog()
        .id_for_path(&waml::source::BundlePath::parse("class.md").unwrap())
        .unwrap();
    let root = analysis.syntax.document(id).unwrap().syntax().root();
    fn typed_nodes_have_no_raw(node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>) {
        let typed = matches!(
            node.kind(),
            waml::uml::syntax::UmlSyntaxKind::Value
                | waml::uml::syntax::UmlSyntaxKind::Slot
                | waml::uml::syntax::UmlSyntaxKind::Relationship
                | waml::uml::syntax::UmlSyntaxKind::Member
                | waml::uml::syntax::UmlSyntaxKind::InlineInstance
        );
        if typed {
            assert!(!node
                .children()
                .any(|e| e.kind() == waml::uml::syntax::UmlSyntaxKind::RawMarkdownToken));
        }
        for child in node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
        {
            typed_nodes_have_no_raw(child);
        }
    }
    typed_nodes_have_no_raw(root);
}
