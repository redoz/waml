use std::sync::Arc;

use makepad_widgets::{dvec2, Cx, Rect, ScriptNew, WidgetRef};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    input::EditorInput,
    layout::{CaretStop, GlyphCluster, LayoutSnapshot, VisualLine},
    selection::{Affinity, TextPosition},
    session::MarkdownDocumentSession,
    widget::{MarkdownEditor, MarkdownEditorRef, MarkdownEditorWidgetRefExt},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextRange, TextSize,
};

fn size(value: usize) -> TextSize {
    TextSize::try_from_usize(value).unwrap()
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(size(start), size(end)).unwrap()
}

fn layout(text: &str) -> LayoutSnapshot {
    let clusters = (0..text.len())
        .map(|index| {
            GlyphCluster::for_test(
                range(index, index + 1),
                Rect {
                    pos: dvec2(index as f64 * 10.0, 0.0),
                    size: dvec2(10.0, 20.0),
                },
                vec![
                    CaretStop::new(
                        TextPosition::new(size(index), Affinity::Before),
                        dvec2(index as f64 * 10.0, 0.0),
                    ),
                    CaretStop::new(
                        TextPosition::new(size(index + 1), Affinity::After),
                        dvec2((index + 1) as f64 * 10.0, 0.0),
                    ),
                ],
            )
        })
        .collect();
    LayoutSnapshot::from_parts_for_test(
        DocumentRevision::INITIAL,
        dvec2(text.len() as f64 * 10.0, 20.0),
        vec![VisualLine::for_test(range(0, text.len()), 0.0, 20.0)],
        clusters,
        Vec::new(),
    )
}

fn mounted(text: &str) -> (Cx, MarkdownEditorRef, MarkdownDocumentSession) {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    waml_markdown_editor::live_design(&mut cx);
    let widget = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(MarkdownEditor::script_new_with_default),
    ));
    let editor = widget.as_markdown_editor();
    editor.test_set_layout(Arc::new(layout(text)));
    let text = SourceText::new(text.to_owned()).unwrap();
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let session = MarkdownDocumentSession::new(Arc::new(MarkdownDocumentSnapshot::new(syntax)));
    (cx, editor, session)
}

#[test]
fn editable_mount_emits_an_exact_revisioned_source_proposal() {
    let (mut cx, editor, mut session) = mounted("# Order\n");

    let actions = editor
        .handle_input_with_session(&mut cx, &mut session, EditorInput::Text(Arc::from("X")))
        .unwrap();
    let proposal = MarkdownEditorRef::proposed_edit(&actions).unwrap();

    assert_eq!(proposal.edit.base_revision, DocumentRevision::INITIAL);
    assert_eq!(proposal.edit.changes.len(), 1);
    assert_eq!(proposal.snapshot.text().shared().as_str(), "X# Order\n");
    assert!(Arc::ptr_eq(
        proposal.snapshot.syntax(),
        &proposal.syntax_update.snapshot
    ));
}

#[test]
fn read_only_mount_never_emits_a_source_proposal() {
    let (mut cx, editor, mut session) = mounted("# Order\n");
    editor.set_read_only(&mut cx, true);
    session.set_read_only(true);

    let actions = editor
        .handle_input_with_session(&mut cx, &mut session, EditorInput::Text(Arc::from("X")))
        .unwrap();

    assert!(MarkdownEditorRef::proposed_edit(&actions).is_none());
    assert_eq!(session.snapshot().text().shared().as_str(), "# Order\n");
}
