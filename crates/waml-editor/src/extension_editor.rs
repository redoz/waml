//! Task E2: the editor-side half of a middleware extension -- pairs the
//! headless `CoreExtension` (row projection, `crates/waml/src/view/`) with
//! the surfaces that can open the rows it names.
//!
//! A `SurfaceFactory` is a factory, not an instance: it is called only when
//! a row is *opened*. A `DocView` per listed row would allocate widgets and
//! fonts for rows nobody ever opens.
//!
//! Keyed on `RowTarget`, not `RowId` (spike Q2's non-negotiable correction):
//! a `hide`-hidden concept has no `RowId` in Projected mode, so `RowId`
//! keying would make it unopenable. Each factory returns the same
//! `OpenDocument` the live provider path already builds for that target --
//! `Option` return means "this surface does not apply to this target", not
//! failure.

use waml::analysis::OkfAnalysis;
use waml::diagnostic::{DiagCode, Diagnostic};
use waml::view::chain::ChainLimits;
use waml::view::kind::RowKind;
use waml::view::row::{IconId, RowTarget};

use crate::document::{NavCategory, OpenDocument};
use crate::icons::{Icon, IconSet};
use crate::markdown_hosts::SharedMarkdownAssetHost;

/// Everything a `SurfaceFactory` needs to open a target.
// Not yet constructed outside tests: Task E2's remaining half (rewiring
// `documents.rs`/`folder_view.rs` to open rows through this table instead of
// the current provider-chain `describe()` mechanism) is the concrete
// consumer, deferred to a following unit -- see the plan's Task E2 notes.
#[allow(dead_code)]
pub struct OpenCtx<'a> {
    pub analysis: &'a OkfAnalysis,
    pub uml: &'a waml::uml::Analysis,
    pub assets: SharedMarkdownAssetHost,
    pub limits: ChainLimits,
    /// The session's projection mask, carried rather than assumed: a folder
    /// surface opened through this seam must list what the tree and the
    /// folder tabs list. One mask, read everywhere, is the invariant.
    pub mask: &'a waml::view::mask::ProjectionMask,
}

/// A factory, not an instance -- called when a target is opened, not when it
/// is listed. See module docs for why this returns `Option`.
// Consumer: the documents.rs/folder_view.rs rewiring, deferred (see OpenCtx).
#[allow(dead_code)]
pub type SurfaceFactory = Box<dyn Fn(&OpenCtx<'_>, &RowTarget) -> Option<OpenDocument>>;

// Consumer: the documents.rs/folder_view.rs rewiring, deferred (see OpenCtx).
#[allow(dead_code)]
pub trait EditorExtension {
    /// Matches its `CoreExtension` half's `name()` -- checked by Task E4's
    /// gate assertion, not at runtime.
    fn name(&self) -> &str;
    fn surfaces(&self) -> Vec<(&'static str, SurfaceFactory)>;
    /// The `IconId` names this extension's stage can stamp, paired with the
    /// `Icon` they resolve to. Default empty: most extensions mint no icons
    /// of their own and rely on the row's default-by-target degrade path.
    fn icons(&self) -> Vec<(&'static str, Icon)> {
        Vec::new()
    }
}

/// Today's four surfaces: markdown reading, source, canvas, and the folder
/// listing itself. No speculative format registry -- only what the editor
/// can already open.
// Consumer: the documents.rs/folder_view.rs rewiring, deferred (see OpenCtx).
#[allow(dead_code)]
pub struct CoreEditorExtension;

impl EditorExtension for CoreEditorExtension {
    fn name(&self) -> &str {
        "core"
    }

    fn surfaces(&self) -> Vec<(&'static str, SurfaceFactory)> {
        vec![
            ("markdown", Box::new(open_markdown)),
            ("source", Box::new(open_source)),
            ("canvas", Box::new(open_canvas)),
            ("folder", Box::new(open_folder)),
        ]
    }

    fn icons(&self) -> Vec<(&'static str, Icon)> {
        let mut icons: Vec<(&'static str, Icon)> = RowKind::ALL
            .into_iter()
            .filter_map(|kind| {
                IconSet::icon_for(NavCategory::from(kind)).map(|icon| (kind.as_icon_name(), icon))
            })
            .collect();
        icons.push(("book", Icon::Book));
        icons
    }
}

/// `uml`'s editor half: no surfaces of its own yet (it decorates rows the
/// core surfaces already open), and one icon -- the package/box glyph its
/// `UmlView` middleware stamps on `uml-domain` folders.
#[allow(dead_code)]
pub struct UmlEditorExtension;

impl EditorExtension for UmlEditorExtension {
    fn name(&self) -> &str {
        "uml"
    }

    fn surfaces(&self) -> Vec<(&'static str, SurfaceFactory)> {
        Vec::new()
    }

    fn icons(&self) -> Vec<(&'static str, Icon)> {
        vec![("box", Icon::Box)]
    }
}

/// Resolve a row's `IconId` to the `Icon` it draws, against the table an
/// editor build actually registered. `None` (no stage stamped an icon)
/// degrades to the row's default by target -- a pure degrade path now that
/// every shipped stage names its icon. An unknown name degrades to that same
/// default **and** emits an `UnknownIcon` warning diagnostic, mirroring
/// `waml::view::surface::resolve_surface` exactly.
#[allow(dead_code)]
pub fn resolve_icon(
    icon: Option<&IconId>,
    target: &RowTarget,
    known: &[(&str, Icon)],
    file: &str,
    line: usize,
) -> (Icon, Option<Diagnostic>) {
    let default = match target {
        RowTarget::Folder(_) => Icon::Folder,
        RowTarget::Concept(_) | RowTarget::Virtual => Icon::FileText,
    };
    match icon {
        None => (default, None),
        Some(id) => match known.iter().find(|(name, _)| *name == id.as_str()) {
            Some((_, icon)) => (*icon, None),
            None => {
                let diagnostic = Diagnostic::new(
                    DiagCode::UnknownIcon,
                    format!("unknown icon `{id}`, falling back to the default glyph"),
                    file,
                    line,
                );
                (default, Some(diagnostic))
            }
        },
    }
}

// The following free functions are the `SurfaceFactory` values registered by
// `CoreEditorExtension::surfaces`, itself unreachable outside tests until the
// rewiring above lands -- deferred, not orphaned. Each delegates to the same
// provider function the LIVE open path already uses, so a factory produces
// the exact same tab (wrappers included) as today's provider chain.
#[allow(dead_code)]
fn open_markdown(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
    let RowTarget::Concept(id) = target else {
        return None;
    };
    crate::okf_documents::open_with_asset_host(ctx.analysis, id, &ctx.assets)
}

// `open_source` becomes folder-capable in Task 3 (a folder's index document).
#[allow(dead_code)]
fn open_source(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
    let RowTarget::Concept(id) = target else {
        return None;
    };
    crate::okf_documents::open_source_with_asset_host(ctx.analysis, id, &ctx.assets)
}

#[allow(dead_code)]
fn open_canvas(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
    let RowTarget::Concept(id) = target else {
        return None;
    };
    crate::uml_documents::open_with_asset_host(ctx.analysis, ctx.uml, id, &ctx.assets)
}

#[allow(dead_code)]
fn open_folder(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
    let RowTarget::Folder(directory) = target else {
        return None;
    };
    crate::documents::open_folder(ctx.analysis, directory, ctx.limits, ctx.mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn todays_four_surfaces_are_registered_by_the_core_editor_half() {
        let ext = CoreEditorExtension;
        let names: BTreeSet<&'static str> =
            ext.surfaces().into_iter().map(|(name, _)| name).collect();
        let expected: BTreeSet<&'static str> = ["markdown", "source", "canvas", "folder"]
            .into_iter()
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn core_editor_extension_name_is_core() {
        assert_eq!(CoreEditorExtension.name(), "core");
    }

    #[test]
    fn every_row_kind_resolves_to_the_same_icon_it_resolves_to_through_icon_set() {
        for kind in RowKind::ALL {
            let via_extension = CoreEditorExtension
                .icons()
                .into_iter()
                .find(|(name, _)| *name == kind.as_icon_name())
                .map(|(_, icon)| icon);
            let via_icon_set = IconSet::icon_for(NavCategory::from(kind));
            assert_eq!(
                via_extension, via_icon_set,
                "kind {:?} disagrees between CoreEditorExtension::icons and IconSet::icon_for",
                kind
            );
        }
        let names: BTreeSet<&'static str> = CoreEditorExtension
            .icons()
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert!(names.contains("book"));
    }

    #[test]
    fn uml_editor_extension_names_and_icons() {
        let ext = UmlEditorExtension;
        assert_eq!(ext.name(), "uml");
        assert!(ext.surfaces().is_empty());
        assert_eq!(ext.icons(), vec![("box", Icon::Box)]);
    }

    #[test]
    fn resolve_icon_a_known_name_resolves_silently() {
        let table = CoreEditorExtension.icons();
        let (icon, diagnostic) = resolve_icon(
            Some(&IconId::new("class")),
            &RowTarget::Concept("order".to_string()),
            &table,
            "index.waml",
            1,
        );
        assert_eq!(icon, Icon::PanelTop);
        assert!(diagnostic.is_none());
    }

    #[test]
    fn resolve_icon_an_unknown_name_degrades_and_warns() {
        let table = CoreEditorExtension.icons();
        let (icon, diagnostic) = resolve_icon(
            Some(&IconId::new("no-such-icon")),
            &RowTarget::Concept("order".to_string()),
            &table,
            "index.waml",
            3,
        );
        assert_eq!(icon, Icon::FileText);
        let diagnostic = diagnostic.expect("unknown icon must produce a diagnostic");
        assert_eq!(diagnostic.code, DiagCode::UnknownIcon);
        assert_eq!(diagnostic.severity, waml::diagnostic::Severity::Warning);
    }

    #[test]
    fn resolve_icon_none_degrades_with_no_diagnostic() {
        let table = CoreEditorExtension.icons();
        let (icon, diagnostic) = resolve_icon(
            None,
            &RowTarget::Folder("/sales".to_string()),
            &table,
            "index.waml",
            1,
        );
        assert_eq!(icon, Icon::Folder);
        assert!(diagnostic.is_none());

        let (icon, diagnostic) = resolve_icon(None, &RowTarget::Virtual, &table, "index.waml", 1);
        assert_eq!(icon, Icon::FileText);
        assert!(diagnostic.is_none());
    }

    fn prepared() -> waml::analysis::PreparedCandidate {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n"),
            ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ])
        .unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    /// `OpenCtx` borrows the mask, so a per-call temporary would not outlive
    /// the ctx this helper returns.
    fn default_mask() -> &'static waml::view::mask::ProjectionMask {
        static MASK: std::sync::OnceLock<waml::view::mask::ProjectionMask> =
            std::sync::OnceLock::new();
        MASK.get_or_init(waml::view::mask::ProjectionMask::default)
    }

    fn ctx_for<'a>(
        analysis: &'a waml::analysis::OkfAnalysis,
        uml: &'a waml::uml::Analysis,
    ) -> OpenCtx<'a> {
        OpenCtx {
            analysis,
            uml,
            assets: crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            limits: ChainLimits::default(),
            mask: default_mask(),
        }
    }

    #[test]
    fn open_markdown_degrades_to_none_for_a_folder_target() {
        let prepared = prepared();
        let (_, analysis, uml, _) = prepared.into_parts();
        let ctx = ctx_for(&analysis, &uml);
        let folder = RowTarget::Folder("/sales".to_string());
        assert!(open_markdown(&ctx, &folder).is_none());
        assert!(open_folder(&ctx, &folder).is_some());
    }

    #[test]
    fn open_canvas_matches_the_live_paths_tab_identity() {
        let prepared = prepared();
        let (_, analysis, uml, _) = prepared.into_parts();
        let ctx = ctx_for(&analysis, &uml);
        let target = RowTarget::Concept("order".to_string());
        let doc = open_canvas(&ctx, &target).expect("a claimed uml.Class opens on canvas");
        assert_eq!(
            doc.tab_id,
            crate::uml_documents::uml_document_tab_id("order")
        );
    }
}
