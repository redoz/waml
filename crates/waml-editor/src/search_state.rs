//! Editor-side index lifecycle: build the bundle's text index on open,
//! refresh a single document on save. `App` owns one `SearchState`; every
//! search surface (results tab, palette, find strip) reads it through
//! `query`/`snippet` and never sees the backend (`MemSearchIndex`) directly
//! (spec §Engine boundary).

use waml::analysis::OkfAnalysis;
use waml::search::extract::extract_bundle;
use waml::search::{Hit, MemSearchIndex, QueryScope, SearchIndex, Snippet};
use waml::source::SourceBundle;

/// Whether the index reflects the live bundle yet. v1 builds synchronously
/// (single-digit ms at current bundle size), so `rebuild`/`refresh_document`
/// always leave `status` at `Ready`; `Building` exists for the palette's
/// `indexing…` row and a later async backend (decision 6).
///
/// `Building` is unconstructed until the palette (spec Task 10) renders it --
/// this lib-crate seam lands now so the surface tasks that consume it need no
/// further `SearchState` changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIndexStatus {
    #[allow(dead_code)]
    Building,
    Ready,
}

pub struct SearchState {
    index: MemSearchIndex,
    status: TextIndexStatus,
}

impl SearchState {
    pub fn empty() -> Self {
        SearchState {
            index: MemSearchIndex::build(std::iter::empty()),
            status: TextIndexStatus::Ready,
        }
    }

    /// Full rebuild from the live session (bundle open / session replace).
    pub fn rebuild(&mut self, source: &SourceBundle, okf: &OkfAnalysis, uml: &waml::uml::Analysis) {
        self.index = MemSearchIndex::build(extract_bundle(source, okf, uml));
        self.status = TextIndexStatus::Ready;
    }

    /// Per-document refresh (document save), spec §Index lifecycle. Recomputes
    /// fields for the whole bundle (cheap, decision 6) but only applies the
    /// entry for `path` -- every other document's postings are untouched.
    pub fn refresh_document(
        &mut self,
        path: &str,
        source: &SourceBundle,
        okf: &OkfAnalysis,
        uml: &waml::uml::Analysis,
    ) {
        match extract_bundle(source, okf, uml)
            .into_iter()
            .find(|fields| fields.path == path)
        {
            Some(fields) => self.index.update_document(path, fields),
            None => self.index.remove_document(path),
        }
        self.status = TextIndexStatus::Ready;
    }

    // `status`/`query`/`snippet` are this task's lib-crate seam: the results
    // tab (Task 8/9), palette (Task 10/11) and find strip (Task 12/13) all
    // call them, but none of those surfaces exist yet, so nothing in-crate
    // reaches these today besides the unit tests below.
    #[allow(dead_code)]
    pub fn status(&self) -> TextIndexStatus {
        self.status
    }

    #[allow(dead_code)]
    pub fn query(&self, query: &str, scope: &QueryScope) -> Vec<Hit> {
        self.index.query(query, scope)
    }

    #[allow(dead_code)]
    pub fn snippet(&self, hit: &Hit, width: usize) -> Snippet {
        self.index.snippet(hit, width)
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_session::EditorSession;

    fn session_for(pairs: &[(&str, &str)]) -> EditorSession {
        let mut session = EditorSession::default();
        session
            .replace(SourceBundle::try_from_pairs(pairs.iter().copied()).unwrap())
            .unwrap();
        session
    }

    #[test]
    fn rebuild_indexes_a_known_class_name_as_a_names_hit() {
        let session = session_for(&[("order.md", "---\ntype: uml.Class\n---\n# Order\n")]);
        let snapshot = session.snapshot();
        let mut state = SearchState::empty();

        state.rebuild(
            &snapshot.source,
            &snapshot.okf_analysis,
            &snapshot.uml_analysis,
        );

        let hits = state.query("order", &QueryScope::default());
        assert!(!hits.is_empty());
        assert_eq!(hits[0].group, waml::search::FieldGroup::Names);
    }

    #[test]
    fn refresh_document_drops_old_terms_and_picks_up_new_ones() {
        let mut session = session_for(&[(
            "order.md",
            "---\ntype: uml.Class\n---\n# Order\n\nAbout payments.\n",
        )]);
        let snapshot = session.snapshot();
        let mut state = SearchState::empty();
        state.rebuild(
            &snapshot.source,
            &snapshot.okf_analysis,
            &snapshot.uml_analysis,
        );
        assert!(!state.query("payments", &QueryScope::default()).is_empty());

        let path = waml::source::BundlePath::parse("order.md").unwrap();
        let document = snapshot.okf_analysis.catalog.id_for_path(&path).unwrap();
        let syntax = snapshot.markdown_snapshot(document).unwrap();
        let edited = "---\ntype: uml.Class\n---\n# Order\n\nAbout shipping.\n";
        session
            .apply(waml::edit::ExactSourceEdit {
                document,
                base_revision: syntax.revision(),
                changes: std::sync::Arc::from([waml_markdown_editor::syntax::TextChange {
                    old_range: waml_markdown_editor::syntax::TextRange::new(
                        waml_markdown_editor::syntax::TextSize::new(0),
                        syntax.text().len(),
                    )
                    .unwrap(),
                    replacement: std::sync::Arc::from(edited),
                }]),
                expected_text: waml_markdown_editor::syntax::SourceText::new(edited.to_string())
                    .unwrap(),
            })
            .unwrap();
        let after = session.snapshot();

        state.refresh_document(
            "order.md",
            &after.source,
            &after.okf_analysis,
            &after.uml_analysis,
        );

        assert!(state.query("payments", &QueryScope::default()).is_empty());
        assert!(!state.query("shipping", &QueryScope::default()).is_empty());
    }

    #[test]
    fn status_is_ready_after_rebuild() {
        let snapshot = session_for(&[("order.md", "# Order\n")]).snapshot();
        let mut state = SearchState::empty();
        state.rebuild(
            &snapshot.source,
            &snapshot.okf_analysis,
            &snapshot.uml_analysis,
        );
        assert_eq!(state.status(), TextIndexStatus::Ready);
    }
}
