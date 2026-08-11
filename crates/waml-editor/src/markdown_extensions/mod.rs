use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

#[cfg(any(target_arch = "wasm32", test))]
use std::collections::VecDeque;

use makepad_widgets::SignalToUI;
use waml_markdown_editor::{
    presentation::PresentationItemId,
    reading::{
        BlockExtensionEvent, BlockExtensionRequest, BlockExtensionRequestId,
        MarkdownBlockExtensionHost, RegisteredBlockExtensions,
    },
    syntax::{DocumentRevision, TextRange},
};

mod mermaid;

type BlockRenderResult = Result<waml_markdown_editor::reading::RenderedBlockSvg, Arc<str>>;
type Completion = (MarkdownExtensionLeaseId, BlockExtensionEvent);
type WakeUi = Arc<dyn Fn() + Send + Sync>;

trait FencedBlockRenderer: Send + Sync {
    fn language(&self) -> &'static str;
    fn cached(&self, request: &BlockExtensionRequest) -> Option<BlockRenderResult>;
    fn render_and_cache(&self, request: &BlockExtensionRequest) -> BlockRenderResult;
}

impl FencedBlockRenderer for mermaid::MermaidRenderer {
    fn language(&self) -> &'static str {
        self.language()
    }

    fn cached(&self, request: &BlockExtensionRequest) -> Option<BlockRenderResult> {
        self.cached(request)
    }

    fn render_and_cache(&self, request: &BlockExtensionRequest) -> BlockRenderResult {
        self.render_and_cache(request)
    }
}

pub type SharedMarkdownExtensionHost = Rc<RefCell<EditorMarkdownExtensionHost>>;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MarkdownExtensionLeaseId(u64);

pub struct MarkdownExtensionLease {
    shared: SharedMarkdownExtensionHost,
    id: MarkdownExtensionLeaseId,
}

#[derive(Clone, Copy)]
enum RenderExecutor {
    #[cfg(not(target_arch = "wasm32"))]
    Native,
    #[cfg(any(target_arch = "wasm32", test))]
    Cooperative,
}

#[derive(Clone, Copy)]
struct PendingRender {
    revision: DocumentRevision,
    item: PresentationItemId,
    source_range: TextRange,
}

impl PendingRender {
    fn from_request(request: &BlockExtensionRequest) -> Self {
        Self {
            revision: request.revision,
            item: request.item,
            source_range: request.source_range,
        }
    }

    fn matches_event(self, event: &BlockExtensionEvent) -> bool {
        let (_, revision, item, source_range) = event_identity(event);
        (self.revision, self.item, self.source_range) == (revision, item, source_range)
    }
}

#[cfg(target_arch = "wasm32")]
struct QueuedRender {
    lease: MarkdownExtensionLeaseId,
    renderer: Arc<dyn FencedBlockRenderer>,
    request: BlockExtensionRequest,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
struct QueuedRender {
    lease: MarkdownExtensionLeaseId,
    renderer: Arc<dyn FencedBlockRenderer>,
    request: BlockExtensionRequest,
}

pub struct EditorMarkdownExtensionHost {
    renderers: BTreeMap<Arc<str>, Arc<dyn FencedBlockRenderer>>,
    completed: BTreeMap<(MarkdownExtensionLeaseId, BlockExtensionRequestId), BlockExtensionEvent>,
    canceled: BTreeSet<(MarkdownExtensionLeaseId, BlockExtensionRequestId)>,
    pending: BTreeMap<(MarkdownExtensionLeaseId, BlockExtensionRequestId), PendingRender>,
    active_leases: BTreeSet<MarkdownExtensionLeaseId>,
    completion_tx: Sender<Completion>,
    completion_rx: Receiver<Completion>,
    next_lease: u64,
    executor: RenderExecutor,
    #[cfg(not(target_arch = "wasm32"))]
    wake_ui: WakeUi,
    #[cfg(any(target_arch = "wasm32", test))]
    deferred: VecDeque<QueuedRender>,
}

impl EditorMarkdownExtensionHost {
    pub fn shared() -> SharedMarkdownExtensionHost {
        let renderer: Arc<dyn FencedBlockRenderer> = mermaid::renderer();
        #[cfg(not(target_arch = "wasm32"))]
        let executor = RenderExecutor::Native;
        #[cfg(target_arch = "wasm32")]
        let executor = RenderExecutor::Cooperative;
        Self::shared_with([renderer], executor, Arc::new(SignalToUI::set_ui_signal))
    }

    pub fn open_lease(shared: &SharedMarkdownExtensionHost) -> MarkdownExtensionLease {
        let id = {
            let mut host = shared.borrow_mut();
            host.next_lease = host.next_lease.wrapping_add(1);
            let id = MarkdownExtensionLeaseId(host.next_lease);
            host.active_leases.insert(id);
            id
        };
        MarkdownExtensionLease {
            shared: shared.clone(),
            id,
        }
    }

    fn shared_with(
        renderers: impl IntoIterator<Item = Arc<dyn FencedBlockRenderer>>,
        executor: RenderExecutor,
        _wake_ui: WakeUi,
    ) -> SharedMarkdownExtensionHost {
        let (completion_tx, completion_rx) = mpsc::channel();
        let renderers = renderers
            .into_iter()
            .filter(|renderer| renderer.language().is_ascii())
            .map(|renderer| {
                (
                    Arc::<str>::from(renderer.language().to_ascii_lowercase()),
                    renderer,
                )
            })
            .collect();
        Rc::new(RefCell::new(Self {
            renderers,
            completed: BTreeMap::new(),
            canceled: BTreeSet::new(),
            pending: BTreeMap::new(),
            active_leases: BTreeSet::new(),
            completion_tx,
            completion_rx,
            next_lease: 0,
            executor,
            #[cfg(not(target_arch = "wasm32"))]
            wake_ui: _wake_ui,
            #[cfg(target_arch = "wasm32")]
            deferred: VecDeque::new(),
            #[cfg(all(test, not(target_arch = "wasm32")))]
            deferred: VecDeque::new(),
        }))
    }

    #[cfg(test)]
    fn with_renderer_for_test(
        renderer: Arc<dyn FencedBlockRenderer>,
        executor: RenderExecutor,
        wake_ui: WakeUi,
    ) -> SharedMarkdownExtensionHost {
        Self::shared_with([renderer], executor, wake_ui)
    }

    fn request_for_lease(
        &mut self,
        lease: MarkdownExtensionLeaseId,
        request: BlockExtensionRequest,
    ) {
        let key = (lease, request.request_id);
        self.canceled.remove(&key);
        self.completed.remove(&key);
        self.pending
            .insert(key, PendingRender::from_request(&request));

        let Some(renderer) = self.renderer_for(&request.language) else {
            self.admit_event(
                lease,
                failed_event(&request, "Markdown block language is not registered".into()),
            );
            return;
        };
        if let Some(result) = renderer.cached(&request) {
            self.admit_event(lease, completed_event(&request, result));
            return;
        }

        match self.executor {
            #[cfg(not(target_arch = "wasm32"))]
            RenderExecutor::Native => {
                let completion = self.completion_tx.clone();
                let wake_ui = self.wake_ui.clone();
                std::thread::spawn(move || {
                    let event = completed_event(&request, renderer.render_and_cache(&request));
                    if completion.send((lease, event)).is_ok() {
                        wake_ui();
                    }
                });
            }
            #[cfg(any(target_arch = "wasm32", test))]
            RenderExecutor::Cooperative => self.deferred.push_back(QueuedRender {
                lease,
                renderer,
                request,
            }),
        }
    }

    fn renderer_for(&self, language: &str) -> Option<Arc<dyn FencedBlockRenderer>> {
        language
            .is_ascii()
            .then(|| language.to_ascii_lowercase())
            .and_then(|language| self.renderers.get(language.as_str()).cloned())
    }

    fn cancel_for_lease(
        &mut self,
        lease: MarkdownExtensionLeaseId,
        request_id: BlockExtensionRequestId,
    ) {
        let key = (lease, request_id);
        if self.pending.contains_key(&key) {
            self.canceled.insert(key);
        }
    }

    fn drain_events_for_lease(
        &mut self,
        lease: MarkdownExtensionLeaseId,
    ) -> Vec<BlockExtensionEvent> {
        while let Ok((event_lease, event)) = self.completion_rx.try_recv() {
            self.admit_event(event_lease, event);
        }

        let keys: Vec<_> = self
            .completed
            .keys()
            .filter(|(event_lease, _)| *event_lease == lease)
            .copied()
            .collect();
        let mut ready = Vec::with_capacity(keys.len());
        for key in keys {
            let Some(event) = self.completed.remove(&key) else {
                continue;
            };
            self.pending.remove(&key);
            if self.canceled.remove(&key) || !self.active_leases.contains(&lease) {
                continue;
            }
            ready.push(event);
        }
        ready
    }

    fn admit_event(&mut self, lease: MarkdownExtensionLeaseId, event: BlockExtensionEvent) {
        let request_id = event_identity(&event).0;
        let key = (lease, request_id);
        let is_expected = self
            .pending
            .get(&key)
            .is_some_and(|pending| pending.matches_event(&event));
        if !self.active_leases.contains(&lease) || self.canceled.contains(&key) {
            self.retire(key);
            return;
        }
        if !is_expected {
            return;
        }
        self.completed.insert(key, event);
    }

    fn close_lease(&mut self, lease: MarkdownExtensionLeaseId) {
        self.active_leases.remove(&lease);
        self.completed
            .retain(|(event_lease, _), _| *event_lease != lease);
        self.canceled
            .retain(|(event_lease, _)| *event_lease != lease);
        self.pending
            .retain(|(event_lease, _), _| *event_lease != lease);
        #[cfg(any(target_arch = "wasm32", test))]
        self.deferred.retain(|queued| queued.lease != lease);
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn run_one_deferred_render(&mut self) -> bool {
        while let Some(queued) = self.deferred.pop_front() {
            let key = (queued.lease, queued.request.request_id);
            if !self.render_is_live(key, &queued.request) {
                if self.render_should_retire(key) {
                    self.retire(key);
                }
                continue;
            }

            let result = queued.renderer.render_and_cache(&queued.request);
            if self.render_is_live(key, &queued.request) {
                self.admit_event(queued.lease, completed_event(&queued.request, result));
            } else if self.render_should_retire(key) {
                self.retire(key);
            }
            break;
        }
        !self.deferred.is_empty()
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn render_is_live(
        &self,
        key: (MarkdownExtensionLeaseId, BlockExtensionRequestId),
        request: &BlockExtensionRequest,
    ) -> bool {
        self.active_leases.contains(&key.0)
            && !self.canceled.contains(&key)
            && self
                .pending
                .get(&key)
                .is_some_and(|pending| *pending == PendingRender::from_request(request))
    }

    #[cfg(any(target_arch = "wasm32", test))]
    fn render_should_retire(
        &self,
        key: (MarkdownExtensionLeaseId, BlockExtensionRequestId),
    ) -> bool {
        !self.active_leases.contains(&key.0)
            || self.canceled.contains(&key)
            || !self.pending.contains_key(&key)
    }

    fn retire(&mut self, key: (MarkdownExtensionLeaseId, BlockExtensionRequestId)) {
        self.completed.remove(&key);
        self.canceled.remove(&key);
        self.pending.remove(&key);
    }
}

impl PartialEq for PendingRender {
    fn eq(&self, other: &Self) -> bool {
        (self.revision, self.item, self.source_range)
            == (other.revision, other.item, other.source_range)
    }
}

impl Eq for PendingRender {}

impl MarkdownExtensionLease {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn id(&self) -> MarkdownExtensionLeaseId {
        self.id
    }

    pub fn registered_languages(&self) -> RegisteredBlockExtensions {
        RegisteredBlockExtensions::from_languages(self.shared.borrow().renderers.keys().cloned())
    }

    pub fn has_deferred_work(&self) -> bool {
        #[cfg(any(target_arch = "wasm32", test))]
        {
            return !self.shared.borrow().deferred.is_empty();
        }
        #[cfg(not(any(target_arch = "wasm32", test)))]
        false
    }

    pub fn run_one_deferred(&mut self) -> bool {
        #[cfg(any(target_arch = "wasm32", test))]
        {
            return self.shared.borrow_mut().run_one_deferred_render();
        }
        #[cfg(not(any(target_arch = "wasm32", test)))]
        false
    }
}

impl Drop for MarkdownExtensionLease {
    fn drop(&mut self) {
        self.shared.borrow_mut().close_lease(self.id);
    }
}

impl MarkdownBlockExtensionHost for MarkdownExtensionLease {
    fn request(&mut self, request: BlockExtensionRequest) {
        self.shared.borrow_mut().request_for_lease(self.id, request);
    }

    fn cancel(&mut self, request_id: BlockExtensionRequestId) {
        self.shared
            .borrow_mut()
            .cancel_for_lease(self.id, request_id);
    }

    fn drain_events(&mut self) -> Vec<BlockExtensionEvent> {
        self.shared.borrow_mut().drain_events_for_lease(self.id)
    }
}

fn completed_event(
    request: &BlockExtensionRequest,
    result: BlockRenderResult,
) -> BlockExtensionEvent {
    match result {
        Ok(svg) => BlockExtensionEvent::Ready {
            request_id: request.request_id,
            revision: request.revision,
            item: request.item,
            source_range: request.source_range,
            svg,
        },
        Err(message) => failed_event(request, message),
    }
}

fn failed_event(request: &BlockExtensionRequest, message: Arc<str>) -> BlockExtensionEvent {
    BlockExtensionEvent::Failed {
        request_id: request.request_id,
        revision: request.revision,
        item: request.item,
        source_range: request.source_range,
        message,
    }
}

fn event_identity(
    event: &BlockExtensionEvent,
) -> (
    BlockExtensionRequestId,
    DocumentRevision,
    PresentationItemId,
    TextRange,
) {
    match event {
        BlockExtensionEvent::Ready {
            request_id,
            revision,
            item,
            source_range,
            ..
        }
        | BlockExtensionEvent::Failed {
            request_id,
            revision,
            item,
            source_range,
            ..
        } => (*request_id, *revision, *item, *source_range),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        thread::{self, ThreadId},
        time::{Duration, Instant},
    };

    use waml_markdown_editor::{
        presentation::{EmbeddedBlockRole, PresentationItemId, PresentationRole},
        reading::{
            BlockExtensionAppearance, BlockExtensionEvent, BlockExtensionRequest,
            BlockExtensionRequestId, MarkdownBlockExtensionHost, RenderedBlockSvg,
        },
        syntax::{DocumentRevision, TextRange, TextSize},
    };
    use waml_syntax::SyntaxIdentity;

    use super::{
        completed_event, EditorMarkdownExtensionHost, FencedBlockRenderer, RenderExecutor,
        SharedMarkdownExtensionHost,
    };

    #[derive(Default)]
    struct CountingRenderer {
        render_calls: AtomicUsize,
        render_threads: Mutex<Vec<ThreadId>>,
    }

    impl CountingRenderer {
        fn render_calls(&self) -> usize {
            self.render_calls.load(Ordering::Relaxed)
        }

        fn render_threads(&self) -> Vec<ThreadId> {
            self.render_threads
                .lock()
                .expect("render thread list poisoned")
                .clone()
        }
    }

    impl FencedBlockRenderer for CountingRenderer {
        fn language(&self) -> &'static str {
            "mermaid"
        }

        fn cached(&self, request: &BlockExtensionRequest) -> Option<BlockRenderResult> {
            (request.content.as_ref() == "cached").then(|| Ok(svg(b"cached")))
        }

        fn render_and_cache(&self, request: &BlockExtensionRequest) -> BlockRenderResult {
            self.render_calls.fetch_add(1, Ordering::Relaxed);
            self.render_threads
                .lock()
                .expect("render thread list poisoned")
                .push(thread::current().id());
            Ok(svg(request.content.as_bytes()))
        }
    }

    type BlockRenderResult = Result<RenderedBlockSvg, Arc<str>>;

    fn svg(data: &[u8]) -> RenderedBlockSvg {
        RenderedBlockSvg {
            data: Arc::from(data),
            logical_size: (12.0, 8.0),
        }
    }

    fn request(id: u64, language: &str, content: &str) -> BlockExtensionRequest {
        BlockExtensionRequest {
            request_id: BlockExtensionRequestId(id),
            revision: DocumentRevision::new(id + 10),
            item: PresentationItemId {
                owner: SyntaxIdentity::from_raw_for_test(id + 20),
                role: PresentationRole::Embedded(EmbeddedBlockRole::FencedExtension),
                fragment_ordinal: u32::try_from(id).expect("test id must fit in u32"),
            },
            source_range: TextRange::new(TextSize::new(3), TextSize::new(9)).unwrap(),
            content_range: TextRange::new(TextSize::new(4), TextSize::new(8)).unwrap(),
            language: Arc::from(language),
            content: Arc::from(content),
            appearance: BlockExtensionAppearance::Light,
        }
    }

    fn cooperative_host(renderer: Arc<dyn FencedBlockRenderer>) -> SharedMarkdownExtensionHost {
        EditorMarkdownExtensionHost::with_renderer_for_test(
            renderer,
            RenderExecutor::Cooperative,
            Arc::new(|| {}),
        )
    }

    fn only_event(host: &mut impl MarkdownBlockExtensionHost) -> BlockExtensionEvent {
        let events = host.drain_events();
        assert_eq!(events.len(), 1);
        events.into_iter().next().unwrap()
    }

    #[test]
    fn registered_language_and_lookup_are_ascii_case_insensitive() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        assert!(lease.registered_languages().contains("MERMAID"));
        lease.request(request(1, "MeRmAiD", "flowchart TD\nA-->B"));
        assert!(!lease.run_one_deferred());

        assert_eq!(renderer.render_calls(), 1);
        assert!(matches!(
            only_event(&mut lease),
            BlockExtensionEvent::Ready { .. }
        ));
    }

    #[test]
    fn unregistered_language_fails_without_invoking_a_renderer() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        lease.request(request(2, "plantuml", "Alice -> Bob"));

        assert_eq!(renderer.render_calls(), 0);
        assert!(matches!(
            only_event(&mut lease),
            BlockExtensionEvent::Failed {
                request_id: BlockExtensionRequestId(2),
                ..
            }
        ));
    }

    #[test]
    fn cache_hit_completes_immediately() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        lease.request(request(3, "mermaid", "cached"));

        assert!(!lease.has_deferred_work());
        assert_eq!(renderer.render_calls(), 0);
        match only_event(&mut lease) {
            BlockExtensionEvent::Ready { svg, .. } => assert_eq!(svg.data.as_ref(), b"cached"),
            BlockExtensionEvent::Failed { message, .. } => panic!("unexpected failure: {message}"),
        }
    }

    #[test]
    fn cancellation_before_a_deferred_render_retires_the_work() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        lease.request(request(4, "mermaid", "flowchart TD\nA-->B"));
        lease.cancel(BlockExtensionRequestId(4));

        assert!(!lease.run_one_deferred());
        assert_eq!(renderer.render_calls(), 0);
        assert!(lease.drain_events().is_empty());
    }

    #[test]
    fn a_completion_for_a_closed_lease_is_rejected() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer);
        let mut closed = EditorMarkdownExtensionHost::open_lease(&shared);
        let late = request(5, "mermaid", "flowchart TD\nA-->B");
        closed.request(late.clone());
        let completion = completed_event(&late, Ok(svg(b"late")));
        shared
            .borrow()
            .completion_tx
            .send((closed.id(), completion))
            .unwrap();

        drop(closed);

        let mut survivor = EditorMarkdownExtensionHost::open_lease(&shared);
        assert!(survivor.drain_events().is_empty());
        assert!(shared.borrow().completed.is_empty());
        assert!(shared.borrow().pending.is_empty());
    }

    #[test]
    fn completion_preserves_request_identity() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer);
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);
        let expected = request(6, "mermaid", "flowchart TD\nA-->B");
        lease.request(expected.clone());

        assert!(!lease.run_one_deferred());

        match only_event(&mut lease) {
            BlockExtensionEvent::Ready {
                request_id,
                revision,
                item,
                source_range,
                svg,
            } => {
                assert_eq!(request_id, expected.request_id);
                assert_eq!(revision, expected.revision);
                assert_eq!(item, expected.item);
                assert_eq!(source_range, expected.source_range);
                assert_eq!(svg.data.as_ref(), expected.content.as_bytes());
            }
            BlockExtensionEvent::Failed { message, .. } => panic!("unexpected failure: {message}"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_cache_miss_runs_off_thread_and_wakes_after_send() {
        let caller_thread = thread::current().id();
        let renderer = Arc::new(CountingRenderer::default());
        let wakes = Arc::new(AtomicUsize::new(0));
        let wake_count = wakes.clone();
        let shared = EditorMarkdownExtensionHost::with_renderer_for_test(
            renderer.clone(),
            RenderExecutor::Native,
            Arc::new(move || {
                wake_count.fetch_add(1, Ordering::Relaxed);
            }),
        );
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        lease.request(request(7, "mermaid", "flowchart TD\nA-->B"));
        let event = wait_for_event(&mut lease);
        wait_until(|| wakes.load(Ordering::Relaxed) == 1);

        assert!(matches!(event, BlockExtensionEvent::Ready { .. }));
        assert_eq!(renderer.render_calls(), 1);
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
        assert!(renderer
            .render_threads()
            .into_iter()
            .all(|render_thread| render_thread != caller_thread));
    }

    #[test]
    fn cooperative_queue_renders_one_miss_per_turn_and_cache_hits_are_free() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);
        lease.request(request(8, "mermaid", "cached"));
        lease.request(request(9, "mermaid", "flowchart TD\nA-->B"));
        lease.request(request(10, "mermaid", "sequenceDiagram\nA->>B: hi"));

        assert!(lease.run_one_deferred());
        assert_eq!(renderer.render_calls(), 1);
        assert_eq!(lease.drain_events().len(), 2);

        assert!(!lease.run_one_deferred());
        assert_eq!(renderer.render_calls(), 2);
        assert_eq!(lease.drain_events().len(), 1);
    }

    #[test]
    fn stale_queued_work_does_not_retire_a_reused_request_id() {
        let renderer = Arc::new(CountingRenderer::default());
        let shared = cooperative_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);
        lease.request(request(11, "mermaid", "old"));
        let mut current = request(11, "mermaid", "current");
        current.revision = DocumentRevision::new(999);
        lease.request(current);

        assert!(!lease.run_one_deferred());

        assert_eq!(renderer.render_calls(), 1);
        match only_event(&mut lease) {
            BlockExtensionEvent::Ready { svg, .. } => assert_eq!(svg.data.as_ref(), b"current"),
            BlockExtensionEvent::Failed { message, .. } => panic!("unexpected failure: {message}"),
        }
    }

    fn wait_for_event(host: &mut impl MarkdownBlockExtensionHost) -> BlockExtensionEvent {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(event) = host.drain_events().into_iter().next() {
                return event;
            }
            assert!(Instant::now() < deadline, "renderer did not complete");
            thread::yield_now();
        }
    }

    fn wait_until(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "condition did not become true");
            thread::yield_now();
        }
    }
}
