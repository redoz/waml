# UML extension + row icons — design

Date: 2026-08-05
Status: proposed
Builds on: `2026-08-05-folder-view-middleware-design.md` (the `CoreExtension` /
`EditorExtension` pair, `Chain`, `Row`, `SurfaceId`).

## Problem

Folder rows all render the same hardcoded `"\u{2022}"` bullet
(`folder_view.rs:68`, drawn by the `bullet` Label in `folder_list.rs:47`).
Two concrete wants:

1. The **UML view** should show the **box** icon on its packages.
2. The **core OKF view** should show the **book** icon on its folders.

`IconBook` and `IconBox` are already catalogued in `icons.rs` (3961, 3985),
both with the comment "catalogued ahead of a call site". This is that call
site.

Neither want can be expressed today: `Row` carries no icon, and there is no
UML middleware — the registry ships exactly `index` and `hide`
(`extension.rs:35`), and `uml-domain` exists only as a `ProfileDef` with
`default_view: None`.

## Shape

An icon is **named headlessly and resolved by the editor half** — the exact
mechanism `surface` already uses. Nothing new is invented:

| concern | headless half names it | editor half resolves it |
| --- | --- | --- |
| surface | `Row.surface: Option<SurfaceId>` | `EditorExtension::surfaces()` → `DocView` factory |
| icon | `Row.icon: Option<IconId>` | `EditorExtension::icons()` → `Icon` |

`waml` cannot name `crate::icons::Icon` and must not learn to. `IconId` is a
plain name; an unknown name **degrades to a fallback and emits a warning
diagnostic**, mirroring `resolve_surface` (`surface.rs:56`) — never a blank
row, never a panic.

The two wants then decompose cleanly onto two extension pairs:

- `CoreExt` / `CoreEditorExtension` — `RootView` stamps `IconId("book")` on
  its folder rows; the core editor half maps `"book"` → `Icon::Book`.
- **new** `UmlExt` / `UmlEditorExtension` — ships the `uml` middleware and
  the `uml-domain` profile; the middleware stamps `IconId("box")` on package
  rows; the editor half maps `"box"` → `Icon::Box`.

Splitting the UML pair out (rather than piling `uml` into `CoreExt` beside
`hide`) is the point: it is the first non-core extension, and it exercises
the two-registry gate assertion
(`script_gate.rs::every_core_extension_has_a_paired_editor_extension_by_name`)
against a set of size two instead of one.

### What is a package

A **child directory whose index frontmatter declares `profile: uml-domain`**
(`okf::Index.profile`, `okf.rs:392`). Not "every folder under a UML view",
and not the `uml.Package` concept type. `RootView::folder_row`
(`root.rs:110`) already reads `ctx.bundle.index(address)` for the label, so
the profile is in hand at exactly the point the row is minted.

### What `uml` is, as middleware

A **decorating** stage, not a listing one. It mints no rows of its own:

```rust
fn project(&self, ctx, next) -> Result<Vec<Row>, ProjectionError> {
    let mut rows = next.project(ctx)?;      // whatever the rest of the chain listed
    for row in &mut rows {
        if is_package(ctx, row) {
            row.icon = Some(IconId::new("box"));
        }
    }
    Ok(rows)
}
```

Consequences of minting nothing, each of which is a required behaviour:

- `resolve` returns `Err(Unresolved)` — it owns no paths. Resolution
  dispatches to the real owner (`RootView`), as `Chain::resolve` already
  does.
- `occludes` stays the default `false` — it drops nothing.
- `apply` and `surface` forward to `next` unconditionally.
- Row **identity is unchanged**: the `RowId.owner` on a decorated row is
  still the minting stage's. A decorator must not rewrite `owner`, or
  persisted `RowId`s break and `apply` dispatches to a stage that cannot
  lower the op.

Reached two ways, both already-built paths in `Bundle::resolved_view`
(`okf.rs:556`): an explicit `view: uml` on the folder's index, **or** — with
no `view:` of its own — `profile: uml-domain` inheriting the profile's
`default_view`.

That second path means `PROFILES` stops being a `const &[ProfileDef]`:
`ViewDecl` holds a `Vec`, so the table is built by `shipped_profiles()` /
`uml_profiles()` at call time instead. `uml-domain` then declares
`default_view: Some(ViewDecl { entries: [ViewEntry { raw: "uml", line: 0 }] })`.
`line: 0` is the "not authored in any file" sentinel — a diagnostic spanning
an inherited default must not point at a line the author never wrote. The
resolution order (`view:` → profile default → root only) is unchanged and
already tested
(`resolved_view_walks_local_then_profile_default_then_root_only`).

### Concept rows carry icons too

The same seam, no exception: every row a shipped stage mints names its icon,
including concept ("file") rows. The editor-side target default becomes a
pure degrade path for unknown/absent names, not the thing that normally
decides what a file row looks like.

**No existing glyph changes.** An `IconId` for a concept row names the
**kind**, and the editor half resolves that kind through the mapping that is
already there — `kind_of(&ElementType)` (`tree.rs:83`) feeding
`IconSet::icon_for(kind)` (`tree_panel.rs:319`). The ten `NavCategory` names
become the ten shipped `IconId`s. Nothing is re-decided; the seam just now
carries what the tree panel was already computing on its own.

| stage | row | `IconId` | glyph (unchanged, from `icon_for`) |
| --- | --- | --- | --- |
| `RootView` | concept | its kind, e.g. `"class"` | `Icon::PanelTop` |
| | | `"interface"` | `Icon::SquareDashedTopSolid` |
| | | `"enum"` | `Icon::List` |
| | | `"data-type"` | `Icon::Braces` |
| | | `"diagram"` | `Icon::Workflow` |
| | | `"behavior"` | `Icon::Activity` |
| | | `"sequence"` | `Icon::ArrowLeftRight` |
| | | `"note"` | `Icon::StickyNote` |
| | | `"okf-document"` | `Icon::FileText` |
| `RootView` | folder | `"book"` | `Icon::Book` ← **the one change** |
| `uml` | `uml-domain` folder (package) | `"box"` | `Icon::Box` ← **the other** |

So the `uml` stage overrides **only** package folders. It does not touch
concept rows at all: a `uml.Class` in a UML folder already resolves to
`Icon::PanelTop` through its kind, and that is the icon it has today.

`kind_of` is pure and depends only on `waml::model`, but lives in the editor
crate — so it **moves** to `waml` as `view::kind::RowKind` (same ten variants,
same match arms, including `uml.Package` → `Directory`), and the editor's
`NavCategory` gains `From<RowKind>`. One classifier, two consumers; a second
copy that drifts is exactly the failure `folder_projection.rs`'s doc comment
already warns about for the registry.

`ElementType` is read the way `default_surface` already reads it
(`surface.rs:29`): `bundle.concept(id)` then `ElementType::parse(&concept.ty)`,
with a concept missing from the bundle leaving the icon `None` — total, never
a panic.

### The folder glyph changes in the tree panel too

`folder_projection.rs` exists so the folder surface and the tree seam list
**the same rows from the same chain** — that is its stated invariant. A row
icon is part of the row, so directory rows in the tree panel become the book
glyph as well, where they are `Icon::Folder` today
(`tree_panel.rs:323`, `tree.rs:150`/`200`). That is a real visible change
beyond the folder view, and it is the correct one under the invariant: the
tree showing a different icon than the folder tab for the same row is the
drift the seam exists to prevent. Flagged rather than assumed — if folders
should stay `Icon::Folder` in the tree, the wants are in conflict and the
invariant has to give.

## Changes

### Headless (`crates/waml`)

1. **`view/row.rs`** — add `IconId(String)` newtype (`new`, `as_str`,
   `Display`), beside `ViewId`/`SurfaceId` in style. Add
   `pub icon: Option<IconId>` to `Row`; `Row::new` defaults it to `None`.
   No constructor-time validation — an icon is presentational, and an
   unknown name is the editor's degrade path, not a construction error.

2. **`view/kind.rs`** (new) — `RowKind`, the ten `NavCategory` variants moved
   headless, plus `kind_of(&ElementType) -> RowKind` lifted verbatim from
   `tree.rs:83` (same arms, `uml.Package` → `Directory` included) and
   `RowKind::as_icon_name()` (`Class` → `"class"`, `DataType` → `"data-type"`,
   `OkfDocument` → `"okf-document"`, …).

3. **`view/root.rs`** — `folder_row` sets `row.icon = Some(IconId::new("book"))`;
   `concept_row` sets the row's kind name via `kind_of(&ElementType::parse(&concept.ty))`,
   or `None` for a concept the bundle cannot resolve. `row_for_member`
   dispatches to those two, so it needs no separate change.

4. **`view/uml.rs`** (new) — `pub struct UmlView` implementing `Projection`
   per the decorator shape above. Overrides **package folders only**:
   `RowTarget::Folder(addr)` whose
   `ctx.bundle.index(addr).profile.as_deref() == Some("uml-domain")` gets
   `IconId::new("box")`. Concept rows are passed through untouched — their
   kind name from the root view already resolves to today's glyph.

5. **`view.rs`** — `pub mod kind;` and `pub mod uml;`.

6. **`extension.rs`** — new `pub struct UmlExt` implementing `CoreExtension`:
   `name() == "uml"`, `middleware()` registers `("uml", || Box::new(UmlView))`,
   `profiles()` returns the `uml-domain` `ProfileDef`. **Move** `uml-domain`
   out of `CoreExt`'s profile list into `UmlExt` — a profile that a shipped
   extension owns is the whole point of `CoreExtension::profiles()`, and
   leaving it in core would mean `uml-domain` resolves with the UML extension
   absent.

7. **`profile.rs`** — `PROFILES` stops being a `const` (a `ViewDecl` default
   needs a `Vec`) and becomes two builder functions: `shipped_profiles()`
   returning core's `okf`, and `uml_profiles()` returning `uml-domain` with
   `default_view: Some(["uml"])`, reachable from `UmlExt::profiles()`.
   `profile(name)` keeps looking across the whole shipped set — it is the
   parse-time name check and must not become extension-order dependent. It
   now allocates per call, so it returns an owned `ProfileDef` (or a
   `OnceLock`-cached table) rather than the current `&'static`; `resolved_view`
   holds the borrow only across `Chain::build`, so either works.

### Editor (`crates/waml-editor`)

8. **`tree.rs`** — `NavCategory` gains `From<RowKind>`, and the local
   `kind_of` becomes a thin forward to `waml::view::kind::kind_of` (or is
   deleted at its call sites). `TreeKind = NavCategory` and
   `IconSet::icon_for` are untouched: the glyph table stays exactly where it
   is and says exactly what it says.

9. **`extension_editor.rs`** — `EditorExtension` gains
   `fn icons(&self) -> Vec<(&'static str, Icon)>` (default `Vec::new()`).
   `CoreEditorExtension::icons()` returns the ten kind names mapped through
   the **existing** `IconSet::icon_for(NavCategory::from(kind))` — written as
   an iteration over `RowKind`'s variants, not a second hand-written glyph
   table — plus `("book", Icon::Book)`. `UmlEditorExtension`
   (`name() == "uml"`, `surfaces()` empty for now) returns
   `[("box", Icon::Box)]`.

10. **New `resolve_icon`** (editor side, beside the extension table): given
   `Option<&IconId>` and the registered icon table, return
   `(Icon, Option<Diagnostic>)`. `None` → the row's default by target
   (`Folder` → `Icon::Folder`, `Concept`/`Virtual` → `Icon::FileText`) — a
   pure degrade path now that every shipped stage names its icon; unknown
   name → that same default plus a
   `DiagCode::UnknownIcon` warning. Add `UnknownIcon` to
   `diagnostic.rs`'s enum, `as_str()` (`"unknown-icon"`), and the
   `Severity::Warning` arm.

11. **`folder_projection.rs`** — a `core_registry()` sibling for the editor
    extension list, so the two construction sites cannot disagree (same
    argument the existing doc comment makes for the middleware registry).
    `core_registry()` itself grows `&UmlExt`.

12. **`folder_view.rs`** — `FolderRowView.bullet: &'static str` becomes
    `icon: Icon`, resolved in `row_view` through (10). `row_view` gains the
    context it needs to resolve (the icon table).

13. **`tree_panel.rs`** — a tree row's icon comes from the projected row's
    `IconId` when it has one, falling back to `IconSet::icon_for(kind)` for
    rows minted outside the chain. Same resolution as the folder surface,
    per `folder_projection.rs`'s invariant.

14. **`folder_list.rs`** — replace the `bullet` Label with a 16×16 anchor
    `View` and draw the icon immediate-mode over it in `draw_walk` via
    `IconSet::draw`, the established pattern in `recent_row.rs:278`
    (`self.icons.draw(cx, icon, self.glyph_rect, tint)`). `FolderRow` gains
    `icons: IconSet`, the per-row `Icon`, and the anchor rect captured in
    `draw_walk`. Tint: `atlas.text_dim`, matching the bullet it replaces.

15. **`script_gate.rs`** — extend the existing pairing assertion to the set
    `{core, uml}`, and add its icon analogue: **every `IconId` a registered
    middleware can mint has a registered `Icon`**, the same shape as
    `every_surface_id_resolvable_by_a_registered_chain_has_a_registered_factory`.
    A stage that stamps a name nothing resolves must fail the gate, not
    degrade silently in front of the user.

## Tests

Headless, in-crate:

- `kind.rs` — `kind_of` agrees with the arms it was lifted from, variant for
  variant, `uml.Package` → `Directory` included. This is the moved-code
  regression check.
- `root.rs` — a projected folder row carries `IconId("book")`; a `uml.Class`
  concept row carries `IconId("class")`, a `note` carries `IconId("note")`,
  and a concept absent from the bundle carries `None`.
- `uml.rs` — a chain `["uml"]` over a directory with one `uml-domain` child
  and one plain child stamps `"box"` on exactly the first, and leaves
  `"book"` (from `RootView` below it) on the second.
- `uml.rs` — **concept rows are untouched by the decorator**: a `uml.Class`
  row projected through `["uml"]` carries the same `IconId("class")` it
  carries through `["index"]`. The glyph a file shows today does not move.
- `okf.rs` — a folder with `profile: uml-domain` and **no** `view:` resolves
  to the `["uml"]` chain through the profile default, and its `uml-domain`
  child comes back with `"box"`. This is the `default_view` wiring end to
  end, not just the table entry.
- `uml.rs` — the decorator preserves `RowId.owner` (still the root view's),
  and `resolve` of a decorated path still succeeds through the chain.
- `uml.rs` — `occludes` is false for every projected path; `apply` of a
  `Rename` through `["uml"]` lowers to the same `okf::Op`s as through
  `["index"]`.
- `extension.rs` — `MiddlewareRegistry::from_extensions(&[&CoreExt, &UmlExt])`
  builds without a duplicate-name error and resolves both `index` and `uml`.
- `profile.rs` — `profile("uml-domain")` still resolves after the move off
  the `const` table, and carries `default_view: Some(["uml"])`; `profile("okf")`
  still resolves with `None`; `profile("UML-Domain")` still does not (no case
  folding).

Editor:

- `resolve_icon` — known name resolves silently; unknown name degrades to the
  target default **and** returns an `UnknownIcon` warning; `None` resolves to
  the target default with no diagnostic.
- `folder_view.rs` — `row_views` over a mixed listing yields
  `[Icon::Box, Icon::Book, Icon::PanelTop, Icon::StickyNote]` in projected
  order — the last two being today's class and note glyphs, unchanged.
- **`extension_editor.rs` — every `RowKind` resolves to the same `Icon` it
  resolves to through `IconSet::icon_for` today.** The whole "no existing
  glyph changes" claim, asserted rather than promised.
- `tree_panel.rs` — a tree row and the folder row for the same directory
  resolve to the same `Icon`.
- `script_gate.rs` — the two gate assertions above.

Visual (owed, cannot be automated — see memory
`implement-plan-cannot-do-visual-verification`):

- **V1** core folder rows draw the book glyph, vertically centred against the
  label baseline, at the bullet's old position.
- **V2** a `view: uml` folder draws the box glyph on `uml-domain` children and
  the book glyph on plain ones, in one listing.
- **V3** a `profile: uml-domain` folder with **no** `view:` draws the same as
  V2 — the profile default reaching the surface.
- **V4** concept rows draw exactly the glyphs they draw today (class, note,
  diagram, sequence side by side), in both the folder view and the tree.
- **V5** the tree panel's directory rows draw the book glyph, matching the
  folder tab for the same directory.
- **V6** the icon tints with the theme in both light and dark.

## Deferred

- `UmlEditorExtension::surfaces()` is empty. When the UML canvas moves behind
  the extension seam, `canvas` belongs there rather than in core.
- The `#[allow(dead_code)]` on `EditorExtension`/`OpenCtx` stays: this change
  adds a second implementor, not the deferred `documents.rs` rewiring that
  finally consumes the table.

## Risks

- **Row-struct churn.** `Row` gains a field; every literal construction
  breaks. All construction already funnels through `Row::new` plus field
  assignment, so the blast radius is the test doubles in `chain.rs` and
  `tree.rs` — compile errors, not silent drift.
- **`folder_list.rs` is a `script_mod!` widget.** Removing the `bullet` Label
  and adding an anchor changes the DSL namespace; per
  `script-mod-namespace-object-literal` and `iconbutton-child-needs-script-mod-order`,
  the `IconSet` material must already be registered when `FolderRow` draws, or
  the glyph is silently absent with a green gate. V1 is the check that catches
  it.
- **`kind_of` moving crates.** It is lifted verbatim, but a dropped or
  reordered match arm silently reclassifies documents — and a reclassified
  document changes its icon *and* its accent bucket (`accent.rs:73`). The
  variant-for-variant `kind.rs` test is what stops that; `accent.rs`'s
  existing `tree_kinds_agree_with_the_element_type_buckets` is a second net.

- **Two profile tables.** Splitting `PROFILES` risks `profile()` silently
  missing `uml-domain` if the lookup is left extension-scoped. The
  `profile.rs` test above pins it.

- **`uml-domain` folders change behaviour, not just appearance.** Giving the
  profile a `default_view` means every existing `profile: uml-domain` folder
  with no `view:` starts running a chain where it previously ran
  `Chain::root_only`. `UmlView` is a pure decorator — same rows, same order,
  same `RowId`s, same `apply` lowering — so the change is confined to icons
  by construction. The `apply`-parity and `occludes` tests above are what
  hold that line; if `uml` ever gains listing behaviour, this stops being
  free and the profile default needs revisiting.
