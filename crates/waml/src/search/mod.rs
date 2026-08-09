//! Bundle-wide search: tokenizer, query parser, field extraction, and the
//! in-memory `SearchIndex` backend. Surfaces depend only on the vocabulary
//! re-exported from this module, never on a backend's internals.

pub mod extract;
pub mod query;
pub mod tokenize;

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
