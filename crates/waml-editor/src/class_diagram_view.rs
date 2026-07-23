//! `ClassDiagramView` — the full class-diagram surface (canvas + inspector-with-
//! picker + tool dock + selection toolbar). Stub in Task 2; filled in Task 3.

// Dead until Task 3 wires this view into the shell's registry. Same
// convention as `doc_view.rs`.
#![allow(dead_code)]

use makepad_widgets::*;
use std::collections::HashSet;
use waml::model::Model;

use crate::doc_view::{BodyWidgets, DocView, ViewOutcome};

#[derive(Default)]
pub struct ClassDiagramView {
    /// Node keys whose card body is expanded. Per-tab live state; moved off the
    /// shell in Task 3. Cleared when the diagram changes.
    expanded: HashSet<String>,
}

impl ClassDiagramView {
    pub fn new() -> ClassDiagramView {
        ClassDiagramView::default()
    }
}

impl DocView for ClassDiagramView {
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
        true
    }
}
