# UML extension + row icons — implementation plan

Spec: `docs/superpowers/specs/2026-08-05-uml-extension-row-icons-design.md`

Outcome: folder rows carry a stage-stamped icon. Core OKF folders draw the
book glyph, UML packages the box glyph, and **every other glyph in the editor
stays exactly what it is today**. The `uml` middleware ships as the first
non-core `CoreExtension`/`EditorExtension` pair.

Ordering constraint: tasks 1–6 are headless (`crates/waml`) and land before
anything in `crates/waml-editor` can compile against them. Task 7 is the
crate-crossing move and is the one to review hardest.

Every task is a committable green unit: `cargo test --workspace` plus the
`editors/vscode` test/lint/build gate.

---

### Task 1 — `IconId` on `Row`

`crates/waml/src/view/row.rs`.

Add `IconId(String)` — `new`, `as_str`, `Display` — styled after the adjacent
`ViewId`. Add `pub icon: Option<IconId>` to `Row`, defaulted to `None` in
`Row::new`. No construction-time validation: an icon is presentational, and
an unknown name is the editor's degrade path, not a construction error.

Fixes the fallout in test doubles (`chain.rs`, `tree.rs`) — compile errors,
not silent drift, since all construction funnels through `Row::new`.

Test: a row constructed through `Row::new` has `icon: None`; `IconId`
round-trips through `as_str`/`Display`.

### Task 2 — `RowKind` and `kind_of`, headless

New `crates/waml/src/view/kind.rs`; `pub mod kind;` in `view.rs`.

`RowKind` = the ten `NavCategory` variants (`document.rs:20`). `kind_of(&ElementType)
-> RowKind` lifted **verbatim** from `tree.rs:83` — same arms in the same
order, `ElementType::Uml(UmlMetaclass::Package) => Directory` included.
`RowKind::as_icon_name()` gives the shipped `IconId` names: `Class` →
`"class"`, `DataType` → `"data-type"`, `OkfDocument` → `"okf-document"`, and
so on.

Nothing consumes it yet. `tree.rs` still has its own copy until Task 7 —
deliberate: the move and the duplicate-deletion are separated so the
verbatim-ness is reviewable on its own.

Test: `kind_of` agrees with `tree.rs`'s arms variant for variant (the
moved-code regression check named in the spec's Risks); `as_icon_name` is
distinct and kebab-case across all ten.

### Task 3 — `RootView` stamps its rows

`crates/waml/src/view/root.rs`.

`folder_row` sets `row.icon = Some(IconId::new("book"))`. `concept_row` sets
the kind name via `kind_of(&ElementType::parse(&concept.ty))`, or leaves
`None` when the concept is not in the bundle. `row_for_member` dispatches to
both, so it needs no change.

Test: a folder row carries `IconId("book")`; a `uml.Class` concept carries
`IconId("class")`; a `note` carries `IconId("note")`; a concept absent from
the bundle carries `None`.

### Task 4 — the `uml` decorator middleware

New `crates/waml/src/view/uml.rs`; `pub mod uml;` in `view.rs`.

`UmlView` implements `Projection` as a pure decorator:

```rust
fn project(&self, ctx, next) -> Result<Vec<Row>, ProjectionError> {
    let mut rows = next.project(ctx)?;
    for row in &mut rows {
        if is_package(ctx, row) {
            row.icon = Some(IconId::new("box"));
        }
    }
    Ok(rows)
}
```

`is_package` = `RowTarget::Folder(addr)` whose
`ctx.bundle.index(addr).profile.as_deref() == Some("uml-domain")`. Concept
rows are passed through untouched.

Because it mints nothing: `resolve` returns `Err(Unresolved)`, `occludes`
keeps the default `false`, `apply`/`surface` forward to `next`
unconditionally, and `RowId.owner` is never rewritten — rewriting it would
break persisted ids and misdirect `apply`.

Tests: `["uml"]` over a directory with one `uml-domain` child and one plain
child stamps `"box"` on exactly the first and leaves `"book"` on the second;
a `uml.Class` row carries the same `IconId("class")` through `["uml"]` as
through `["index"]`; `RowId.owner` is still the root view's and `resolve`
of a decorated path still succeeds through the chain; `occludes` is false for
every projected path; `apply` of a `Rename` through `["uml"]` lowers to the
same `okf::Op`s as through `["index"]`.

### Task 5 — `PROFILES` stops being a `const`

`crates/waml/src/profile.rs`.

A `ViewDecl` default holds a `Vec`, so the table becomes builder functions:
`shipped_profiles()` returns core's `okf`; `uml_profiles()` returns
`uml-domain` with
`default_view: Some(ViewDecl { entries: [ViewEntry { raw: "uml", line: 0 }] })`.
`line: 0` is the "not authored in any file" sentinel — a diagnostic spanning
an inherited default must not point at a line the author never wrote.

`profile(name)` keeps searching the whole shipped set; it must not become
extension-order dependent, since it is the parse-time name check. It can no
longer hand back `&'static` — return owned, or cache in a `OnceLock`.
`resolved_view` holds the borrow only across `Chain::build`, so either works.

Test: `profile("uml-domain")` resolves and carries `default_view: Some(["uml"])`;
`profile("okf")` resolves with `None`; `profile("UML-Domain")` still does not
(no case folding).

### Task 6 — `UmlExt`

`crates/waml/src/extension.rs`.

`pub struct UmlExt` implementing `CoreExtension`: `name() == "uml"`,
`middleware()` registers `("uml", || Box::new(UmlView))`, `profiles()`
returns `uml_profiles()`. **Move** `uml-domain` out of `CoreExt::profiles()`
— a profile the UML extension owns must not resolve with that extension
absent.

Tests: `MiddlewareRegistry::from_extensions(&[&CoreExt, &UmlExt])` builds
with no duplicate-name error and resolves both `index` and `uml`; a folder
with `profile: uml-domain` and **no** `view:` resolves to the `["uml"]` chain
through the profile default and its `uml-domain` child comes back with
`"box"` (the `default_view` wiring end to end — put this in `okf.rs` beside
`resolved_view_walks_local_then_profile_default_then_root_only`).

### Task 7 — one classifier: `tree.rs` consumes `RowKind`

`crates/waml-editor/src/tree.rs`, `document.rs`.

`NavCategory` gains `From<RowKind>`; the local `kind_of` becomes a forward to
`waml::view::kind::kind_of` (or is deleted at its call sites). `TreeKind =
NavCategory` and `IconSet::icon_for` are **untouched** — the glyph table
stays where it is and says what it says.

This is the crate-crossing step. A dropped or reordered arm silently
reclassifies documents, which moves the icon *and* the accent bucket
(`accent.rs:73`). Task 2's variant-for-variant test is the primary net;
`accent.rs`'s existing `tree_kinds_agree_with_the_element_type_buckets` is the
second.

### Task 8 — `EditorExtension::icons` and `resolve_icon`

`crates/waml-editor/src/extension_editor.rs`, `diagnostic.rs`.

`EditorExtension` gains `fn icons(&self) -> Vec<(&'static str, Icon)>`
(default `Vec::new()`). `CoreEditorExtension::icons()` returns the ten kind
names mapped through the **existing** `IconSet::icon_for(NavCategory::from(kind))`
— written as an iteration over `RowKind`'s variants, never a second
hand-written glyph table — plus `("book", Icon::Book)`. New
`UmlEditorExtension`: `name() == "uml"`, `surfaces()` empty for now,
`icons()` returns `[("box", Icon::Box)]`.

`resolve_icon(Option<&IconId>, table) -> (Icon, Option<Diagnostic>)`: `None`
→ the row's default by target (`Folder` → `Icon::Folder`, `Concept`/`Virtual`
→ `Icon::FileText`), a pure degrade path now that every shipped stage names
its icon; an unknown name → that same default plus a `DiagCode::UnknownIcon`
warning. Add `UnknownIcon` to `diagnostic.rs`'s enum, its `as_str()`
(`"unknown-icon"`), and the `Severity::Warning` arm — mirroring
`UnknownSurface` exactly.

Tests: known name resolves silently; unknown name degrades **and** warns;
`None` degrades with no diagnostic. And the load-bearing one: **every
`RowKind` resolves to the same `Icon` it resolves to through
`IconSet::icon_for` today** — the "no existing glyph changes" claim asserted
rather than promised.

### Task 9 — the editor extension registry

`crates/waml-editor/src/folder_projection.rs`.

A `core_registry()` sibling for the editor extension list, so two
construction sites cannot disagree — the same argument the existing doc
comment makes for the middleware registry. `core_registry()` itself grows
`&UmlExt`.

Test: the middleware registry and the editor registry name the same extension
set.

### Task 10 — `FolderRowView` carries an `Icon`

`crates/waml-editor/src/folder_view.rs`.

`FolderRowView.bullet: &'static str` becomes `icon: Icon`, resolved in
`row_view` through Task 8. `row_view`/`row_views` take the icon table they
need to resolve.

Test: `row_views` over a mixed listing yields
`[Icon::Box, Icon::Book, Icon::PanelTop, Icon::StickyNote]` in projected
order — the last two being today's class and note glyphs, unchanged.

### Task 11 — the tree panel resolves the row's icon

`crates/waml-editor/src/tree_panel.rs`.

A tree row's icon comes from the projected row's `IconId` when it has one,
falling back to `IconSet::icon_for(kind)` for rows minted outside the chain.
Directory rows become the book glyph, matching the folder tab for the same
directory — that is `folder_projection.rs`'s invariant applied to the icon,
and it is intended.

Test: a tree row and the folder row for the same directory resolve to the
same `Icon`.

### Task 12 — draw the icon (headless-untestable; visual checks deferred)

`crates/waml-editor/src/folder_list.rs`.

Replace the `bullet` Label with a 16×16 anchor `View` and draw the icon
immediate-mode in `draw_walk` via `IconSet::draw`, the pattern established in
`recent_row.rs:278`. `FolderRow` gains `icons: IconSet`, the per-row `Icon`,
and the anchor rect captured during `draw_walk`. Tint `atlas.text_dim`,
matching the bullet it replaces.

This is a `script_mod!` widget: changing the DSL namespace risks the silent
blank-chrome failure (`script-mod-namespace-object-literal`), and the
`IconSet` material must be registered before `FolderRow` draws
(`iconbutton-child-needs-script-mod-order`) or the glyph is absent with a
green gate. **No automated check covers this** — V1–V6 below are the
verification, and they are owed to a human at the end.

### Task 13 — gate assertions

`crates/waml-editor/src/script_gate.rs`.

Extend `every_core_extension_has_a_paired_editor_extension_by_name` to the
set `{core, uml}`. Add its icon analogue: **every `IconId` a registered
middleware can mint has a registered `Icon`**, shaped like the existing
`every_surface_id_resolvable_by_a_registered_chain_has_a_registered_factory`.
A stage stamping a name nothing resolves must fail the gate, not degrade
silently in front of a reader.

---

## Visual checks (owed — cannot be automated)

- **V1** core folder rows draw the book glyph, vertically centred against the
  label baseline, at the bullet's old position.
- **V2** a `view: uml` folder draws the box glyph on `uml-domain` children and
  the book glyph on plain ones, in one listing.
- **V3** a `profile: uml-domain` folder with **no** `view:` draws the same as
  V2 — the profile default reaching the surface.
- **V4** concept rows draw exactly the glyphs they draw today (class, note,
  diagram, sequence side by side), in the folder view and the tree.
- **V5** the tree panel's directory rows draw the book glyph, matching the
  folder tab for the same directory.
- **V6** the icon tints with the theme in both light and dark.
