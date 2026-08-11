//! Bundle-wide search: tokenizer, query parser, field extraction, and the
//! in-memory `SearchIndex` backend. Surfaces depend only on the vocabulary
//! re-exported from this module, never on a backend's internals.

pub mod asset;
pub mod export;
pub mod extract;
pub mod index;
pub mod query;
pub mod tokenize;

pub use index::MemSearchIndex;

/// Ranking tier order is declaration order: Names > Model > Prose > Structure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FieldGroup {
    Names,
    Model,
    Prose,
    Structure,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum HitTarget {
    /// Byte span in the document raw source, with a 1-based line for display.
    TextSpan { start: u32, end: u32, line: u32 },
    /// A model element reference (concept id / classifier key).
    ModelElement { key: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub document: String, // bundle-relative path, e.g. "guides/checkout.md"
    pub concept_id: Option<String>,
    pub group: FieldGroup,
    pub target: HitTarget,
    /// Which of the document's extracted entries matched, as an index into
    /// [`extract::DocumentFields::entries`]. `target` does NOT identify one:
    /// extraction gives every Names/Model/Structure entry of a concept the
    /// same `HitTarget::ModelElement { key }`, so without this a snippet
    /// resolves to the first entry sharing the target and several matching
    /// entries collapse into indistinguishable rows.
    pub entry: u32,
    pub score: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueryScope {
    pub document: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snippet {
    pub text: String,
    pub highlights: Vec<(usize, usize)>,
}

/// The engine boundary (spec §Engine boundary). Surfaces depend on THIS,
/// never on a backend, so a native-only tantivy backend can slot in later
/// without touching any surface.
pub trait SearchIndex {
    fn update_document(&mut self, path: &str, fields: extract::DocumentFields);
    fn remove_document(&mut self, path: &str);
    fn query(&self, query: &str, scope: &QueryScope) -> Vec<Hit>;
    fn snippet(&self, hit: &Hit, width: usize) -> Snippet;
}
