use waml::analysis::{
    prepare_candidate, AnalysisError, OkfAnalysis, PreparedCandidate, PreviousAnalyses,
};
use waml::edit::{EditBatch, EditContext, EditError};
use waml::source::SourceBundle;

pub struct EditorSession {
    source: SourceBundle,
    persisted_source: SourceBundle,
    okf_analysis: OkfAnalysis,
    uml: waml::uml::Analysis,
    revision: u64,
    dirty_revision: Option<u64>,
}

impl Default for EditorSession {
    fn default() -> Self {
        let prepared = prepare_candidate(SourceBundle::default(), None, 0)
            .expect("the empty source bundle must produce valid analyses");
        let (source, okf_analysis, uml, revision) = prepared.into_parts();
        Self {
            persisted_source: source.clone(),
            source,
            okf_analysis,
            uml,
            revision,
            dirty_revision: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionChange {
    pub revision: u64,
    pub source_changed: bool,
    pub okf_changed: bool,
    pub uml_changed: bool,
    pub navigation_changed: bool,
    pub conflicts_changed: bool,
}

impl SessionChange {
    fn full(revision: u64) -> SessionChange {
        SessionChange {
            revision,
            source_changed: true,
            okf_changed: true,
            uml_changed: true,
            navigation_changed: true,
            conflicts_changed: true,
        }
    }
}

impl EditorSession {
    pub fn replace(&mut self, source: SourceBundle) -> Result<SessionChange, EditError> {
        let next_revision = self.revision.wrapping_add(1);
        let prepared = prepare_candidate(source, None, next_revision)?;
        let (source, okf_analysis, uml, revision) = prepared.into_parts();

        self.persisted_source = source.clone();
        self.source = source;
        self.okf_analysis = okf_analysis;
        self.uml = uml;
        self.revision = revision;
        self.dirty_revision = None;
        Ok(SessionChange::full(self.revision))
    }

    pub fn apply<B: EditBatch>(&mut self, batch: B) -> Result<SessionChange, EditError> {
        self.apply_with_preparer(batch, prepare_candidate)
    }

    fn apply_with_preparer<B, F>(
        &mut self,
        batch: B,
        prepare: F,
    ) -> Result<SessionChange, EditError>
    where
        B: EditBatch,
        F: for<'a> FnOnce(
            SourceBundle,
            Option<PreviousAnalyses<'a>>,
            u64,
        ) -> Result<PreparedCandidate, AnalysisError>,
    {
        let candidate_source = batch.lower(EditContext {
            source: &self.source,
            okf_analysis: &self.okf_analysis,
            session_revision: self.revision,
            uml: &self.uml,
        })?;
        let next_revision = self.revision.wrapping_add(1);
        let prepared = prepare(
            candidate_source,
            Some(PreviousAnalyses {
                okf: &self.okf_analysis,
                uml: &self.uml,
            }),
            next_revision,
        )?;
        let (source, okf_analysis, uml, revision) = prepared.into_parts();

        self.source = source;
        self.okf_analysis = okf_analysis;
        self.uml = uml;
        self.revision = revision;
        self.dirty_revision = Some(self.revision);
        Ok(SessionChange::full(self.revision))
    }

    pub fn source(&self) -> &SourceBundle {
        &self.source
    }

    #[cfg(test)]
    fn bundle(&self) -> &SourceBundle {
        self.source()
    }

    pub fn persisted_bundle(&self) -> &SourceBundle {
        &self.persisted_source
    }

    pub fn okf_analysis(&self) -> &OkfAnalysis {
        &self.okf_analysis
    }

    pub fn okf(&self) -> &waml::okf::Bundle {
        &self.okf_analysis.bundle
    }

    pub fn uml_analysis(&self) -> &waml::uml::Analysis {
        &self.uml
    }

    pub fn uml_projection(&self) -> &waml::uml::Projection {
        &self.uml.projection
    }

    #[cfg(test)]
    fn model(&self) -> &waml::uml::Projection {
        self.uml_projection()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_revision.is_some()
    }

    pub fn mark_saved(&mut self, revision: u64) {
        if self.dirty_revision == Some(revision) {
            self.persisted_source.clone_from(&self.source);
            self.dirty_revision = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use waml::action::{ActionBasis, CodeAction, SyntaxChangeBatch};
    use waml::analysis::AnalysisStage;
    use waml::syntax::Direction;
    use waml::uml::Op;

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

    #[test]
    fn replacement_owns_one_revision_scoped_analysis_snapshot() {
        let bundle = diagram_bundle("");
        let mut session = EditorSession::default();

        let change = session.replace(bundle.clone()).unwrap();

        assert_eq!(change, SessionChange::full(1));
        assert_eq!(session.okf_analysis().catalog.session_revision(), 1);
        assert_eq!(
            session.uml_analysis().syntax.catalog().session_revision(),
            1
        );
        assert!(std::sync::Arc::ptr_eq(
            &session.okf_analysis().catalog,
            session.uml_analysis().syntax.catalog(),
        ));
        assert_eq!(session.okf_analysis().bundle, *session.okf());
        assert_eq!(session.uml_analysis().projection, *session.uml_projection());
        assert_eq!(session.source(), &bundle);
        assert_eq!(session.persisted_bundle(), &bundle);
        let path = waml::source::BundlePath::parse("dia.md").unwrap();
        let document_id = session.okf_analysis().catalog.id_for_path(&path).unwrap();
        let catalog_document = session
            .okf_analysis()
            .catalog
            .document(document_id)
            .unwrap();
        assert!(Arc::ptr_eq(
            session
                .okf_analysis()
                .shell
                .document(document_id)
                .unwrap()
                .document(),
            catalog_document
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
    fn replace_fully_invalidates_and_starts_clean() {
        let bundle = diagram_bundle("");
        let mut session = EditorSession::default();

        let change = session.replace(bundle.clone()).unwrap();

        assert_eq!(change, SessionChange::full(1));
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

        assert_eq!(change, SessionChange::full(2));
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
            waml::analysis::AnalysisStage::Shell,
            waml::analysis::AnalysisStage::Okf,
            waml::analysis::AnalysisStage::Specialization("uml"),
            waml::analysis::AnalysisStage::Claims,
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
            let before_shell_catalog = session.okf_analysis().shell.catalog().clone();
            let before_uml_catalog = session.uml_analysis().syntax.catalog().clone();
            let before_projection = session.uml_projection().clone();
            let before_revision = session.revision();
            let before_dirty_revision = session.dirty_revision;
            let document_id = session
                .okf_analysis()
                .catalog
                .id_for_path(&waml::source::BundlePath::parse("a.md").unwrap())
                .unwrap();
            let before_document = session
                .okf_analysis()
                .catalog
                .document(document_id)
                .unwrap()
                .clone();
            let before_shell_tree = session
                .okf_analysis()
                .shell
                .document(document_id)
                .unwrap()
                .syntax()
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
                session.okf_analysis().shell.catalog(),
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
                    .shell
                    .document(document_id)
                    .unwrap()
                    .syntax(),
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
            assert_eq!(session.dirty_revision, before_dirty_revision);

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
                    .id_for_path(&waml::source::BundlePath::parse("a.md").unwrap()),
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
            include_str!("../tests/fixtures/parser-actions/class.md").into(),
        )]);
        let mut session = EditorSession::default();
        session.replace(bundle).unwrap();
        let path = waml::source::BundlePath::parse("class.md").unwrap();
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
        let committed_dirty = session.dirty_revision;

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
        assert_eq!(session.dirty_revision, committed_dirty);
    }

    #[test]
    fn recoverable_malformed_source_commits_with_diagnostics() {
        let bundle = source(vec![(
            "recoverable.md".into(),
            include_str!("../tests/fixtures/parser-actions/recoverable.md").into(),
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
            .apply(waml::compat::Batch::new(vec![
                waml::compat::Step::Okf(waml::okf::Op::IndexRetitle {
                    directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                    title: "Sales".into(),
                }),
                waml::compat::Step::Uml(Op::ClassifierRename {
                    from: "sales/order".into(),
                    to: "purchase-order".into(),
                }),
                waml::compat::Step::Uml(Op::PlacementSet {
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

        let result = session.apply(waml::compat::Batch::new(vec![
            waml::compat::Step::Okf(waml::okf::Op::IndexRetitle {
                directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
                title: "Sales Domain".into(),
            }),
            waml::compat::Step::Uml(Op::ClassifierRename {
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
    fn saving_an_old_revision_cannot_clear_a_newer_dirty_revision() {
        let bundle = diagram_bundle("");
        let mut session = EditorSession::default();
        session.replace(bundle).unwrap();
        let old = session.revision();
        session.apply(waml::uml::Batch(vec![place_set()])).unwrap();

        session.mark_saved(old);
        assert!(session.is_dirty());

        session.mark_saved(session.revision());
        assert!(!session.is_dirty());
        assert_eq!(session.persisted_bundle(), session.bundle());
        assert!(session
            .persisted_bundle()
            .shares_text_with(session.bundle(), "dia.md"));
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
                include_str!("../../waml/tests/fixtures/parser-platform/generic.md"),
            ),
            (
                "unknown-uml.md",
                include_str!("../../waml/tests/fixtures/parser-platform/unknown-uml.md"),
            ),
            (
                "index.md",
                include_str!("../../waml/tests/fixtures/parser-platform/index.md"),
            ),
            (
                "log.md",
                include_str!("../../waml/tests/fixtures/parser-platform/log.md"),
            ),
            (
                "class.md",
                include_str!("../../waml/tests/fixtures/parser-platform/class.md"),
            ),
            (
                "enum.md",
                include_str!("../../waml/tests/fixtures/parser-platform/enum.md"),
            ),
            (
                "object.md",
                include_str!("../../waml/tests/fixtures/parser-platform/object.md"),
            ),
            (
                "diagram.md",
                include_str!("../../waml/tests/fixtures/parser-platform/diagram.md"),
            ),
            (
                "activity.md",
                include_str!("../../waml/tests/fixtures/parser-platform/activity.md"),
            ),
            (
                "state-machine.md",
                include_str!("../../waml/tests/fixtures/parser-platform/state-machine.md"),
            ),
            (
                "sequence.md",
                include_str!("../../waml/tests/fixtures/parser-platform/sequence.md"),
            ),
            (
                "broken-frontmatter.md",
                include_str!("../../waml/tests/fixtures/parser-platform/broken-frontmatter.md"),
            ),
            (
                "malformed.md",
                include_str!("../../waml/tests/fixtures/parser-platform/malformed.md"),
            ),
            (
                "malformed-crlf-unicode.md",
                include_str!("../../waml/tests/fixtures/parser-platform/malformed-crlf-unicode.md"),
            ),
        ];
        let source = SourceBundle::try_from_pairs(fixtures).unwrap();
        let mut session = EditorSession::default();
        session.replace(source).unwrap();

        let change = session
            .apply(waml::compat::Batch::new(vec![
                waml::compat::Step::Okf(waml::okf::Op::IndexRetitle {
                    directory: waml::okf::DirectoryAddress::parse("/").unwrap(),
                    title: "Parser Platform Baseline".into(),
                }),
                waml::compat::Step::Uml(Op::ClassifierSet {
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
        let expected = waml::serialize::serialize_document(&waml::parse::parse_document(
            include_str!("../../waml/tests/fixtures/parser-platform/class.md"),
        ))
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
}
