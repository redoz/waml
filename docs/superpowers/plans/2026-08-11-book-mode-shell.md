# Book Mode Phase 1 — The Book Shell — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A folder whose `index.md` declares `view: book` opens as one continuous, read-only, virtualized scroll of its projected rows — prose sections typeset, diagram sections embedded live at a capped height, everything else a link — with the tree panel acting as its table of contents in both directions.

**Architecture:** `book` becomes a fifth registered surface: a surface-resolution entry in the core `view:` chain (the same category `markdown` occupies), a factory arm in the editor's surface table, and a `BookView: DocView` drawing on a new `book_surface` DSL sibling. The section model (`BookSection`/`SectionBody`) is pure data built from an `EditorSessionSnapshot` by walking the folder's projected rows depth-first through the editor's ONE surface resolution (`resolve_surface_for`), so it is unit-testable with no GUI. The widget layer virtualizes: only viewport-adjacent sections hold live `MarkdownViewer`/`ClassDiagramSurface` children. Tree-to-book sync rides on a new `RevealTarget::Row` variant; book-to-tree rides on `ProjectTree::reveal_target`.

**Tech Stack:** Rust workspace (`crates/waml` core + `crates/waml-editor` makepad app, script-fork makepad with `script_mod!` DSL), headless `Cx` unit tests in `#[cfg(test)] mod tests`, headless typed shell tests in `crates/waml-editor/src/app/tests/`.

## Global Constraints

Every task's requirements implicitly include this section.

- **Full gate per task, run before every commit, all green:**
  - `cargo test --workspace` (unfiltered — see `crates/waml-editor/tests/README.md`: a red in the `waml-syntax` property tests is a NEW defect)
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`
  - In `editors/vscode`: `pnpm run build`, `pnpm run lint`, `pnpm test`
- **Clippy promotes `dead_code` to a hard error.** Never land a type/function with no caller. Either land it with its caller in the same task, or mark it `#[allow(dead_code)]` with a comment naming the task that removes the allow (existing precedent: `FolderView::rows`, `folder_projection::editor_registry`).
- **No GUI available.** Every verification step is a `cargo`/`pnpm` command. Anything needing human eyes is ONLY in the final "Visual sign-off" section, never a task step.
- **Doc comments explain WHY**, densely, matching the register of `crates/waml-editor/src/documents.rs` and `doc_view.rs` (read a few before writing any). Tests are named as full sentences describing behavior (`fn opening_a_folder_yields_a_folder_presentation_and_tab_identity()`). Unit tests live in `#[cfg(test)] mod tests` at the bottom of the file they test.
- **Commit style:** conventional-commit subject + a body explaining why. NO `Co-Authored-By` trailer, no AI attribution of any kind.
- **Makepad traps (respected throughout, called out in Tasks 5-6):** a `draw_walk` rect goes stale after a `Size::Fill` sibling — book sections are fixed-height or `Fit`, never `Fill`; a lone fixed child inside a fixed parent can blank its content; a `script_mod!` namespace must be one object literal; a child widget is dead and invisible unless its `script_mod` registers before its parent's DSL is evaluated.
- **Spec:** `docs/superpowers/specs/2026-08-11-book-mode-design.md`, "Phase 1 architecture — the book shell" ONLY. Phases 2-4 are out of scope; do not add editing, bullets, or board machinery.
- **Snippets vs. codebase:** where a snippet in this plan disagrees with an existing idiom in the named template file (widget derive attributes, action plumbing, DSL field names), the codebase wins — mirror the named template exactly and keep the snippet's logic.

## File Structure

| File | Change | Responsibility |
| --- | --- | --- |
| `crates/waml/src/view/surface.rs` | modify | `SurfaceId::book()` constructor |
| `crates/waml/src/view/chain.rs` | modify | `Resolution::Book`, `"book"` arm in `Chain::build`/`Chain::run`, `Chain::resolution_surface()` accessor |
| `crates/waml/src/view/decl.rs` | modify | module doc comment: `book` joins the surface-resolution entry list |
| `crates/waml-editor/src/book_model.rs` | create | Pure-data section model: `BookModel`, `BookSection`, `SectionBody`, `LinkReason`, `build_book`, `compile_prose`, `body_for` |
| `crates/waml-editor/src/book_layout.rs` | create | Pure virtualization policy: per-kind height estimates, `section_tops`, `live_window`, `current_section` |
| `crates/waml-editor/src/book_documents.rs` | create | Document provider: `describe`/`open` for the book surface (sibling of `folder_documents.rs`) |
| `crates/waml-editor/src/book_view.rs` | create | `BookView: DocView` — snapshot-guarded model rebuild, chrome, action handling, reveal |
| `crates/waml-editor/src/book_surface.rs` | create | `BookSurface` widget — virtualized scroll, section drawing, live children, actions |
| `crates/waml-editor/src/extension_editor.rs` | modify | `("book", open_book)` factory; gate test renamed to five surfaces |
| `crates/waml-editor/src/documents.rs` | modify | `KNOWN_SURFACES` gains `"book"`; `locator_opens` book arm; probe-parity test list |
| `crates/waml-editor/src/folder_documents.rs` | modify | `title_for` becomes `pub(crate)` (shared with `book_documents`) |
| `crates/waml-editor/src/doc_view.rs` | modify | `DocViewIdentity::Book`, `show_book_view`, sibling-hide lines, `hide_document_surfaces`, `BodyWidgets::book_view_widget`, `RevealTarget::Row`, `DocView::reveal_target_for`, `ViewOutcome::{open_folder_listing, tree_mark}` |
| `crates/waml-editor/src/document_host.rs` | modify | `active_reveal_for_target` |
| `crates/waml-editor/src/app.rs` | modify | `book_surface` DSL sibling; `crate::book_surface::script_mod(vm)` registration |
| `crates/waml-editor/src/lib.rs` | modify | `mod book_model; mod book_layout; mod book_documents; mod book_view; mod book_surface;` |
| `crates/waml-editor/src/app/navigation.rs` | modify | `primary_folder_locator`, Directory-arm routing, book-tab refresh |
| `crates/waml-editor/src/app/actions.rs` | modify | `apply_view_outcome` arms for `open_folder_listing`/`tree_mark`; tree-click reveal intercept in `handle_tree_navigation` |
| `crates/waml-editor/src/app/tests/navigation.rs` | modify | Headless typed shell tests for open routing, toggle, reveal, tree marking |
| `crates/waml-editor/src/tree_panel.rs` | modify | `#[cfg(test)]` accessor for `reveal_key` (Task 8 assertion only) |

---

### Task 1: Core `book` surface-resolution entry

The chain already resolves `markdown` and `member:<href>` as surface resolutions (Task E3 machinery in `crates/waml/src/view/chain.rs`). `book` is a third entry of the same category: it does not project rows, it only changes the surface the folder's own tab resolves to. This task also adds the accessor the editor needs later (`Chain::resolution_surface`) — with its core-side tests as callers, so no dead code.

**Files:**
- Modify: `crates/waml/src/view/surface.rs` (constructor block, ~line 13-33)
- Modify: `crates/waml/src/view/chain.rs` (`Resolution` enum ~line 200, `Chain::build` match ~line 270, `Chain::run` resolution match ~line 421, new accessor on `impl Chain`, tests in `mod surface_resolutions` ~line 1755)
- Modify: `crates/waml/src/view/decl.rs` (module doc comment only, lines 6-8)

**Interfaces:**
- Consumes: existing `Resolution::{Markdown, Member}` plumbing, `ChainOutcome.surface`.
- Produces: `SurfaceId::book() -> SurfaceId` (the string `"book"`); `Chain::resolution_surface(&self) -> Option<SurfaceId>` — `Some(markdown/book)` for a statically declared resolution, `None` for `member:` (needs a projection run) and for no resolution. Tasks 2 and 4 call it.

- [ ] **Step 1: Write the failing tests**

In `crates/waml/src/view/chain.rs`, inside the existing `#[cfg(test)] mod tests` / `mod surface_resolutions` (~line 1755). Copy the body of the sibling test `view_markdown_resolves_the_folder_target_to_the_markdown_surface` (~line 1788) — same bundle constructor, same registry helper, same `run_root` helper returning `(rows, surface, diagnostics)` — and adapt:

```rust
#[test]
fn view_book_resolves_the_folder_target_to_the_book_surface() {
    let bundle = bundle(&[
        (
            "index.md",
            "---\nview: book\n---\n# Root\n\n* [Orders](orders.md)\n",
        ),
        ("orders.md", "# Orders\n"),
    ]);
    let identity = bundle(&[
        ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
        ("orders.md", "# Orders\n"),
    ]);
    let registry = registry();
    let (rows, surface, diagnostics) = run_root(&bundle, &registry);
    let (identity_rows, identity_surface, identity_diags) = run_root(&identity, &registry);
    assert!(diagnostics.is_empty());
    assert!(identity_diags.is_empty());
    assert_eq!(surface.as_str(), "book");
    assert_ne!(identity_surface.as_str(), "book");
    // A resolution never changes the projection, only the folder's own surface.
    assert_eq!(
        rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        identity_rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
    );
}

#[test]
fn view_book_after_middleware_keeps_the_projection_and_resolves_book() {
    // `view: [hide, book]` plus a hide: glob (copy the frontmatter shape from
    // this file's hide fixtures). The hidden row must be gone from `rows` AND
    // the surface must be "book": middleware may precede the resolution entry.
    let bundle = bundle(&[
        (
            "index.md",
            "---\nview: [hide, book]\nhide: [\"secret\"]\n---\n# Root\n\n* [Orders](orders.md)\n* [Secret](secret.md)\n",
        ),
        ("orders.md", "# Orders\n"),
        ("secret.md", "# Secret\n"),
    ]);
    let (rows, surface, diagnostics) = run_root(&bundle, &registry());
    assert!(diagnostics.is_empty());
    assert_eq!(surface.as_str(), "book");
    assert_eq!(
        rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(),
        vec!["Orders"],
    );
}

#[test]
fn resolution_surface_reports_statically_declared_resolutions() {
    // Build chains straight from decls (use this module's decl/index/mask
    // helpers, as `a_mask_never_drops_a_surface_resolution` does).
    for (entries, expected) in [
        (vec!["book"], Some(SurfaceId::book())),
        (vec!["markdown"], Some(SurfaceId::markdown())),
        (vec!["member:./orders"], None),
        (vec![], None),
    ] {
        let (chain, diags) = Chain::build(
            &decl(&entries),
            &registry(),
            &index(),
            &ProjectionMask::default(),
        );
        assert!(diags.is_empty(), "{entries:?}");
        assert_eq!(chain.resolution_surface(), expected, "{entries:?}");
    }
}
```

Note: `bundle`, `registry`, `run_root`, `decl`, `index` above stand for whatever those helpers are actually named inside `mod surface_resolutions` — they are in the same file being edited; match them exactly. Also extend the existing `a_mask_never_drops_a_surface_resolution` test (~line 1053) with a `["book"]` decl masked by `ProjectionMask::from_names(["book"])`, asserting the outcome surface is still `"book"`.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml view_book` and `cargo test -p waml resolution_surface_reports`
Expected: COMPILE FAILURE — `Resolution::Book`, `resolution_surface`, and `SurfaceId::book` do not exist yet.

- [ ] **Step 3: Implement**

`crates/waml/src/view/surface.rs`, after `folder()`:

```rust
    /// The book surface (spec 2026-08-11-book-mode-design): a folder read as
    /// one continuous scroll. Declared per folder via `view: book`; never a
    /// type default, so it does not appear in [`default_surface`].
    pub fn book() -> Self {
        SurfaceId("book".into())
    }
```

`crates/waml/src/view/chain.rs` — four edits mirroring the `Markdown` variant exactly:

1. The `Resolution` enum gains a variant:

```rust
    /// `view: book` -- the folder's own tab renders the book surface (one
    /// continuous scroll of the projected rows), rows unchanged. Same
    /// category as `Markdown`: a resolution, never a middleware stage.
    Book,
```

2. `Chain::build`'s entry-name match, directly after the `"markdown"` arm (inherits the same "resolved BEFORE the mask" comment block):

```rust
                "book" => {
                    resolution = Some(Resolution::Book);
                    continue;
                }
```

3. `Chain::run`'s resolution match, after the `Markdown` arm:

```rust
                    Some(Resolution::Book) => (SurfaceId("book".to_string()), Vec::new()),
```

4. New accessor on `impl Chain` (place near `ids()`):

```rust
    /// The surface this chain's declared RESOLUTION names, when that answer
    /// is static. `markdown` and `book` are decided at declaration time;
    /// `member:<href>` needs a projection run to resolve the member's own
    /// surface, so it (and a chain with no resolution) answers `None` -- a
    /// caller that needs the member answer runs the chain. Exists so a click
    /// site can ask "does this folder open as a book?" without projecting
    /// rows (the editor's `App::primary_folder_locator`).
    pub fn resolution_surface(&self) -> Option<SurfaceId> {
        match &self.resolution {
            Some(Resolution::Markdown) => Some(SurfaceId::markdown()),
            Some(Resolution::Book) => Some(SurfaceId::book()),
            Some(Resolution::Member(_)) | None => None,
        }
    }
```

5. `crates/waml/src/view/decl.rs` module doc, line 6: change the sentence to name all three: "`markdown`, `book`, and `member:<href>` are surface-resolution entries, not middleware".

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml`
Expected: PASS, including every pre-existing `surface_resolutions` test.

- [ ] **Step 5: Full gate, then commit**

Run the full gate from Global Constraints, then:

```bash
git add crates/waml/src/view/surface.rs crates/waml/src/view/chain.rs crates/waml/src/view/decl.rs
git commit -m "feat(view): add book as a chain surface-resolution entry" -m "A folder opts into book mode by declaring view: book in its index
frontmatter. Like markdown (Task E3), it is a resolution the chain
attaches to its own surface, not a middleware stage: the row projection
is unchanged and middleware may precede it. Chain::resolution_surface
exposes the statically-decidable answer so the editor's click sites can
route a directory open without projecting rows."
```

---

### Task 2: The book section model (pure data, no GUI)

`BookSection` is built from the folder's projected rows, walked depth-first, each row's body chosen by the editor's ONE surface resolution (`documents::resolve_surface_for`) — never a locally re-derived surface — so a book section and the tab that row would open can never disagree. Pure data over an `EditorSessionSnapshot`, unit-testable from `SourceBundle` fixtures.

Prose reuses the exact compile path `ReadingView::install_snapshot` uses today (`crates/waml-editor/src/reading_view.rs`, 95 lines — read it first). Diagram sections reuse `crate::scene::build_scene` exactly as `ClassDiagramView::sync` does (`class_diagram_view.rs:451-469`). `Scene` already derives `Clone` (`scene.rs:116`).

**Files:**
- Create: `crates/waml-editor/src/book_model.rs`
- Modify: `crates/waml-editor/src/lib.rs` (add `mod book_model;` beside `mod folder_projection;`)

**Interfaces:**
- Consumes: `folder_projection::{core_registry, project_rows, chain_for}`; `documents::resolve_surface_for` (pub(crate)); `Chain::resolution_surface` (Task 1); `SourceView::resolve_document(snapshot, key) -> Option<(_, syntax)>`; `compile_presentation(&syntax, &styles, &highlighters)` + `build_reading_document(&plan)` + `PresentationStyles::balanced()` + `WamlCodeHighlightHost::registry(Arc<EditorSessionSnapshot>)` (all as used in `reading_view.rs`); `crate::scene::{build_scene, Scene}`; `crate::diagram_display::resolve_display`; `EditorSessionSnapshot::borrowed()`.
- Produces (consumed by Tasks 3, 5, 6, 7):

```rust
pub struct BookModel {
    pub directory: String,
    pub title: String,
    pub sections: Vec<BookSection>,
    pub diagnostics: Vec<waml::diagnostic::Diagnostic>,
    pub revision: u64,
}
pub struct BookSection {
    pub row_id: waml::view::row::RowId,
    pub depth: u8,
    pub title: String,
    pub target: waml::view::row::RowTarget,
    pub body: SectionBody,
}
pub enum SectionBody {
    Heading,
    Prose { document: std::sync::Arc<ReadingDocument>, source: std::sync::Arc<str> },
    Diagram { scene: crate::scene::Scene, concept_id: String },
    Link { reason: LinkReason },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkReason {
    UnrenderedSurface(String),
    NestedBook,
    CompileFailed(String),
}
pub fn build_book(
    snapshot: &crate::editor_session::EditorSessionSnapshot,
    directory: &str,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<BookModel>;
```

Dead-code discipline: the unit tests below are real callers, but clippy's lib target does not count them, so annotate each not-yet-consumed public item (`build_book`, the types) with `#[allow(dead_code)] // consumed by book_view.rs in Task 3, which removes this allow`. Task 3 removes every one of them.

- [ ] **Step 1: Write the failing tests**

At the bottom of the new `crates/waml-editor/src/book_model.rs`. The fixture is shared by later tasks — keep its shape exactly:

```rust
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
                "---\ntype: Diagram\ntitle: Flow\n---\n# Flow\n",
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

    fn snapshot_of(source: SourceBundle) -> std::sync::Arc<crate::editor_session::EditorSessionSnapshot> {
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
        // the body follows the degraded surface — so this IS Prose, and the
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
        assert!(
            matches!(&body, SectionBody::Link { reason: LinkReason::CompileFailed(_) })
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
```

Note on `an_unknown_declared_surface_degrades...`: the spec's "unknown declared surface degrades to `Link` and emits the `UnknownSurface` warning" is satisfied by the degrade CHAIN: `resolve_surface_for` degrades unknown ids to the claims default (never a blank), and the warning lands in `model.diagnostics`. The body then follows the RESOLVED surface — identical to what that row's own tab would open. Rendering it as a dead Link while its tab renders fine would make the book and the tab disagree, which the spec forbids more strongly ("never by re-deriving a surface locally"). A surface that resolves to something the book cannot inline (`source`, `search:*`) is the `Link { UnrenderedSurface }` arm.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor book_model`
Expected: COMPILE FAILURE — module does not exist.

- [ ] **Step 3: Implement `book_model.rs`**

```rust
//! The book's section model (spec 2026-08-11-book-mode-design, Phase 1):
//! a folder's projected rows walked depth-first into an ordered, flat list
//! of sections. Pure data over an `EditorSessionSnapshot` — no widgets, no
//! `Cx` — so ordering, depth capping, surface mapping, and degrade posture
//! are all unit-testable from a `SourceBundle` fixture.
//!
//! Each row's body is chosen by the editor's ONE surface resolution
//! (`documents::resolve_surface_for`), passing `row.surface` as the
//! requested id — never a locally re-derived surface — so a book section
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

// (types as given in Interfaces above, each with a WHY doc comment; e.g.
// SectionBody::Link's comment: "A row this build does not render inline: an
// unrenderable resolved surface, a nested book (recursive inlining has no
// natural bottom), or a failed compile — one row, icon and title, opening
// that row's own tab. Never a silently blank section.")

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
                let (chain, _) = crate::folder_projection::chain_for(
                    data.okf_analysis,
                    address,
                    mask,
                    registry,
                );
                chain.resolution_surface() == Some(waml::view::surface::SurfaceId::book())
            }
            _ => false,
        };
        let body = body_for(
            snapshot,
            row.surface.as_ref().map(|s| s.0.as_str()),
            &row.target,
            nested_book,
            diagnostics,
        );
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
                        snapshot, registry, address, depth + 1, limits, mask, visited,
                        sections, diagnostics,
                    );
                }
            }
        }
    }
}

/// One row's body, from the editor's one resolution. `pub(crate)` so the
/// degrade posture is testable directly — a projected row never carries an
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
        return SectionBody::Link { reason: LinkReason::NestedBook };
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
                // Same posture as ReadingView::install_snapshot: log and
                // degrade visibly, never a silently blank section.
                log::error!("book section {id}: {reason}");
                SectionBody::Link { reason: LinkReason::CompileFailed(reason) }
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
                    SectionBody::Diagram { scene, concept_id: id.clone() }
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
```

Notes for the implementer:
- `log::error!` above stands for whatever this crate actually uses — `reading_view.rs` uses makepad's `log!`, but this module is makepad-free; use `eprintln!`-free, makepad-free logging the crate already has, or route the reason ONLY through `LinkReason` and drop the log line (acceptable: the reason is user-visible on the section).
- `folder_documents::title_for` must become `pub(crate)` in this task (one-line visibility change).
- If `model.diagrams`'s element type field names differ (`key`, `display`, `title`), mirror `class_diagram_view.rs:455-457` exactly.
- Add `#[allow(dead_code)]` (with the Task 3 removal comment) on any item clippy flags in the lib target.

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml-editor book_model`
Expected: PASS (7 tests). If `each_default_surface_maps_to_its_section_body` fails because the uml projection does not list a `type: Diagram` document under `model.diagrams` keyed by concept id, STOP and check how `diagram_properties_app` (app/tests/navigation.rs:100) resolves `"orders"` — mirror that lookup; do not weaken the assertion.

- [ ] **Step 5: Full gate, then commit**

```bash
git add crates/waml-editor/src/book_model.rs crates/waml-editor/src/lib.rs crates/waml-editor/src/folder_documents.rs
git commit -m "feat(editor): pure-data book section model over the projected rows" -m "Sections come from the folder's projected rows walked depth-first,
bounded by ChainLimits::max_depth and a visited-set cycle guard (the
same guard shape tree.rs uses). Each body is chosen by the editor's one
surface resolution so a section and the tab its row opens can never
disagree; unknown surfaces degrade with the UnknownSurface warning, a
failed prose compile degrades to a Link carrying its reason, and a
nested book is a Link because recursive inlining has no bottom.
Makepad-free by design: every posture is unit-tested from a
SourceBundle fixture with no window."
```

---

### Task 3: Register the fifth surface — `BookView`, `book_documents`, surface table, probe

The `book` surface joins every registration site at once, because their parity is pinned by tests: `KNOWN_SURFACES` (documents.rs:71), `CoreEditorExtension::surfaces` (extension_editor.rs:66, pinned by the gate test at :196 — its name and expected set move to five), the surface-table factory, and `locator_opens` (whose arms mirror the factories, pinned by `the_open_probe_agrees_with_the_open_path_on_every_surface`). `folder_documents.rs` (97 lines) is the template for the provider; read it first.

`BookView` lands here too (the factory needs it), with the reading-view snapshot pattern: the model is built lazily from the session snapshot, guarded on `revision`, and a failed rebuild KEEPS the previous model (reading_view.rs:64-74 posture). Its `sync` shows the (not-yet-existing) `book_surface` — `BodyWidgets` lookups on absent widgets are no-ops, which is exactly how every headless test in this repo already runs.

**Files:**
- Create: `crates/waml-editor/src/book_documents.rs`
- Create: `crates/waml-editor/src/book_view.rs`
- Modify: `crates/waml-editor/src/lib.rs` (`mod book_documents; mod book_view;`)
- Modify: `crates/waml-editor/src/documents.rs` (`KNOWN_SURFACES` ~line 71 + its doc comment naming the gate test; `locator_opens` match ~line 132; probe test surface list ~line 717)
- Modify: `crates/waml-editor/src/extension_editor.rs` (surfaces vec ~line 66, module doc "four surfaces" comment ~line 56, `open_book` factory beside `open_folder` ~line 183, gate test ~line 196)
- Modify: `crates/waml-editor/src/doc_view.rs` (`DocViewIdentity` ~line 520; `show_book_view` + `book_surface` hide lines in the five existing `show_*` fns ~lines 182-273; `hide_document_surfaces` ~line 281)

**Interfaces:**
- Consumes: `build_book` (Task 2, allows removed here); `folder_documents::title_for`; `tab_id_for`; `OpenCtx`.
- Produces (consumed by Tasks 4-8):
  - `book_documents::describe(okf, directory) -> Option<DocumentDescriptor>` and `book_documents::open(okf, directory, limits, &mask) -> Option<OpenDocument>` — locator `DocumentLocator::new(RowTarget::Folder(..), SurfaceId::book())`, icon `Icon::Book`, category `NavCategory::Directory`.
  - `BookView::new(directory: String, limits: ChainLimits, mask: ProjectionMask) -> BookView`; `BookView::model(&self) -> Option<&Rc<BookModel>>` (pub(crate)); `DocViewIdentity::Book`.
  - `BodyWidgets::show_book_view(&self, cx)`.

- [ ] **Step 1: Write the failing tests**

`book_documents.rs` tests (mirror `folder_documents.rs`'s test exactly; same fixture as Task 2's `book_source()` — duplicate the fixture fn here, tests may not import each other's test modules):

```rust
#[test]
fn opening_a_book_folder_yields_a_book_presentation_and_tab_identity() {
    let prepared = waml::analysis::prepare_candidate(book_source(), None, 1).unwrap();
    let document = open(
        prepared.okf(),
        "/guide",
        waml::view::chain::ChainLimits::default(),
        &waml::view::mask::ProjectionMask::default(),
    )
    .unwrap();
    assert_eq!(document.presentation.category, NavCategory::Directory);
    assert_eq!(document.presentation.icon, Icon::Book);
    assert_eq!(document.title, "Guide");
    assert_eq!(
        document.locator,
        DocumentLocator::new(
            waml::view::row::RowTarget::Folder("/guide".to_string()),
            waml::view::surface::SurfaceId::book(),
        )
    );
    assert_eq!(document.tab_id, crate::documents::tab_id_for(&document.locator));
    // A book tab and a folder-listing tab of the SAME directory are two tabs.
    assert_ne!(
        document.tab_id,
        crate::documents::tab_id_for(&DocumentLocator::folder("/guide"))
    );
    assert!(open(
        prepared.okf(),
        "/missing",
        waml::view::chain::ChainLimits::default(),
        &waml::view::mask::ProjectionMask::default(),
    )
    .is_none());
}
```

In `extension_editor.rs`: rename the gate test and its expected set:

```rust
#[test]
fn todays_five_surfaces_are_registered_by_the_core_editor_half() {
    let ext = CoreEditorExtension;
    let names: BTreeSet<&'static str> =
        ext.surfaces().into_iter().map(|(name, _)| name).collect();
    let expected: BTreeSet<&'static str> =
        ["markdown", "source", "canvas", "folder", "book"].into_iter().collect();
    assert_eq!(names, expected);
}
```

In `documents.rs`: add `"book"` to the surface list inside `the_open_probe_agrees_with_the_open_path_on_every_surface` (~line 717): `for surface in ["markdown", "source", "canvas", "folder", "book", "no-such-surface"]`. The probe and the open path must agree for book locators on every target — folder targets open, concept/virtual targets do not.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor todays_five` and `cargo test -p waml-editor opening_a_book_folder`
Expected: COMPILE FAILURE (missing modules/fns), and the probe test fails once `"book"` is in its list but nowhere else.

- [ ] **Step 3: Implement**

`book_documents.rs` (sibling of `folder_documents.rs`, same shape):

```rust
//! Document provider for a folder's BOOK surface (spec
//! 2026-08-11-book-mode-design). Keyed on a directory address like
//! `folder_documents`, but a different surface: the same directory's book
//! tab and listing tab are two tabs (`tab_id_for` bakes the surface in).
//!
//! Deliberately opens ANY directory in the bundle, declared or not: a stored
//! book locator (history, reopened tab) must keep opening after the folder's
//! declaration is edited away -- the `view: book` declaration is a CLICK
//! routing decision (`App::primary_folder_locator`), not a capability gate.

use crate::document::{
    DocumentCapabilities, DocumentDescriptor, DocumentPresentation, NavCategory, OpenDocument,
};
use crate::icons::Icon;
use crate::navigation::DocumentLocator;

pub fn describe(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
) -> Option<DocumentDescriptor> {
    analysis.bundle.directory(directory)?;
    Some(DocumentDescriptor {
        presentation: DocumentPresentation {
            icon: Icon::Book,
            accent: None,
            category: NavCategory::Directory,
        },
        capabilities: DocumentCapabilities::default(),
    })
}

pub fn open(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    limits: waml::view::chain::ChainLimits,
    mask: &waml::view::mask::ProjectionMask,
) -> Option<OpenDocument> {
    let presentation = describe(analysis, directory)?.presentation;
    let title = crate::folder_documents::title_for(analysis, directory);
    let locator = DocumentLocator::new(
        waml::view::row::RowTarget::Folder(directory.to_string()),
        waml::view::surface::SurfaceId::book(),
    );
    Some(OpenDocument {
        tab_id: crate::documents::tab_id_for(&locator),
        locator,
        title,
        presentation,
        view: Box::new(crate::book_view::BookView::new(
            directory.to_string(),
            limits,
            mask.clone(),
        )),
    })
}
```

`book_view.rs`:

```rust
//! The `waml-editor` side of the book surface. Mirrors the split
//! `ReadingView` uses: the model is compiled from the session snapshot
//! (revision-guarded; a failed rebuild keeps the previous revision and says
//! so), and `sync` only pushes already-built state at the shared body.

use std::rc::Rc;

use makepad_widgets::*;

use crate::book_model::{build_book, BookModel};
use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, HeaderViewAction,
    ViewData, ViewOutcome,
};
use crate::editor_session::EditorSessionSnapshot;
use crate::icons::Icon;

pub struct BookView {
    directory: String,
    limits: waml::view::chain::ChainLimits,
    /// The mask the book was opened under -- the session's one mask, carried
    /// like `OpenCtx.mask` so the book lists what the tree lists.
    mask: waml::view::mask::ProjectionMask,
    model: Option<Rc<BookModel>>,
    revision: Option<u64>,
}

impl BookView {
    pub fn new(
        directory: String,
        limits: waml::view::chain::ChainLimits,
        mask: waml::view::mask::ProjectionMask,
    ) -> BookView {
        BookView { directory, limits, mask, model: None, revision: None }
    }

    pub(crate) fn model(&self) -> Option<&Rc<BookModel>> {
        self.model.as_ref()
    }

    fn install_snapshot(&mut self, snapshot: &EditorSessionSnapshot) {
        let revision = snapshot.borrowed().revision;
        if self.revision == Some(revision) {
            return;
        }
        // On failure keep the previous model -- the same posture as
        // ReadingView: a stale book beats a blank one, and the next good
        // snapshot heals it.
        match build_book(snapshot, &self.directory, self.limits, &self.mask) {
            Some(model) => {
                self.revision = Some(revision);
                self.model = Some(Rc::new(model));
            }
            None => log!(
                "book view {}: directory left the bundle, keeping the previous sections",
                self.directory
            ),
        }
    }
}

impl DocView for BookView {
    fn identity(&self) -> DocViewIdentity {
        DocViewIdentity::Book
    }

    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.show_book_view(cx);
        // Task 5 pushes self.model at the BookSurface widget here.
    }

    fn sync_from_session(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
    ) {
        self.install_snapshot(snapshot);
        self.sync(cx, body, snapshot.borrowed().into());
    }

    fn after_session_snapshot(
        &mut self,
        cx: &mut Cx,
        body: &BodyWidgets,
        snapshot: &EditorSessionSnapshot,
        _change: crate::editor_session::SessionChange,
    ) {
        self.install_snapshot(snapshot);
        self.sync(cx, body, snapshot.borrowed().into());
    }

    fn handle(
        &mut self,
        _cx: &mut Cx,
        _body: &BodyWidgets,
        _actions: &Actions,
        _data: ViewData<'_>,
    ) -> ViewOutcome {
        // Task 4: header toggle. Task 6: link/open-full clicks. Task 8: fold marking.
        ViewOutcome::default()
    }

    fn chrome(&self) -> BodyChrome {
        BodyChrome {
            tool_dock: false,
            view_bar: false,
            canvas_overlays: false,
            document_header: DocumentHeaderChrome {
                breadcrumb: true,
                right_dock: None,
                // The destination the toggle LEADS to: the plain listing of
                // the same directory -- the same shape as the source toggle.
                view_toggle: Some(HeaderViewAction {
                    icon: Icon::Folder,
                    tooltip: "View folder listing",
                }),
            },
        }
    }
}
```

Add a small unit test in `book_view.rs` asserting the snapshot guard (this is also the live caller that lets Task 2's `#[allow(dead_code)]`s come off):

```rust
#[test]
fn install_snapshot_rebuilds_only_when_the_revision_moves() {
    let mut session = crate::editor_session::EditorSession::default();
    session.replace(book_source()).unwrap(); // same fixture fn as book_model tests
    let snapshot = session.snapshot();
    let mut view = BookView::new(
        "/guide".to_string(),
        waml::view::chain::ChainLimits::default(),
        waml::view::mask::ProjectionMask::default(),
    );
    view.install_snapshot(&snapshot);
    let first = Rc::as_ptr(view.model().unwrap());
    view.install_snapshot(&snapshot);
    assert_eq!(first, Rc::as_ptr(view.model().unwrap()), "same revision, same model");
}
```

`doc_view.rs`:
- `DocViewIdentity` gains `Book`.
- `show_book_view` (sibling of `show_folder_view`, same shape — every other surface off, `book_surface` on, canvas interaction off):

```rust
    /// Show the book surface (`book_surface`), mutually exclusive with every
    /// sibling above. The widget itself arrives in a later task; until the
    /// DSL mounts it, the lookup is an absent-widget no-op, which is also
    /// what keeps every headless test green.
    pub fn show_book_view(&self, cx: &mut Cx) {
        for surface in [
            ids!(markdown_surface),
            ids!(markdown_viewer_surface),
            ids!(folder_view_surface),
            ids!(search_results_surface),
            ids!(canvas_wrap),
        ] {
            self.ui.widget(cx, surface).set_visible(cx, false);
        }
        self.ui.widget(cx, ids!(book_surface)).set_visible(cx, true);
        self.set_canvas_interaction_enabled(cx, false);
    }
```

- Each of the five existing `show_*` fns gains one line hiding `ids!(book_surface)` (mine on, siblings off — a book left visible behind a canvas is exactly the bug `hide_document_surfaces`'s doc comment warns about).
- `hide_document_surfaces`'s array gains `ids!(book_surface)`.

`extension_editor.rs`:
- Module doc: "Today's four surfaces" comment becomes five, naming book.
- `surfaces()` vec gains `("book", Box::new(open_book))`.
- Factory beside `open_folder`:

```rust
fn open_book(ctx: &OpenCtx<'_>, target: &RowTarget) -> Option<OpenDocument> {
    let RowTarget::Folder(directory) = target else {
        return None;
    };
    crate::book_documents::open(ctx.analysis, directory, ctx.limits, ctx.mask)
}
```

`documents.rs`:
- `KNOWN_SURFACES` becomes `&["markdown", "source", "canvas", "folder", "book"]`; update its doc comment's test name to `todays_five_surfaces_are_registered_by_the_core_editor_half`.
- `locator_opens` match gains, beside the `("folder", ..)` arm:

```rust
        // `open_book` -> `book_documents::open`, whose only `None` is a
        // directory not in the bundle -- the same probe `folder` uses.
        ("book", RowTarget::Folder(directory)) => okf.bundle.directory(directory).is_some(),
```

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml-editor`
Expected: PASS — including the renamed gate test, the widened probe-parity test, and every pre-existing surface test. Grep for the old test name to make sure nothing still references `todays_four_surfaces`.

- [ ] **Step 5: Full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(editor): register the book surface and its document provider" -m "book joins every pinned registration site in one commit because their
parity is what the tests protect: KNOWN_SURFACES, the core editor
half's surface list (gate test renamed to five), the surface-table
factory, and locator_opens (probe parity widened to book). BookView
follows the ReadingView snapshot pattern -- revision-guarded rebuild,
failure keeps the previous model -- and its tab is distinct from the
same directory's listing tab by surface, per tab_id_for. The provider
opens any bundled directory: the view:book declaration is click
routing, not a capability, so stored locators survive edits to it."
```

---

### Task 4: Route the click path — a `view: book` folder opens as a book

Today the editor never consumes the chain's resolved surface for a folder's own tab: `navigate_with`'s `Directory` arm (app/navigation.rs:403) always builds `DocumentLocator::folder(..)`. This task adds the folder sibling of `primary_locator`: `primary_folder_locator` asks the directory's declared chain (via Task 1's `resolution_surface`) and routes to the book locator iff the folder declares `book`. Deliberately narrowed to `book`: wiring the `markdown`/`member:` folder resolutions into the editor is existing, separate debt and out of this spec's scope.

Also lands the book side of the header toggle (chrome already offers it since Task 3): `ViewOutcome.open_folder_listing` drops the reader from the book to the plain listing of the same directory. And `refresh_folder_tabs` learns to re-run book tabs under a mask change, so the book and the tree never disagree about what a directory contains.

**Files:**
- Modify: `crates/waml-editor/src/app/navigation.rs` (`primary_folder_locator` new; `Directory` arm ~line 403; `refresh_folder_tabs` ~line 890; `open_folder_listing_addresses` ~line 916)
- Modify: `crates/waml-editor/src/doc_view.rs` (`ViewOutcome` ~line 373)
- Modify: `crates/waml-editor/src/book_view.rs` (`handle` gains the toggle branch)
- Modify: `crates/waml-editor/src/app/actions.rs` (`apply_view_outcome` ~line 1699)
- Test: `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Consumes: `Chain::resolution_surface` (Task 1), `folder_projection::chain_for`, `book_documents::open` (Task 3), `transition_to_location`, `handle_navigation_intent` (via `navigate_with`).
- Produces: `App::primary_folder_locator(&self, address: &str) -> DocumentLocator` (pub(super)); `ViewOutcome.open_folder_listing: Option<String>` (the directory address whose LISTING surface should open).

- [ ] **Step 1: Write the failing tests**

In `crates/waml-editor/src/app/tests/navigation.rs`. Reuse the file's existing `navigation_app()` helper and `FakeBrowser`; install the book fixture over it (the `app.session.replace(source)` pattern from actions.rs:1824):

```rust
fn book_navigation_app() -> (Cx, App) {
    let (mut cx, mut app) = navigation_app();
    let source = waml::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Guide](guide/)\n* [Plain](plain/)\n"),
        (
            "guide/index.md",
            "---\nview: book\n---\n# Guide\n\n* [Intro](intro.md)\n",
        ),
        ("guide/intro.md", "# Intro\n\nSome prose.\n"),
        ("plain/index.md", "# Plain\n\n* [Note](note.md)\n"),
        ("plain/note.md", "# Note\n"),
    ])
    .unwrap();
    app.session.replace(source).unwrap();
    (cx, app)
}

#[test]
fn a_directory_declaring_view_book_opens_on_the_book_surface() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/guide".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let tab = app.documents.active_tab().unwrap();
    assert_eq!(tab.locator().surface, waml::view::surface::SurfaceId::book());
    assert!(matches!(
        &tab.locator().target,
        waml::view::row::RowTarget::Folder(address) if address == "/guide"
    ));
}

#[test]
fn a_plain_directory_still_opens_the_folder_listing() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/plain".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        crate::view_history::DocumentLocator::folder("/plain")
    );
}

#[test]
fn the_book_header_toggle_drops_to_the_folder_listing_of_the_same_directory() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/guide".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let outcome = crate::doc_view::ViewOutcome {
        open_folder_listing: Some("/guide".to_string()),
        ..Default::default()
    };
    app.apply_view_outcome(&mut cx, outcome);
    assert_eq!(
        app.documents.active_tab().unwrap().locator(),
        crate::view_history::DocumentLocator::folder("/guide")
    );
}

#[test]
fn refreshing_folder_tabs_rebuilds_an_open_book_tab_in_place() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/guide".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let before = app.documents.active_id();
    app.refresh_folder_tabs(&mut cx);
    assert_eq!(app.documents.active_id(), before, "same tab identity after reopen-in-place");
    assert_eq!(
        app.documents.active_tab().unwrap().locator().surface,
        waml::view::surface::SurfaceId::book()
    );
}
```

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor a_directory_declaring_view_book`
Expected: COMPILE FAILURE on `open_folder_listing`; after adding the field alone, the first test FAILS with the active surface being `folder`, proving the routing is the change under test.

- [ ] **Step 3: Implement**

`app/navigation.rs`, beside `primary_locator` (~line 39):

```rust
    /// The folder sibling of `primary_locator`: the surface a DIRECTORY
    /// opens on when nothing requests one. A folder declaring `view: book`
    /// resolves to the book surface; everything else keeps today's listing.
    /// Asks the declared chain statically (`Chain::resolution_surface`)
    /// instead of running it -- a click site must not project rows twice.
    /// Deliberately narrowed to `book`: the `markdown`/`member:` folder
    /// resolutions are not yet consumed by any editor open path, and
    /// widening a click route is its own spec, not a side effect of this
    /// one.
    pub(super) fn primary_folder_locator(&self, address: &str) -> crate::navigation::DocumentLocator {
        let registry = crate::folder_projection::core_registry();
        let (chain, _diagnostics) = crate::folder_projection::chain_for(
            self.session.okf_analysis(),
            address,
            &self.projection_mask,
            &registry,
        );
        if chain.resolution_surface() == Some(waml::view::surface::SurfaceId::book()) {
            crate::navigation::DocumentLocator::new(
                waml::view::row::RowTarget::Folder(address.to_string()),
                waml::view::surface::SurfaceId::book(),
            )
        } else {
            crate::navigation::DocumentLocator::folder(address)
        }
    }
```

In the `Directory` arm (~line 420), the `ViewLocation` document becomes `self.primary_folder_locator(&address)` (keep the surrounding comment block; extend it with one sentence: "The locator is chain-routed: a `view: book` declaration opens the book surface, everything else the listing.").

`doc_view.rs`, `ViewOutcome` gains:

```rust
    /// Ask the shell to open this directory's LISTING surface -- the book
    /// header's toggle destination. A `NavigationTarget::Directory` cannot
    /// express this (the Directory arm chain-routes back to the book), so
    /// like `view_source` it names the surface explicitly.
    pub open_folder_listing: Option<String>,
```

`book_view.rs`, `handle` gains (before the default return; mirror `generic_okf_view.rs:128-136`):

```rust
        if body
            .header_view_action_button(cx)
            .as_icon_button()
            .clicked(actions)
        {
            let mut outcome = ViewOutcome::default();
            outcome.open_folder_listing = Some(self.directory.clone());
            return outcome;
        }
```

(This needs `use crate::icon_button::IconButtonWidgetRefExt;` — see generic_okf_view.rs:9.)

`app/actions.rs`, in `apply_view_outcome`, directly after the `outcome.navigation` block (~line 1756):

```rust
        if let Some(address) = outcome.open_folder_listing {
            self.transition_to_location(
                cx,
                ViewLocation {
                    document: crate::navigation::DocumentLocator::folder(&address),
                    anchor: ViewAnchor::None,
                },
                TransitionCause::UserNavigation,
            );
            flow = ActionFlow::Consumed;
        }
```

`app/navigation.rs`, book-tab refresh: generalize `refresh_folder_tabs`'s loop. Replace the body with one that walks BOTH surface kinds (keep the existing doc comments, extend them):

```rust
    pub(super) fn refresh_folder_tabs(&mut self, cx: &mut Cx) {
        for (directory, surface) in self.open_directory_tab_addresses() {
            let document = if surface == waml::view::surface::SurfaceId::book() {
                crate::book_documents::open(
                    self.session.okf_analysis(),
                    &directory,
                    self.chain_limits,
                    &self.projection_mask,
                )
            } else {
                crate::documents::open_folder(
                    self.session.okf_analysis(),
                    &directory,
                    self.chain_limits,
                    &self.projection_mask,
                )
            };
            let Some(document) = document else { continue; };
            self.documents.transition(
                cx,
                &self.ui,
                &self.session,
                DocumentCommand::ReopenInPlace { document },
            );
        }
    }

    /// The addresses of open tabs showing a directory SURFACE (listing or
    /// book) -- both halves of the locator matter, same reasoning as before:
    /// a folder's `source` tab shares the target but must not be rebuilt as
    /// a listing.
    pub(super) fn open_directory_tab_addresses(&self) -> Vec<(String, waml::view::surface::SurfaceId)> {
        self.documents
            .tabs()
            .iter()
            .filter_map(|tab| match &tab.locator.target {
                waml::view::row::RowTarget::Folder(address)
                    if tab.locator.surface == waml::view::surface::SurfaceId::folder()
                        || tab.locator.surface == waml::view::surface::SurfaceId::book() =>
                {
                    Some((address.clone(), tab.locator.surface.clone()))
                }
                _ => None,
            })
            .collect()
    }
```

Delete `open_folder_listing_addresses` and fix its callers (grep for it; `refresh_folder_tabs` was the only production caller — if a test calls it, update the test to the new fn).

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml-editor` (the four new tests plus every existing navigation test — Back/Forward over a book tab is covered free by `locator_opens`/`open_locator` parity from Task 3).
Expected: PASS.

- [ ] **Step 5: Full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(editor): route directory opens through the chain-declared book surface" -m "primary_folder_locator is the folder sibling of primary_locator: a
directory declaring view:book opens its book tab, everything else keeps
the listing -- asked statically off the declared chain so click sites
never project rows twice. The book header toggle drops back to the
listing via ViewOutcome.open_folder_listing (a NavigationTarget cannot
express it: the Directory arm now chain-routes). Mask changes re-run
open book tabs in place alongside folder listings so the book and the
tree cannot disagree about a directory's contents."
```

---

### Task 5: The `BookSurface` widget shell — DSL, owned scroll, virtualization policy

Virtualization is required, not an optimization: a live `MarkdownViewer` or `ClassDiagramSurface` per section will not survive a two-hundred-file book. The POLICY is pure math in a new `book_layout.rs` (per-kind estimates, prefix-sum tops, viewport window with one screen of margin, current-section-at-fold) so it is fully unit-tested headless; the widget only applies it.

The widget mirrors `tree_panel.rs`'s ownership model: it owns its scroll offset (wheel-adjusted, draw subtracts it) and a hand-drawn scrollbar, because a `View`-owned scroll cannot drive child virtualization decisions. Read `folder_list.rs` (whole file) for the `script_mod!`/`register_widget`/`Widget` impl idioms and `tree_panel.rs`'s scroll/wheel handling before writing any widget code.

**Makepad traps, load-bearing here:**
- Sections draw at fixed or `Fit` heights, NEVER `Fill` — a `draw_walk` rect goes stale after a `Size::Fill` sibling, which would corrupt every measured height after the first offender.
- The `book_surface` DSL wrapper mounts `BookSurface` alongside a background — never a LONE fixed child inside a fixed parent (blank-content trap); mirror `folder_view_surface`'s wrapper shape (app.rs ~line 505).
- ONE object literal per `script_mod!` namespace.
- `crate::book_surface::script_mod(vm)` must be registered in `App`'s registration list (app.rs ~line 1400-1480) BEFORE the App DSL is evaluated, with the same eager `mod.widgets.*` comment its neighbours carry — an unregistered child is a dead, invisible, unqueryable node that stays green in tests.

**Files:**
- Create: `crates/waml-editor/src/book_layout.rs`
- Create: `crates/waml-editor/src/book_surface.rs`
- Modify: `crates/waml-editor/src/lib.rs` (`mod book_layout; mod book_surface;`)
- Modify: `crates/waml-editor/src/app.rs` (DSL sibling beside `folder_view_surface` ~line 505; `script_mod` registration ~line 1445)
- Modify: `crates/waml-editor/src/doc_view.rs` (`BodyWidgets` field + `book_view_widget()` accessor, constructor ~line 39)
- Modify: `crates/waml-editor/src/book_view.rs` (`sync` pushes the model at the widget)

**Interfaces:**
- Consumes: `BookModel`/`BookSection`/`SectionBody` (Task 2), `BookView::model` (Task 3).
- Produces (consumed by Tasks 6-8):
  - `book_layout`: `pub const DIAGRAM_EMBED_HEIGHT: f64 = 400.0;` (the single legibility-cap constant to tune during visual sign-off), `pub const LIVE_MARGIN_SCREENS: f64 = 1.0;`, `pub fn estimated_height(body: &SectionBody) -> f64`, `pub fn section_tops(heights: &[f64]) -> Vec<f64>`, `pub fn live_window(tops: &[f64], heights: &[f64], scroll: f64, viewport: f64) -> std::ops::Range<usize>`, `pub fn current_section(tops: &[f64], scroll: f64) -> Option<usize>`.
  - `BookSurface` widget with `pub fn set_model(&mut self, cx: &mut Cx, model: Rc<BookModel>)`, `pub(crate) fn reconcile_live(&mut self, cx: &mut Cx, viewport_height: f64)`, `pub(crate) fn live_section_indices(&self) -> Vec<usize>`, `pub fn scroll_to_section(&mut self, cx: &mut Cx, index: usize)`, `pub(crate) fn current_section_index(&self) -> Option<usize>`, `pub(crate) fn scroll(&self) -> f64` (test accessor).
  - `BodyWidgets::book_view_widget(&self) -> WidgetRef` (looked up as `ids!(book_surface.book)` in the constructor, like `folder_list`).
  - The widget registration also mints `BookSurfaceRef` and an `as_book_surface()` ref-ext exactly as `FolderListView` mints `FolderListViewRef`/`as_folder_list_view` -- mirror that macro/derive shape; Tasks 6 and 8 hang their action accessors (`link_clicked`, `open_full_clicked`, `fold_moved`) off `BookSurfaceRef`.

- [ ] **Step 1: Write the failing layout tests**

Bottom of the new `book_layout.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn heights(n: usize, h: f64) -> Vec<f64> {
        vec![h; n]
    }

    #[test]
    fn section_tops_are_prefix_sums_of_heights() {
        assert_eq!(section_tops(&[10.0, 20.0, 30.0]), vec![0.0, 10.0, 30.0]);
        assert!(section_tops(&[]).is_empty());
    }

    #[test]
    fn the_live_window_covers_the_viewport_plus_one_screen_each_side() {
        let hs = heights(100, 100.0); // 10_000 tall book
        let tops = section_tops(&hs);
        // Viewport 600 tall, scrolled to 3000: visible 3000..3600, margin
        // 2400..4200 -> sections 24..42.
        assert_eq!(live_window(&tops, &hs, 3000.0, 600.0), 24..42);
        // Top of the book clamps at 0.
        assert_eq!(live_window(&tops, &hs, 0.0, 600.0), 0..12);
        // Bottom clamps at len.
        assert_eq!(live_window(&tops, &hs, 9800.0, 600.0), 92..100);
    }

    #[test]
    fn an_empty_book_has_an_empty_live_window() {
        assert_eq!(live_window(&[], &[], 0.0, 600.0), 0..0);
    }

    #[test]
    fn the_current_section_is_the_nearest_top_at_or_above_the_fold() {
        let hs = heights(5, 100.0);
        let tops = section_tops(&hs);
        assert_eq!(current_section(&tops, 0.0), Some(0));
        assert_eq!(current_section(&tops, 150.0), Some(1));
        assert_eq!(current_section(&tops, 100.0), Some(1));
        assert_eq!(current_section(&tops, 5000.0), Some(4));
        assert_eq!(current_section(&[], 0.0), None);
    }

    #[test]
    fn estimates_are_per_kind_and_the_diagram_estimate_is_the_cap() {
        use crate::book_model::{LinkReason, SectionBody};
        let heading = estimated_height(&SectionBody::Heading);
        let link = estimated_height(&SectionBody::Link {
            reason: LinkReason::NestedBook,
        });
        assert!(heading > link, "a heading reserves more than a link row");
        // The diagram estimate includes the caption strip above the cap.
        // (Constructing Prose/Diagram bodies needs real documents; the two
        // constants are asserted directly instead.)
        assert!(DIAGRAM_EMBED_HEIGHT > 0.0);
    }
}
```

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor book_layout`
Expected: COMPILE FAILURE — module does not exist.

- [ ] **Step 3: Implement `book_layout.rs`**

```rust
//! The book's virtualization policy, as pure math (spec: "Virtualization is
//! required, not an optimization"). The widget applies these answers; keeping
//! them makepad-free is what makes a two-hundred-section policy testable
//! without a window.

use crate::book_model::SectionBody;

/// The one legibility cap for inline diagram embeds -- a single constant so
/// visual sign-off tunes one number, not a scatter of literals.
pub const DIAGRAM_EMBED_HEIGHT: f64 = 400.0;
/// The caption strip (title + open-full button) above a diagram embed.
pub const DIAGRAM_CAPTION_HEIGHT: f64 = 28.0;
/// Screens of margin either side of the viewport that stay live.
pub const LIVE_MARGIN_SCREENS: f64 = 1.0;

/// A never-measured section reserves this until first drawn; a measured one
/// keeps its last real height (cached by the widget, keyed by RowId).
pub fn estimated_height(body: &SectionBody) -> f64 {
    match body {
        SectionBody::Heading => 56.0,
        SectionBody::Prose { .. } => 320.0,
        SectionBody::Diagram { .. } => DIAGRAM_EMBED_HEIGHT + DIAGRAM_CAPTION_HEIGHT,
        SectionBody::Link { .. } => 32.0,
    }
}

pub fn section_tops(heights: &[f64]) -> Vec<f64> {
    let mut tops = Vec::with_capacity(heights.len());
    let mut y = 0.0;
    for h in heights {
        tops.push(y);
        y += h;
    }
    tops
}

/// The half-open index range of sections intersecting the viewport plus
/// [`LIVE_MARGIN_SCREENS`] each side. Only these hold live child widgets.
pub fn live_window(
    tops: &[f64],
    heights: &[f64],
    scroll: f64,
    viewport: f64,
) -> std::ops::Range<usize> {
    if tops.is_empty() {
        return 0..0;
    }
    let margin = viewport * LIVE_MARGIN_SCREENS;
    let lo = scroll - margin;
    let hi = scroll + viewport + margin;
    // First section whose bottom is past `lo`; first section whose top is
    // past `hi`. Tops are sorted, so partition_point is exact for the end.
    let start = tops
        .iter()
        .zip(heights)
        .position(|(&top, &h)| top + h > lo)
        .unwrap_or(tops.len());
    let end = tops.partition_point(|&top| top < hi);
    start..end.max(start)
}

/// The section whose top is nearest AT OR ABOVE the fold (`scroll`): the
/// reader is "in" it. `None` only for an empty book.
pub fn current_section(tops: &[f64], scroll: f64) -> Option<usize> {
    if tops.is_empty() {
        return None;
    }
    Some(tops.partition_point(|&top| top <= scroll).saturating_sub(1))
}
```


- [ ] **Step 4: Layout tests pass**

Run: `cargo test -p waml-editor book_layout` — PASS.

- [ ] **Step 5: Write the failing widget tests**

Bottom of the new `book_surface.rs`. Headless `Cx` widget construction mirrors `generic_okf_view.rs:217-222` and `app/tests/navigation.rs:100-127`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn model() -> Rc<crate::book_model::BookModel> {
        let mut session = crate::editor_session::EditorSession::default();
        // 40 prose sections: far more than a 600px viewport holds at the
        // 320px prose estimate.
        let mut pairs = vec![(
            "index.md".to_string(),
            {
                let mut index = String::from("---\nview: book\n---\n# Big\n\n");
                for i in 0..40 {
                    index.push_str(&format!("* [S{i}](s{i}.md)\n"));
                }
                index
            },
        )];
        for i in 0..40 {
            pairs.push((format!("s{i}.md"), format!("# S{i}\n\nBody.\n")));
        }
        session
            .replace(
                waml::source::SourceBundle::try_from_pairs(
                    pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())),
                )
                .unwrap(),
            )
            .unwrap();
        Rc::new(
            crate::book_model::build_book(
                &session.snapshot(),
                "/",
                waml::view::chain::ChainLimits::default(),
                &waml::view::mask::ProjectionMask::default(),
            )
            .unwrap(),
        )
    }

    fn surface(cx: &mut Cx) -> BookSurface {
        cx.with_vm(BookSurface::script_new_with_default)
    }

    #[test]
    fn only_viewport_adjacent_sections_hold_live_children() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.reconcile_live(&mut cx, 600.0);
        let live = book.live_section_indices();
        assert!(!live.is_empty());
        assert!(
            live.len() < 40,
            "a 40-section book must not instantiate 40 live children, got {live:?}"
        );
        // Scrolled to the bottom, the live window moves with it.
        book.scroll_to_section(&mut cx, 39);
        book.reconcile_live(&mut cx, 600.0);
        let live_at_end = book.live_section_indices();
        assert!(live_at_end.contains(&39));
        assert!(!live_at_end.contains(&0));
    }

    #[test]
    fn scroll_to_section_lands_the_sections_top_at_the_fold() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.scroll_to_section(&mut cx, 5);
        assert_eq!(book.current_section_index(), Some(5));
    }

    #[test]
    fn a_model_swap_drops_stale_children_and_clamps_scroll() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let mut book = surface(&mut cx);
        book.set_model(&mut cx, model());
        book.scroll_to_section(&mut cx, 39);
        book.set_model(&mut cx, model());
        // A rebuild keyed on the same RowIds keeps the reader's place
        // (the mint/resolve round-trip invariant in root.rs is exactly this
        // promise); scroll is preserved, not reset to zero.
        assert!(book.scroll() > 0.0);
        book.reconcile_live(&mut cx, 600.0);
        assert!(!book.live_section_indices().is_empty());
    }
}
```

- [ ] **Step 6: Run and see them fail**

Run: `cargo test -p waml-editor book_surface`
Expected: COMPILE FAILURE.

- [ ] **Step 7: Implement `book_surface.rs` (shell) and mount it**

Widget skeleton — mirror `folder_list.rs`'s `FolderListView` for the derive/`register_widget`/`Widget` impl idioms and `tree_panel.rs` for wheel + hand-drawn scrollbar. The state and logic:

```rust
//! The book's one drawing surface: a virtualized vertical scroll of
//! sections. Owns its scroll offset (like tree_panel's TreeLayout) because a
//! View-owned scroll cannot drive the live-child window; sections draw at
//! fixed or Fit heights, NEVER Fill (a draw_walk rect goes stale after a
//! Size::Fill sibling -- the measured-height cache would corrupt).

pub struct BookSurface {
    // ... makepad boilerplate fields exactly as FolderListView declares
    // (view/area/draw state), plus:
    model: Option<Rc<crate::book_model::BookModel>>,
    /// Measured heights by RowId -- survives a model rebuild so a reloaded
    /// book keeps its layout; invalidated per-row when the rebuild changes
    /// that row's body (compare Rc identity of Prose docs / scene equality).
    measured: HashMap<waml::view::row::RowId, f64>,
    heights: Vec<f64>,
    tops: Vec<f64>,
    scroll: f64,
    live: HashMap<usize, WidgetRef>,
    last_viewport: f64,
}

impl BookSurface {
    pub fn set_model(&mut self, cx: &mut Cx, model: Rc<crate::book_model::BookModel>) {
        self.live.clear(); // children are per-section; a swap re-creates lazily
        self.model = Some(model);
        self.rebuild_layout();
        let total: f64 = self.heights.iter().sum();
        self.scroll = self.scroll.min((total - self.last_viewport).max(0.0));
        self.redraw(cx);
    }

    fn rebuild_layout(&mut self) {
        let Some(model) = &self.model else { /* clear vecs */ return; };
        self.heights = model
            .sections
            .iter()
            .map(|s| {
                self.measured
                    .get(&s.row_id)
                    .copied()
                    .unwrap_or_else(|| crate::book_layout::estimated_height(&s.body))
            })
            .collect();
        self.tops = crate::book_layout::section_tops(&self.heights);
    }

    pub(crate) fn reconcile_live(&mut self, cx: &mut Cx, viewport_height: f64) {
        self.last_viewport = viewport_height;
        let window =
            crate::book_layout::live_window(&self.tops, &self.heights, self.scroll, viewport_height);
        self.live.retain(|index, _| window.contains(index));
        let Some(model) = self.model.clone() else { return; };
        for index in window {
            if self.live.contains_key(&index) {
                continue;
            }
            if let Some(child) = self.make_child(cx, &model.sections[index]) {
                self.live.insert(index, child);
            }
        }
    }

    /// The shell holds a PLACEHOLDER child per Prose/Diagram section (a bare
    /// View) so the virtualization tests exercise real child lifecycle now;
    /// Task 6 swaps the placeholders for MarkdownViewer/ClassDiagramSurface.
    /// Heading and Link sections never hold a child -- they draw
    /// immediate-mode.
    fn make_child(&self, cx: &mut Cx, section: &crate::book_model::BookSection) -> Option<WidgetRef> {
        match &section.body {
            crate::book_model::SectionBody::Prose { .. }
            | crate::book_model::SectionBody::Diagram { .. } => Some(WidgetRef::new_with_inner(
                Box::new(cx.with_vm(View::script_new_with_default)),
            )),
            crate::book_model::SectionBody::Heading
            | crate::book_model::SectionBody::Link { .. } => None,
        }
    }

    pub(crate) fn live_section_indices(&self) -> Vec<usize> {
        let mut indices: Vec<usize> = self.live.keys().copied().collect();
        indices.sort_unstable();
        indices
    }

    pub fn scroll_to_section(&mut self, cx: &mut Cx, index: usize) {
        if let Some(&top) = self.tops.get(index) {
            self.scroll = top;
            self.redraw(cx);
        }
    }

    pub(crate) fn current_section_index(&self) -> Option<usize> {
        crate::book_layout::current_section(&self.tops, self.scroll)
    }

    pub(crate) fn scroll(&self) -> f64 {
        self.scroll
    }
}
```

`draw_walk` (shell version): walk the turtle for the surface rect; call `self.reconcile_live(cx, rect.size.y)`; then per section, translated by `-self.scroll`: live children draw with `Walk::fixed_size(...)`/`Height::Fit`; a non-live section reserves its cached height with an empty fixed walk. Record each live section's actually-drawn height into `measured` (keyed by `row_id`) and rebuild `tops` when any measurement changed. Headings and Links draw immediate-mode text (mirror `folder_list.rs`'s `DrawText` usage; heading font size steps down with `section.depth`, plus a rule line). Wheel events adjust `self.scroll` clamped to `0.0..=(total - viewport).max(0.0)` and trigger `reconcile_live` + redraw — mirror `tree_panel.rs`'s scroll handling verbatim, including the hand-drawn scrollbar.

The placeholder children in `make_child` above are the shell's own deliverable, not a stub: they give the virtualization tests a real child lifecycle to assert against, and Task 6 swaps them for the viewer/canvas children without touching the window logic.

`script_mod!` block (one object literal namespace):

```rust
script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.BookSurfaceBase = #(BookSurface::register_widget(vm))
    mod.widgets.BookSurface = set_type_default() do mod.widgets.BookSurfaceBase{
        width: Fill
        height: Fill
    }
}
```

`app.rs` DSL, sibling directly after `folder_view_surface` (mirror its wrapper: `width: Fill height: Fill visible: false show_bg: true` + background — never a lone fixed child):

```text
book_surface := View{
    width: Fill
    height: Fill
    visible: false
    show_bg: true
    // same draw_bg as folder_view_surface
    book := BookSurface{
        width: Fill
        height: Fill
    }
}
```

(`width/height: Fill` on the WIDGET is the surface filling the body slot — the Fill trap is about SECTION children inside `draw_walk`, which stay fixed/Fit.)

`app.rs` registration, beside `crate::tree_panel::script_mod(vm);` (~line 1445):

```rust
        // `BookSurface` is mounted by App's own live layout (`book_surface.book`),
        // so it must register before the App DSL is evaluated -- the DSL
        // resolves `mod.widgets.*` eagerly at `use`-time, not lazily; an
        // unregistered child is a dead, invisible, unqueryable node.
        crate::book_surface::script_mod(vm);
```

`doc_view.rs`: `BodyWidgets` gains a `book: WidgetRef` field (constructor: `ui.widget(_cx, ids!(book_surface.book))`) and:

```rust
    pub fn book_view_widget(&self) -> WidgetRef {
        self.book.clone()
    }
```

`book_view.rs` `sync` becomes:

```rust
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, _data: ViewData<'_>) {
        body.show_book_view(cx);
        if let (Some(model), Some(mut widget)) = (
            self.model.clone(),
            body.book_view_widget().borrow_mut::<crate::book_surface::BookSurface>(),
        ) {
            widget.set_model(cx, model);
        }
    }
```

- [ ] **Step 8: Run and see them pass**

Run: `cargo test -p waml-editor` — the three widget tests, five layout tests, and every existing test PASS.

- [ ] **Step 9: Full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(editor): virtualized book surface widget shell" -m "The virtualization policy (per-kind estimates, prefix-sum tops, a
viewport-plus-one-screen live window, current-section-at-fold) is pure
math in book_layout.rs so a two-hundred-section policy is testable
with no window; the widget only applies it. BookSurface owns its
scroll like tree_panel's TreeLayout because a View-owned scroll cannot
drive the live-child window. Sections draw fixed or Fit, never Fill --
a draw_walk rect goes stale after a Fill sibling, which would corrupt
the measured-height cache. Heights are cached by RowId so a reloaded
book keeps its place."
```

---

### Task 6: Inline embeds and section clicks — prose, diagrams, links

The live children become real: a Prose section holds a per-section `MarkdownViewer` (`Height::Fit`, non-interactive in Phase 1), a Diagram section holds a caption strip (title + open-full `IconButton`) above a `ClassDiagramSurface` given a FIXED-height walk with interaction off (it fits the scene to that rect on first draw — read `crates/waml-editor/src/canvas/class/widget.rs:683-690, 802-812, 884-886`). A Link section is one row: icon and title; clicking it opens that row's own tab through the existing navigation path, exactly as a tree click would.

**Files:**
- Modify: `crates/waml-editor/src/book_surface.rs` (real `make_child`, section drawing, hit areas, actions, `Ref` accessors)
- Modify: `crates/waml-editor/src/book_view.rs` (`handle` maps actions to `NavigationIntent`; `navigation_for_section` helper with unit tests)

**Interfaces:**
- Consumes: `MarkdownViewer` + `install_document` (`waml_markdown_editor::reading`, as `reading_view.rs:89-93` calls it), `ClassDiagramSurface::{set_scene, set_interaction_enabled}`, `crate::icon_button::IconButton`, `book_layout::{DIAGRAM_EMBED_HEIGHT, DIAGRAM_CAPTION_HEIGHT}`.
- Produces (consumed by Task 8's accessor pattern):
  - `enum BookSurfaceAction { None, LinkClicked(usize), OpenFullClicked(usize), FoldMoved(usize) }` (`FoldMoved` emitted in Task 8; declare it here so the enum is stable — reference it from Task 8's emitter, and mark nothing dead because the enum variants are constructed via actions).
  - `BookSurfaceRef::link_clicked(&self, actions) -> Option<usize>`, `BookSurfaceRef::open_full_clicked(&self, actions) -> Option<usize>` (mirror `folder_list.rs`'s `FolderListView` action-accessor pattern, e.g. `row_opened`).
  - `book_view::navigation_for_section(section: &BookSection) -> Option<NavigationIntent>` (pub(crate)).

- [ ] **Step 1: Write the failing tests**

`book_view.rs` tests (pure mapping — the click contract):

```rust
#[test]
fn a_link_section_navigates_to_its_own_rows_tab() {
    // Build the Task 2 fixture model; take the "Inner" nested-book section.
    let model = built_model(); // helper: build_book over book_source(), "/guide"
    let inner = model.sections.iter().find(|s| s.title == "Inner").unwrap();
    let Some(crate::navigation::NavigationIntent::Resolved { target, disposition }) =
        crate::book_view::navigation_for_section(inner)
    else {
        panic!("a link section must resolve to a navigation");
    };
    assert!(matches!(
        target,
        crate::navigation::NavigationTarget::Directory { ref address } if address == "/guide/inner"
    ));
    assert_eq!(disposition, crate::navigation::OpenDisposition::Preview);
}

#[test]
fn open_full_on_a_diagram_section_navigates_to_the_concept() {
    let model = built_model();
    let flow = model.sections.iter().find(|s| s.title == "Flow").unwrap();
    let Some(crate::navigation::NavigationIntent::Resolved { target, .. }) =
        crate::book_view::navigation_for_section(flow)
    else {
        panic!("a diagram section must resolve to a navigation");
    };
    assert!(matches!(
        target,
        crate::navigation::NavigationTarget::Document { ref concept_id, .. }
            if concept_id == "guide/flow"
    ));
}
```

`book_surface.rs` tests (child kinds, extending Task 5's module — the placeholder assertion upgrades):

```rust
#[test]
fn live_children_match_their_section_kind() {
    // Same headless construction as Task 5's tests, over the Task 2 fixture
    // (prose + diagram + heading + link). After reconcile_live at the top:
    // the prose section's child borrows as MarkdownViewer, the diagram
    // section's as ClassDiagramSurface, and Heading/Link sections hold NO
    // child (they are immediate-mode rows).
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let mut book = surface(&mut cx);
    book.set_model(&mut cx, guide_model()); // Task 2 fixture, "/guide"
    book.reconcile_live(&mut cx, 2000.0);
    let live = book.live_children_for_test(); // Vec<(usize, WidgetRef)>
    let viewer_count = live
        .iter()
        .filter(|(_, w)| w.borrow::<waml_markdown_editor::reading::MarkdownViewer>().is_some())
        .count();
    let canvas_count = live
        .iter()
        .filter(|(_, w)| w.borrow::<crate::canvas::ClassDiagramSurface>().is_some())
        .count();
    assert!(viewer_count >= 2, "Intro and Leaf are prose");
    assert_eq!(canvas_count, 1, "Flow is the one diagram embed");
}
```

(Add the `pub(crate) fn live_children_for_test(&self) -> Vec<(usize, WidgetRef)>` accessor under `#[cfg(test)]`.)

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor navigation_for_section` and `cargo test -p waml-editor live_children_match`
Expected: COMPILE FAILURE (missing fns), then assertion failure while `make_child` still returns placeholders.

- [ ] **Step 3: Implement**

`book_surface.rs` `make_child` becomes:

```rust
    fn make_child(&self, cx: &mut Cx, section: &crate::book_model::BookSection) -> Option<WidgetRef> {
        match &section.body {
            crate::book_model::SectionBody::Prose { document, source } => {
                let child = WidgetRef::new_with_inner(Box::new(cx.with_vm(
                    waml_markdown_editor::reading::MarkdownViewer::script_new_with_default,
                )));
                child
                    .as_markdown_viewer()
                    .install_document(cx, document.clone(), source.clone());
                Some(child)
            }
            crate::book_model::SectionBody::Diagram { scene, .. } => {
                let child = WidgetRef::new_with_inner(Box::new(cx.with_vm(
                    crate::canvas::ClassDiagramSurface::script_new_with_default,
                )));
                if let Some(mut canvas) = child.borrow_mut::<crate::canvas::ClassDiagramSurface>() {
                    canvas.set_scene(cx, scene.clone());
                    // Read-only embed: the surface fits the scene to its rect
                    // on first draw; interaction stays off in Phase 1.
                    canvas.set_interaction_enabled(cx, false);
                }
                Some(child)
            }
            crate::book_model::SectionBody::Heading | crate::book_model::SectionBody::Link { .. } => None,
        }
    }
```

Drawing per section in `draw_walk` (all fixed/Fit, never Fill):
- **Heading:** immediate-mode `DrawText` at a size stepped down by `depth` (e.g. `22.0 - 2.0 * depth as f64`, floor 14.0), then a 1px rule; mirror `folder_list.rs`'s text-draw idiom.
- **Prose:** the live child drawn with `Walk { height: Size::Fit, .. }`; after the child draws, read its drawn rect height into `measured`.
- **Diagram:** caption strip (`DIAGRAM_CAPTION_HEIGHT`): section title left, an `IconButton` (created programmatically like the viewer children, `Icon::ExternalLink` if it exists, else `Icon::ArrowRight`) right; then the canvas child with `Walk::fixed(Size::Fill-width, DIAGRAM_EMBED_HEIGHT)` — fixed HEIGHT; width fills the column, which is safe (the Fill trap is vertical stacking of sibling rects).
- **Link:** one 32px row, row icon + title, hover state; register a finger hit area per drawn Link row and per open-full button, mapping `FingerUp` to `cx.widget_action(uid, &scope.path, BookSurfaceAction::LinkClicked(index))` / `OpenFullClicked(index)` — mirror how `folder_list.rs` emits `FolderListViewAction` and how `tree_panel.rs` caches per-row rects for hit testing.

Action accessors on the widget ref (mirror `folder_list.rs`'s accessor shape exactly):

```rust
impl BookSurfaceRef {
    pub fn link_clicked(&self, actions: &Actions) -> Option<usize> { /* find_widget_action + cast */ }
    pub fn open_full_clicked(&self, actions: &Actions) -> Option<usize> { /* likewise */ }
}
```

`book_view.rs`:

```rust
/// One section's click destination, through the SAME navigation path a tree
/// click takes -- the book adds no second way to open things (spec: "opens
/// that concept's own tab through the existing navigation path").
pub(crate) fn navigation_for_section(
    section: &crate::book_model::BookSection,
) -> Option<crate::navigation::NavigationIntent> {
    let target = match &section.target {
        waml::view::row::RowTarget::Concept(id) => {
            crate::navigation::NavigationTarget::Document {
                concept_id: id.clone(),
                surface: None,
                fragment: None,
            }
        }
        waml::view::row::RowTarget::Folder(address) => {
            crate::navigation::NavigationTarget::Directory { address: address.clone() }
        }
        waml::view::row::RowTarget::Virtual => return None,
    };
    Some(crate::navigation::NavigationIntent::Resolved {
        target,
        disposition: crate::navigation::OpenDisposition::Preview,
    })
}
```

`BookView::handle` gains, after the toggle branch:

```rust
        let book = body.book_view_widget().as_book_surface();
        let clicked = book
            .link_clicked(actions)
            .or_else(|| book.open_full_clicked(actions));
        if let (Some(index), Some(model)) = (clicked, self.model.as_ref()) {
            if let Some(section) = model.sections.get(index) {
                let mut outcome = ViewOutcome::default();
                outcome.navigation = navigation_for_section(section);
                return outcome;
            }
        }
```

(If `NavigationTarget::Document`'s `surface` field is not `Option<SurfaceId>`, mirror its actual type from `navigation.rs` — `None`/absent means "primary", which is what both clicks want: a diagram's primary is canvas, a nested book's Directory routes through `primary_folder_locator`.)

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml-editor` — PASS, including Task 5's virtualization tests now running against real children.

- [ ] **Step 5: Full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(editor): inline prose and diagram embeds in the book surface" -m "Prose sections hold a per-section MarkdownViewer at Fit height;
diagram sections hold a caption strip above a ClassDiagramSurface given
a fixed-height walk with interaction off -- the surface already fits
its scene to an arbitrary rect on first draw, so the embed needs no new
canvas code. Links and open-full route through navigation_for_section
into the existing NavigationIntent path, so a book click can only ever
open exactly the tab a tree click would."
```

---

### Task 7: Tree -> book — a tree click on a section reveals instead of opening

When the active view is a book and the clicked tree row is one of its sections, the click resolves to a reveal instead of an open (decision 4: the tree stays one tree). `RevealTarget` gains a `Row { id: RowId }` variant; `DocView::reveal` already has a no-op default, so no other view changes. A tree row NOT in the active book opens a tab exactly as today.

The intercept lives in `handle_tree_navigation` (app/actions.rs:1072) — the ONE place tree clicks enter — not in `navigate_with`, so palette commits, markdown links, and breadcrumbs keep opening tabs even while a book is active.

**Files:**
- Modify: `crates/waml-editor/src/doc_view.rs` (`RevealTarget` ~line 544; `DocView` trait — `reveal_target_for` default beside `mark_search_cursor` ~line 668)
- Modify: `crates/waml-editor/src/document_host.rs` (`active_reveal_for_target` beside `reveal_active` ~line 297)
- Modify: `crates/waml-editor/src/book_view.rs` (`reveal_target_for` + `reveal` impls)
- Modify: `crates/waml-editor/src/app/actions.rs` (`handle_tree_navigation` intercept + `row_target_of` helper)
- Test: `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Consumes: `BookView::model`, `BookSurface::scroll_to_section`, `DocumentHost.views` (private map — the new method lives inside `document_host.rs`).
- Produces: `RevealTarget::Row { id: waml::view::row::RowId }`; `DocView::reveal_target_for(&self, target: &RowTarget) -> Option<RevealTarget>` (default `None`); `DocumentHost::active_reveal_for_target(&self, target: &RowTarget) -> Option<RevealTarget>`; `App::try_reveal_in_active_book(&mut self, cx, target: &NavigationTarget) -> bool` (pub(super), called by `handle_tree_navigation`, named for what it does today while staying view-agnostic in type).

- [ ] **Step 1: Write the failing tests**

`app/tests/navigation.rs` (reusing Task 4's `book_navigation_app`):

```rust
#[test]
fn a_tree_click_on_a_section_of_the_active_book_reveals_instead_of_opening() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/guide".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let tabs_before = app.documents.tabs().len();
    let handled = app.try_reveal_in_active_book(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "guide/intro".to_string(),
            surface: None,
            fragment: None,
        },
    );
    assert!(handled, "a section concept must resolve to a reveal");
    assert_eq!(app.documents.tabs().len(), tabs_before, "no new tab");
    assert_eq!(
        app.documents.active_tab().unwrap().locator().surface,
        waml::view::surface::SurfaceId::book(),
        "the book stays active"
    );
}

#[test]
fn a_tree_click_outside_the_active_book_still_opens_a_tab() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/guide".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    let handled = app.try_reveal_in_active_book(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "plain/note".to_string(),
            surface: None,
            fragment: None,
        },
    );
    assert!(!handled, "a non-section target falls through to the open path");
}

#[test]
fn a_tree_click_reveals_nothing_when_a_folder_listing_is_active() {
    let (mut cx, mut app) = book_navigation_app();
    assert!(app.navigate_with(
        &mut cx,
        NavigationTarget::Directory { address: "/plain".to_string() },
        crate::navigation::OpenDisposition::Preview,
        &mut FakeBrowser::default(),
    ));
    assert!(!app.try_reveal_in_active_book(
        &mut cx,
        &NavigationTarget::Document {
            concept_id: "plain/note".to_string(),
            surface: None,
            fragment: None,
        },
    ));
}
```

NOTE: the first test needs the book's MODEL built (reveal_target_for reads it), and the headless `navigate_with` path reaches `BookView::sync_from_session` through the document host's open/restore machinery. If the model turns out not to be built in this headless path (assert `handled` fails), have `reveal_target_for` build on demand is NOT an option (no snapshot in scope) — instead drive one explicit `app.documents.sync_active(&mut cx, &app.ui, &app.session);` after the navigate and re-check; whichever call populates it, encode that call in the test with a comment saying why.

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor try_reveal_in_active_book` (compile failure), then after stubs, the first test fails with `handled == false`.

- [ ] **Step 3: Implement**

`doc_view.rs`:

```rust
pub enum RevealTarget {
    TextSpan { start: u32, end: u32 },
    ModelElement { key: String },
    /// A projected row inside a composite surface (the book): scroll that
    /// row's section to the fold. Minted by the view that owns the rows
    /// (`DocView::reveal_target_for`), because only it knows which RowIds it
    /// is showing; every other view keeps its no-op `reveal` default.
    Row { id: waml::view::row::RowId },
}
```

Trait method beside `mark_search_cursor` (same "no-op default, one implementor" shape):

```rust
    /// Translate a navigation target into a reveal THIS view can service, or
    /// `None` to let the ordinary open path run. Only the book answers today
    /// (a tree click on a section scrolls instead of opening -- spec
    /// decision 4); the default keeps every other view's click behavior
    /// byte-for-byte unchanged.
    fn reveal_target_for(&self, target: &waml::view::row::RowTarget) -> Option<RevealTarget> {
        let _ = target;
        None
    }
```

`document_host.rs`, beside `reveal_active`:

```rust
    pub fn active_reveal_for_target(
        &self,
        target: &waml::view::row::RowTarget,
    ) -> Option<RevealTarget> {
        self.views
            .get(&self.tabs.active)
            .and_then(|view| view.reveal_target_for(target))
    }
```

`book_view.rs`:

```rust
    fn reveal_target_for(&self, target: &waml::view::row::RowTarget) -> Option<crate::doc_view::RevealTarget> {
        let model = self.model.as_ref()?;
        model
            .sections
            .iter()
            .find(|section| &section.target == target)
            .map(|section| crate::doc_view::RevealTarget::Row { id: section.row_id.clone() })
    }

    fn reveal(&mut self, cx: &mut Cx, body: &BodyWidgets, target: &crate::doc_view::RevealTarget) {
        let crate::doc_view::RevealTarget::Row { id } = target else {
            return; // text/model reveals do not apply to a book
        };
        let Some(model) = self.model.as_ref() else { return; };
        let Some(index) = model.sections.iter().position(|s| &s.row_id == id) else {
            return;
        };
        if let Some(mut widget) = body
            .book_view_widget()
            .borrow_mut::<crate::book_surface::BookSurface>()
        {
            widget.scroll_to_section(cx, index);
        }
    }
```

`app/actions.rs`:

```rust
    /// The tree-click intercept (spec: "the tree stays one tree"): when the
    /// ACTIVE view can show the clicked row in place, scroll it there instead
    /// of opening a tab. Sits in the tree's own action handler, NOT in
    /// navigate_with, so palette commits, markdown links, and breadcrumbs
    /// keep opening tabs while a book is active.
    pub(super) fn try_reveal_in_active_book(
        &mut self,
        cx: &mut Cx,
        target: &crate::navigation::NavigationTarget,
    ) -> bool {
        let row_target = match target {
            crate::navigation::NavigationTarget::Document { concept_id, .. } => {
                waml::view::row::RowTarget::Concept(concept_id.clone())
            }
            crate::navigation::NavigationTarget::Directory { address } => {
                waml::view::row::RowTarget::Folder(address.clone())
            }
            crate::navigation::NavigationTarget::ExternalUrl(_) => return false,
        };
        let Some(reveal) = self.documents.active_reveal_for_target(&row_target) else {
            return false;
        };
        self.documents.reveal_active(cx, &self.ui, &reveal)
    }
```

`handle_tree_navigation` (~line 1078), between the intent extraction and `handle_navigation_intent`:

```rust
        if let crate::navigation::NavigationIntent::Resolved { target, .. } = &intent {
            if self.try_reveal_in_active_book(cx, target) {
                return ActionFlow::Consumed;
            }
        }
```

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml-editor` — the three new tests plus every existing reveal/search test (the new `RevealTarget::Row` variant must not break exhaustive matches; fix any non-wildcard `match` on `RevealTarget` the compiler reports, keeping each view's behavior for `Row` a no-op).
Expected: PASS.

- [ ] **Step 5: Full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(editor): tree click reveals the section in an active book" -m "RevealTarget grows a Row variant minted by the view that owns the rows
(DocView::reveal_target_for, no-op default -- the same shape as
mark_search_cursor). The intercept lives in handle_tree_navigation, the
one entry point for tree clicks, so every other navigation source keeps
opening tabs; a clicked row outside the active book falls through to
the ordinary open path unchanged."
```

---

### Task 8: Book -> tree — scrolling the book marks the current section in the tree

On scroll, the nearest section top at or above the fold is the current section; its target is handed to the tree's existing reveal path (`ProjectTree::reveal_target`, tree_panel.rs:1166), which already owns unfold + scroll-into-view + the highlight pulse. The widget emits `FoldMoved(index)` only when the current section CHANGES (never per scroll tick), `BookView::handle` translates it to a `NavigationTarget`, and the shell applies it — the view never touches the tree directly (shell owns cross-surface effects, spec §3).

**Files:**
- Modify: `crates/waml-editor/src/book_surface.rs` (emit `FoldMoved` on current-section change inside the wheel/scroll handler; `BookSurfaceRef::fold_moved(actions) -> Option<usize>` accessor)
- Modify: `crates/waml-editor/src/book_view.rs` (`handle` maps `FoldMoved` to `ViewOutcome.tree_mark`)
- Modify: `crates/waml-editor/src/doc_view.rs` (`ViewOutcome.tree_mark: Option<crate::navigation::NavigationTarget>`)
- Modify: `crates/waml-editor/src/app/actions.rs` (`apply_view_outcome` applies `tree_mark` via `ProjectTree::reveal_target`; does NOT set `flow = Consumed` — marking is a side effect, not a claim on the event)
- Modify: `crates/waml-editor/src/tree_panel.rs` (`#[cfg(test)] pub(crate) fn test_reveal_key(&self) -> Option<&str>`)
- Test: `crates/waml-editor/src/app/tests/navigation.rs`, `book_surface.rs` tests

**Interfaces:**
- Consumes: `book_layout::current_section`, `ProjectTree::reveal_target(&mut self, cx, &NavigationTarget) -> bool`, `book_view::navigation_for_section` (Task 6 — reuse its target arm by splitting out `pub(crate) fn navigation_target_for(target: &RowTarget) -> Option<NavigationTarget>` and having `navigation_for_section` wrap it; update Task 6's callers).
- Produces: `ViewOutcome.tree_mark`.

- [ ] **Step 1: Write the failing tests**

`book_surface.rs`:

```rust
#[test]
fn crossing_a_section_boundary_updates_the_current_section_once() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let mut book = surface(&mut cx);
    book.set_model(&mut cx, model());
    assert_eq!(book.current_section_index(), Some(0));
    book.scroll_to_section(&mut cx, 3);
    assert_eq!(book.current_section_index(), Some(3));
    // The change gate: the SAME index twice must not re-emit. Assert via the
    // widget's own `last_marked` state accessor rather than the action queue
    // (headless tests have no action collection loop):
    assert!(book.take_fold_moved_for_test(), "first crossing marks");
    assert!(!book.take_fold_moved_for_test(), "no re-mark without movement");
}
```

(Emitting through `cx.widget_action` AND tracking `last_marked: Option<usize>` internally; `take_fold_moved_for_test` is a `#[cfg(test)]` accessor over the same gate the emitter uses.)

`app/tests/navigation.rs` — the shell applies the mark to a real mounted tree:

```rust
#[test]
fn a_tree_mark_outcome_scrolls_and_pulses_the_tree_row() {
    let (mut cx, mut app) = book_navigation_app();
    // Mount a real ProjectTree as `project_tree`, the same explicit-mount
    // pattern diagram_properties_app uses for its widgets.
    let tree = WidgetRef::new_with_inner(Box::new(
        cx.with_vm(crate::tree_panel::ProjectTree::script_new_with_default),
    ));
    let mut ui = cx.with_vm(View::script_new_with_default);
    ui.children.push((live_id!(project_tree), tree));
    app.ui = WidgetRef::new_with_inner(Box::new(ui));
    app.refresh_nav(&mut cx, false); // populate the panel's roots

    let outcome = crate::doc_view::ViewOutcome {
        tree_mark: Some(NavigationTarget::Document {
            concept_id: "guide/intro".to_string(),
            surface: None,
            fragment: None,
        }),
        ..Default::default()
    };
    app.apply_view_outcome(&mut cx, outcome);

    let panel = app.ui.widget(&cx, ids!(project_tree));
    let reveal = panel
        .borrow::<crate::tree_panel::ProjectTree>()
        .and_then(|p| p.test_reveal_key().map(str::to_string));
    assert!(reveal.is_some(), "the tree's reveal pulse path is armed");
}
```

- [ ] **Step 2: Run and see them fail**

Run: `cargo test -p waml-editor a_tree_mark_outcome` and `cargo test -p waml-editor crossing_a_section_boundary`
Expected: COMPILE FAILURE on `tree_mark` / `test_reveal_key` / `take_fold_moved_for_test`.

- [ ] **Step 3: Implement**

`book_surface.rs`: in the scroll handler (wheel + scrollbar drag), after clamping the new offset:

```rust
        let current = self.current_section_index();
        if current != self.last_marked {
            self.last_marked = current;
            if let Some(index) = current {
                cx.widget_action(
                    self.widget_uid(),
                    &scope.path,
                    BookSurfaceAction::FoldMoved(index),
                );
            }
        }
```

Plus `fold_moved` on the ref (same accessor shape as Task 6's) and the `#[cfg(test)]` gate accessor.

`doc_view.rs`:

```rust
    /// Mark this target's row in the tree (scroll-into-view + pulse) WITHOUT
    /// opening anything -- the book's scroll position mirrored onto the table
    /// of contents. A side effect, not a navigation: the shell must not
    /// consume the event for it.
    pub tree_mark: Option<crate::navigation::NavigationTarget>,
```

`book_view.rs` `handle`, after the click branches:

```rust
        if let (Some(index), Some(model)) = (book.fold_moved(actions), self.model.as_ref()) {
            if let Some(section) = model.sections.get(index) {
                let mut outcome = ViewOutcome::default();
                outcome.tree_mark = navigation_target_for(&section.target);
                return outcome;
            }
        }
```

`app/actions.rs` `apply_view_outcome`, after the `open_folder_listing` block:

```rust
        if let Some(target) = outcome.tree_mark {
            if let Some(mut panel) = self
                .ui
                .widget(cx, ids!(project_tree))
                .borrow_mut::<crate::tree_panel::ProjectTree>()
            {
                panel.reveal_target(cx, &target);
            }
            // Deliberately no flow change: marking is a mirror, not a claim.
        }
```

`tree_panel.rs`, beside `test_folder_is_open`:

```rust
    #[cfg(test)]
    pub(crate) fn test_reveal_key(&self) -> Option<&str> {
        self.reveal_key.as_deref()
    }
```

- [ ] **Step 4: Run and see them pass**

Run: `cargo test -p waml-editor` — PASS.

- [ ] **Step 5: Full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(editor): book scroll marks the current section in the tree" -m "The widget emits FoldMoved only when the current section changes, the
view translates it to a NavigationTarget, and the shell hands it to the
tree's existing reveal_target path, which already owns unfold,
scroll-into-view, and the pulse. Both directions of tree<->book sync
now key on the projection's stable RowIds; the mark is applied as a
side effect and never consumes the event, so scrolling a book cannot
swallow unrelated actions."
```

---

## Spec deviations, explicit and owed

- **`waml-ui-test` journey scenarios are deferred, deliberately.** The spec's testing section lists typed UI scenarios on the `waml-ui-test` harness. That harness is a feature-gated (`--features ui-tests`), Linux-headless-only GPU journey (`crates/waml-editor/tests/README.md`): it is not part of `cargo test --workspace`, cannot RUN on the Windows development host at all, and adding book domain operations means writing `makepad_test` adapter code that the implementer could never once execute — a violation of this plan's TDD contract (every red must be SEEN red, every green SEEN green). Every behavior those scenarios would pin — book surface shown and siblings hidden on open, tree click scrolls instead of opening, outside click still opens, scroll marks the tree, open-full opens the concept tab — is pinned instead by the headless typed shell tests in `app/tests/navigation.rs` (Tasks 4, 7, 8), which DO run in the workspace gate on every platform. Extending `tests/ui.rs` + `WamlApp` with `ensure_book_open` / `expect_book_surface_active` operations and a Book fixture workspace is a follow-up for a session with a Linux runner in the loop.
- **"Unknown declared surface degrades to `Link`"** is implemented as: the surface DEGRADES through `resolve_surface_for` (claims default + `UnknownSurface` warning in `model.diagnostics`), and the section renders as whatever the degraded surface renders — identical to the tab that row would open, which the spec's "one resolution, never disagree" clause demands more strongly. A surface that resolves but is not inlineable (`source`, `search:*`) is the `Link { UnrenderedSurface }` arm. (See Task 2, Step 1 note.)
- **Anchors:** a book tab's `capture_anchor`/`restore_anchor` stay at the `ViewAnchor::None` default in Phase 1. In-view position is preserved across model rebuilds by the RowId-keyed height cache (Task 5); cross-history-traversal scroll restoration is a Phase 2 concern alongside block editing.

## Visual sign-off (owed to a human, NOT part of the gate)

The implementer cannot verify appearance. After all eight tasks land, a human runs `./run.ps1` on a staged fixture bundle containing a `view: book` folder (the Task 2 fixture shape, scaled up: several prose files, one real diagram, one nested folder, one nested book) and checks:

- [ ] Opening the `view: book` folder from the tree shows one continuous scroll; a plain folder still shows today's listing.
- [ ] Prose typography inside the book matches the standalone reading view of the same file (same fonts, same inset rhythm).
- [ ] Heading hierarchy reads as a hierarchy: depth-stepped sizes and the rule line make nesting obvious without indentation.
- [ ] The diagram embed is legible at its capped height; tune `book_layout::DIAGRAM_EMBED_HEIGHT` (one constant) if not. "Open full" opens the concept's own canvas tab.
- [ ] A Link section (the nested book, any `source`-surface row) reads as a link row — icon, title, hover — and opens its own tab.
- [ ] Scrolling a long book (200+ sections) stays smooth; live-child churn at the window edges causes no visible flicker or layout jumps as estimated heights are replaced by measured ones.
- [ ] Tree click on a section scrolls the book there (no tab opened, no selection stolen); tree click outside the book opens a tab as before.
- [ ] Scrolling the book pulses/scrolls the tree row of the section at the fold; the pulse is not annoying at normal reading speed (if it is, file a follow-up to mark without pulsing — the mechanism is one call site in `apply_view_outcome`).
- [ ] The header toggle drops from the book to the folder listing of the same directory; breadcrumb shows on both.
- [ ] No blank sections anywhere: every row of the folder is a heading, prose, a diagram, or a link with a visible title.
