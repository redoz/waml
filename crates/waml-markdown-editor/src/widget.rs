use std::sync::Arc;

use makepad_widgets::*;

use crate::{
    edit::ProposedMarkdownEdit,
    input::{
        ControllerError, EditorInput, EditorKey, MarkdownEditorController, PointerGesture,
        SelectionModifier,
    },
    layout::{
        FontKey, FontResolver, LayoutDocument, LayoutElementId, LayoutEngine, LayoutError,
        LayoutInvalidation, LayoutSnapshot, LayoutViewport, MakepadTextShaper, TextMetrics,
    },
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
    Error(MarkdownEditorError),
    #[default]
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MarkdownEditorError {
    Layout(LayoutError),
    ControllerLayout(LayoutError),
    ControllerEdit,
    MissingLayoutDocument,
}

impl From<ControllerError> for MarkdownEditorError {
    fn from(error: ControllerError) -> Self {
        match error {
            ControllerError::Layout(error) => Self::ControllerLayout(error),
            ControllerError::Edit(_) => Self::ControllerEdit,
        }
    }
}

#[derive(Default)]
struct WidgetFonts;

impl FontResolver for WidgetFonts {
    fn configure_draw_text(&mut self, _key: FontKey, metrics: TextMetrics, draw: &mut DrawText) {
        draw.text_style.font_size = metrics.font_size;
    }
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
    primitive_counts: Vec<usize>,
}

impl DrawRecorder {
    pub fn layers(&self) -> &[DrawLayer] {
        &self.layers
    }

    pub fn snapshot_ptrs(&self) -> &[*const LayoutSnapshot] {
        &self.snapshot_ptrs
    }

    pub fn primitive_counts(&self) -> &[usize] {
        &self.primitive_counts
    }

    fn record(&mut self, layer: DrawLayer, layout: &Arc<LayoutSnapshot>) {
        self.layers.push(layer);
        self.snapshot_ptrs.push(Arc::as_ptr(layout));
        self.primitive_counts.push(0);
    }

    fn set_last_primitive_count(&mut self, count: usize) {
        if let Some(last) = self.primitive_counts.last_mut() {
            *last = count;
        }
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
    presentation: Option<Arc<LayoutDocument>>,
    #[rust]
    pointer_drag_active: bool,
    #[rust]
    read_only: bool,
    #[rust]
    reduced_motion: bool,
    #[rust]
    last_ime_point: DVec2,
    #[rust]
    scroll_y: f64,
    #[rust]
    has_focus: bool,
    #[live]
    draw_text: DrawText,
    #[live]
    draw_background: DrawColor,
    #[live]
    draw_selection: DrawColor,
    #[live]
    draw_decoration: DrawColor,
    #[live]
    draw_embedded: DrawColor,
    #[live]
    draw_caret: DrawColor,
    #[rust]
    fonts: WidgetFonts,
    #[rust]
    last_draw: DrawRecorder,
}

impl Widget for MarkdownEditor {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let _ = (&mut self.layout_engine, self.pointer_drag_active);
        if let Some(layout) = &self.layout {
            self.last_draw = DrawRecorder::default();
            draw_visible_layers_for_test(layout, &mut self.last_draw);
        }
        self.view.draw_walk(cx, scope, walk)
    }
}

impl MarkdownEditor {
    pub fn handle_event_with_session(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        session: &mut MarkdownDocumentSession,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        if self.has_focus {
            if let Event::TextInput(event) = event {
                let input = if event.was_paste {
                    EditorInput::Paste(Arc::from(event.input.as_str()))
                } else {
                    EditorInput::Text(Arc::from(event.input.as_str()))
                };
                return self.handle_input_with_session(cx, session, input);
            }
        }
        let input = match event.hits(cx, self.view.area()) {
            Hit::TextInput(event) if !self.read_only => Some(if event.was_paste {
                EditorInput::Paste(Arc::from(event.input.as_str()))
            } else {
                EditorInput::Text(Arc::from(event.input.as_str()))
            }),
            Hit::TextCopy(event) => {
                let layout = match self.layout.as_ref() {
                    Some(layout) => layout.clone(),
                    None => self.install_layout(cx, session)?,
                };
                let response = self
                    .controller
                    .handle(session, &layout, EditorInput::Copy)
                    .map_err(MarkdownEditorError::from)?;
                *event.response.borrow_mut() = response.clipboard;
                return Ok(Vec::new());
            }
            Hit::TextCut(event) => {
                let layout = match self.layout.as_ref() {
                    Some(layout) => layout.clone(),
                    None => self.install_layout(cx, session)?,
                };
                let copied = self
                    .controller
                    .handle(session, &layout, EditorInput::Copy)
                    .map_err(MarkdownEditorError::from)?;
                *event.response.borrow_mut() = copied.clipboard;
                return self.handle_input_with_session(cx, session, EditorInput::Cut);
            }
            Hit::KeyDown(event) => key_input(event),
            Hit::FingerDown(event) if event.is_primary_hit() => {
                cx.set_key_focus(self.view.area());
                let point = event.abs - self.view.area().rect(cx).pos + dvec2(0.0, self.scroll_y);
                self.pointer_drag_active = true;
                Some(EditorInput::PointerDown(PointerGesture {
                    point,
                    clicks: event.tap_count as u8,
                    modifier: if event.modifiers.is_primary() {
                        SelectionModifier::Add
                    } else if event.modifiers.shift {
                        SelectionModifier::Extend
                    } else {
                        SelectionModifier::Replace
                    },
                }))
            }
            Hit::FingerMove(event) if self.pointer_drag_active => Some(EditorInput::PointerMove {
                point: event.abs - self.view.area().rect(cx).pos + dvec2(0.0, self.scroll_y),
            }),
            Hit::FingerUp(event) if self.pointer_drag_active => {
                self.pointer_drag_active = false;
                if event.was_tap() {
                    let point =
                        event.abs - self.view.area().rect(cx).pos + dvec2(0.0, self.scroll_y);
                    if let Some((id, _)) = self.embedded_at(point) {
                        return Ok(vec![self.make_action(
                            MarkdownEditorAction::EmbeddedBlockEvent {
                                id,
                                event: EmbeddedBlockEvent::Activated,
                            },
                        )]);
                    }
                }
                Some(EditorInput::PointerUp)
            }
            Hit::KeyFocusLost(_) => {
                self.has_focus = false;
                cx.hide_text_ime();
                None
            }
            _ => None,
        };
        if let Event::Scroll(event) = event {
            if self.view.area().rect(cx).contains(event.abs) && !event.handled_y.get() {
                self.scroll_y = (self.scroll_y + event.scroll.y).max(0.0);
                self.layout = None;
                self.view.redraw(cx);
                event.handled_y.set(true);
            }
        }
        input.map_or(Ok(Vec::new()), |input| {
            self.handle_input_with_session(cx, session, input)
        })
    }

    pub fn draw_walk_with_session(
        &mut self,
        cx: &mut Cx2d,
        session: &mut MarkdownDocumentSession,
        scope: &mut Scope,
        walk: Walk,
    ) -> Result<DrawStep, MarkdownEditorError> {
        let layout = self.install_layout(cx, session)?;
        self.last_draw = DrawRecorder::default();
        let document = self.presentation.as_ref().unwrap().clone();
        for layer in [
            DrawLayer::BlockBackground,
            DrawLayer::Selection,
            DrawLayer::Text,
            DrawLayer::Decoration,
            DrawLayer::EmbeddedBlock,
            DrawLayer::CaretAndIme,
        ] {
            self.last_draw.record(layer, &layout);
            let primitive_count = match layer {
                DrawLayer::BlockBackground => {
                    let mut count = 0;
                    for block in &layout.blocks()[layout.visible_block_range()] {
                        self.draw_background.color = vec4(0.97, 0.97, 0.97, 1.0);
                        self.draw_background.draw_abs(cx, block.rect);
                        count += 1;
                    }
                    count
                }
                DrawLayer::Selection => {
                    let mut count = 0;
                    for selection in session.selections().as_slice() {
                        for rect in layout.selection_rects(*selection).unwrap_or_default() {
                            self.draw_selection.color = vec4(0.35, 0.55, 0.95, 0.28);
                            self.draw_selection.draw_abs(cx, rect);
                            count += 1;
                        }
                    }
                    count
                }
                DrawLayer::Text => {
                    let visible = layout.visible_source_range();
                    let mut count = 0;
                    for run in document.text_runs.iter() {
                        if run.range.end() <= visible.start() || visible.end() <= run.range.start()
                        {
                            continue;
                        }
                        if let (Ok(text), Some(caret)) = (
                            session.snapshot().text().slice(run.range),
                            layout.source_to_point(TextPosition::new(
                                run.range.start(),
                                crate::selection::Affinity::Before,
                            )),
                        ) {
                            self.fonts.configure_draw_text(
                                run.metrics.font,
                                run.metrics,
                                &mut self.draw_text,
                            );
                            self.draw_text.draw_abs(cx, caret.rect.pos, text);
                            count += 1;
                        }
                    }
                    count
                }
                DrawLayer::Decoration => {
                    // The neutral layout contract currently has no decoration geometry.
                    let _ = &mut self.draw_decoration;
                    0
                }
                DrawLayer::EmbeddedBlock => {
                    let mut count = 0;
                    for block in &layout.blocks()[layout.visible_block_range()] {
                        if document
                            .embedded_blocks
                            .iter()
                            .any(|item| item.id == block.id)
                        {
                            self.draw_embedded.color = vec4(0.88, 0.89, 0.91, 1.0);
                            self.draw_embedded.draw_abs(cx, block.rect);
                            count += 1;
                        }
                    }
                    count
                }
                DrawLayer::CaretAndIme => {
                    let mut count = 0;
                    for selection in session.selections().as_slice() {
                        if selection.anchor == selection.cursor {
                            if let Some(caret) = layout.source_to_point(selection.cursor) {
                                self.draw_caret.color = vec4(0.1, 0.1, 0.1, 1.0);
                                self.draw_caret.draw_abs(cx, caret.rect);
                                count += 1;
                            }
                        }
                    }
                    count
                }
            };
            self.last_draw.set_last_primitive_count(primitive_count);
        }
        if cx.has_key_focus(self.view.area()) && !self.read_only {
            self.show_ime(cx, session);
        }
        Ok(self.view.draw_walk(cx, scope, walk))
    }

    fn embedded_at(&self, point: DVec2) -> Option<(LayoutElementId, Rect)> {
        let layout = self.layout.as_ref()?;
        let document = self.presentation.as_ref()?;
        layout.blocks()[layout.visible_block_range()]
            .iter()
            .find(|block| {
                block.rect.contains(point)
                    && document
                        .embedded_blocks
                        .iter()
                        .any(|item| item.id == block.id)
            })
            .map(|block| (block.id, block.rect))
    }

    fn install_layout(
        &mut self,
        cx: &mut Cx,
        session: &MarkdownDocumentSession,
    ) -> Result<Arc<LayoutSnapshot>, MarkdownEditorError> {
        let document = self
            .presentation
            .as_ref()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?;
        let viewport_rect = self.view.area().rect(cx);
        let mut shaper = MakepadTextShaper {
            cx,
            draw_text: &mut self.draw_text,
            fonts: &mut self.fonts,
        };
        let layout = self
            .layout_engine
            .layout(
                document,
                session.snapshot(),
                LayoutViewport::new(
                    viewport_rect.size.x.max(1.0),
                    viewport_rect.size.y.max(1.0),
                    self.scroll_y,
                    0.0,
                ),
                LayoutInvalidation::Document,
                &mut shaper,
            )
            .map_err(MarkdownEditorError::Layout)?;
        let layout = Arc::new(layout);
        self.layout = Some(layout.clone());
        Ok(layout)
    }

    pub fn handle_input_with_session(
        &mut self,
        cx: &mut Cx,
        session: &mut MarkdownDocumentSession,
        input: EditorInput,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        if self.read_only
            && matches!(
                input,
                EditorInput::Text(_) | EditorInput::Paste(_) | EditorInput::Cut
            )
        {
            return Ok(Vec::new());
        }
        let layout = match self.layout.as_ref() {
            Some(layout) => layout.clone(),
            None => self.install_layout(cx, session)?,
        };
        let old_selection = session.selections().clone();
        let response = self
            .controller
            .handle(session, &layout, input)
            .map_err(MarkdownEditorError::from)?;
        let mut actions: Vec<Action> = response
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
            .collect();
        if session.selections() != &old_selection {
            actions.push(self.make_action(MarkdownEditorAction::SelectionChanged));
        }
        if response.request_redraw {
            self.view.redraw(cx);
        }
        if let Some(point) = response.request_ime_at {
            self.last_ime_point = point;
            cx.show_text_ime(self.view.area(), point);
        }
        Ok(actions)
    }

    fn make_action(&self, action: MarkdownEditorAction) -> Action {
        Box::new(WidgetAction {
            data: None,
            action: Box::new(action),
            widget_uid: self.widget_uid(),
            group: None,
        })
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

fn key_input(event: KeyEvent) -> Option<EditorInput> {
    let extend = event.modifiers.shift;
    let key = match event.key_code {
        KeyCode::ReturnKey | KeyCode::NumpadEnter => EditorKey::Enter,
        KeyCode::Tab if extend => EditorKey::BackTab,
        KeyCode::Tab => EditorKey::Tab,
        KeyCode::Delete => EditorKey::Delete,
        KeyCode::Backspace => EditorKey::Backspace,
        KeyCode::ArrowLeft => EditorKey::Left { extend },
        KeyCode::ArrowRight => EditorKey::Right { extend },
        KeyCode::ArrowUp => EditorKey::Up { extend },
        KeyCode::ArrowDown => EditorKey::Down { extend },
        KeyCode::KeyA if event.modifiers.is_primary() => EditorKey::SelectAll,
        KeyCode::KeyZ if event.modifiers.is_primary() && extend => EditorKey::Redo,
        KeyCode::KeyZ if event.modifiers.is_primary() => EditorKey::Undo,
        _ => return None,
    };
    Some(EditorInput::Key(key))
}

impl MarkdownEditorRef {
    pub fn handle_event_with_session(
        &self,
        cx: &mut Cx,
        event: &Event,
        session: &mut MarkdownDocumentSession,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        self.borrow_mut()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .handle_event_with_session(cx, event, session)
    }
    pub fn set_key_focus(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.has_focus = true;
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

    pub fn set_layout_document(&self, cx: &mut Cx, document: Arc<LayoutDocument>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.presentation = Some(document);
            inner.layout = None;
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

    pub fn handle_input_with_session(
        &self,
        cx: &mut Cx,
        session: &mut MarkdownDocumentSession,
        input: EditorInput,
    ) -> Result<Vec<Action>, MarkdownEditorError> {
        self.borrow_mut()
            .ok_or(MarkdownEditorError::MissingLayoutDocument)?
            .handle_input_with_session(cx, session, input)
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
