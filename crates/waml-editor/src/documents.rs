use crate::document::{DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument};
use crate::view_history::DocumentLocator;
#[cfg(test)]
use waml::view::row::Row;
use waml::view::row::RowTarget;

/// THE tab identity: one function over the locator (spec §3). The target
/// discriminant is baked into the string so a folder "/x" and a concept
/// "x" can never collide, and two surfaces of one target stay two tabs.
pub fn tab_id_for(locator: &DocumentLocator) -> makepad_widgets::LiveId {
    let target = match &locator.target {
        RowTarget::Concept(id) => format!("c:{id}"),
        RowTarget::Folder(address) => format!("f:{address}"),
        RowTarget::Virtual => "v:".to_string(),
    };
    makepad_widgets::LiveId::from_str(&format!(
        "__doc_tab__{}__{target}",
        locator.surface.as_str()
    ))
}

pub fn describe(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<DocumentDescriptor> {
    crate::uml_documents::describe(okf, uml, concept_id)
        .or_else(|| crate::okf_documents::describe(okf, concept_id))
}

/// THE combined uml-then-generic open: the canvas surface factory
/// (`extension_editor::open_canvas`) calls this exact function, and the
/// behavior-preservation tests (`uml_provider_precedes_generic_okf_provider`
/// et al.) pin this exact function -- one definition, so the live open path
/// and its pinned baseline cannot drift.
pub fn open_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
) -> Option<OpenDocument> {
    crate::uml_documents::open_with_asset_host(okf, uml, concept_id, assets, emphasis)
        .or_else(|| crate::okf_documents::open_with_asset_host(okf, concept_id, assets, emphasis))
}

/// The surface a target opens on when nothing requests one. "canvas" iff
/// the UML analysis claims the concept (the claim set is what decides
/// provider ownership today -- NOT ElementType parsing, which can disagree
/// on invalid-but-claimed documents); "markdown" otherwise; "folder" for a
/// folder. Virtual has no default (Row::new enforces an explicit surface).
pub fn default_surface_for(
    _okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    target: &RowTarget,
) -> waml::view::surface::SurfaceId {
    match target {
        RowTarget::Folder(_) => waml::view::surface::SurfaceId::folder(),
        RowTarget::Concept(id) if uml.claims.contains(id) => {
            waml::view::surface::SurfaceId::canvas()
        }
        RowTarget::Concept(_) | RowTarget::Virtual => waml::view::surface::SurfaceId::markdown(),
    }
}

/// The five registered surface ids (Task E2, widened by the book-mode
/// spec), for `resolve_surface`'s `known` slice. Kept beside the call site
/// so it cannot silently drift from `CoreEditorExtension::surfaces` --
/// `extension_editor`'s own gate test
/// (`todays_five_surfaces_are_registered_by_the_core_editor_half`) asserts
/// the other side of the same set.
pub(crate) const KNOWN_SURFACES: &[&str] = &["markdown", "source", "canvas", "folder", "book"];

/// The editor's ONE surface resolution: `waml::view::surface::resolve_surface`
/// itself (a `None` request or a registered id resolves silently; an unknown id
/// degrades with an `UnknownSurface` warning), given the editor's claims-based
/// `default_surface_for` as the default/degrade target rather than the core's
/// `ElementType::parse`-based `default_surface` -- the core has no UML analysis
/// to consult and so can disagree on an invalid-but-claimed document. Every
/// editor-side resolution goes through here so a stale or unknown stored
/// surface can never degrade to a different surface than `primary_locator`
/// would have chosen for the same target.
pub(crate) fn resolve_surface_for(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    requested: Option<&str>,
    target: &RowTarget,
    file: &str,
    line: usize,
) -> (
    waml::view::surface::SurfaceId,
    Option<waml::diagnostic::Diagnostic>,
) {
    waml::view::surface::resolve_surface(
        requested,
        default_surface_for(okf, uml, target),
        KNOWN_SURFACES,
        file,
        line,
    )
}

/// Cheap existence probe: would `open_locator_with_asset_host` produce a
/// document for this locator? Answers the same question WITHOUT resolving a
/// chain, projecting rows, or allocating a view -- the history controls ask it
/// twice per sync (and a sync runs on every shell/workspace refresh), where
/// building and dropping a `FolderView` per stored location is pure waste.
///
/// The arms mirror the surface table's factories exactly; the parity is
/// pinned by `the_open_probe_agrees_with_the_open_path_on_every_surface`, so
/// the two cannot drift.
pub fn locator_opens(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    locator: &DocumentLocator,
) -> bool {
    // A search-results locator (decision 7) has no factory in the surface
    // table -- it is rebuilt by re-running the query, not by resolving a
    // concept/folder/source surface -- so it must be recognized before the
    // claims-based resolution below, which would otherwise degrade its
    // unknown `search:{query}` surface to the target's ordinary default.
    if search_query_from_locator(locator).is_some() {
        return matches!(locator.target, RowTarget::Virtual);
    }
    let (surface, _diagnostic) = resolve_surface_for(
        okf,
        uml,
        Some(locator.surface.as_str()),
        &locator.target,
        "index.md",
        0,
    );
    match (surface.as_str(), &locator.target) {
        // `open_markdown`/`open_canvas` both bottom out in a bundle concept
        // lookup (the uml provider's own gate is the same lookup, and its
        // generic fallback covers every concept it declines).
        ("markdown" | "canvas", RowTarget::Concept(id)) => okf.bundle.concept(id).is_some(),
        // `open_folder` -> `folder_documents::open`, whose only `None` is a
        // directory that is not in the bundle (`FolderView::build` fails on
        // exactly the same missing directory).
        ("folder", RowTarget::Folder(directory)) => okf.bundle.directory(directory).is_some(),
        // `open_book` -> `book_documents::open`, whose only `None` is a
        // directory not in the bundle -- the same probe `folder` uses.
        ("book", RowTarget::Folder(directory)) => okf.bundle.directory(directory).is_some(),
        ("source", target) => crate::okf_documents::source_opens_for_target(okf, target),
        _ => false,
    }
}

/// Opens a projected chain `Row` (Task E2's "open rows through the surface
/// table"), honoring an explicit `row.surface` override (a future
/// middleware's `surface:` declaration -- unreachable today, since neither
/// `hide` nor the root view sets one, but this is the seam Task E3's
/// `member:`/`markdown` resolutions and any later middleware plug into).
///
/// A row with no override (`row.surface == None`) -- today's only reachable
/// case -- delegates unchanged to `open_with_asset_host`/`open_folder`, so
/// this function is byte-for-byte behavior-preserving for every row the
/// editor can currently produce. An explicit override dispatches to the
/// matching surface directly, bypassing the uml-vs-generic type detection
/// `open_with_asset_host` otherwise performs; an unknown override id
/// degrades to the row's type default with a diagnostic (never a blank
/// tab, never a panic) per `waml::view::surface::resolve_surface`.
///
/// The locator path (`open_locator_with_asset_host`) is live through the
/// surface table today; this row-click path still routes through
/// `NavigationTarget`, which has no surface field, so an override can only
/// be exercised directly (as this module's own tests do) until a variant or
/// field is added to carry a resolved `SurfaceId` end to end (Task 6). Every
/// row the editor can produce today has `surface: None` and is unaffected
/// either way -- so this entry point still has no live (non-test) caller
/// until Task 6 wires `folder_view.rs`'s click-through path to it.
#[cfg(test)]
pub fn open_row_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    row: &Row,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> (Option<OpenDocument>, Option<waml::diagnostic::Diagnostic>) {
    let (surface, diagnostic) = resolve_surface_for(
        okf,
        uml,
        row.surface.as_ref().map(|s| s.0.as_str()),
        &row.target,
        "index.md",
        0,
    );
    let ctx = crate::extension_editor::OpenCtx {
        analysis: okf,
        uml,
        assets: assets.clone(),
        emphasis,
        limits,
        mask,
    };
    let doc = open_on_surface(&ctx, &row.target, &surface);
    (doc, diagnostic)
}

/// The one surface dispatch both entry shapes share: look the resolved
/// surface up in this build's registered table and hand the target to its
/// factory. Kept private and single so the row path and the locator path
/// cannot drift on what "open on surface S" means.
fn open_on_surface(
    ctx: &crate::extension_editor::OpenCtx<'_>,
    target: &RowTarget,
    surface: &waml::view::surface::SurfaceId,
) -> Option<OpenDocument> {
    crate::extension_editor::surface_table()
        .into_iter()
        .find(|(name, _)| *name == surface.as_str())
        .and_then(|(_, factory)| factory(ctx, target))
}

/// The folder-view provider entry: keyed on a directory address, not a
/// concept id, so it sits beside `open_with_asset_host` rather than inside
/// its `.or_else` chain -- a folder and a concept never share a key. Called
/// from `App::navigate_with`'s `Directory` arm.
pub fn open_folder(
    okf: &waml::analysis::OkfAnalysis,
    directory: &str,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    crate::folder_documents::open(okf, directory, limits, mask)
}

/// The query string baked into a search-results locator's surface
/// (`"search:{query}"`, decision 7), or `None` for any other locator. A
/// search tab's target is always `RowTarget::Virtual` -- `Virtual` has no
/// default surface (`default_surface_for`), so an unrecognized `search:`
/// prefix must never reach the claims-based resolution the other surfaces
/// go through.
pub fn search_query_from_locator(locator: &DocumentLocator) -> Option<&str> {
    if !matches!(locator.target, RowTarget::Virtual) {
        return None;
    }
    locator.surface.as_str().strip_prefix("search:")
}

/// Builds the results-tab `OpenDocument` for `query`'s already-computed rows
/// (decision 7): a `RowTarget::Virtual` locator whose surface bakes the
/// query in, so `tab_id_for` gives the same tab id to a re-run of the same
/// query -- re-activating its tab rather than duplicating it. Sits beside
/// `open_folder` as the results-tab counterpart: keyed on a query string
/// rather than a directory address or concept id, so it cannot collide with
/// either. Called by `App::open_search_results` and by the locator-driven
/// reopen path (history traversal) that re-runs the query off the surface
/// string.
pub fn open_search(query: &str, rows: Vec<crate::search_results_view::ResultRow>) -> OpenDocument {
    let locator = DocumentLocator::new(
        RowTarget::Virtual,
        waml::view::surface::SurfaceId(format!("search:{query}")),
    );
    OpenDocument {
        tab_id: tab_id_for(&locator),
        locator,
        title: format!("Search: {query}"),
        presentation: DocumentPresentation {
            icon: crate::icons::Icon::Search,
            accent: None,
            category: NavCategory::Note,
        },
        view: Box::new(crate::search_results_view::SearchResultsView::new(
            query.to_string(),
            rows,
        )),
    }
}

#[cfg(test)]
pub fn open(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    open_with_asset_host(
        okf,
        uml,
        concept_id,
        &assets_for_test(),
        waml_markdown_editor::EditorEmphasis::Code,
    )
}

pub fn reopen_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    tab: &crate::doc_tabs::DocTab,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    open_locator_with_asset_host(okf, uml, &tab.locator(), assets, emphasis, limits, mask)
}

/// Opens a stored `DocumentLocator` by looking its surface up in the editor
/// build's registered surface table (`extension_editor::surface_table`) and
/// calling the matching factory with the locator's target. Keyed on
/// `RowTarget`, never on row/tab identity -- a `hide`-hidden concept has no
/// `RowId` in Projected mode, so this is the only open path that can reach
/// it in both view modes (see `a_hidden_concept_still_opens_through_the_surface_path_in_both_modes`).
/// An unknown surface id degrades via `resolve_surface_for`'s claims-based
/// default -- the same one `App::primary_locator` uses -- rather than
/// returning `None`: never a blank tab, and never a different surface than a
/// fresh click on the same target would have opened.
pub fn open_locator_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    locator: &DocumentLocator,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    emphasis: waml_markdown_editor::EditorEmphasis,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    let (surface, _diagnostic) = resolve_surface_for(
        okf,
        uml,
        Some(locator.surface.as_str()),
        &locator.target,
        "index.md",
        0,
    );
    let ctx = crate::extension_editor::OpenCtx {
        analysis: okf,
        uml,
        assets: assets.clone(),
        emphasis,
        limits,
        mask,
    };
    open_on_surface(&ctx, &locator.target, &surface)
}

#[cfg(test)]
fn assets_for_test() -> crate::markdown_hosts::SharedMarkdownAssetHost {
    crate::markdown_hosts::EditorMarkdownAssetHost::shared(
        crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_maskable_name() -> waml::view::mask::ProjectionMask {
        let registry = crate::folder_projection::core_registry();
        waml::view::mask::ProjectionMask::from_names(
            crate::folder_projection::maskable_names(&registry)
                .into_iter()
                .flat_map(|(_owner, names)| names)
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
        )
    }

    use crate::{
        document::{DocumentCapabilities, DocumentPresentation, NavCategory},
        icons::Icon,
    };
    use waml::source::SourceBundle;

    fn assets() -> crate::markdown_hosts::SharedMarkdownAssetHost {
        crate::markdown_hosts::EditorMarkdownAssetHost::shared(
            crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
        )
    }

    fn test_emphasis() -> waml_markdown_editor::EditorEmphasis {
        waml_markdown_editor::EditorEmphasis::Code
    }

    fn future_sibling_descriptor() -> DocumentDescriptor {
        DocumentDescriptor {
            presentation: DocumentPresentation {
                icon: Icon::StickyNote,
                accent: None,
                category: NavCategory::OkfDocument,
            },
            capabilities: DocumentCapabilities::default(),
        }
    }

    #[test]
    fn uml_provider_precedes_generic_okf_provider() {
        let source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 7).unwrap();

        assert!(crate::uml_documents::open(prepared.okf(), prepared.uml(), "order").is_some());
        assert!(crate::okf_documents::open_with_asset_host(
            prepared.okf(),
            "order",
            &assets(),
            test_emphasis(),
        )
        .is_some());
        assert_eq!(
            open_with_asset_host(
                prepared.okf(),
                prepared.uml(),
                "order",
                &assets(),
                test_emphasis(),
            )
            .unwrap()
            .tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::canvas()
            ))
        );
        assert!(crate::uml_documents::open(prepared.okf(), prepared.uml(), "runbook").is_none());
        assert_eq!(
            open_with_asset_host(
                prepared.okf(),
                prepared.uml(),
                "runbook",
                &assets(),
                test_emphasis(),
            )
            .unwrap()
            .tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "runbook",
                waml::view::surface::SurfaceId::markdown()
            ))
        );

        let (generic_tab, _) = open_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            "runbook",
            &assets(),
            test_emphasis(),
        )
        .unwrap()
        .into_tab(true);
        assert_eq!(
            reopen_with_asset_host(
                prepared.okf(),
                prepared.uml(),
                &generic_tab,
                &assets(),
                test_emphasis(),
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap()
            .tab_id,
            generic_tab.id
        );

        let (source_tab, _) = crate::okf_documents::open_source_with_asset_host(
            prepared.okf(),
            "runbook",
            &assets(),
            test_emphasis(),
        )
        .unwrap()
        .into_tab(false);
        assert_eq!(
            reopen_with_asset_host(
                prepared.okf(),
                prepared.uml(),
                &source_tab,
                &assets(),
                test_emphasis(),
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap()
            .tab_id,
            source_tab.id
        );
    }

    #[test]
    fn layout_emphasis_reaches_direct_and_reopened_source_factories() {
        let source =
            SourceBundle::try_from_pairs([("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 8).unwrap();
        let assets = assets();
        let direct = crate::okf_documents::open_source_with_asset_host(
            prepared.okf(),
            "runbook",
            &assets,
            waml_markdown_editor::EditorEmphasis::Layout,
        )
        .expect("the direct source factory must open the document");
        // A source tab opens on the source FACE of the shared view: the
        // rendered surface is one `view_toggle` click away, and the session's
        // layout emphasis reaches the mode button beside it.
        assert_eq!(
            direct.view.chrome().document_header.view_toggle,
            Some(crate::doc_view::HeaderViewAction::destination(
                Icon::Eye,
                "View rendered"
            ))
        );
        assert_eq!(
            direct.view.chrome().document_header.emphasis_toggle,
            Some(crate::doc_view::HeaderViewAction {
                icon: Icon::Paintbrush,
                tooltip: "Use code emphasis",
                active: true,
            })
        );

        let (source_tab, _) = direct.into_tab(false);
        let reopened = reopen_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &source_tab,
            &assets,
            waml_markdown_editor::EditorEmphasis::Layout,
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .expect("the stored source locator must reopen the document");
        assert_eq!(
            reopened.view.chrome().document_header.emphasis_toggle,
            Some(crate::doc_view::HeaderViewAction {
                icon: Icon::Paintbrush,
                tooltip: "Use code emphasis",
                active: true,
            })
        );
    }

    #[test]
    fn sibling_descriptor_stays_outside_static_uml_generic_selection() {
        let descriptor = future_sibling_descriptor();
        assert_eq!(descriptor.presentation.category, NavCategory::OkfDocument);

        let source = SourceBundle::try_from_pairs([(
            "widget.md",
            "---\ntype: future.Widget\n---\n# Widget\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 9).unwrap();
        assert!(crate::uml_documents::describe(prepared.okf(), prepared.uml(), "widget").is_none());
        assert_eq!(
            describe(prepared.okf(), prepared.uml(), "widget").unwrap(),
            crate::okf_documents::describe(prepared.okf(), "widget").unwrap()
        );
    }

    #[test]
    fn invalid_claimed_uml_stays_owned_and_exposes_revision_bound_repairs() {
        let source = SourceBundle::try_from_pairs([(
            "broken.md",
            "---\ntype: uml.Class\n---\n# Broken\n\n## Attributes\n- name String [oops 42]\n",
        )])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 11).unwrap();

        let document = open_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            "broken",
            &assets(),
            test_emphasis(),
        )
        .unwrap();
        assert_eq!(
            document.tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "broken",
                waml::view::surface::SurfaceId::canvas()
            ))
        );
        let id = prepared
            .okf()
            .catalog
            .id_for_path(&waml::source::BundlePath::parse("broken.md").unwrap())
            .unwrap();
        let context =
            waml::uml::ActionContext::new(prepared.okf(), prepared.uml(), prepared.revision())
                .unwrap();
        let actions = waml::uml::repair_actions(context, id).unwrap();
        assert!(actions
            .iter()
            .any(|action| action.title == "Insert missing `: `"));
        assert!(actions
            .iter()
            .any(|action| action.title == "Replace invalid multiplicity"));
    }

    #[test]
    fn indexes_and_logs_are_not_openable_as_concepts() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Index\n- [Runbook](./runbook.md)\n"),
            ("log.md", "---\ntype: Log\n---\n# Log\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 13).unwrap();

        assert!(open_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            "index",
            &assets(),
            test_emphasis(),
        )
        .is_none());
        assert!(open_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            "log",
            &assets(),
            test_emphasis(),
        )
        .is_none());
    }

    #[test]
    fn default_surface_for_matches_claimed_uml_generic_and_folder_targets() {
        let source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 15).unwrap();

        assert_eq!(
            default_surface_for(
                prepared.okf(),
                prepared.uml(),
                &RowTarget::Concept("order".to_string())
            ),
            waml::view::surface::SurfaceId::canvas()
        );
        assert_eq!(
            default_surface_for(
                prepared.okf(),
                prepared.uml(),
                &RowTarget::Concept("runbook".to_string())
            ),
            waml::view::surface::SurfaceId::markdown()
        );
        assert_eq!(
            default_surface_for(
                prepared.okf(),
                prepared.uml(),
                &RowTarget::Folder("/".to_string())
            ),
            waml::view::surface::SurfaceId::folder()
        );
    }

    /// The degrade arm must use the SAME "default surface" definition as
    /// `App::primary_locator` (claims-based `default_surface_for`), not the
    /// core's `ElementType::parse`-based `default_surface`. The two disagree
    /// exactly on a document the UML analysis claims but whose bundle entry
    /// does not parse as a UML/diagram type -- and an unknown or stale stored
    /// surface must not land on a different surface than a fresh click would.
    #[test]
    fn an_unknown_surface_degrades_to_the_same_default_as_a_fresh_click() {
        let claimed =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let claimed = waml::analysis::prepare_candidate(claimed, None, 51).unwrap();
        // A bundle where `order` is NOT a diagram-like type: the core's
        // parse-based default says `markdown` for this okf, while the UML
        // analysis above still claims the concept.
        let plain =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: Runbook\n---\n# Order\n")])
                .unwrap();
        let plain = waml::analysis::prepare_candidate(plain, None, 53).unwrap();

        let target = RowTarget::Concept("order".to_string());
        assert_eq!(
            waml::view::surface::default_surface(&target, &plain.okf().bundle),
            waml::view::surface::SurfaceId::markdown(),
            "fixture must actually make the two definitions disagree"
        );
        let fresh = default_surface_for(plain.okf(), claimed.uml(), &target);
        assert_eq!(fresh, waml::view::surface::SurfaceId::canvas());

        let (degraded, diagnostic) = resolve_surface_for(
            plain.okf(),
            claimed.uml(),
            Some("no-such-surface"),
            &target,
            "index.md",
            0,
        );
        assert!(diagnostic.is_some());
        assert_eq!(degraded, fresh);

        let (defaulted, diagnostic) =
            resolve_surface_for(plain.okf(), claimed.uml(), None, &target, "index.md", 0);
        assert!(diagnostic.is_none());
        assert_eq!(defaulted, fresh);
    }

    /// The history controls probe with `locator_opens` instead of opening;
    /// the two must answer identically for every (target, surface) pair,
    /// including the degrade arms.
    #[test]
    fn the_open_probe_agrees_with_the_open_path_on_every_surface() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Shop](shop/)\n* [Loose](loose/)\n"),
            ("shop/index.md", "# Shop\n"),
            ("loose/thing.md", "---\ntype: Runbook\n---\n# Thing\n"),
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 61).unwrap();

        let targets = [
            RowTarget::Concept("order".to_string()),
            RowTarget::Concept("runbook".to_string()),
            RowTarget::Concept("loose/thing".to_string()),
            RowTarget::Concept("no-such-concept".to_string()),
            RowTarget::Folder("/".to_string()),
            RowTarget::Folder("/shop".to_string()),
            RowTarget::Folder("/loose".to_string()),
            RowTarget::Folder("/nowhere".to_string()),
            RowTarget::Virtual,
        ];
        for target in targets {
            for surface in [
                "markdown",
                "source",
                "canvas",
                "folder",
                "book",
                "no-such-surface",
            ] {
                let locator = DocumentLocator::new(
                    target.clone(),
                    waml::view::surface::SurfaceId(surface.to_string()),
                );
                let opened = open_locator_with_asset_host(
                    prepared.okf(),
                    prepared.uml(),
                    &locator,
                    &assets(),
                    test_emphasis(),
                    waml::view::chain::ChainLimits::default(),
                    &waml::view::mask::ProjectionMask::default(),
                )
                .is_some();
                assert_eq!(
                    locator_opens(prepared.okf(), prepared.uml(), &locator),
                    opened,
                    "probe disagrees with the open path for {target:?} on `{surface}`"
                );
            }
        }
    }

    /// `waml::view::chain` stamps surfaces with the core's parse-based
    /// `default_surface` (it has no UML analysis); the editor resolves with
    /// the claims-based `default_surface_for`. For any ONE analysis pair the
    /// two definitions must agree, or a projected row would carry a surface
    /// the editor's own resolution disowns (the disagreement the degrade test
    /// above stages is only reachable by crossing two different bundles).
    #[test]
    fn the_row_stamped_surface_agrees_with_the_editors_default_for_one_analysis() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Shop](shop/)\n"),
            ("shop/index.md", "# Shop\n"),
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
            ("flow.md", "---\ntype: uml.ActivityDiagram\n---\n# Flow\n"),
            ("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n"),
            ("plain.md", "# Plain\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 63).unwrap();

        let mut targets: Vec<RowTarget> = prepared
            .okf()
            .bundle
            .concepts()
            .iter()
            .map(|concept| RowTarget::Concept(concept.id.clone()))
            .collect();
        targets.push(RowTarget::Folder("/".to_string()));
        targets.push(RowTarget::Folder("/shop".to_string()));
        assert!(targets.len() > 2);
        for target in targets {
            assert_eq!(
                default_surface_for(prepared.okf(), prepared.uml(), &target),
                waml::view::surface::default_surface(&target, &prepared.okf().bundle),
                "{target:?} is stamped with a surface the editor would not choose"
            );
        }
    }

    fn test_row(target: waml::view::row::RowTarget, surface: Option<&str>) -> Row {
        Row::new(
            waml::view::row::RowId {
                owner: waml::view::row::ViewId::new("root"),
                path: waml::view::row::RowPath::parse("x").unwrap(),
            },
            "x".to_string(),
            target,
            surface.map(|s| waml::view::surface::SurfaceId(s.to_string())),
        )
        .unwrap()
    }

    #[test]
    fn a_row_with_no_surface_override_delegates_unchanged_to_open_with_asset_host() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let row = test_row(RowTarget::Concept("order".to_string()), None);

        let (doc, diagnostic) = open_row_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &row,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        );
        assert!(diagnostic.is_none());
        assert_eq!(
            doc.unwrap().tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::canvas()
            ))
        );
    }

    #[test]
    fn an_explicit_surface_override_bypasses_uml_type_detection() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 3).unwrap();
        let row = test_row(RowTarget::Concept("order".to_string()), Some("source"));

        let (doc, diagnostic) = open_row_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &row,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        );
        assert!(diagnostic.is_none());
        assert_eq!(
            doc.unwrap().tab_id,
            crate::documents::tab_id_for(&DocumentLocator::source("order"))
        );
    }

    #[test]
    fn an_unknown_surface_override_degrades_with_a_diagnostic() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 5).unwrap();
        let row = test_row(
            RowTarget::Concept("order".to_string()),
            Some("no-such-surface"),
        );

        let (doc, diagnostic) = open_row_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &row,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        );
        assert!(diagnostic.is_some());
        // Degrades to the type default -- uml.Class -> canvas -> the same
        // uml-aware open path as the unadorned case.
        assert_eq!(
            doc.unwrap().tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::canvas()
            ))
        );
    }

    #[test]
    fn a_folder_row_opens_through_open_folder() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 7).unwrap();
        let row = test_row(RowTarget::Folder("/sales".to_string()), None);

        let (doc, diagnostic) = open_row_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &row,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        );
        assert!(diagnostic.is_none());
        assert!(doc.is_some());
    }

    #[test]
    fn a_virtual_row_has_nothing_to_open() {
        let source = SourceBundle::try_from_pairs([("index.md", "# Root\n")]).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 9).unwrap();
        let row = test_row(waml::view::row::RowTarget::Virtual, Some("markdown"));

        let (doc, diagnostic) = open_row_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &row,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        );
        assert!(diagnostic.is_none());
        assert!(doc.is_none());
    }

    #[test]
    fn a_folder_tabs_locator_reopens_the_folder_view() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 21).unwrap();
        let (folder_tab, _) = open_folder(
            prepared.okf(),
            "/sales",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .unwrap()
        .into_tab(true);

        let reopened = reopen_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &folder_tab,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .expect("a folder tab's locator must resolve (spike Q5: today it never does)");
        assert_eq!(reopened.tab_id, folder_tab.id);
    }

    #[test]
    fn locator_reopens_the_correct_view_after_transient_tab_identity_is_gone() {
        let source =
            SourceBundle::try_from_pairs([("runbook.md", "---\ntype: Runbook\n---\n# Runbook\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 17).unwrap();
        let (mut old_source_tab, _) = crate::okf_documents::open_source_with_asset_host(
            prepared.okf(),
            "runbook",
            &assets(),
            test_emphasis(),
        )
        .unwrap()
        .into_tab(false);
        old_source_tab.id = makepad_widgets::LiveId::from_str("closed-transient-tab");

        let reopened = reopen_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &old_source_tab,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .unwrap();

        assert_eq!(reopened.locator(), old_source_tab.locator());
        assert_eq!(
            reopened.locator.surface,
            waml::view::surface::SurfaceId::source()
        );
        assert_eq!(
            reopened.tab_id,
            crate::documents::tab_id_for(&DocumentLocator::source("runbook"))
        );
    }

    /// `hide` is presentational, never a permission boundary (hide.rs:7). A
    /// concept a middleware hides has NO RowId in Projected mode (spike Q2),
    /// so any open path keyed on row identity would make it unopenable. The
    /// locator path must open it in BOTH modes.
    #[test]
    fn a_hidden_concept_still_opens_through_the_surface_path_in_both_modes() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Shop](shop/)\n"),
            (
                "shop/index.md",
                "---\nview: hide\nhide: [\"shop/secret\"]\n---\n# Shop\n\n* [Order](order.md)\n* [Secret](secret.md)\n",
            ),
            ("shop/order.md", "---\ntype: Runbook\n---\n# Order\n"),
            ("shop/secret.md", "---\ntype: Runbook\n---\n# Secret\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 41).unwrap();
        let locator =
            DocumentLocator::concept("shop/secret", waml::view::surface::SurfaceId::markdown());
        // Both ends of the projection range: nothing masked (the chain runs,
        // so `hide` swallows the row) and every maskable stage masked (raw).
        for mask in [
            waml::view::mask::ProjectionMask::default(),
            every_maskable_name(),
        ] {
            assert!(
                open_locator_with_asset_host(
                    prepared.okf(),
                    prepared.uml(),
                    &locator,
                    &assets(),
                    test_emphasis(),
                    waml::view::chain::ChainLimits::default(),
                    &mask,
                )
                .is_some(),
                "hide must stay presentational with mask {mask:?}"
            );
        }
        // The source surface of a hidden concept opens too.
        assert!(open_locator_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &DocumentLocator::source("shop/secret"),
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .is_some());
    }

    #[test]
    fn an_unknown_surface_locator_degrades_to_the_type_default() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 43).unwrap();
        let locator = DocumentLocator::new(
            RowTarget::Concept("order".to_string()),
            waml::view::surface::SurfaceId("no-such-surface".to_string()),
        );

        let doc = open_locator_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &locator,
            &assets(),
            test_emphasis(),
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .expect("an unknown surface must degrade, never open nothing");
        assert_eq!(
            doc.tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::canvas()
            ))
        );
    }

    fn search_locator(query: &str) -> DocumentLocator {
        DocumentLocator::new(
            RowTarget::Virtual,
            waml::view::surface::SurfaceId(format!("search:{query}")),
        )
    }

    #[test]
    fn tab_id_for_gives_distinct_stable_ids_for_two_queries() {
        let payment = tab_id_for(&search_locator("payment"));
        let payment_again = tab_id_for(&search_locator("payment"));
        let capture = tab_id_for(&search_locator("capture"));
        assert_eq!(payment, payment_again);
        assert_ne!(payment, capture);
    }

    #[test]
    fn locator_opens_accepts_a_search_locator() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 71).unwrap();

        assert!(locator_opens(
            prepared.okf(),
            prepared.uml(),
            &search_locator("payment")
        ));
        assert!(locator_opens(
            prepared.okf(),
            prepared.uml(),
            &search_locator("")
        ));
    }

    #[test]
    fn a_search_shaped_surface_on_a_non_virtual_target_is_not_the_search_arm() {
        // A `search:`-prefixed surface only means something on a `Virtual`
        // target (decision 7) -- on any other target it is just an
        // unrecognized surface string, falling through the ordinary
        // claims-based degrade like any other unknown surface.
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 73).unwrap();

        assert!(!locator_opens(
            prepared.okf(),
            prepared.uml(),
            &DocumentLocator::new(
                RowTarget::Concept("no-such-concept".to_string()),
                waml::view::surface::SurfaceId("search:payment".to_string()),
            )
        ));
    }

    #[test]
    fn search_query_from_locator_extracts_the_query_and_rejects_other_locators() {
        assert_eq!(
            search_query_from_locator(&search_locator("payment")),
            Some("payment")
        );
        assert_eq!(
            search_query_from_locator(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::markdown()
            )),
            None
        );
    }

    #[test]
    fn open_search_bakes_the_query_into_the_locator_and_title() {
        let document = open_search("payment", Vec::new());
        assert_eq!(document.title, "Search: payment");
        assert_eq!(document.tab_id, tab_id_for(&search_locator("payment")));
        assert_eq!(
            search_query_from_locator(&document.locator),
            Some("payment")
        );
    }
}
