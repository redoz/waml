# Surface-routed navigation

**Status:** scheduled FIRST, ahead of `2026-08-08-source-as-navigation-design.md`.

Originally parked behind that spec, on the reasoning that a two-destination
toggle does not need a general surface router. That held until "a folder tab
should have view-source too" — see "Why this now goes first" below, which is a
requirement this design has to satisfy and the `DocumentKind` design provably
cannot.

## Problem

The editor has two parallel vocabularies for "which way am I looking at this
concept", and only one of them is alive.

The live one is `DocumentKind` (`view_history.rs:19`): a two-variant enum,
`Primary` and `Source`. It is the second half of `DocumentLocator`, which is the
key for three separate things — tab identity (`tab_id_for_locator`), view
history entries (`ViewLocation::document`), and tab-id namespacing
(`__doc_tab_okf__{concept_id}` vs `__doc_tab_source__{concept_id}`,
`okf_documents.rs:14-20`). Dispatch on it is a hand-written match
(`documents.rs:130-142`), and its `Primary` arm is itself a hand-written
provider chain: `uml_documents::open_with_asset_host(...).or_else(|| okf_documents::open_with_asset_host(...))`
(`documents.rs:14-22`).

The dead one is the surface registry. `EditorExtension::surfaces()` returns
`Vec<(&'static str, SurfaceFactory)>`; `CoreEditorExtension` registers exactly
four — `"markdown"`, `"source"`, `"canvas"`, `"folder"`
(`extension_editor.rs:89-96`). `waml::view::surface::resolve_surface` resolves a
row's `surface:` override against that set, degrading an unknown id to the
row's type default with a diagnostic rather than a blank tab. The whole module
is `#[allow(dead_code)]`.

`open_row_with_asset_host` (`documents.rs:59`) is the bridge, and it is dead for
one specific reason, stated in its own doc comment (`documents.rs:49-57`):
`NavigationTarget::Document` has no surface field, so a resolved `SurfaceId`
cannot be carried from a click to an open. Every `Row` the editor can currently
produce has `surface: None`, so nothing exercises it.

The cost is not hypothetical. `"source"` exists twice — as `DocumentKind::Source`
and as surface id `"source"` — with two independent open paths
(`open_locator_with_asset_host`'s `Source` arm and the factory registered under
`"source"`). Both call `okf_documents::open_source_with_asset_host` today, so
they agree by coincidence, not by construction. Any behavior added to one is
silently absent from the other. Making source a first-class navigation
destination (the `source-as-navigation` spec) puts weight on that coincidence.

A third vocabulary sits alongside both: folder tabs. `open_folder`
(`documents.rs:103`) is keyed on a directory address, not a concept id, and
therefore lives outside `DocumentLocator` entirely — a folder tab has no
locator, so it is invisible to locator-keyed tab lookup and to view history.
`"folder"` is nonetheless a registered surface.

## Why this now goes first

The forcing case is view-source on a folder tab, which the source-as-navigation
work wanted and could not express.

Half of it already works on today's shapes. `document_by_concept_id`
(`source.rs:357`) is pure path derivation — it parses `"{id}.md"` — so directory
`/shop` reaches its index through the key `"shop/index"`, and `Bundle::index`
(`okf.rs:494`) answers whether that index exists. `SourceView` would render it
unchanged.

The return direction is what fails, and it fails structurally. Indexes are
reserved documents held separately from concepts, so `bundle.concept("shop/index")`
misses; `open_source_with_asset_host` gates on exactly that lookup
(`okf_documents.rs:91`). Loosening the gate opens the source tab but strands it:
`Eye` would navigate to `DocumentLocator::primary("shop/index")`, and
`open_with_asset_host` misses on the same lookup, because an index has no
primary document.

The deeper reason is the one this spec exists for — **a folder tab has no
locator at all**. `DocumentLocator` is `(concept_id, DocumentKind)`; folders are
keyed on a directory address and sit outside the system (`documents.rs:103`).
There is no value expressible in today's locator meaning "back to the folder
view of `/shop`". The alternatives are a remembered-origin field on the tab —
rejected, because view history already does that job — or widening the locator,
which is this spec.

Under §1 below the case is ordinary: the folder tab is
`{ target: Folder("/shop"), surface: "folder" }`, its source is
`{ target: Folder("/shop"), surface: "source" }`, and both directions are
locator-expressible, tab-reusable, and history-recorded like any other pair.

## Goal

One vocabulary. A navigation names a target and a surface; the surface table
resolves it; `DocumentKind` stops existing.

Success is measured by the folder case above working end to end, not by the
refactor landing.

## Design

### 1. `SurfaceId` replaces `DocumentKind` in the locator

`DocumentLocator` becomes `{ target: RowTarget, surface: SurfaceId }`, where
`SurfaceId` is the registered string id (newtyped, not a bare `String`, so an
unresolved id cannot be constructed by accident).

This subsumes the folder case: a folder tab is
`{ target: RowTarget::Folder(address), surface: "folder" }` and gains a locator,
tab-lookup, and view history for free — three things it does not have today.
`RowTarget::Virtual` gets a locator too, though nothing can open one until a
middleware interprets it.

`DocumentLocator::primary(id)` / `::source(id)` become
`::concept(id, surface)`. "Primary" is not a surface — it is a *resolution*
("whatever surface this concept's type defaults to"), and conflating the two is
what makes the current `Primary` arm a provider chain instead of a lookup. The
default resolution is `resolve_surface(None, &target, ...)`.

### 2. `NavigationTarget::Document` carries the surface

```rust
NavigationTarget::Document {
    concept_id: String,
    surface: Option<SurfaceId>,   // None = the target's default resolution
    fragment: Option<String>,
}
```

This is the field whose absence `documents.rs:49-57` names as the blocker. With
it, `open_row_with_asset_host` becomes reachable from a live click and replaces
`open_locator_with_asset_host` as *the* open path.

### 3. Tab ids derive from the locator

`__doc_tab_okf__` / `__doc_tab_source__` become one function over
`(target, surface)`. Two surfaces of one concept remain distinct tabs; that
property is preserved, not invented — it is what makes source-as-navigation work
at all.

### 4. The `uml`-then-`okf` provider chain becomes a resolution

`uml_documents::describe` winning over `okf_documents::describe` is today an
`.or_else` ordering. It becomes: the concept's type resolves to the `"canvas"`
surface when the UML analysis claims it, `"markdown"` otherwise. `UmlEditorExtension`
registers no surfaces of its own (`extension_editor.rs:121-123`) — it decorates
rows the core surfaces open — so this is a resolution change, not a new factory.

### 5. The `"source"` surface resolves per target, not per concept

`open_source_with_asset_host` currently hard-gates on `bundle.concept(id)`
(`okf_documents.rs:91`), which is what excludes folders. As a surface factory it
resolves its key from the `RowTarget` instead:

- `Concept(id)` → key `id`, gated on `bundle.concept(id)`.
- `Folder(address)` → key is the address's index document (`/shop` →
  `"shop/index"`, root → `"index"`), gated on `bundle.index(address)`.
- `Virtual` → no source.

The `"source"` surface being absent for a given target is the single answer to
"should the toggle be shown", replacing the source-as-navigation spec's two
independent suppressions (`no_source` plus a concept lookup). A surface that
does not resolve is not offered — one rule, and it covers the virtual-view case
that `no_source` was invented for.

### 6. `OpenCtx` gets constructed for real

`OpenCtx` (`extension_editor.rs:40`) is `#[allow(dead_code)]` and built only in
tests. Live construction needs a per-directory `ProjectionCtx` to back its
`resolve: &dyn Fn(&RowId) -> Option<Row>`, which the document-provider layer
already owns. This is the largest unknown in the design and the first thing to
prototype.

## Explicitly out of scope

- New surfaces. The four registered ids are the four the editor can already
  open. No speculative format registry.
- Third-party extensions. `EditorExtension` stays an in-tree trait with two
  implementors.

## Risks

**Locator widening is a three-way blast radius.** Tab identity, view history,
and tab-id namespacing all key on `DocumentLocator`. A session's persisted
history entries are locator-valued, so a shape change is a migration or a
deliberate history reset.

**Folders joining the locator system changes tab behavior.** A folder tab
becoming locator-keyed makes it eligible for preview-slot reuse and for
back/forward, which it is not today. That is the desired end state, but it is a
user-visible behavior change riding along with a refactor — it needs its own
verification, not a "should be equivalent" claim.

**The dead code has never run.** `resolve_surface`'s degrade path,
`SurfaceFactory`'s `Option` return, and `OpenCtx`'s `resolve` closure are all
covered only by tests written against a seam nothing drives. Expect the first
live wiring to find that the seam's shape is subtly wrong.

## Testing

- Every `(target, surface)` pair that today produces a tab still produces the
  same tab, asserted against the existing `documents.rs` / `doc_tabs.rs` tests
  rather than new ones.
- An unknown `surface:` override degrades to the type default and emits a
  diagnostic — the existing `resolve_surface` totality tests, now on a live path.
- A folder tab round-trips through back/forward (new capability).
- The `"source"`-surface factory and the source-navigation toggle open the same
  tab — the duplication this spec exists to remove.
- **The forcing case:** a folder tab whose directory has an `index.md` resolves
  the `"source"` surface, opens it, and returns to the folder tab — the
  round-trip that is inexpressible today.
- A folder whose directory has no `index.md` does not resolve `"source"`, and is
  offered no toggle. Same rule, no special case.
