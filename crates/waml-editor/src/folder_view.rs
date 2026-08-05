//! The folder view-model: a folder's declared middleware chain, resolved to
//! projected rows for the folder surface
//! (spec 2026-08-05-folder-view-middleware-design.md).
//!
//! Modeled on `generic_okf_view.rs` -- the closest existing read-only
//! `DocView` + provider pair. `FolderListView` (Task D1b) binds the row
//! view-model here to a widget; nothing in this module draws.

use makepad_widgets::*;

use waml::frontmatter::Frontmatter;
use waml::okf::Directory;
use waml::view::chain::{Chain, ChainLimits, MiddlewareRegistry};
use waml::view::projection::ProjectionCtx;
use waml::view::row::{Row, RowTarget};

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::navigation::NavigationTarget;

/// One projected row, ready for display: bullet, label, optional blurb, and
/// the navigation action a click on it performs. Order matches the chain's
/// projected order.
///
/// Unused outside tests until Task D1b binds it to `FolderListView`; that
/// commit removes this allow.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRowView {
    pub bullet: &'static str,
    pub label: String,
    pub blurb: Option<String>,
    pub action: FolderRowAction,
}

/// What clicking a projected row does. `Virtual` rows (no file behind them)
/// carry no navigation target -- surfaced as `None` rather than guessed;
/// a future middleware that wants one must say so explicitly.
///
/// Unused outside tests until Task D1b binds it to `FolderListView`; that
/// commit removes this allow.
#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderRowAction {
    OpenConcept(String),
    OpenFolder(String),
    None,
}

/// The row -> navigation-action mapping, plain function form -- no widget
/// behind it yet.
///
/// Unused outside tests until Task D1b binds it to `FolderListView`; that
/// commit removes this allow.
#[allow(dead_code)]
pub fn action_for(row: &Row) -> FolderRowAction {
    match &row.target {
        RowTarget::Concept(concept_id) => FolderRowAction::OpenConcept(concept_id.clone()),
        RowTarget::Folder(address) => FolderRowAction::OpenFolder(address.clone()),
        RowTarget::Virtual => FolderRowAction::None,
    }
}

/// The row -> navigation-target mapping the shell's `NavigationIntent`
/// expects, built from the action-enum above.
///
/// Unused outside tests until Task D1b/D2 wire a row click to navigation;
/// that commit removes this allow.
#[allow(dead_code)]
pub fn navigation_for(action: &FolderRowAction) -> Option<NavigationTarget> {
    match action {
        FolderRowAction::OpenConcept(concept_id) => Some(NavigationTarget::Document {
            concept_id: concept_id.clone(),
            fragment: None,
        }),
        FolderRowAction::OpenFolder(address) => Some(NavigationTarget::Directory {
            address: address.clone(),
        }),
        FolderRowAction::None => None,
    }
}

/// Unused outside tests until Task D1b binds it to `FolderListView`; that
/// commit removes this allow.
#[allow(dead_code)]
fn row_view(row: &Row) -> FolderRowView {
    FolderRowView {
        bullet: "\u{2022}",
        label: row.label.clone(),
        blurb: row.blurb.clone(),
        action: action_for(row),
    }
}

/// Unused outside tests until Task D1b binds it to `FolderListView`; that
/// commit removes this allow.
#[allow(dead_code)]
pub fn row_views(rows: &[Row]) -> Vec<FolderRowView> {
    rows.iter().map(row_view).collect()
}

/// The `__doc_tab_folder__`-namespaced tab identity for a directory address.
/// Distinct from every concept-id tab namespace (`okf_documents`,
/// `uml_documents`) -- opening a folder never collides with opening a
/// concept of the same name.
pub fn folder_document_tab_id(directory: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_folder__{directory}"))
}

/// A folder's own view: the resolved chain's outcome, held for the row
/// view-model and any diagnostics the chain produced (Task D2 surfaces
/// these; this task only carries them).
pub struct FolderView {
    directory: String,
    /// Unused as a field read outside tests until Task D1b binds it to
    /// `FolderListView`; that commit removes this allow.
    #[allow(dead_code)]
    rows: Vec<Row>,
    diagnostics: Vec<waml::diagnostic::Diagnostic>,
}

impl FolderView {
    /// Resolve `directory`'s declared view against an empty middleware
    /// registry (Task E1's `CoreExtension` populates the real registry;
    /// until then the terminal `RootView` fallback alone is reachable) and
    /// run it under `limits`.
    pub fn build(
        analysis: &waml::analysis::OkfAnalysis,
        directory: &str,
        limits: ChainLimits,
    ) -> Option<FolderView> {
        let registry = MiddlewareRegistry::new();
        let (chain, mut diagnostics) = analysis.bundle.resolved_view(directory, &registry);
        let dir: Directory = analysis.bundle.directory(directory)?.clone();
        let params = Frontmatter::default();
        let descend = |_: &Directory| Chain::default();
        let ctx = ProjectionCtx {
            dir: &dir,
            bundle: &analysis.bundle,
            params: &params,
            descend: &descend,
        };
        let outcome = chain.run(&ctx, limits);
        diagnostics.extend(outcome.diagnostics);
        Some(FolderView {
            directory: directory.to_string(),
            rows: outcome.rows,
            diagnostics,
        })
    }

    /// Unused outside tests until Task D2 surfaces the diagnostics strip;
    /// that commit removes this allow.
    #[allow(dead_code)]
    pub fn directory(&self) -> &str {
        &self.directory
    }

    /// Unused outside tests until Task D1b binds it to `FolderListView`;
    /// that commit removes this allow.
    #[allow(dead_code)]
    pub fn row_views(&self) -> Vec<FolderRowView> {
        row_views(&self.rows)
    }

    /// Unused outside tests until Task D2 surfaces the diagnostics strip;
    /// that commit removes this allow.
    #[allow(dead_code)]
    pub fn diagnostics(&self) -> &[waml::diagnostic::Diagnostic] {
        &self.diagnostics
    }
}

impl DocView for FolderView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::Folder
    }

    fn sync(&mut self, _cx: &mut Cx, _body: &BodyWidgets, _data: ViewData<'_>) {
        // No widget bound yet -- Task D1b wires `FolderListView` here.
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        ViewOutcome::default()
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: None,
                view_toggle: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    fn analysis(
        pairs: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> waml::analysis::PreparedCandidate {
        let source = SourceBundle::try_from_pairs(pairs).unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    #[test]
    fn folder_view_model_lists_projected_rows_in_order() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Orders](orders.md)\n* [Sales](sales/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("sales/index.md", "# Sales\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let rows = view.row_views();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Orders");
        assert_eq!(rows[1].label, "Sales");
        assert!(rows.iter().all(|row| row.bullet == "\u{2022}"));
    }

    #[test]
    fn clicking_a_row_maps_to_the_right_navigation_target() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Orders](orders.md)\n* [Sales](sales/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("sales/index.md", "# Sales\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let rows = view.row_views();

        assert_eq!(
            navigation_for(&rows[0].action),
            Some(NavigationTarget::Document {
                concept_id: "orders".to_string(),
                fragment: None,
            })
        );
        assert_eq!(
            navigation_for(&rows[1].action),
            Some(NavigationTarget::Directory {
                address: "/sales".to_string(),
            })
        );
    }

    #[test]
    fn a_folder_target_gets_its_own_tab_identity() {
        assert_ne!(
            folder_document_tab_id("/sales"),
            crate::okf_documents::okf_document_tab_id("/sales"),
        );
        assert_eq!(
            folder_document_tab_id("/sales"),
            folder_document_tab_id("/sales")
        );
    }
}
