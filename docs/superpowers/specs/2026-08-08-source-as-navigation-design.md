# Source as navigation, not an in-tab toggle

**Status:** blocked on `2026-08-08-surface-routed-navigation-design.md`, which
now lands first. The problem statement, the deletion of `SourceToggleView`, and
the shell-owned-toggle direction all stand. What changes once the surface work
lands: the toggle dispatches on the active tab's **surface**, not on
`DocumentKind` (which stops existing), and §3's two suppressions collapse into
one — a target that does not resolve the `"source"` surface is offered no
toggle. Sections below are written in the pre-surface vocabulary; read them for
intent, not for field names.

The order flipped because view-source on a folder tab turned out to be
inexpressible in `(concept_id, DocumentKind)`. That reasoning lives in the
surface spec's "Why this now goes first".

## Problem

"View source" means two different things depending on how you reach it.

From the document header's `FileCode` button, it flips the *current tab* in
place: `SourceToggleView` (`source_toggle_view.rs`) holds a `showing_source`
flag, swaps the body surface underneath the wrapped view, suppresses the tool
dock / view bar / canvas overlays, forks `route_ui_event`, and exits on Escape.
The tab, its identity, and its history entry are unchanged — you are looking at
a different thing in the same slot.

From the node context menu's "View Source", it opens a real
`DocumentKind::Source` tab through `App::open_view_source`
(`app/navigation.rs:432`), with tab reuse, preview-slot semantics, and a view
history entry.

Two mechanisms, two mental models, one name. And the in-place one is a dead end:
`DocumentKind::Source` tabs deliberately have no toggle, so once you are in a
real source tab there is no affordance back to what the source belongs to. The
eye only exists on the surface you came from, and only if you got there by
flipping.

## Goal

One mechanism. View-source is a navigation. The source of a document is a
document, reachable and returnable-from by the same rules as anything else.

## Design

### 1. Delete `SourceToggleView`

`source_toggle_view.rs` goes. `uml_documents::open_with_asset_host` returns the
bare inner views (`ClassDiagramView`, `BehaviorDocView`, `ClassifierPreviewView`)
instead of wrapping them, and no longer needs the asset host it takes only to
construct the wrapper's `SourceView`.

Removed with it: `showing_source`, `assert_source_surface` /
`restore_inner_surface`, the chrome-suppression branch in `chrome()`, the
source-mode fork in `route_ui_event` / `handle` / `sync` /
`sync_from_session` / `after_session_snapshot` / `on_activate`, and the
Escape-exits-source arm in `on_escape`.

The wrapper's careful surface-ordering comments describe hazards that only exist
because two views shared one body. They stop being true and stop being needed.
The behaviors they protected — the behavior canvas being a sibling of the
canvas/markdown swap, `install_snapshot` unconditionally showing the raw
surface — remain relevant to `SourceView` in its own tab, where they are already
handled.

### 2. The shell owns the toggle

The document header's toggle button is handled once, in `app/actions.rs`, for
every document tab. It dispatches on the active tab's kind:

- `DocumentKind::Source` → `Eye` → `transition_to_location(DocumentLocator::primary(concept_id), UserNavigation)`
- otherwise → `FileCode` → `open_view_source(concept_id)`

No view participates. A view can no longer show the wrong icon, forget to wire
the button, or hold its own idea of which surface is showing, because none of
those are per-view facts any more.

The `concept_id` comes from the active tab's `locator()` — the value already on
the tab. Nothing is resolved at click time.

### 3. `DocumentHeaderChrome` gains an opt-out, not an opt-in

`view_toggle: Option<Icon>` is deleted; the shell derives the icon. In its place:

```rust
pub struct DocumentHeaderChrome {
    pub breadcrumb: bool,
    pub right_dock: Option<Icon>,
    /// A purely virtual surface with no backing markdown sets this. Default
    /// `false` = toggle shown, because every real document has a source.
    pub no_source: bool,
}
```

The polarity is deliberate. `DocumentHeaderChrome` derives `Default`, and
`bool::default()` is `false` — a positively-named `source_toggle: bool` would
default to *hidden*, silently, for every view that constructs its chrome from
`Default`. Naming the exceptional case makes the derive correct by construction.

There is a second, automatic suppression: the shell also hides the button when
`analysis.bundle.concept(&concept_id)` misses, since that is exactly the
condition under which `open_source_with_asset_host` returns `None` and the
navigation would silently do nothing (`okf_documents.rs:86`). `no_source`
therefore exists for the view that *has* a real concept but still wants no
toggle; the lookup covers the view that has no concept at all. Neither
subsumes the other.

### 4. Source tabs gain the toggle

`DocumentKind::Source` was excluded from the toggle on purpose, and that
exclusion was only coherent while the toggle meant "flip in place" — a source
tab has nothing to flip back to. As a navigation it is well-defined, so the
exclusion goes and source tabs get the `Eye`.

### 5. Navigation semantics — unchanged, inherited

Both directions run through `transition_to_location`, so both get, without new
code:

- **Existing tab wins.** `tab_id_for_locator` matches on the `(concept_id, kind)`
  pair; a hit activates that tab (`document_host.rs:219`).
- **Otherwise the preview slot.** A miss opens with `persistent: false`,
  replacing a current preview tab in place. Covered by
  `open_source_uses_the_preview_slot_and_is_a_source_tab`,
  `open_source_twice_reuses_the_same_slot_and_focuses`, and
  `open_source_replaces_an_existing_preview_in_place` (`doc_tabs.rs:1215-1267`).
- **Back/forward returns you where you came from.** Each direction records a
  view history entry, so "go back to what I was looking at" is the existing
  history mechanism rather than a remembered origin tab.

Tab ids are namespaced by kind (`__doc_tab_okf__` / `__doc_tab_source__`,
`okf_documents.rs:14-20`), so a concept's primary and source tabs coexist.

## Decisions taken

**The source belongs to the tab's concept, not to what is rendered.** A class
diagram for `orders` opens `orders`' markdown, even though the diagram draws
classes declared in sibling concepts. `FileCode` is "the source of this
document", not "the source of what I am looking at". This is the only definition
that round-trips: `Eye` from that source tab returns to the same diagram.

**Source opens as a preview tab**, not pinned — the same disposition the context
menu uses today. It promotes to a persistent tab by the existing rules
(double-click, edit, explicit pin).

**Escape no longer exits source mode**, because there is no mode. Escape in a
source tab does whatever `SourceView::on_escape` does; the tab stays open.

## Out of scope

Routing this through the surface registry rather than `DocumentKind`. That is
`2026-08-08-surface-routed-navigation-design.md`, parked deliberately: the
toggle is a two-destination affordance and does not need a general surface
router to be correct, and doing both at once makes the work mostly a
`DocumentLocator` refactor with tab identity and view history as collateral.

## Testing

- The header toggle on each of the three previously-wrapped view kinds opens a
  source tab rather than flipping in place.
- The toggle on a source tab returns to that concept's primary tab.
- Both directions reuse an already-open tab of the target kind.
- Both directions land in the preview slot when no such tab is open, replacing
  an existing preview.
- Back after a toggle returns to the departing document.
- The button is absent when `no_source` is set, and absent when the concept
  does not resolve.
- Visual verification of the toggle on all three view kinds plus a source tab —
  the previous toggle work was verified by synthetic clicks at the header's
  top-right, and the same method applies. This cannot be verified by the
  automated gate and must be a deferred, explicitly-flagged step.
