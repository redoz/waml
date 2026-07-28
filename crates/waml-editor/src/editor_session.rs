use waml::edit::{EditBatch, EditContext, EditError};
use waml::source::SourceBundle;
use waml::uml::Projection;

#[derive(Default)]
pub struct EditorSession {
    source: SourceBundle,
    persisted_source: SourceBundle,
    okf: waml::okf::Bundle,
    uml_projection: Projection,
    revision: u64,
    dirty_revision: Option<u64>,
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
    pub fn replace(&mut self, bundle: SourceBundle, uml_projection: Projection) -> SessionChange {
        self.okf = waml::okf::Bundle::parse(&bundle)
            .expect("validated SourceBundle must produce an OKF bundle");
        self.persisted_source = bundle.clone();
        self.source = bundle;
        self.uml_projection = uml_projection;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = None;
        SessionChange::full(self.revision)
    }

    pub fn apply<B: EditBatch>(&mut self, batch: B) -> Result<SessionChange, EditError> {
        let candidate_source = batch.lower(EditContext {
            source: &self.source,
            okf: &self.okf,
            uml: &self.uml_projection,
        })?;
        let candidate_okf =
            waml::okf::Bundle::parse(&candidate_source).map_err(|error| EditError {
                index: 0,
                op: "okf.parse".into(),
                selector: None,
                reason: error.to_string(),
            })?;
        let candidate_uml = waml::uml::project(&candidate_okf);

        self.source = candidate_source;
        self.okf = candidate_okf;
        self.uml_projection = candidate_uml;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = Some(self.revision);
        Ok(SessionChange::full(self.revision))
    }

    pub fn source(&self) -> &SourceBundle {
        &self.source
    }

    pub fn bundle(&self) -> &SourceBundle {
        self.source()
    }

    pub fn persisted_bundle(&self) -> &SourceBundle {
        &self.persisted_source
    }

    #[allow(dead_code)] // consumed by Task 7's OKF-backed navigator
    pub fn okf(&self) -> &waml::okf::Bundle {
        &self.okf
    }

    pub fn uml_projection(&self) -> &Projection {
        &self.uml_projection
    }

    pub fn model(&self) -> &Projection {
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
    fn replace_fully_invalidates_and_starts_clean() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();

        let change = session.replace(bundle.clone(), model);

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
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();

        session.replace(bundle.clone(), model);

        assert_eq!(session.bundle(), &bundle);
        assert_eq!(session.persisted_bundle(), &bundle);
    }

    #[test]
    fn successful_ops_increment_once_and_mark_the_revision_dirty() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

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
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

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
    fn failed_ops_leave_bundle_model_revision_and_dirty_state_unchanged() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
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
        let projection = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, projection);
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
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
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
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
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
        let model = waml::parse::build_model_from_source(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

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
        let projection = waml::uml::project(&waml::okf::Bundle::parse(&source).unwrap());
        let mut session = EditorSession::default();
        session.replace(source, projection);

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
