//! `ClassifierPreviewView` — the single-element preview (focus canvas + inspector-
//! without-picker, no tool dock). Stub in Task 2; filled in Task 4.

// Dead until Task 4 wires this view into the shell's registry. Same
// convention as `doc_view.rs`.
#![allow(dead_code)]

use makepad_widgets::*;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};

pub struct ClassifierPreviewView {
    /// The previewed classifier/package key.
    key: String,
}

impl ClassifierPreviewView {
    pub fn new(key: String) -> ClassifierPreviewView {
        ClassifierPreviewView { key }
    }
}

impl DocView for ClassifierPreviewView {
    fn sync(&mut self, _cx: &mut Cx, _body: &BodyWidgets, _model: &Model) {}
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
}
