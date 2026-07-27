use waml::model::Model;
use waml::ops::{Op, OpError};

#[derive(Default)]
pub struct EditorSession {
    bundle: Vec<(String, String)>,
    model: Model,
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
    pub fn replace(&mut self, bundle: Vec<(String, String)>, model: Model) -> SessionChange {
        self.bundle = bundle;
        self.model = model;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = None;
        SessionChange::full(self.revision)
    }

    pub fn apply_ops(&mut self, ops: &[Op]) -> Result<SessionChange, OpError> {
        let bundle = waml::ops::apply(&self.bundle, ops)?;
        let model = waml::parse::build_model(&bundle);
        self.bundle = bundle;
        self.model = model;
        self.revision = self.revision.wrapping_add(1);
        self.dirty_revision = Some(self.revision);
        Ok(SessionChange::full(self.revision))
    }

    pub fn bundle(&self) -> &[(String, String)] {
        &self.bundle
    }

    pub fn model(&self) -> &Model {
        &self.model
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_revision.is_some()
    }

    pub fn mark_saved(&mut self, revision: u64) {
        if self.dirty_revision == Some(revision) {
            self.dirty_revision = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::syntax::Direction;

    fn diagram_bundle(layout: &str) -> Vec<(String, String)> {
        vec![(
            "dia.md".to_string(),
            format!(
                "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Layout\n{layout}"
            ),
        )]
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
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();

        let change = session.replace(bundle.clone(), model);

        assert_eq!(change, SessionChange::full(1));
        assert_eq!(session.bundle(), bundle.as_slice());
        assert_eq!(session.revision(), 1);
        assert!(!session.is_dirty());
    }

    #[test]
    fn successful_ops_increment_once_and_mark_the_revision_dirty() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

        let change = session.apply_ops(&[place_set()]).unwrap();

        assert_eq!(change, SessionChange::full(2));
        assert_eq!(session.revision(), 2);
        assert!(session.is_dirty());
        assert!(session.bundle()[0].1.contains("left of"));
    }

    #[test]
    fn failed_ops_leave_bundle_model_revision_and_dirty_state_unchanged() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
        let before_bundle = session.bundle().to_vec();
        let before_model = session.model().clone();
        let before_revision = session.revision();

        let result = session.apply_ops(&[Op::AttrRm {
            node: "missing".into(),
            name: "also-missing".into(),
        }]);

        assert!(result.is_err());
        assert_eq!(session.bundle(), before_bundle.as_slice());
        assert_eq!(session.model(), &before_model);
        assert_eq!(session.revision(), before_revision);
        assert!(!session.is_dirty());
    }

    #[test]
    fn saving_an_old_revision_cannot_clear_a_newer_dirty_revision() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);
        let old = session.revision();
        session.apply_ops(&[place_set()]).unwrap();

        session.mark_saved(old);
        assert!(session.is_dirty());

        session.mark_saved(session.revision());
        assert!(!session.is_dirty());
    }

    #[test]
    fn place_set_and_place_rm_use_the_same_transaction() {
        let bundle = diagram_bundle("");
        let model = waml::parse::build_model(&bundle);
        let mut session = EditorSession::default();
        session.replace(bundle, model);

        let set = session.apply_ops(&[place_set()]).unwrap();
        assert!(session.bundle()[0].1.contains("left of"));
        let remove = session.apply_ops(&[place_rm()]).unwrap();

        assert_eq!(set.revision + 1, remove.revision);
        assert!(!session.bundle()[0].1.contains("left of"));
        assert!(session.is_dirty());
    }
}
