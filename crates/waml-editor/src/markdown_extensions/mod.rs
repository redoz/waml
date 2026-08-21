use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

#[cfg(not(target_arch = "wasm32"))]
use std::{
    io,
    panic::{catch_unwind, AssertUnwindSafe},
};

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
type Completion = (Option<usize>, MarkdownExtensionLeaseId, BlockExtensionEvent);
type WakeUi = Arc<dyn Fn() + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
type NativeTask = Box<dyn FnOnce() + Send + 'static>;
#[cfg(not(target_arch = "wasm32"))]
type NativeSpawner = Arc<dyn Fn(String, NativeTask) -> io::Result<()> + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
/// Maximum number of persistent native renderer workers per shared host.
const NATIVE_RENDER_CONCURRENCY: usize = 4;
#[cfg(not(target_arch = "wasm32"))]
/// Maximum cold misses waiting for a native renderer worker.
const NATIVE_RENDER_QUEUE_CAPACITY: usize = 64;
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_QUEUE_FULL_MESSAGE: &str = "Markdown block renderer queue is full";
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_RENDER_PANIC_MESSAGE: &str = "Markdown block renderer panicked";
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_WORKER_SPAWN_MESSAGE: &str = "Markdown block renderer worker could not start";

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

fn renderer_registry(
    renderers: impl IntoIterator<Item = Arc<dyn FencedBlockRenderer>>,
) -> BTreeMap<Arc<str>, Arc<dyn FencedBlockRenderer>> {
    renderers
        .into_iter()
        .filter(|renderer| renderer.language().is_ascii())
        .map(|renderer| {
            (
                Arc::<str>::from(renderer.language().to_ascii_lowercase()),
                renderer,
            )
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn default_native_spawner() -> NativeSpawner {
    Arc::new(|name, task| std::thread::Builder::new().name(name).spawn(task).map(drop))
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

struct QueuedRender {
    lease: MarkdownExtensionLeaseId,
    renderer: Arc<dyn FencedBlockRenderer>,
    request: BlockExtensionRequest,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
struct NativeInflight {
    key: (MarkdownExtensionLeaseId, BlockExtensionRequestId),
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeInflight {
    fn from_queued(queued: &QueuedRender) -> Self {
        Self {
            key: (queued.lease, queued.request.request_id),
        }
    }
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
    #[cfg(not(target_arch = "wasm32"))]
    native_spawner: NativeSpawner,
    #[cfg(not(target_arch = "wasm32"))]
    native_workers: Vec<Option<Sender<QueuedRender>>>,
    #[cfg(not(target_arch = "wasm32"))]
    native_inflight: Vec<Option<NativeInflight>>,
    #[cfg(not(target_arch = "wasm32"))]
    native_idle: VecDeque<usize>,
    #[cfg(not(target_arch = "wasm32"))]
    native_active: usize,
    #[cfg(not(target_arch = "wasm32"))]
    native_queue: VecDeque<QueuedRender>,
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
        #[cfg(not(target_arch = "wasm32"))]
        return Self::shared_with_native_spawner(
            renderers,
            executor,
            _wake_ui,
            default_native_spawner(),
        );

        #[cfg(target_arch = "wasm32")]
        {
            let (completion_tx, completion_rx) = mpsc::channel();
            Rc::new(RefCell::new(Self {
                renderers: renderer_registry(renderers),
                completed: BTreeMap::new(),
                canceled: BTreeSet::new(),
                pending: BTreeMap::new(),
                active_leases: BTreeSet::new(),
                completion_tx,
                completion_rx,
                next_lease: 0,
                executor,
                deferred: VecDeque::new(),
            }))
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn shared_with_native_spawner(
        renderers: impl IntoIterator<Item = Arc<dyn FencedBlockRenderer>>,
        executor: RenderExecutor,
        wake_ui: WakeUi,
        native_spawner: NativeSpawner,
    ) -> SharedMarkdownExtensionHost {
        let (completion_tx, completion_rx) = mpsc::channel();
        Rc::new(RefCell::new(Self {
            renderers: renderer_registry(renderers),
            completed: BTreeMap::new(),
            canceled: BTreeSet::new(),
            pending: BTreeMap::new(),
            active_leases: BTreeSet::new(),
            completion_tx,
            completion_rx,
            next_lease: 0,
            executor,
            wake_ui,
            native_spawner,
            native_workers: (0..NATIVE_RENDER_CONCURRENCY).map(|_| None).collect(),
            native_inflight: (0..NATIVE_RENDER_CONCURRENCY).map(|_| None).collect(),
            native_idle: VecDeque::new(),
            native_active: 0,
            native_queue: VecDeque::new(),
            #[cfg(test)]
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

    #[cfg(all(test, not(target_arch = "wasm32")))]
    fn with_native_spawner_for_test(
        renderer: Arc<dyn FencedBlockRenderer>,
        wake_ui: WakeUi,
        native_spawner: NativeSpawner,
    ) -> SharedMarkdownExtensionHost {
        Self::shared_with_native_spawner(
            [renderer],
            RenderExecutor::Native,
            wake_ui,
            native_spawner,
        )
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
                self.schedule_native(QueuedRender {
                    lease,
                    renderer,
                    request,
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
        while let Ok((worker, event_lease, event)) = self.completion_rx.try_recv() {
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(worker) = worker {
                self.native_active = self.native_active.saturating_sub(1);
                self.native_inflight[worker] = None;
                if self.native_workers[worker].is_some() {
                    self.native_idle.push_back(worker);
                }
            }
            #[cfg(target_arch = "wasm32")]
            let _ = worker;
            self.admit_event(event_lease, event);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if matches!(self.executor, RenderExecutor::Native) {
            self.dispatch_native_queue();
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

    #[cfg(not(target_arch = "wasm32"))]
    fn schedule_native(&mut self, queued: QueuedRender) {
        match self.try_start_native(queued) {
            NativeDispatch::Handled => {}
            NativeDispatch::NoCapacity(queued) => {
                self.prune_native_queue();
                if self.native_queue.len() < NATIVE_RENDER_QUEUE_CAPACITY {
                    self.native_queue.push_back(queued);
                } else {
                    self.admit_event(
                        queued.lease,
                        failed_event(&queued.request, NATIVE_QUEUE_FULL_MESSAGE.into()),
                    );
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn try_start_native(&mut self, mut queued: QueuedRender) -> NativeDispatch {
        loop {
            while let Some(worker) = self.native_idle.pop_front() {
                let Some(sender) = self.native_workers[worker].as_ref().cloned() else {
                    continue;
                };
                let inflight = NativeInflight::from_queued(&queued);
                match sender.send(queued) {
                    Ok(()) => {
                        self.native_active += 1;
                        self.native_inflight[worker] = Some(inflight);
                        return NativeDispatch::Handled;
                    }
                    Err(error) => {
                        self.native_workers[worker] = None;
                        queued = error.0;
                    }
                }
            }

            let Some(worker) = self.native_workers.iter().position(Option::is_none) else {
                return NativeDispatch::NoCapacity(queued);
            };
            if self.spawn_native_worker(worker).is_err() {
                self.admit_event(
                    queued.lease,
                    failed_event(&queued.request, NATIVE_WORKER_SPAWN_MESSAGE.into()),
                );
                return NativeDispatch::Handled;
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_native_worker(&mut self, worker: usize) -> io::Result<()> {
        let (job_tx, job_rx) = mpsc::channel::<QueuedRender>();
        let completion = self.completion_tx.clone();
        let wake_ui = self.wake_ui.clone();
        let task: NativeTask = Box::new(move || {
            while let Ok(queued) = job_rx.recv() {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    queued.renderer.render_and_cache(&queued.request)
                }))
                .unwrap_or_else(|_| Err(NATIVE_RENDER_PANIC_MESSAGE.into()));
                let event = completed_event(&queued.request, result);
                if completion
                    .send((Some(worker), queued.lease, event))
                    .is_err()
                {
                    break;
                }
                wake_ui();
            }
        });
        (self.native_spawner)(format!("waml-markdown-render-{worker}"), task)?;
        self.native_workers[worker] = Some(job_tx);
        self.native_idle.push_back(worker);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn dispatch_native_queue(&mut self) {
        while let Some(queued) = self.native_queue.pop_front() {
            if self.discard_native_queue_entry(&queued) {
                continue;
            }
            match self.try_start_native(queued) {
                NativeDispatch::Handled => {}
                NativeDispatch::NoCapacity(queued) => {
                    self.native_queue.push_front(queued);
                    break;
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn prune_native_queue(&mut self) {
        let mut queued = std::mem::take(&mut self.native_queue);
        while let Some(render) = queued.pop_front() {
            if !self.discard_native_queue_entry(&render) {
                self.native_queue.push_back(render);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn discard_native_queue_entry(&mut self, queued: &QueuedRender) -> bool {
        let key = (queued.lease, queued.request.request_id);
        if self.render_is_live(key, &queued.request) {
            return false;
        }
        let same_key_is_active = self
            .native_inflight
            .iter()
            .flatten()
            .any(|inflight| inflight.key == key);
        if self.render_should_retire(key) && !same_key_is_active {
            self.retire(key);
        }
        true
    }

    fn close_lease(&mut self, lease: MarkdownExtensionLeaseId) {
        self.active_leases.remove(&lease);
        self.completed
            .retain(|(event_lease, _), _| *event_lease != lease);
        self.canceled
            .retain(|(event_lease, _)| *event_lease != lease);
        self.pending
            .retain(|(event_lease, _), _| *event_lease != lease);
        #[cfg(not(target_arch = "wasm32"))]
        self.native_queue.retain(|queued| queued.lease != lease);
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

#[cfg(not(target_arch = "wasm32"))]
enum NativeDispatch {
    Handled,
    NoCapacity(QueuedRender),
}

impl MarkdownExtensionLease {
    /// Test seam: assertions key the registry's internal maps by lease id.
    #[cfg(test)]
    pub fn id(&self) -> MarkdownExtensionLeaseId {
        self.id
    }

    pub fn registered_languages(&self) -> RegisteredBlockExtensions {
        RegisteredBlockExtensions::from_languages(self.shared.borrow().renderers.keys().cloned())
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub fn has_deferred_work(&self) -> bool {
        !self.shared.borrow().deferred.is_empty()
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub fn run_one_deferred(&mut self) -> bool {
        self.shared.borrow_mut().run_one_deferred_render()
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
        collections::HashSet,
        io,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Condvar, Mutex,
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

    #[derive(Default)]
    struct BlockingRenderer {
        active: AtomicUsize,
        max_active: AtomicUsize,
        started: Mutex<Vec<BlockExtensionRequestId>>,
        render_threads: Mutex<Vec<ThreadId>>,
        released: Mutex<bool>,
        release: Condvar,
    }

    impl BlockingRenderer {
        fn started(&self) -> Vec<BlockExtensionRequestId> {
            self.started.lock().expect("started list poisoned").clone()
        }

        fn max_active(&self) -> usize {
            self.max_active.load(Ordering::SeqCst)
        }

        fn unique_render_threads(&self) -> usize {
            self.render_threads
                .lock()
                .expect("render thread list poisoned")
                .iter()
                .copied()
                .collect::<HashSet<_>>()
                .len()
        }

        fn release_all(&self) {
            *self.released.lock().expect("release gate poisoned") = true;
            self.release.notify_all();
        }
    }

    impl FencedBlockRenderer for BlockingRenderer {
        fn language(&self) -> &'static str {
            "mermaid"
        }

        fn cached(&self, _request: &BlockExtensionRequest) -> Option<BlockRenderResult> {
            None
        }

        fn render_and_cache(&self, request: &BlockExtensionRequest) -> BlockRenderResult {
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.max_active.fetch_max(active, Ordering::SeqCst);
            self.started
                .lock()
                .expect("started list poisoned")
                .push(request.request_id);
            self.render_threads
                .lock()
                .expect("render thread list poisoned")
                .push(thread::current().id());

            let mut released = self.released.lock().expect("release gate poisoned");
            while !*released {
                released = self.release.wait(released).expect("release gate poisoned");
            }
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(svg(request.content.as_bytes()))
        }
    }

    struct PanickingRenderer;

    impl FencedBlockRenderer for PanickingRenderer {
        fn language(&self) -> &'static str {
            "mermaid"
        }

        fn cached(&self, _request: &BlockExtensionRequest) -> Option<BlockRenderResult> {
            None
        }

        fn render_and_cache(&self, _request: &BlockExtensionRequest) -> BlockRenderResult {
            panic!("synthetic renderer panic")
        }
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

    #[cfg(not(target_arch = "wasm32"))]
    fn native_host(renderer: Arc<dyn FencedBlockRenderer>) -> SharedMarkdownExtensionHost {
        EditorMarkdownExtensionHost::with_renderer_for_test(
            renderer,
            RenderExecutor::Native,
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
            .send((None, closed.id(), completion))
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_burst_uses_four_reused_workers_and_defers_fifo() {
        let renderer = Arc::new(BlockingRenderer::default());
        let shared = native_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        for id in 100..106 {
            lease.request(request(id, "mermaid", "cold"));
        }
        wait_until(|| renderer.started().len() == 4);
        let active = shared.borrow().native_active;
        let queued: Vec<_> = shared
            .borrow()
            .native_queue
            .iter()
            .map(|job| job.request.request_id)
            .collect();
        renderer.release_all();

        let events = drain_until(&mut lease, 6);

        assert_eq!(active, 4);
        assert_eq!(
            queued,
            [BlockExtensionRequestId(104), BlockExtensionRequestId(105)]
        );
        assert_eq!(events.len(), 6);
        assert!(renderer.max_active() <= 4);
        assert!(renderer.unique_render_threads() <= 4);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_full_queue_fails_the_excess_request_immediately() {
        let renderer = Arc::new(BlockingRenderer::default());
        let shared = native_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        for id in 200..269 {
            lease.request(request(id, "mermaid", "cold"));
        }
        wait_until(|| renderer.started().len() == 4);
        let overflow = only_event(&mut lease);
        for id in 204..268 {
            lease.cancel(BlockExtensionRequestId(id));
        }
        renderer.release_all();
        let completed = drain_until(&mut lease, 4);

        assert_eq!(shared.borrow().native_queue.len(), 0);
        assert_eq!(completed.len(), 4);
        match overflow {
            BlockExtensionEvent::Failed {
                request_id,
                message,
                ..
            } => {
                assert_eq!(request_id, BlockExtensionRequestId(268));
                assert_eq!(message.as_ref(), "Markdown block renderer queue is full");
            }
            BlockExtensionEvent::Ready { .. } => panic!("overflow request rendered"),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn canceled_full_queue_accepts_and_renders_a_replacement_before_release() {
        let renderer = Arc::new(BlockingRenderer::default());
        let shared = native_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        for id in 500..568 {
            lease.request(request(id, "mermaid", "cold"));
        }
        wait_until(|| renderer.started().len() == 4);
        for id in 504..568 {
            lease.cancel(BlockExtensionRequestId(id));
        }
        lease.request(request(568, "mermaid", "replacement"));
        let before_release = lease.drain_events();
        renderer.release_all();

        let completed = drain_native_until_idle(&mut lease, &shared);

        assert!(before_release.is_empty());
        assert_eq!(completed.len(), 5);
        assert!(completed.iter().any(|event| matches!(
            event,
            BlockExtensionEvent::Ready {
                request_id: BlockExtensionRequestId(568),
                svg,
                ..
            } if svg.data.as_ref() == b"replacement"
        )));
        assert!(renderer.started().contains(&BlockExtensionRequestId(568)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn canceled_native_queue_entry_never_renders() {
        let renderer = Arc::new(BlockingRenderer::default());
        let shared = native_host(renderer.clone());
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        for id in 300..305 {
            lease.request(request(id, "mermaid", "cold"));
        }
        wait_until(|| renderer.started().len() == 4);
        lease.cancel(BlockExtensionRequestId(304));
        renderer.release_all();

        let events = drain_until(&mut lease, 4);

        assert_eq!(events.len(), 4);
        assert!(!renderer.started().contains(&BlockExtensionRequestId(304)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_renderer_panic_becomes_a_failed_event_and_wake() {
        let wakes = Arc::new(AtomicUsize::new(0));
        let wake_count = wakes.clone();
        let shared = EditorMarkdownExtensionHost::with_renderer_for_test(
            Arc::new(PanickingRenderer),
            RenderExecutor::Native,
            Arc::new(move || {
                wake_count.fetch_add(1, Ordering::Relaxed);
            }),
        );
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);
        lease.request(request(400, "mermaid", "panic"));

        let event = wait_for_event(&mut lease);
        wait_until(|| wakes.load(Ordering::Relaxed) == 1);

        match event {
            BlockExtensionEvent::Failed { message, .. } => {
                assert_eq!(message.as_ref(), "Markdown block renderer panicked")
            }
            BlockExtensionEvent::Ready { .. } => panic!("panicking renderer returned Ready"),
        }
        assert_eq!(wakes.load(Ordering::Relaxed), 1);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_worker_spawn_failure_becomes_a_failed_event() {
        let shared = EditorMarkdownExtensionHost::with_native_spawner_for_test(
            Arc::new(CountingRenderer::default()),
            Arc::new(|| {}),
            Arc::new(|_, _| Err(io::Error::other("synthetic spawn failure"))),
        );
        let mut lease = EditorMarkdownExtensionHost::open_lease(&shared);

        lease.request(request(401, "mermaid", "cold"));

        match only_event(&mut lease) {
            BlockExtensionEvent::Failed { message, .. } => assert_eq!(
                message.as_ref(),
                "Markdown block renderer worker could not start"
            ),
            BlockExtensionEvent::Ready { .. } => panic!("failed spawn returned Ready"),
        }
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

    fn drain_until(
        host: &mut impl MarkdownBlockExtensionHost,
        expected: usize,
    ) -> Vec<BlockExtensionEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut events = Vec::with_capacity(expected);
        while events.len() < expected {
            events.extend(host.drain_events());
            assert!(Instant::now() < deadline, "renderer did not drain");
            thread::yield_now();
        }
        events
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn drain_native_until_idle(
        host: &mut impl MarkdownBlockExtensionHost,
        shared: &SharedMarkdownExtensionHost,
    ) -> Vec<BlockExtensionEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut events = Vec::new();
        loop {
            events.extend(host.drain_events());
            let native_idle = {
                let host = shared.borrow();
                host.native_active == 0 && host.native_queue.is_empty()
            };
            if native_idle {
                return events;
            }
            assert!(
                Instant::now() < deadline,
                "native renderer did not become idle"
            );
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
