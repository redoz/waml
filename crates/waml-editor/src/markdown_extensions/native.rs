//! The threaded fenced-block render scheduler.
//!
//! Renders run on a small pool of persistent worker threads and report back
//! over an mpsc channel that the UI thread drains. This module is compiled
//! only off wasm, which is why nothing in it is `cfg`-gated on the target:
//! the gate lives on the `mod native;` declaration in the parent.

use std::{
    collections::VecDeque,
    io,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
};

use makepad_widgets::SignalToUI;
use waml_markdown_editor::reading::BlockExtensionEvent;
#[cfg(test)]
use waml_markdown_editor::reading::BlockExtensionRequestId;

use super::{
    completed_event, failed_event, MarkdownExtensionLeaseId, QueuedRender, RenderKey, RenderLedger,
    RenderScheduler,
};

/// Called from a worker thread once a render lands, so the UI wakes up and
/// drains the completion channel.
pub(super) type WakeUi = Arc<dyn Fn() + Send + Sync>;

type NativeTask = Box<dyn FnOnce() + Send + 'static>;

/// Injected so a test can make worker startup fail on demand.
pub(super) type NativeSpawner = Arc<dyn Fn(String, NativeTask) -> io::Result<()> + Send + Sync>;

/// `(worker index, lease, event)` — the worker index lets the host mark that
/// slot idle again.
type Completion = (usize, MarkdownExtensionLeaseId, BlockExtensionEvent);

/// Maximum number of persistent native renderer workers per shared host.
const RENDER_CONCURRENCY: usize = 4;
/// Maximum cold misses waiting for a native renderer worker.
const RENDER_QUEUE_CAPACITY: usize = 64;
const QUEUE_FULL_MESSAGE: &str = "Markdown block renderer queue is full";
const RENDER_PANIC_MESSAGE: &str = "Markdown block renderer panicked";
const WORKER_SPAWN_MESSAGE: &str = "Markdown block renderer worker could not start";

fn default_spawner() -> NativeSpawner {
    Arc::new(|name, task| std::thread::Builder::new().name(name).spawn(task).map(drop))
}

#[derive(Clone, Copy)]
struct Inflight {
    key: RenderKey,
}

impl Inflight {
    fn from_queued(queued: &QueuedRender) -> Self {
        Self {
            key: (queued.lease, queued.request.request_id),
        }
    }
}

enum Dispatch {
    Handled,
    NoCapacity(QueuedRender),
}

pub(super) struct NativeScheduler {
    completion_tx: Sender<Completion>,
    completion_rx: Receiver<Completion>,
    wake_ui: WakeUi,
    spawner: NativeSpawner,
    workers: Vec<Option<Sender<QueuedRender>>>,
    inflight: Vec<Option<Inflight>>,
    idle: VecDeque<usize>,
    active: usize,
    queue: VecDeque<QueuedRender>,
}

impl NativeScheduler {
    pub(super) fn new() -> Self {
        Self::with_wake(Arc::new(SignalToUI::set_ui_signal))
    }

    pub(super) fn with_wake(wake_ui: WakeUi) -> Self {
        Self::with_spawner(wake_ui, default_spawner())
    }

    pub(super) fn with_spawner(wake_ui: WakeUi, spawner: NativeSpawner) -> Self {
        let (completion_tx, completion_rx) = mpsc::channel();
        Self {
            completion_tx,
            completion_rx,
            wake_ui,
            spawner,
            workers: (0..RENDER_CONCURRENCY).map(|_| None).collect(),
            inflight: (0..RENDER_CONCURRENCY).map(|_| None).collect(),
            idle: VecDeque::new(),
            active: 0,
            queue: VecDeque::new(),
        }
    }

    fn try_start(&mut self, ledger: &mut RenderLedger, mut queued: QueuedRender) -> Dispatch {
        loop {
            while let Some(worker) = self.idle.pop_front() {
                let Some(sender) = self.workers[worker].as_ref().cloned() else {
                    continue;
                };
                let inflight = Inflight::from_queued(&queued);
                match sender.send(queued) {
                    Ok(()) => {
                        self.active += 1;
                        self.inflight[worker] = Some(inflight);
                        return Dispatch::Handled;
                    }
                    Err(error) => {
                        self.workers[worker] = None;
                        queued = error.0;
                    }
                }
            }

            let Some(worker) = self.workers.iter().position(Option::is_none) else {
                return Dispatch::NoCapacity(queued);
            };
            if self.spawn_worker(worker).is_err() {
                ledger.admit_event(
                    queued.lease,
                    failed_event(&queued.request, WORKER_SPAWN_MESSAGE.into()),
                );
                return Dispatch::Handled;
            }
        }
    }

    fn spawn_worker(&mut self, worker: usize) -> io::Result<()> {
        let (job_tx, job_rx) = mpsc::channel::<QueuedRender>();
        let completion = self.completion_tx.clone();
        let wake_ui = self.wake_ui.clone();
        let task: NativeTask = Box::new(move || {
            while let Ok(queued) = job_rx.recv() {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    queued.renderer.render_and_cache(&queued.request)
                }))
                .unwrap_or_else(|_| Err(RENDER_PANIC_MESSAGE.into()));
                let event = completed_event(&queued.request, result);
                if completion.send((worker, queued.lease, event)).is_err() {
                    break;
                }
                wake_ui();
            }
        });
        (self.spawner)(format!("waml-markdown-render-{worker}"), task)?;
        self.workers[worker] = Some(job_tx);
        self.idle.push_back(worker);
        Ok(())
    }

    fn dispatch_queue(&mut self, ledger: &mut RenderLedger) {
        while let Some(queued) = self.queue.pop_front() {
            if self.discard_queue_entry(ledger, &queued) {
                continue;
            }
            match self.try_start(ledger, queued) {
                Dispatch::Handled => {}
                Dispatch::NoCapacity(queued) => {
                    self.queue.push_front(queued);
                    break;
                }
            }
        }
    }

    fn prune_queue(&mut self, ledger: &mut RenderLedger) {
        let mut queued = std::mem::take(&mut self.queue);
        while let Some(render) = queued.pop_front() {
            if !self.discard_queue_entry(ledger, &render) {
                self.queue.push_back(render);
            }
        }
    }

    /// Whether `queued` is dead weight, retiring its ledger entry if so.
    fn discard_queue_entry(&self, ledger: &mut RenderLedger, queued: &QueuedRender) -> bool {
        let key = (queued.lease, queued.request.request_id);
        if ledger.render_is_live(key, &queued.request) {
            return false;
        }
        let same_key_is_active = self
            .inflight
            .iter()
            .flatten()
            .any(|inflight| inflight.key == key);
        if ledger.render_should_retire(key) && !same_key_is_active {
            ledger.retire(key);
        }
        true
    }
}

impl RenderScheduler for NativeScheduler {
    fn submit(&mut self, ledger: &mut RenderLedger, queued: QueuedRender) {
        match self.try_start(ledger, queued) {
            Dispatch::Handled => {}
            Dispatch::NoCapacity(queued) => {
                self.prune_queue(ledger);
                if self.queue.len() < RENDER_QUEUE_CAPACITY {
                    self.queue.push_back(queued);
                } else {
                    ledger.admit_event(
                        queued.lease,
                        failed_event(&queued.request, QUEUE_FULL_MESSAGE.into()),
                    );
                }
            }
        }
    }

    fn poll(&mut self, ledger: &mut RenderLedger) {
        while let Ok((worker, lease, event)) = self.completion_rx.try_recv() {
            self.active = self.active.saturating_sub(1);
            self.inflight[worker] = None;
            if self.workers[worker].is_some() {
                self.idle.push_back(worker);
            }
            ledger.admit_event(lease, event);
        }
        self.dispatch_queue(ledger);
    }

    fn forget_lease(&mut self, lease: MarkdownExtensionLeaseId) {
        self.queue.retain(|queued| queued.lease != lease);
    }

    /// A threaded scheduler never owes the UI thread a turn.
    ///
    /// Only declared under `test` because the trait only declares it there off
    /// wasm — the UI-driven half of the contract belongs to the cooperative
    /// scheduler.
    #[cfg(test)]
    fn has_deferred_work(&self) -> bool {
        false
    }

    #[cfg(test)]
    fn run_one_deferred(&mut self, _ledger: &mut RenderLedger) -> bool {
        false
    }

    #[cfg(test)]
    fn active_count(&self) -> usize {
        self.active
    }

    #[cfg(test)]
    fn queued_request_ids(&self) -> Vec<BlockExtensionRequestId> {
        self.queue
            .iter()
            .map(|queued| queued.request.request_id)
            .collect()
    }
}
