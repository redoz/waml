use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use waml_syntax::{DocumentRevision, TextRange};

use crate::presentation::PresentationItemId;

use super::{ReadingBlock, ReadingBlockKind, ReadingDocument};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BlockExtensionAppearance {
    Light,
    Dark,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FencedBlockExtension {
    pub id: PresentationItemId,
    pub language: Arc<str>,
    pub source_range: TextRange,
    pub content_range: TextRange,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegisteredBlockExtensions {
    languages: BTreeSet<Arc<str>>,
}

impl RegisteredBlockExtensions {
    pub fn from_languages(languages: impl IntoIterator<Item = Arc<str>>) -> Self {
        Self {
            languages: languages
                .into_iter()
                .filter(|language| language.is_ascii())
                .collect(),
        }
    }

    pub fn contains(&self, language: &str) -> bool {
        language.is_ascii()
            && self
                .languages
                .iter()
                .any(|registered| registered.eq_ignore_ascii_case(language))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockExtensionRequestId(pub u64);

#[derive(Clone, Debug)]
pub struct BlockExtensionRequest {
    pub request_id: BlockExtensionRequestId,
    pub revision: DocumentRevision,
    pub item: PresentationItemId,
    pub source_range: TextRange,
    pub content_range: TextRange,
    pub language: Arc<str>,
    pub content: Arc<str>,
    pub appearance: BlockExtensionAppearance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderedBlockSvg {
    pub data: Arc<[u8]>,
    pub logical_size: (f64, f64),
}

#[derive(Clone, Debug)]
pub enum BlockExtensionEvent {
    Ready {
        request_id: BlockExtensionRequestId,
        revision: DocumentRevision,
        item: PresentationItemId,
        source_range: TextRange,
        svg: RenderedBlockSvg,
    },
    Failed {
        request_id: BlockExtensionRequestId,
        revision: DocumentRevision,
        item: PresentationItemId,
        source_range: TextRange,
        message: Arc<str>,
    },
}

pub trait MarkdownBlockExtensionHost {
    fn request(&mut self, request: BlockExtensionRequest);
    fn cancel(&mut self, request_id: BlockExtensionRequestId);
    fn drain_events(&mut self) -> Vec<BlockExtensionEvent>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum BlockExtensionState {
    Loading,
    Ready(RenderedBlockSvg),
    Failed(Arc<str>),
}

#[derive(Clone, Debug)]
struct BlockExtensionEntry {
    request_id: BlockExtensionRequestId,
    source_range: TextRange,
    state: BlockExtensionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockExtensionEventOutcome {
    Applied,
    IgnoredStale,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockExtensionFrame {
    pub revision: DocumentRevision,
    pub items: Arc<[(PresentationItemId, BlockExtensionState)]>,
}

#[derive(Debug, Default)]
pub struct BlockExtensionStates {
    revision: Option<DocumentRevision>,
    entries: BTreeMap<PresentationItemId, BlockExtensionEntry>,
    next_request: u64,
}

impl BlockExtensionStates {
    pub fn reconcile(
        &mut self,
        host: &mut dyn MarkdownBlockExtensionHost,
        revision: DocumentRevision,
        document: &ReadingDocument,
        source: Arc<str>,
        appearance: BlockExtensionAppearance,
    ) {
        if self.revision != Some(revision) {
            for entry in self.entries.values() {
                host.cancel(entry.request_id);
            }
            self.entries.clear();
            self.revision = Some(revision);
        }

        let mut live = BTreeSet::new();
        for extension in fenced_extensions(&document.roots) {
            live.insert(extension.id);
            if self.entries.contains_key(&extension.id) {
                continue;
            }
            let request_id = self.allocate();
            let Some(content) = content_for(&source, extension.content_range) else {
                self.entries.insert(
                    extension.id,
                    BlockExtensionEntry {
                        request_id,
                        source_range: extension.source_range,
                        state: BlockExtensionState::Failed(Arc::from(
                            "invalid fenced extension content range",
                        )),
                    },
                );
                continue;
            };
            self.entries.insert(
                extension.id,
                BlockExtensionEntry {
                    request_id,
                    source_range: extension.source_range,
                    state: BlockExtensionState::Loading,
                },
            );
            host.request(BlockExtensionRequest {
                request_id,
                revision,
                item: extension.id,
                source_range: extension.source_range,
                content_range: extension.content_range,
                language: extension.language.clone(),
                content,
                appearance,
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
                host.cancel(entry.request_id);
            }
        }
    }

    pub fn apply_event(&mut self, event: BlockExtensionEvent) -> BlockExtensionEventOutcome {
        let (request_id, revision, item, source_range) = event_identity(&event);
        if self.revision != Some(revision) {
            return BlockExtensionEventOutcome::IgnoredStale;
        }
        let Some(entry) = self.entries.get_mut(&item) else {
            return BlockExtensionEventOutcome::IgnoredStale;
        };
        if entry.request_id != request_id
            || entry.source_range != source_range
            || !matches!(entry.state, BlockExtensionState::Loading)
        {
            return BlockExtensionEventOutcome::IgnoredStale;
        }
        entry.state = match event {
            BlockExtensionEvent::Ready { svg, .. } => BlockExtensionState::Ready(svg),
            BlockExtensionEvent::Failed { message, .. } => BlockExtensionState::Failed(message),
        };
        BlockExtensionEventOutcome::Applied
    }

    pub fn pending_count(&self) -> usize {
        self.entries
            .values()
            .filter(|entry| matches!(entry.state, BlockExtensionState::Loading))
            .count()
    }

    pub fn frame(&self, revision: DocumentRevision) -> Arc<BlockExtensionFrame> {
        let items = (self.revision == Some(revision))
            .then(|| {
                self.entries
                    .iter()
                    .map(|(id, entry)| (*id, entry.state.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Arc::new(BlockExtensionFrame {
            revision,
            items: items.into(),
        })
    }

    fn allocate(&mut self) -> BlockExtensionRequestId {
        self.next_request += 1;
        BlockExtensionRequestId(self.next_request)
    }
}

fn fenced_extensions(blocks: &[ReadingBlock]) -> Vec<&FencedBlockExtension> {
    let mut extensions = Vec::new();
    let mut stack = blocks.iter().collect::<Vec<_>>();
    while let Some(block) = stack.pop() {
        if let ReadingBlockKind::FencedExtension(extension) = &block.kind {
            extensions.push(extension);
        }
        stack.extend(block.children.iter());
    }
    extensions
}

fn content_for(source: &str, range: TextRange) -> Option<Arc<str>> {
    let start = range.start().to_usize();
    let end = range.end().to_usize();
    (start <= end
        && end <= source.len()
        && source.is_char_boundary(start)
        && source.is_char_boundary(end))
    .then(|| Arc::from(&source[start..end]))
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
