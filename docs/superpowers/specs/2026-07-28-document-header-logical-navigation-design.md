# Document header and logical navigation

**Date:** 2026-07-28
**Status:** Approved design

## Context

The active document currently has no document-local header. Its tabs live in
the window caption, and the active view's right-dock toggle is also mounted
there. That toggle is already declared by the active `DocView` through
`BodyChrome.right_dock`, so its state belongs to the document view even though
its widget belongs to window chrome.

Source views and generic OKF fallback views render Markdown through the shared
Makepad `Markdown` surface. Makepad already reports clicked Markdown hrefs as
`MarkdownAction::LinkNavigated(String)`, but waml-editor does not consume that
action. The project tree, future breadcrumb segments, and Markdown links must
not grow separate interpretations of logical navigation.

## Goals

- Add a compact header above the central active-document surface.
- Show the active document's canonical logical breadcrumb when its view type
  opts in.
- Make every breadcrumb segment clickable.
- Move the active view's right-dock toggle from the window caption into the
  document header.
- Collapse the header to zero height when it has neither a breadcrumb nor a
  right-dock toggle.
- Route project-tree activation, breadcrumb activation, and rendered Markdown
  links through one logical navigation path.
- Support logical document and directory targets, document fragments, and
  external HTTP(S) URLs.
- Preserve preview versus persistent tab-opening behavior.

## Non-goals

- Filesystem breadcrumbs or filesystem fallback for unresolved links.
- A generic list of document-header actions.
- Navigation history or back/forward controls.
- A breadcrumb overflow popup.
- Special visual marking for external Markdown links.
- Changes to the start screen.

External URLs will work in this milestone, but visually distinguishing them in
rendered Markdown is deferred.

## Decisions

### A shared central header slot

The central document column owns one shared `DocumentHeader` widget. Individual
`DocView` implementations do not render their own headers. Instead, the active
view declares a small, typed header configuration through the existing chrome
contract:

```rust
pub struct DocumentHeaderChrome {
    pub breadcrumb: bool,
    pub right_dock: Option<Icon>,
}
```

`BodyChrome` carries this value. The start screen and any document view that
wants no header use an empty configuration. The header is visible when either
field contributes content and otherwise occupies zero height.

`right_dock` stays explicit. It is a shell-owned command whose availability and
glyph are declared by the active view; one example is not enough evidence for a
generic header-action framework.

### Canonical logical breadcrumbs

Breadcrumbs always describe the authored logical hierarchy. They never expose
backing paths.

A pure query beside `tree::build_tree` derives breadcrumb segments from the
canonical full project tree:

```rust
pub struct BreadcrumbSegment {
    pub title: String,
    pub target: NavigationTarget,
}

pub fn breadcrumb_for(
    bundle: &waml::okf::Bundle,
    uml: &waml::uml::Projection,
    concept_id: &str,
) -> Option<Vec<BreadcrumbSegment>>;
```

The query must not split a concept ID into display segments and must not inspect
the current `NavView`. Concept IDs are identifiers, while the current navigator
view may be scoped, filtered, or searched and may omit ancestors.

The application derives the active breadcrumb during document-shell
synchronization and pushes it into `DocumentHeader`. A missing path hides only
the breadcrumb; an available right-dock toggle still keeps the header visible.

### One logical navigation path

Navigation is divided into resolution and execution.

The model-facing resolver converts a Markdown href into a resolved target using
the rendered document as its logical base:

```rust
pub enum NavigationTarget {
    Document {
        concept_id: String,
        fragment: Option<String>,
    },
    Directory {
        address: String,
    },
    ExternalUrl(String),
}

pub fn resolve_link(
    bundle: &waml::okf::Bundle,
    current_concept_id: &str,
    href: &str,
) -> Result<NavigationTarget, NavigationError>;
```

The resolver recognizes:

- same-directory and parent-relative OKF links such as `./customer.md`;
- bundle-root logical links;
- `#fragment` within the current document;
- a logical document link followed by a fragment;
- logical directory links;
- `http:` and `https:` external URLs.

It rejects malformed URLs, unsupported schemes, and relative paths that escape
the bundle. It does not consult the filesystem.

Tree rows and breadcrumb segments already carry resolved logical targets.
Markdown contributes a raw href plus the current logical document ID. All
sources converge on one application-owned navigation handler after resolution.
Widgets render and emit intent; they do not mutate tabs, navigator state, or
docks directly.

Document activation uses the existing `DocumentHost` transition choke point.
A normal activation opens or focuses a preview tab. Persistent opening remains
an explicit disposition, currently produced by tree double-click. Activating
the already-active document is harmless.

Directory activation has one explicit meaning in this milestone: toggle the
logical directory's expanded state in the project tree, matching a folder-row
click. It does not change the active document or navigator scope. The tree must
emit the same directory command instead of keeping fold state as a private,
second activation path. Breadcrumb and Markdown directory targets send that
command through the application handler. The bundle root is not a rendered
folder row, so activating `/` expands the tree dock and restores the root
navigator scope. No directory activation opens a filesystem path or fabricates
a document.

External targets are passed to a small platform-browser adapter. Tests replace
that adapter and never launch a real browser.

### Markdown integration

The shared Markdown surface observes Makepad's existing
`MarkdownAction::LinkNavigated(String)` action and returns the raw href to the
active `DocView`/application action flow. Both `SourceView` and
`GenericOkfView` use this same surface, so neither view implements link parsing
or navigation.

Fragments are resolved after document activation. Makepad's Markdown renderer
owns heading layout and exposes one narrow operation:

```rust
fn scroll_to_fragment(&self, cx: &mut Cx, fragment: &str) -> bool;
```

The renderer records heading positions while drawing and derives stable
GitHub-style heading slugs: lowercase text, whitespace collapsed to `-`,
punctuation removed, and repeated slugs suffixed in document order. The method
returns `false` when no matching heading was drawn. waml-editor never
reconstructs renderer geometry. When a target document is not yet active, the
application records the pending fragment, activates and draws the document,
then applies the scroll once the anchor exists.

## Layout and interaction

`DocumentHeader` is the first row of the central document column, below the
window caption/tab strip and between the left and right dock slots.

- Its compact fixed height matches the existing 30 px panel-button geometry.
- Breadcrumbs are left-aligned; the right-dock toggle is right-aligned.
- Ancestor labels use subdued chrome text.
- Separators use the existing chevron glyph.
- The current document label uses emphasized text.
- Every visible segment has an independent hit rectangle.
- Under constrained width, the current document remains visible and older
  ancestors are elided first.
- The current document segment is still an activation target; activating an
  already-active target is a no-op.

The inspector toggle retains its existing icon and active-state styling. Only
its mounting point changes. `DocTabs` no longer needs to know whether the
right-dock button is present or extend its caption rule across the button's
width.

In wide mode, the header naturally spans only the center between dock slots.
In narrow mode, the floating inspector begins below a visible document header
so opening the inspector cannot cover its only pointer-accessible close
control. When the header is absent, the narrow overlay reclaims that vertical
space.

## Data flow

### Active-document synchronization

1. `DocumentHost` completes an activation or transition, synchronizes the
   active `DocView`, and applies its `BodyChrome` through `BodyWidgets`.
2. `BodyWidgets::apply_chrome` updates the shared header declaration and dock
   button.
3. `App::sync_document_shell` obtains the active tab and derives the canonical
   breadcrumb when the declaration requests one.
4. The application pushes the resulting segments into `DocumentHeader`.
5. `DocumentHeader` shows, updates, or collapses based on its resulting content.

Header chrome is static for a view activation in this milestone. If a future
view needs mode-dependent header capabilities, it must add one explicit chrome
invalidation path rather than imperatively mutating the shared header.

### Navigation

1. A tree row, breadcrumb segment, or Markdown link emits navigation intent.
2. Raw Markdown hrefs are resolved relative to the rendered logical document.
3. The application receives one resolved `NavigationTarget`.
4. Documents transition through `DocumentHost`; directories use the shared
   navigator command; external URLs use the platform adapter.
5. A fragment is applied after the target Markdown surface is active and has
   reported its heading anchors.
6. Document-shell projections, including the breadcrumb and selected tree row,
   synchronize from the resulting active document.

## Failure behavior

- An unresolved, malformed, unsupported-scheme, or out-of-bundle link leaves
  the current document unchanged and reports a concise status-bar message.
- A missing breadcrumb path hides the breadcrumb but preserves other header
  content.
- A missing fragment leaves the target document active and reports that the
  section was not found.
- Browser-launch failure leaves the editor unchanged and reports the error.
- Repeated activation and late fragment application are idempotent.

## Testing

### Pure model and resolver tests

- Canonical breadcrumbs use authored titles and hierarchy.
- Scoped, filtered, and searched navigator projections cannot change a
  breadcrumb.
- Same-directory, parent-relative, and bundle-root links resolve correctly.
- Current-document and cross-document fragments resolve correctly.
- External HTTP(S), unsupported schemes, malformed targets, and bundle-escape
  attempts are classified correctly.
- Missing logical documents and directories return typed errors.

### Navigation policy tests

- Tree, breadcrumb, and Markdown activation of the same logical document
  produce the same document command.
- Single activation previews; persistent activation remains explicit.
- Activating an already-active document does not duplicate its tab.
- Directory activation has the same result from all three entry points.
- External navigation reaches only the platform adapter.
- Cross-document fragments apply after activation; missing anchors preserve the
  activated document.

### Header and layout tests

- Empty, breadcrumb-only, button-only, and combined configurations produce the
  correct header height and visibility.
- Switching document types reapplies header configuration without stale state.
- The current segment survives narrow-width elision.
- Segment hit rectangles emit the correct logical targets.
- The right-dock button retains active styling and toggles the same dock state.
- In narrow mode, opening the inspector never covers its close toggle.
- The start screen renders no document header.

### Renderer integration tests

- Makepad Markdown link clicks surface their original href.
- Both source and generic OKF views feed link actions through the shared path.
- Heading anchors are stable and can be scrolled into view.
