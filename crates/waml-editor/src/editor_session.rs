use waml::ops::{Op, OpError};
use waml::source::SourceBundle;
use waml::uml::Projection;

#[derive(Default)]
pub struct EditorSession {
    bundle: SourceBundle,
    persisted_bundle: SourceBundle,
    uml_projection: Projection,
    revision: u64,
    dirty_revision: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionChange {
    pub revision: u64,
    pub model_changed: bool,
    pub source_changed: bool,
    pub navigation_changed: bool,
    pub conflicts_changed: bool,
}

impl SessionChange {
    fn full(revision: u64) -> SessionChange {
        SessionChange {
            revision,
            model_changed: true,
            source_changed: true,
            navigation_changed: true,
            conflicts_changed: true,
        }
    }
}

impl EditorSession {
    pub fn replace(&mut self, bundle: SourceBundle, uml_projection: Projection) -> SessionChange {
        self.persisted_bundle = bundle.clone();
        self.bundle = bundle;
        self.uml_projection = uml_projection;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = None;
        SessionChange::full(self.revision)
    }

    pub fn apply_ops(&mut self, ops: &[Op]) -> Result<SessionChange, OpError> {
        let bundle = waml::ops::apply_source(&self.bundle, ops)?;
        let uml_projection = waml::parse::build_model_from_source(&bundle);
        self.bundle = bundle;
        self.uml_projection = uml_projection;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = Some(self.revision);
        Ok(SessionChange::full(self.revision))
    }

    pub fn bundle(&self) -> &SourceBundle {
        &self.bundle
    }

    pub fn persisted_bundle(&self) -> &SourceBundle {
        &self.persisted_bundle
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
            self.persisted_bundle.clone_from(&self.bundle);
            self.dirty_revision = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::syntax::Direction;

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
        Op::PlaceSet {
            diagram: "dia".into(),
            subject_title: "Order".into(),
            subject_slug: "order".into(),
            reference_title: "Customer".into(),
            reference_slug: "customer".into(),
            directions: vec![Direction::LeftOf],
        }
    }

    fn place_rm() -> Op {
        Op::PlaceRm {
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

        let change = session.apply_ops(&[place_set()]).unwrap();

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
            .apply_ops(&[Op::NodeSet {
                slug: "a".into(),
                title: Some("Changed A".into()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }])
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

        let result = session.apply_ops(&[Op::AttrRm {
            node: "missing".into(),
            name: "also-missing".into(),
        }]);

        assert!(result.is_err());
        assert_eq!(session.bundle(), &before_bundle);
        assert!(session.bundle().shares_text_with(&before_bundle, "dia.md"));
        assert_eq!(session.model(), &before_model);
        assert_eq!(session.revision(), before_revision);
        assert!(!session.is_dirty());
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

        let result = session.apply_ops(&[
            Op::PkgRetitle {
                path: "sales".into(),
                title: "Sales Domain".into(),
            },
            Op::NodeRename {
                from: "sales/order".into(),
                to: "customer".into(),
            },
        ]);

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
        session.apply_ops(&[place_set()]).unwrap();

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

        let set = session.apply_ops(&[place_set()]).unwrap();
        assert!(session.bundle().documents()[0].text().contains("left of"));
        let remove = session.apply_ops(&[place_rm()]).unwrap();

        assert_eq!(set.revision + 1, remove.revision);
        assert!(!session.bundle().documents()[0].text().contains("left of"));
        assert!(session.is_dirty());
    }
}
