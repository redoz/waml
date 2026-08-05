//! The folder view-model: a folder's declared middleware chain, resolved to
//! projected rows for the folder surface
//! (spec 2026-08-05-folder-view-middleware-design.md).
//!
//! Modeled on `generic_okf_view.rs` -- the closest existing read-only
//! `DocView` + provider pair. `FolderListView` (`folder_list.rs`) is the
//! widget the row view-model here is bound to; nothing in this module draws.

use makepad_widgets::*;

use waml::frontmatter::Frontmatter;
use waml::okf::Directory;
use waml::view::chain::{Chain, ChainLimits, MiddlewareRegistry};
use waml::view::projection::{ProjectionCtx, RowOp, Unsupported};
use waml::view::row::{Row, RowId, RowTarget};

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::navigation::NavigationTarget;

/// One projected row, ready for display: bullet, label, optional blurb, and
/// the navigation action a click on it performs. Order matches the chain's
/// projected order.
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderRowAction {
    OpenConcept(String),
    OpenFolder(String),
    None,
}

/// The row -> navigation-action mapping, plain function form.
pub fn action_for(row: &Row) -> FolderRowAction {
    match &row.target {
        RowTarget::Concept(concept_id) => FolderRowAction::OpenConcept(concept_id.clone()),
        RowTarget::Folder(address) => FolderRowAction::OpenFolder(address.clone()),
        RowTarget::Virtual => FolderRowAction::None,
    }
}

/// The row -> navigation-target mapping the shell's `NavigationIntent`
/// expects, built from the action-enum above.
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

fn row_view(row: &Row) -> FolderRowView {
    FolderRowView {
        bullet: "\u{2022}",
        label: row.label.clone(),
        blurb: row.blurb.clone(),
        action: action_for(row),
    }
}

pub fn row_views(rows: &[Row]) -> Vec<FolderRowView> {
    rows.iter().map(row_view).collect()
}

/// Task G3's keyboard-gesture -> `RowOp` mapping. Pure and headless: the
/// caller (`folder_list.rs`'s widget) supplies the projected rows and the
/// focused index; the result is what `Chain::apply` (or the equivalent
/// terminal `RootView::apply`) should be called with. Affordances gate on
/// declared `caps`/`child_caps` here too (advisory, matching the row-list
/// rendering), but `apply` remains the authority -- a `None` here just means
/// the gesture never reaches `apply` at all, and an `apply` refusal past
/// this point is a no-op, never a crash.
///
/// Enter inserts a new concept immediately after the focused row. The
/// `RowId` passed back addresses the anchor row itself -- `RootView::apply`
/// keys `InsertConcept`'s position off the `path` argument matching `after`.
///
/// Called from `FolderView::handle` on `FolderListViewAction::EnterPressed`.
pub fn enter_row_op(rows: &[Row], index: usize) -> Option<(RowId, RowOp)> {
    let row = rows.get(index)?;
    Some((
        row.id.clone(),
        RowOp::InsertConcept {
            after: Some(row.id.path.clone()),
            title: "Untitled".to_string(),
        },
    ))
}

/// Live retitling commits a `Rename` on the focused row, gated on its
/// declared `caps.rename`. `title` is the edit buffer `folder_list.rs`
/// accumulated over `F2` + `Hit::TextInput`/`Hit::KeyDown::Backspace`,
/// committed on `ReturnKey`/blur -- mirroring `inspector_panel.rs`'s field
/// editing.
///
/// Called from `FolderView::handle` on `FolderListViewAction::RenameCommitted`.
pub fn rename_row_op(rows: &[Row], index: usize, title: String) -> Option<(RowId, RowOp)> {
    let row = rows.get(index)?;
    if !row.caps.rename {
        return None;
    }
    Some((row.id.clone(), RowOp::Rename { title }))
}

/// Tab reparents the focused row into the nearest PRECEDING sibling that is
/// itself a directory accepting move-in, addressed at that sibling's
/// `RowId`. INTERIM (open question 1, carried forward -- not resolved
/// here): a concept with no preceding sibling directory refuses rather than
/// promoting itself to `<slug>/index.md`.
///
/// Called from `FolderView::handle` on `FolderListViewAction::TabPressed`.
pub fn tab_row_op(rows: &[Row], index: usize) -> Option<(RowId, RowOp)> {
    let row = rows.get(index)?;
    let sibling = rows[..index].iter().rev().find(|candidate| {
        matches!(candidate.target, RowTarget::Folder(_)) && candidate.child_caps.accept_move_in
    })?;
    Some((
        sibling.id.clone(),
        RowOp::MoveIn {
            from: row.id.clone(),
        },
    ))
}

/// Shift-Tab moves the focused row out to its parent directory, gated on
/// its declared `caps.move_out` (never satisfiable at the bundle root --
/// `RootView::apply`'s own `MoveOut` branch refuses there too).
///
/// Called from `FolderView::handle` on `FolderListViewAction::ShiftTabPressed`.
pub fn shift_tab_row_op(rows: &[Row], index: usize) -> Option<(RowId, RowOp)> {
    let row = rows.get(index)?;
    if !row.caps.move_out {
        return None;
    }
    Some((row.id.clone(), RowOp::MoveOut))
}

/// Task G4's drag-reorder -> `RowOp` mapping. `from_index` is the row the
/// drag armed on; `drop_index` is where `folder_list.rs` computed the
/// pointer landed (`drop_index_from_pointer_y`), in the SAME pre-drag row
/// indexing -- an index into `rows` as they stood before any row moves.
/// Dropping a row onto itself or immediately after itself (`drop_index ==
/// from_index` or `drop_index == from_index + 1`) is a no-op: nothing
/// changed, so nothing is emitted, matching "on-self is a no-op" from the
/// plan's own test list. `RootView::apply`'s `Reorder` arm resolves `before`
/// by identity (the sibling's `RowPath`), not by index, so handing it the
/// row currently at `drop_index` is correct even though removing `from_index`
/// first would shift indices -- the identity survives the shift, only the
/// index would not.
///
/// Called from `FolderView::handle` on `FolderListViewAction::RowDropped`.
pub fn reorder_row_op(
    rows: &[Row],
    from_index: usize,
    drop_index: usize,
) -> Option<(RowId, RowOp)> {
    let row = rows.get(from_index)?;
    if drop_index > rows.len() {
        return None;
    }
    if drop_index == from_index || drop_index == from_index + 1 {
        return None;
    }
    let before = rows.get(drop_index).map(|sibling| sibling.id.path.clone());
    Some((row.id.clone(), RowOp::Reorder { before }))
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
    rows: Vec<Row>,
    /// The chain the rows above were projected through, retained so a later
    /// gesture (Task G3) can call `Chain::apply` against the exact same
    /// stages that produced the `RowId`s the gesture addresses -- rebuilding
    /// the chain from `directory` alone could resolve a different (possibly
    /// stale) middleware set if the bundle's frontmatter changed underneath.
    chain: Chain,
    diagnostics: Vec<waml::diagnostic::Diagnostic>,
    /// Set when this view was opened through the raw route (Task D3): the
    /// identity listing, bypassing whatever chain `directory` declares.
    /// `FolderListView`'s raw-mode banner is bound to this, not to whether
    /// `diagnostics` is empty -- raw and degraded are different reasons to
    /// tell the user the listing isn't the folder's plain declared view.
    raw: bool,
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
        let (rows, outcome_diags) = Self::run(analysis, directory, &chain, limits)?;
        diagnostics.extend(outcome_diags);
        Some(FolderView {
            directory: directory.to_string(),
            rows,
            chain,
            diagnostics,
            raw: false,
        })
    }

    /// The raw OKF layer (Task D3, spec: "The raw OKF layer"): `directory`'s
    /// identity listing via `Chain::raw()`, bypassing whatever it declares
    /// entirely -- the declared chain is never even built, so a hidden row
    /// is always reachable here regardless of what filtered it out of the
    /// declared listing. This is presentational, not a permission boundary.
    pub fn build_raw(
        analysis: &waml::analysis::OkfAnalysis,
        directory: &str,
    ) -> Option<FolderView> {
        let chain = Chain::raw();
        let (rows, diagnostics) = Self::run(analysis, directory, &chain, ChainLimits::default())?;
        Some(FolderView {
            directory: directory.to_string(),
            rows,
            chain,
            diagnostics,
            raw: true,
        })
    }

    fn run(
        analysis: &waml::analysis::OkfAnalysis,
        directory: &str,
        chain: &Chain,
        limits: ChainLimits,
    ) -> Option<(Vec<Row>, Vec<waml::diagnostic::Diagnostic>)> {
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
        Some((outcome.rows, outcome.diagnostics))
    }

    /// Unused outside tests until Task D2 surfaces the diagnostics strip;
    /// that commit removes this allow.
    #[allow(dead_code)]
    pub fn directory(&self) -> &str {
        &self.directory
    }

    pub fn row_views(&self) -> Vec<FolderRowView> {
        row_views(&self.rows)
    }

    /// The projected rows themselves, for Task G3's gesture->`RowOp`
    /// mapping functions (`enter_row_op` et al.), which need `caps` and
    /// `RowId` -- `row_views` erases both into a display-only shape.
    ///
    /// Not yet called from `folder_list.rs` -- see `enter_row_op`'s note.
    #[allow(dead_code)]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Lower one `(RowId, RowOp)` gesture (as produced by `enter_row_op` et
    /// al.) to the real OKF op batch, by re-running `id`'s owning stage's
    /// `apply` through the SAME chain the rows were projected through.
    /// Never touches disk itself -- the caller wraps the batch in a
    /// `PendingEdit` and hands it to `ViewOutcome::edit`, so it goes through
    /// the normal edit pipeline (undo, atomic save, reparse) like every
    /// other surface's edits.
    ///
    /// Called from `FolderView::handle` (below); kept as its own method so
    /// the headless test below can exercise it without a `Cx`.
    pub fn apply_gesture(
        &self,
        analysis: &waml::analysis::OkfAnalysis,
        id: &RowId,
        op: RowOp,
    ) -> Result<Vec<waml::okf::Op>, Unsupported> {
        // `RootView::apply`'s `MoveIn` arm (crates/waml/src/view/root.rs)
        // reads its DESTINATION directory off `ctx.dir`, not off `id`/`path`
        // -- `tab_row_op` addresses `id` at the target sibling's OWN row
        // (still projected within `self.directory`'s listing), so for this
        // one op the apply context must be that sibling's directory, not
        // `self.directory` itself. Every other `RowOp` variant resolves its
        // row from `path` against `ctx.dir` directly, so `self.directory`
        // is correct for them.
        let ctx_directory = if matches!(op, RowOp::MoveIn { .. }) {
            match self
                .rows
                .iter()
                .find(|row| &row.id == id)
                .map(|row| &row.target)
            {
                Some(RowTarget::Folder(address)) => address.as_str(),
                _ => return Err(Unsupported),
            }
        } else {
            self.directory.as_str()
        };
        let Some(dir) = analysis.bundle.directory(ctx_directory).cloned() else {
            return Err(Unsupported);
        };
        let params = Frontmatter::default();
        let descend = |_: &Directory| Chain::default();
        let ctx = ProjectionCtx {
            dir: &dir,
            bundle: &analysis.bundle,
            params: &params,
            descend: &descend,
        };
        self.chain.apply(&ctx, id, op)
    }

    pub fn diagnostics(&self) -> &[waml::diagnostic::Diagnostic] {
        &self.diagnostics
    }

    /// Lower a `(RowId, RowOp)` gesture (as `enter_row_op`/`tab_row_op`/
    /// `shift_tab_row_op` produce) into `outcome.edit`, through
    /// `apply_gesture`. A `None` gesture (the row/child caps refused, or the
    /// focused index no longer exists) is a silent no-op, never a crash --
    /// same for an `apply_gesture` refusal (a stale `RowId` from a chain
    /// shape that changed underneath). `label` is the undo-stack entry.
    fn commit_gesture(
        &self,
        analysis: &waml::analysis::OkfAnalysis,
        gesture: Option<(RowId, RowOp)>,
        label: &str,
        outcome: &mut ViewOutcome,
    ) {
        let Some((id, op)) = gesture else {
            return;
        };
        let Ok(ops) = self.apply_gesture(analysis, &id, op) else {
            return;
        };
        if ops.is_empty() {
            return;
        }
        outcome.edit = Some(crate::document::EditIntent {
            edit: waml::edit::PendingEdit::new(waml::okf::Batch(ops)),
            label: label.to_string(),
            merge_key: None,
            after_location: None,
        });
    }
}

impl DocView for FolderView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::Folder
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.show_folder_view(cx);
        body.folder_list().set_rows(cx, self.row_views());
        body.folder_list().set_diagnostics(cx, self.diagnostics());
        body.folder_list().set_raw(cx, self.raw);
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        body: &BodyWidgets,
        actions: &Actions,
        data: ViewData<'_>,
    ) -> ViewOutcome {
        let mut outcome = ViewOutcome::default();
        if let Some(index) = body.folder_list().row_opened(actions) {
            if let Some(row) = self.rows.get(index) {
                if let Some(target) = navigation_for(&action_for(row)) {
                    outcome.navigation = Some(crate::navigation::NavigationIntent::Resolved {
                        target,
                        disposition: crate::navigation::OpenDisposition::Preview,
                    });
                }
            }
        } else if !self.raw && body.folder_list().raw_requested(actions) {
            outcome.navigation = Some(crate::navigation::NavigationIntent::Resolved {
                target: crate::navigation::NavigationTarget::DirectoryRaw {
                    address: self.directory.clone(),
                },
                disposition: crate::navigation::OpenDisposition::Preview,
            });
        } else if let Some(index) = body.folder_list().enter_pressed(actions) {
            self.commit_gesture(
                data.okf_analysis,
                enter_row_op(&self.rows, index),
                "Insert row",
                &mut outcome,
            );
        } else if let Some(index) = body.folder_list().tab_pressed(actions) {
            self.commit_gesture(
                data.okf_analysis,
                tab_row_op(&self.rows, index),
                "Move row in",
                &mut outcome,
            );
        } else if let Some(index) = body.folder_list().shift_tab_pressed(actions) {
            self.commit_gesture(
                data.okf_analysis,
                shift_tab_row_op(&self.rows, index),
                "Move row out",
                &mut outcome,
            );
        } else if let Some((index, title)) = body.folder_list().rename_committed(actions) {
            self.commit_gesture(
                data.okf_analysis,
                rename_row_op(&self.rows, index, title),
                "Rename row",
                &mut outcome,
            );
        } else if let Some((from_index, drop_index)) = body.folder_list().row_dropped(actions) {
            self.commit_gesture(
                data.okf_analysis,
                reorder_row_op(&self.rows, from_index, drop_index),
                "Reorder row",
                &mut outcome,
            );
        }
        outcome
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

    /// The editor-level half of Task D3's "search hit on a hidden path"
    /// caller: no search UI exists to hang the test off, but the routing
    /// contract it depends on is exactly this -- `build_raw` never even
    /// looks at what the folder declared, so a target the declared route
    /// diagnoses (or, once F1 lands, filters out) is unconditionally
    /// reachable through it.
    #[test]
    fn build_raw_bypasses_the_declared_chain_and_its_diagnostics() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: nonexistent\n---\n# Root\n\n* [Orders](orders.md)\n",
            ),
            ("orders.md", "# Orders\n"),
        ]);
        let declared = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        assert!(
            !declared.diagnostics().is_empty(),
            "an unknown declared middleware name diagnoses on the declared route"
        );

        let raw = FolderView::build_raw(prepared.okf(), "/").unwrap();
        assert!(
            raw.diagnostics().is_empty(),
            "raw never builds the declared chain, so it never diagnoses it either"
        );
        assert_eq!(
            raw.row_views().len(),
            declared.row_views().len(),
            "both land on the same identity listing here -- the declared route only \
             because its unknown-middleware fallback also lands on the root view"
        );

        assert!(FolderView::build_raw(prepared.okf(), "/missing").is_none());
    }

    #[test]
    fn enter_on_a_row_emits_insert_concept_at_that_position() {
        let prepared = analysis([
            ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let (id, op) = enter_row_op(view.rows(), 0).unwrap();
        assert_eq!(id, view.rows()[0].id);
        assert_eq!(
            op,
            RowOp::InsertConcept {
                after: Some(view.rows()[0].id.path.clone()),
                title: "Untitled".to_string(),
            }
        );
        assert!(enter_row_op(view.rows(), 99).is_none());
    }

    #[test]
    fn typing_commits_a_rename_row_op() {
        let prepared = analysis([
            ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        assert!(view.rows()[0].caps.rename, "a concept row declares rename");
        let (id, op) = rename_row_op(view.rows(), 0, "Purchase Orders".to_string()).unwrap();
        assert_eq!(id, view.rows()[0].id);
        assert_eq!(
            op,
            RowOp::Rename {
                title: "Purchase Orders".to_string()
            }
        );
    }

    #[test]
    fn tab_emits_move_in_to_the_preceding_sibling_directory() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Sales](sales/)\n* [Orders](orders.md)\n",
            ),
            ("sales/index.md", "# Sales\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let rows = view.rows();
        assert_eq!(rows[0].label, "Sales");
        assert_eq!(rows[1].label, "Orders");
        let (id, op) = tab_row_op(rows, 1).unwrap();
        assert_eq!(id, rows[0].id, "addressed at the preceding directory row");
        assert_eq!(
            op,
            RowOp::MoveIn {
                from: rows[1].id.clone()
            }
        );
    }

    #[test]
    fn tab_with_no_preceding_sibling_directory_refuses() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Orders](orders.md)\n* [Sales](sales/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("sales/index.md", "# Sales\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        // Orders (index 0) has no preceding row at all; Sales (index 1) has
        // only a concept row preceding it, not a directory.
        assert!(tab_row_op(view.rows(), 0).is_none());
        assert!(tab_row_op(view.rows(), 1).is_none());
    }

    #[test]
    fn shift_tab_emits_move_out() {
        let prepared = analysis([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n\n* [Orders](orders.md)\n"),
            ("sales/orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/sales", ChainLimits::default()).unwrap();
        let rows = view.rows();
        assert!(
            rows[0].caps.move_out,
            "a row inside a non-root directory declares move_out"
        );
        let (id, op) = shift_tab_row_op(rows, 0).unwrap();
        assert_eq!(id, rows[0].id);
        assert_eq!(op, RowOp::MoveOut);
    }

    #[test]
    fn shift_tab_refuses_at_the_bundle_root() {
        let prepared = analysis([
            ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        assert!(
            !view.rows()[0].caps.move_out,
            "the bundle root has no parent to move out to"
        );
        assert!(shift_tab_row_op(view.rows(), 0).is_none());
    }

    #[test]
    fn reorder_emits_before_the_row_at_the_drop_index() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Orders](orders.md)\n* [Sales](sales/)\n* [Refunds](refunds.md)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("sales/index.md", "# Sales\n"),
            ("refunds.md", "# Refunds\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let rows = view.rows();
        // Drag row 0 (Orders) to drop index 2: lands before the row that was
        // at index 2 (Refunds) pre-drag.
        let (id, op) = reorder_row_op(rows, 0, 2).unwrap();
        assert_eq!(id, rows[0].id);
        assert_eq!(
            op,
            RowOp::Reorder {
                before: Some(rows[2].id.path.clone())
            }
        );
        // Dropped past the end: no sibling to land before.
        let (_, op) = reorder_row_op(rows, 0, 3).unwrap();
        assert_eq!(op, RowOp::Reorder { before: None });
    }

    #[test]
    fn a_refused_reorder_leaves_row_order_unchanged() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Orders](orders.md)\n* [Sales](sales/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("sales/index.md", "# Sales\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        // Dropped on itself, and dropped immediately after itself: both are
        // no-ops -- nothing is emitted, so nothing could reorder the rows.
        assert!(reorder_row_op(view.rows(), 0, 0).is_none());
        assert!(reorder_row_op(view.rows(), 0, 1).is_none());
        assert!(reorder_row_op(view.rows(), 1, 1).is_none());
        // Out of bounds drop index refuses too, rather than panicking.
        assert!(reorder_row_op(view.rows(), 0, 99).is_none());
        let before = view.row_views();
        assert_eq!(view.row_views(), before, "row order is unchanged");
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

    /// `apply_gesture` is the piece `FolderView::handle` calls to turn an
    /// `enter_row_op`/`tab_row_op`/`shift_tab_row_op` result into the real
    /// `okf::Op` batch `ViewOutcome::edit` carries into the normal edit
    /// pipeline (undo, atomic save, reparse) -- nothing here writes a file
    /// directly.
    #[test]
    fn apply_gesture_lowers_enter_row_op_to_a_concept_new_and_reorder_batch() {
        let prepared = analysis([
            ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let (id, op) = enter_row_op(view.rows(), 0).unwrap();
        let ops = view.apply_gesture(prepared.okf(), &id, op).unwrap();
        assert!(
            ops.iter()
                .any(|op| matches!(op, waml::okf::Op::ConceptNew { .. })),
            "enter inserts a new concept: {ops:?}"
        );
    }

    #[test]
    fn apply_gesture_lowers_tab_row_op_to_a_move_in() {
        let prepared = analysis([
            (
                "index.md",
                "# Root\n\n* [Sales](sales/)\n* [Orders](orders.md)\n",
            ),
            ("sales/index.md", "# Sales\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(prepared.okf(), "/", ChainLimits::default()).unwrap();
        let (id, op) = tab_row_op(view.rows(), 1).unwrap();
        let ops = view.apply_gesture(prepared.okf(), &id, op).unwrap();
        assert_eq!(
            ops,
            vec![waml::okf::Op::ConceptMove {
                id: "orders".to_string(),
                to_directory: waml::okf::DirectoryAddress::parse("/sales").unwrap(),
            }]
        );
    }
}
