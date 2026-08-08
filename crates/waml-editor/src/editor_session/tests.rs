use super::*;
use crate::document::EditIntent;
use crate::markdown_analysis::{
    run_semantic_request, run_semantic_request_with_preparer, CompletionInstall,
};
use crate::view_history::{DocumentLocator, ViewAnchor, ViewLocation};
use std::{num::NonZeroU64, sync::Arc};
use waml::action::{ActionBasis, CodeAction, SyntaxChangeBatch, TextEdit, VersionedDocumentChange};
use waml::analysis::AnalysisStage;
use waml::layout::Direction;
use waml::source::BundlePath;
use waml::uml::Op;
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    edit::{EditCommand as MarkdownEditCommand, HistoryGroup, ProposedMarkdownEdit},
    session::MarkdownDocumentSession,
};
use waml_syntax::{
    annotate_occurrence, find_annotation, GreenElement, GreenText, MarkdownDialect, RewriteError,
    SyntaxAnnotation, SyntaxTree,
};

fn source(pairs: Vec<(String, String)>) -> SourceBundle {
    SourceBundle::try_from_pairs(pairs).unwrap()
}

fn diagram_bundle(layout: &str) -> SourceBundle {
    source(vec![(
        "dia.md".to_string(),
        format!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n{layout}"
        ),
    )])
}

fn source_location(concept_id: &str) -> ViewLocation {
    ViewLocation {
        document: DocumentLocator::concept(concept_id, waml::view::surface::SurfaceId::markdown()),
        anchor: ViewAnchor::None,
    }
}

fn source_session(text: &str) -> EditorSession {
    let mut session = EditorSession::default();
    session
        .replace(source(vec![("order.md".into(), text.into())]))
        .unwrap();
    session
}

fn set_current_source(session: &mut EditorSession, source: SourceBundle) {
    let mut current = (*session.current).clone();
    current.source = Arc::new(source);
    session.current = Arc::new(current);
}

fn install_prepared_for_test(session: &mut EditorSession, prepared: PreparedCandidate) {
    let persisted = session.current.persisted_source.clone();
    let affected = session.current.affected_documents.clone();
    let dirty = session.current.dirty_revision;
    session.current = snapshot_from_prepared(prepared, Some(persisted), affected);
    session.set_dirty_revision(dirty);
}

fn document_id(snapshot: &EditorSessionSnapshot, path: &str) -> waml::analysis::DocumentId {
    snapshot
        .okf_analysis
        .catalog
        .id_for_path(&BundlePath::parse(path).unwrap())
        .unwrap()
}

fn local_replacement(
    syntax: Arc<waml_syntax::MarkdownSyntaxSnapshot>,
    replacement: &str,
) -> ProposedMarkdownEdit {
    let mut local = MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)));
    local.select_all().unwrap();
    local
        .execute(
            MarkdownEditCommand::ReplaceSelections(Arc::from(replacement)),
            HistoryGroup::isolated(),
        )
        .unwrap()
        .proposal
        .unwrap()
}

fn local_insert(
    syntax: Arc<waml_syntax::MarkdownSyntaxSnapshot>,
    offset: usize,
    inserted: &str,
) -> ProposedMarkdownEdit {
    let mut local = MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)));
    local
        .set_primary_offset(waml_syntax::TextSize::new(offset.try_into().unwrap()))
        .unwrap();
    local
        .execute(
            MarkdownEditCommand::Insert(Arc::from(inserted)),
            HistoryGroup::isolated(),
        )
        .unwrap()
        .proposal
        .unwrap()
}

// Scenario: NATIVE-027
// Scenario: NATIVE-062
#[test]
fn accepted_source_edit_advances_once_and_promotes_the_same_syntax_arc() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let id = document_id(&before, "order.md");
    let local = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let syntax = local.syntax_update.snapshot.clone();

    let (change, request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, local),
            source_location("order"),
        )
        .unwrap();

    let promoted = session.snapshot();
    assert_eq!(promoted.revision, before.revision + 1);
    assert_eq!(change.revision, promoted.revision);
    assert_eq!(promoted.dirty_revision, Some(promoted.revision));
    assert!(Arc::ptr_eq(
        promoted.markdown_snapshot(id).unwrap(),
        &syntax
    ));
    let completion = run_semantic_request(request).unwrap();
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));
    assert!(Arc::ptr_eq(
        session
            .snapshot()
            .okf_analysis
            .markdown_snapshot(id)
            .unwrap(),
        &syntax,
    ));
    assert!(Arc::ptr_eq(
        session
            .snapshot()
            .okf_analysis
            .catalog
            .document(id)
            .unwrap()
            .text()
            .shared(),
        syntax.text().shared(),
    ));
}

// Scenario: NATIVE-032
#[test]
fn pending_source_changes_map_only_unchanged_diagnostic_ranges() {
    let mut session = source_session("# Order\nBody\n");
    let before = session.snapshot();
    let document = document_id(&before, "order.md");
    let body_range = waml_syntax::TextRange::new(
        waml_syntax::TextSize::new(8),
        waml_syntax::TextSize::new(12),
    )
    .unwrap();
    let insert = local_insert(before.markdown_snapshot(document).unwrap().clone(), 0, "X");
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(document, insert),
            source_location("order"),
        )
        .unwrap();

    assert_eq!(
        session.snapshot().map_source_range_to_current(
            document,
            before.markdown_snapshot(document).unwrap().revision(),
            body_range,
        ),
        Ok(waml_syntax::TextRange::new(
            waml_syntax::TextSize::new(9),
            waml_syntax::TextSize::new(13),
        )
        .unwrap())
    );

    let pending = session.snapshot();
    let replacement = local_replacement(
        pending.markdown_snapshot(document).unwrap().clone(),
        "# Order\nChanged\n",
    );
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(document, replacement),
            source_location("order"),
        )
        .unwrap();
    assert_eq!(
        session.snapshot().map_source_range_to_current(
            document,
            before.markdown_snapshot(document).unwrap().revision(),
            body_range,
        ),
        Err(SourceRangeMapError::RangeChanged)
    );
}

#[test]
fn semantic_completion_installs_affected_documents_and_diagrams() {
    let mut session = EditorSession::default();
    session.replace(diagram_bundle("")).unwrap();
    let before = session.snapshot();
    let document = document_id(&before, "dia.md");
    let local = local_replacement(
        before.markdown_snapshot(document).unwrap().clone(),
        "---\ntype: Diagram\ntitle: Changed\nprofile: uml-domain\n---\n# D\n\n## Layout\n",
    );
    let request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(document, local),
            source_location("dia"),
        )
        .unwrap()
        .1;

    let completion = run_semantic_request(request).unwrap();
    assert_eq!(completion.prepared.affected.documents.as_ref(), &[document]);
    assert_eq!(
        completion
            .prepared
            .affected
            .diagrams
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>(),
        ["dia"]
    );
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));

    let installed = session.snapshot();
    assert_eq!(installed.affected_documents.as_ref(), &[document]);
    assert_eq!(
        installed
            .affected_diagrams
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>(),
        ["dia"]
    );
}

// Scenario: NATIVE-049
#[test]
fn stale_or_invalid_source_proposals_do_not_mutate_snapshot_or_history() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let before_state = session.history_state();
    let id = document_id(&before, "order.md");
    let local = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let mut stale = ProposedSourceEdit::from_local(id, local.clone());
    stale.base_revision = stale.base_revision.checked_next().unwrap();

    assert!(matches!(
        session.promote_source_edit(stale, source_location("order")),
        Err(SourceEditError::StaleBaseRevision { .. })
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &before));
    assert_eq!(session.history_state(), before_state);

    let mut invalid = ProposedSourceEdit::from_local(id, local);
    let duplicate = invalid.changes[0].clone();
    invalid.changes = Arc::from([duplicate.clone(), duplicate]);
    assert!(matches!(
        session.promote_source_edit(invalid, source_location("order")),
        Err(SourceEditError::InvalidChanges { .. })
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &before));
    assert_eq!(session.history_state(), before_state);
}

#[test]
fn mismatched_syntax_text_is_rejected_before_history_insertion() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let before_state = session.history_state();
    let id = document_id(&before, "order.md");
    let purchase = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let customer = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Customer\n",
    );
    let mut proposal = ProposedSourceEdit::from_local(id, purchase);
    proposal.syntax_update = customer.syntax_update;

    assert!(matches!(
        session.promote_source_edit(proposal, source_location("order")),
        Err(SourceEditError::ResultTextMismatch { .. })
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &before));
    assert_eq!(session.history_state(), before_state);
}

#[test]
fn stale_completions_and_equal_bytes_from_another_source_arc_are_ignored() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let id = document_id(&before, "order.md");
    let first = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let (_, first_request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, first),
            source_location("order"),
        )
        .unwrap();
    let stale_first_completion = run_semantic_request(first_request.clone()).unwrap();
    assert!(matches!(
        session.install_semantic_completion(run_semantic_request(first_request).unwrap()),
        CompletionInstall::Installed(_)
    ));
    let current = session.snapshot();
    let second = local_replacement(
        current.markdown_snapshot(id).unwrap().clone(),
        "# Customer\n",
    );
    let (_, second_request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, second),
            source_location("order"),
        )
        .unwrap();
    let second_completion = run_semantic_request(second_request).unwrap();
    assert!(matches!(
        session.install_semantic_completion(second_completion),
        CompletionInstall::Installed(_)
    ));
    let after_newer = session.snapshot();
    assert!(matches!(
        session.install_semantic_completion(stale_first_completion),
        CompletionInstall::IgnoredStale
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &after_newer));

    let current = session.snapshot();
    let third = local_replacement(
        current.markdown_snapshot(id).unwrap().clone(),
        "# Invoice\n",
    );
    let (_, request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, third),
            source_location("order"),
        )
        .unwrap();
    let mut completion = run_semantic_request(request).unwrap();
    completion.source = Arc::new((*completion.source).clone());
    let before_install = session.snapshot();
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::IgnoredStale
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &before_install));
}

#[test]
fn proposal_uses_published_syntax_when_semantic_catalog_lags() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let id = document_id(&before, "order.md");
    let first = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, first),
            source_location("order"),
        )
        .unwrap();
    let pending = session.snapshot();
    assert!(Arc::ptr_eq(
        pending.source.documents()[0].text_shared(),
        pending.markdown_snapshot(id).unwrap().text().shared(),
    ));
    assert_ne!(
        pending.markdown_snapshot(id).unwrap().revision(),
        pending
            .okf_analysis
            .markdown_snapshot(id)
            .unwrap()
            .revision(),
        "fixture must keep semantic catalog one accepted edit behind",
    );
    let second = local_replacement(
        pending.markdown_snapshot(id).unwrap().clone(),
        "# Customer\n",
    );
    let expected = second.syntax_update.snapshot.clone();

    let (_, second_request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, second),
            source_location("order"),
        )
        .unwrap();

    assert!(Arc::ptr_eq(
        session.snapshot().markdown_snapshot(id).unwrap(),
        &expected,
    ));
    let second_pending = session.snapshot();
    assert!(Arc::ptr_eq(
        second_pending.source.documents()[0].text_shared(),
        second_pending
            .markdown_snapshot(id)
            .unwrap()
            .text()
            .shared(),
    ));
    assert!(
        second_pending
            .okf_analysis
            .markdown_snapshot(id)
            .unwrap()
            .revision()
            < second_pending.markdown_snapshot(id).unwrap().revision(),
        "the semantic catalog must remain behind the second accepted source revision",
    );

    let completion = run_semantic_request(second_request).unwrap();
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));
    assert!(Arc::ptr_eq(
        session
            .snapshot()
            .okf_analysis
            .markdown_snapshot(id)
            .unwrap(),
        &expected,
    ));

    session.undo().unwrap().unwrap();
    assert_eq!(session.source().documents()[0].text(), "# Purchase\n");
}

#[test]
fn three_pending_edits_replay_every_successor_without_reparsing() {
    let mut session = source_session("# Order\n");
    let id = document_id(&session.snapshot(), "order.md");
    let mut final_request = None;
    let mut final_syntax = None;
    for replacement in ["# Purchase\n", "# Customer\n", "# Invoice\n"] {
        let local = local_replacement(
            session.snapshot().markdown_snapshot(id).unwrap().clone(),
            replacement,
        );
        final_syntax = Some(local.syntax_update.snapshot.clone());
        final_request = Some(
            session
                .promote_source_edit(
                    ProposedSourceEdit::from_local(id, local),
                    source_location("order"),
                )
                .unwrap()
                .1,
        );
    }

    let completion = run_semantic_request(final_request.unwrap()).unwrap();
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));
    assert!(Arc::ptr_eq(
        session
            .snapshot()
            .okf_analysis
            .markdown_snapshot(id)
            .unwrap(),
        &final_syntax.unwrap(),
    ));
}

#[test]
fn immediate_undo_discards_pending_semantic_steps_and_restores_prior_text() {
    let mut session = source_session("# Order\n");
    let id = document_id(&session.snapshot(), "order.md");
    for replacement in ["# Purchase\n", "# Customer\n"] {
        let local = local_replacement(
            session.snapshot().markdown_snapshot(id).unwrap().clone(),
            replacement,
        );
        session
            .promote_source_edit(
                ProposedSourceEdit::from_local(id, local),
                source_location("order"),
            )
            .unwrap();
    }

    session.undo().unwrap().unwrap();

    assert_eq!(session.source().documents()[0].text(), "# Purchase\n");
    assert!(session.snapshot().pending_semantic_steps.is_empty());
}

#[test]
fn interleaved_pending_documents_publish_each_exact_syntax_arc() {
    let mut session = EditorSession::default();
    session
        .replace(source(vec![
            ("order.md".into(), "# Order\n".into()),
            ("customer.md".into(), "# Customer\n".into()),
        ]))
        .unwrap();
    let initial = session.snapshot();
    let order = document_id(&initial, "order.md");
    let customer = document_id(&initial, "customer.md");

    let first = local_replacement(
        initial.markdown_snapshot(order).unwrap().clone(),
        "# Purchase\n",
    );
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(order, first),
            source_location("order"),
        )
        .unwrap();
    let second = local_replacement(
        session
            .snapshot()
            .markdown_snapshot(customer)
            .unwrap()
            .clone(),
        "# Account\n",
    );
    let customer_syntax = second.syntax_update.snapshot.clone();
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(customer, second),
            source_location("customer"),
        )
        .unwrap();
    let third = local_replacement(
        session.snapshot().markdown_snapshot(order).unwrap().clone(),
        "# Invoice\n",
    );
    let order_syntax = third.syntax_update.snapshot.clone();
    let request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(order, third),
            source_location("order"),
        )
        .unwrap()
        .1;

    assert!(matches!(
        session.install_semantic_completion(run_semantic_request(request).unwrap()),
        CompletionInstall::Installed(_)
    ));
    let installed = session.snapshot();
    assert!(Arc::ptr_eq(
        installed.okf_analysis.markdown_snapshot(order).unwrap(),
        &order_syntax,
    ));
    assert!(Arc::ptr_eq(
        installed.okf_analysis.markdown_snapshot(customer).unwrap(),
        &customer_syntax,
    ));
}

#[test]
fn newer_completion_installs_before_older_completion_without_pruning_state() {
    let mut session = source_session("# Order\n");
    let id = document_id(&session.snapshot(), "order.md");
    let first = local_replacement(
        session.snapshot().markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let first_request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, first),
            source_location("order"),
        )
        .unwrap()
        .1;
    let first_completion = run_semantic_request(first_request).unwrap();
    let second = local_replacement(
        session.snapshot().markdown_snapshot(id).unwrap().clone(),
        "# Customer\n",
    );
    let second_syntax = second.syntax_update.snapshot.clone();
    let second_request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, second),
            source_location("order"),
        )
        .unwrap()
        .1;

    assert!(matches!(
        session.install_semantic_completion(run_semantic_request(second_request).unwrap()),
        CompletionInstall::Installed(_)
    ));
    let after_newer = session.snapshot();
    assert!(matches!(
        session.install_semantic_completion(first_completion),
        CompletionInstall::IgnoredStale
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &after_newer));
    assert!(Arc::ptr_eq(
        after_newer.okf_analysis.markdown_snapshot(id).unwrap(),
        &second_syntax,
    ));
}

#[test]
fn retained_completion_clone_does_not_control_install_authority() {
    let mut session = source_session("# Order\n");
    let id = document_id(&session.snapshot(), "order.md");
    let local = local_replacement(
        session.snapshot().markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, local),
            source_location("order"),
        )
        .unwrap()
        .1;
    let completion = run_semantic_request(request).unwrap();
    let _retained = completion.clone();

    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));
}

#[test]
fn mismatched_document_path_is_an_invariant_and_preserves_state() {
    let mut session = source_session("# Order\n");
    let initial = session.snapshot();
    let id = document_id(&initial, "order.md");
    let local = local_replacement(
        initial.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let mut corrupted = (*initial).clone();
    let mut paths = (*corrupted.document_paths).clone();
    paths.insert(id, BundlePath::parse("missing.md").unwrap());
    corrupted.document_paths = Arc::new(paths);
    session.current = Arc::new(corrupted);
    let before = session.snapshot();
    let history = session.history_state();

    assert!(matches!(
        session.promote_source_edit(
            ProposedSourceEdit::from_local(id, local),
            source_location("order"),
        ),
        Err(SourceEditError::DocumentPathInvariant { document }) if document == id
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &before));
    assert_eq!(session.history_state(), history);
}

#[test]
fn copied_source_base_is_rejected_without_mutating_snapshot_or_history() {
    let mut session = source_session("# Order\n");
    let initial = session.snapshot();
    let id = document_id(&initial, "order.md");
    let local = local_replacement(
        initial.markdown_snapshot(id).unwrap().clone(),
        "# Purchase\n",
    );
    let copied = SourceBundle::try_from_pairs([("order.md", "# Order\n")]).unwrap();
    assert!(!Arc::ptr_eq(
        copied.documents()[0].text_shared(),
        initial.markdown_snapshot(id).unwrap().text().shared(),
    ));
    let mut corrupted = (*initial).clone();
    corrupted.source = Arc::new(copied);
    session.current = Arc::new(corrupted);
    let before = session.snapshot();
    let history = session.history_state();

    assert!(matches!(
        session.promote_source_edit(
            ProposedSourceEdit::from_local(id, local),
            source_location("order"),
        ),
        Err(SourceEditError::BaseIdentityMismatch { document }) if document == id
    ));
    assert!(Arc::ptr_eq(&session.snapshot(), &before));
    assert_eq!(session.history_state(), history);
}

#[test]
fn semantic_failure_completion_keeps_the_accepted_source_and_fallback_syntax() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let id = document_id(&before, "order.md");
    let local = local_replacement(
        before.markdown_snapshot(id).unwrap().clone(),
        "# Literal invalid island ???\n",
    );
    let (_, request) = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(id, local),
            source_location("order"),
        )
        .unwrap();
    let accepted = session.snapshot();

    let completion = run_semantic_request_with_preparer(request, |_, _, _, _| {
        Err(AnalysisError::StructuralInvariant {
            stage: AnalysisStage::Okf,
            reason: "injected semantic failure".into(),
        })
    })
    .unwrap();

    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));
    let after = session.snapshot();
    assert!(Arc::ptr_eq(&after.source, &accepted.source));
    assert_eq!(
        after.source.documents()[0].text(),
        "# Literal invalid island ???\n"
    );
    assert!(Arc::ptr_eq(
        after.okf_analysis.markdown_snapshot(id).unwrap(),
        before.okf_analysis.markdown_snapshot(id).unwrap(),
    ));
    assert!(after.semantic_diagnostics.contains_key(&id));
}

#[test]
fn one_failed_semantic_island_keeps_only_its_fallback_and_installs_the_success() {
    let mut session = EditorSession::default();
    session
        .replace(source(vec![
            ("order.md".into(), "# Order\n".into()),
            ("customer.md".into(), "# Customer\n".into()),
        ]))
        .unwrap();
    let initial = session.snapshot();
    let order = document_id(&initial, "order.md");
    let customer = document_id(&initial, "customer.md");
    let customer_fallback = initial
        .okf_analysis
        .markdown_snapshot(customer)
        .unwrap()
        .clone();

    let order_edit = local_replacement(
        initial.markdown_snapshot(order).unwrap().clone(),
        "# Purchase\n",
    );
    let order_syntax = order_edit.syntax_update.snapshot.clone();
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(order, order_edit),
            source_location("order"),
        )
        .unwrap();
    let customer_edit = local_replacement(
        session
            .snapshot()
            .markdown_snapshot(customer)
            .unwrap()
            .clone(),
        "# Broken customer island\n",
    );
    let request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(customer, customer_edit),
            source_location("customer"),
        )
        .unwrap()
        .1;
    let accepted = session.snapshot();

    let completion =
        run_semantic_request_with_preparer(request, |source, previous, revision, promoted| {
            if promoted[0].document == customer {
                Err(AnalysisError::StructuralInvariant {
                    stage: AnalysisStage::Okf,
                    reason: "injected customer island failure".into(),
                })
            } else {
                waml::analysis::prepare_candidate_with_markdown_updates(
                    source, previous, revision, promoted,
                )
            }
        })
        .unwrap();
    assert!(matches!(
        session.install_semantic_completion(completion),
        CompletionInstall::Installed(_)
    ));

    let installed = session.snapshot();
    assert!(Arc::ptr_eq(&installed.source, &accepted.source));
    assert!(Arc::ptr_eq(
        installed.okf_analysis.markdown_snapshot(order).unwrap(),
        &order_syntax,
    ));
    assert!(Arc::ptr_eq(
        installed.okf_analysis.markdown_snapshot(customer).unwrap(),
        &customer_fallback,
    ));
    let diagnostic = installed.semantic_diagnostics.get(&customer).unwrap();
    assert_eq!(diagnostic.document, customer);
    assert_eq!(diagnostic.session_revision, installed.revision);
    assert!(matches!(
        diagnostic.error.as_ref(),
        AnalysisError::StructuralInvariant {
            stage: AnalysisStage::Okf,
            ..
        }
    ));
    assert!(!installed.semantic_diagnostics.contains_key(&order));

    let recovery = local_replacement(
        installed.markdown_snapshot(customer).unwrap().clone(),
        "# Recovered customer\n",
    );
    let recovery_request = session
        .promote_source_edit(
            ProposedSourceEdit::from_local(customer, recovery),
            source_location("customer"),
        )
        .unwrap()
        .1;
    assert!(matches!(
        session.install_semantic_completion(run_semantic_request(recovery_request).unwrap()),
        CompletionInstall::Installed(_)
    ));
    let recovered = session.snapshot();
    assert!(!recovered.semantic_diagnostics.contains_key(&customer));
    assert_eq!(
        recovered
            .okf_analysis
            .catalog
            .document(customer)
            .unwrap()
            .text()
            .shared()
            .as_str(),
        "# Recovered customer\n",
    );
}

fn token_content_range<L: waml_syntax::SyntaxLanguage>(
    token: &waml_syntax::SyntaxToken<L>,
) -> waml_syntax::TextRange {
    let zero = waml_syntax::TextSize::try_from(0usize).unwrap();
    let leading = token
        .leading_trivia()
        .iter()
        .try_fold(zero, |sum, trivia| {
            let width =
                waml_syntax::TextSize::try_from(trivia.text.write_to_string().len()).unwrap();
            sum.checked_add(width)
        })
        .unwrap();
    let content = waml_syntax::TextSize::try_from(token.text().write_to_string().len()).unwrap();
    let trailing = token
        .trailing_trivia()
        .iter()
        .try_fold(zero, |sum, trivia| {
            let width =
                waml_syntax::TextSize::try_from(trivia.text.write_to_string().len()).unwrap();
            sum.checked_add(width)
        })
        .unwrap();
    let start = token.range().start().checked_add(leading).unwrap();
    let end = start.checked_add(content).unwrap();
    assert_eq!(
        end.checked_add(trailing).unwrap(),
        token.range().end(),
        "content plus both trivia sides must cover the token",
    );
    waml_syntax::TextRange::new(start, end).unwrap()
}

fn unique_token_content_range<L: waml_syntax::SyntaxLanguage>(
    tree: &SyntaxTree<L>,
    spelling: &str,
) -> waml_syntax::TextRange {
    fn visit<L: waml_syntax::SyntaxLanguage>(
        node: &waml_syntax::SyntaxNode<L>,
        spelling: &str,
        matches: &mut Vec<waml_syntax::TextRange>,
    ) {
        for child in node.children() {
            match child {
                waml_syntax::SyntaxElement::Node(node) => visit(&node, spelling, matches),
                waml_syntax::SyntaxElement::Token(token)
                    if token.text().write_to_string() == spelling =>
                {
                    matches.push(token_content_range(&token));
                }
                waml_syntax::SyntaxElement::Token(_) => {}
            }
        }
    }

    let mut matches = Vec::new();
    visit(&tree.root(), spelling, &mut matches);
    assert_eq!(matches.len(), 1, "test token spelling must be unique");
    matches[0]
}

fn assert_all_source_slices_match(
    element: &GreenElement<waml::uml::syntax::UmlLanguage>,
    current: &waml_syntax::SourceText,
) {
    fn text_matches(text: &GreenText, current: &waml_syntax::SourceText) {
        if let GreenText::SourceSlice { source, .. } = text {
            assert!(Arc::ptr_eq(source.shared(), current.shared()));
        }
    }
    match element {
        GreenElement::Node(node) => node
            .children()
            .iter()
            .for_each(|child| assert_all_source_slices_match(child, current)),
        GreenElement::Token(token) => {
            text_matches(token.text(), current);
            token
                .leading_trivia()
                .iter()
                .for_each(|trivia| text_matches(&trivia.text, current));
            token
                .trailing_trivia()
                .iter()
                .for_each(|trivia| text_matches(&trivia.text, current));
        }
    }
}

fn assert_clean_layout_alignment(tree: &SyntaxTree<waml::uml::syntax::UmlLanguage>) {
    fn collect_alignments(
        node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
        alignments: &mut Vec<waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>>,
    ) {
        if node.kind() == waml::uml::syntax::UmlSyntaxKind::LayoutAlignment {
            alignments.push(node.clone());
        }
        for child in node
            .children()
            .filter_map(waml_syntax::SyntaxElement::into_node)
        {
            collect_alignments(child, alignments);
        }
    }

    assert!(
        tree.diagnostics().is_empty(),
        "fixture must stay on clean UML syntax: {:#?}",
        tree.diagnostics(),
    );
    let mut alignments = Vec::new();
    collect_alignments(tree.root(), &mut alignments);
    assert_eq!(alignments.len(), 1, "fixture must contain one alignment");
    assert_eq!(
        alignments[0]
            .children()
            .map(|child| child.kind())
            .collect::<Vec<_>>(),
        vec![
            waml::uml::syntax::UmlSyntaxKind::Anchored,
            waml::uml::syntax::UmlSyntaxKind::DirectionClause,
            waml::uml::syntax::UmlSyntaxKind::Anchored,
        ],
        "alignment must retain both anchors and its join clause",
    );
}

#[test]
fn repeated_atomic_edits_bound_sources_and_preserve_mapped_annotation() {
    fn first_attribute(
        node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
    ) -> waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage> {
        fn find(
            node: waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>,
        ) -> Option<waml_syntax::SyntaxNode<waml::uml::syntax::UmlLanguage>> {
            if node.kind() == waml::uml::syntax::UmlSyntaxKind::Attribute {
                return Some(node);
            }
            node.children()
                .find_map(|child| child.into_node().and_then(find))
        }

        find(node).expect("fixture must contain an attribute node")
    }

    let mut session = EditorSession::default();
    session
        .replace(source(vec![
            (
                "class.md".into(),
                "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- name: String\n\n## Layout\n-left of Class aligned with Class\n"
                    .into(),
            ),
            ("notes.md".into(), "# Notes\nUntouched\n".into()),
        ]))
        .unwrap();
    let document_id = session
        .uml_analysis()
        .syntax
        .catalog()
        .id_for_path(&BundlePath::parse("class.md").unwrap())
        .unwrap();
    let old_tree = session
        .uml_analysis()
        .syntax
        .document(document_id)
        .unwrap()
        .syntax();
    assert_clean_layout_alignment(old_tree);
    let attribute_island = session
        .uml_analysis()
        .island_syntax
        .document(document_id)
        .unwrap()
        .values()
        .find(|snapshot| snapshot.kind() == waml_syntax::WamlSectionKind::Attributes)
        .unwrap();
    let old_attribute = first_attribute(attribute_island.syntax().root());
    let old_locator = old_attribute.locator();
    let annotation = SyntaxAnnotation::new(NonZeroU64::new(22).unwrap(), "selection", None);
    let annotation_id = annotation.id();
    let annotated_tree = Arc::new(SyntaxTree::new(
        annotate_occurrence(attribute_island.syntax(), &old_locator, annotation).unwrap(),
        Arc::from(attribute_island.syntax().diagnostics()),
        MarkdownDialect::WAML_DEFAULT,
    ));
    let replacement_syntax = waml::uml::analysis::test_support::island_syntax_with_replaced_tree(
        session.uml_analysis(),
        document_id,
        attribute_island.owner(),
        annotated_tree.clone(),
    )
    .unwrap();
    Arc::get_mut(&mut Arc::get_mut(&mut session.current).unwrap().uml_analysis)
        .unwrap()
        .island_syntax = replacement_syntax;
    let baseline_current = session.source().clone();
    let baseline_persisted = session.persisted_bundle().clone();

    for iteration in 0..32 {
        let snapshot = session.uml_analysis().syntax.document(document_id).unwrap();
        assert_clean_layout_alignment(snapshot.syntax());
        let current_document = snapshot.document();
        let authored = current_document.text().shared();
        let (needle, replacement) = if authored.contains("left") {
            ("left", "right")
        } else {
            ("right", "left")
        };
        let edit = TextEdit {
            range: unique_token_content_range(snapshot.syntax(), needle),
            replacement: Arc::from(replacement),
        };
        let action = CodeAction {
            title: format!("toggle layout direction {iteration}"),
            basis: ActionBasis::Document {
                document: document_id,
                document_revision: current_document.revision(),
                session_revision: session.revision(),
            },
            changes: Arc::from([VersionedDocumentChange {
                document: document_id,
                base_document_revision: current_document.revision(),
                edits: Arc::from([edit]),
            }]),
        };
        let batch = SyntaxChangeBatch::new(action).unwrap();
        session.apply(batch).unwrap();
        assert_clean_layout_alignment(
            session
                .uml_analysis()
                .syntax
                .document(document_id)
                .unwrap()
                .syntax(),
        );
    }

    assert!(session.is_dirty());
    assert!(!session
        .source()
        .shares_text_with(session.persisted_bundle(), "class.md"));
    assert!(session
        .source()
        .shares_text_with(session.persisted_bundle(), "notes.md"));
    assert!(session
        .source()
        .shares_text_with(&baseline_current, "notes.md"));
    assert!(session
        .persisted_bundle()
        .shares_text_with(&baseline_persisted, "class.md"));

    let final_snapshot = session.uml_analysis().syntax.document(document_id).unwrap();
    let final_tree = final_snapshot.syntax();
    let final_attribute = first_attribute(final_tree.root());
    let mapped_annotations = find_annotation(final_tree, annotation_id);
    assert_eq!(mapped_annotations.len(), 1);
    let mapped_attribute = &mapped_annotations[0];
    assert!(mapped_attribute.same_green(&final_attribute));
    assert_eq!(
        mapped_attribute
            .syntax_annotations()
            .iter()
            .filter(|annotation| annotation.id() == annotation_id)
            .map(|annotation| (annotation.kind(), annotation.data()))
            .collect::<Vec<_>>(),
        vec![("selection", None)],
    );
    assert!(matches!(
        final_tree.resolve(&old_locator),
        Err(RewriteError::WrongTree { .. })
    ));
    assert!(!old_attribute.same_green(&final_attribute));
    assert_all_source_slices_match(
        &GreenElement::Node(final_tree.root_green().clone()),
        final_snapshot.document().text(),
    );

    let revision = session.revision();
    let state = session.history_state();
    session.finish_save(SaveCompletion {
        revision,
        history_state: state,
        result: Ok(()),
    });
    assert!(!session.is_dirty());
    assert!(session
        .source()
        .shares_text_with(session.persisted_bundle(), "class.md"));
    assert!(session
        .source()
        .shares_text_with(session.persisted_bundle(), "notes.md"));
}

fn place_set() -> Op {
    Op::PlacementSet {
        diagram: "dia".into(),
        subject_title: "Order".into(),
        subject_slug: "order".into(),
        reference_title: "Customer".into(),
        reference_slug: "customer".into(),
        directions: vec![Direction::LeftOf],
    }
}

fn place_rm() -> Op {
    Op::PlacementRemove {
        diagram: "dia".into(),
        subject_slug: "order".into(),
        reference_slug: "customer".into(),
    }
}

fn location(scroll_y: f64) -> ViewLocation {
    ViewLocation {
        document: DocumentLocator::concept("dia", waml::view::surface::SurfaceId::canvas()),
        anchor: ViewAnchor::markdown_start(
            DocumentRevision::INITIAL,
            None,
            waml_markdown_editor::input::ScrollState {
                x: 0.0,
                y: scroll_y,
            },
        ),
    }
}

fn request(edit: PendingEdit, label: &str) -> EditRequest {
    EditRequest {
        before_location: location(1.0),
        intent: EditIntent {
            edit,
            label: label.into(),
            merge_key: None,
            after_location: Some(location(2.0)),
        },
    }
}

#[test]
fn replacement_owns_one_revision_scoped_analysis_snapshot() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();

    let change = session.replace(bundle.clone()).unwrap();

    assert_eq!(
        change,
        SessionChange::full(
            1,
            session.current.affected_documents.clone(),
            session.current.affected_diagrams.clone(),
        )
    );
    assert_eq!(session.okf_analysis().catalog.session_revision(), 1);
    assert_eq!(
        session.uml_analysis().syntax.catalog().session_revision(),
        1
    );
    assert!(Arc::ptr_eq(
        &session.okf_analysis().catalog,
        session.uml_analysis().syntax.catalog(),
    ));
    assert_eq!(session.okf_analysis().bundle, *session.okf());
    assert_eq!(session.uml_analysis().projection, *session.uml_projection());
    assert_eq!(session.source(), &bundle);
    assert_eq!(session.persisted_bundle(), &bundle);
    let path = BundlePath::parse("dia.md").unwrap();
    let document_id = session.okf_analysis().catalog.id_for_path(&path).unwrap();
    let catalog_document = session
        .okf_analysis()
        .catalog
        .document(document_id)
        .unwrap();
    assert!(Arc::ptr_eq(
        session
            .okf_analysis()
            .markdown
            .document(document_id)
            .unwrap()
            .text()
            .shared(),
        catalog_document.text().shared()
    ));
    assert!(Arc::ptr_eq(
        session
            .uml_analysis()
            .syntax
            .document(document_id)
            .unwrap()
            .document(),
        catalog_document
    ));
    assert!(!session.is_dirty());
}

#[test]
fn snapshot_exposes_one_coherent_revision_and_provenance_set() {
    let mut session = EditorSession::default();
    session.replace(diagram_bundle("")).unwrap();

    let snapshot = session.snapshot();
    assert_eq!(snapshot.revision, 1);
    assert_eq!(
        snapshot.okf_analysis.catalog.session_revision(),
        snapshot.revision
    );
    assert_eq!(snapshot.uml_analysis.session_revision(), snapshot.revision);
    assert!(Arc::ptr_eq(
        &snapshot.okf_analysis.catalog,
        snapshot.uml_analysis.syntax.catalog(),
    ));
    assert_eq!(snapshot.source.as_ref(), session.source());
    assert_eq!(
        snapshot.persisted_source.as_ref(),
        session.persisted_bundle()
    );
    assert_eq!(snapshot.dirty_revision, None);
}

#[test]
fn replace_fully_invalidates_and_starts_clean() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();

    let change = session.replace(bundle.clone()).unwrap();

    assert_eq!(
        change,
        SessionChange::full(
            1,
            session.current.affected_documents.clone(),
            session.current.affected_diagrams.clone(),
        )
    );
    assert_eq!(session.bundle(), &bundle);
    assert_eq!(session.persisted_bundle(), &bundle);
    assert!(session
        .bundle()
        .shares_text_with(session.persisted_bundle(), "dia.md"));
    assert_eq!(session.revision(), 1);
    assert!(!session.is_dirty());
}

#[test]
fn replacement_keeps_current_and_persisted_text_equal() {
    let bundle = source(vec![("notes.md".into(), "# Notes\n".into())]);
    let mut session = EditorSession::default();

    session.replace(bundle.clone()).unwrap();

    assert_eq!(session.bundle(), &bundle);
    assert_eq!(session.persisted_bundle(), &bundle);
}

#[test]
fn successful_ops_increment_once_and_mark_the_revision_dirty() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();

    let change = session.apply(waml::uml::Batch(vec![place_set()])).unwrap();

    assert_eq!(
        change,
        SessionChange::full(
            2,
            session.current.affected_documents.clone(),
            session.current.affected_diagrams.clone(),
        )
    );
    assert_eq!(session.revision(), 2);
    assert!(session.is_dirty());
    assert!(session.bundle().documents()[0].text().contains("left of"));
}

#[test]
fn successful_one_document_edit_detaches_only_changed_text() {
    let bundle = source(vec![
        (
            "a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n".into(),
        ),
        (
            "b.md".into(),
            "---\ntype: uml.Class\ntitle: B\n---\n# B\n".into(),
        ),
    ]);
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();

    session
        .apply(waml::uml::Batch(vec![Op::ClassifierSet {
            id: "a".into(),
            title: Some("Changed A".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        }]))
        .unwrap();

    assert!(!session
        .bundle()
        .shares_text_with(session.persisted_bundle(), "a.md"));
    assert!(session
        .bundle()
        .shares_text_with(session.persisted_bundle(), "b.md"));
}

#[test]
fn every_preparation_failure_preserves_the_complete_committed_snapshot() {
    for stage in [
        AnalysisStage::Shell,
        AnalysisStage::Okf,
        AnalysisStage::Specialization("uml"),
        AnalysisStage::Claims,
    ] {
        let bundle = source(vec![(
            "a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n".into(),
        )]);
        let mut session = EditorSession::default();
        session.replace(bundle).unwrap();
        let before_source = session.source().clone();
        let before_persisted = session.persisted_bundle().clone();
        let before_catalog = session.okf_analysis().catalog.clone();
        let before_shell_catalog = session.okf_analysis().catalog.clone();
        let before_uml_catalog = session.uml_analysis().syntax.catalog().clone();
        let before_projection = session.uml_projection().clone();
        let before_revision = session.revision();
        let before_dirty_revision = session.snapshot().dirty_revision;
        let document_id = session
            .okf_analysis()
            .catalog
            .id_for_path(&BundlePath::parse("a.md").unwrap())
            .unwrap();
        let before_document = session
            .okf_analysis()
            .catalog
            .document(document_id)
            .unwrap()
            .clone();
        let before_shell_tree = session
            .okf_analysis()
            .markdown
            .document(document_id)
            .unwrap()
            .tree()
            .clone();
        let before_uml_tree = session
            .uml_analysis()
            .syntax
            .document(document_id)
            .unwrap()
            .syntax()
            .clone();

        let result = session.apply_with_preparer(
            waml::uml::Batch(vec![Op::ClassifierSet {
                id: "a".into(),
                title: Some("Changed".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }]),
            |candidate, previous, revision| {
                let _prepared = prepare_candidate(candidate, previous, revision)?;
                Err(AnalysisError::StructuralInvariant {
                    stage,
                    reason: "injected after complete preparation".into(),
                })
            },
        );

        assert!(result.is_err());
        assert_eq!(session.source(), &before_source);
        assert_eq!(session.persisted_bundle(), &before_persisted);
        assert!(session.source().shares_text_with(&before_source, "a.md"));
        assert!(Arc::ptr_eq(
            &session.okf_analysis().catalog,
            &before_catalog
        ));
        assert!(Arc::ptr_eq(
            &session.okf_analysis().catalog,
            &before_shell_catalog
        ));
        assert!(Arc::ptr_eq(
            session.uml_analysis().syntax.catalog(),
            &before_uml_catalog
        ));
        assert!(Arc::ptr_eq(
            session
                .okf_analysis()
                .catalog
                .document(document_id)
                .unwrap(),
            &before_document
        ));
        assert!(Arc::ptr_eq(
            session
                .okf_analysis()
                .markdown
                .document(document_id)
                .unwrap()
                .tree(),
            &before_shell_tree
        ));
        assert!(Arc::ptr_eq(
            session
                .uml_analysis()
                .syntax
                .document(document_id)
                .unwrap()
                .syntax(),
            &before_uml_tree
        ));
        assert_eq!(session.uml_projection(), &before_projection);
        assert_eq!(session.revision(), before_revision);
        assert_eq!(session.snapshot().dirty_revision, before_dirty_revision);

        let retry = session
            .apply(waml::uml::Batch(vec![Op::ClassifierSet {
                id: "a".into(),
                title: Some("Changed".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }]))
            .unwrap();
        assert_eq!(retry.revision, before_revision + 1);
        assert_eq!(
            session
                .okf_analysis()
                .catalog
                .id_for_path(&BundlePath::parse("a.md").unwrap()),
            Some(document_id)
        );
    }
}

#[test]
fn successful_transaction_prepares_each_real_phase_exactly_once() {
    let bundle = source(vec![(
        "a.md".into(),
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n".into(),
    )]);
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let mut probe = waml::analysis::test_support::PreparationProbe::succeed();

    let change = session
        .apply_with_preparer(
            waml::uml::Batch(vec![Op::ClassifierSet {
                id: "a".into(),
                title: Some("Changed".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }]),
            |source, previous, revision| {
                waml::analysis::test_support::prepare_candidate_with_probe(
                    source, previous, revision, &mut probe,
                )
            },
        )
        .unwrap();

    assert_eq!(change.revision, 2);
    assert_eq!(probe.phase_names(), ["shell", "okf", "uml", "claims"]);
}

#[test]
fn failed_phase_stops_preparation_and_retry_has_no_hidden_duplicates() {
    for (fail_at, failed_calls) in [
        (AnalysisStage::Shell, &["shell"][..]),
        (AnalysisStage::Okf, &["shell", "okf"][..]),
        (
            AnalysisStage::Specialization("uml"),
            &["shell", "okf", "uml"][..],
        ),
        (
            AnalysisStage::Claims,
            &["shell", "okf", "uml", "claims"][..],
        ),
    ] {
        let bundle = source(vec![(
            "a.md".into(),
            "---\ntype: uml.Class\ntitle: A\n---\n# A\n".into(),
        )]);
        let mut session = EditorSession::default();
        session.replace(bundle).unwrap();
        let before_revision = session.revision();
        let before_catalog = session.okf_analysis().catalog.clone();
        let mut failed_probe = waml::analysis::test_support::PreparationProbe::fail_at(fail_at);

        let failed = session.apply_with_preparer(
            waml::uml::Batch(vec![Op::ClassifierSet {
                id: "a".into(),
                title: Some("Changed".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }]),
            |source, previous, revision| {
                waml::analysis::test_support::prepare_candidate_with_probe(
                    source,
                    previous,
                    revision,
                    &mut failed_probe,
                )
            },
        );

        assert!(failed.is_err());
        assert_eq!(failed_probe.phase_names(), failed_calls);
        assert_eq!(session.revision(), before_revision);
        assert!(Arc::ptr_eq(
            &session.okf_analysis().catalog,
            &before_catalog
        ));

        let mut retry_probe = waml::analysis::test_support::PreparationProbe::succeed();
        let retry = session
            .apply_with_preparer(
                waml::uml::Batch(vec![Op::ClassifierSet {
                    id: "a".into(),
                    title: Some("Changed".into()),
                    description: None,
                    stereotype: None,
                    abstract_: None,
                    ty: None,
                }]),
                |source, previous, revision| {
                    waml::analysis::test_support::prepare_candidate_with_probe(
                        source,
                        previous,
                        revision,
                        &mut retry_probe,
                    )
                },
            )
            .unwrap();

        assert_eq!(retry.revision, before_revision + 1);
        assert_eq!(retry_probe.phase_names(), ["shell", "okf", "uml", "claims"]);
    }
}

#[test]
fn syntax_action_commits_once_and_the_same_action_is_stale_afterward() {
    let bundle = source(vec![(
        "class.md".into(),
        include_str!("../../tests/fixtures/parser-actions/class.md").into(),
    )]);
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let path = BundlePath::parse("class.md").unwrap();
    let action = CodeAction {
        title: "Validated no-op".into(),
        basis: ActionBasis::Bundle {
            session_revision: session.revision(),
        },
        changes: Arc::from([]),
    };
    let batch = SyntaxChangeBatch::new(action).unwrap();

    let change = session.apply(batch.clone()).unwrap();

    assert_eq!(change.revision, 2);
    assert!(session
        .source()
        .document(&path)
        .unwrap()
        .text()
        .contains("Order"));
    assert!(session
        .source()
        .shares_text_with(session.persisted_bundle(), "class.md"));
    let committed_source = session.source().clone();
    let committed_catalog = session.okf_analysis().catalog.clone();
    let committed_revision = session.revision();
    let committed_dirty = session.snapshot().dirty_revision;

    let stale = session.apply(batch);

    assert!(stale.is_err());
    assert_eq!(session.source(), &committed_source);
    assert!(session
        .source()
        .shares_text_with(&committed_source, "class.md"));
    assert!(Arc::ptr_eq(
        &session.okf_analysis().catalog,
        &committed_catalog
    ));
    assert_eq!(session.revision(), committed_revision);
    assert_eq!(session.snapshot().dirty_revision, committed_dirty);
}

#[test]
fn multiplicity_repair_commits_and_reanalysis_removes_the_diagnostic() {
    let mut session = EditorSession::default();
    session
        .replace(source(vec![(
            "class.md".into(),
            "---\r\ntype: uml.Class\r\n---\r\n# Café 😀\r\n\r\n## Attributes\r\n- quantité: Number [oops 42]\r\n".into(),
        )]))
        .unwrap();
    let path = BundlePath::parse("class.md").unwrap();
    let document = session.okf_analysis().catalog.id_for_path(&path).unwrap();
    let action = waml::uml::repair_actions(
        waml::uml::ActionContext::new(
            session.okf_analysis(),
            session.uml_analysis(),
            session.revision(),
        )
        .unwrap(),
        document,
    )
    .unwrap()
    .into_iter()
    .find(|action| action.title == "Replace invalid multiplicity")
    .unwrap();

    let change = session
        .apply(SyntaxChangeBatch::new(action).unwrap())
        .unwrap();

    assert_eq!(change.revision, 2);
    assert!(session
        .source()
        .document(&path)
        .unwrap()
        .text()
        .contains("quantité: Number {42}\r\n"));
    assert!(session.uml_analysis().diagnostics.iter().all(|diagnostic| {
        diagnostic.message != "invalid multiplicity"
            && diagnostic.message != "unterminated multiplicity"
    }));
    let attribute = &session
        .uml_analysis()
        .projection
        .node("class")
        .unwrap()
        .attributes[0];
    assert_eq!(attribute.multiplicity.as_ref().unwrap().as_str(), "42");
    assert!(waml::uml::repair_actions(
        waml::uml::ActionContext::new(
            session.okf_analysis(),
            session.uml_analysis(),
            session.revision(),
        )
        .unwrap(),
        document,
    )
    .unwrap()
    .into_iter()
    .all(|action| action.title != "Replace invalid multiplicity"));
}

#[test]
fn recoverable_malformed_source_commits_with_diagnostics() {
    let bundle = source(vec![(
        "recoverable.md".into(),
        include_str!("../../tests/fixtures/parser-actions/recoverable.md").into(),
    )]);
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let before_revision = session.revision();

    let change = session
        .apply(waml::uml::Batch(vec![Op::ClassifierSet {
            id: "recoverable".into(),
            title: Some("Still Recoverable".into()),
            description: None,
            stereotype: None,
            abstract_: None,
            ty: None,
        }]))
        .unwrap();

    assert_eq!(change.revision, before_revision + 1);
    assert!(session.is_dirty());
    assert!(!session.uml_analysis().diagnostics.is_empty());
    assert!(session
        .source()
        .document_by_concept_id("recoverable")
        .unwrap()
        .text()
        .contains("Still Recoverable"));
}

#[test]
fn failed_ops_leave_bundle_model_revision_and_dirty_state_unchanged() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let before_bundle = session.bundle().clone();
    let before_model = session.model().clone();
    let before_revision = session.revision();

    let result = session.apply(waml::uml::Batch(vec![Op::AttributeRemove {
        node: "missing".into(),
        name: "also-missing".into(),
    }]));

    assert!(result.is_err());
    assert_eq!(session.bundle(), &before_bundle);
    assert!(session.bundle().shares_text_with(&before_bundle, "dia.md"));
    assert_eq!(session.model(), &before_model);
    assert_eq!(session.revision(), before_revision);
    assert!(!session.is_dirty());
}

#[test]
fn ordered_mixed_batch_commits_once_with_one_final_projection() {
    let bundle = source(vec![
        (
            "sales/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into(),
        ),
        (
            "sales/customer.md".into(),
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into(),
        ),
        (
            "sales/orders-diagram.md".into(),
            "---\ntype: Diagram\ntitle: Orders\nprofile: uml-domain\n---\n# Orders\n\n## Layout\n"
                .into(),
        ),
    ]);
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let revision = session.revision();

    let change = session
        .apply(waml::edit::Batch::new(vec![
            waml::edit::Step::Okf(waml::okf::Op::IndexRetitle {
                directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                title: "Sales".into(),
            }),
            waml::edit::Step::Uml(Op::ClassifierRename {
                from: "sales/order".into(),
                to: "purchase-order".into(),
            }),
            waml::edit::Step::Uml(Op::PlacementSet {
                diagram: "sales/orders-diagram".into(),
                subject_title: "Purchase Order".into(),
                subject_slug: "sales/purchase-order".into(),
                reference_title: "Customer".into(),
                reference_slug: "sales/customer".into(),
                directions: vec![Direction::RightOf],
            }),
        ]))
        .unwrap();

    assert_eq!(change.revision, revision + 1);
    assert!(session.okf().index("/sales").unwrap().authored);
    assert!(session
        .uml_projection()
        .contains_concept("sales/purchase-order"));
    assert!(session
        .bundle()
        .documents()
        .iter()
        .find(|document| document.path().as_str() == "sales/orders-diagram.md")
        .unwrap()
        .text()
        .contains("right of"));
}

#[test]
fn late_batch_failure_rolls_back_every_session_field() {
    let bundle = source(vec![
        (
            "sales/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into(),
        ),
        (
            "sales/customer.md".into(),
            "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n".into(),
        ),
    ]);
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let revision = session.revision();
    let source = session.bundle().clone();
    let persisted = session.persisted_bundle().clone();
    let model = session.model().clone();

    let result = session.apply(waml::edit::Batch::new(vec![
        waml::edit::Step::Okf(waml::okf::Op::IndexRetitle {
            directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
            title: "Sales Domain".into(),
        }),
        waml::edit::Step::Uml(Op::ClassifierRename {
            from: "sales/order".into(),
            to: "customer".into(),
        }),
    ]));

    assert!(result.is_err());
    assert_eq!(session.revision(), revision);
    assert_eq!(session.bundle(), &source);
    assert_eq!(session.persisted_bundle(), &persisted);
    assert!(session.bundle().shares_text_with(&source, "sales/order.md"));
    assert!(session
        .bundle()
        .shares_text_with(&source, "sales/customer.md"));
    assert_eq!(session.model(), &model);
    assert!(!session.is_dirty());
}

#[test]
fn old_save_completion_cannot_clear_new_literal_source() {
    let mut session = source_session("# Order\n[");
    let before = session.snapshot();
    let document = document_id(&before, "order.md");
    let first = local_insert(
        before.markdown_snapshot(document).unwrap().clone(),
        before.source.documents()[0].text().len(),
        "x",
    );
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(document, first),
            source_location("order"),
        )
        .unwrap();
    let old = session.save_ticket().unwrap();

    let pending = session.snapshot();
    let second = local_insert(
        pending.markdown_snapshot(document).unwrap().clone(),
        pending.source.documents()[0].text().len(),
        "[",
    );
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(document, second),
            source_location("order"),
        )
        .unwrap();
    let current = session.snapshot();

    assert!(!session.finish_save(SaveCompletion {
        revision: old.revision,
        history_state: old.history_state,
        result: Ok(()),
    }));
    assert_eq!(session.snapshot().dirty_revision, Some(current.revision));
    assert_eq!(session.snapshot().source, current.source);
    assert!(session.snapshot().source.documents()[0]
        .text()
        .contains("["));
}

#[test]
fn dirty_external_replacement_reports_conflict_without_overwriting_source() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let document = document_id(&before, "order.md");
    let local = local_insert(
        before.markdown_snapshot(document).unwrap().clone(),
        before.source.documents()[0].text().len(),
        "local",
    );
    session
        .promote_source_edit(
            ProposedSourceEdit::from_local(document, local),
            source_location("order"),
        )
        .unwrap();
    let dirty = session.snapshot();
    let base = dirty.markdown_snapshot(document).unwrap().revision();

    let result = session
        .replace_external(document, base, "# External\n".to_string())
        .unwrap();

    assert!(matches!(
        result,
        ExternalReplacement::Conflict { dirty_revision }
            if dirty_revision == dirty.revision
    ));
    assert_eq!(session.snapshot().source, dirty.source);
}

#[test]
fn stale_external_replacement_is_ignored_without_parsing() {
    let mut session = source_session("# Order\n");
    let current = session.snapshot();
    let document = document_id(&current, "order.md");
    let stale = DocumentRevision::INITIAL;

    let result = session
        .replace_external(document, stale, "# Stale\n".to_string())
        .unwrap();

    assert!(matches!(result, ExternalReplacement::IgnoredStale));
    assert!(Arc::ptr_eq(&session.snapshot(), &current));
}

#[test]
fn clean_external_replacement_parses_once_at_ingress_and_installs_clean() {
    let mut session = source_session("# Order\n");
    let before = session.snapshot();
    let document = document_id(&before, "order.md");
    let base = before.markdown_snapshot(document).unwrap().revision();
    let mut probe = waml::analysis::test_support::PreparationProbe::succeed();

    let result = session
        .replace_external_with_preparer(
            document,
            base,
            "# External\n[".to_string(),
            |source, previous, revision, promoted| {
                waml::analysis::test_support::prepare_candidate_with_promoted_probe(
                    source, previous, revision, promoted, &mut probe,
                )
            },
        )
        .unwrap();

    assert!(matches!(result, ExternalReplacement::Installed(_)));
    assert_eq!(probe.markdown_parse_calls(document), 0);
    assert_eq!(probe.markdown_reparse_calls(document), 0);
    assert_eq!(probe.markdown_promotions(document), 1);
    let installed = session.snapshot();
    assert_eq!(installed.source.documents()[0].text(), "# External\n[");
    assert_eq!(installed.persisted_source, installed.source);
    assert_eq!(installed.dirty_revision, None);
}

#[test]
fn place_set_and_place_rm_use_the_same_transaction() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();

    let set = session.apply(waml::uml::Batch(vec![place_set()])).unwrap();
    assert!(session.bundle().documents()[0].text().contains("left of"));
    let remove = session.apply(waml::uml::Batch(vec![place_rm()])).unwrap();

    assert_eq!(set.revision + 1, remove.revision);
    assert!(!session.bundle().documents()[0].text().contains("left of"));
    assert!(session.is_dirty());
}

#[test]
fn parser_platform_baseline_lowerer_and_session_atomicity_are_exact() {
    let fixtures = [
        (
            "generic.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/generic.md"),
        ),
        (
            "unknown-uml.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/unknown-uml.md"),
        ),
        (
            "index.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/index.md"),
        ),
        (
            "log.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/log.md"),
        ),
        (
            "class.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/class.md"),
        ),
        (
            "enum.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/enum.md"),
        ),
        (
            "object.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/object.md"),
        ),
        (
            "diagram.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/diagram.md"),
        ),
        (
            "activity.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/activity.md"),
        ),
        (
            "state-machine.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/state-machine.md"),
        ),
        (
            "sequence.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/sequence.md"),
        ),
        (
            "broken-frontmatter.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/broken-frontmatter.md"),
        ),
        (
            "malformed.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/malformed.md"),
        ),
        (
            "malformed-crlf-unicode.md",
            include_str!("../../../waml/tests/fixtures/parser-platform/malformed-crlf-unicode.md"),
        ),
    ];
    let source = SourceBundle::try_from_pairs(fixtures).unwrap();
    let mut session = EditorSession::default();
    session.replace(source).unwrap();

    let change = session
        .apply(waml::edit::Batch::new(vec![
            waml::edit::Step::Okf(waml::okf::Op::IndexRetitle {
                directory: waml::okf::DirectoryAddress::parse("/").unwrap(),
                title: "Parser Platform Baseline".into(),
            }),
            waml::edit::Step::Uml(Op::ClassifierSet {
                id: "class".into(),
                title: Some("Café Order Baseline".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }),
        ]))
        .unwrap();
    assert_eq!(change.revision, 2, "class.md successful lowerer revision");
    assert!(
        session.is_dirty(),
        "class.md successful lowerer dirty state"
    );
    // UML lowering edits only its typed title slots. The raw Operations
    // island and its authored whitespace are not formatter-owned.
    let expected = include_str!("../../../waml/tests/fixtures/parser-platform/class.md")
        .replace("title: Café Order", "title: Café Order Baseline")
        .replace("# Café Order", "# Café Order Baseline");
    let changed = session
        .bundle()
        .documents()
        .iter()
        .find(|document| document.path().as_str() == "class.md")
        .unwrap()
        .text();
    assert_eq!(changed, expected, "class.md exact UML Lowerer output");
    assert!(changed.contains("This **raw Markdown** is deliberately not semantic."));
    assert!(changed.ends_with("Trailing whitespace stays here.   \n"));
    let changed_index = session
        .bundle()
        .documents()
        .iter()
        .find(|document| document.path().as_str() == "index.md")
        .unwrap()
        .text();
    let mut index_hash = 0xcbf29ce484222325u64;
    for byte in changed_index.bytes() {
        index_hash ^= u64::from(byte);
        index_hash = index_hash.wrapping_mul(0x100000001b3);
    }
    assert_eq!(
        format!("{index_hash:016x}"),
        "660b50f247619dc4",
        "index.md exact OKF Lowerer output"
    );
    assert!(
        session
            .bundle()
            .shares_text_with(session.persisted_bundle(), "enum.md"),
        "enum.md untouched by class.md lowerer"
    );

    let source_before_failure = session.bundle().clone();
    let projection_before_failure = session.uml_projection().clone();
    let revision_before_failure = session.revision();
    let dirty_before_failure = session.is_dirty();
    let result = session.apply(waml::uml::Batch(vec![Op::AttributeRemove {
        node: "class".into(),
        name: "missing".into(),
    }]));
    assert!(result.is_err(), "class.md failed lowerer result");
    assert_eq!(
        session.bundle(),
        &source_before_failure,
        "class.md failed lowerer source atomicity"
    );
    assert_eq!(
        session.uml_projection(),
        &projection_before_failure,
        "class.md failed lowerer projection atomicity"
    );
    assert_eq!(
        session.revision(),
        revision_before_failure,
        "class.md failed lowerer revision atomicity"
    );
    assert_eq!(
        session.is_dirty(),
        dirty_before_failure,
        "class.md failed lowerer dirty atomicity"
    );
}

#[test]
fn successful_edit_undo_and_redo_publish_reciprocals_and_locations() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle.clone()).unwrap();

    session
        .apply_edit(request(
            PendingEdit::new(waml::uml::Batch(vec![place_set()])),
            "Place Order",
        ))
        .unwrap();
    assert!(session.can_undo());
    assert!(session.bundle().documents()[0].text().contains("left of"));

    let undone = session.undo().unwrap().unwrap();
    assert_eq!(undone.label, "Place Order");
    assert_eq!(undone.location, location(1.0));
    assert_eq!(session.bundle(), &bundle);
    assert!(session.can_redo());
    assert!(
        !session.is_dirty(),
        "undo returned to the initial savepoint"
    );

    let redone = session.redo().unwrap().unwrap();
    assert_eq!(redone.label, "Place Order");
    assert_eq!(redone.location, location(2.0));
    assert!(session.bundle().documents()[0].text().contains("left of"));
    assert!(session.can_undo());
}

#[test]
fn newest_typed_character_undo_keeps_the_rest_of_customerr() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let typed = |title: &str, label: &str, span: std::ops::Range<usize>| EditRequest {
        before_location: location(1.0),
        intent: EditIntent {
            edit: PendingEdit::new(waml::uml::Batch(vec![Op::DiagramSet {
                key: "dia".into(),
                title: Some(title.into()),
                description: None,
                clear_description: false,
                display: None,
            }])),
            label: label.into(),
            merge_key: Some(crate::editor_history::EditMergeKey {
                document: DocumentLocator::concept("dia", waml::view::surface::SurfaceId::canvas()),
                control: "title".into(),
                kind: crate::editor_history::EditMergeKind::Insert,
                span: Some(span),
            }),
            after_location: Some(location(2.0)),
        },
    };

    session
        .apply_edit(typed("Customer", "Type Customer", 0..8))
        .unwrap();
    session
        .apply_edit(typed("Customerr", "Type r", 8..9))
        .unwrap();
    assert!(session.bundle().documents()[0].text().contains("Customerr"));

    let undone = session.undo().unwrap().unwrap();

    assert_eq!(undone.label, "Type r");
    let text = session.bundle().documents()[0].text();
    assert!(text.contains("Customer"));
    assert!(!text.contains("Customerr"));
}

#[test]
fn failed_edit_does_not_allocate_history_or_change_session_state() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle.clone()).unwrap();
    let state = session.history_state();
    let revision = session.revision();

    let result = session.apply_edit(request(
        PendingEdit::new(waml::uml::Batch(vec![Op::AttributeRemove {
            node: "missing".into(),
            name: "missing".into(),
        }])),
        "Broken",
    ));

    assert!(result.is_err());
    assert_eq!(session.history_state(), state);
    assert_eq!(session.revision(), revision);
    assert_eq!(session.bundle(), &bundle);
    assert!(!session.can_undo());
}

#[test]
fn failed_undo_and_redo_keep_their_source_stack_intact() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    session
        .apply_edit(request(
            PendingEdit::new(waml::uml::Batch(vec![Op::DiagramSet {
                key: "dia".into(),
                title: Some("Changed".into()),
                description: None,
                clear_description: false,
                display: None,
            }])),
            "Retitle diagram",
        ))
        .unwrap();
    let applied_source = session.source().clone();

    set_current_source(&mut session, diagram_bundle("tampered"));
    let failed_undo_source = session.source().clone();
    let failed_undo_revision = session.revision();
    assert!(session.undo().is_err());
    assert!(session.can_undo());
    assert_eq!(session.source(), &failed_undo_source);
    assert_eq!(session.revision(), failed_undo_revision);

    let prepared = prepare_candidate(applied_source, None, session.revision()).unwrap();
    install_prepared_for_test(&mut session, prepared);
    session.undo().unwrap().unwrap();
    assert!(session.can_redo());

    let tampered = SourceBundle::try_from_pairs([(
        "dia.md",
        session.source().documents()[0]
            .text()
            .replace("title: D", "title: Other"),
    )])
    .unwrap();
    set_current_source(&mut session, tampered);
    let failed_redo_source = session.source().clone();
    let failed_redo_revision = session.revision();
    assert!(session.redo().is_err());
    assert!(session.can_redo());
    assert_eq!(session.source(), &failed_redo_source);
    assert_eq!(session.revision(), failed_redo_revision);
}

#[test]
fn save_requires_matching_revision_and_state_and_undo_tracks_savepoint_identity() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    let clean_state = session.history_state();
    session
        .apply_edit(request(
            PendingEdit::new(waml::uml::Batch(vec![place_set()])),
            "Place Order",
        ))
        .unwrap();
    let edited_revision = session.revision();
    let edited_state = session.history_state();

    assert!(!session.finish_save(SaveCompletion {
        revision: edited_revision,
        history_state: clean_state,
        result: Ok(()),
    }));
    assert!(session.is_dirty());
    assert!(session.finish_save(SaveCompletion {
        revision: edited_revision,
        history_state: edited_state,
        result: Ok(()),
    }));
    assert!(!session.is_dirty());

    session.undo().unwrap().unwrap();
    assert!(session.is_dirty(), "undo after save leaves the saved state");
    session.redo().unwrap().unwrap();
    assert!(!session.is_dirty(), "redo returns to the saved state");
}

#[test]
fn replace_resets_both_history_stacks() {
    let bundle = diagram_bundle("");
    let mut session = EditorSession::default();
    session.replace(bundle).unwrap();
    session
        .apply_edit(request(
            PendingEdit::new(waml::uml::Batch(vec![place_set()])),
            "Place Order",
        ))
        .unwrap();
    session.undo().unwrap().unwrap();
    assert!(session.can_redo());

    let replacement = diagram_bundle("");
    session.replace(replacement).unwrap();

    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert!(!session.is_dirty());
}
