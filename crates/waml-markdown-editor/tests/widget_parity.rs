use std::sync::Arc;

use makepad_widgets::{dvec2, Cx, Event, Rect, ScriptNew, TextInputEvent, WidgetRef};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    input::{
        ControllerError, EditorInput, EditorResponse, MarkdownEditorController, PointerGesture,
        SelectionModifier,
    },
    layout::{Affinity, CaretStop, GlyphCluster, LayoutError, LayoutSnapshot, VisualLine},
    selection::TextPosition,
    session::MarkdownDocumentSession,
    widget::{
        draw_visible_layers_for_test, DrawLayer, DrawRecorder, MarkdownEditor, MarkdownEditorRef,
        MarkdownEditorWidgetRefExt,
    },
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextRange, TextSize,
};

#[test]
fn retained_normal_click_places_caret() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(2, 1, SelectionModifier::Replace);
    assert!(fixture.primary().is_empty());
    assert_eq!(fixture.primary().cursor.offset, t(2));
}

#[test]
fn retained_drag_extends_selection() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(2, 1, SelectionModifier::Replace);
    fixture.drag_to_offset(5);
    assert_eq!(fixture.selected_text(), "pha");
}

#[test]
fn retained_double_click_selects_word() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(8, 2, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "beta");
}

#[test]
fn retained_triple_click_selects_source_line() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(13, 3, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "second\n");
}

#[test]
fn retained_platform_modifier_adds_selection() {
    let mut fixture = Fixture::new("one two");
    fixture.click_at_offset(1, 1, SelectionModifier::Replace);
    fixture.click_at_offset(5, 1, SelectionModifier::Add);
    assert_eq!(fixture.session().selections().as_slice().len(), 2);
    fixture.click_at_offset(7, 1, SelectionModifier::Extend);
    assert_eq!(fixture.selected_text(), "two");
}

#[test]
fn read_only_mode_allows_selection_and_copy_but_not_mutation() {
    let mut fixture = Fixture::new("raw *markdown*");
    fixture.session_mut().set_read_only(true);
    fixture.select_all();
    assert_eq!(fixture.copy(), "raw *markdown*");
    let response = fixture.type_text("x");
    assert!(response.proposals.is_empty());
    assert_eq!(fixture.text(), "raw *markdown*");
}

#[test]
fn retained_keyboard_motion_keeps_caret_visible() {
    let mut fixture = Fixture::with_variable_layout();
    fixture.set_viewport(100.0, 40.0);
    fixture.place_caret_at_end();
    let first = fixture.ensure_caret_visible();
    assert!(first.scroll_y > 0.0);
    fixture.resize_width(50.0);
    let second = fixture.ensure_caret_visible();
    assert!(second.scroll_y >= first.scroll_y);
}

#[test]
fn stale_pointer_geometry_is_rejected_without_mutating_selection() {
    let mut session = session_at_revision("abc", DocumentRevision::new(1));
    let before = session.selections().clone();
    let layout = linear_layout("abc");
    let error = match MarkdownEditorController::default().handle(
        &mut session,
        &layout,
        EditorInput::PointerDown(PointerGesture {
            point: dvec2(20.0, 0.0),
            clicks: 1,
            modifier: SelectionModifier::Replace,
        }),
    ) {
        Err(error) => error,
        Ok(_) => panic!("stale pointer geometry was accepted"),
    };
    assert!(matches!(
        error,
        ControllerError::Layout(LayoutError::RevisionMismatch { document, layout })
            if document == DocumentRevision::new(1) && layout == DocumentRevision::INITIAL
    ));
    assert_eq!(session.selections().as_slice(), before.as_slice());
}

#[test]
fn stale_layout_cannot_publish_ime_coordinates() {
    let mut session = session_at_revision("abc", DocumentRevision::new(1));
    let layout = linear_layout("abc");
    let error = match MarkdownEditorController::default().handle(
        &mut session,
        &layout,
        EditorInput::Copy,
    ) {
        Err(error) => error,
        Ok(_) => panic!("stale IME geometry was accepted"),
    };
    assert!(matches!(
        error,
        ControllerError::Layout(LayoutError::RevisionMismatch { document, layout })
            if document == DocumentRevision::new(1) && layout == DocumentRevision::INITIAL
    ));
}

#[test]
fn mutating_input_does_not_publish_ime_coordinates_from_entry_layout() {
    let mut fixture = Fixture::new("abc");
    fixture.click_at_offset(1, 1, SelectionModifier::Replace);
    let response = fixture.type_text("x");
    assert_eq!(response.proposals.len(), 1);
    assert_eq!(fixture.text(), "axbc");
    assert!(response.request_ime_at.is_none());
}

#[test]
fn divergence_widget_emits_exact_changes_not_full_string() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    widget.set_key_focus(&mut cx);
    let actions = widget
        .handle_input_with_session(&mut cx, &mut session, EditorInput::Text(Arc::from("x")))
        .unwrap();
    let proposal = MarkdownEditorRef::proposed_edit(&actions).unwrap();
    assert_eq!(proposal.edit.base_revision, DocumentRevision::INITIAL);
    assert_eq!(proposal.edit.changes.len(), 1);
    assert_eq!(proposal.snapshot.text().shared().as_str(), "xab");
}

#[test]
fn retained_copy_cut_paste_use_source_text() {
    let mut fixture = Fixture::new("raw *markdown*");
    fixture.select_all();
    assert_eq!(fixture.copy(), "raw *markdown*");
    let cut = fixture.handle(EditorInput::Cut);
    assert_eq!(cut.clipboard.as_deref(), Some("raw *markdown*"));
    assert_eq!(fixture.text(), "");
    fixture.refresh_layout();
    fixture.handle(EditorInput::Paste(Arc::from("raw *markdown*")));
    assert_eq!(fixture.text(), "raw *markdown*");
}

#[test]
fn retained_undo_redo_restore_selection() {
    let mut fixture = Fixture::new("");
    fixture.type_text("abc");
    fixture.refresh_layout();
    fixture.handle(EditorInput::Key(
        waml_markdown_editor::input::EditorKey::Undo,
    ));
    assert_eq!(fixture.text(), "");
    assert_eq!(fixture.primary().cursor.offset, t(0));
    fixture.refresh_layout();
    fixture.handle(EditorInput::Key(
        waml_markdown_editor::input::EditorKey::Redo,
    ));
    assert_eq!(fixture.text(), "abc");
    assert_eq!(fixture.primary().cursor.offset, t(3));
}

#[test]
fn divergence_extended_graphemes_replace_scalar_steps() {
    let mut fixture = Fixture::new("a👩‍💻b");
    fixture.place_caret_at_end();
    fixture.handle(EditorInput::Key(
        waml_markdown_editor::input::EditorKey::Left { extend: false },
    ));
    fixture.handle(EditorInput::Key(
        waml_markdown_editor::input::EditorKey::Left { extend: false },
    ));
    assert_eq!(fixture.primary().cursor.offset.to_usize(), 1);
}

#[test]
fn divergence_variable_metrics_replace_fixed_cell_grid() {
    let layout = variable_layout();
    assert_eq!(
        layout
            .source_to_point(TextPosition::new(t(3), Affinity::Before))
            .unwrap()
            .rect
            .pos,
        dvec2(0.0, 50.0)
    );
}

#[test]
fn divergence_ime_preedit_is_not_committed_text() {
    let mut fixture = Fixture::new("ab");
    fixture.handle(EditorInput::ImeStart);
    fixture.handle(EditorInput::ImeUpdate {
        preedit: "候補".to_owned(),
        selection: 0..2,
    });
    assert_eq!(fixture.text(), "ab");
    assert_eq!(
        fixture.session().local_revision(),
        DocumentRevision::INITIAL
    );
}

#[test]
fn mounted_widget_adapts_a_makepad_text_event() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    widget.set_key_focus(&mut cx);
    let actions = widget
        .handle_event_with_session(
            &mut cx,
            &Event::TextInput(TextInputEvent {
                input: "x".to_owned(),
                ..Default::default()
            }),
            &mut session,
        )
        .unwrap();
    assert_eq!(
        MarkdownEditorRef::proposed_edit(&actions)
            .unwrap()
            .snapshot
            .text()
            .shared()
            .as_str(),
        "xab"
    );
}

#[test]
fn every_layer_uses_one_layout_snapshot_in_required_order() {
    let mut recorder = DrawRecorder::default();
    let layout = Arc::new(LayoutSnapshot::wrapped_fixture_for_test());
    draw_visible_layers_for_test(&layout, &mut recorder);
    assert_eq!(
        recorder.layers(),
        &[
            DrawLayer::BlockBackground,
            DrawLayer::Selection,
            DrawLayer::Text,
            DrawLayer::Decoration,
            DrawLayer::EmbeddedBlock,
            DrawLayer::CaretAndIme,
        ]
    );
    assert!(recorder
        .snapshot_ptrs()
        .iter()
        .all(|ptr| *ptr == Arc::as_ptr(&layout)));
}

#[test]
fn ime_window_uses_current_interpolated_caret_geometry() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    let target = Arc::new(LayoutSnapshot::wrapped_fixture_for_test());
    widget.test_set_layout(target.clone());
    widget.test_show_ime(&mut cx, &mut session);
    assert_eq!(
        widget.test_last_ime_point(),
        target
            .source_to_point(session.selections().primary().cursor)
            .unwrap()
            .rect
            .pos
    );
}

#[test]
fn mounted_widget_keeps_two_host_sessions_isolated() {
    let (mut cx, widget, mut first) = mounted_editor("ab");
    let mut second = session_at_revision("cd", DocumentRevision::INITIAL);
    widget
        .handle_input_with_session(&mut cx, &mut first, EditorInput::Text(Arc::from("x")))
        .unwrap();
    assert_eq!(first.snapshot().text().shared().as_str(), "xab");
    assert_eq!(second.snapshot().text().shared().as_str(), "cd");
    widget.test_set_layout(Arc::new(linear_layout("cd")));
    widget
        .handle_input_with_session(&mut cx, &mut second, EditorInput::Text(Arc::from("y")))
        .unwrap();
    assert_eq!(first.snapshot().text().shared().as_str(), "xab");
    assert_eq!(second.snapshot().text().shared().as_str(), "ycd");
}

#[test]
fn mounted_widget_reports_stale_layout_as_a_typed_error() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    session
        .execute(
            waml_markdown_editor::edit::EditCommand::Insert(Arc::from("x")),
            waml_markdown_editor::edit::HistoryGroup::isolated(),
        )
        .unwrap();
    // The widget first tries to reinstall the layout; with a stale
    // presentation behind it that recovery cannot succeed, and the original
    // mismatch is what surfaces.
    let error = widget
        .handle_input_with_session(&mut cx, &mut session, EditorInput::Text(Arc::from("y")))
        .unwrap_err();
    assert!(matches!(
        error,
        waml_markdown_editor::widget::MarkdownEditorError::ControllerLayout(
            waml_markdown_editor::layout::LayoutError::RevisionMismatch { .. }
        )
    ));
}

#[test]
fn mounted_read_only_widget_rejects_mutating_input() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    widget.set_read_only(&mut cx, true);
    let actions = widget
        .handle_input_with_session(&mut cx, &mut session, EditorInput::Text(Arc::from("x")))
        .unwrap();
    assert!(actions.is_empty());
    assert_eq!(session.snapshot().text().shared().as_str(), "ab");
}

#[test]
fn mounted_widget_primary_modifier_adds_a_selection() {
    let (mut cx, widget, mut session) = mounted_editor("ab");
    widget
        .handle_input_with_session(
            &mut cx,
            &mut session,
            EditorInput::PointerDown(PointerGesture {
                point: dvec2(10.0, 0.0),
                clicks: 1,
                modifier: SelectionModifier::Add,
            }),
        )
        .unwrap();
    assert_eq!(session.selections().as_slice().len(), 2);
}

struct Fixture {
    session: MarkdownDocumentSession,
    controller: MarkdownEditorController,
    layout: LayoutSnapshot,
    viewport_width: f64,
    viewport_height: f64,
}

impl Fixture {
    fn new(text: &str) -> Self {
        let source = SourceText::new(text.to_owned()).unwrap();
        let syntax = parse_markdown(
            DocumentRevision::INITIAL,
            source,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let snapshot = Arc::new(MarkdownDocumentSnapshot::new(syntax));
        Self {
            layout: linear_layout(text),
            session: MarkdownDocumentSession::new(snapshot),
            controller: MarkdownEditorController::default(),
            viewport_width: 100.0,
            viewport_height: 40.0,
        }
    }

    fn with_variable_layout() -> Self {
        let mut fixture = Self::new("abcdef");
        fixture.layout = variable_layout();
        fixture
    }

    fn point_at_offset(&self, offset: usize) -> makepad_widgets::DVec2 {
        self.layout
            .source_to_point(TextPosition::new(t(offset), Affinity::Before))
            .or_else(|| {
                self.layout
                    .source_to_point(TextPosition::new(t(offset), Affinity::After))
            })
            .unwrap()
            .rect
            .pos
    }

    fn click_at_offset(&mut self, offset: usize, clicks: u8, modifier: SelectionModifier) {
        let point = self.point_at_offset(offset);
        self.handle(EditorInput::PointerDown(PointerGesture {
            point,
            clicks,
            modifier,
        }));
    }

    fn drag_to_offset(&mut self, offset: usize) {
        let point = self.point_at_offset(offset);
        self.handle(EditorInput::PointerMove { point });
        self.handle(EditorInput::PointerUp);
    }

    fn handle(&mut self, input: EditorInput) -> EditorResponse {
        self.controller
            .handle(&mut self.session, &self.layout, input)
            .unwrap()
    }

    fn primary(&self) -> waml_markdown_editor::selection::Selection {
        self.session.selections().primary()
    }

    fn selected_text(&self) -> String {
        self.session
            .snapshot()
            .text()
            .slice(self.primary().range())
            .unwrap()
            .to_owned()
    }

    fn session(&self) -> &MarkdownDocumentSession {
        &self.session
    }

    fn session_mut(&mut self) -> &mut MarkdownDocumentSession {
        &mut self.session
    }

    fn select_all(&mut self) {
        self.session.select_all().unwrap();
    }

    fn copy(&mut self) -> String {
        self.handle(EditorInput::Copy).clipboard.unwrap()
    }

    fn type_text(&mut self, text: &str) -> EditorResponse {
        self.handle(EditorInput::Text(Arc::from(text)))
    }

    fn refresh_layout(&mut self) {
        self.layout = linear_layout_at(self.text(), self.session.local_revision());
    }

    fn text(&self) -> &str {
        self.session.snapshot().text().shared().as_str()
    }

    fn set_viewport(&mut self, width: f64, height: f64) {
        self.viewport_width = width;
        self.viewport_height = height;
    }

    fn place_caret_at_end(&mut self) {
        self.session
            .set_primary_offset(t(self.text().len()))
            .unwrap();
    }

    fn ensure_caret_visible(&mut self) -> waml_markdown_editor::input::ScrollAdjustment {
        self.controller
            .ensure_primary_caret_visible(&mut self.session, &self.layout, self.viewport_height)
            .unwrap()
    }

    fn resize_width(&mut self, width: f64) {
        let anchor = self
            .controller
            .capture_scroll_anchor(&self.session, &self.layout)
            .unwrap();
        self.viewport_width = width;
        self.controller
            .restore_scroll_anchor(&mut self.session, &self.layout, anchor)
            .unwrap();
    }
}

fn linear_layout(text: &str) -> LayoutSnapshot {
    linear_layout_at(text, DocumentRevision::INITIAL)
}

fn linear_layout_at(text: &str, revision: DocumentRevision) -> LayoutSnapshot {
    let mut clusters = Vec::new();
    for index in 0..text.len() {
        clusters.push(GlyphCluster::for_test(
            range(index, index + 1),
            Rect {
                pos: dvec2(index as f64 * 10.0, 0.0),
                size: dvec2(10.0, 20.0),
            },
            vec![
                CaretStop::new(
                    TextPosition::new(t(index), Affinity::Before),
                    dvec2(index as f64 * 10.0, 0.0),
                ),
                CaretStop::new(
                    TextPosition::new(t(index + 1), Affinity::After),
                    dvec2((index + 1) as f64 * 10.0, 0.0),
                ),
            ],
        ));
    }
    LayoutSnapshot::from_parts_for_test(
        revision,
        dvec2(text.len() as f64 * 10.0, 20.0),
        vec![VisualLine::for_test(range(0, text.len()), 0.0, 20.0)],
        clusters,
        Vec::new(),
    )
}

fn session_at_revision(text: &str, revision: DocumentRevision) -> MarkdownDocumentSession {
    let source = SourceText::new(text.to_owned()).unwrap();
    let syntax = parse_markdown(revision, source, MarkdownDialect::WAML_DEFAULT).unwrap();
    MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)))
}

fn mounted_editor(text: &str) -> (Cx, MarkdownEditorRef, MarkdownDocumentSession) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    waml_markdown_editor::live_design(&mut cx);
    let widget = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(MarkdownEditor::script_new_with_default),
    ));
    let editor = widget.as_markdown_editor();
    editor.test_set_layout(Arc::new(linear_layout(text)));
    (
        cx,
        editor,
        session_at_revision(text, DocumentRevision::INITIAL),
    )
}

fn variable_layout() -> LayoutSnapshot {
    let mut clusters = Vec::new();
    for index in 0..6 {
        let y = if index < 3 { 0.0 } else { 50.0 };
        let x = (index % 3) as f64 * 13.0;
        clusters.push(GlyphCluster::for_test(
            range(index, index + 1),
            Rect {
                pos: dvec2(x, y),
                size: dvec2(13.0, if index < 3 { 18.0 } else { 30.0 }),
            },
            vec![
                CaretStop::new(TextPosition::new(t(index), Affinity::Before), dvec2(x, y)),
                CaretStop::new(
                    TextPosition::new(t(index + 1), Affinity::After),
                    dvec2(x + 13.0, y),
                ),
            ],
        ));
    }
    LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(100.0, 80.0),
        vec![
            VisualLine::for_test(range(0, 3), 0.0, 18.0),
            VisualLine::for_test(range(3, 6), 50.0, 30.0),
        ],
        clusters,
        Vec::new(),
    )
}

fn t(value: usize) -> TextSize {
    TextSize::try_from_usize(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(t(start), t(end)).unwrap()
}
