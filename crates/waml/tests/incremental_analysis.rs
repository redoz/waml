use std::sync::Arc;

use waml::{
    analysis::{prepare_candidate, PreparedCandidate, PreviousAnalyses},
    host::replace_document,
    source::{BundlePath, SourceBundle, SourceDocument},
};
use waml_syntax::{GreenElement, GreenText, OkfMarkdownLanguage, SyntaxTree, TextSize};

fn prepared(
    source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    revision: u64,
) -> PreparedCandidate {
    prepare_candidate(source, previous, revision).unwrap()
}

fn document_id(candidate: &PreparedCandidate, path: &str) -> waml::analysis::DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse(path).unwrap())
        .unwrap()
}

fn shell_fingerprint(tree: &SyntaxTree<OkfMarkdownLanguage>) -> Vec<String> {
    fn visit(
        element: &GreenElement<OkfMarkdownLanguage>,
        at: TextSize,
        result: &mut Vec<String>,
    ) -> TextSize {
        match element {
            GreenElement::Node(node) => {
                let end = at.checked_add(node.width()).unwrap();
                result.push(format!("node:{:?}:{at:?}..{end:?}", node.kind()));
                node.children()
                    .iter()
                    .fold(at, |offset, child| visit(child, offset, result))
            }
            GreenElement::Token(token) => {
                let end = at.checked_add(token.width()).unwrap();
                let text = match token.text() {
                    GreenText::Static(value) => format!("static:{value}"),
                    GreenText::Owned(value) => format!("owned:{value}"),
                    GreenText::SourceSlice { range, .. } => {
                        format!("source:{range:?}:{}", token.text().write_to_string())
                    }
                };
                result.push(format!(
                    "token:{:?}:{at:?}..{end:?}:{text}:missing={}:bad={}",
                    token.kind(),
                    token.flags().is_missing(),
                    token.flags().is_bad(),
                ));
                end
            }
        }
    }

    let mut result = Vec::new();
    visit(
        &GreenElement::Node(tree.root_green().clone()),
        TextSize::try_from_usize(0).unwrap(),
        &mut result,
    );
    result
}

fn diagnostic_fingerprint(tree: &SyntaxTree<OkfMarkdownLanguage>) -> Vec<String> {
    tree.diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{:?}:{:?}:{:?}:{}",
                diagnostic.code, diagnostic.severity, diagnostic.range, diagnostic.message
            )
        })
        .collect()
}

#[test]
fn retained_okf_analysis_reuses_unchanged_snapshots_and_matches_full_oracle() {
    let source = SourceBundle::try_from_pairs([
        ("touched.md", "# Before\nraw text\n"),
        ("untouched.md", "# Untouched\nother text\n"),
    ])
    .unwrap();
    let baseline = prepared(source.clone(), None, 1);
    let identical = prepared(
        source.clone(),
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );

    for path in ["touched.md", "untouched.md"] {
        let id = document_id(&baseline, path);
        assert!(Arc::ptr_eq(
            baseline.okf().shell.document(id).unwrap(),
            identical.okf().shell.document(id).unwrap(),
        ));
        assert!(Arc::ptr_eq(
            baseline.okf().shell.document(id).unwrap().syntax(),
            identical.okf().shell.document(id).unwrap().syntax(),
        ));
        assert!(Arc::ptr_eq(
            baseline.okf().structures.get(&id).unwrap(),
            identical.okf().structures.get(&id).unwrap(),
        ));
    }

    let edited = replace_document(
        &source,
        SourceDocument::new(
            BundlePath::parse("touched.md").unwrap(),
            "# After!\nraw text\n".into(),
        ),
    )
    .unwrap();
    let incremental = prepared(
        edited.clone(),
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );
    let full = prepared(edited, None, 2);

    let touched = document_id(&baseline, "touched.md");
    let untouched = document_id(&baseline, "untouched.md");
    assert!(Arc::ptr_eq(
        baseline.okf().shell.document(untouched).unwrap(),
        incremental.okf().shell.document(untouched).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        baseline.okf().shell.document(untouched).unwrap().syntax(),
        incremental
            .okf()
            .shell
            .document(untouched)
            .unwrap()
            .syntax(),
    ));
    assert!(Arc::ptr_eq(
        baseline.okf().structures.get(&untouched).unwrap(),
        incremental.okf().structures.get(&untouched).unwrap(),
    ));
    assert_ne!(
        incremental
            .okf()
            .catalog
            .document(touched)
            .unwrap()
            .revision(),
        baseline.okf().catalog.document(touched).unwrap().revision(),
    );
    assert!(!Arc::ptr_eq(
        incremental.okf().catalog.document(touched).unwrap(),
        baseline.okf().catalog.document(touched).unwrap(),
    ));
    assert!(!incremental
        .okf()
        .shell
        .document(touched)
        .unwrap()
        .syntax()
        .root()
        .same_green(
            &baseline
                .okf()
                .shell
                .document(touched)
                .unwrap()
                .syntax()
                .root()
        ));

    let old_children = baseline
        .okf()
        .shell
        .document(touched)
        .unwrap()
        .syntax()
        .root()
        .children()
        .collect::<Vec<_>>();
    let new_children = incremental
        .okf()
        .shell
        .document(touched)
        .unwrap()
        .syntax()
        .root()
        .children()
        .collect::<Vec<_>>();
    assert!(old_children
        .last()
        .unwrap()
        .clone()
        .into_token()
        .unwrap()
        .same_green(&new_children.last().unwrap().clone().into_token().unwrap()));
    assert!(!old_children[0]
        .clone()
        .into_node()
        .unwrap()
        .same_green(&new_children[0].clone().into_node().unwrap()));
    assert!(!old_children[1]
        .clone()
        .into_node()
        .unwrap()
        .same_green(&new_children[1].clone().into_node().unwrap()));

    let touched_source = incremental
        .source()
        .document(&BundlePath::parse("touched.md").unwrap())
        .unwrap()
        .text();
    let touched_slices = incremental
        .okf()
        .bundle
        .concepts()
        .iter()
        .filter(|concept| concept.id == "touched")
        .map(|concept| concept.body.as_str())
        .collect::<Vec<_>>();
    assert!(!touched_slices.is_empty());
    assert!(touched_slices
        .iter()
        .all(|slice| slice.as_ptr() == touched_source.as_ptr()));

    assert_eq!(incremental.okf().bundle, full.okf().bundle);
    for path in ["touched.md", "untouched.md"] {
        let incremental_id = document_id(&incremental, path);
        let full_id = document_id(&full, path);
        let incremental_tree = incremental
            .okf()
            .shell
            .document(incremental_id)
            .unwrap()
            .syntax();
        let full_tree = full.okf().shell.document(full_id).unwrap().syntax();
        assert_eq!(
            shell_fingerprint(incremental_tree),
            shell_fingerprint(full_tree)
        );
        assert_eq!(
            diagnostic_fingerprint(incremental_tree),
            diagnostic_fingerprint(full_tree)
        );
    }
}
