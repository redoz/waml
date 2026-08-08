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
`"folder"` is separately a registered surface.

Folder tabs do not sit outside `DocumentLocator` — they sit inside it holding a
value it cannot honour. `folder_documents::open` (`folder_documents.rs:50-57`)
stamps `concept_id: "/shop", kind: DocumentKind::Primary`, so a folder tab's
locator claims to be the primary view of a concept named `/shop`. No such
concept exists, so `open_locator_with_asset_host`'s `Primary` arm returns `None`
and the locator never resolves. Tab lookup by locator misses, and view history
silently skips the entry. This is a live bug, not a gap.

## Why this now goes first

The forcing case is view-source on a folder tab, which the source-as-navigation
work wanted and could not express.

Rendering already works on today's shapes, verified by experiment (see the spike
note): `document_by_concept_id` (`source.rs:357`) is pure path derivation — it
parses `"{id}.md"` — so directory `/shop` reaches its index through the key
`"shop/index"`, and `SourceView::resolve_document` resolves it. The root
directory works too, as the key `"index"`.

The return direction is what fails, and it fails structurally. Indexes are
reserved documents held separately from concepts, so `bundle.concept("shop/index")`
misses; `open_source_with_asset_host` gates on exactly that lookup
(`okf_documents.rs:91`). Loosening the gate opens the source tab but strands it:
`Eye` would navigate to `DocumentLocator::primary("shop/index")`, and
`open_with_asset_host` misses on the same lookup, because an index has no
primary document.

The deeper reason is the one this spec exists for: `DocumentLocator` is
`(concept_id, DocumentKind)`, and **neither field can carry a directory**. The
folder tab's existing locator proves the point by failing — it stuffs an address
into the concept-id slot and calls the result `Primary`, which is why it never
resolves. There is no *honest* value expressible in today's locator meaning
"the folder view of `/shop`", let alone "the source of the folder view of
`/shop`". The alternatives are a remembered-origin field on the tab — rejected,
because view history already does that job — or widening the locator, which is
this spec.

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
`{ target: RowTarget::Folder(address), surface: "folder" }`, which resolves —
unlike the `{"/shop", Primary}` it holds today — so tab lookup and view history
start working for folder tabs. `RowTarget::Virtual` gets a locator too, though
nothing can open one until a middleware interprets it.

**`SurfaceFactory` must be re-keyed on `RowTarget`, not `RowId`.** Today it is
`Fn(&OpenCtx, &RowId) -> Option<Box<dyn DocView>>` (`extension_editor.rs:60`).
That signature cannot serve the live open path, for a reason the spike proved
rather than argued: a `RowId` is meaningful only relative to one directory's
`Chain`, no navigation target carries one, and a concept hidden by a `hide`
middleware has **no `RowId` at all** in Projected mode — while
`okf_documents::open_with_asset_host` opens that same concept today. Keying the
open path on `RowId` would therefore make hidden documents unopenable, turning
`hide` into a permission boundary in violation of its stated invariant
(`crates/waml/src/view/hide.rs:7`). Re-keying on `RowTarget` also removes
`OpenCtx::resolve` from the critical path entirely.

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

Four namespaces — `__doc_tab_okf__`, `__doc_tab_source__`, `__doc_tab_uml__`,
`__doc_tab_folder__` — become one function over `(target, surface)`. Two
surfaces of one concept remain distinct tabs; that property is preserved, not
invented — it is what makes source-as-navigation work at all.

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
  `"shop/index"`, root → `"index"`).
- `Virtual` → no source.

The folder gate is **not** `bundle.index(address)`. `Index` values are
synthesized for directories that have no `index.md` on disk, so that lookup
returns `Some` for a directory whose source does not exist and the toggle would
open a broken tab — the spike demonstrated this against a bare directory. Gate
instead on the document actually resolving, which is the same check
`SourceView::resolve_document` performs (`source_view.rs:144-155`). One gate,
and it is the gate that decides whether the surface can render.

The `"source"` surface being absent for a given target is the single answer to
"should the toggle be shown", replacing the source-as-navigation spec's two
independent suppressions (`no_source` plus a concept lookup). A surface that
does not resolve is not offered — one rule, and it covers the virtual-view case
that `no_source` was invented for.

### 6. `OpenCtx` gets constructed for real — and shrinks

Settled by the spike, which built a live `OpenCtx` from nothing but an
`OkfAnalysis` and a directory address and opened real `DocView`s through all
four registered factories. `ProjectionCtx` needs only `dir` / `bundle` /
`params` / `descend`, and `folder_projection.rs:124-129` already builds one per
listing pass. This was written up as the design's largest unknown; it is not.

With §1's `RowTarget` re-keying, `OpenCtx::resolve` leaves the critical path.
It should be dropped rather than carried: as specified it is
`Fn(&RowId) -> Option<Row>`, but `Chain::resolve` returns `Vec<Row>` and falls
back to the whole folder listing, so the closure needs an id filter it does not
have — it would hand back the wrong row. A field that is wrong and unused is
worse than no field.

## Explicitly out of scope

- New surfaces. The four registered ids are the four the editor can already
  open. No speculative format registry.
- Third-party extensions. `EditorExtension` stays an in-tree trait with two
  implementors.

## Risks

**Locator widening is a recompile, not a migration.** Measured by the spike: 36
`DocumentKind` mentions across 13 files, 41 `DocumentLocator` construction sites
(12 in production), and exactly one dispatching match (`documents.rs:136`).
Nothing locator-valued is persisted — no `Serialize`/`Deserialize` on
`view_history.rs`, `doc_tabs.rs`, `document.rs`, or `editor_history.rs`; only
theme/recents and `.waml/settings.json` reach disk. Wide, shallow, compiler-
guided.

**Folder tabs change behavior because they start working.** Preview-slot reuse
already applies to them; what changes is that their locator resolves, so tab
lookup finds them and Back/Forward stops skipping them. This is a bug fix riding
inside a refactor, which makes it easy to mistake for a regression in either
direction — it needs its own before/after verification rather than a
"should be equivalent" claim.

**`hide` must not become a permission boundary.** See §1: the `RowId` keying
would have done exactly that. Any later design that reintroduces row-relative
identity on the open path reintroduces the bug, so the invariant
(`crates/waml/src/view/hide.rs:7`) belongs in the test suite, not just in a
comment.

**The `__legacy_edit__` sentinel has no `RowTarget`.** `editor_session.rs:515`
constructs `DocumentLocator::primary("__legacy_edit__")`, which works only
because the concept-id slot is an unvalidated `String`. A `RowTarget`-typed
locator has nowhere to put it; it needs a real representation or removal before
§1 lands.

**The dead code has never run.** `resolve_surface`'s degrade path and
`SurfaceFactory`'s `Option` return are covered only by tests written against a
seam nothing drives. The spike found two shape errors on first contact
(`RowId` keying, `resolve`'s return type); assume it has not found the last one.

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
  offered no toggle. Same rule, no special case. Note this fails if the gate is
  `bundle.index` (§5) — it is the test that catches that mistake.
- A folder tab's locator resolves, is found by tab lookup, and round-trips
  through Back/Forward — none of which it does today (see Risks).
- A concept hidden by a `hide` middleware still opens through the surface path,
  in both Projected and Raw modes. This is the `hide`-is-not-a-permission-
  boundary invariant, and it is the regression the `RowTarget` re-keying exists
  to prevent.
