//! Editor-side index lifecycle: build the bundle's text index on open,
//! refresh a single document on save. `App` owns one `SearchState`; every
//! search surface (results tab, palette, find strip) reads it through
//! `query`/`snippet` and never sees the backend (`MemSearchIndex`) directly
//! (spec §Engine boundary).

use std::collections::HashSet;

use waml::analysis::OkfAnalysis;
use waml::search::extract::extract_bundle;
use waml::search::{Hit, MemSearchIndex, QueryScope, SearchIndex, Snippet};
use waml::source::SourceBundle;

/// Whether the index reflects the live bundle yet. v1 builds synchronously
/// (single-digit ms at current bundle size), so `rebuild`/`refresh_document`
/// always leave `status` at `Ready`; `Building` exists for the palette's
/// `indexing…` row and a later async backend (decision 6).
///
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextIndexStatus {
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

    // `status` is this task's lib-crate seam for the palette's `indexing…`
    // row (Task 10/11); nothing in-crate reaches it yet besides the unit
    // test below. `query`/`snippet` are reached by `App::open_search_results`
    // (Task 9) now.
    #[allow(dead_code)]
    pub fn status(&self) -> TextIndexStatus {
        self.status
    }

    pub fn query(&self, query: &str, scope: &QueryScope) -> Vec<Hit> {
        self.index.query(query, scope)
    }

    pub fn snippet(&self, hit: &Hit, width: usize) -> Snippet {
        self.index.snippet(hit, width)
    }

    /// Concepts present in the bundle but absent from the projected tree
    /// (decision 9): the same document-level hiding a folder-view middleware
    /// chain performs today (element-level masking is not modelled in v1).
    /// Built the way `tree_panel.rs` composes its own tree
    /// (`crate::tree::build_tree`), under the DEFAULT projection mask and
    /// chain-descent cap -- `SearchState` carries neither the session's live
    /// mask nor its cap, so this is a best-effort badge, not an exhaustive
    /// one: a hit hidden only under a non-default mask is not caught here.
    pub fn hidden_documents(
        &self,
        okf: &OkfAnalysis,
        uml: &waml::uml::Analysis,
    ) -> HashSet<String> {
        let tree = crate::tree::build_tree(
            okf,
            uml,
            "",
            &waml::view::mask::ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let mut visible = HashSet::new();
        collect_concept_ids(&tree.roots, &mut visible);
        okf.bundle
            .concepts()
            .iter()
            .filter(|concept| !visible.contains(&concept.id))
            .map(|concept| format!("{}.md", concept.id))
            .collect()
    }
}

fn collect_concept_ids(nodes: &[crate::tree::TreeNode], out: &mut HashSet<String>) {
    for node in nodes {
        if let Some(id) = &node.concept_id {
            out.insert(id.clone());
        }
        collect_concept_ids(&node.children, out);
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

    #[test]
    fn hidden_documents_reports_concepts_the_projected_tree_declines_to_list() {
        let session = session_for(&[
            ("index.md", "# Root\n\n* [Shop](shop/)\n"),
            (
                "shop/index.md",
                "---\nview: hide\nhide: [\"shop/secret\"]\n---\n# Shop\n\n* [Order](order.md)\n* [Secret](secret.md)\n",
            ),
            ("shop/order.md", "---\ntype: Runbook\n---\n# Order\n"),
            ("shop/secret.md", "---\ntype: Runbook\n---\n# Secret\n"),
        ]);
        let snapshot = session.snapshot();
        let state = SearchState::empty();

        let hidden = state.hidden_documents(&snapshot.okf_analysis, &snapshot.uml_analysis);

        assert!(hidden.contains("shop/secret.md"));
        assert!(!hidden.contains("shop/order.md"));
    }
}
