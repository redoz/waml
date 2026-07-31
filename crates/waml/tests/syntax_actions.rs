use std::sync::Arc;

use waml::{
    action::{
        ActionBasis, ActionError, CodeAction, SyntaxChangeBatch, TextEdit, VersionedDocumentChange,
        VersionedSyntaxLocator,
    },
    analysis::{analyze_okf, DocumentId, DocumentRevision, DomainAnalysisContext},
    edit::{EditBatch, EditContext},
    source::{BundlePath, SourceBundle},
    uml,
};
use waml_syntax::{RewriteError, TextRange, TextSize};

struct Analyses {
    okf: waml::analysis::OkfAnalysis,
    uml: uml::Analysis,
}

fn build_analyses(source: &SourceBundle, previous: Option<&Analyses>, revision: u64) -> Analyses {
    let okf = analyze_okf(source, previous.map(|value| &value.okf), revision).unwrap();
    let uml = uml::analyze(
        DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: revision,
        },
        previous.map(|value| &value.uml),
    )
    .unwrap();
    Analyses { okf, uml }
}

fn source() -> SourceBundle {
    SourceBundle::try_from_pairs([
        ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\nCafé\n"),
        ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\nBody\n"),
        ("notes.md", "# Notes\nUntouched\n"),
    ])
    .unwrap()
}

fn id(analyses: &Analyses, path: &str) -> DocumentId {
    analyses
        .okf
        .catalog
        .id_for_path(&BundlePath::parse(path).unwrap())
        .unwrap()
}

fn revision(analyses: &Analyses, document: DocumentId) -> DocumentRevision {
    analyses.okf.catalog.document(document).unwrap().revision()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}

fn change(
    document: DocumentId,
    base_document_revision: DocumentRevision,
    edits: impl Into<Vec<TextEdit>>,
) -> VersionedDocumentChange {
    VersionedDocumentChange {
        document,
        base_document_revision,
        edits: edits.into().into(),
    }
}

fn edit(start: usize, end: usize, replacement: &str) -> TextEdit {
    TextEdit {
        range: range(start, end),
        replacement: Arc::from(replacement),
    }
}

fn bundle_action(revision: u64, changes: Vec<VersionedDocumentChange>) -> CodeAction {
    CodeAction {
        title: "test action".into(),
        basis: ActionBasis::Bundle {
            session_revision: revision,
        },
        changes: changes.into(),
    }
}

fn lower(
    source: &SourceBundle,
    analyses: &Analyses,
    session_revision: u64,
    action: CodeAction,
) -> Result<SourceBundle, waml::edit::EditError> {
    SyntaxChangeBatch::new(action)
        .map_err(waml::edit::EditError::from)?
        .lower(EditContext {
            source,
            okf_analysis: &analyses.okf,
            session_revision,
            uml: &analyses.uml,
        })
}

#[test]
fn constructor_sorts_edits_and_lowering_applies_them_from_the_end() {
    let source = source();
    let analyses = build_analyses(&source, None, 7);
    let document = id(&analyses, "notes.md");
    let action = bundle_action(
        7,
        vec![change(
            document,
            revision(&analyses, document),
            vec![edit(8, 8, "A"), edit(0, 1, "##")],
        )],
    );

    let batch = SyntaxChangeBatch::new(action).unwrap();
    let stored = &batch.action().changes[0].edits;
    assert_eq!(stored[0].range, range(0, 1));
    assert_eq!(stored[1].range, range(8, 8));
    let candidate = batch
        .lower(EditContext {
            source: &source,
            okf_analysis: &analyses.okf,
            session_revision: 7,
            uml: &analyses.uml,
        })
        .unwrap();

    assert_eq!(
        candidate
            .document(&BundlePath::parse("notes.md").unwrap())
            .unwrap()
            .text(),
        "## Notes\nAUntouched\n"
    );
}

#[test]
fn constructor_rejects_overlaps_duplicate_documents_and_document_scope_escape() {
    let source = source();
    let analyses = build_analyses(&source, None, 3);
    let a = id(&analyses, "a.md");
    let b = id(&analyses, "b.md");
    let a_revision = revision(&analyses, a);
    let b_revision = revision(&analyses, b);

    let overlap = SyntaxChangeBatch::new(bundle_action(
        3,
        vec![change(
            a,
            a_revision,
            vec![edit(0, 3, "x"), edit(2, 4, "y")],
        )],
    ))
    .unwrap_err();
    assert!(matches!(overlap, ActionError::Overlap { document, .. } if document == a));

    let duplicate = SyntaxChangeBatch::new(bundle_action(
        3,
        vec![change(a, a_revision, vec![]), change(a, a_revision, vec![])],
    ))
    .unwrap_err();
    assert!(matches!(duplicate, ActionError::StructuralInvariant { .. }));

    let escaped = SyntaxChangeBatch::new(CodeAction {
        title: "scoped".into(),
        basis: ActionBasis::Document {
            document: a,
            document_revision: a_revision,
            session_revision: 3,
        },
        changes: vec![change(b, b_revision, vec![])].into(),
    })
    .unwrap_err();
    assert_eq!(escaped, ActionError::BasisScope { document: b });
}

#[test]
fn empty_action_is_a_noop_and_preserves_all_source_allocations() {
    let source = source();
    let analyses = build_analyses(&source, None, 11);
    let candidate = lower(&source, &analyses, 11, bundle_action(11, vec![])).unwrap();

    assert_eq!(candidate, source);
    for path in ["a.md", "b.md", "notes.md"] {
        assert!(candidate.shares_text_with(&source, path), "{path}");
    }
}

#[test]
fn empty_document_change_preserves_source_catalog_and_document_identity() {
    let source = source();
    let analyses = build_analyses(&source, None, 12);
    let a = id(&analyses, "a.md");
    let catalog = analyses.okf.catalog.clone();
    let document = catalog.document(a).unwrap().clone();
    let action = bundle_action(12, vec![change(a, revision(&analyses, a), vec![])]);

    let candidate = lower(&source, &analyses, 12, action).unwrap();

    assert!(candidate.shares_text_with(&source, "a.md"));
    assert!(Arc::ptr_eq(&catalog, &analyses.okf.catalog));
    assert!(Arc::ptr_eq(
        &document,
        analyses.okf.catalog.document(a).unwrap()
    ));
    assert!(Arc::ptr_eq(
        document.text().shared(),
        analyses.okf.catalog.document(a).unwrap().text().shared()
    ));
}

#[test]
fn mixed_empty_and_edited_changes_detach_only_the_edited_document() {
    let source = source();
    let analyses = build_analyses(&source, None, 14);
    let a = id(&analyses, "a.md");
    let b = id(&analyses, "b.md");
    let action = bundle_action(
        14,
        vec![
            change(a, revision(&analyses, a), vec![]),
            change(b, revision(&analyses, b), vec![edit(0, 1, "##")]),
        ],
    );

    let candidate = lower(&source, &analyses, 14, action).unwrap();

    assert!(candidate.shares_text_with(&source, "a.md"));
    assert!(!candidate.shares_text_with(&source, "b.md"));
    assert!(candidate.shares_text_with(&source, "notes.md"));
}

#[test]
fn lowering_rejects_unknown_stale_out_of_bounds_and_non_utf8_edits() {
    let source = source();
    let analyses = build_analyses(&source, None, 5);
    let a = id(&analyses, "a.md");
    let a_revision = revision(&analyses, a);
    let current_analyses = build_analyses(&source, Some(&analyses), 6);

    let stale_session = lower(
        &source,
        &current_analyses,
        6,
        bundle_action(5, vec![change(a, a_revision, vec![])]),
    )
    .unwrap_err();
    assert!(stale_session.reason.contains("StaleSession"));

    let changed_source = SourceBundle::try_from_pairs([
        (
            "a.md",
            "---\ntype: uml.Class\ntitle: A\n---\n# A changed\nCafé\n",
        ),
        ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\nBody\n"),
        ("notes.md", "# Notes\nUntouched\n"),
    ])
    .unwrap();
    let changed_analyses = build_analyses(&changed_source, Some(&analyses), 5);
    let changed_revision = revision(&changed_analyses, a);
    let stale_document = lower(
        &source,
        &analyses,
        5,
        bundle_action(5, vec![change(a, changed_revision, vec![])]),
    )
    .unwrap_err();
    assert!(stale_document.reason.contains("StaleDocument"));

    let source_with_extra = SourceBundle::try_from_pairs([
        ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\nCafé\n"),
        ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\nBody\n"),
        ("notes.md", "# Notes\nUntouched\n"),
        ("extra.md", "# Extra\n"),
    ])
    .unwrap();
    let extra_analyses = build_analyses(&source_with_extra, Some(&analyses), 5);
    let extra = id(&extra_analyses, "extra.md");
    let unknown = lower(
        &source,
        &analyses,
        5,
        bundle_action(
            5,
            vec![change(extra, revision(&extra_analyses, extra), vec![])],
        ),
    )
    .unwrap_err();
    assert!(unknown.reason.contains("UnknownDocument"));

    let invalid = lower(
        &source,
        &analyses,
        5,
        bundle_action(
            5,
            vec![change(
                a,
                a_revision,
                vec![edit(0, usize::MAX as u32 as usize, "")],
            )],
        ),
    )
    .unwrap_err();
    assert!(invalid.reason.contains("InvalidRange"));

    let text = source
        .document(&BundlePath::parse("a.md").unwrap())
        .unwrap()
        .text();
    let inside_e_acute = text.find('é').unwrap() + 1;
    let non_utf8 = lower(
        &source,
        &analyses,
        5,
        bundle_action(
            5,
            vec![change(
                a,
                a_revision,
                vec![edit(inside_e_acute, inside_e_acute, "")],
            )],
        ),
    )
    .unwrap_err();
    assert!(non_utf8.reason.contains("NonUtf8Boundary"));
}

#[test]
fn multi_document_failure_is_atomic_and_success_detaches_only_touched_documents() {
    let source = source();
    let original = source.clone();
    let analyses = build_analyses(&source, None, 9);
    let a = id(&analyses, "a.md");
    let b = id(&analyses, "b.md");
    let failing = bundle_action(
        9,
        vec![
            change(a, revision(&analyses, a), vec![edit(0, 1, "##")]),
            change(b, revision(&analyses, b), vec![edit(10_000, 10_000, "!")]),
        ],
    );

    assert!(lower(&source, &analyses, 9, failing).is_err());
    for path in ["a.md", "b.md", "notes.md"] {
        assert!(source.shares_text_with(&original, path), "{path}");
    }

    let success = bundle_action(
        9,
        vec![
            change(a, revision(&analyses, a), vec![edit(0, 1, "##")]),
            change(b, revision(&analyses, b), vec![edit(0, 1, "##")]),
        ],
    );
    let candidate = lower(&source, &analyses, 9, success).unwrap();
    assert!(!candidate.shares_text_with(&source, "a.md"));
    assert!(!candidate.shares_text_with(&source, "b.md"));
    assert!(candidate.shares_text_with(&source, "notes.md"));
}

#[test]
fn lowering_rejects_mismatched_source_catalog_and_analysis_revision() {
    let source = source();
    let analyses = build_analyses(&source, None, 21);
    let action = bundle_action(21, vec![]);
    let rebuilt_source = SourceBundle::try_from_pairs(source.to_pairs()).unwrap();
    let mismatched_source = SyntaxChangeBatch::new(action.clone())
        .unwrap()
        .lower(EditContext {
            source: &rebuilt_source,
            okf_analysis: &analyses.okf,
            session_revision: 21,
            uml: &analyses.uml,
        })
        .unwrap_err();
    assert!(mismatched_source.reason.contains("MismatchedCatalog"));

    let mismatched_revision = SyntaxChangeBatch::new(action)
        .unwrap()
        .lower(EditContext {
            source: &source,
            okf_analysis: &analyses.okf,
            session_revision: 22,
            uml: &analyses.uml,
        })
        .unwrap_err();
    assert_eq!(mismatched_revision.op, "syntax.action");
    assert!(mismatched_revision
        .reason
        .contains("MismatchedAnalysisRevision"));
}

#[test]
fn action_and_analysis_errors_convert_to_sealed_edit_errors() {
    let analyses_error = analyze_okf(&source(), None, u64::MAX).err().unwrap_or(
        waml::analysis::AnalysisError::StructuralInvariant {
            stage: waml::analysis::AnalysisStage::Shell,
            reason: "sentinel".into(),
        },
    );
    let edit_error = waml::edit::EditError::from(analyses_error);
    assert_eq!(edit_error.index, 0);
    assert_eq!(edit_error.op, "analysis.prepare");
    assert!(edit_error.selector.is_none());

    let action_error = waml::edit::EditError::from(ActionError::StructuralInvariant {
        reason: "sentinel".into(),
    });
    assert_eq!(action_error.index, 0);
    assert_eq!(action_error.op, "syntax.action");
    assert!(action_error.reason.contains("StructuralInvariant"));

    fn assert_error(_: &dyn std::error::Error) {}
    assert_error(&action_error);
}

#[test]
fn versioned_locator_preserves_occurrence_identity_and_wrong_tree_error() {
    let source = source();
    let analyses = build_analyses(&source, None, 13);
    let document = id(&analyses, "a.md");
    let snapshot = analyses.okf.markdown.document(document).unwrap();
    let node = snapshot.tree().root();
    let token = node
        .children()
        .find_map(|element| element.into_token())
        .unwrap();

    let node_locator =
        VersionedSyntaxLocator::for_node(document, revision(&analyses, document), 13, &node);
    let token_locator =
        VersionedSyntaxLocator::for_token(document, revision(&analyses, document), 13, &token);
    assert_eq!(node_locator.document(), document);
    assert_eq!(
        node_locator.document_revision(),
        revision(&analyses, document)
    );
    assert_eq!(node_locator.session_revision(), 13);
    let occurrence_locator = node.locator();
    assert_eq!(
        node_locator.locator().tree_id(),
        occurrence_locator.tree_id()
    );
    assert_eq!(node_locator.locator().path(), occurrence_locator.path());
    assert_eq!(
        node_locator.locator().expected_kind(),
        occurrence_locator.expected_kind()
    );
    let resolved = token_locator
        .resolve_in(snapshot.tree())
        .unwrap()
        .locator();
    let token_occurrence = token.locator();
    assert_eq!(resolved.tree_id(), token_occurrence.tree_id());
    assert_eq!(resolved.path(), token_occurrence.path());
    assert_eq!(resolved.expected_kind(), token_occurrence.expected_kind());

    let other_tree = waml_syntax::SyntaxTree::new(
        snapshot.tree().root_green().clone(),
        snapshot.tree().diagnostics().into(),
        waml_syntax::MarkdownDialect::CommonMarkCurrent,
    );
    assert!(matches!(
        node_locator.resolve_in(&other_tree),
        Err(RewriteError::WrongTree { .. })
    ));
}
