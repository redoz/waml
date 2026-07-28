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
