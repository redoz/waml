//! `SourceView` -- the View Source tab body. Renders the subject classifier's
//! raw markdown into the shared `source_view` slot (a scrolling `Markdown`
//! surface fed the verbatim bundle file text; the feed itself is pushed from
//! the shell in `App::sync_active_tab`) and hides the diagram chrome: the
//! canvas is occluded by the opaque slot, the tool dock by
//! `wants_tooldock() == false`, the inspector's element picker explicitly.

use makepad_widgets::*;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};
use crate::inspector::Subject;

pub struct SourceView {
    /// The subject key whose source this tab shows.
    key: String,
}

impl SourceView {
    pub fn new(key: String) -> SourceView {
        SourceView { key }
    }
}

impl DocView for SourceView {
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, model: &Model) {
        if let Some(mut inspector) = body
            .inspector(cx)
            .borrow_mut::<crate::inspector_panel::Inspector>()
        {
            inspector.set_subject(cx, model, Subject::Classifier(self.key.clone()));
            // A source view is not a diagram: no element picker.
            inspector.set_picker_visible(cx, false);
        }
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _actions: &Actions,
        _model: &Model,
    ) -> ViewOutcome {
        ViewOutcome::default()
    }

    fn wants_tooldock(&self) -> bool {
        false
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
