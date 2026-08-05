# Tree-panel projected/raw toggle

**Status:** design, awaiting implementation plan
**Builds on:** `2026-08-05-folder-view-middleware-design.md` (the chain, the raw
OKF layer, `RowId` ownership). That spec is the vocabulary; this one does not
restate it.

## Why

Two gaps, one shape.

The folder-view middleware change shipped the raw OKF layer as a per-folder
"View raw" link. It does not work: `folder_documents::open_raw` deliberately
reuses `folder_document_tab_id(directory)`, and `DocumentHost::apply_command`
(`document_host.rs:71`) inserts the incoming view only when the tab is not
already open. Opening raw from a folder's own tab therefore builds the raw
`FolderView` and discards it. Verified in a window: the affordance draws, the
click lands, nothing changes.

Separately, the tree never ran the chain at all. `build_tree` lists real OKF
members and borrows `core_registry()` only to compute `view_degraded` for the
marker. So a `hide: ["**"]` folder shows no rows in its folder view and still
lists every hidden child in the tree — the folder-view spec's own checklist item
("an opaque folder shows no descendants in the tree") does not hold.

One session-wide mode fixes both: the tree becomes a projection, and raw becomes
a mode rather than a per-tab navigation.

## The mode

A single session-wide switch with two states:

- **Projected** (default) — tree rows and folder views run the declared chain.
- **Raw** — both run `Chain::raw()`.

Never persisted. Every launch starts Projected, so an author's declared `view:`
is what a reader sees unless they deliberately ask otherwise. The state lives in
memory on the app; `.waml/settings.json` is untouched.

Both surfaces read the same flag, so there is no state in which the tree and a
folder view disagree about what a directory contains.

**The per-folder "View raw" affordance and its raw-mode banner are deleted**,
along with `folder_documents::open_raw` and the `NavigationTarget::DirectoryRaw`
route. They were the only consumers of the shared-tab-id path that made the
affordance inert. Removing them removes the defect rather than working around it.

Hiding remains presentational and never a permission boundary. A session-wide
switch that any reader can flip makes that more obviously true, not less. No code
or comment may imply a hidden file is protected.

## The tree becomes a projection

`build_tree` stops listing OKF members directly. A directory's children are the
chain's rows for that directory, in the chain's order, carrying the chain's
labels.

- `TreeNode.key` becomes the row's `RowId` rather than a file address. `RowId` is
  stable across re-projection, so selection and expansion survive a chain re-run,
  including the re-run a mode flip triggers.
- A virtual row has no file behind it. Click routing resolves through the row's
  surface (`SurfaceFactory` already returns `Option<Box<dyn DocView>>`, so an
  unresolvable row degrades rather than panicking).
- Tree edits — rename, delete, drag — route through `Chain::apply` to the owning
  stage instead of straight to OKF ops.
- `caps` / `child_caps` gate the tree's context menu, replacing today's
  `can_edit_classifier` / `can_delete_classifier`.
- `view_degraded` is unchanged. It already comes from `resolved_view`, and the
  tree must keep using the same registry as `FolderView::build` or it marks a
  folder degraded that opens fine.

Resolution follows the ownership rule the chain spec already states: `RowId`
carries `owner: ViewId`, resolution dispatches to that owner, and ownership is
total — any row no middleware claims is emitted by the root view with a real
member href. In Raw the chain is `[index]`, every row is owned by
`ROOT_VIEW_OWNER`, and `RootView::resolve` answers every path. Raw is therefore
today's listing, which is the behaviour that already ships.

One consequence, deliberately accepted: a `RowId` minted by a middleware while
Projected does not resolve in Raw, because its owner is not in the raw chain.
Selection or expansion sitting on a virtual row falls back to the nearest
resolvable prefix — at worst the folder. That is the existing `Unresolved` rule,
not a new one.

## The toggle

One `IconButton` in the tree panel. The glyph shows the **current state**, not
the action it would perform:

| State | Glyph |
|-------|-------|
| Projected | `SquareLibrary` — the curated shelf the author's `view:` describes |
| Raw | `SquareCode` — the files as they sit on disk, chain bypassed |

Two known traps. The tree panel owns no `IconButton` children today
(`tree_panel.rs:915` — collapse and expand both arrive from the caption bar), so
this is the first one back. And a child widget is silently dead and invisible
unless its `script_mod!` registers **before** its consumer; there is no glob
import in `app.rs`.

### Icons to add

Four glyphs, generated per-glyph with `scripts/gen-icon.py` (`gen-all-icons.py`
is stale — never run it), then wired into the catalog in the fixed order: enum,
field, DSL, `get`, `ALL`, label, with counts bumped.

| Glyph | Purpose |
|-------|---------|
| `square-code` | Raw state of the toggle |
| `square-library` | Projected state of the toggle |
| `book` | Catalogued ahead of a call site |
| `box` | Catalogued ahead of a call site |

`package` and its family already ship; nothing to do there.

Every catalog glyph also needs a row in `ICON_GROUPS` (`icons_overlay.rs`) via
`ie!(Variant, "purpose line")`. `book` and `box` have no call site, so they
additionally join `UNWIRED_BUT_LISTED` (`icons_overlay.rs:291`) with a comment
saying they are catalogued ahead of a call site — matching the existing
`FileCodeCorner` / `Plus` precedent. The catalog is add-only; pruning is
deliberate. Three guards enforce this: no duplicate rows, no unlisted call site,
every unwired entry still present in the reference.

## Flipping with tabs open

A flip re-runs every open folder tab in place — same tab, view swapped. Concept
tabs are untouched.

That needs `DocumentHost` to gain an explicit view-replace path, since
`apply_command` keeps the existing view whenever the tab id is already open. This
is the same missing capability that made "View raw" inert, so the fix lands the
capability rather than a workaround.

## Testing

Headless, in `tree.rs`:

- A `hide: ["**"]` folder yields zero tree children in Projected and its real
  children in Raw.
- Tree children equal the folder view's rows for the same directory — row for
  row, in order — in both modes.
- Expansion and selection keyed on `RowId` survive a mode flip for rows the raw
  chain still owns.
- A `RowId` whose owner is absent from the raw chain falls back to its nearest
  resolvable prefix rather than vanishing or panicking.
- The tree and `FolderView::build` use the same registry, so `view_degraded`
  never disagrees with whether the folder opens clean.

Widget-level, in `document_host.rs`: re-opening an already-open tab with a new
view replaces the view rather than discarding it.

Verified visually, and stated as such (the gate cannot assert drawing):

- The toggle's two glyphs and their lit states.
- A hidden file disappearing from the tree in Projected and reappearing in Raw.
- An open folder tab changing content in place on a flip rather than opening a
  second tab.

## Rejected

- **Persisting the mode in `.waml/settings.json`.** Raw is a deliberate act, not
  a preference. Starting every session Projected keeps the author's declared view
  the default thing a reader sees.
- **Keeping "View raw" alongside the switch.** Two independent raw controls give
  four combinations and a tab that can disagree with the tree, for no gain.
- **Filtering the tree by occlusion only** — keeping OKF members as tree rows and
  dropping the occluded ones. Cheaper, and indistinguishable today because
  `index` and `hide` are the only middleware. It diverges the moment a middleware
  reorders, relabels, or mints rows, and then the tree and the folder view
  disagree with no way for a reader to tell which is right.
