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

use crate::extension_editor::{CoreEditorExtension, EditorExtension, UmlEditorExtension};
use crate::icons::Icon;
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
    Raw,
}

/// The middleware registry every folder-listing path in the editor resolves
/// against: the core extension's `index` and `hide`.
///
/// One function because two construction sites that disagree are invisible --
/// a folder resolves fine in one and reports `unknown view middleware` in the
/// other, with the gate green either way. Build it ONCE per listing pass and
/// pass it down: `build_tree` recurses over every directory in the bundle on
/// every model refresh, and re-minting the name table per directory put that
/// cost on a path that runs per document activation.
pub fn core_registry() -> MiddlewareRegistry {
    // Driven off `SHIPPED_EXTENSIONS` so this registry and `waml::profile`'s
    // name-check table are built from the same list by construction.
    let extensions: Vec<&dyn waml::extension::CoreExtension> = waml::extension::SHIPPED_EXTENSIONS
        .iter()
        .map(|ext| *ext as &dyn waml::extension::CoreExtension)
        .collect();
    MiddlewareRegistry::from_extensions(&extensions)
        .expect("the core extension registers a conflict-free name table")
}

/// The `EditorExtension` list every editor-side registry (surfaces, icons)
/// resolves against -- the sibling of `core_registry()`'s middleware list, so
/// the two construction sites cannot disagree about which extensions are
/// live. Grows alongside `core_registry()`: a middleware whose editor half
/// is missing here mints rows nothing can open or draw an icon for.
// Consumer: Tasks 10-11's icon-resolution wiring, deferred.
#[allow(dead_code)]
pub fn editor_registry() -> Vec<Box<dyn EditorExtension>> {
    vec![Box::new(CoreEditorExtension), Box::new(UmlEditorExtension)]
}

/// The `IconId` name -> `Icon` table every icon-resolving surface (the folder
/// listing, the tree panel) draws against: `editor_registry()`'s extensions'
/// `icons()`, flattened. One function so every consumer resolves against the
/// same set `editor_registry()` names -- a stage registered in the extension
/// list but missing here would stamp a name nothing resolves.
pub fn icon_table() -> Vec<(&'static str, Icon)> {
    editor_registry()
        .into_iter()
        .flat_map(|ext| ext.icons())
        .collect()
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
    registry: &MiddlewareRegistry,
) -> (Chain, Vec<Diagnostic>) {
    match mode {
        ViewMode::Projected => analysis.bundle.resolved_view(
            directory,
            registry,
            &waml::view::mask::ProjectionMask::default(),
        ),
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
    registry: &MiddlewareRegistry,
) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)> {
    let dir: Directory = analysis.bundle.directory(directory)?.clone();
    let (chain, mut diagnostics) = chain_for(analysis, directory, mode, registry);
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
    use std::collections::BTreeSet;
    use waml::extension::CoreExtension;
    use waml::source::SourceBundle;

    /// Task 9: the middleware registry and the editor registry must name the
    /// same extension set -- two construction sites that disagree are
    /// invisible until a reader opens a row nothing can draw or resolve.
    #[test]
    fn middleware_and_editor_registries_name_the_same_extensions() {
        let middleware_names: BTreeSet<&str> = [
            waml::extension::CoreExt.name(),
            waml::extension::UmlExt.name(),
        ]
        .into_iter()
        .collect();
        let editor_names_owned: BTreeSet<String> = editor_registry()
            .into_iter()
            .map(|ext| ext.name().to_string())
            .collect();
        let editor_names: BTreeSet<&str> = editor_names_owned.iter().map(String::as_str).collect();
        assert_eq!(
            middleware_names, editor_names,
            "core_registry() and editor_registry() must name the same extension set"
        );
    }

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

        let (_, projected, diagnostics) = project_rows(
            prepared.okf(),
            "/",
            ViewMode::Projected,
            limits,
            &core_registry(),
        )
        .unwrap();
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
            project_rows(prepared.okf(), "/", ViewMode::Raw, limits, &core_registry()).unwrap();
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

        let (_, _, declared) = project_rows(
            prepared.okf(),
            "/",
            ViewMode::Projected,
            limits,
            &core_registry(),
        )
        .unwrap();
        assert!(!declared.is_empty(), "an unknown middleware name diagnoses");

        let (_, _, raw) =
            project_rows(prepared.okf(), "/", ViewMode::Raw, limits, &core_registry()).unwrap();
        assert!(raw.is_empty());
    }

    #[test]
    fn a_missing_directory_yields_none_rather_than_panicking() {
        let prepared = hidden_bundle();
        assert!(project_rows(
            prepared.okf(),
            "/missing",
            ViewMode::Projected,
            ChainLimits::default(),
            &core_registry(),
        )
        .is_none());
    }

    #[test]
    fn raw_mode_owns_every_row_through_the_root_view() {
        let prepared = hidden_bundle();
        let (_, rows, _) = project_rows(
            prepared.okf(),
            "/",
            ViewMode::Raw,
            ChainLimits::default(),
            &core_registry(),
        )
        .unwrap();
        assert!(
            rows.iter()
                .all(|row| row.id.owner.as_str() == waml::view::ROOT_VIEW_OWNER),
            "in Raw the chain is [index], so RootView owns every row",
        );
    }
}
