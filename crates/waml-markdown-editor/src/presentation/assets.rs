//! Revision-bound embedded image state.
//!
//! Image resolution is application-authorized, asynchronous, and revision
//! bound. This module owns only the state machine: it asks an application host
//! for approved sources, ignores every stale completion, and reports embedded
//! sizes to layout. It never fetches, decodes, or reads a file.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use waml_syntax::{DocumentRevision, TextRange};

use crate::layout::{LayoutElementId, LayoutInvalidation, MeasuredBlock};

use super::{
    layout::EmbeddedMeasurements, EmbeddedBlockKind, PresentationItem, PresentationItemId,
    PresentationPlan,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AssetRequestId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageAssetRequest {
    pub request_id: AssetRequestId,
    pub revision: DocumentRevision,
    pub item: PresentationItemId,
    pub source_range: TextRange,
    pub destination: Arc<str>,
}

/// The application's asset authority. There is deliberately no method that
/// takes a URL to fetch: a remote host must return approved bytes.
pub trait MarkdownAssetHost {
    fn request_image(&mut self, request: ImageAssetRequest);
    fn cancel_image(&mut self, request_id: AssetRequestId);
    fn drain_events(&mut self) -> Vec<ImageAssetEvent>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageMediaType {
    Svg,
    Png,
    Jpeg,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApprovedImageSource {
    Bytes {
        cache_key: Arc<str>,
        media_type: ImageMediaType,
        data: Arc<[u8]>,
        pixel_size: (u32, u32),
    },
    CanonicalFile {
        path: Arc<PathBuf>,
        pixel_size: (u32, u32),
    },
}

impl ApprovedImageSource {
    pub fn pixel_size(&self) -> (u32, u32) {
        match self {
            Self::Bytes { pixel_size, .. } | Self::CanonicalFile { pixel_size, .. } => *pixel_size,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageAssetEvent {
    Ready {
        request_id: AssetRequestId,
        revision: DocumentRevision,
        item: PresentationItemId,
        source: ApprovedImageSource,
    },
    Failed {
        request_id: AssetRequestId,
        revision: DocumentRevision,
        item: PresentationItemId,
        message: Arc<str>,
    },
}

impl ImageAssetEvent {
    fn request_id(&self) -> AssetRequestId {
        match self {
            Self::Ready { request_id, .. } | Self::Failed { request_id, .. } => *request_id,
        }
    }

    fn revision(&self) -> DocumentRevision {
        match self {
            Self::Ready { revision, .. } | Self::Failed { revision, .. } => *revision,
        }
    }

    fn item(&self) -> PresentationItemId {
        match self {
            Self::Ready { item, .. } | Self::Failed { item, .. } => *item,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbeddedState {
    Loading,
    Ready { source: ApprovedImageSource },
    Failed { message: Arc<str> },
}

#[derive(Clone, Debug)]
pub enum AssetEventOutcome {
    Applied {
        invalidation: Option<LayoutInvalidation>,
    },
    IgnoredStale,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmbeddedAssetFrame {
    pub revision: DocumentRevision,
    pub items: Arc<[(PresentationItemId, EmbeddedState)]>,
}

/// Placeholder and clamp geometry, in logical pixels.
pub const LOADING_MAX_WIDTH: f64 = 240.0;
pub const LOADING_HEIGHT: f64 = 72.0;
pub const FAILED_MAX_WIDTH: f64 = 320.0;
pub const FAILED_HEIGHT: f64 = 48.0;
pub const MAX_IMAGE_HEIGHT: f64 = 480.0;

#[derive(Clone, Debug)]
struct Entry {
    request_id: AssetRequestId,
    source_range: TextRange,
    state: EmbeddedState,
}

/// Per-revision embedded image state, keyed by `(revision, item)`.
#[derive(Debug, Default)]
pub struct EmbeddedAssets {
    revision: Option<DocumentRevision>,
    entries: BTreeMap<PresentationItemId, Entry>,
    next_request: u64,
}

impl EmbeddedAssets {
    /// Requests every parsed image of `plan` exactly once and cancels requests
    /// whose item no longer exists. A new revision restarts every request,
    /// because approvals are bound to one revision.
    pub fn reconcile(&mut self, host: &mut dyn MarkdownAssetHost, plan: &PresentationPlan) {
        if self.revision != Some(plan.revision) {
            for entry in self.entries.values() {
                host.cancel_image(entry.request_id);
            }
            self.entries.clear();
            self.revision = Some(plan.revision);
        }
        let mut live = Vec::new();
        for item in plan.items.iter() {
            let PresentationItem::EmbeddedBlock {
                id,
                source_range,
                kind: EmbeddedBlockKind::Image { destination, .. },
                ..
            } = item
            else {
                continue;
            };
            live.push(*id);
            if self.entries.contains_key(id) {
                continue;
            }
            let request_id = self.allocate();
            self.entries.insert(
                *id,
                Entry {
                    request_id,
                    source_range: *source_range,
                    state: EmbeddedState::Loading,
                },
            );
            host.request_image(ImageAssetRequest {
                request_id,
                revision: plan.revision,
                item: *id,
                source_range: *source_range,
                destination: destination.clone(),
            });
        }
        let removed = self
            .entries
            .keys()
            .filter(|id| !live.contains(id))
            .copied()
            .collect::<Vec<_>>();
        for id in removed {
            if let Some(entry) = self.entries.remove(&id) {
                host.cancel_image(entry.request_id);
            }
        }
    }

    /// Re-requests one failed item. Loading and ready items are left alone, so
    /// activating them cannot spam the host.
    pub fn retry(
        &mut self,
        host: &mut dyn MarkdownAssetHost,
        plan: &PresentationPlan,
        item: PresentationItemId,
    ) -> bool {
        if self.revision != Some(plan.revision) {
            return false;
        }
        let Some(entry) = self.entries.get(&item) else {
            return false;
        };
        if !matches!(entry.state, EmbeddedState::Failed { .. }) {
            return false;
        }
        let Some(destination) = plan.items.iter().find_map(|candidate| match candidate {
            PresentationItem::EmbeddedBlock {
                id,
                kind: EmbeddedBlockKind::Image { destination, .. },
                ..
            } if *id == item => Some(destination.clone()),
            _ => None,
        }) else {
            return false;
        };
        let source_range = entry.source_range;
        let request_id = self.allocate();
        self.entries.insert(
            item,
            Entry {
                request_id,
                source_range,
                state: EmbeddedState::Loading,
            },
        );
        host.request_image(ImageAssetRequest {
            request_id,
            revision: plan.revision,
            item,
            source_range,
            destination,
        });
        true
    }

    /// Applies one host completion. Request id, revision, and item must all
    /// match the pending entry; anything else is stale and changes nothing.
    pub fn apply_event(&mut self, event: ImageAssetEvent) -> AssetEventOutcome {
        if self.revision != Some(event.revision()) {
            return AssetEventOutcome::IgnoredStale;
        }
        let item = event.item();
        let Some(entry) = self.entries.get_mut(&item) else {
            return AssetEventOutcome::IgnoredStale;
        };
        if entry.request_id != event.request_id() {
            return AssetEventOutcome::IgnoredStale;
        }
        let previous = entry.state.clone();
        entry.state = match event {
            ImageAssetEvent::Ready { source, .. } => EmbeddedState::Ready { source },
            ImageAssetEvent::Failed { message, .. } => EmbeddedState::Failed { message },
        };
        let invalidation = (previous != entry.state).then_some(
            LayoutInvalidation::BlockMeasurement(LayoutElementId {
                owner: item.owner,
                fragment_ordinal: item.fragment_ordinal,
            }),
        );
        AssetEventOutcome::Applied { invalidation }
    }

    /// Embedded sizes for the current states, clamped to the content width.
    pub fn measurements(&self, available_width: f64) -> EmbeddedMeasurements {
        let blocks = self
            .entries
            .iter()
            .map(|(item, entry)| MeasuredBlock {
                id: LayoutElementId {
                    owner: item.owner,
                    fragment_ordinal: item.fragment_ordinal,
                },
                source_range: entry.source_range,
                size: measured_size(&entry.state, available_width),
                baseline: None,
            })
            .collect::<Vec<_>>();
        EmbeddedMeasurements {
            revision: self.revision,
            blocks: blocks.into(),
        }
    }

    /// The immutable per-frame view drawing consumes.
    pub fn frame(&self, plan: &PresentationPlan) -> Arc<EmbeddedAssetFrame> {
        let items = plan
            .items
            .iter()
            .filter_map(|item| match item {
                PresentationItem::EmbeddedBlock { id, .. } => Some(*id),
                _ => None,
            })
            .map(|id| {
                let state = self
                    .entries
                    .get(&id)
                    .map_or(EmbeddedState::Loading, |entry| entry.state.clone());
                (id, state)
            })
            .collect::<Vec<_>>();
        Arc::new(EmbeddedAssetFrame {
            revision: plan.revision,
            items: items.into(),
        })
    }

    /// Whether the outcome changed state, for tests and callers that only need
    /// to know if a relayout is owed.
    pub fn state(&self, item: PresentationItemId) -> Option<&EmbeddedState> {
        self.entries.get(&item).map(|entry| &entry.state)
    }

    fn allocate(&mut self) -> AssetRequestId {
        self.next_request += 1;
        AssetRequestId(self.next_request)
    }
}

/// Placeholder sizes are fixed; a ready image keeps its aspect ratio inside the
/// available width and the 480-pixel height clamp.
fn measured_size(state: &EmbeddedState, available_width: f64) -> makepad_widgets::DVec2 {
    use makepad_widgets::dvec2;
    match state {
        EmbeddedState::Loading => dvec2(available_width.min(LOADING_MAX_WIDTH), LOADING_HEIGHT),
        EmbeddedState::Failed { .. } => dvec2(available_width.min(FAILED_MAX_WIDTH), FAILED_HEIGHT),
        EmbeddedState::Ready { source } => {
            let (width, height) = source.pixel_size();
            let (width, height) = (width.max(1) as f64, height.max(1) as f64);
            let fit = (available_width / width).min(MAX_IMAGE_HEIGHT / height);
            let scale = fit.clamp(0.0, 1.0);
            dvec2(width * scale, height * scale)
        }
    }
}
