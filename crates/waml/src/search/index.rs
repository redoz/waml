//! `MemSearchIndex`: the in-memory `SearchIndex` backend. Builds an inverted
//! index over the [`extract::FieldEntry`] text of every document, then
//! answers queries with an AND-across-terms, BM25-ranked, tier-first sort
//! (spec: names before model before prose before structure, always).
//!
//! Granularity: postings are per-entry (one heading, one sentence, one
//! relationship line, …), never per-document — a `Hit` always points at the
//! exact entry that matched. The AND across query terms is evaluated at
//! document granularity (a document qualifies once every term appears
//! *somewhere* in it), then every entry in a qualifying document that
//! actually carries at least one matched term becomes its own scored hit.

use std::collections::{HashMap, HashSet};

use super::extract::DocumentFields;
use super::query::{parse_query, QueryFilter};
use super::tokenize::tokenize;
use super::{FieldGroup, Hit, HitTarget, QueryScope, SearchIndex, Snippet};

const K1: f32 = 1.2;
const B: f32 = 0.75;

struct EntryMeta {
    doc: u32,
    entry_idx: u32,
    group: FieldGroup,
    len: u32,
}

struct Posting {
    entry_id: u32,
    tf: u32,
}

/// The in-memory `SearchIndex` backend (spec §Engine boundary). Nothing
/// outside this module names `MemSearchIndex`'s fields — surfaces depend
/// only on the `SearchIndex` trait.
pub struct MemSearchIndex {
    docs: Vec<DocumentFields>,
    doc_index: HashMap<String, usize>,
    entries: Vec<EntryMeta>,
    postings: HashMap<String, Vec<Posting>>,
    tokens_sorted: Vec<String>,
    avg_len: HashMap<FieldGroup, f32>,
}

impl MemSearchIndex {
    /// Builds a fresh index from `documents`. Also the empty-index
    /// constructor (`build(std::iter::empty())` / `build(None)`).
    pub fn build(documents: impl IntoIterator<Item = DocumentFields>) -> Self {
        let mut index = Self {
            docs: documents.into_iter().collect(),
            doc_index: HashMap::new(),
            entries: Vec::new(),
            postings: HashMap::new(),
            tokens_sorted: Vec::new(),
            avg_len: HashMap::new(),
        };
        index.reindex();
        index
    }

    fn reindex(&mut self) {
        self.doc_index.clear();
        self.entries.clear();
        self.postings.clear();
        self.tokens_sorted.clear();
        self.avg_len.clear();

        let mut group_len_sum: HashMap<FieldGroup, u64> = HashMap::new();
        let mut group_len_count: HashMap<FieldGroup, u64> = HashMap::new();

        for (doc_idx, doc) in self.docs.iter().enumerate() {
            self.doc_index.insert(doc.path.clone(), doc_idx);
            for (entry_idx, entry) in doc.entries.iter().enumerate() {
                let tokens = tokenize(&entry.text);
                let len = tokens.len() as u32;
                *group_len_sum.entry(entry.group).or_insert(0) += u64::from(len);
                *group_len_count.entry(entry.group).or_insert(0) += 1;

                let entry_id = self.entries.len() as u32;
                self.entries.push(EntryMeta {
                    doc: doc_idx as u32,
                    entry_idx: entry_idx as u32,
                    group: entry.group,
                    len,
                });

                let mut tf_map: HashMap<String, u32> = HashMap::new();
                for token in tokens {
                    *tf_map.entry(token.text).or_insert(0) += 1;
                }
                for (text, tf) in tf_map {
                    self.postings
                        .entry(text)
                        .or_default()
                        .push(Posting { entry_id, tf });
                }
            }
        }

        for (group, sum) in &group_len_sum {
            let count = group_len_count.get(group).copied().unwrap_or(1).max(1);
            self.avg_len
                .insert(*group, (*sum as f32 / count as f32).max(1.0));
        }

        self.tokens_sorted = self.postings.keys().cloned().collect();
        self.tokens_sorted.sort();
    }

    /// The source documents this index was built from, in build order.
    /// `pub(crate)` for `asset.rs`'s encoder -- nothing outside this crate
    /// (and nothing outside `search`, in spirit) should reach past the
    /// `SearchIndex` trait to see these.
    pub(crate) fn documents(&self) -> &[DocumentFields] {
        &self.docs
    }

    /// Every indexed token starting with `prefix`, via a sorted-slice range
    /// scan (`partition_point`, no linear scan of the vocabulary).
    fn matching_tokens(&self, prefix: &str) -> &[String] {
        let start = self.tokens_sorted.partition_point(|t| t.as_str() < prefix);
        let end =
            start + self.tokens_sorted[start..].partition_point(|t| t.as_str().starts_with(prefix));
        &self.tokens_sorted[start..end]
    }

    fn idf(&self, token: &str) -> f32 {
        let n = self.entries.len().max(1) as f32;
        let df = self.postings.get(token).map_or(0, Vec::len) as f32;
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    fn bm25_term_component(tf: u32, len: u32, avg_len: f32) -> f32 {
        let tf = tf as f32;
        let len = len as f32;
        (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * (len / avg_len.max(1.0))))
    }

    /// Every document that has at least one entry carrying a token matching
    /// `term_text` as a prefix.
    fn doc_ids_matching_term(&self, term_text: &str) -> HashSet<u32> {
        let mut docs = HashSet::new();
        for token in self.matching_tokens(term_text) {
            if let Some(postings) = self.postings.get(token) {
                for posting in postings {
                    docs.insert(self.entries[posting.entry_id as usize].doc);
                }
            }
        }
        docs
    }

    fn eligible_docs(&self, filters: &[QueryFilter], scope: &QueryScope) -> HashSet<u32> {
        let mut eligible: HashSet<u32> = (0..self.docs.len() as u32).collect();
        for filter in filters {
            match filter {
                QueryFilter::Kind(kind) => {
                    eligible.retain(|&doc_id| {
                        self.docs[doc_id as usize].kind.as_deref() == Some(kind.as_str())
                    });
                }
                QueryFilter::InPath(prefix) => {
                    eligible.retain(|&doc_id| {
                        self.docs[doc_id as usize].path.starts_with(prefix.as_str())
                    });
                }
            }
        }
        if let Some(document) = &scope.document {
            eligible.retain(|&doc_id| &self.docs[doc_id as usize].path == document);
        }
        eligible
    }

    fn hit_for_entry(&self, entry_id: u32, score: f32) -> Hit {
        let meta = &self.entries[entry_id as usize];
        let doc = &self.docs[meta.doc as usize];
        let entry = &doc.entries[meta.entry_idx as usize];
        Hit {
            document: doc.path.clone(),
            concept_id: doc.concept_id.clone(),
            group: entry.group,
            target: entry.target.clone(),
            score,
        }
    }
}

/// Sort key used to break ties on `(document, target)` deterministically —
/// `HitTarget` carries no `Ord` impl itself, so this maps it onto one.
fn target_sort_key(target: &HitTarget) -> (u8, u32, u32, &str) {
    match target {
        HitTarget::TextSpan { start, end, .. } => (0, *start, *end, ""),
        HitTarget::ModelElement { key } => (1, 0, 0, key.as_str()),
    }
}

impl SearchIndex for MemSearchIndex {
    fn update_document(&mut self, path: &str, fields: DocumentFields) {
        if let Some(&pos) = self.doc_index.get(path) {
            self.docs[pos] = fields;
        } else {
            self.docs.push(fields);
        }
        self.reindex();
    }

    fn remove_document(&mut self, path: &str) {
        self.docs.retain(|doc| doc.path != path);
        self.reindex();
    }

    fn query(&self, query: &str, scope: &QueryScope) -> Vec<Hit> {
        let parsed = parse_query(query);
        if parsed.is_empty() {
            return Vec::new();
        }

        let eligible = self.eligible_docs(&parsed.filters, scope);
        if eligible.is_empty() {
            return Vec::new();
        }

        let mut hits: Vec<Hit> = if parsed.terms.is_empty() {
            // Filter-only query (e.g. `kind:actor`): every entry in an
            // eligible document is a hit, unscored (tier order still
            // applies in the final sort).
            self.entries
                .iter()
                .enumerate()
                .filter(|(_, meta)| eligible.contains(&meta.doc))
                .map(|(entry_id, _)| self.hit_for_entry(entry_id as u32, 0.0))
                .collect()
        } else {
            let mut term_eligible = eligible.clone();
            for term in &parsed.terms {
                let matching = self.doc_ids_matching_term(&term.text);
                term_eligible.retain(|doc_id| matching.contains(doc_id));
            }
            if term_eligible.is_empty() {
                return Vec::new();
            }

            let mut scores: HashMap<u32, f32> = HashMap::new();
            for term in &parsed.terms {
                for token in self.matching_tokens(&term.text) {
                    let idf = self.idf(token);
                    let boost = if token == &term.text { 2.0 } else { 1.0 };
                    let Some(postings) = self.postings.get(token) else {
                        continue;
                    };
                    for posting in postings {
                        let meta = &self.entries[posting.entry_id as usize];
                        if !term_eligible.contains(&meta.doc) {
                            continue;
                        }
                        let avg_len = self.avg_len.get(&meta.group).copied().unwrap_or(1.0);
                        let contribution =
                            idf * boost * Self::bm25_term_component(posting.tf, meta.len, avg_len);
                        *scores.entry(posting.entry_id).or_insert(0.0) += contribution;
                    }
                }
            }

            scores
                .into_iter()
                .filter(|(_, score)| *score > 0.0)
                .map(|(entry_id, score)| self.hit_for_entry(entry_id, score))
                .collect()
        };

        hits.sort_by(|a, b| {
            a.group
                .cmp(&b.group)
                .then_with(|| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.document.cmp(&b.document))
                .then_with(|| target_sort_key(&a.target).cmp(&target_sort_key(&b.target)))
        });

        hits
    }

    fn snippet(&self, hit: &Hit, width: usize) -> Snippet {
        let Some(&doc_idx) = self.doc_index.get(&hit.document) else {
            return Snippet {
                text: String::new(),
                highlights: Vec::new(),
            };
        };
        let doc = &self.docs[doc_idx];
        let Some(entry) = doc.entries.iter().find(|entry| entry.target == hit.target) else {
            return Snippet {
                text: String::new(),
                highlights: Vec::new(),
            };
        };
        build_snippet(&entry.text, width)
    }
}

/// Windows `text` to at most `width` chars (never splitting a char), centred
/// on the midpoint, and highlights the whole window — the underlying entry
/// text already IS the matched span (extraction stores span-scoped text), so
/// there is one highlight covering everything shown.
fn build_snippet(text: &str, width: usize) -> Snippet {
    let chars: Vec<char> = text.chars().collect();
    if width == 0 || chars.len() <= width {
        let len = chars.len();
        return Snippet {
            text: text.to_string(),
            highlights: if len == 0 { Vec::new() } else { vec![(0, len)] },
        };
    }

    let mid = chars.len() / 2;
    let half = width / 2;
    let start = mid.saturating_sub(half).min(chars.len() - width);
    let end = start + width;
    let windowed: String = chars[start..end].iter().collect();
    Snippet {
        text: windowed,
        highlights: vec![(0, width)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::extract::FieldEntry;

    fn doc(
        path: &str,
        kind: Option<&str>,
        entries: Vec<(FieldGroup, &str, HitTarget)>,
    ) -> DocumentFields {
        DocumentFields {
            path: path.to_string(),
            concept_id: Some(path.to_string()),
            title: path.to_string(),
            kind: kind.map(str::to_string),
            entries: entries
                .into_iter()
                .map(|(group, text, target)| FieldEntry {
                    group,
                    text: text.to_string(),
                    target,
                })
                .collect(),
        }
    }

    fn text_span(start: u32, end: u32) -> HitTarget {
        HitTarget::TextSpan {
            start,
            end,
            line: 1,
        }
    }

    fn model(key: &str) -> HitTarget {
        HitTarget::ModelElement { key: key.into() }
    }

    fn fixture() -> MemSearchIndex {
        MemSearchIndex::build(vec![
            doc(
                "payment.md",
                Some("class"),
                vec![
                    (FieldGroup::Names, "Payment", model("payment")),
                    (
                        FieldGroup::Prose,
                        "A payment holds funds.",
                        text_span(0, 23),
                    ),
                ],
            ),
            doc(
                "notes.md",
                None,
                vec![(
                    FieldGroup::Prose,
                    "See the payment capture flow for details.",
                    text_span(0, 43),
                )],
            ),
            doc(
                "unrelated.md",
                Some("actor"),
                vec![(FieldGroup::Names, "Customer", model("customer"))],
            ),
        ])
    }

    #[test]
    fn a_name_hit_outranks_a_prose_hit_for_the_same_term() {
        let index = fixture();
        let hits = index.query("payment", &QueryScope::default());
        assert!(!hits.is_empty());
        assert_eq!(hits[0].group, FieldGroup::Names);
        assert_eq!(hits[0].document, "payment.md");
        assert!(hits.iter().any(|h| h.group == FieldGroup::Prose));
    }

    #[test]
    fn prefix_matches_as_you_type() {
        let index = fixture();
        let hits = index.query("paym", &QueryScope::default());
        assert!(hits.iter().any(|h| h.document == "payment.md"));
    }

    #[test]
    fn terms_are_anded() {
        let index = fixture();
        let hits = index.query("payment capture", &QueryScope::default());
        let docs: HashSet<&str> = hits.iter().map(|h| h.document.as_str()).collect();
        // Only notes.md has both "payment" and "capture" in its prose; even
        // though payment.md has "payment", it never says "capture".
        assert_eq!(docs, HashSet::from(["notes.md"]));
    }

    #[test]
    fn kind_filter_restricts_and_in_filter_scopes_by_path_prefix() {
        let index = fixture();

        let by_kind = index.query("kind:actor", &QueryScope::default());
        assert!(!by_kind.is_empty());
        assert!(by_kind.iter().all(|h| h.document == "unrelated.md"));

        let by_path = index.query("in:payment", &QueryScope::default());
        assert!(!by_path.is_empty());
        assert!(by_path.iter().all(|h| h.document == "payment.md"));
    }

    #[test]
    fn scope_restricts_to_one_document() {
        let index = fixture();
        let scope = QueryScope {
            document: Some("notes.md".to_string()),
        };
        let hits = index.query("payment", &scope);
        assert!(!hits.is_empty());
        assert!(hits.iter().all(|h| h.document == "notes.md"));
    }

    #[test]
    fn update_document_replaces_and_remove_document_forgets() {
        let mut index = fixture();
        index.update_document(
            "payment.md",
            doc(
                "payment.md",
                Some("class"),
                vec![(FieldGroup::Names, "Refund", model("payment"))],
            ),
        );
        let hits = index.query("payment", &QueryScope::default());
        assert!(hits.iter().all(|h| h.document != "payment.md"));
        assert!(!index.query("refund", &QueryScope::default()).is_empty());

        index.remove_document("payment.md");
        assert!(index.query("refund", &QueryScope::default()).is_empty());
    }

    #[test]
    fn snippet_windows_the_span_on_char_boundaries_with_highlight_ranges() {
        let index = fixture();
        let hits = index.query("capture", &QueryScope::default());
        let hit = hits
            .iter()
            .find(|h| h.document == "notes.md")
            .expect("notes.md should match capture");

        let full = index.snippet(hit, 100);
        assert_eq!(full.text, "See the payment capture flow for details.");
        assert_eq!(full.highlights, vec![(0, full.text.chars().count())]);

        let windowed = index.snippet(hit, 10);
        assert_eq!(windowed.text.chars().count(), 10);
        assert_eq!(windowed.highlights, vec![(0, 10)]);
        // Never splits a multi-byte char: re-encoding the windowed chars
        // must round-trip through String without loss.
        assert_eq!(windowed.text.chars().collect::<Vec<_>>().len(), 10);
    }

    #[test]
    fn empty_queries_return_empty_not_panic() {
        let index = fixture();
        assert!(index.query("", &QueryScope::default()).is_empty());
        assert!(index.query("   ", &QueryScope::default()).is_empty());
    }
}
