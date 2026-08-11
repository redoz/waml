use std::sync::Arc;

use makepad_widgets::{dvec2, Cx, Rect, ScriptNew, WidgetRef};
use waml_markdown_editor::{
    document::MarkdownDocumentSnapshot,
    input::{EditorInput, ScrollState},
    layout::{CaretStop, GlyphCluster, LayoutSnapshot, VisualLine},
    motion::{LayoutChangeCause, MotionConfig, MotionController, MotionCutReason},
    presentation::{compile_presentation, HighlighterRegistry, PresentationStyles},
    reading::{
        build_reading_document, BlockExtensionAppearance, BlockExtensionEvent,
        BlockExtensionEventOutcome, BlockExtensionRequest, BlockExtensionRequestId,
        BlockExtensionState, BlockExtensionStates, MarkdownBlockExtensionHost, ReadingBlock,
        ReadingDocument, RegisteredBlockExtensions, RenderedBlockSvg,
    },
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

#[derive(Default)]
struct RecordingExtensionHost {
    requests: Vec<BlockExtensionRequest>,
    canceled: Vec<BlockExtensionRequestId>,
    events: Vec<BlockExtensionEvent>,
}

impl MarkdownBlockExtensionHost for RecordingExtensionHost {
    fn request(&mut self, request: BlockExtensionRequest) {
        self.requests.push(request);
    }

    fn cancel(&mut self, request_id: BlockExtensionRequestId) {
        self.canceled.push(request_id);
    }

    fn drain_events(&mut self) -> Vec<BlockExtensionEvent> {
        std::mem::take(&mut self.events)
    }
}

fn mermaid_reading_document(source: &str, revision: DocumentRevision) -> ReadingDocument {
    let syntax = parse_markdown(
        revision,
        SourceText::new(source.to_owned()).unwrap(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let plan = compile_presentation(
        &syntax,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .unwrap();
    build_reading_document(
        &plan,
        &RegisteredBlockExtensions::from_languages([Arc::from("mermaid")]),
    )
    .unwrap()
}

fn has_visible_text(blocks: &[ReadingBlock], source: &str, expected: &str) -> bool {
    blocks.iter().any(|block| {
        block.pieces.iter().any(|piece| {
            piece.emit
                && source
                    .get(piece.range.start().to_usize()..piece.range.end().to_usize())
                    .is_some_and(|text| text.contains(expected))
        }) || has_visible_text(&block.children, source, expected)
    })
}

fn ready_event(request: &BlockExtensionRequest) -> BlockExtensionEvent {
    BlockExtensionEvent::Ready {
        request_id: request.request_id,
        revision: request.revision,
        item: request.item,
        source_range: request.source_range,
        svg: RenderedBlockSvg {
            data: Arc::from(&b"<svg xmlns='http://www.w3.org/2000/svg'></svg>"[..]),
            logical_size: (120.0, 80.0),
        },
    }
}

fn failed_event(request: &BlockExtensionRequest) -> BlockExtensionEvent {
    BlockExtensionEvent::Failed {
        request_id: request.request_id,
        revision: request.revision,
        item: request.item,
        source_range: request.source_range,
        message: Arc::from("invalid Mermaid"),
    }
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

#[test]
fn mermaid_fences_move_from_loading_to_ready_or_failed_without_changing_source() {
    let source = "Before.\n\n```mermaid\nflowchart LR\n    A --> B\n```\n\nBetween.\n\n```MERMAID\nflowchart LR\n    A -->\n```\n\nAfter.\n";
    let revision = DocumentRevision::new(11);
    let document = mermaid_reading_document(source, revision);
    let source_bytes = source.as_bytes().to_vec();
    let shared_source: Arc<str> = Arc::from(source);
    let mut host = RecordingExtensionHost::default();
    let mut states = BlockExtensionStates::default();

    states.reconcile(
        &mut host,
        revision,
        &document,
        shared_source.clone(),
        BlockExtensionAppearance::Light,
    );

    assert_eq!(host.requests.len(), 2);
    assert_eq!(states.pending_count(), 2);
    assert!(states
        .frame(revision)
        .items
        .iter()
        .all(|(_, state)| matches!(state, BlockExtensionState::Loading)));
    let ready = ready_event(&host.requests[0]);
    let failed = failed_event(&host.requests[1]);
    assert_eq!(
        states.apply_event(ready),
        BlockExtensionEventOutcome::Applied
    );
    assert_eq!(
        states.apply_event(failed),
        BlockExtensionEventOutcome::Applied
    );

    let frame = states.frame(revision);
    assert_eq!(states.pending_count(), 0);
    assert_eq!(
        frame
            .items
            .iter()
            .filter(|(_, state)| matches!(state, BlockExtensionState::Ready(_)))
            .count(),
        1
    );
    assert_eq!(
        frame
            .items
            .iter()
            .filter(|(_, state)| matches!(state, BlockExtensionState::Failed(_)))
            .count(),
        1
    );
    assert_eq!(shared_source.as_bytes(), source_bytes);
    for prose in ["Before.", "Between.", "After."] {
        assert!(has_visible_text(&document.roots, source, prose));
    }
}

#[test]
fn a_new_mermaid_revision_cancels_old_work_and_rejects_its_event() {
    let old_revision = DocumentRevision::new(21);
    let new_revision = DocumentRevision::new(22);
    let old_source = "```mermaid\nflowchart LR\n    Old --> Ready\n```\n";
    let new_source = "```mermaid\nflowchart LR\n    New -->\n```\n";
    let old_document = mermaid_reading_document(old_source, old_revision);
    let new_document = mermaid_reading_document(new_source, new_revision);
    let mut host = RecordingExtensionHost::default();
    let mut states = BlockExtensionStates::default();
    states.reconcile(
        &mut host,
        old_revision,
        &old_document,
        Arc::from(old_source),
        BlockExtensionAppearance::Light,
    );
    let old_request = host.requests[0].clone();

    states.reconcile(
        &mut host,
        new_revision,
        &new_document,
        Arc::from(new_source),
        BlockExtensionAppearance::Light,
    );
    let new_request = host.requests[1].clone();

    assert_eq!(host.canceled, vec![old_request.request_id]);
    assert_eq!(
        states.apply_event(ready_event(&old_request)),
        BlockExtensionEventOutcome::IgnoredStale
    );
    assert_eq!(
        states.apply_event(failed_event(&new_request)),
        BlockExtensionEventOutcome::Applied
    );
    assert!(matches!(
        states.frame(new_revision).items.as_ref(),
        [(_, BlockExtensionState::Failed(_))]
    ));
}

#[test]
fn a_non_mermaid_fence_never_requests_the_extension_host() {
    let source = "Before.\n\n```rust\nfn main() {}\n```\n\nAfter.\n";
    let revision = DocumentRevision::new(31);
    let document = mermaid_reading_document(source, revision);
    let mut host = RecordingExtensionHost::default();
    let mut states = BlockExtensionStates::default();

    states.reconcile(
        &mut host,
        revision,
        &document,
        Arc::from(source),
        BlockExtensionAppearance::Dark,
    );

    assert!(host.requests.is_empty());
    assert_eq!(states.pending_count(), 0);
    assert!(states.frame(revision).items.is_empty());
}
