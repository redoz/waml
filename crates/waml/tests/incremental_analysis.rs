use std::sync::Arc;

use waml::uml::syntax::{UmlLanguage, UmlSyntaxKind};
use waml::uml::WamlCodeRole;
use waml::{
    analysis::{
        prepare_candidate, prepare_candidate_with_markdown_updates, AnalysisError,
        InvalidPromotedMarkdownUpdateReason, PreparedCandidate, PreviousAnalyses,
        ProjectionFreshness, PromotedMarkdownUpdate,
    },
    edit::{
        apply_exact_source_edit, EditBatch, EditContext, ExactSourceEdit, ExactSourceEditError,
    },
    host::replace_document,
    source::{BundlePath, SourceBundle, SourceDocument},
    uml::{DeclaredBundle, DeclaredField, DeclaredLayoutStatement},
};
use waml_syntax::{
    reparse_markdown, AstNode, DocumentRevision, GreenElement, GreenFactory, GreenText,
    MarkdownDialect, MarkdownSemanticRole, MarkdownSyntaxUpdate, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind, SourceText, SyntaxAnnotation, SyntaxElement, SyntaxNode, SyntaxToken,
    SyntaxTree, TextChange, TextRange, TextSize, TriviaKind, WamlSectionKind,
};

fn prepared(
    source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    revision: u64,
) -> PreparedCandidate {
    prepare_candidate(source, previous, revision).unwrap()
}

fn path(value: &str) -> BundlePath {
    BundlePath::parse(value).unwrap()
}

fn replace(start: u32, end: u32, replacement: &str) -> TextChange {
    TextChange {
        old_range: TextRange::new(TextSize::new(start), TextSize::new(end)).unwrap(),
        replacement: Arc::from(replacement),
    }
}

fn prepared_two_document_fixture(revision: u64) -> PreparedCandidate {
    prepared(
        SourceBundle::try_from_pairs([("order.md", "# Order\n"), ("other.md", "# Other\n")])
            .unwrap(),
        None,
        revision,
    )
}

#[test]
fn transition_trace_indexes_refresh_after_target_fragment_changes() {
    let baseline_source = SourceBundle::try_from_pairs([
        (
            "behavior.md",
            "---\ntype: uml.StateMachineDiagram\ntitle: Behavior\n---\n# Behavior\n\n## Nodes\n\n### Idle\n- on `go` transitions to Done traces [CLAIM](./contract.md#claim) traces [External](https://example.com/spec)\n\n### Done\n",
        ),
        ("contract.md", "# Contract\n"),
    ])
    .unwrap();
    let baseline = prepared(baseline_source.clone(), None, 70);
    let edge_key = baseline.uml().projection.flow_edges[0].key.clone();

    let outgoing = baseline.uml().traces_from(&edge_key);
    assert_eq!(outgoing.len(), 2);
    assert_eq!(outgoing[0].label, "CLAIM");
    assert_eq!(outgoing[0].trace_index, 0);
    assert!(matches!(
        outgoing[1].target,
        waml::model::TraceTarget::Https { .. }
    ));
    assert!(baseline
        .uml()
        .traces_to("contract", Some("claim"))
        .is_empty());

    let edited_source = replace_document(
        &baseline_source,
        SourceDocument::new(path("contract.md"), "# Contract\n\n## Claim\n".into()),
    )
    .unwrap();
    let current = prepared(
        edited_source,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        71,
    );

    let incoming = current.uml().traces_to("contract", Some("claim"));
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].flow_edge_key, edge_key);
    assert_eq!(incoming[0].behavior, "behavior");
}

fn edit_context(candidate: &PreparedCandidate) -> EditContext<'_> {
    EditContext {
        source: candidate.source(),
        okf_analysis: candidate.okf(),
        session_revision: candidate.revision(),
        uml: candidate.uml(),
    }
}

fn exact_order_edit(
    before: &PreparedCandidate,
    expected_text: SourceText,
    changes: Arc<[TextChange]>,
) -> ExactSourceEdit {
    let document = document_id(before, "order.md");
    ExactSourceEdit {
        document,
        base_revision: before.okf().markdown_snapshot(document).unwrap().revision(),
        changes,
        expected_text,
    }
}

fn apply_exact_order_edit(
    before: &PreparedCandidate,
    expected_text: SourceText,
    changes: Arc<[TextChange]>,
) -> SourceBundle {
    exact_order_edit(before, expected_text, changes)
        .apply_reversible(edit_context(before))
        .unwrap()
        .source
}

fn promoted_order_update(
    before: &PreparedCandidate,
    revision: DocumentRevision,
    text: SourceText,
    changes: &[TextChange],
) -> MarkdownSyntaxUpdate {
    let document = document_id(before, "order.md");
    reparse_markdown(
        before.okf().markdown_snapshot(document).unwrap(),
        revision,
        text,
        changes,
    )
    .unwrap()
}

fn task5_diagram_fixture() -> SourceBundle {
    SourceBundle::try_from_pairs([
        (
            "order.md",
            "---\ntype: uml.ClassDiagram\ntitle: Order\nprofile: uml-domain\n---\n# Order\n\n## Layout\n- [Order](order-node.md)\n",
        ),
        (
            "customer.md",
            "---\ntype: uml.ClassDiagram\ntitle: Customer\nprofile: uml-domain\n---\n# Customer\n\n## Layout\n- [Customer](customer-node.md)\n",
        ),
        ("order-node.md", "---\ntype: uml.Class\n---\n# Order\n"),
        (
            "customer-node.md",
            "---\ntype: uml.Class\n---\n# Customer\n",
        ),
    ])
    .unwrap()
}

fn island_owner(
    candidate: &PreparedCandidate,
    path: &str,
    kind: WamlSectionKind,
) -> waml_syntax::SyntaxIdentity {
    let document = document_id(candidate, path);
    candidate
        .okf()
        .markdown_snapshot(document)
        .unwrap()
        .structure()
        .islands
        .iter()
        .find(|island| island.kind == kind)
        .unwrap()
        .owner
}

#[test]
fn shared_waml_code_spans_use_exact_island_tokens_and_absolute_ranges() {
    let authored = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- count: Number {0..42}\n\n## Layout\n- column of \"Café\", ([Order](order.md)) as row with frame, small margins left of Ghost\n";
    let candidate = prepared(
        SourceBundle::try_from_pairs([("order.md", authored)]).unwrap(),
        None,
        41,
    );
    let document = document_id(&candidate, "order.md");
    let markdown = candidate.okf().markdown_snapshot(document).unwrap();
    let island = markdown
        .structure()
        .islands
        .iter()
        .find(|island| island.kind == WamlSectionKind::Attributes)
        .unwrap();

    let spans = candidate
        .uml()
        .code_spans(island.owner, island.content_range)
        .expect("the exact immutable WAML island must have shared code roles");

    assert!(!spans.is_empty());
    assert!(spans.iter().all(|span| {
        island.content_range.start() <= span.range.start()
            && span.range.end() <= island.content_range.end()
    }));
    assert!(spans
        .windows(2)
        .all(|pair| pair[0].range.end() <= pair[1].range.start()));
    let classified = spans
        .iter()
        .map(|span| {
            (
                &authored[span.range.start().to_usize()..span.range.end().to_usize()],
                span.role,
            )
        })
        .collect::<Vec<_>>();
    assert!(classified.contains(&("count", WamlCodeRole::Property)));
    assert!(classified.contains(&("Number", WamlCodeRole::Type)));
    assert!(classified.contains(&("0..42", WamlCodeRole::Number)));
    assert!(classified.contains(&(":", WamlCodeRole::Punctuation)));
    assert!(classified.contains(&("{", WamlCodeRole::Punctuation)));
    assert!(classified.contains(&("}", WamlCodeRole::Punctuation)));

    let clipped = TextRange::new(
        island
            .content_range
            .start()
            .checked_add(TextSize::new(1))
            .unwrap(),
        island.content_range.end(),
    )
    .unwrap();
    assert!(candidate.uml().code_spans(island.owner, clipped).is_none());
}

#[test]
fn shared_waml_code_roles_cover_layout_strings_keywords_and_recovery() {
    let authored = "---\ntype: uml.ClassDiagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n- column of \"Café\", ([Order](order.md)) as row with frame, small margins left of Ghost\n";
    let candidate = prepared(
        SourceBundle::try_from_pairs([("diagram.md", authored)]).unwrap(),
        None,
        42,
    );
    let document = document_id(&candidate, "diagram.md");
    let island = candidate
        .okf()
        .markdown_snapshot(document)
        .unwrap()
        .structure()
        .islands
        .iter()
        .find(|island| island.kind == WamlSectionKind::Layout)
        .unwrap();
    let spans = candidate
        .uml()
        .code_spans(island.owner, island.content_range)
        .unwrap();
    let classified = spans
        .iter()
        .map(|span| {
            (
                &authored[span.range.start().to_usize()..span.range.end().to_usize()],
                span.role,
            )
        })
        .collect::<Vec<_>>();
    assert!(
        classified.contains(&("column", WamlCodeRole::Keyword)),
        "{classified:?}"
    );
    assert!(classified.contains(&("\"Café\"", WamlCodeRole::String)));

    let broken =
        "---\ntype: uml.Class\n---\n# Broken\n\n## Attributes\n- repeated: T {1} trailing\n";
    let recovered = prepared(
        SourceBundle::try_from_pairs([("broken.md", broken)]).unwrap(),
        None,
        43,
    );
    let document = document_id(&recovered, "broken.md");
    let island = recovered
        .okf()
        .markdown_snapshot(document)
        .unwrap()
        .structure()
        .islands
        .iter()
        .find(|island| island.kind == WamlSectionKind::Attributes)
        .unwrap();
    let spans = recovered
        .uml()
        .code_spans(island.owner, island.content_range)
        .unwrap();
    assert!(spans.iter().any(|span| span.role == WamlCodeRole::Invalid));
}

#[test]
fn shared_waml_code_spans_follow_the_installed_document_revision() {
    let original = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- count: Number {0..42}\n";
    let baseline_source = SourceBundle::try_from_pairs([("order.md", original)]).unwrap();
    let baseline = prepared(baseline_source.clone(), None, 44);
    let edited_text = "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- total: Text {1..8}\n";
    let edited_source = replace_document(
        &baseline_source,
        SourceDocument::new(path("order.md"), edited_text.into()),
    )
    .unwrap();
    let current = prepared(
        edited_source,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        45,
    );
    let document = document_id(&current, "order.md");
    let markdown = current.okf().markdown_snapshot(document).unwrap();
    let island = markdown
        .structure()
        .islands
        .iter()
        .find(|island| island.kind == WamlSectionKind::Attributes)
        .unwrap();
    let spans = current
        .uml()
        .code_spans(island.owner, island.content_range)
        .unwrap();
    let classified = spans
        .iter()
        .map(|span| {
            (
                &edited_text[span.range.start().to_usize()..span.range.end().to_usize()],
                span.role,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(markdown.revision(), DocumentRevision::new(2));
    assert!(classified.contains(&("total", WamlCodeRole::Property)));
    assert!(classified.contains(&("Text", WamlCodeRole::Type)));
    assert!(classified.contains(&("1..8", WamlCodeRole::Number)));
    assert!(!classified.iter().any(|(text, _)| *text == "count"));
}

#[test]
fn shared_waml_code_spans_include_exact_fenced_waml_and_document_parity() {
    let authored = "---\ntype: uml.Class\n---\n# Example\n\n## Attributes\n- unknown: Number {0..42}\n\n```waml\n## Attributes\n- unknown: Number {0..42}\n```\n";
    let candidate = prepared(
        SourceBundle::try_from_pairs([("example.md", authored)]).unwrap(),
        None,
        46,
    );
    let document = document_id(&candidate, "example.md");
    let markdown = candidate.okf().markdown_snapshot(document).unwrap();
    let full_range = TextRange::new(TextSize::new(0), markdown.text().len()).unwrap();
    let owner = markdown
        .queries()
        .spans(full_range)
        .find(|span| span.semantic_role == MarkdownSemanticRole::FencedCode)
        .expect("the fenced code must have a semantic owner")
        .owner;
    let fence = markdown
        .queries()
        .fenced_code(owner)
        .expect("the semantic owner must resolve to fenced-code metadata");
    let island = markdown
        .structure()
        .islands
        .iter()
        .find(|island| island.kind == WamlSectionKind::Attributes)
        .expect("the document must retain the matching section syntax");

    let spans = candidate
        .uml()
        .code_spans(fence.owner, fence.content_range)
        .expect("an exact fenced waml block must have shared code roles");
    assert!(spans.windows(2).all(|pair| {
        fence.content_range.start() <= pair[0].range.start()
            && pair[0].range.end() <= pair[1].range.start()
            && pair[1].range.end() <= fence.content_range.end()
    }));
    let classified = spans
        .iter()
        .map(|span| {
            (
                &authored[span.range.start().to_usize()..span.range.end().to_usize()],
                span.role,
            )
        })
        .collect::<Vec<_>>();
    assert!(classified.contains(&("unknown", WamlCodeRole::Property)));
    assert!(classified.contains(&("Number", WamlCodeRole::Type)));
    assert!(classified.contains(&("0..42", WamlCodeRole::Number)));
    assert!(classified.contains(&(":", WamlCodeRole::Punctuation)));
    assert!(classified.contains(&("{", WamlCodeRole::Punctuation)));
    assert!(classified.contains(&("}", WamlCodeRole::Punctuation)));

    let section_spans = candidate
        .uml()
        .code_spans(island.owner, island.content_range)
        .expect("the matching WAML section must have shared code roles");
    let section_classified = section_spans
        .iter()
        .map(|span| {
            (
                &authored[span.range.start().to_usize()..span.range.end().to_usize()],
                span.role,
            )
        })
        .collect::<Vec<_>>();
    let classified_syntax = classified
        .iter()
        .copied()
        .filter(|(_, role)| *role != WamlCodeRole::Comment)
        .collect::<Vec<_>>();
    let section_syntax = section_classified
        .iter()
        .copied()
        .filter(|(_, role)| *role != WamlCodeRole::Comment)
        .collect::<Vec<_>>();
    assert_eq!(classified_syntax, section_syntax);

    let document_spans = candidate
        .uml()
        .document_code_spans(document)
        .expect("the document must expose its retained WAML code roles");
    let fenced_document_spans = document_spans
        .iter()
        .copied()
        .filter(|span| {
            fence.content_range.start() <= span.range.start()
                && span.range.end() <= fence.content_range.end()
        })
        .collect::<Vec<_>>();
    assert_eq!(fenced_document_spans.as_slice(), spans.as_ref());

    let clipped = TextRange::new(
        fence
            .content_range
            .start()
            .checked_add(TextSize::new(1))
            .unwrap(),
        fence.content_range.end(),
    )
    .unwrap();
    assert!(candidate.uml().code_spans(fence.owner, clipped).is_none());
}

#[test]
fn invalid_edited_island_keeps_unrelated_projection_current() {
    let source = task5_diagram_fixture();
    let baseline = prepared(source.clone(), None, 1);
    let order = document_id(&baseline, "order.md");
    let customer_island = island_owner(&baseline, "customer.md", WamlSectionKind::Layout);
    let order_projection = baseline.uml().diagram("order").unwrap().clone();
    let customer_projection = baseline.uml().diagram("customer").unwrap().clone();
    let edited = replace_document(
        &source,
        SourceDocument::new(
            path("order.md"),
            "---\ntype: uml.ClassDiagram\ntitle: Order\nprofile: uml-domain\n---\n# Order\n\n## Layout\n- left of\n".into(),
        ),
    )
    .unwrap();

    let after = prepared(
        edited,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );
    let failed_revision = after.okf().catalog.document(order).unwrap().revision();
    let order_island = island_owner(&after, "order.md", WamlSectionKind::Layout);
    assert_eq!(
        after.uml().projection_freshness(order_island),
        ProjectionFreshness::RetainedStale { failed_revision }
    );
    assert_eq!(
        after.uml().projection_freshness(customer_island),
        ProjectionFreshness::Current
    );
    assert!(Arc::ptr_eq(
        after.uml().diagram("order").unwrap(),
        &order_projection
    ));
    assert!(Arc::ptr_eq(
        after.uml().diagram("customer").unwrap(),
        &customer_projection
    ));
    assert!(after
        .uml()
        .revisioned_diagnostics()
        .iter()
        .any(|diagnostic| {
            diagnostic.document == order && diagnostic.revision == failed_revision
        }));
}

#[test]
fn affected_analysis_is_sorted_deduplicated_and_includes_semantic_dependents() {
    let source = SourceBundle::try_from_pairs([
        (
            "order.md",
            "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- id: String\n",
        ),
        (
            "customer.md",
            "---\ntype: uml.Class\n---\n# Customer\n\n## Attributes\n- id: String\n",
        ),
        (
            "overview.md",
            "---\ntype: uml.ClassDiagram\n---\n# Overview\n\n## Members\n- [Order](order.md)\n- [Customer](customer.md)\n",
        ),
    ])
    .unwrap();
    let baseline = prepared(source.clone(), None, 1);
    let order = document_id(&baseline, "order.md");
    let customer = document_id(&baseline, "customer.md");
    let edited = replace_document(
        &replace_document(
            &source,
            SourceDocument::new(
                path("customer.md"),
                "---\ntype: uml.Class\n---\n# Customer\n\n## Attributes\n- name: String\n".into(),
            ),
        )
        .unwrap(),
        SourceDocument::new(
            path("order.md"),
            "---\ntype: uml.Class\n---\n# Order\n\n## Attributes\n- number: String\n".into(),
        ),
    )
    .unwrap();

    let after = prepared(
        edited,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );
    let affected = after.uml().affected();

    assert_eq!(affected.documents.as_ref(), &[order, customer]);
    assert!(affected.islands.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        affected
            .diagrams
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>(),
        vec!["overview"]
    );
}

#[test]
fn valid_recovery_replaces_stale_projection_and_clears_diagnostic() {
    let source = task5_diagram_fixture();
    let baseline = prepared(source.clone(), None, 1);
    let order = document_id(&baseline, "order.md");
    let invalid = replace_document(
        &source,
        SourceDocument::new(
            path("order.md"),
            "---\ntype: uml.ClassDiagram\ntitle: Order\nprofile: uml-domain\n---\n# Order\n\n## Layout\n- left of\n".into(),
        ),
    )
    .unwrap();
    let stale = prepared(
        invalid.clone(),
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );
    let stale_projection = stale.uml().diagram("order").unwrap().clone();
    let recovered_source = replace_document(
        &invalid,
        SourceDocument::new(
            path("order.md"),
            "---\ntype: uml.ClassDiagram\ntitle: Order\nprofile: uml-domain\n---\n# Order\n\n## Layout\n- [Order](order-node.md)\n".into(),
        ),
    )
    .unwrap();

    let recovered = prepared(
        recovered_source,
        Some(PreviousAnalyses {
            okf: stale.okf(),
            uml: stale.uml(),
        }),
        3,
    );
    let owner = island_owner(&recovered, "order.md", WamlSectionKind::Layout);

    assert_eq!(
        recovered.uml().projection_freshness(owner),
        ProjectionFreshness::Current
    );
    assert!(!Arc::ptr_eq(
        recovered.uml().diagram("order").unwrap(),
        &stale_projection
    ));
    assert!(recovered
        .uml()
        .revisioned_diagnostics()
        .iter()
        .all(|diagnostic| diagnostic.document != order));
}

#[test]
fn exact_source_edit_installs_expected_allocation_and_is_reversible() {
    let before = prepared_two_document_fixture(41);
    let expected_text = SourceText::new("# Purchase\n").unwrap();
    let changes: Arc<[TextChange]> = Arc::from([replace(2, 7, "Purchase")]);
    let applied = exact_order_edit(&before, expected_text.clone(), changes)
        .apply_reversible(edit_context(&before))
        .unwrap();
    let installed = applied.source.document(&path("order.md")).unwrap();

    assert_eq!(installed.text(), "# Purchase\n");
    assert!(Arc::ptr_eq(installed.text_shared(), expected_text.shared()));
    assert!(before
        .source()
        .shares_text_with(&applied.source, "other.md"));

    let restored = applied
        .inverse
        .apply_reversible(EditContext {
            source: &applied.source,
            ..edit_context(&before)
        })
        .unwrap()
        .source;
    assert_eq!(restored, *before.source());
}

#[test]
fn exact_source_edit_rejects_unknown_document_without_changing_source() {
    let before = prepared(
        SourceBundle::try_from_pairs([("order.md", "# Order\n")]).unwrap(),
        None,
        41,
    );
    let foreign = prepared_two_document_fixture(41);
    let existing = document_id(&before, "order.md");
    let first = document_id(&foreign, "order.md");
    let second = document_id(&foreign, "other.md");
    let unknown = if first == existing { second } else { first };
    let original = before.source().document(&path("order.md")).unwrap();
    let error = ExactSourceEdit {
        document: unknown,
        base_revision: DocumentRevision::new(1),
        changes: Arc::from([replace(2, 7, "Purchase")]),
        expected_text: SourceText::new("# Purchase\n").unwrap(),
    }
    .lower(edit_context(&before))
    .unwrap_err();

    assert_eq!(error.op, "source.document");
    assert_eq!(original.text(), "# Order\n");
}

#[test]
fn exact_source_edit_accepts_current_base_identity_while_semantic_catalog_lags() {
    let semantic = prepared_two_document_fixture(41);
    let document = document_id(&semantic, "order.md");
    let first_text = SourceText::new("# Purchase\n").unwrap();
    let first_changes: Arc<[TextChange]> = Arc::from([replace(2, 7, "Purchase")]);
    let first_source = apply_exact_order_edit(&semantic, first_text.clone(), first_changes.clone());
    let first_revision = semantic
        .okf()
        .markdown_snapshot(document)
        .unwrap()
        .revision()
        .checked_next()
        .unwrap();
    let first_syntax = reparse_markdown(
        semantic.okf().markdown_snapshot(document).unwrap(),
        first_revision,
        first_text,
        &first_changes,
    )
    .unwrap()
    .snapshot;
    let second_text = SourceText::new("# Customer\n").unwrap();
    let applied = apply_exact_source_edit(
        &first_source,
        &path("order.md"),
        first_syntax.text(),
        &[replace(2, 10, "Customer")],
        second_text.clone(),
    )
    .unwrap();

    let installed = applied.source.document(&path("order.md")).unwrap();
    assert_eq!(installed.text(), "# Customer\n");
    assert!(Arc::ptr_eq(installed.text_shared(), second_text.shared()));
    assert_eq!(
        applied
            .inverse
            .apply_reversible(EditContext {
                source: &applied.source,
                ..edit_context(&semantic)
            })
            .unwrap()
            .source,
        first_source,
    );
}

#[test]
fn exact_source_edit_rejects_equal_bytes_from_a_different_base_allocation() {
    let before = prepared_two_document_fixture(41);
    let error = match apply_exact_source_edit(
        before.source(),
        &path("order.md"),
        &SourceText::new("# Order\n").unwrap(),
        &[replace(2, 7, "Purchase")],
        SourceText::new("# Purchase\n").unwrap(),
    ) {
        Ok(_) => panic!("a copied base allocation must be rejected"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        ExactSourceEditError::BaseIdentityMismatch { .. }
    ));
    assert_eq!(
        before.source().document(&path("order.md")).unwrap().text(),
        "# Order\n"
    );
}

#[test]
fn exact_source_edit_rejects_overlapping_changes_without_changing_source() {
    let before = prepared_two_document_fixture(41);
    let edit = exact_order_edit(
        &before,
        SourceText::new("# Purrchase\n").unwrap(),
        Arc::from([replace(2, 4, "Pur"), replace(3, 7, "rchase")]),
    );

    let error = edit.lower(edit_context(&before)).unwrap_err();

    assert_eq!(error.op, "source.change_map");
    assert_eq!(
        before.source().document(&path("order.md")).unwrap().text(),
        "# Order\n"
    );
}

#[test]
fn exact_source_edit_rejects_result_text_mismatch_without_changing_source() {
    let before = prepared_two_document_fixture(41);
    let edit = exact_order_edit(
        &before,
        SourceText::new("# Wrong\n").unwrap(),
        Arc::from([replace(2, 7, "Purchase")]),
    );

    let error = edit.lower(edit_context(&before)).unwrap_err();

    assert_eq!(error.op, "source.expected_text");
    assert_eq!(
        before.source().document(&path("order.md")).unwrap().text(),
        "# Order\n"
    );
}

#[cfg(feature = "test-support")]
#[test]
fn promoted_markdown_update_is_installed_by_arc_without_a_second_parse() {
    let before = prepared_two_document_fixture(41);
    let document = document_id(&before, "order.md");
    let other = document_id(&before, "other.md");
    let old = before.okf().markdown_snapshot(document).unwrap().clone();
    let unchanged = before.okf().markdown_snapshot(other).unwrap().clone();
    let changes: Arc<[TextChange]> = Arc::from([replace(2, 7, "Purchase")]);
    let next_text = SourceText::new("# Purchase\n").unwrap();
    let update = promoted_order_update(
        &before,
        old.revision().checked_next().unwrap(),
        next_text.clone(),
        &changes,
    );
    let candidate = apply_exact_order_edit(&before, next_text.clone(), changes.clone());
    let mut probe = waml::analysis::test_support::PreparationProbe::succeed();

    let after = waml::analysis::test_support::prepare_candidate_with_promoted_probe(
        candidate,
        PreviousAnalyses {
            okf: before.okf(),
            uml: before.uml(),
        },
        42,
        Arc::from([PromotedMarkdownUpdate {
            document,
            base_revision: old.revision(),
            update: update.clone(),
        }]),
        &mut probe,
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        after.okf().markdown_snapshot(document).unwrap(),
        &update.snapshot,
    ));
    assert!(Arc::ptr_eq(
        after.okf().markdown_snapshot(other).unwrap(),
        &unchanged,
    ));
    assert!(Arc::ptr_eq(
        after
            .source()
            .document(&path("order.md"))
            .unwrap()
            .text_shared(),
        update.snapshot.text().shared(),
    ));
    assert_eq!(probe.markdown_parse_calls(document), 0);
    assert_eq!(probe.markdown_reparse_calls(document), 0);
    assert_eq!(probe.markdown_promotions(document), 1);
}

#[cfg(feature = "test-support")]
#[test]
fn recovered_markdown_update_is_installed_by_arc_without_parse_or_reparse() {
    let before = prepared_two_document_fixture(41);
    let document = document_id(&before, "order.md");
    let old = before.okf().markdown_snapshot(document).unwrap().clone();
    let changes: Arc<[TextChange]> = Arc::from([replace(2, 7, "Purchase")]);
    let next_text = SourceText::new("# Purchase\n").unwrap();
    let recovered = promoted_order_update(
        &before,
        old.revision().checked_next().unwrap(),
        next_text.clone(),
        &changes,
    );
    let expected = recovered.snapshot.clone();
    let candidate = apply_exact_order_edit(&before, next_text, changes);
    let mut probe = waml::analysis::test_support::PreparationProbe::succeed();

    let after = waml::analysis::test_support::prepare_candidate_with_recovery_probe(
        candidate,
        PreviousAnalyses {
            okf: before.okf(),
            uml: before.uml(),
        },
        42,
        PromotedMarkdownUpdate {
            document,
            base_revision: old.revision(),
            update: recovered,
        },
        &mut probe,
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        after.okf().markdown_snapshot(document).unwrap(),
        &expected,
    ));
    assert_eq!(probe.markdown_parse_calls(document), 0);
    assert_eq!(probe.markdown_reparse_calls(document), 0);
}

#[cfg(feature = "test-support")]
#[test]
fn chained_promoted_updates_never_parse_or_reparse() {
    let before = prepared_two_document_fixture(41);
    let document = document_id(&before, "order.md");
    let mut probe = waml::analysis::test_support::PreparationProbe::succeed();
    let first_changes: Arc<[TextChange]> = Arc::from([replace(2, 7, "Purchase")]);
    let first_text = SourceText::new("# Purchase\n").unwrap();
    let first_update = promoted_order_update(
        &before,
        before
            .okf()
            .markdown_snapshot(document)
            .unwrap()
            .revision()
            .checked_next()
            .unwrap(),
        first_text.clone(),
        &first_changes,
    );
    let first_source = apply_exact_order_edit(&before, first_text, first_changes);
    let first = waml::analysis::test_support::prepare_candidate_with_promoted_probe(
        first_source,
        PreviousAnalyses {
            okf: before.okf(),
            uml: before.uml(),
        },
        42,
        Arc::from([PromotedMarkdownUpdate {
            document,
            base_revision: before.okf().markdown_snapshot(document).unwrap().revision(),
            update: first_update,
        }]),
        &mut probe,
    )
    .unwrap();

    let second_changes: Arc<[TextChange]> = Arc::from([replace(2, 10, "Customer")]);
    let second_text = SourceText::new("# Customer\n").unwrap();
    let second_base = first.okf().markdown_snapshot(document).unwrap().revision();
    let second_update = promoted_order_update(
        &first,
        second_base.checked_next().unwrap(),
        second_text.clone(),
        &second_changes,
    );
    let expected = second_update.snapshot.clone();
    let second_source = apply_exact_order_edit(&first, second_text, second_changes);
    let second = waml::analysis::test_support::prepare_candidate_with_promoted_probe(
        second_source,
        PreviousAnalyses {
            okf: first.okf(),
            uml: first.uml(),
        },
        43,
        Arc::from([PromotedMarkdownUpdate {
            document,
            base_revision: second_base,
            update: second_update,
        }]),
        &mut probe,
    )
    .unwrap();

    assert!(Arc::ptr_eq(
        second.okf().markdown_snapshot(document).unwrap(),
        &expected,
    ));
    assert_eq!(probe.markdown_parse_calls(document), 0);
    assert_eq!(probe.markdown_reparse_calls(document), 0);
    assert_eq!(probe.markdown_promotions(document), 2);
}

fn assert_invalid_promoted_update(
    result: Result<PreparedCandidate, AnalysisError>,
    document: waml::source::DocumentId,
    expected: InvalidPromotedMarkdownUpdateReason,
) {
    let error = match result {
        Ok(_) => panic!("invalid promoted update was accepted"),
        Err(error) => error,
    };
    match error {
        AnalysisError::InvalidPromotedMarkdownUpdate {
            document: actual,
            reason,
        } => {
            assert_eq!(actual, document);
            assert_eq!(reason, expected);
        }
        other => panic!("unexpected analysis error: {other:?}"),
    }
}

fn promotion_fixture() -> (
    PreparedCandidate,
    waml::source::DocumentId,
    Arc<[TextChange]>,
    SourceText,
    MarkdownSyntaxUpdate,
    SourceBundle,
) {
    let before = prepared_two_document_fixture(41);
    let document = document_id(&before, "order.md");
    let old = before.okf().markdown_snapshot(document).unwrap();
    let changes: Arc<[TextChange]> = Arc::from([replace(2, 7, "Purchase")]);
    let next_text = SourceText::new("# Purchase\n").unwrap();
    let update = promoted_order_update(
        &before,
        old.revision().checked_next().unwrap(),
        next_text.clone(),
        &changes,
    );
    let candidate = apply_exact_order_edit(&before, next_text.clone(), changes.clone());
    (before, document, changes, next_text, update, candidate)
}

#[test]
fn promoted_markdown_update_rejects_missing_previous_document() {
    let before = prepared(
        SourceBundle::try_from_pairs([("other.md", "# Other\n")]).unwrap(),
        None,
        41,
    );
    let candidate_source =
        SourceBundle::try_from_pairs([("other.md", "# Other\n"), ("order.md", "# Purchase\n")])
            .unwrap();
    let candidate_preview = prepared(
        candidate_source.clone(),
        Some(PreviousAnalyses {
            okf: before.okf(),
            uml: before.uml(),
        }),
        42,
    );
    let document = document_id(&candidate_preview, "order.md");
    let update = candidate_preview
        .okf()
        .markdown_snapshot(document)
        .unwrap()
        .clone();

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate_source,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: DocumentRevision::new(0),
                update: MarkdownSyntaxUpdate {
                    snapshot: update,
                    affected_ranges: Arc::from([]),
                    outcome: waml_syntax::MarkdownReparseOutcome::Full {
                        reason: waml_syntax::FullReparseReason::UnsafeSynchronization,
                    },
                },
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
    );
}

#[test]
fn promoted_markdown_update_rejects_missing_candidate_document() {
    let (before, document, _, _, update, _) = promotion_fixture();
    let candidate = SourceBundle::try_from_pairs([("other.md", "# Other\n")]).unwrap();

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: before.okf().markdown_snapshot(document).unwrap().revision(),
                update,
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::MissingCandidateDocument,
    );
}

#[test]
fn promoted_markdown_update_rejects_stale_base_revision() {
    let (before, document, _, _, update, candidate) = promotion_fixture();
    let expected = before.okf().markdown_snapshot(document).unwrap().revision();
    let actual = DocumentRevision::new(expected.get() - 1);

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: actual,
                update,
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::StaleBaseRevision { expected, actual },
    );
}

#[test]
fn promoted_markdown_update_rejects_non_successor_revision() {
    let (before, document, changes, next_text, _, candidate) = promotion_fixture();
    let base = before.okf().markdown_snapshot(document).unwrap().revision();
    let expected = base.checked_next().unwrap();
    let actual = DocumentRevision::new(expected.get() + 1);
    let update = promoted_order_update(&before, actual, next_text, &changes);

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: base,
                update,
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::NonSuccessorRevision { expected, actual },
    );
}

#[test]
fn promoted_markdown_update_reports_stale_candidate_revision_as_actual() {
    let before = prepared_two_document_fixture(41);
    let document = document_id(&before, "order.md");
    let old = before.okf().markdown_snapshot(document).unwrap();
    let expected = old.revision().checked_next().unwrap();
    let actual = old.revision();
    let update = promoted_order_update(&before, expected, old.text().clone(), &[]);

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            before.source().clone(),
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: old.revision(),
                update,
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::NonSuccessorRevision { expected, actual },
    );
}

#[test]
fn promoted_markdown_update_rejects_duplicate_document() {
    let (before, document, _, _, update, candidate) = promotion_fixture();
    let promoted = PromotedMarkdownUpdate {
        document,
        base_revision: before.okf().markdown_snapshot(document).unwrap().revision(),
        update,
    };

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([promoted.clone(), promoted]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::DuplicateDocument,
    );
}

#[test]
fn promoted_markdown_update_rejects_result_text_mismatch() {
    let (before, document, _, _, update, _) = promotion_fixture();
    let candidate =
        SourceBundle::try_from_pairs([("order.md", "# Different\n"), ("other.md", "# Other\n")])
            .unwrap();

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: before.okf().markdown_snapshot(document).unwrap().revision(),
                update,
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::ResultTextMismatch,
    );
}

#[test]
fn promoted_markdown_update_rejects_invalid_affected_range() {
    let (before, document, _, _, mut update, candidate) = promotion_fixture();
    let invalid = TextRange::new(TextSize::new(0), TextSize::new(100)).unwrap();
    update.affected_ranges = Arc::from([invalid]);

    assert_invalid_promoted_update(
        prepare_candidate_with_markdown_updates(
            candidate,
            PreviousAnalyses {
                okf: before.okf(),
                uml: before.uml(),
            },
            42,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision: before.okf().markdown_snapshot(document).unwrap().revision(),
                update,
            }]),
        ),
        document,
        InvalidPromotedMarkdownUpdateReason::InvalidAffectedRange { range: invalid },
    );
}

fn document_id(candidate: &PreparedCandidate, path: &str) -> waml::source::DocumentId {
    candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse(path).unwrap())
        .unwrap()
}

fn annotation_fingerprint(annotations: &[SyntaxAnnotation]) -> Vec<(u64, &str, Option<&str>)> {
    annotations
        .iter()
        .map(|annotation| (annotation.id().get(), annotation.kind(), annotation.data()))
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
enum TextFingerprint {
    Static(String),
    Owned(String),
    SourceSlice { range: TextRange, spelling: String },
}

fn text_fingerprint(text: &GreenText) -> TextFingerprint {
    match text {
        GreenText::Static(value) => TextFingerprint::Static((*value).to_owned()),
        GreenText::Owned(value) => TextFingerprint::Owned(value.to_string()),
        GreenText::SourceSlice { range, .. } => TextFingerprint::SourceSlice {
            range: *range,
            spelling: text.write_to_string(),
        },
    }
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
                result.push(format!(
                    "node:{:?}:{at:?}..{end:?}:{:?}",
                    node.kind(),
                    annotation_fingerprint(node.annotations())
                ));
                node.children()
                    .iter()
                    .fold(at, |offset, child| visit(child, offset, result))
            }
            GreenElement::Token(token) => {
                let end = at.checked_add(token.width()).unwrap();
                let leading = token
                    .leading_trivia()
                    .iter()
                    .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                    .collect::<Vec<_>>();
                let trailing = token
                    .trailing_trivia()
                    .iter()
                    .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                    .collect::<Vec<_>>();
                result.push(format!(
                    "token:{:?}:{at:?}..{end:?}:{:?}:{leading:?}:{trailing:?}:missing={}:bad={}:codes={:?}:syntax={:?}",
                    token.kind(),
                    text_fingerprint(token.text()),
                    token.flags().is_missing(),
                    token.flags().is_bad(),
                    token.annotations(),
                    annotation_fingerprint(token.syntax_annotations()),
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

fn uml_fingerprint(tree: &SyntaxTree<UmlLanguage>) -> Vec<String> {
    fn visit(
        element: &GreenElement<UmlLanguage>,
        at: TextSize,
        result: &mut Vec<String>,
    ) -> TextSize {
        match element {
            GreenElement::Node(node) => {
                let end = at.checked_add(node.width()).unwrap();
                result.push(format!(
                    "node:{:?}:{at:?}..{end:?}:{:?}",
                    node.kind(),
                    annotation_fingerprint(node.annotations())
                ));
                node.children()
                    .iter()
                    .fold(at, |offset, child| visit(child, offset, result))
            }
            GreenElement::Token(token) => {
                let end = at.checked_add(token.width()).unwrap();
                let leading = token
                    .leading_trivia()
                    .iter()
                    .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                    .collect::<Vec<_>>();
                let trailing = token
                    .trailing_trivia()
                    .iter()
                    .map(|trivia| (trivia.kind, text_fingerprint(&trivia.text)))
                    .collect::<Vec<_>>();
                result.push(format!(
                    "token:{:?}:{at:?}..{end:?}:{:?}:{leading:?}:{trailing:?}:missing={}:bad={}:codes={:?}:syntax={:?}",
                    token.kind(),
                    text_fingerprint(token.text()),
                    token.flags().is_missing(),
                    token.flags().is_bad(),
                    token.annotations(),
                    annotation_fingerprint(token.syntax_annotations()),
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

fn uml_diagnostic_fingerprint(tree: &SyntaxTree<UmlLanguage>) -> Vec<String> {
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

#[allow(clippy::type_complexity)]
fn semantic_diagnostic_fingerprint(
    analysis: &waml::uml::Analysis,
) -> Vec<(
    String,
    String,
    String,
    String,
    usize,
    Option<(usize, usize)>,
    Option<TextRange>,
)> {
    analysis
        .diagnostics
        .iter()
        .map(|diagnostic| {
            (
                format!("{:?}", diagnostic.severity),
                diagnostic.code.as_str().to_owned(),
                diagnostic.message.clone(),
                diagnostic.file.clone(),
                diagnostic.line,
                diagnostic.span,
                diagnostic.range,
            )
        })
        .collect()
}

fn declared_occurrence_fingerprint(declared: &DeclaredBundle) -> Vec<String> {
    fn syntax(node: &SyntaxNode<UmlLanguage>) -> String {
        format!("{:?}:{:?}", node.kind(), node.range())
    }
    fn field<T>(label: &str, field: &DeclaredField<UmlLanguage, T>, out: &mut Vec<String>) {
        match field {
            DeclaredField::Absent => out.push(format!("{label}:absent")),
            DeclaredField::Valid { syntax: node, .. } => {
                out.push(format!("{label}:valid:{}", syntax(node)))
            }
            DeclaredField::Incomplete {
                syntax: node,
                expected,
            } => out.push(format!("{label}:incomplete:{}:{expected:?}", syntax(node))),
            DeclaredField::Invalid {
                syntax: node,
                diagnostics,
            } => out.push(format!("{label}:invalid:{}:{diagnostics:?}", syntax(node))),
        }
    }

    let mut out = Vec::new();
    for concept in declared.concepts() {
        out.push(format!(
            "concept:{}:attributes={}:values={}:slots={}:relationships={}:members={}:member_groups={}:inline_instances={}:layout={}:flow_nodes={}:lifelines={}:gates={}:messages={}:fragments={}:operands={}:interaction_uses={}",
            concept.concept_id,
            concept.attributes.len(),
            concept.values.len(),
            concept.slots.len(),
            concept.relationships.len(),
            concept.members.len(),
            concept.member_groups.len(),
            concept.inline_instances.len(),
            concept.layout.len(),
            concept.flow_nodes.len(),
            concept.lifelines.len(),
            concept.gates.len(),
            concept.messages.len(),
            concept.fragments.len(),
            concept.operands.len(),
            concept.interaction_uses.len(),
        ));
        for (index, attribute) in concept.attributes.iter().enumerate() {
            out.push(format!(
                "attribute:{index}:{}",
                syntax(attribute.syntax.syntax())
            ));
            field("visibility", &attribute.visibility, &mut out);
            field("name", &attribute.name, &mut out);
            field("type", &attribute.ty, &mut out);
            field("multiplicity", &attribute.multiplicity, &mut out);
        }
        for (index, layout) in concept.layout.iter().enumerate() {
            field(&format!("layout:{index}"), layout, &mut out);
            if let DeclaredField::Valid { value, .. } = layout {
                match value {
                    DeclaredLayoutStatement::Placement {
                        operands,
                        directions,
                    } => {
                        for (field_index, operand) in operands.iter().enumerate() {
                            field(
                                &format!("layout:{index}:operand:{field_index}"),
                                operand,
                                &mut out,
                            );
                        }
                        for (field_index, direction) in directions.iter().enumerate() {
                            field(
                                &format!("layout:{index}:direction:{field_index}"),
                                direction,
                                &mut out,
                            );
                        }
                    }
                    DeclaredLayoutStatement::Alignment { left, right } => {
                        field(&format!("layout:{index}:left"), left, &mut out);
                        field(&format!("layout:{index}:right"), right, &mut out);
                    }
                    DeclaredLayoutStatement::Standalone(operand) => {
                        field(&format!("layout:{index}:standalone"), operand, &mut out)
                    }
                }
            }
        }
    }
    out
}

fn nth_missing_token(
    tree: &SyntaxTree<UmlLanguage>,
    parent_kind: UmlSyntaxKind,
    token_kind: UmlSyntaxKind,
    occurrence: usize,
) -> SyntaxToken<UmlLanguage> {
    fn find_node(
        node: SyntaxNode<UmlLanguage>,
        kind: UmlSyntaxKind,
    ) -> Option<SyntaxNode<UmlLanguage>> {
        if node.kind() == kind {
            return Some(node);
        }
        node.children()
            .find_map(|child| child.into_node().and_then(|node| find_node(node, kind)))
    }
    fn collect(
        node: SyntaxNode<UmlLanguage>,
        kind: UmlSyntaxKind,
        tokens: &mut Vec<SyntaxToken<UmlLanguage>>,
    ) {
        for child in node.children() {
            match child {
                SyntaxElement::Token(token)
                    if token.kind() == kind && token.flags().is_missing() =>
                {
                    tokens.push(token)
                }
                SyntaxElement::Node(node) => collect(node, kind, tokens),
                _ => {}
            }
        }
    }
    let parent = find_node(tree.root(), parent_kind).expect("expected UML parent node");
    let mut tokens = Vec::new();
    collect(parent, token_kind, &mut tokens);
    tokens
        .into_iter()
        .nth(occurrence)
        .expect("expected exact missing UML recovery token")
}

fn assert_uml_current_source_provenance(
    tree: &SyntaxTree<UmlLanguage>,
    current: &Arc<String>,
) -> usize {
    fn text_source_slice(text: &GreenText, current: &Arc<String>) -> usize {
        match text {
            GreenText::SourceSlice { source, .. } => {
                assert!(Arc::ptr_eq(source.shared(), current));
                1
            }
            GreenText::Static(_) | GreenText::Owned(_) => 0,
        }
    }
    fn visit(element: &GreenElement<UmlLanguage>, current: &Arc<String>) -> usize {
        match element {
            GreenElement::Node(node) => node
                .children()
                .iter()
                .map(|child| visit(child, current))
                .sum(),
            GreenElement::Token(token) => {
                text_source_slice(token.text(), current)
                    + token
                        .leading_trivia()
                        .iter()
                        .map(|trivia| text_source_slice(&trivia.text, current))
                        .sum::<usize>()
                    + token
                        .trailing_trivia()
                        .iter()
                        .map(|trivia| text_source_slice(&trivia.text, current))
                        .sum::<usize>()
            }
        }
    }
    visit(&GreenElement::Node(tree.root_green().clone()), current)
}

fn assert_uml_rebased_identity(
    previous: &GreenElement<UmlLanguage>,
    current: &GreenElement<UmlLanguage>,
) -> (usize, usize) {
    match (previous, current) {
        (GreenElement::Node(previous), GreenElement::Node(current)) => {
            assert_eq!(previous.kind(), current.kind());
            assert_eq!(previous.children().len(), current.children().len());
            if previous.is_source_independent() {
                return (0, usize::from(Arc::ptr_eq(previous, current)));
            }
            assert!(!Arc::ptr_eq(previous, current));
            previous
                .children()
                .iter()
                .zip(current.children())
                .map(|(previous, current)| assert_uml_rebased_identity(previous, current))
                .fold((1, 0), |(source_backed, reused_static), counts| {
                    (source_backed + counts.0, reused_static + counts.1)
                })
        }
        (GreenElement::Token(previous), GreenElement::Token(current)) => {
            assert_eq!(previous.kind(), current.kind());
            if previous.is_source_independent() {
                (0, usize::from(Arc::ptr_eq(previous, current)))
            } else {
                assert!(!Arc::ptr_eq(previous, current));
                (1, 0)
            }
        }
        _ => panic!("incremental UML reparse changed green element shape"),
    }
}

fn assert_current_source_provenance(
    tree: &SyntaxTree<OkfMarkdownLanguage>,
    current: &Arc<String>,
) -> usize {
    fn text_source_slice(text: &GreenText, current: &Arc<String>) -> usize {
        match text {
            GreenText::SourceSlice { source, .. } => {
                assert!(Arc::ptr_eq(source.shared(), current));
                1
            }
            GreenText::Static(_) | GreenText::Owned(_) => 0,
        }
    }

    fn visit(element: &GreenElement<OkfMarkdownLanguage>, current: &Arc<String>) -> usize {
        match element {
            GreenElement::Node(node) => node
                .children()
                .iter()
                .map(|child| visit(child, current))
                .sum(),
            GreenElement::Token(token) => {
                text_source_slice(token.text(), current)
                    + token
                        .leading_trivia()
                        .iter()
                        .map(|trivia| text_source_slice(&trivia.text, current))
                        .sum::<usize>()
                    + token
                        .trailing_trivia()
                        .iter()
                        .map(|trivia| text_source_slice(&trivia.text, current))
                        .sum::<usize>()
            }
        }
    }

    visit(&GreenElement::Node(tree.root_green().clone()), current)
}

fn assert_rebased_identity(
    previous: &GreenElement<OkfMarkdownLanguage>,
    current: &GreenElement<OkfMarkdownLanguage>,
) -> (usize, usize) {
    match (previous, current) {
        (GreenElement::Node(previous), GreenElement::Node(current)) => {
            assert_eq!(previous.kind(), current.kind());
            assert_eq!(previous.children().len(), current.children().len());
            if previous.is_source_independent() {
                return (0, usize::from(Arc::ptr_eq(previous, current)));
            }
            assert!(!Arc::ptr_eq(previous, current));
            previous
                .children()
                .iter()
                .zip(current.children())
                .map(|(previous, current)| assert_rebased_identity(previous, current))
                .fold((1, 0), |(source_backed, reused_static), counts| {
                    (source_backed + counts.0, reused_static + counts.1)
                })
        }
        (GreenElement::Token(previous), GreenElement::Token(current)) => {
            assert_eq!(previous.kind(), current.kind());
            if previous.is_source_independent() {
                (0, usize::from(Arc::ptr_eq(previous, current)))
            } else {
                assert!(!Arc::ptr_eq(previous, current));
                (1, 0)
            }
        }
        _ => panic!("incremental reparse changed green element shape"),
    }
}

#[test]
fn shell_fingerprint_detects_trivia_storage_mutations() {
    let factory = GreenFactory::<OkfMarkdownLanguage>::new();
    let static_trivia = factory
        .trivia(TriviaKind::Whitespace, GreenText::Static(" "))
        .unwrap();
    let owned_trivia = factory
        .trivia(TriviaKind::Whitespace, GreenText::Owned(Arc::from(" ")))
        .unwrap();
    let tree = |trivia| {
        let token = factory
            .token(
                OkfMarkdownSyntaxKind::RawTextToken,
                GreenText::Static("x"),
                [trivia],
                [],
            )
            .unwrap();
        let root = factory
            .node(OkfMarkdownSyntaxKind::Root, [GreenElement::Token(token)])
            .unwrap();
        SyntaxTree::new(root, Arc::from([]), MarkdownDialect::WAML_DEFAULT)
    };

    assert_ne!(
        shell_fingerprint(&tree(static_trivia)),
        shell_fingerprint(&tree(owned_trivia))
    );
}

#[test]
fn byte_identical_fresh_document_rebases_every_source_slice() {
    let path = BundlePath::parse("same.md").unwrap();
    let text = "---\ntype: Note\n---\n# Same\nbody  \n";
    let mut saved = SourceBundle::try_from_pairs([("same.md", text)]).unwrap();
    let baseline = prepared(saved.clone(), None, 1);
    let id = document_id(&baseline, "same.md");
    let prior_weak = Arc::downgrade(baseline.okf().catalog.document(id).unwrap().text().shared());
    let fresh_source =
        replace_document(&saved, SourceDocument::new(path.clone(), text.to_owned())).unwrap();
    let current = prepared(
        fresh_source,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );
    let current_document = current.okf().catalog.document(id).unwrap();
    let previous_tree = baseline.okf().markdown.document(id).unwrap().tree();
    let current_tree = current.okf().markdown.document(id).unwrap().tree();

    assert!(!Arc::ptr_eq(
        baseline.okf().catalog.document(id).unwrap(),
        current_document
    ));
    assert!(!previous_tree.root().same_green(&current_tree.root()));
    let (source_backed, reused_static) = assert_rebased_identity(
        &GreenElement::Node(previous_tree.root_green().clone()),
        &GreenElement::Node(current_tree.root_green().clone()),
    );
    assert!(source_backed > 0);
    assert!(reused_static > 0);
    assert!(assert_current_source_provenance(current_tree, current_document.text().shared()) > 0);
    assert_eq!(
        current.source().document(&path).unwrap().text().as_ptr(),
        current_document.text().shared().as_str().as_ptr()
    );

    drop(baseline);
    saved.clone_from(current.source());
    assert!(prior_weak.upgrade().is_none());
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
            baseline.okf().markdown.document(id).unwrap(),
            identical.okf().markdown.document(id).unwrap(),
        ));
        assert!(Arc::ptr_eq(
            baseline.okf().markdown.document(id).unwrap().tree(),
            identical.okf().markdown.document(id).unwrap().tree(),
        ));
        assert!(Arc::ptr_eq(
            baseline.okf().markdown.document(id).unwrap().structure(),
            identical.okf().markdown.document(id).unwrap().structure(),
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
        baseline.okf().markdown.document(untouched).unwrap(),
        incremental.okf().markdown.document(untouched).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        baseline.okf().markdown.document(untouched).unwrap().tree(),
        incremental
            .okf()
            .markdown
            .document(untouched)
            .unwrap()
            .tree(),
    ));
    assert!(Arc::ptr_eq(
        baseline
            .okf()
            .markdown
            .document(untouched)
            .unwrap()
            .structure(),
        incremental
            .okf()
            .markdown
            .document(untouched)
            .unwrap()
            .structure(),
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
        .markdown
        .document(touched)
        .unwrap()
        .tree()
        .root()
        .same_green(
            &baseline
                .okf()
                .markdown
                .document(touched)
                .unwrap()
                .tree()
                .root()
        ));

    let old_children = baseline
        .okf()
        .markdown
        .document(touched)
        .unwrap()
        .tree()
        .root()
        .children()
        .collect::<Vec<_>>();
    let new_children = incremental
        .okf()
        .markdown
        .document(touched)
        .unwrap()
        .tree()
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
        let incremental_document = incremental.okf().catalog.document(incremental_id).unwrap();
        let incremental_tree = incremental
            .okf()
            .markdown
            .document(incremental_id)
            .unwrap()
            .tree();
        let full_tree = full.okf().markdown.document(full_id).unwrap().tree();
        assert!(
            assert_current_source_provenance(
                incremental_tree,
                incremental_document.text().shared()
            ) > 0
        );
        assert_eq!(
            incremental_tree.write_to_string(),
            full_tree.write_to_string()
        );
        assert_eq!(
            diagnostic_fingerprint(incremental_tree),
            diagnostic_fingerprint(full_tree)
        );
    }
}

#[test]
fn retained_uml_analysis_matches_full_declared_and_projection_oracles() {
    let source = SourceBundle::try_from_pairs([
        (
            "class.md",
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- name: String\n\n## Layout\n- Class\n",
        ),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ])
    .unwrap();
    let baseline = prepared(source.clone(), None, 1);
    let edited = replace_document(
        &source,
        SourceDocument::new(
            BundlePath::parse("class.md").unwrap(),
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- title: String\n\n## Layout\n- Class\n".into(),
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

    assert_eq!(incremental.source(), full.source());
    assert_eq!(incremental.uml().projection, full.uml().projection);
    assert_eq!(
        incremental.uml().declared.concepts().count(),
        full.uml().declared.concepts().count()
    );
    assert_eq!(
        declared_occurrence_fingerprint(&incremental.uml().declared),
        declared_occurrence_fingerprint(&full.uml().declared)
    );
    assert_eq!(
        semantic_diagnostic_fingerprint(incremental.uml()),
        semantic_diagnostic_fingerprint(full.uml())
    );
    let untouched = document_id(&baseline, "other.md");
    assert!(Arc::ptr_eq(
        baseline.uml().syntax.document(untouched).unwrap(),
        incremental.uml().syntax.document(untouched).unwrap(),
    ));
    for path in ["class.md", "other.md"] {
        let incremental_id = document_id(&incremental, path);
        let full_id = document_id(&full, path);
        let incremental_tree = incremental
            .uml()
            .syntax
            .document(incremental_id)
            .unwrap()
            .syntax();
        let full_tree = full.uml().syntax.document(full_id).unwrap().syntax();
        assert_eq!(
            incremental_tree.write_to_string(),
            full_tree.write_to_string()
        );
        assert_eq!(
            uml_fingerprint(incremental_tree),
            uml_fingerprint(full_tree)
        );
        assert_eq!(
            uml_diagnostic_fingerprint(incremental_tree),
            uml_diagnostic_fingerprint(full_tree)
        );
    }
}

#[test]
fn retained_uml_analysis_reuses_exact_unchanged_and_static_greens() {
    let source = SourceBundle::try_from_pairs([
        (
            "class.md",
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- old: String\n\n## Layout\n- left of\n",
        ),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ])
    .unwrap();
    let baseline = prepared(source.clone(), None, 1);
    let edited = replace_document(
        &source,
        SourceDocument::new(
            BundlePath::parse("class.md").unwrap(),
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- new: String\n\n## Layout\n- left of\n".into(),
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
    let touched = document_id(&baseline, "class.md");
    let untouched = document_id(&baseline, "other.md");
    assert!(Arc::ptr_eq(
        baseline.uml().syntax.document(untouched).unwrap(),
        incremental.uml().syntax.document(untouched).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        baseline.uml().syntax.document(untouched).unwrap().syntax(),
        incremental
            .uml()
            .syntax
            .document(untouched)
            .unwrap()
            .syntax(),
    ));
    assert!(!Arc::ptr_eq(
        baseline.uml().syntax.document(touched).unwrap(),
        incremental.uml().syntax.document(touched).unwrap(),
    ));
    assert!(!baseline
        .uml()
        .syntax
        .document(touched)
        .unwrap()
        .syntax()
        .root()
        .same_green(
            &incremental
                .uml()
                .syntax
                .document(touched)
                .unwrap()
                .syntax()
                .root(),
        ));
    let old_tree = baseline.uml().syntax.document(touched).unwrap().syntax();
    let new_tree = incremental.uml().syntax.document(touched).unwrap().syntax();
    let old_recovery = nth_missing_token(
        old_tree,
        UmlSyntaxKind::LayoutSection,
        UmlSyntaxKind::LayoutWordToken,
        0,
    );
    let new_recovery = nth_missing_token(
        new_tree,
        UmlSyntaxKind::LayoutSection,
        UmlSyntaxKind::LayoutWordToken,
        0,
    );
    assert!(old_recovery.same_green(&new_recovery));
    let (source_backed, reused_static) = assert_uml_rebased_identity(
        &GreenElement::Node(old_tree.root_green().clone()),
        &GreenElement::Node(new_tree.root_green().clone()),
    );
    assert!(source_backed > 0);
    assert!(reused_static > 0);
    let current = incremental
        .okf()
        .catalog
        .document(touched)
        .unwrap()
        .text()
        .shared();
    assert_eq!(new_tree.write_to_string(), current.as_str());
    assert_eq!(
        uml_fingerprint(new_tree),
        uml_fingerprint(full.uml().syntax.document(touched).unwrap().syntax())
    );
    assert_eq!(
        uml_diagnostic_fingerprint(new_tree),
        uml_diagnostic_fingerprint(full.uml().syntax.document(touched).unwrap().syntax())
    );
    assert!(assert_uml_current_source_provenance(new_tree, current) > 0);
    assert!(Arc::ptr_eq(
        incremental
            .uml()
            .syntax
            .document(touched)
            .unwrap()
            .document()
            .text()
            .shared(),
        current,
    ));
}

#[test]
fn retained_uml_analysis_falls_back_at_unsafe_heading_boundary() {
    let source = SourceBundle::try_from_pairs([(
        "class.md",
        "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- old: String\n\n## Layout\n- left of\n",
    )])
    .unwrap();
    let baseline = prepared(source.clone(), None, 1);
    let edited = replace_document(
        &source,
        SourceDocument::new(
            BundlePath::parse("class.md").unwrap(),
            "---\ntype: uml.Class\n---\n# Class\n\n## attributes\n- old: String\n\n## Layout\n- left of\n".into(),
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
    let incremental_id = document_id(&incremental, "class.md");
    let full_id = document_id(&full, "class.md");
    let previous_tree = baseline
        .uml()
        .syntax
        .document(document_id(&baseline, "class.md"))
        .unwrap()
        .syntax();
    let incremental_tree = incremental
        .uml()
        .syntax
        .document(incremental_id)
        .unwrap()
        .syntax();
    let full_tree = full.uml().syntax.document(full_id).unwrap().syntax();

    assert_eq!(incremental.source(), full.source());
    assert_eq!(
        uml_fingerprint(incremental_tree),
        uml_fingerprint(full_tree)
    );
    assert_eq!(
        uml_diagnostic_fingerprint(incremental_tree),
        uml_diagnostic_fingerprint(full_tree)
    );
    assert_eq!(
        declared_occurrence_fingerprint(&incremental.uml().declared),
        declared_occurrence_fingerprint(&full.uml().declared)
    );
    assert_eq!(incremental.uml().projection, full.uml().projection);
    assert_eq!(
        semantic_diagnostic_fingerprint(incremental.uml()),
        semantic_diagnostic_fingerprint(full.uml())
    );
    assert!(!Arc::ptr_eq(previous_tree, incremental_tree));
    let current = incremental
        .okf()
        .catalog
        .document(incremental_id)
        .unwrap()
        .text()
        .shared();
    assert!(assert_uml_current_source_provenance(incremental_tree, current) > 0);
}

#[test]
fn retained_uml_analysis_never_cross_wires_snapshot_provenance() {
    let source = SourceBundle::try_from_pairs([
        (
            "class.md",
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- name: String\n",
        ),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ])
    .unwrap();
    let a = prepared(source.clone(), None, 1);
    let b = prepared(source.clone(), None, 2);
    for path in ["class.md", "other.md"] {
        let id = document_id(&a, path);
        let a_document = a.okf().catalog.document(id).unwrap();
        let b_document = b.okf().catalog.document(id).unwrap();
        assert!(!Arc::ptr_eq(a_document, b_document));
        assert!(Arc::ptr_eq(
            a_document.text().shared(),
            b_document.text().shared()
        ));
    }

    let current = prepared(
        source,
        Some(PreviousAnalyses {
            okf: b.okf(),
            uml: a.uml(),
        }),
        3,
    );
    assert_eq!(current.revision(), 3);
    assert_eq!(current.uml().session_revision(), 3);
    for path in ["class.md", "other.md"] {
        let id = document_id(&current, path);
        let catalog_document = current.okf().catalog.document(id).unwrap();
        let snapshot = current.uml().syntax.document(id).unwrap();
        assert!(Arc::ptr_eq(catalog_document, snapshot.document()));
        assert_eq!(snapshot.document().revision(), catalog_document.revision());
        assert!(Arc::ptr_eq(
            snapshot.document().text().shared(),
            catalog_document.text().shared()
        ));
        let source_document = current
            .source()
            .document(&BundlePath::parse(path).unwrap())
            .unwrap();
        assert_eq!(
            snapshot.document().text().shared().as_ptr(),
            source_document.text().as_ptr()
        );
    }
}

#[test]
fn markdown_snapshots_promote_updates_and_isolate_broken_islands() {
    let source = SourceBundle::try_from_pairs([
        (
            "class.md",
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- first: String\n\n## Values\n- Ready\n",
        ),
        ("other.md", "---\ntype: uml.Class\n---\n# Other\n"),
    ])
    .unwrap();
    let baseline = prepared(source.clone(), None, 1);
    let class = document_id(&baseline, "class.md");
    let other = document_id(&baseline, "other.md");
    let baseline_values = baseline
        .uml()
        .island_syntax
        .document(class)
        .unwrap()
        .values()
        .find(|snapshot| snapshot.kind() == WamlSectionKind::Values)
        .unwrap()
        .clone();
    assert_eq!(
        baseline_values.syntax().write_to_string(),
        "## Values\n- Ready\n"
    );
    let edited = replace_document(
        &source,
        SourceDocument::new(
            BundlePath::parse("class.md").unwrap(),
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- first String\n\n## Values\n- Ready\n".into(),
        ),
    )
    .unwrap();
    let current = prepared(
        edited,
        Some(PreviousAnalyses {
            okf: baseline.okf(),
            uml: baseline.uml(),
        }),
        2,
    );

    assert!(!Arc::ptr_eq(
        baseline.okf().markdown.document(class).unwrap(),
        current.okf().markdown.document(class).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        baseline.okf().markdown.document(other).unwrap(),
        current.okf().markdown.document(other).unwrap(),
    ));
    assert!(Arc::ptr_eq(
        baseline.uml().syntax.document(other).unwrap().syntax(),
        current.uml().syntax.document(other).unwrap().syntax(),
    ));
    let current_values = current
        .uml()
        .island_syntax
        .by_owner(class, baseline_values.owner())
        .unwrap();
    assert!(Arc::ptr_eq(
        baseline_values.syntax(),
        current_values.syntax(),
    ));
    assert!(current.uml().declared.concept("class").is_some());
}
