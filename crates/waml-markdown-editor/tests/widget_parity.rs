use std::sync::Arc;

use makepad_widgets::{dvec2, Rect};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    input::{
        ControllerError, EditorInput, EditorResponse, MarkdownEditorController, PointerGesture,
        SelectionModifier,
    },
    layout::{Affinity, CaretStop, GlyphCluster, LayoutError, LayoutSnapshot, VisualLine},
    selection::TextPosition,
    session::MarkdownDocumentSession,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextRange, TextSize,
};

#[test]
fn click_drag_double_and_triple_click_match_retained_editor_behavior() {
    let mut fixture = Fixture::new("alpha beta\nsecond\n");
    fixture.click_at_offset(2, 1, SelectionModifier::Replace);
    assert!(fixture.primary().is_empty());
    fixture.drag_to_offset(5);
    assert_eq!(fixture.selected_text(), "pha");
    fixture.click_at_offset(8, 2, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "beta");
    fixture.click_at_offset(13, 3, SelectionModifier::Replace);
    assert_eq!(fixture.selected_text(), "second\n");
}

#[test]
fn platform_modifier_adds_selection_and_shift_extends_primary() {
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
fn caret_visibility_and_resize_use_geometry_not_line_numbers() {
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
        DocumentRevision::INITIAL,
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
