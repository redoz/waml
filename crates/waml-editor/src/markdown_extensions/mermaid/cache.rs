use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, MutexGuard},
};

use waml_markdown_editor::reading::{
    BlockExtensionAppearance, BlockExtensionRequest, RenderedBlockSvg,
};

use super::BlockRenderResult;

pub(super) const CACHE_MAX_ENTRIES: usize = 64;
pub(super) const CACHE_MAX_SVG_BYTES: usize = 32 * 1024 * 1024;
const ADAPTER_SCHEMA_VERSION: u32 = 1;
const MERMAN_VERSION: &str = "0.8.0-alpha.5";

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct CacheKey {
    content: Arc<str>,
    appearance: BlockExtensionAppearance,
    adapter_schema: u32,
    merman_version: &'static str,
}

impl CacheKey {
    pub(super) fn from_request(request: &BlockExtensionRequest) -> Self {
        Self {
            content: request.content.clone(),
            appearance: request.appearance,
            adapter_schema: ADAPTER_SCHEMA_VERSION,
            merman_version: MERMAN_VERSION,
        }
    }

    pub(super) fn diagram_id(&self) -> String {
        format!("waml-mermaid-{:032x}", self.stable_digest())
    }

    fn stable_digest(&self) -> u128 {
        const FNV_OFFSET_BASIS: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
        const FNV_PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

        let mut digest = FNV_OFFSET_BASIS;
        update_digest(
            &mut digest,
            &(self.content.len() as u64).to_le_bytes(),
            FNV_PRIME,
        );
        update_digest(&mut digest, self.content.as_bytes(), FNV_PRIME);
        update_digest(
            &mut digest,
            &[match self.appearance {
                BlockExtensionAppearance::Light => 0,
                BlockExtensionAppearance::Dark => 1,
            }],
            FNV_PRIME,
        );
        update_digest(&mut digest, &self.adapter_schema.to_le_bytes(), FNV_PRIME);
        update_digest(
            &mut digest,
            &(self.merman_version.len() as u64).to_le_bytes(),
            FNV_PRIME,
        );
        update_digest(&mut digest, self.merman_version.as_bytes(), FNV_PRIME);
        digest
    }
}

fn update_digest(digest: &mut u128, bytes: &[u8], prime: u128) {
    for byte in bytes {
        *digest ^= u128::from(*byte);
        *digest = digest.wrapping_mul(prime);
    }
}

#[derive(Debug, Default)]
pub(super) struct MermaidCache {
    entries: HashMap<CacheKey, BlockRenderResult>,
    recency: VecDeque<CacheKey>,
    successful_svg_bytes: usize,
}

pub(super) fn lock_cache(cache: &Mutex<MermaidCache>) -> MutexGuard<'_, MermaidCache> {
    match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

impl MermaidCache {
    pub(super) fn get(&mut self, key: &CacheKey) -> Option<BlockRenderResult> {
        let result = self.entries.get(key)?.clone();
        self.remove_from_recency(key);
        self.recency.push_back(key.clone());
        Some(result)
    }

    pub(super) fn insert(&mut self, key: CacheKey, result: BlockRenderResult) {
        if let Some(previous) = self.entries.remove(&key) {
            self.successful_svg_bytes = self
                .successful_svg_bytes
                .saturating_sub(successful_svg_len(&previous));
            self.remove_from_recency(&key);
        }

        self.successful_svg_bytes = self
            .successful_svg_bytes
            .saturating_add(successful_svg_len(&result));
        self.entries.insert(key.clone(), result);
        self.recency.push_back(key);
        self.enforce_bounds();
    }

    fn remove_from_recency(&mut self, key: &CacheKey) {
        if let Some(index) = self.recency.iter().position(|candidate| candidate == key) {
            self.recency.remove(index);
        }
    }

    fn enforce_bounds(&mut self) {
        while self.entries.len() > CACHE_MAX_ENTRIES
            || self.successful_svg_bytes > CACHE_MAX_SVG_BYTES
        {
            let Some(oldest) = self.recency.pop_front() else {
                break;
            };
            if let Some(result) = self.entries.remove(&oldest) {
                self.successful_svg_bytes = self
                    .successful_svg_bytes
                    .saturating_sub(successful_svg_len(&result));
            }
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn successful_svg_bytes(&self) -> usize {
        self.successful_svg_bytes
    }
}

fn successful_svg_len(result: &BlockRenderResult) -> usize {
    result
        .as_ref()
        .map(|rendered: &RenderedBlockSvg| rendered.data.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        thread,
    };

    use waml_markdown_editor::{
        presentation::{EmbeddedBlockRole, PresentationItemId, PresentationRole},
        reading::{
            BlockExtensionAppearance, BlockExtensionRequest, BlockExtensionRequestId,
            RenderedBlockSvg,
        },
        syntax::{DocumentRevision, TextRange, TextSize},
    };
    use waml_syntax::SyntaxIdentity;

    use super::{lock_cache, CacheKey, MermaidCache, CACHE_MAX_ENTRIES, CACHE_MAX_SVG_BYTES};

    fn request(content: &str, appearance: BlockExtensionAppearance) -> BlockExtensionRequest {
        BlockExtensionRequest {
            request_id: BlockExtensionRequestId(7),
            revision: DocumentRevision::new(11),
            item: PresentationItemId {
                owner: SyntaxIdentity::from_raw_for_test(13),
                role: PresentationRole::Embedded(EmbeddedBlockRole::FencedExtension),
                fragment_ordinal: 2,
            },
            source_range: TextRange::new(TextSize::new(0), TextSize::new(0)).unwrap(),
            content_range: TextRange::new(TextSize::new(0), TextSize::new(0)).unwrap(),
            language: Arc::from("mermaid"),
            content: Arc::from(content),
            appearance,
        }
    }

    fn rendered(byte_len: usize) -> Result<RenderedBlockSvg, Arc<str>> {
        Ok(RenderedBlockSvg {
            data: vec![b'x'; byte_len].into(),
            logical_size: (10.0, 10.0),
        })
    }

    #[test]
    fn cache_key_ignores_request_and_document_identity() {
        let first = request("flowchart TD\nA-->B", BlockExtensionAppearance::Light);
        let mut second = first.clone();
        second.request_id = BlockExtensionRequestId(99);
        second.revision = DocumentRevision::new(42);
        second.item.owner = SyntaxIdentity::from_raw_for_test(77);
        second.item.fragment_ordinal = 8;
        second.source_range = TextRange::new(TextSize::new(5), TextSize::new(9)).unwrap();

        assert_eq!(
            CacheKey::from_request(&first),
            CacheKey::from_request(&second)
        );
    }

    #[test]
    fn cache_key_separates_light_and_dark_appearance() {
        let light = request("flowchart TD\nA-->B", BlockExtensionAppearance::Light);
        let dark = request("flowchart TD\nA-->B", BlockExtensionAppearance::Dark);

        assert_ne!(
            CacheKey::from_request(&light),
            CacheKey::from_request(&dark)
        );
    }

    #[test]
    fn diagram_id_is_stable_for_the_full_cache_key() {
        let first = request("flowchart TD\nA-->B", BlockExtensionAppearance::Light);
        let mut same_key = first.clone();
        same_key.item.owner = SyntaxIdentity::from_raw_for_test(99);
        same_key.item.fragment_ordinal = 31;
        let different_source = request("flowchart TD\nA-->C", BlockExtensionAppearance::Light);
        let different_theme = request("flowchart TD\nA-->B", BlockExtensionAppearance::Dark);

        let first_key = CacheKey::from_request(&first);
        let first_id = first_key.diagram_id();
        assert_eq!(first_id, CacheKey::from_request(&same_key).diagram_id());
        assert_ne!(
            first_id,
            CacheKey::from_request(&different_source).diagram_id()
        );
        assert_ne!(
            first_id,
            CacheKey::from_request(&different_theme).diagram_id()
        );

        let mut different_schema = first_key.clone();
        different_schema.adapter_schema += 1;
        assert_ne!(first_id, different_schema.diagram_id());

        let mut different_renderer = first_key;
        different_renderer.merman_version = "different-renderer";
        assert_ne!(first_id, different_renderer.diagram_id());
    }

    #[test]
    fn cache_stores_deterministic_failures() {
        let request = request("not a diagram", BlockExtensionAppearance::Light);
        let key = CacheKey::from_request(&request);
        let expected = Err(Arc::<str>::from("diagram type was not detected"));
        let mut cache = MermaidCache::default();

        cache.insert(key.clone(), expected.clone());

        assert_eq!(cache.get(&key), Some(expected));
    }

    #[test]
    fn sixty_fifth_entry_evicts_the_oldest_entry() {
        let mut cache = MermaidCache::default();
        let oldest = CacheKey::from_request(&request("entry-0", BlockExtensionAppearance::Light));
        for index in 0..=CACHE_MAX_ENTRIES {
            let key = CacheKey::from_request(&request(
                &format!("entry-{index}"),
                BlockExtensionAppearance::Light,
            ));
            cache.insert(key, rendered(1));
        }

        assert_eq!(cache.len(), CACHE_MAX_ENTRIES);
        assert!(cache.get(&oldest).is_none());
    }

    #[test]
    fn access_refreshes_recency_before_count_eviction() {
        let mut cache = MermaidCache::default();
        let oldest = CacheKey::from_request(&request("entry-0", BlockExtensionAppearance::Light));
        for index in 0..CACHE_MAX_ENTRIES {
            let key = CacheKey::from_request(&request(
                &format!("entry-{index}"),
                BlockExtensionAppearance::Light,
            ));
            cache.insert(key, rendered(1));
        }
        assert!(cache.get(&oldest).is_some());
        let next = CacheKey::from_request(&request("entry-64", BlockExtensionAppearance::Light));

        cache.insert(next, rendered(1));

        assert!(cache.get(&oldest).is_some());
        let evicted = CacheKey::from_request(&request("entry-1", BlockExtensionAppearance::Light));
        assert!(cache.get(&evicted).is_none());
    }

    #[test]
    fn byte_pressure_evicts_until_the_bound_holds() {
        let mut cache = MermaidCache::default();
        let entry_bytes = 8 * 1024 * 1024;
        for index in 0..5 {
            let key = CacheKey::from_request(&request(
                &format!("large-{index}"),
                BlockExtensionAppearance::Dark,
            ));
            cache.insert(key, rendered(entry_bytes));
        }

        assert!(cache.successful_svg_bytes() <= CACHE_MAX_SVG_BYTES);
        assert_eq!(cache.len(), 4);
        let oldest = CacheKey::from_request(&request("large-0", BlockExtensionAppearance::Dark));
        assert!(cache.get(&oldest).is_none());
    }

    #[test]
    fn replacement_does_not_double_count_svg_bytes() {
        let mut cache = MermaidCache::default();
        let key = CacheKey::from_request(&request("same", BlockExtensionAppearance::Light));

        cache.insert(key.clone(), rendered(12));
        cache.insert(key, rendered(20));

        assert_eq!(cache.len(), 1);
        assert_eq!(cache.successful_svg_bytes(), 20);
    }

    #[test]
    fn poisoned_mutex_recovers_the_existing_cache_and_accepts_later_entries() {
        let cache = Arc::new(Mutex::new(MermaidCache::default()));
        let poisoned = cache.clone();
        let first = CacheKey::from_request(&request("first", BlockExtensionAppearance::Light));
        let first_for_worker = first.clone();
        let worker = thread::spawn(move || {
            let mut guard = poisoned.lock().unwrap();
            guard.insert(first_for_worker, rendered(4));
            panic!("poison the Mermaid cache")
        });
        assert!(worker.join().is_err());
        let second = CacheKey::from_request(&request("second", BlockExtensionAppearance::Dark));

        let mut recovered = lock_cache(&cache);
        assert!(recovered.get(&first).is_some());
        recovered.insert(second.clone(), rendered(8));
        assert!(recovered.get(&second).is_some());
    }
}
