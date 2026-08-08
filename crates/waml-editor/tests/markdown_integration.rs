use std::sync::Arc;

use makepad_widgets::{dvec2, Cx, Rect, ScriptNew, WidgetRef};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    input::{EditorInput, ScrollState},
    layout::{CaretStop, GlyphCluster, LayoutSnapshot, VisualLine},
    motion::{LayoutChangeCause, MotionConfig, MotionController, MotionCutReason},
    selection::{Affinity, TextPosition},
    session::{HostSnapshotCause, HostSyncOutcome, MarkdownDocumentSession},
    widget::{MarkdownEditor, MarkdownEditorRef, MarkdownEditorWidgetRefExt},
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextChange, TextRange, TextSize,
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

// Scenario: NATIVE-023
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

// Scenario: NATIVE-022
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

// Scenario: NATIVE-045
#[test]
fn external_replacement_maps_selection_and_scroll_and_cuts_motion() {
    let (_, _, mut session) = mounted("abcdef");
    session.set_primary_offset(size(5)).unwrap();
    session.set_scroll_state(ScrollState { x: 3.0, y: 48.0 });
    let incoming_text = SourceText::new("aXYZdef".to_string()).unwrap();
    let incoming_syntax = parse_markdown(
        DocumentRevision::new(1),
        incoming_text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let incoming = Arc::new(MarkdownDocumentSnapshot::new(incoming_syntax));
    let change = TextChange {
        old_range: range(1, 3),
        replacement: Arc::from("XYZ"),
    };

    let outcome = session
        .synchronize_from_host(
            incoming,
            Some(std::slice::from_ref(&change)),
            HostSnapshotCause::ExternalReplacement,
        )
        .unwrap();

    assert_eq!(outcome, HostSyncOutcome::Installed);
    assert_eq!(session.selections().primary().cursor.offset, size(6));
    assert_eq!(*session.scroll_state(), ScrollState { x: 3.0, y: 48.0 });

    let mut motion = MotionController::new(200.0);
    motion.commit(
        0.0,
        None,
        Arc::new(layout("abcdef")),
        LayoutChangeCause::InitialLoad,
        false,
        None,
        MotionConfig::default(),
    );
    let frame = motion.commit(
        1.0,
        None,
        Arc::new(layout("aXYZdef")),
        LayoutChangeCause::ExternalReplacement,
        false,
        None,
        MotionConfig::default(),
    );
    assert!(!frame.active);
    assert_eq!(frame.cut_reason, Some(MotionCutReason::ExternalReplacement));
}
