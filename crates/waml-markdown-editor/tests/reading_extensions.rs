use std::sync::Arc;

use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::{
    build_reading_document, BlockExtensionAppearance, BlockExtensionEvent,
    BlockExtensionEventOutcome, BlockExtensionRequest, BlockExtensionRequestId,
    BlockExtensionState, BlockExtensionStates, MarkdownBlockExtensionHost, ReadingBlock,
    ReadingBlockKind, ReadingDocument, RegisteredBlockExtensions, RenderedBlockSvg,
};
use waml_markdown_editor::syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, TextRange, TextSize,
};

#[derive(Default)]
struct FakeHost {
    requests: Vec<BlockExtensionRequest>,
    cancellations: Vec<BlockExtensionRequestId>,
}

impl MarkdownBlockExtensionHost for FakeHost {
    fn request(&mut self, request: BlockExtensionRequest) {
        self.requests.push(request);
    }

    fn cancel(&mut self, request_id: BlockExtensionRequestId) {
        self.cancellations.push(request_id);
    }

    fn drain_events(&mut self) -> Vec<BlockExtensionEvent> {
        Vec::new()
    }
}

fn document(source: &str) -> ReadingDocument {
    let source_text = SourceText::new(source).expect("valid source");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        source_text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let plan = compile_presentation(
        &syntax,
        &PresentationStyles::balanced(),
        &HighlighterRegistry::default(),
    )
    .expect("presentation compiles");
    build_reading_document(
        &plan,
        &RegisteredBlockExtensions::from_languages([Arc::from("mermaid")]),
    )
    .expect("reading model builds")
}

fn first_extension(
    document: &ReadingDocument,
) -> waml_markdown_editor::reading::FencedBlockExtension {
    fn find(
        blocks: &[ReadingBlock],
    ) -> Option<waml_markdown_editor::reading::FencedBlockExtension> {
        blocks.iter().find_map(|block| match &block.kind {
            ReadingBlockKind::FencedExtension(extension) => Some(extension.clone()),
            _ => find(&block.children),
        })
    }
    find(&document.roots).expect("an extension block")
}

fn svg() -> RenderedBlockSvg {
    RenderedBlockSvg {
        data: Arc::from(&b"<svg/>"[..]),
        logical_size: (20.0, 10.0),
    }
}

#[test]
fn reconciliation_requests_live_fences_once_and_cancels_removed_or_revised_work() {
    let source: Arc<str> = Arc::from("```mermaid\ngraph TD; A-->B\n```\n");
    let reading_document = document(&source);
    let mut host = FakeHost::default();
    let mut states = BlockExtensionStates::default();

    states.reconcile(
        &mut host,
        DocumentRevision::INITIAL,
        &reading_document,
        source.clone(),
        BlockExtensionAppearance::Light,
    );
    assert_eq!(host.requests.len(), 1);
    assert_eq!(host.requests[0].content.as_ref(), "graph TD; A-->B\n");
    assert_eq!(states.pending_count(), 1);

    states.reconcile(
        &mut host,
        DocumentRevision::INITIAL,
        &reading_document,
        source.clone(),
        BlockExtensionAppearance::Light,
    );
    assert_eq!(
        host.requests.len(),
        1,
        "a live fence is requested once per revision"
    );

    let empty_document = document("plain text\n");
    states.reconcile(
        &mut host,
        DocumentRevision::INITIAL,
        &empty_document,
        Arc::from("plain text\n"),
        BlockExtensionAppearance::Light,
    );
    assert_eq!(host.cancellations, vec![host.requests[0].request_id]);
    assert_eq!(states.frame(DocumentRevision::INITIAL).items.len(), 0);

    states.reconcile(
        &mut host,
        DocumentRevision::INITIAL,
        &reading_document,
        source.clone(),
        BlockExtensionAppearance::Light,
    );
    let initial_request = host.requests[1].request_id;
    states.reconcile(
        &mut host,
        DocumentRevision::new(1),
        &reading_document,
        source,
        BlockExtensionAppearance::Light,
    );
    assert_eq!(
        host.cancellations,
        vec![host.requests[0].request_id, initial_request]
    );
    assert_eq!(
        host.requests.len(),
        3,
        "a new revision requests the live fence again"
    );
}

#[test]
fn events_change_only_the_matching_live_loading_entry() {
    let source: Arc<str> = Arc::from("```mermaid\ngraph TD; A-->B\n```\n");
    let document = document(&source);
    let extension = first_extension(&document);
    let mut host = FakeHost::default();
    let mut states = BlockExtensionStates::default();
    states.reconcile(
        &mut host,
        DocumentRevision::INITIAL,
        &document,
        source,
        BlockExtensionAppearance::Dark,
    );
    let request = host.requests[0].clone();

    let stale_events = [
        BlockExtensionEvent::Ready {
            request_id: BlockExtensionRequestId(request.request_id.0 + 1),
            revision: request.revision,
            item: request.item,
            source_range: request.source_range,
            svg: svg(),
        },
        BlockExtensionEvent::Ready {
            request_id: request.request_id,
            revision: DocumentRevision::new(1),
            item: request.item,
            source_range: request.source_range,
            svg: svg(),
        },
        BlockExtensionEvent::Ready {
            request_id: request.request_id,
            revision: request.revision,
            item: waml_markdown_editor::presentation::PresentationItemId {
                fragment_ordinal: request.item.fragment_ordinal + 1,
                ..request.item
            },
            source_range: request.source_range,
            svg: svg(),
        },
        BlockExtensionEvent::Ready {
            request_id: request.request_id,
            revision: request.revision,
            item: request.item,
            source_range: TextRange::new(
                request.source_range.start(),
                request.source_range.start(),
            )
            .expect("valid empty range"),
            svg: svg(),
        },
    ];
    for event in stale_events {
        assert_eq!(
            states.apply_event(event),
            BlockExtensionEventOutcome::IgnoredStale
        );
        assert_eq!(states.pending_count(), 1);
    }

    assert_eq!(
        states.apply_event(BlockExtensionEvent::Ready {
            request_id: request.request_id,
            revision: request.revision,
            item: request.item,
            source_range: request.source_range,
            svg: svg(),
        }),
        BlockExtensionEventOutcome::Applied
    );
    assert_eq!(states.pending_count(), 0);
    assert!(matches!(
        &states.frame(DocumentRevision::INITIAL).items[0],
        (_, BlockExtensionState::Ready(_))
    ));
    assert_eq!(
        states.apply_event(BlockExtensionEvent::Ready {
            request_id: request.request_id,
            revision: request.revision,
            item: request.item,
            source_range: request.source_range,
            svg: svg(),
        }),
        BlockExtensionEventOutcome::IgnoredStale,
        "a duplicate completion cannot replace a ready state"
    );
    assert!(matches!(
        &states.frame(DocumentRevision::INITIAL).items[0],
        (_, BlockExtensionState::Ready(_))
    ));

    let mut failed_states = BlockExtensionStates::default();
    let mut failed_host = FakeHost::default();
    failed_states.reconcile(
        &mut failed_host,
        DocumentRevision::INITIAL,
        &document,
        Arc::from("```mermaid\ngraph TD; A-->B\n```\n"),
        BlockExtensionAppearance::Dark,
    );
    let failed_request = failed_host.requests[0].clone();
    assert_eq!(
        failed_states.apply_event(BlockExtensionEvent::Failed {
            request_id: failed_request.request_id,
            revision: failed_request.revision,
            item: failed_request.item,
            source_range: failed_request.source_range,
            message: Arc::from("diagram has a cycle"),
        }),
        BlockExtensionEventOutcome::Applied
    );
    assert_eq!(failed_states.pending_count(), 0);
    assert_eq!(
        failed_states.frame(DocumentRevision::INITIAL).items[0],
        (
            extension.id,
            BlockExtensionState::Failed(Arc::from("diagram has a cycle"))
        )
    );
}

#[test]
fn an_invalid_extension_content_range_fails_without_requesting_the_host() {
    let source: Arc<str> = Arc::from("```mermaid\ngraph TD; A-->B\n```\n");
    let mut document = document(&source);
    let ReadingBlockKind::FencedExtension(extension) = &mut document.roots[0].kind else {
        panic!("an extension block")
    };
    extension.content_range = TextRange::new(
        extension.content_range.start(),
        TextSize::try_from_usize(source.len() + 1).expect("small test source"),
    )
    .expect("ordered invalid range");

    let mut host = FakeHost::default();
    let mut states = BlockExtensionStates::default();
    states.reconcile(
        &mut host,
        DocumentRevision::INITIAL,
        &document,
        source,
        BlockExtensionAppearance::Light,
    );

    assert!(host.requests.is_empty(), "an invalid range stays local");
    assert_eq!(states.pending_count(), 0);
    assert!(matches!(
        &states.frame(DocumentRevision::INITIAL).items[0],
        (_, BlockExtensionState::Failed(message)) if message.as_ref() == "invalid fenced extension content range"
    ));
}
