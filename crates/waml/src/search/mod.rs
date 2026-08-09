//! Bundle-wide search: tokenizer, query parser, field extraction, and the
//! in-memory `SearchIndex` backend. Surfaces depend only on the vocabulary
//! re-exported from this module, never on a backend's internals.

pub mod query;
pub mod tokenize;
