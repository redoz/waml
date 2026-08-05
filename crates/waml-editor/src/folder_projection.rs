//! Where a folder's rows come from — for BOTH surfaces that show them.
//!
//! The folder surface (`folder_view.rs`) and the tree seam (`tree.rs`) run
//! the same chain, against the same registry, in the same mode. Two row
//! sources that disagree are invisible: the tree lists a child the folder
//! view does not, or marks a folder degraded that opens clean, and the gate
//! is green either way.
//!
//! Deliberately makepad-free, like `tree.rs`, so both consumers can depend on
//! it and its behaviour is unit-testable with no window.

use waml::diagnostic::Diagnostic;
use waml::okf::Directory;
use waml::view::chain::{Chain, ChainLimits, MiddlewareRegistry};
use waml::view::projection::ProjectionCtx;
use waml::view::row::Row;

/// The session-wide projected/raw switch, held in memory on `App` and read by
/// every surface that lists a folder's contents.
///
/// NOT persisted, and `.waml/settings.json` never sees it: raw is a deliberate
/// act, not a preference, so every launch starts `Projected` and an author's
/// declared `view:` is what a reader sees unless they ask otherwise.
///
/// `Raw` is presentational reachability and performs no access check. Nothing
/// in waml treats a row a chain declined to emit as protected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Projected,
    /// Unconstructed outside tests until Task 9/10 wire the tree-panel and
    /// folder-tab toggle that flips `App::view_mode` to it; this task only
    /// lands the mode and the row source that honors it.
    #[allow(dead_code)]
    Raw,
}

/// The middleware registry every folder-listing path in the editor resolves
/// against: the core extension's `index` and `hide`.
///
/// One function because two construction sites that disagree are invisible --
/// a folder resolves fine in one and reports `unknown view middleware` in the
/// other, with the gate green either way. Cheap enough to build per call;
/// nothing here caches across frames.
pub fn core_registry() -> MiddlewareRegistry {
    MiddlewareRegistry::from_extensions(&[&waml::extension::CoreExt])
        .expect("the core extension registers a conflict-free name table")
}

/// The chain `directory` runs under `mode`, plus any build-level diagnostics
/// (unknown middleware name, bad params) the declared chain produced.
///
/// `Raw` pins the chain to `Chain::raw()` -- the identity listing -- and never
/// builds the declared chain at all, which is why it never diagnoses one.
pub fn chain_for(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mode: ViewMode,
) -> (Chain, Vec<Diagnostic>) {
    match mode {
        ViewMode::Projected => analysis.bundle.resolved_view(directory, &core_registry()),
        ViewMode::Raw => (Chain::raw(), Vec::new()),
    }
}

/// Run `directory`'s chain for `mode` and hand back the chain itself, its
/// rows, and every diagnostic (build-level and run-level) it produced.
///
/// The chain comes back with the rows because a later gesture must call
/// `Chain::apply` against the exact stages that minted the `RowId`s it
/// addresses; rebuilding from `directory` alone could resolve a different
/// stage set if the bundle changed underneath.
///
/// `None` means `directory` is not in the bundle at all.
pub fn project_rows(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mode: ViewMode,
    limits: ChainLimits,
) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)> {
    let dir: Directory = analysis.bundle.directory(directory)?.clone();
    let (chain, mut diagnostics) = chain_for(analysis, directory, mode);
    // A middleware's params ARE the folder's own index frontmatter -- `hide`
    // reads its globs from here, and `Chain::build` validated them against
    // this same map. Passing an empty one makes every param-taking stage fail
    // its own declaration check and trip the whole-chain fallback.
    let params = analysis
        .bundle
        .index(directory)
        .map(|index| index.extra.clone())
        .unwrap_or_default();
    let descend = |_: &Directory| Chain::default();
    let ctx = ProjectionCtx {
        dir: &dir,
        bundle: &analysis.bundle,
        params: &params,
        descend: &descend,
    };
    let outcome = chain.run(&ctx, limits);
    diagnostics.extend(outcome.diagnostics);
    Some((chain, outcome.rows, diagnostics))
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

    fn hidden_bundle() -> waml::analysis::PreparedCandidate {
        analysis([
            (
                "index.md",
                "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n* [Orders](orders.md)\n* [References](references/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("references/index.md", "# References\n"),
        ])
    }

    #[test]
    fn projected_runs_the_declared_chain_and_raw_bypasses_it() {
        let prepared = hidden_bundle();
        let limits = ChainLimits::default();

        let (_, projected, diagnostics) =
            project_rows(prepared.okf(), "/", ViewMode::Projected, limits).unwrap();
        assert!(
            diagnostics.is_empty(),
            "a correctly authored `view: hide` must not be diagnosed: {diagnostics:?}"
        );
        assert_eq!(
            projected
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Orders"],
        );

        let (_, raw, raw_diagnostics) =
            project_rows(prepared.okf(), "/", ViewMode::Raw, limits).unwrap();
        assert!(
            raw_diagnostics.is_empty(),
            "raw never builds the declared chain"
        );
        assert_eq!(
            raw.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
            vec!["Orders", "References"],
            "raw is presentational reachability, not a permission decision",
        );
    }

    #[test]
    fn raw_never_diagnoses_a_declared_chain_it_does_not_build() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: nonexistent\n---\n# Root\n\n* [Orders](orders.md)\n",
            ),
            ("orders.md", "# Orders\n"),
        ]);
        let limits = ChainLimits::default();

        let (_, _, declared) =
            project_rows(prepared.okf(), "/", ViewMode::Projected, limits).unwrap();
        assert!(!declared.is_empty(), "an unknown middleware name diagnoses");

        let (_, _, raw) = project_rows(prepared.okf(), "/", ViewMode::Raw, limits).unwrap();
        assert!(raw.is_empty());
    }

    #[test]
    fn a_missing_directory_yields_none_rather_than_panicking() {
        let prepared = hidden_bundle();
        assert!(project_rows(
            prepared.okf(),
            "/missing",
            ViewMode::Projected,
            ChainLimits::default()
        )
        .is_none());
    }

    #[test]
    fn raw_mode_owns_every_row_through_the_root_view() {
        let prepared = hidden_bundle();
        let (_, rows, _) =
            project_rows(prepared.okf(), "/", ViewMode::Raw, ChainLimits::default()).unwrap();
        assert!(
            rows.iter()
                .all(|row| row.id.owner.as_str() == waml::view::ROOT_VIEW_OWNER),
            "in Raw the chain is [index], so RootView owns every row",
        );
    }
}
