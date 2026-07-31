use std::sync::Arc;

use makepad_widgets::*;

use crate::{
    edit::ProposedMarkdownEdit,
    input::{EditorInput, MarkdownEditorController},
    layout::{LayoutDocument, LayoutElementId, LayoutEngine, LayoutSnapshot},
    selection::TextPosition,
    session::MarkdownDocumentSession,
};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    mod.widgets.MarkdownEditorBase = #(MarkdownEditor::register_widget(vm))

    mod.widgets.MarkdownEditor = set_type_default() do mod.widgets.MarkdownEditorBase {
        width: Fill
        height: Fill
    }
}

pub fn live_design(cx: &mut Cx) {
    cx.with_vm(script_mod);
}

pub struct MarkdownEditorScope<'a> {
    pub session: &'a mut MarkdownDocumentSession,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmbeddedBlockEvent {
    Activated,
    RetryRequested,
    MeasurementChanged { size: DVec2 },
}

#[derive(Clone, Debug, Default)]
pub enum MarkdownEditorAction {
    ProposedEdit(ProposedMarkdownEdit),
    SelectionChanged,
    NavigationRequested {
        position: TextPosition,
    },
    EmbeddedBlockEvent {
        id: LayoutElementId,
        event: EmbeddedBlockEvent,
    },
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawLayer {
    BlockBackground,
    Selection,
    Text,
    Decoration,
    EmbeddedBlock,
    CaretAndIme,
}

#[derive(Default)]
pub struct DrawRecorder {
    layers: Vec<DrawLayer>,
    snapshot_ptrs: Vec<*const LayoutSnapshot>,
}

impl DrawRecorder {
    pub fn layers(&self) -> &[DrawLayer] {
        &self.layers
    }

    pub fn snapshot_ptrs(&self) -> &[*const LayoutSnapshot] {
        &self.snapshot_ptrs
    }

    fn record(&mut self, layer: DrawLayer, layout: &Arc<LayoutSnapshot>) {
        self.layers.push(layer);
        self.snapshot_ptrs.push(Arc::as_ptr(layout));
    }
}

pub fn draw_visible_layers_for_test(layout: &Arc<LayoutSnapshot>, recorder: &mut DrawRecorder) {
    for layer in [
        DrawLayer::BlockBackground,
        DrawLayer::Selection,
        DrawLayer::Text,
        DrawLayer::Decoration,
        DrawLayer::EmbeddedBlock,
        DrawLayer::CaretAndIme,
    ] {
        recorder.record(layer, layout);
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct MarkdownEditor {
    #[deref]
    view: View,
    #[rust]
    controller: MarkdownEditorController,
    #[rust]
    layout_engine: LayoutEngine,
    #[rust]
    layout: Option<Arc<LayoutSnapshot>>,
    #[rust]
    layout_document: Option<LayoutDocument>,
    #[rust]
    pointer_active: bool,
    #[rust]
    read_only: bool,
    #[rust]
    reduced_motion: bool,
    #[rust]
    last_ime_point: DVec2,
}

impl Widget for MarkdownEditor {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let _ = (&mut self.layout_engine, self.pointer_active);
        if let Some(layout) = &self.layout {
            let mut recorder = DrawRecorder::default();
            draw_visible_layers_for_test(layout, &mut recorder);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MarkdownEditor {
    fn actions_for_input(
        &mut self,
        session: &mut MarkdownDocumentSession,
        input: EditorInput,
    ) -> Vec<Action> {
        if self.read_only
            && matches!(
                input,
                EditorInput::Text(_) | EditorInput::Paste(_) | EditorInput::Cut
            )
        {
            return Vec::new();
        }
        let Some(layout) = self.layout.as_ref() else {
            return Vec::new();
        };
        let Ok(response) = self.controller.handle(session, layout, input) else {
            return Vec::new();
        };
        response
            .proposals
            .into_iter()
            .map(|proposal| {
                Box::new(WidgetAction {
                    data: None,
                    action: Box::new(MarkdownEditorAction::ProposedEdit(proposal)),
                    widget_uid: self.widget_uid(),
                    group: None,
                }) as Action
            })
            .collect()
    }

    fn show_ime(&mut self, cx: &mut Cx, session: &MarkdownDocumentSession) {
        let Some(point) = self.layout.as_ref().and_then(|layout| {
            layout
                .source_to_point(session.selections().primary().cursor)
                .map(|caret| caret.rect.pos)
        }) else {
            return;
        };
        self.last_ime_point = point;
        cx.show_text_ime(self.view.area(), point);
    }
}

impl MarkdownEditorRef {
    pub fn set_key_focus(&self, cx: &mut Cx) {
        if let Some(inner) = self.borrow() {
            cx.set_key_focus(inner.view.area());
        }
    }

    pub fn redraw(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.redraw(cx);
        }
    }

    pub fn set_read_only(&self, cx: &mut Cx, read_only: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.read_only = read_only;
            if read_only {
                cx.hide_text_ime();
            }
        }
    }

    pub fn set_reduced_motion(&self, reduced_motion: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.reduced_motion = reduced_motion;
        }
    }

    pub fn set_layout_document(&self, cx: &mut Cx, document: LayoutDocument) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.layout_document = Some(document);
            inner.view.redraw(cx);
        }
    }

    pub fn target_layout(&self) -> Option<Arc<LayoutSnapshot>> {
        self.borrow().and_then(|inner| inner.layout.clone())
    }

    pub fn proposed_edit(actions: &Actions) -> Option<ProposedMarkdownEdit> {
        actions.iter().find_map(|action| {
            let widget_action = action.downcast_ref::<WidgetAction>()?;
            match widget_action
                .action
                .downcast_ref::<MarkdownEditorAction>()?
            {
                MarkdownEditorAction::ProposedEdit(proposal) => Some(proposal.clone()),
                _ => None,
            }
        })
    }

    pub fn selection_changed(actions: &Actions) -> bool {
        has_action(actions, |action| {
            matches!(action, MarkdownEditorAction::SelectionChanged)
        })
    }

    pub fn navigation_requested(actions: &Actions) -> Option<TextPosition> {
        find_action(actions, |action| match action {
            MarkdownEditorAction::NavigationRequested { position } => Some(*position),
            _ => None,
        })
    }

    pub fn embedded_block_event(
        actions: &Actions,
    ) -> Option<(LayoutElementId, EmbeddedBlockEvent)> {
        find_action(actions, |action| match action {
            MarkdownEditorAction::EmbeddedBlockEvent { id, event } => Some((*id, *event)),
            _ => None,
        })
    }

    #[doc(hidden)]
    pub fn test_handle_input(
        &self,
        _cx: &mut Cx,
        session: &mut MarkdownDocumentSession,
        input: EditorInput,
    ) -> Vec<Action> {
        self.borrow_mut().map_or_else(Vec::new, |mut inner| {
            inner.actions_for_input(session, input)
        })
    }

    #[doc(hidden)]
    pub fn test_set_layout(&self, layout: Arc<LayoutSnapshot>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.layout = Some(layout);
        }
    }

    #[doc(hidden)]
    pub fn test_show_ime(&self, cx: &mut Cx, session: &mut MarkdownDocumentSession) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show_ime(cx, session);
        }
    }

    #[doc(hidden)]
    pub fn test_last_ime_point(&self) -> DVec2 {
        self.borrow()
            .map_or(DVec2::default(), |inner| inner.last_ime_point)
    }
}

fn find_action<T>(
    actions: &Actions,
    project: impl Fn(&MarkdownEditorAction) -> Option<T>,
) -> Option<T> {
    actions.iter().find_map(|action| {
        let widget_action = action.downcast_ref::<WidgetAction>()?;
        project(
            widget_action
                .action
                .downcast_ref::<MarkdownEditorAction>()?,
        )
    })
}

fn has_action(actions: &Actions, predicate: impl Fn(&MarkdownEditorAction) -> bool) -> bool {
    find_action(actions, |action| predicate(action).then_some(())).is_some()
}
