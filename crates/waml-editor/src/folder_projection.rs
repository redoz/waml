//! Where a folder's rows come from — for BOTH surfaces that show them.
//!
//! The folder surface (`folder_view.rs`) and the tree seam (`tree.rs`) run
//! the same chain, against the same registry, under the same mask. Two row
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
use waml::view::mask::ProjectionMask;
use waml::view::projection::ProjectionCtx;
use waml::view::row::Row;

/// The stages a reader may switch off, grouped by owning extension.
///
/// `registry.owners()` minus the terminal `index` stage: `RootView` is where a
/// chain lands whenever it runs out of declared stages, so masking `index`
/// cannot remove the listing and offering it would be a lie. An extension left
/// with nothing maskable is dropped rather than shown as an empty group.
///
/// Driven off the registry -- NOT a second hand-written extension list. Two
/// construction sites that disagree are invisible (see `editor_registry`).
pub fn maskable_names(registry: &MiddlewareRegistry) -> Vec<(&str, Vec<&str>)> {
    registry
        .owners()
        .into_iter()
        .filter_map(|(owner, names)| {
            let names: Vec<&str> = names
                .into_iter()
                .filter(|name| *name != waml::view::ROOT_VIEW_NAME)
                .collect();
            (!names.is_empty()).then_some((owner, names))
        })
        .collect()
}

/// A human label for an extension that owns maskable stages. The registry
/// name (`core`, `uml`) is an identifier, not a caption -- it is what the DSL
/// and the mask are keyed on and must never change to make a menu read better.
///
/// An unknown owner falls back to its raw name capitalised, so a new extension
/// lists sanely before anyone writes it a label.
pub fn extension_label(owner: &str) -> String {
    match owner {
        "core" => "Built-in".to_string(),
        _ => capitalise(owner),
    }
}

/// A human label for one maskable stage. Phrased as what the stage DOES while
/// it runs, because the checklist's checkmark means running (see
/// `app::menus::projection_menu_items`) -- a label naming the off-state would
/// invert with its own checkbox.
///
/// Same fallback rule as [`extension_label`].
pub fn stage_label(name: &str) -> String {
    match name {
        "hide" => "Hide marked items".to_string(),
        // Never offered (`maskable_names` filters it), listed so the mapping
        // covers every stage the core registry declares.
        "index" => "Folder listing".to_string(),
        _ => capitalise(name),
    }
}

fn capitalise(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
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

/// The chain `directory` runs under `mask`, plus any build-level diagnostics
/// (unknown middleware name, bad params) the declared chain produced.
///
/// An empty mask is exactly today's projected behaviour. A mask naming every
/// maskable stage yields the identity listing -- the chain builds, every
/// declared stage is skipped, and `RootView` owns every row.
pub fn chain_for(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mask: &ProjectionMask,
    registry: &MiddlewareRegistry,
) -> (Chain, Vec<Diagnostic>) {
    waml::view::chain::resolved_view(&analysis.bundle, directory, registry, mask)
}

/// Run `directory`'s chain under `mask` and hand back the chain itself, its
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
    mask: &ProjectionMask,
    limits: ChainLimits,
    registry: &MiddlewareRegistry,
) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)> {
    let dir: Directory = analysis.bundle.directory(directory)?.clone();
    let (chain, mut diagnostics) = chain_for(analysis, directory, mask, registry);
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
    use waml::view::mask::ProjectionMask;

    fn every_maskable_name() -> ProjectionMask {
        let registry = core_registry();
        ProjectionMask::from_names(
            maskable_names(&registry)
                .into_iter()
                .flat_map(|(_owner, names)| names)
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
        )
    }

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
    fn an_empty_mask_runs_the_declared_chain_and_a_full_mask_bypasses_it() {
        let prepared = hidden_bundle();
        let limits = ChainLimits::default();

        let (_, projected, diagnostics) = project_rows(
            prepared.okf(),
            "/",
            &ProjectionMask::default(),
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

        let (_, raw, raw_diagnostics) = project_rows(
            prepared.okf(),
            "/",
            &every_maskable_name(),
            limits,
            &core_registry(),
        )
        .unwrap();
        assert!(
            raw_diagnostics.is_empty(),
            "masking a stage is not an author error"
        );
        assert_eq!(
            raw.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
            vec!["Orders", "References"],
            "a full mask is presentational reachability, not a permission decision",
        );
    }

    #[test]
    fn masking_only_hide_leaves_every_other_stage_running() {
        let prepared = hidden_bundle();
        let (_, rows, diagnostics) = project_rows(
            prepared.okf(),
            "/",
            &ProjectionMask::from_names(["hide"]),
            ChainLimits::default(),
            &core_registry(),
        )
        .unwrap();
        assert!(diagnostics.is_empty());
        assert_eq!(
            rows.iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Orders", "References"],
            "`hide` is the only stage this folder declares, so its rows come back",
        );
    }

    /// An unknown declared name is not in the maskable universe -- only names
    /// the registry offers are -- so a FULL mask cannot bypass it: the chain
    /// is still built and still diagnoses. This is the raw-mode behaviour the
    /// binary `ViewMode` used to swallow, and the fence against re-silencing
    /// it by widening what a full mask covers.
    #[test]
    fn an_unknown_middleware_name_still_diagnoses_under_every_mask() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: nonexistent\n---\n# Root\n\n* [Orders](orders.md)\n",
            ),
            ("orders.md", "# Orders\n"),
        ]);
        for mask in [ProjectionMask::default(), every_maskable_name()] {
            let (_, _, diagnostics) = project_rows(
                prepared.okf(),
                "/",
                &mask,
                ChainLimits::default(),
                &core_registry(),
            )
            .unwrap();
            assert!(
                !diagnostics.is_empty(),
                "an unknown middleware name diagnoses under mask {mask:?}",
            );
        }
    }

    #[test]
    fn a_missing_directory_yields_none_rather_than_panicking() {
        let prepared = hidden_bundle();
        assert!(project_rows(
            prepared.okf(),
            "/missing",
            &ProjectionMask::default(),
            ChainLimits::default(),
            &core_registry(),
        )
        .is_none());
    }

    #[test]
    fn a_full_mask_leaves_every_row_owned_by_the_root_view() {
        let prepared = hidden_bundle();
        let (_, rows, _) = project_rows(
            prepared.okf(),
            "/",
            &every_maskable_name(),
            ChainLimits::default(),
            &core_registry(),
        )
        .unwrap();
        assert!(
            rows.iter()
                .all(|row| row.id.owner.as_str() == waml::view::ROOT_VIEW_OWNER),
            "with every declared stage masked the chain is empty, so RootView owns every row",
        );
    }

    #[test]
    fn index_is_never_offered_as_maskable() {
        let registry = core_registry();
        let offered: Vec<&str> = maskable_names(&registry)
            .into_iter()
            .flat_map(|(_owner, names)| names)
            .collect();
        assert!(
            !offered.contains(&"index"),
            "`index` is the terminal stage; masking it cannot remove the listing",
        );
        assert!(offered.contains(&"hide"));
        // `uml` is no longer a middleware stage -- the box glyph is a profile property.
    }

    #[test]
    fn an_extension_toggle_masks_exactly_that_extensions_names() {
        let registry = core_registry();
        let core_names: Vec<&str> = maskable_names(&registry)
            .into_iter()
            .find(|(owner, _)| *owner == "core")
            .expect("core owns at least one maskable stage")
            .1;
        let mask = ProjectionMask::from_names(core_names.iter().map(|n| n.to_string()));
        assert!(mask.is_masked("hide"));
        assert!(
            !mask.is_masked("uml"),
            "an extension toggle must not reach another extension's stages",
        );
    }

    /// Every name the shipped registry offers has a written caption. The
    /// fallback exists for an extension nobody has labelled yet, not for the
    /// ones we ship: a menu row reading `hide` is a leaked identifier.
    #[test]
    fn every_shipped_maskable_name_has_a_written_label() {
        let registry = core_registry();
        for (owner, names) in maskable_names(&registry) {
            assert_ne!(
                extension_label(owner),
                capitalise(owner),
                "extension `{owner}` is still falling back to its registry name",
            );
            for name in names {
                assert_ne!(
                    stage_label(name),
                    capitalise(name),
                    "stage `{name}` is still falling back to its registry name",
                );
            }
        }
    }

    /// An extension nobody has written labels for still lists sanely rather
    /// than showing a lowercase identifier row.
    #[test]
    fn an_unknown_name_falls_back_to_its_capitalised_self() {
        assert_eq!(extension_label("erd"), "Erd");
        assert_eq!(stage_label("collapse"), "Collapse");
        assert_eq!(stage_label(""), "");
    }
}
