use crate::document::{DocumentDescriptor, OpenDocument};
use crate::view_history::DocumentLocator;
use waml::view::row::{Row, RowTarget};

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

/// The combined uml-then-generic open, kept as a directly callable unit so
/// the behavior-preservation tests (`uml_provider_precedes_generic_okf_provider`
/// et al.) can pin its tab identity independent of the surface table's own
/// wiring in `open_locator_with_asset_host`/`open_row_with_asset_host` --
/// both of which now route through `extension_editor::surface_table`'s
/// `open_canvas` (which folds in the same uml-then-generic degrade) rather
/// than calling this function. Not dead: exercised directly by this
/// module's tests as the behavior baseline the table must match.
#[allow(dead_code)]
pub fn open_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
) -> Option<OpenDocument> {
    crate::uml_documents::open_with_asset_host(okf, uml, concept_id, assets)
        .or_else(|| crate::okf_documents::open_with_asset_host(okf, concept_id, assets))
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

/// The four registered surface ids (Task E2), for `resolve_surface`'s
/// `known` slice. Kept beside the call site so it cannot silently drift
/// from `CoreEditorExtension::surfaces` -- `extension_editor`'s own gate
/// test (`todays_four_surfaces_are_registered_by_the_core_editor_half`)
/// asserts the other side of the same set.
pub(crate) const KNOWN_SURFACES: &[&str] = &["markdown", "source", "canvas", "folder"];

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
#[allow(dead_code)]
pub fn open_row_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    row: &Row,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> (Option<OpenDocument>, Option<waml::diagnostic::Diagnostic>) {
    let (surface, diagnostic) = waml::view::surface::resolve_surface(
        row.surface.as_ref().map(|s| s.0.as_str()),
        &row.target,
        &okf.bundle,
        KNOWN_SURFACES,
        "index.md",
        0,
    );
    let ctx = crate::extension_editor::OpenCtx {
        analysis: okf,
        uml,
        assets: assets.clone(),
        limits,
        mask,
    };
    let doc = crate::extension_editor::surface_table()
        .into_iter()
        .find(|(name, _)| *name == surface.as_str())
        .and_then(|(_, factory)| factory(&ctx, &row.target));
    (doc, diagnostic)
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

#[cfg(test)]
pub fn open(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    concept_id: &str,
) -> Option<OpenDocument> {
    open_with_asset_host(okf, uml, concept_id, &assets_for_test())
}

pub fn reopen_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    tab: &crate::doc_tabs::DocTab,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    open_locator_with_asset_host(okf, uml, &tab.locator(), assets, limits, mask)
}

/// Opens a stored `DocumentLocator` by looking its surface up in the editor
/// build's registered surface table (`extension_editor::surface_table`) and
/// calling the matching factory with the locator's target. Keyed on
/// `RowTarget`, never on row/tab identity -- a `hide`-hidden concept has no
/// `RowId` in Projected mode, so this is the only open path that can reach
/// it in both view modes (see `a_hidden_concept_still_opens_through_the_surface_path_in_both_modes`).
/// An unknown surface id degrades via `resolve_surface`'s type-based default
/// rather than returning `None` -- never a blank tab.
pub fn open_locator_with_asset_host(
    okf: &waml::analysis::OkfAnalysis,
    uml: &waml::uml::Analysis,
    locator: &DocumentLocator,
    assets: &crate::markdown_hosts::SharedMarkdownAssetHost,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    let (surface, _diagnostic) = waml::view::surface::resolve_surface(
        Some(locator.surface.as_str()),
        &locator.target,
        &okf.bundle,
        KNOWN_SURFACES,
        "index.md",
        0,
    );
    let ctx = crate::extension_editor::OpenCtx {
        analysis: okf,
        uml,
        assets: assets.clone(),
        limits,
        mask,
    };
    crate::extension_editor::surface_table()
        .into_iter()
        .find(|(name, _)| *name == surface.as_str())
        .and_then(|(_, factory)| factory(&ctx, &locator.target))
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
        assert!(
            crate::okf_documents::open_with_asset_host(prepared.okf(), "order", &assets())
                .is_some()
        );
        assert_eq!(
            open_with_asset_host(prepared.okf(), prepared.uml(), "order", &assets())
                .unwrap()
                .tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "order",
                waml::view::surface::SurfaceId::canvas()
            ))
        );
        assert!(crate::uml_documents::open(prepared.okf(), prepared.uml(), "runbook").is_none());
        assert_eq!(
            open_with_asset_host(prepared.okf(), prepared.uml(), "runbook", &assets())
                .unwrap()
                .tab_id,
            crate::documents::tab_id_for(&DocumentLocator::concept(
                "runbook",
                waml::view::surface::SurfaceId::markdown()
            ))
        );

        let (generic_tab, _) =
            open_with_asset_host(prepared.okf(), prepared.uml(), "runbook", &assets())
                .unwrap()
                .into_tab(true);
        assert_eq!(
            reopen_with_asset_host(
                prepared.okf(),
                prepared.uml(),
                &generic_tab,
                &assets(),
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap()
            .tab_id,
            generic_tab.id
        );

        let (source_tab, _) =
            crate::okf_documents::open_source_with_asset_host(prepared.okf(), "runbook", &assets())
                .unwrap()
                .into_tab(false);
        assert_eq!(
            reopen_with_asset_host(
                prepared.okf(),
                prepared.uml(),
                &source_tab,
                &assets(),
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap()
            .tab_id,
            source_tab.id
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

        let document =
            open_with_asset_host(prepared.okf(), prepared.uml(), "broken", &assets()).unwrap();
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

        assert!(open_with_asset_host(prepared.okf(), prepared.uml(), "index", &assets()).is_none());
        assert!(open_with_asset_host(prepared.okf(), prepared.uml(), "log", &assets()).is_none());
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
        let (mut old_source_tab, _) =
            crate::okf_documents::open_source_with_asset_host(prepared.okf(), "runbook", &assets())
                .unwrap()
                .into_tab(false);
        old_source_tab.id = makepad_widgets::LiveId::from_str("closed-transient-tab");

        let reopened = reopen_with_asset_host(
            prepared.okf(),
            prepared.uml(),
            &old_source_tab,
            &assets(),
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
}
