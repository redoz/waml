//! The cooperative fenced-block render scheduler.
//!
//! wasm has no worker threads to hand a render to, so renders run on the UI
//! thread itself — one per frame, driven by the reading view's deferred-frame
//! loop. This module is compiled on wasm and in native test builds (the tests
//! are the only automated coverage the wasm path has), which is why nothing
//! in it is `cfg`-gated on the target: the gate lives on the
//! `mod cooperative;` declaration in the parent.

use std::collections::VecDeque;

#[cfg(test)]
use waml_markdown_editor::reading::BlockExtensionRequestId;

use super::{
    completed_event, MarkdownExtensionLeaseId, QueuedRender, RenderLedger, RenderScheduler,
};

#[derive(Default)]
pub(super) struct CooperativeScheduler {
    deferred: VecDeque<QueuedRender>,
}

impl RenderScheduler for CooperativeScheduler {
    fn submit(&mut self, _ledger: &mut RenderLedger, queued: QueuedRender) {
        self.deferred.push_back(queued);
    }

    /// Nothing lands asynchronously here — every completion is admitted inline
    /// by `run_one_deferred`, so there is nothing to harvest.
    fn poll(&mut self, _ledger: &mut RenderLedger) {}

    fn forget_lease(&mut self, lease: MarkdownExtensionLeaseId) {
        self.deferred.retain(|queued| queued.lease != lease);
    }

    fn has_deferred_work(&self) -> bool {
        !self.deferred.is_empty()
    }

    fn run_one_deferred(&mut self, ledger: &mut RenderLedger) -> bool {
        while let Some(queued) = self.deferred.pop_front() {
            let key = (queued.lease, queued.request.request_id);
            if !ledger.render_is_live(key, &queued.request) {
                if ledger.render_should_retire(key) {
                    ledger.retire(key);
                }
                continue;
            }

            let result = queued.renderer.render_and_cache(&queued.request);
            // The render just ran on the UI thread, but it can still have been
            // superseded by an edit that arrived before it started.
            if ledger.render_is_live(key, &queued.request) {
                ledger.admit_event(queued.lease, completed_event(&queued.request, result));
            } else if ledger.render_should_retire(key) {
                ledger.retire(key);
            }
            break;
        }
        !self.deferred.is_empty()
    }

    #[cfg(test)]
    fn active_count(&self) -> usize {
        0
    }

    #[cfg(test)]
    fn queued_request_ids(&self) -> Vec<BlockExtensionRequestId> {
        self.deferred
            .iter()
            .map(|queued| queued.request.request_id)
            .collect()
    }
}
