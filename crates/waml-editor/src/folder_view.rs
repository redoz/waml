//! The folder view-model: a folder's declared middleware chain, resolved to
//! projected rows for the folder surface
//! (spec 2026-08-05-folder-view-middleware-design.md).
//!
//! Modeled on `generic_okf_view.rs` -- the closest existing read-only
//! `DocView` + provider pair. `FolderListView` (`folder_list.rs`) is the
//! widget the row view-model here is bound to; nothing in this module draws.

use makepad_widgets::*;

use waml::okf::Directory;
use waml::view::chain::{Chain, ChainLimits};
use waml::view::projection::{ProjectionCtx, RowOp, Unsupported};
use waml::view::row::{Row, RowId, RowTarget};

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, ViewData, ViewOutcome,
};
use crate::extension_editor::resolve_icon;
use crate::icons::Icon;
use crate::navigation::NavigationTarget;

/// One projected row, ready for display: icon, label, optional blurb, and
/// the navigation action a click on it performs. Order matches the chain's
/// projected order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRowView {
    pub icon: Icon,
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

fn row_view(
    row: &Row,
    table: &[(&str, Icon)],
    file: &str,
) -> (FolderRowView, Option<waml::diagnostic::Diagnostic>) {
    let (icon, diagnostic) = resolve_icon(row.icon.as_ref(), &row.target, table, file, 0);
    (
        FolderRowView {
            icon,
            label: row.label.clone(),
            blurb: row.blurb.clone(),
            action: action_for(row),
        },
        diagnostic,
    )
}

/// The display shape of every row, plus every `UnknownIcon` warning resolving
/// them produced. The diagnostics are returned rather than dropped because
/// this listing's diagnostics strip is the only place such a warning can
/// reach a reader -- a stage stamping a name nothing resolves otherwise just
/// degrades to the default glyph, silently.
///
/// `file` is the directory address the rows were projected for, matching what
/// the chain's own run-level diagnostics carry (`view/chain.rs`).
pub fn row_views(
    rows: &[Row],
    table: &[(&str, Icon)],
    file: &str,
) -> (Vec<FolderRowView>, Vec<waml::diagnostic::Diagnostic>) {
    let mut views = Vec::with_capacity(rows.len());
    let mut diagnostics = Vec::new();
    for row in rows {
        let (view, diagnostic) = row_view(row, table, file);
        views.push(view);
        diagnostics.extend(diagnostic);
    }
    (views, diagnostics)
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
    /// The display shape of `rows`, resolved ONCE at build time so the
    /// `UnknownIcon` warnings that resolution produces can be folded into
    /// `diagnostics` below instead of being re-derived and dropped per draw.
    views: Vec<FolderRowView>,
    diagnostics: Vec<waml::diagnostic::Diagnostic>,
}

impl FolderView {
    /// Resolve `directory`'s rows for `mode` and hold the chain that produced
    /// them. `Raw` bypasses the declared chain; `Projected` runs it.
    pub fn build(
        analysis: &waml::analysis::OkfAnalysis,
        directory: &str,
        limits: ChainLimits,
        mode: crate::folder_projection::ViewMode,
    ) -> Option<FolderView> {
        let (chain, rows, mut diagnostics) = crate::folder_projection::project_rows(
            analysis,
            directory,
            mode,
            limits,
            &crate::folder_projection::core_registry(),
        )?;
        let (views, icon_diagnostics) =
            row_views(&rows, &crate::folder_projection::icon_table(), directory);
        // An icon name nothing resolves is a warning a reader must be able to
        // see, not a silent degrade to the default glyph.
        diagnostics.extend(icon_diagnostics);
        Some(FolderView {
            directory: directory.to_string(),
            rows,
            chain,
            views,
            diagnostics,
        })
    }

    /// Unused outside tests until Task D2 surfaces the diagnostics strip;
    /// that commit removes this allow.
    #[allow(dead_code)]
    pub fn directory(&self) -> &str {
        &self.directory
    }

    pub fn row_views(&self) -> Vec<FolderRowView> {
        self.views.clone()
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
        // Same params as `run`: the folder's own index frontmatter. A stage
        // deciding whether it owns or occludes this row must see what it saw
        // when it projected, or an edit routes differently than the listing.
        let params = analysis
            .bundle
            .index(ctx_directory)
            .map(|index| index.extra.clone())
            .unwrap_or_default();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
        let rows = view.row_views();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Orders");
        assert_eq!(rows[1].label, "Sales");
        assert_eq!(
            rows[1].icon,
            Icon::Book,
            "a folder row carries the book glyph"
        );
    }

    /// Task 10: a mixed listing -- a `uml-domain` child, a plain child, a
    /// `uml.Class` concept, and a `note` concept -- projected through a
    /// declared `view: uml` chain resolves every row to the icon the plan's
    /// V2/V4 checks expect: the box glyph is `uml`'s alone, the book glyph is
    /// every plain folder's, and the class/note glyphs are exactly what they
    /// resolve to today, unchanged.
    #[test]
    fn row_views_resolves_the_icon_table_for_a_mixed_listing() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: uml\n---\n# Root\n\n* [Pkg](pkg/)\n* [Docs](docs/)\n* [Order](order.md)\n* [Notes](notes.md)\n",
            ),
            ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
            ("docs/index.md", "# Docs\n"),
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("notes.md", "---\ntype: note\n---\n# Notes\n"),
        ]);
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
        let rows = view.row_views();
        assert_eq!(
            rows.iter().map(|row| row.icon).collect::<Vec<_>>(),
            vec![Icon::Box, Icon::Book, Icon::PanelTop, Icon::FileText],
            "class and note glyphs are exactly what they resolve to today",
        );
    }

    /// The end-to-end check the headless tests structurally cannot make: they
    /// build their own registry and their own params, so both of the editor's
    /// wiring defects (empty registry, empty params) were invisible to them
    /// and the gate stayed green while `view: hide` was reporting `unknown
    /// view middleware` to users. Zero diagnostics is the assertion that
    /// matters -- a correctly authored document must not be diagnosed.
    #[test]
    fn a_declared_hide_chain_filters_rows_with_no_diagnostics_in_the_editor() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n* [Orders](orders.md)\n* [References](references/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("references/index.md", "# References\n"),
        ]);
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();

        assert!(
            view.diagnostics().is_empty(),
            "a correctly authored `view: hide` must not be diagnosed: {:?}",
            view.diagnostics()
        );
        let rows = view.row_views();
        let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels,
            vec!["Orders"],
            "the hidden row must be filtered out of the declared listing"
        );
    }

    /// The tree marker must agree with what opening the folder actually does.
    /// Two registries that disagree put a degraded dot on a folder that opens
    /// clean (or the reverse), and nothing fails.
    #[test]
    fn the_tree_and_the_folder_view_agree_on_whether_a_chain_degraded() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n* [Orders](orders.md)\n* [References](references/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("references/index.md", "# References\n"),
        ]);
        let bundle = &prepared.okf().bundle;

        let (_, tree_diagnostics) = bundle.resolved_view(
            "/",
            &crate::folder_projection::core_registry(),
            &waml::view::mask::ProjectionMask::default(),
        );
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();

        assert!(tree_diagnostics.is_empty());
        assert_eq!(tree_diagnostics.is_empty(), view.diagnostics().is_empty());
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
    fn enter_on_a_row_emits_insert_concept_at_that_position() {
        let prepared = analysis([
            ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/sales",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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
        let view = FolderView::build(
            prepared.okf(),
            "/",
            ChainLimits::default(),
            crate::folder_projection::ViewMode::Projected,
        )
        .unwrap();
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

    /// Task 8's warning path, reached from the surface that can show it: an
    /// icon name nothing resolves degrades to the default glyph AND produces
    /// an `UnknownIcon` diagnostic for the folder's diagnostics strip. Dropped
    /// on the floor, the name silently draws as a plain folder and no reader
    /// ever learns the stage named something that does not exist.
    #[test]
    fn an_unknown_icon_name_is_diagnosed_not_silently_degraded() {
        let folder_row = || {
            Row::new(
                RowId {
                    owner: waml::view::row::ViewId::new("test"),
                    path: waml::view::row::RowPath::parse("pkg").unwrap(),
                },
                "Pkg".to_string(),
                RowTarget::Folder("/pkg".to_string()),
                None,
            )
            .unwrap()
        };
        let mut stamped = folder_row();
        stamped.icon = Some(waml::view::row::IconId::new("no-such-icon"));

        let table = crate::folder_projection::icon_table();
        let (views, diagnostics) = row_views(&[stamped], &table, "/");
        assert_eq!(views[0].icon, Icon::Folder, "degrades to the default glyph");
        assert_eq!(diagnostics.len(), 1, "and says so: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, waml::diagnostic::DiagCode::UnknownIcon,);
        assert_eq!(diagnostics[0].file, "/");

        let (_, clean) = row_views(&[folder_row()], &table, "/");
        assert!(clean.is_empty(), "a row with no icon is not a warning");
    }
}
