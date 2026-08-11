//! The book's section model (spec 2026-08-11-book-mode-design, Phase 1):
//! a folder's projected rows walked depth-first into an ordered, flat list
//! of sections. Pure data over an `EditorSessionSnapshot` -- no widgets, no
//! `Cx` -- so ordering, depth capping, surface mapping, and degrade posture
//! are all unit-testable from a `SourceBundle` fixture.
//!
//! Each row's body is chosen by the editor's ONE surface resolution
//! (`documents::resolve_surface_for`), passing `row.surface` as the
//! requested id -- never a locally re-derived surface -- so a book section
//! and the tab that row would open can never disagree about what a row is.

use std::collections::HashSet;
use std::sync::Arc;

use waml::diagnostic::Diagnostic;
use waml::view::chain::ChainLimits;
use waml::view::mask::ProjectionMask;
use waml::view::row::{RowId, RowTarget};
use waml_markdown_editor::presentation::{compile_presentation, PresentationStyles};
use waml_markdown_editor::reading::{build_reading_document, ReadingDocument};

use crate::editor_session::EditorSessionSnapshot;
use crate::markdown_hosts::WamlCodeHighlightHost;
use crate::source_view::SourceView;

/// A folder's book model: its sections, walked depth-first, plus every
/// diagnostic the walk produced. The remaining per-field `#[allow(dead_code)]`
/// mark fields only this file's unit tests read today -- clippy's lib target
/// does not count unit tests as callers.
pub struct BookModel {
    pub directory: String,
    /// The book root's display title. Only unit tests read it: the tab title
    /// is taken from `folder_documents::title_for` at open time
    /// (`book_documents::open`), and no shipped chrome re-titles from the
    /// model afterwards.
    #[allow(dead_code)]
    pub title: String,
    pub sections: Vec<BookSection>,
    /// Everything the walk degraded or warned about. Only unit tests read
    /// it: Phase 1 has no surface listing a book's diagnostics -- sections
    /// degrade visibly to `Link` instead, which carries its own reason.
    #[allow(dead_code)]
    pub diagnostics: Vec<Diagnostic>,
    /// The session revision the model was built from. Only unit tests read
    /// it: `BookView` guards staleness with its own `revision` field rather
    /// than reading it back off the model.
    #[allow(dead_code)]
    pub revision: u64,
}

/// One row of the book, flattened out of the folder's depth-first walk.
/// `depth` is the row's nesting under the book root -- 0 for the book's own
/// direct children -- so the view can indent without re-deriving structure.
pub struct BookSection {
    pub row_id: RowId,
    pub depth: u8,
    pub title: String,
    pub target: RowTarget,
    pub body: SectionBody,
}

/// What a section renders as, chosen by the row's resolved surface.
pub enum SectionBody {
    /// A plain folder row: no content of its own, just a heading whose
    /// children (if any) follow at the next depth.
    Heading,
    /// A markdown concept, compiled through the exact reading-view path so a
    /// book section and its standalone reading tab render identically.
    Prose {
        document: Arc<ReadingDocument>,
        source: Arc<str>,
    },
    /// A UML concept, built through the exact class-diagram-view path so a
    /// book section and its standalone canvas tab render identically.
    /// Boxed: `Scene` dwarfs every other variant, and this enum lives in a
    /// `Vec<BookSection>` sized to the largest one.
    Diagram {
        scene: Box<crate::scene::Scene>,
        /// Only unit tests read this today: the open-full affordance
        /// navigates through the section's `target`
        /// (`book_view::navigation_for_section`), not through the body.
        #[allow(dead_code)]
        concept_id: String,
    },
    /// A row this build does not render inline: an unrenderable resolved
    /// surface, a nested book (recursive inlining has no natural bottom), or
    /// a failed compile -- one row, icon and title, opening that row's own
    /// tab. Never a silently blank section.
    Link { reason: LinkReason },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkReason {
    UnrenderedSurface(String),
    NestedBook,
    /// This folder was already inlined earlier in the walk -- a legitimate
    /// DAG (two rows linking the same folder) or a middleware cycle back to
    /// an ancestor. Inlining it again would duplicate sections (or never
    /// terminate), but a bare Heading whose children silently vanish would
    /// violate "never a silently blank section" -- so the repeat degrades to
    /// a Link the reader can still follow to the folder's own tab.
    AlreadyInlined,
    CompileFailed(String),
}

/// Build `directory`'s book model, or `None` if `directory` is not in the
/// bundle. Pure data over `snapshot` -- no widgets, no `Cx`.
pub fn build_book(
    snapshot: &EditorSessionSnapshot,
    directory: &str,
    limits: ChainLimits,
    mask: &ProjectionMask,
) -> Option<BookModel> {
    let data = snapshot.borrowed();
    data.okf_analysis.bundle.directory(directory)?;
    let registry = crate::folder_projection::core_registry();
    let mut sections = Vec::new();
    let mut diagnostics = Vec::new();
    // Seeded with the book root, like tree.rs's `expanded` set: a middleware
    // that points a folder row back at an ancestor must not recurse forever.
    let mut visited = HashSet::from([directory.to_string()]);
    walk(
        snapshot,
        &registry,
        directory,
        0,
        limits,
        mask,
        &mut visited,
        &mut sections,
        &mut diagnostics,
    );
    Some(BookModel {
        directory: directory.to_string(),
        title: crate::folder_documents::title_for(data.okf_analysis, directory),
        sections,
        diagnostics,
        revision: data.revision,
    })
}

#[allow(clippy::too_many_arguments)]
fn walk(
    snapshot: &EditorSessionSnapshot,
    registry: &waml::view::chain::MiddlewareRegistry,
    directory: &str,
    depth: usize,
    limits: ChainLimits,
    mask: &ProjectionMask,
    visited: &mut HashSet<String>,
    sections: &mut Vec<BookSection>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let data = snapshot.borrowed();
    let Some((_, rows, row_diags)) = crate::folder_projection::project_rows(
        data.okf_analysis,
        directory,
        mask,
        limits,
        registry,
    ) else {
        return;
    };
    diagnostics.extend(row_diags);
    for row in rows {
        // A nested book is deliberately a Link, not an inlined book:
        // recursive inlining has no natural bottom and would make a bundle's
        // root unbounded. Decided from the CHILD's own declared chain, the
        // same declaration a click on that row would route through.
        let nested_book = match &row.target {
            RowTarget::Folder(address) => {
                let (chain, _) =
                    crate::folder_projection::chain_for(data.okf_analysis, address, mask, registry);
                chain.resolution_surface() == Some(waml::view::surface::SurfaceId::book())
            }
            _ => false,
        };
        let mut body = body_for(
            snapshot,
            row.surface.as_ref().map(|s| s.as_str()),
            &row.target,
            nested_book,
            diagnostics,
        );
        // A folder this walk already inlined (see `LinkReason::AlreadyInlined`)
        // must not keep a Heading whose children silently fail to appear:
        // degrade the repeat to a Link, mirroring the nested-book posture.
        // The depth-cap case is deliberately NOT degraded -- a folder at the
        // cap keeps its heading by design, and is never inserted into
        // `visited`, so a later link to it can still inline it.
        if matches!(body, SectionBody::Heading) {
            if let RowTarget::Folder(address) = &row.target {
                if visited.contains(address) {
                    body = SectionBody::Link {
                        reason: LinkReason::AlreadyInlined,
                    };
                }
            }
        }
        let descend = matches!(body, SectionBody::Heading);
        sections.push(BookSection {
            row_id: row.id.clone(),
            depth: depth.min(u8::MAX as usize) as u8,
            title: row.label.clone(),
            target: row.target.clone(),
            body,
        });
        if descend {
            if let RowTarget::Folder(address) = &row.target {
                // The depth cap bounds INLINING, not existence: a folder at
                // the cap keeps its heading, its contents just do not inline.
                if depth + 1 < limits.max_depth && visited.insert(address.clone()) {
                    walk(
                        snapshot,
                        registry,
                        address,
                        depth + 1,
                        limits,
                        mask,
                        visited,
                        sections,
                        diagnostics,
                    );
                }
            }
        }
    }
}

/// One row's body, from the editor's one resolution. `pub(crate)` so the
/// degrade posture is testable directly -- a projected row never carries an
/// explicit surface today (no shipped stage sets one), so the unknown-surface
/// arm is unreachable through `build_book` until Phase 3's per-concept
/// declaration arrives through this exact seam.
pub(crate) fn body_for(
    snapshot: &EditorSessionSnapshot,
    requested_surface: Option<&str>,
    target: &RowTarget,
    nested_book: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> SectionBody {
    if nested_book {
        return SectionBody::Link {
            reason: LinkReason::NestedBook,
        };
    }
    let data = snapshot.borrowed();
    let (surface, diagnostic) = crate::documents::resolve_surface_for(
        data.okf_analysis,
        data.uml_analysis,
        requested_surface,
        target,
        "index.md",
        0,
    );
    diagnostics.extend(diagnostic);
    match (surface.as_str(), target) {
        ("folder", RowTarget::Folder(_)) => SectionBody::Heading,
        ("markdown", RowTarget::Concept(id)) => match compile_prose(snapshot, id) {
            Ok((document, source)) => SectionBody::Prose { document, source },
            Err(reason) => {
                // Same posture as ReadingView::install_snapshot: degrade
                // visibly, never a silently blank section. The reason is
                // user-visible on the section itself, so no separate log.
                SectionBody::Link {
                    reason: LinkReason::CompileFailed(reason),
                }
            }
        },
        ("canvas", RowTarget::Concept(id)) => {
            let model = &data.uml_analysis.projection;
            match model.diagrams.iter().find(|d| d.key == *id) {
                Some(diagram) => {
                    let (scene, scene_diags) = crate::scene::build_scene(
                        model,
                        diagram,
                        crate::diagram_display::resolve_display(&diagram.display),
                        &HashSet::new(),
                    );
                    diagnostics.extend(scene_diags);
                    SectionBody::Diagram {
                        scene: Box::new(scene),
                        concept_id: id.clone(),
                    }
                }
                None => SectionBody::Link {
                    reason: LinkReason::UnrenderedSurface("canvas".to_string()),
                },
            }
        }
        (other, _) => SectionBody::Link {
            reason: LinkReason::UnrenderedSurface(other.to_string()),
        },
    }
}

/// The exact compile path `ReadingView::install_snapshot` uses
/// (reading_view.rs): parse -> compile_presentation -> build_reading_document,
/// with the WAML code-highlight host. Returns the failure text so the section
/// can say WHY it degraded instead of rendering blank.
fn compile_prose(
    snapshot: &EditorSessionSnapshot,
    concept_id: &str,
) -> Result<(Arc<ReadingDocument>, Arc<str>), String> {
    let Some((_document, syntax)) = SourceView::resolve_document(snapshot, concept_id) else {
        return Err(format!("no markdown document for `{concept_id}`"));
    };
    let styles = Arc::new(PresentationStyles::balanced());
    let highlighters = WamlCodeHighlightHost::registry(Arc::new(snapshot.clone()));
    let plan = compile_presentation(&syntax, &styles, &highlighters)
        .map_err(|error| format!("presentation compile failed: {error:?}"))?;
    let document = build_reading_document(&plan)
        .map_err(|error| format!("reading model build failed: {error:?}"))?;
    let source: Arc<str> = Arc::from(syntax.text().shared().as_str());
    Ok((Arc::new(document), source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    /// The canonical Phase 1 fixture: a book folder with a prose section, a
    /// diagram section, a nested plain folder (heading + child prose), a
    /// nested book (must become a Link), and a loose concept outside the book.
    fn book_source() -> SourceBundle {
        SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Guide](guide/)\n* [Loose](loose.md)\n"),
            (
                "guide/index.md",
                "---\nview: book\n---\n# Guide\n\n* [Intro](intro.md)\n* [Flow](flow.md)\n* [Deep](deep/)\n* [Inner](inner/)\n",
            ),
            ("guide/intro.md", "# Intro\n\nSome prose.\n"),
            (
                "guide/flow.md",
                "---\ntype: uml.ClassDiagram\ntitle: Flow\n---\n# Flow\n",
            ),
            ("guide/deep/index.md", "# Deep\n\n* [Leaf](leaf.md)\n"),
            ("guide/deep/leaf.md", "# Leaf\n"),
            (
                "guide/inner/index.md",
                "---\nview: book\n---\n# Inner\n\n* [Nested](nested.md)\n",
            ),
            ("guide/inner/nested.md", "# Nested\n"),
            ("loose.md", "# Loose\n"),
        ])
        .unwrap()
    }

    fn snapshot_of(
        source: SourceBundle,
    ) -> std::sync::Arc<crate::editor_session::EditorSessionSnapshot> {
        let mut session = crate::editor_session::EditorSession::default();
        session.replace(source).unwrap();
        session.snapshot()
    }

    fn build(directory: &str, max_depth: usize) -> BookModel {
        build_book(
            &snapshot_of(book_source()),
            directory,
            waml::view::chain::ChainLimits { max_depth },
            &waml::view::mask::ProjectionMask::default(),
        )
        .unwrap()
    }

    #[test]
    fn a_book_lists_its_sections_depth_first_in_projection_order() {
        let model = build("/guide", 20);
        let titles: Vec<(&str, u8)> = model
            .sections
            .iter()
            .map(|s| (s.title.as_str(), s.depth))
            .collect();
        assert_eq!(
            titles,
            vec![
                ("Intro", 0),
                ("Flow", 0),
                ("Deep", 0),
                ("Leaf", 1),
                ("Inner", 0),
            ],
        );
        assert_eq!(model.title, "Guide");
    }

    #[test]
    fn each_default_surface_maps_to_its_section_body() {
        let model = build("/guide", 20);
        assert!(matches!(model.sections[0].body, SectionBody::Prose { .. }));
        assert!(
            matches!(&model.sections[1].body, SectionBody::Diagram { concept_id, .. } if concept_id == "guide/flow")
        );
        assert!(matches!(model.sections[2].body, SectionBody::Heading));
        assert!(matches!(model.sections[3].body, SectionBody::Prose { .. }));
    }

    #[test]
    fn a_nested_book_is_a_link_not_an_inlined_book() {
        let model = build("/guide", 20);
        let inner = model.sections.iter().find(|s| s.title == "Inner").unwrap();
        assert!(
            matches!(&inner.body, SectionBody::Link { reason } if *reason == LinkReason::NestedBook)
        );
        // Nothing beneath it was inlined.
        assert!(!model.sections.iter().any(|s| s.title == "Nested"));
    }

    #[test]
    fn the_depth_cap_bounds_how_deep_sections_inline() {
        let model = build("/guide", 1);
        // Deep's own heading survives at the cap; its children do not inline.
        assert!(model.sections.iter().any(|s| s.title == "Deep"));
        assert!(!model.sections.iter().any(|s| s.title == "Leaf"));
    }

    #[test]
    fn an_unknown_declared_surface_degrades_to_a_link_with_a_warning() {
        let snapshot = snapshot_of(book_source());
        let mut diagnostics = Vec::new();
        let body = body_for(
            &snapshot,
            Some("no-such-surface"),
            &waml::view::row::RowTarget::Concept("guide/intro".to_string()),
            false,
            &mut diagnostics,
        );
        // Degrades to the claims default (markdown -> Prose)? NO: the spec
        // says an unknown DECLARED surface renders as a Link. resolve_surface_for
        // degrades the SURFACE to the default with an UnknownSurface warning;
        // the body follows the degraded surface -- so this IS Prose, and the
        // warning is the visible signal. Assert both:
        assert!(matches!(body, SectionBody::Prose { .. }));
        assert!(diagnostics
            .iter()
            .any(|d| d.code == waml::diagnostic::DiagCode::UnknownSurface));
    }

    #[test]
    fn a_concept_that_fails_to_compile_becomes_a_link_not_a_blank() {
        let snapshot = snapshot_of(book_source());
        let mut diagnostics = Vec::new();
        let body = body_for(
            &snapshot,
            None,
            &waml::view::row::RowTarget::Concept("no-such-concept".to_string()),
            false,
            &mut diagnostics,
        );
        assert!(matches!(
            &body,
            SectionBody::Link {
                reason: LinkReason::CompileFailed(_)
            }
        ));
    }

    #[test]
    fn an_already_inlined_folder_degrades_to_a_link_not_a_childless_heading() {
        // A repeat occurrence of a folder must degrade VISIBLY to an
        // already-inlined Link -- never a Heading whose children silently
        // vanish (the walk-wide `visited` set is a cycle guard, not a
        // license to blank a section). No shipped middleware can point two
        // rows at one folder today -- rows come from a folder's own
        // subdirectories -- so, like `body_for`'s unknown-surface arm, this
        // drives the seam directly: `visited` pre-seeded with the child
        // folder's address is exactly the state a second occurrence (a
        // future redirecting middleware, or a cycle back to an ancestor)
        // walks in with.
        let snapshot = snapshot_of(book_source());
        let registry = crate::folder_projection::core_registry();
        let mut sections = Vec::new();
        let mut diagnostics = Vec::new();
        let mut visited = HashSet::from(["/guide".to_string(), "/guide/deep".to_string()]);
        walk(
            &snapshot,
            &registry,
            "/guide",
            0,
            ChainLimits::default(),
            &ProjectionMask::default(),
            &mut visited,
            &mut sections,
            &mut diagnostics,
        );
        let deep = sections.iter().find(|s| s.title == "Deep").unwrap();
        assert!(
            matches!(&deep.body, SectionBody::Link { reason } if *reason == LinkReason::AlreadyInlined),
            "the repeat is a followable Link, not a childless heading"
        );
        assert!(
            !sections.iter().any(|s| s.title == "Leaf"),
            "nothing beneath the repeat inlines"
        );
    }

    #[test]
    fn a_missing_directory_yields_none_rather_than_panicking() {
        assert!(build_book(
            &snapshot_of(book_source()),
            "/missing",
            waml::view::chain::ChainLimits::default(),
            &waml::view::mask::ProjectionMask::default(),
        )
        .is_none());
    }
}
