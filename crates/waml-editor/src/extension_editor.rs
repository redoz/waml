//! Task E2: the editor-side half of a middleware extension -- pairs the
//! headless `CoreExtension` (row projection, `crates/waml/src/view/`) with
//! the surfaces that can open the rows it names.
//!
//! A `SurfaceFactory` is a factory, not an instance: it is called only when
//! a row is *opened*. A `DocView` per listed row would allocate widgets and
//! fonts for rows nobody ever opens.
//!
//! Deviation from the plan's illustrative signature: the factory returns
//! `Option<Box<dyn DocView>>`, not `Box<dyn DocView>`. `RowId` alone does not
//! carry the target it names -- resolving it can fail (a stale id from a
//! session snapshot, a row whose target changed shape between resolutions).
//! A reachable failure degrades to `None` here rather than panicking; the
//! caller is responsible for turning that into the `UnknownSurface`-style
//! diagnostic path E2's headless half already established in
//! `waml::view::surface::resolve_surface`.

use waml::analysis::OkfAnalysis;
use waml::view::chain::ChainLimits;
use waml::view::row::{Row, RowId, RowTarget};

use crate::class_diagram_view::ClassDiagramView;
use crate::doc_view::DocView;
use crate::folder_view::FolderView;
use crate::generic_okf_view::GenericOkfView;
use crate::markdown_hosts::SharedMarkdownAssetHost;
use crate::source_view::SourceView;

/// Everything a `SurfaceFactory` needs to build a `DocView` for a row that
/// is being opened.
// Not yet constructed outside tests: Task E2's remaining half (rewiring
// `documents.rs`/`folder_view.rs` to open rows through this table instead of
// the current provider-chain `describe()` mechanism) is the concrete
// consumer, deferred to a following unit -- see the plan's Task E2 notes.
#[allow(dead_code)]
pub struct OpenCtx<'a> {
    pub analysis: &'a OkfAnalysis,
    pub assets: SharedMarkdownAssetHost,
    pub limits: ChainLimits,
    /// Resolves a `RowId` to the `Row` it names -- the same resolution that
    /// listed it in the tree in the first place (`Chain::resolve`, driven
    /// through the owner's `ProjectionCtx`). Injected so this module stays
    /// ignorant of `ProjectionCtx` construction, which is per-directory and
    /// already owned by the document-provider layer.
    pub resolve: &'a dyn Fn(&RowId) -> Option<Row>,
}

/// A factory, not an instance -- called when a row is opened, not when it is
/// listed. See module docs for why this returns `Option`.
// Consumer: the documents.rs/folder_view.rs rewiring, deferred (see OpenCtx).
#[allow(dead_code)]
pub type SurfaceFactory = Box<dyn Fn(&OpenCtx<'_>, &RowId) -> Option<Box<dyn DocView>>>;

// Consumer: the documents.rs/folder_view.rs rewiring, deferred (see OpenCtx).
#[allow(dead_code)]
pub trait EditorExtension {
    /// Matches its `CoreExtension` half's `name()` -- checked by Task E4's
    /// gate assertion, not at runtime.
    fn name(&self) -> &str;
    fn surfaces(&self) -> Vec<(&'static str, SurfaceFactory)>;
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
}

// The following free functions are the `SurfaceFactory` values registered by
// `CoreEditorExtension::surfaces`, itself unreachable outside tests until the
// rewiring above lands -- deferred, not orphaned.
#[allow(dead_code)]
fn concept_href(ctx: &OpenCtx<'_>, id: &RowId) -> Option<String> {
    match (ctx.resolve)(id)?.target {
        RowTarget::Concept(href) => Some(href),
        RowTarget::Folder(_) | RowTarget::Virtual => None,
    }
}

#[allow(dead_code)]
fn open_markdown(ctx: &OpenCtx<'_>, id: &RowId) -> Option<Box<dyn DocView>> {
    let href = concept_href(ctx, id)?;
    Some(Box::new(GenericOkfView::new_with_asset_host(
        href,
        ctx.assets.clone(),
    )))
}

#[allow(dead_code)]
fn open_source(ctx: &OpenCtx<'_>, id: &RowId) -> Option<Box<dyn DocView>> {
    let href = concept_href(ctx, id)?;
    Some(Box::new(SourceView::new_with_asset_host(
        href,
        ctx.assets.clone(),
    )))
}

#[allow(dead_code)]
fn open_canvas(ctx: &OpenCtx<'_>, id: &RowId) -> Option<Box<dyn DocView>> {
    let href = concept_href(ctx, id)?;
    Some(Box::new(ClassDiagramView::new(href)))
}

#[allow(dead_code)]
fn open_folder(ctx: &OpenCtx<'_>, id: &RowId) -> Option<Box<dyn DocView>> {
    let directory = match (ctx.resolve)(id)?.target {
        RowTarget::Folder(directory) => directory,
        RowTarget::Concept(_) | RowTarget::Virtual => return None,
    };
    let view = FolderView::build(
        ctx.analysis,
        &directory,
        ctx.limits,
        crate::folder_projection::ViewMode::Projected,
    )?;
    Some(Box::new(view))
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

    fn analysis() -> waml::analysis::OkfAnalysis {
        let source = waml::source::SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n"),
        ])
        .unwrap();
        waml::analysis::prepare_candidate(source, None, 1)
            .unwrap()
            .into_parts()
            .1
    }

    #[test]
    fn open_markdown_degrades_to_none_for_a_folder_target() {
        let analysis = analysis();
        let ctx = OpenCtx {
            analysis: &analysis,
            assets: crate::markdown_hosts::EditorMarkdownAssetHost::shared(
                crate::markdown_hosts::MarkdownAssetPolicy::BrowserBundle,
            ),
            limits: ChainLimits::default(),
            resolve: &|id: &RowId| {
                Some(
                    Row::new(
                        id.clone(),
                        "child".to_string(),
                        RowTarget::Folder("/sales".to_string()),
                        None,
                    )
                    .unwrap(),
                )
            },
        };
        let id = RowId {
            owner: waml::view::row::ViewId::new("root"),
            path: waml::view::row::RowPath::parse("sales").unwrap(),
        };
        assert!(open_markdown(&ctx, &id).is_none());
        assert!(open_folder(&ctx, &id).is_some());
    }
}
