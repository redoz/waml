# Folder glyph by declared profile — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a folder's tree/listing glyph a function of the profile the directory declares in its own frontmatter (raw `index.profile`) — plain `folder` when it declares nothing, `book` for `profile: okf`, `box` for `profile: uml-domain`, and `book` for the opened bundle top that declares nothing — instead of a hardcoded `book` stamped on every folder plus a view-chain-dependent `box`.

**Architecture:** Add a `folder_icon` field to `ProfileDef` so the glyph is a property of the profile. `RootView::folder_row` and the tree's root-node path compute the glyph directly from the child's own declared profile, so `box` no longer depends on a `UmlView` view-chain stage. The `UmlView` icon-decorator middleware is then retired entirely; `UmlEditorExtension` stays (it still contributes the `box` icon the new path relies on).

**Tech Stack:** Rust workspace (`crates/waml`, `crates/waml-editor`). Tests are `#[cfg(test)]` unit tests run with `cargo test`. Icons resolve through `waml_editor::extension_editor::resolve_icon` against `folder_projection::icon_table()`.

## Global Constraints

- **Declaration, not inheritance, drives the glyph.** Use the raw `index.profile` of the directory itself, never `resolved_profile` (which inherits). A plain folder inside a package is a `folder`; a declared sub-package inside a package is a `box`.
- **Only the bundle-root (`parent.is_none()`) node defaults an undeclared directory to `book`.** Every other undeclared directory is interior and degrades to the plain `folder` glyph (no stamp).
- **An unknown/foreign profile name resolves no `ProfileDef`** → treat as "no `folder_icon`" → degrade to the plain `folder` glyph.
- **No new icon assets, no DSL changes, no `docs/waml` content edits.** `book`, `box`, `folder` are already in the editor icon table (`CoreEditorExtension` ships `book` + kinds, `UmlEditorExtension` ships `box`, `resolve_icon` degrades an unstamped `Folder` to `Icon::Folder`).
- **`UmlEditorExtension` must stay** (it contributes `("box", Icon::Box)` to `icon_table()`). Only the headless `UmlView` *middleware* stage is retired.
- **Gate after every task** (each task must be an independently committable GREEN unit): `cargo test --workspace` **plus** the editor/vscode lint+build the repo gate runs. Clippy runs `-D warnings`, which promotes `dead_code` to a hard error — retiring `UmlView` must leave no orphaned references. The workspace must compile and gate green after every task; never stamp a profile's icon name before the field exists, and never remove a symbol before its last use is gone.

**Key file map (read before starting):**
- `crates/waml/src/profile.rs` — `ProfileDef`, `shipped_profiles()` (okf), `uml_profiles()` (uml-domain), `profile(name)` lookup, `register_test_profile`, tests.
- `crates/waml/src/view/root.rs` — `RootView::folder_row` (~129-170); `pub const FOLDER_ROW_ICON = "book"` (~41) and its stamp at line ~153.
- `crates/waml/src/view.rs:16` — `pub use root::{FOLDER_ROW_ICON, ROOT_VIEW_NAME, ROOT_VIEW_OWNER};` re-export.
- `crates/waml/src/view/uml.rs` — `UmlView` stage + `is_package` helper + tests (retired in Task 4).
- `crates/waml/src/extension.rs` — `UmlExt` (registers the `uml` middleware + `uml_profiles`), `SHIPPED_EXTENSIONS`, tests.
- `crates/waml/src/okf.rs` — `uml_domain_profile_default_reaches_the_surface_end_to_end` test (~1826), `register_test_profile(ProfileDef {…})` sites (~1763, ~1789).
- `crates/waml/src/view/chain.rs` — `from_extensions_records_which_extension_owns_each_name` (~2286), `masking_one_stage_keeps_its_siblings_and_diagnoses_nothing` (~960), `a_surviving_stage_keeps_the_id_it_would_have_had_unmasked` (~976), `registry_with_doubles()` (~916), `core_registry_for_tests()` (~924).
- `crates/waml-editor/src/extension_editor.rs` — `CoreEditorExtension::icons()` (`book` + kinds), `UmlEditorExtension::icons()` (`box`), `resolve_icon()`.
- `crates/waml-editor/src/folder_projection.rs` — `icon_table()`, `core_registry()`, `maskable_names()`, `stage_label()`/`extension_label()`, tests.
- `crates/waml-editor/src/tree.rs` — `build_tree_with_registry`'s nested `default_directory_icon`/`shallow_directory_node`/`directory_node` (uses `waml::view::FOLDER_ROW_ICON` for the root node), plus the tree tests.
- `crates/waml-editor/src/folder_view.rs` — `folder_view_model_lists_projected_rows_in_order` (~470) and `row_views_resolves_the_icon_table_for_a_mixed_listing` (~503) tests.
- `crates/waml-editor/src/script_gate.rs` — `every_icon_id_a_registered_middleware_can_mint_has_a_registered_icon` (~154).
- `crates/waml-editor/tests/fixtures/packages/` — root + billing declare `profile: uml-domain`; notes declares nothing.

---

### Task 1: Add `folder_icon` to `ProfileDef`

Add the field to the profile record and set it on both shipped profiles. Pure data addition — no behavior change yet (`folder_row` still stamps the hardcoded `FOLDER_ROW_ICON` until Task 2). This unit must compile the whole workspace, so every `ProfileDef {…}` literal is updated in the same commit.

**Files:**
- Modify: `crates/waml/src/profile.rs` (struct + `shipped_profiles()` + `uml_profiles()` + tests)
- Modify: `crates/waml/src/okf.rs` (two `register_test_profile(ProfileDef {…})` literals, ~1763 and ~1789)

**Interfaces:**
- Produces: `waml::profile::ProfileDef` gains `pub folder_icon: &'static str`. `shipped_profiles()` sets `okf` → `"book"`; `uml_profiles()` sets `uml-domain` → `"box"`. `default_view` is UNCHANGED in this task (still `Some(["uml"])` for `uml-domain`).

- [ ] **Step 1: Add the field to the struct**

In `crates/waml/src/profile.rs`, extend `ProfileDef`:

```rust
/// One known profile: its exact name, its optional default view chain, and
/// the folder glyph a directory draws when it declares this profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: &'static str,
    pub default_view: Option<ViewDecl>,
    /// The `IconId` name a directory declaring this profile stamps on its
    /// own folder row (`RootView::folder_row`) and on the tree's root node.
    /// `"book"` marks an OKF bundle root, `"box"` a UML package.
    pub folder_icon: &'static str,
}
```

- [ ] **Step 2: Set it on the shipped profiles**

In `shipped_profiles()`:

```rust
pub(crate) fn shipped_profiles() -> Vec<ProfileDef> {
    vec![ProfileDef {
        name: "okf",
        default_view: None,
        folder_icon: "book",
    }]
}
```

In `uml_profiles()` (leave `default_view` as `Some(["uml"])` for now):

```rust
pub(crate) fn uml_profiles() -> Vec<ProfileDef> {
    vec![ProfileDef {
        name: "uml-domain",
        default_view: Some(ViewDecl {
            entries: vec![ViewEntry {
                raw: "uml".to_string(),
                line: 0,
            }],
        }),
        folder_icon: "box",
    }]
}
```

- [ ] **Step 3: Update the two test-profile literals in `okf.rs`**

Both `register_test_profile(ProfileDef {…})` sites (~1763, ~1789) construct the struct literally and now need the field. Add `folder_icon: "book",` to each (value is irrelevant to those tests, which exercise `default_view`):

```rust
register_test_profile(ProfileDef {
    name: "marked-profile",
    default_view: Some(view_decl("profile-marker")),
    folder_icon: "book",
});
```

```rust
register_test_profile(ProfileDef {
    name: "inherited-only",
    default_view: Some(view_decl("profile-marker")),
    folder_icon: "book",
});
```

- [ ] **Step 4: Assert the new field in the profile test**

In `crates/waml/src/profile.rs`, extend `shipped_profiles_resolve_by_name` to pin the glyphs (keep the existing `default_view` assertions unchanged):

```rust
let okf = profile("okf").expect("okf is shipped");
assert_eq!(okf.name, "okf");
assert_eq!(okf.default_view, None);
assert_eq!(okf.folder_icon, "book");

let uml = profile("uml-domain").expect("uml-domain is shipped");
assert_eq!(uml.folder_icon, "box");
```

- [ ] **Step 5: Run the affected crate tests**

Run: `cargo test -p waml profile:: -- --nocapture` then `cargo test -p waml`
Expected: PASS. (`shipped_table_is_the_union_over_shipped_extensions` still passes — both sides build from the same builders, so `ProfileDef` equality holds.)

- [ ] **Step 6: Gate + commit**

Run: `cargo test --workspace` and the editor/vscode lint+build the repo gate runs.
Expected: PASS (no behavior change; every literal updated so the workspace compiles).

```bash
git add crates/waml/src/profile.rs crates/waml/src/okf.rs
git commit -m "feat(profile): add folder_icon field to ProfileDef"
```

---

### Task 2: Stamp the declared-profile glyph in `folder_row` and the tree root node

Replace the hardcoded `book` stamp with a lookup on the child's own declared profile (Seam 1), and make the tree's root node draw its own declared-profile glyph defaulting to `book` (Seam 2). Remove the now-dead `FOLDER_ROW_ICON` const and its re-export. Update every existing icon-asserting test in the same commit so the unit is green. The `UmlView` middleware stays alive (still redundantly stamps `box`) — it is retired in Task 4.

**Files:**
- Modify: `crates/waml/src/view/root.rs` (`folder_row` stamp; delete `FOLDER_ROW_ICON` const + doc; update `rows_carry_the_icon_their_kind_or_target_implies` test)
- Modify: `crates/waml/src/view.rs:16` (drop `FOLDER_ROW_ICON` from the re-export)
- Modify: `crates/waml/src/view/uml.rs` (delete the superseded `stamps_box_on_the_uml_domain_child_only` test)
- Modify: `crates/waml-editor/src/tree.rs` (root-node glyph; update 4 tests)
- Modify: `crates/waml-editor/src/folder_view.rs` (update 2 tests)
- Modify: `crates/waml-editor/src/script_gate.rs` (comment only)

**Interfaces:**
- Consumes: `ProfileDef.folder_icon` and `waml::profile::profile(name) -> Option<ProfileDef>` (Task 1).
- Produces: `folder_row` sets `row.icon = index(child).profile → profile(p) → IconId::new(def.folder_icon)`, else `None`. The tree gains a helper `declared_directory_glyph(bundle, address) -> &'static str` that returns the directory's declared-profile `folder_icon` defaulting to `"book"`. `waml::view::FOLDER_ROW_ICON` is REMOVED.

- [ ] **Step 1: Rewrite the `folder_row` icon stamp (Seam 1)**

In `crates/waml/src/view/root.rs`, replace the line `row.icon = Some(IconId::new(FOLDER_ROW_ICON));` (~153) with a lookup on the child's own declared profile:

```rust
// The glyph is a property of the profile the directory DECLARES in its
// own frontmatter (raw `index.profile`, never the inherited
// `resolved_profile`): `okf` -> book, `uml-domain` -> box, an unknown or
// foreign name -> no ProfileDef -> no stamp. A child that declares no
// profile stamps nothing; `resolve_icon` degrades an unstamped Folder row
// to `Icon::Folder`, so plain folders draw the plain folder glyph.
row.icon = child_index
    .and_then(|index| index.profile.as_deref())
    .and_then(crate::profile::profile)
    .map(|def| IconId::new(def.folder_icon));
```

(`child_index` is already bound at the top of `folder_row` as `ctx.bundle.index(address.as_str())`.)

- [ ] **Step 2: Delete the `FOLDER_ROW_ICON` const and its re-export**

In `crates/waml/src/view/root.rs`, delete the `pub const FOLDER_ROW_ICON: &str = "book";` declaration (~41) together with its `///` doc comment.

In `crates/waml/src/view.rs`, change line 16 from:

```rust
pub use root::{FOLDER_ROW_ICON, ROOT_VIEW_NAME, ROOT_VIEW_OWNER};
```

to:

```rust
pub use root::{ROOT_VIEW_NAME, ROOT_VIEW_OWNER};
```

- [ ] **Step 3: Make the tree root node draw the declared-profile glyph (Seam 2)**

In `crates/waml-editor/src/tree.rs`, inside `build_tree_with_registry`, add a helper next to `default_directory_icon`, and change `default_directory_icon` to take the glyph name instead of hardcoding `waml::view::FOLDER_ROW_ICON`:

```rust
/// The glyph name a directory draws for its OWN declared profile
/// (`index.profile`), defaulting to `"book"` — the OKF bundle-root glyph —
/// when it declares none. Declaration, not the inherited `resolved_profile`,
/// drives it; an unknown/foreign name resolves no `ProfileDef` and falls
/// back to `"book"` here (this is only reached for the bundle-root node, the
/// one directory no listing produced a row for).
fn declared_directory_glyph(
    bundle: &waml::okf::Bundle,
    address: &waml::okf::DirectoryAddress,
) -> &'static str {
    bundle
        .index(address.as_str())
        .and_then(|index| index.profile.as_deref())
        .and_then(waml::profile::profile)
        .map(|def| def.folder_icon)
        .unwrap_or("book")
}

fn default_directory_icon(
    address: &waml::okf::DirectoryAddress,
    glyph: &str,
    table: &[(&str, Icon)],
) -> (Icon, Option<waml::diagnostic::Diagnostic>) {
    crate::extension_editor::resolve_icon(
        Some(&waml::view::row::IconId::new(glyph)),
        &waml::view::row::RowTarget::Folder(address.as_str().to_string()),
        table,
        address.as_str(),
        0,
    )
}
```

Update the two callers of `default_directory_icon`:

- In `directory_node`, at the `let (default_icon, default_icon_diagnostic) = default_directory_icon(address, table);` site (~250), pass the declared glyph (`bundle` is in scope there):

```rust
let glyph = declared_directory_glyph(bundle, address);
let (default_icon, default_icon_diagnostic) = default_directory_icon(address, glyph, table);
```

- In `shallow_directory_node` (whose result is always overwritten by the producing row's icon at the `Folder` arm, but keep it correct): give it the `bundle` so it can compute the glyph. Change its signature to `fn shallow_directory_node(bundle: &waml::analysis::OkfAnalysis /* or &Bundle */, address, table)` OR pass the glyph from the caller. Simplest: pass the glyph from the `Folder` arm caller, mirroring `directory_node`:

```rust
// in shallow_directory_node's body, replace default_directory_icon(address, table).0
icon: default_directory_icon(address, glyph, table).0,
```

and change its signature to accept `glyph: &str`, then at its single call site (the `repeat` branch of the `Folder` arm) call `shallow_directory_node(&child, declared_directory_glyph(bundle, &child), table)`.

Note: `waml::view::FOLDER_ROW_ICON` no longer exists — this step is what removes the tree's last reference to it, so Steps 1-3 must land in the same commit.

- [ ] **Step 4: Fix the `script_gate` comment (box is now minted by `folder_row`)**

In `crates/waml-editor/src/script_gate.rs`, line ~161, update the comment on `mintable.insert("box");` from `// UmlView's package/box glyph` to `// RootView::folder_row stamps box on uml-domain folders`. (The assertion is unchanged — `box` is still in `icon_table()` via `UmlEditorExtension`.)

- [ ] **Step 5: Update `root.rs`'s folder-icon assertion**

In `crates/waml/src/view/root.rs`, `rows_carry_the_icon_their_kind_or_target_implies`: `archive/index.md` declares no profile, so its folder row now carries NO icon. Change:

```rust
assert_eq!(icon_for("archive"), Some("book".to_string()));
```

to:

```rust
// `archive` declares no profile, so `folder_row` stamps nothing; the row
// carries no icon and `resolve_icon` degrades it to the plain folder glyph.
assert_eq!(icon_for("archive"), None);
```

- [ ] **Step 6: Delete the superseded `UmlView` box test**

In `crates/waml/src/view/uml.rs`, delete the `stamps_box_on_the_uml_domain_child_only` test entirely. Its box-from-the-stage premise is superseded — the box-from-`folder_row` guarantee is added as a new test in Task 3, and `folder_row` now stamps nothing on the `plain` child (so its old `Some("book")` assertion is invalid). The other `uml.rs` tests (`occludes_is_false_for_every_projected_path`, `resolve_always_declines`, `row_id_owner_is_untouched`) stay and pass.

- [ ] **Step 7: Update the two `folder_view.rs` icon tests**

In `crates/waml-editor/src/folder_view.rs`:

`folder_view_model_lists_projected_rows_in_order` — `Sales` declares no profile, so it is now the plain folder glyph:

```rust
assert_eq!(
    rows[1].icon,
    Icon::Folder,
    "a plain folder row carries the folder glyph"
);
```

`row_views_resolves_the_icon_table_for_a_mixed_listing` — drop `view: uml` from the root `index.md` (box no longer needs a stage), and `Docs` (no profile) is now `Folder`:

```rust
let prepared = analysis([
    (
        "index.md",
        "# Root\n\n* [Pkg](pkg/)\n* [Docs](docs/)\n* [Order](order.md)\n* [Notes](notes.md)\n",
    ),
    ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
    ("docs/index.md", "# Docs\n"),
    ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
    ("notes.md", "---\ntype: note\n---\n# Notes\n"),
]);
// ... build FolderView, take rows ...
assert_eq!(
    rows.iter().map(|row| row.icon).collect::<Vec<_>>(),
    vec![Icon::Box, Icon::Folder, Icon::PanelTop, Icon::FileText],
    "box from the declared uml-domain profile, folder for the plain child; \
     class and note glyphs unchanged",
);
```

- [ ] **Step 8: Update the four `tree.rs` icon tests**

In `crates/waml-editor/src/tree.rs`:

`the_packages_fixture_draws_a_box_for_its_declared_package` — `Notes` (no profile) is now `Folder`; `Billing` stays `Box`:

```rust
assert_eq!(icons["Billing"], Icon::Box, "a declared uml-domain package");
assert_eq!(
    icons["Notes"],
    Icon::Folder,
    "a plain folder declaring nothing"
);
```

`a_declared_package_under_a_chainless_parent_draws_the_plain_folder_glyph` — INVERTS. Rename to `a_declared_package_draws_a_box_without_a_uml_stage_in_the_parent_chain`, keep the `resolved_profile` assertion, and flip the icon assertion (the box now comes from `folder_row`, independent of any parent view chain):

```rust
/// A folder declaring `profile: uml-domain` draws the box glyph from its
/// own declaration, with NO `uml` stage in the parent chain — the "no boxes
/// at all" regression this change fixes. Declaration alone is now sufficient.
#[test]
fn a_declared_package_draws_a_box_without_a_uml_stage_in_the_parent_chain() {
    // ... same fixture (root declares nothing, pkg declares uml-domain), same build ...
    assert_eq!(
        prepared.okf().bundle.resolved_profile("/pkg"),
        Some("uml-domain"),
        "the declaration itself resolves",
    );
    assert_eq!(
        tree.roots[0].children[0].presentation.icon,
        Icon::Box,
        "the box comes from folder_row's folder_icon, not a uml stage",
    );
}
```

`tree_row_icon_matches_the_folder_row_icon_for_the_same_directory` — drop `view: uml` from the root `index.md`; `Docs` (no profile) is now `Folder`; `Pkg` stays `Box`; keep the tree==folder-view equality:

```rust
let source = SourceBundle::try_from_pairs([
    ("index.md", "# Root\n\n* [Pkg](pkg/)\n* [Docs](docs/)\n"),
    ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
    ("docs/index.md", "# Docs\n"),
])
.unwrap();
// ... build tree + FolderView ...
assert_eq!(tree_icons["Pkg"], Icon::Box, "a uml-domain package");
assert_eq!(tree_icons["Docs"], Icon::Folder, "a plain folder");
assert_eq!(tree_icons["Pkg"], folder_icons["Pkg"]);
assert_eq!(tree_icons["Docs"], folder_icons["Docs"]);
```

`the_root_node_draws_the_same_glyph_as_its_directory_children` — premise dies (root=book, plain child=folder). Rename to `the_root_node_draws_the_okf_bundle_root_glyph_for_an_undeclared_top`, keep the same fixture (root and `docs` both declare nothing), assert the root books and DROP the root==child equality clause:

```rust
/// The root node is the one directory no listing produced a row for, so it
/// stamps its own declared-profile glyph — defaulting to `book`, the OKF
/// bundle-root glyph, for an undeclared top. Its plain interior child draws
/// the plain folder glyph; the two legitimately differ now.
#[test]
fn the_root_node_draws_the_okf_bundle_root_glyph_for_an_undeclared_top() {
    // ... same fixture + build ...
    let root = &tree.roots[0];
    assert_eq!(root.presentation.icon, Icon::Book);
    assert_eq!(root.children[0].presentation.icon, Icon::Folder);
}
```

- [ ] **Step 9: Run the affected tests**

Run: `cargo test -p waml view::root:: && cargo test -p waml view::uml:: && cargo test -p waml-editor tree:: && cargo test -p waml-editor folder_view::`
Expected: PASS.

- [ ] **Step 10: Gate + commit**

Run: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and the editor/vscode lint+build.
Expected: PASS. Note: `okf::uml_domain_profile_default_reaches_the_surface_end_to_end` still passes — `pkg` gets its box from `folder_row` now (in addition to the still-alive `uml` stage).

```bash
git add crates/waml/src/view/root.rs crates/waml/src/view.rs crates/waml/src/view/uml.rs \
        crates/waml-editor/src/tree.rs crates/waml-editor/src/folder_view.rs \
        crates/waml-editor/src/script_gate.rs
git commit -m "feat(view): draw folder glyph from the directory's declared profile"
```

---

### Task 3: Add the new declared-profile glyph tests

Additive tests only — they lock in the Task 2 behavior. No production code changes. The `UmlView` middleware is still alive here, which is fine (these tests don't depend on it).

**Files:**
- Modify: `crates/waml/src/view/root.rs` (new headless folder-row test)
- Modify: `crates/waml-editor/src/tree.rs` (new root-node box test + new cross-surface tri-glyph test)

**Interfaces:**
- Consumes: `folder_row` icon behavior and the tree root-node glyph from Task 2; `FolderView::build` / `row_views()` and `build_tree` as used by the existing `tree_row_icon_matches...` test.

- [ ] **Step 1: `folder_row` stamps book/box/none by declared profile (headless)**

Add to `crates/waml/src/view/root.rs`'s `mod tests`, mirroring `rows_carry_the_icon_their_kind_or_target_implies` (build with `prepare_candidate`, run `Chain::root_only`, read `row.icon.as_str()`):

```rust
#[test]
fn folder_row_stamps_the_declared_profiles_glyph() {
    let source = SourceBundle::try_from_pairs([
        (
            "index.md",
            "# Root\n\n* [Okf](okf/)\n* [Pkg](pkg/)\n* [Plain](plain/)\n",
        ),
        ("okf/index.md", "---\nprofile: okf\n---\n# Okf\n"),
        ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
        ("plain/index.md", "# Plain\n"),
    ])
    .unwrap();
    let prepared = prepare_candidate(source, None, 1).unwrap();
    let (_, okf, _uml, _) = prepared.into_parts();
    let bundle = okf.bundle;
    let root_address = okf::DirectoryAddress::parse("/").unwrap();
    let directory = bundle.directory(root_address.as_str()).unwrap().clone();
    let params = crate::frontmatter::Frontmatter::default();
    let descend = |_: &okf::Directory| Chain::default();
    let projection_ctx = ctx(&directory, &bundle, &params, &descend);

    let registry = MiddlewareRegistry::new();
    let chain = Chain::root_only(&registry);
    let outcome = chain.run(&projection_ctx, ChainLimits::default());
    let icon_for = |path: &str| -> Option<String> {
        outcome
            .rows
            .iter()
            .find(|row| row.id.path.as_str() == path)
            .and_then(|row| row.icon.as_ref())
            .map(|icon| icon.as_str().to_string())
    };
    assert_eq!(icon_for("okf"), Some("book".to_string()), "profile: okf -> book");
    assert_eq!(icon_for("pkg"), Some("box".to_string()), "profile: uml-domain -> box");
    assert_eq!(icon_for("plain"), None, "no declared profile -> no stamp");
}
```

- [ ] **Step 2: The bundle-root node boxes a `uml-domain` top**

Add to `crates/waml-editor/src/tree.rs`'s `mod tests`. Use the `packages` fixture, whose root declares `profile: uml-domain`:

```rust
/// The bundle-root node stamps its OWN declared-profile glyph. The packages
/// fixture root declares `profile: uml-domain`, so the tree's root node draws
/// a box — the counterpart to `..._for_an_undeclared_top`, which books.
#[test]
fn the_bundle_root_node_boxes_a_uml_domain_top() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packages");
    let source = crate::load::read_bundle(&dir).expect("the packages fixture loads");
    let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
    let tree = build_tree(
        prepared.okf(),
        prepared.uml(),
        "Fallback",
        &ProjectionMask::default(),
        waml::view::chain::ChainLimits::default(),
    );
    assert_eq!(
        tree.roots[0].presentation.icon,
        Icon::Box,
        "the root declares uml-domain, so the root node boxes",
    );
}
```

- [ ] **Step 3: Cross-surface tree==folder-view across all three glyphs**

Add to `crates/waml-editor/src/tree.rs`'s `mod tests`, mirroring `tree_row_icon_matches_the_folder_row_icon_for_the_same_directory` but with one child per glyph:

```rust
/// The tree and the folder view resolve the SAME `IconId` against the SAME
/// table, so an okf child books, a uml-domain child boxes, and a plain child
/// folders — identically on both surfaces.
#[test]
fn tree_and_folder_view_agree_across_book_box_and_folder_glyphs() {
    let source = SourceBundle::try_from_pairs([
        (
            "index.md",
            "# Root\n\n* [Book](book-dir/)\n* [Pkg](pkg/)\n* [Plain](plain/)\n",
        ),
        ("book-dir/index.md", "---\nprofile: okf\n---\n# Book\n"),
        ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
        ("plain/index.md", "# Plain\n"),
    ])
    .unwrap();
    let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
    let limits = waml::view::chain::ChainLimits::default();
    let mask = ProjectionMask::default();

    let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", &mask, limits);
    let tree_icons: std::collections::HashMap<String, Icon> = tree.roots[0]
        .children
        .iter()
        .map(|node| (node.title.clone(), node.presentation.icon))
        .collect();

    let folder = crate::folder_view::FolderView::build(prepared.okf(), "/", limits, &mask)
        .expect("root is in the bundle");
    let folder_icons: std::collections::HashMap<String, Icon> = folder
        .row_views()
        .iter()
        .map(|row| (row.label.clone(), row.icon))
        .collect();

    assert_eq!(tree_icons["Book"], Icon::Book);
    assert_eq!(tree_icons["Pkg"], Icon::Box);
    assert_eq!(tree_icons["Plain"], Icon::Folder);
    for label in ["Book", "Pkg", "Plain"] {
        assert_eq!(tree_icons[label], folder_icons[label], "{label} disagrees across surfaces");
    }
}
```

- [ ] **Step 4: Run the new tests**

Run: `cargo test -p waml folder_row_stamps_the_declared_profiles_glyph && cargo test -p waml-editor the_bundle_root_node_boxes_a_uml_domain_top && cargo test -p waml-editor tree_and_folder_view_agree_across_book_box_and_folder_glyphs`
Expected: PASS.

- [ ] **Step 5: Gate + commit**

Run: `cargo test --workspace` and the editor/vscode lint+build.
Expected: PASS.

```bash
git add crates/waml/src/view/root.rs crates/waml-editor/src/tree.rs
git commit -m "test(view): cover folder glyph by declared profile on both surfaces"
```

---

### Task 4: Retire the `UmlView` middleware stage

The box now comes from `folder_row`, so the `UmlView` icon decorator is redundant. Remove the stage, its `uml` middleware registration, and its module; drop `uml-domain`'s `default_view` to `None`. `UmlExt` stays as an extension (it still ships the `uml-domain` profile), and `UmlEditorExtension` stays (it still ships the `box` icon). Update the registration/masking tests that used `uml` as a live stage. This is the "confirm nothing else needs the `uml` middleware name" task: the only `view: uml` declarations anywhere are in the two test fixtures already rewritten in Task 2 (they no longer declare it); no shipped bundle (`docs/waml`, the `packages` fixture) declares `view: uml`, and the `docs/waml` folders NAMED `uml` declare no profile and are unaffected.

**Files:**
- Delete: `crates/waml/src/view/uml.rs`
- Modify: `crates/waml/src/view.rs` (remove the `uml` module declaration)
- Modify: `crates/waml/src/extension.rs` (`UmlExt::middleware` → empty; doc comment; test rewrite)
- Modify: `crates/waml/src/profile.rs` (`uml_profiles()` `default_view` → `None`; doc comment; `shipped_profiles_resolve_by_name` test)
- Modify: `crates/waml/src/okf.rs` (rename/recomment `uml_domain_profile_default_reaches_the_surface_end_to_end`)
- Modify: `crates/waml/src/view/chain.rs` (2 masking tests off `uml`; the `owners()` shape test)
- Modify: `crates/waml-editor/src/folder_projection.rs` (`index_is_never_offered_as_maskable`; remove the `uml` arms from `stage_label`/`extension_label`)
- Modify: `crates/waml-editor/tests/fixtures/packages/index.md` (prose only — describe the new mechanism)

**Interfaces:**
- Consumes: `folder_row`'s box behavior (Task 2) — the box no longer depends on any stage.
- Produces: `UmlExt::middleware()` returns `Vec::new()`; the `uml` name is no longer a registered middleware (`registry.owner("uml") == None`); `profile("uml-domain").default_view == None` (and `.folder_icon == "box"` still). `waml::view::uml` and `UmlView` cease to exist.

- [ ] **Step 1: Delete the stage and its module declaration**

Delete the file `crates/waml/src/view/uml.rs`.

In `crates/waml/src/view.rs`, remove the `uml` module declaration line (e.g. `mod uml;` / `pub(crate) mod uml;`). Confirm no other `crate::view::uml` reference remains (only `extension.rs` had one, removed next).

- [ ] **Step 2: Empty `UmlExt::middleware` and update its doc**

In `crates/waml/src/extension.rs`, change `UmlExt::middleware` to register nothing and update the struct doc comment:

```rust
/// The `uml` extension: contributes the shipped `uml-domain` profile. It
/// registers NO view middleware — a `uml-domain` folder's box glyph is a
/// property of the profile (`ProfileDef::folder_icon`) that
/// `RootView::folder_row` stamps directly, so no decorator stage is needed.
/// `profile()` still name-checks `uml-domain` against the whole
/// `SHIPPED_EXTENSIONS` union.
pub struct UmlExt;

impl CoreExtension for UmlExt {
    fn name(&self) -> &str {
        "uml"
    }

    fn middleware(&self) -> Vec<(&'static str, MiddlewareFactory)> {
        Vec::new()
    }

    fn profiles(&self) -> Vec<ProfileDef> {
        crate::profile::uml_profiles()
    }
}
```

- [ ] **Step 3: Rewrite the `extension.rs` registration test**

`core_and_uml_extensions_register_with_no_duplicate_names` currently asserts the `uml` name resolves as a middleware. Rewrite it (rename to reflect the new reality) to assert the registry builds, `uml` registers no middleware, and the profile still ships:

```rust
#[test]
fn core_and_uml_extensions_register_with_no_duplicate_names_and_uml_adds_no_middleware() {
    let registry = MiddlewareRegistry::from_extensions(&[&CoreExt, &UmlExt])
        .expect("core and uml share no middleware names");

    let index_rows = run_ids(&registry);
    assert!(index_rows.is_empty());

    assert!(
        UmlExt.middleware().is_empty(),
        "the uml extension registers no view middleware anymore",
    );
    assert_eq!(
        registry.owner("uml"),
        None,
        "no `uml` middleware name is registered",
    );
    assert!(
        crate::profile::profile("uml-domain").is_some(),
        "the uml-domain profile still ships",
    );
}
```

- [ ] **Step 4: Drop `uml-domain`'s default view to `None`**

In `crates/waml/src/profile.rs`, `uml_profiles()`:

```rust
/// The `uml` extension's shipped profile. `uml-domain` folders draw the box
/// glyph (`folder_icon`) from their own declaration; they no longer carry a
/// default `view:` chain — a `uml-domain` folder with no `view:` resolves the
/// root-only chain, and its box comes from `folder_row`, not a stage.
pub(crate) fn uml_profiles() -> Vec<ProfileDef> {
    vec![ProfileDef {
        name: "uml-domain",
        default_view: None,
        folder_icon: "box",
    }]
}
```

In `shipped_profiles_resolve_by_name`, replace the block that asserts the `uml-domain` default view is `["uml"]` with:

```rust
let uml = profile("uml-domain").expect("uml-domain is shipped");
assert_eq!(uml.name, "uml-domain");
assert_eq!(uml.default_view, None);
assert_eq!(uml.folder_icon, "box");
```

- [ ] **Step 5: Rename/recomment the `okf.rs` end-to-end test**

In `crates/waml/src/okf.rs`, `uml_domain_profile_default_reaches_the_surface_end_to_end` still passes (the `pkg` row's box now comes from `folder_row`, and the root resolves the root-only chain with no diagnostics), but its name and comments describe the retired `["uml"]` default chain. Rename to `a_uml_domain_folder_row_draws_the_box_glyph` and update its two comments:

```rust
#[test]
fn a_uml_domain_folder_row_draws_the_box_glyph() {
    use crate::extension::{CoreExt, UmlExt};
    use crate::view::row::{IconId, RowTarget};

    let registry = MiddlewareRegistry::from_extensions(&[&CoreExt, &UmlExt])
        .expect("core and uml register no duplicate names");
    // ... same fixture + build ...
    let (chain, diags) = bundle.resolved_view("/", &registry, &ProjectionMask::default());
    assert!(
        diags.is_empty(),
        "a uml-domain folder with no view: resolves the root-only chain cleanly"
    );
    // ... run chain ...
    // `folder_row` stamps the box from the child's declared uml-domain profile.
    assert_eq!(pkg_row.icon.as_ref().map(IconId::as_str), Some("box"));
}
```

- [ ] **Step 6: Repoint the two `chain.rs` masking tests off `uml`**

`masking_one_stage_keeps_its_siblings_and_diagnoses_nothing` and `a_surviving_stage_keeps_the_id_it_would_have_had_unmasked` use `core_registry_for_tests()` + `decl(&["index", "uml"])`. With `uml` no longer registered, an unmasked build would diagnose it. Rewrite both to use two registered doubles from `registry_with_doubles()` (`pass-through`, `adding`), which build cleanly and don't depend on `uml`:

```rust
#[test]
fn masking_one_stage_keeps_its_siblings_and_diagnoses_nothing() {
    let registry = registry_with_doubles();
    let idx = index();
    let mask = ProjectionMask::from_names(["adding"]);
    let (chain, diags) = Chain::build(&decl(&["pass-through", "adding"]), &registry, &idx, &mask);
    assert!(
        diags.is_empty(),
        "a masked stage is a reader's choice, not an author error: {diags:?}",
    );
    assert_eq!(
        chain.ids().len(),
        1,
        "the sibling survives; only the masked stage is dropped",
    );
}

#[test]
fn a_surviving_stage_keeps_the_id_it_would_have_had_unmasked() {
    let registry = registry_with_doubles();
    let idx = index();
    let unmasked = Chain::build(
        &decl(&["pass-through", "adding"]),
        &registry,
        &idx,
        &ProjectionMask::default(),
    )
    .0;
    let masked = Chain::build(
        &decl(&["pass-through", "adding"]),
        &registry,
        &idx,
        &ProjectionMask::from_names(["pass-through"]),
    )
    .0;
    assert_eq!(
        masked.ids().first(),
        unmasked.ids().get(1),
        "ids come from the DECLARED names, so a mask flip never renumbers an owner",
    );
}
```

- [ ] **Step 7: Update the `chain.rs` owners-shape test**

In `from_extensions_records_which_extension_owns_each_name`, `uml` no longer owns any middleware name:

```rust
assert_eq!(registry.owner("hide"), Some("core"));
assert_eq!(registry.owner("index"), Some("core"));
assert_eq!(registry.owner("uml"), None);
assert_eq!(registry.owner("nonexistent"), None);

let owners = registry.owners();
let shape: Vec<(&str, Vec<&str>)> = owners
    .iter()
    .map(|(owner, names)| (*owner, names.clone()))
    .collect();
assert_eq!(
    shape,
    vec![("core", vec!["hide", "index"])],
    "owners() is the ONE source the editor's popup is built from; uml owns no stage now",
);
```

- [ ] **Step 8: Update `folder_projection.rs` maskable expectations and labels**

In `index_is_never_offered_as_maskable`, remove the `uml` assertion (keep the `index`/`hide` ones):

```rust
assert!(
    !offered.contains(&"index"),
    "`index` is the terminal stage; masking it cannot remove the listing",
);
assert!(offered.contains(&"hide"));
// `uml` is no longer a middleware stage — the box glyph is a profile property.
```

Remove the now-unreachable `uml` arms from `stage_label` and `extension_label` (they named a retired stage; `every_shipped_maskable_name_has_a_written_label` no longer visits `uml`):

```rust
// stage_label: delete the `"uml" => "Package icons".to_string(),` arm.
// extension_label: delete the `"uml" => "UML".to_string(),` arm.
```

- [ ] **Step 9: Update the `packages` fixture prose**

The `packages` fixture body describes the retired `["uml"]` default-view mechanism. In `crates/waml-editor/tests/fixtures/packages/index.md`, replace the two explanatory paragraphs (the ones about the root's `["uml"]` default view chain stamping the package glyph on the LISTING) with prose that matches the new model. Body text does not affect row labels or icons, so no test changes result:

```markdown
A bundle with real nested directories: one declared UML package and one plain
folder, so a reader can tell the package glyph from the folder glyph.

Each folder's glyph comes from the profile it declares in its own frontmatter:
`profile: uml-domain` draws a box, `profile: okf` draws a book, and a folder
declaring nothing draws the plain folder glyph. The root declares
`profile: uml-domain`, so the opened top draws a box.

* [Billing](billing/) - A declared `uml-domain` package.
* [Notes](notes/) - A plain folder, declaring no profile.
```

- [ ] **Step 10: Verify no orphaned `uml`-middleware references remain**

Run: `TOKENSAVE_DISABLE_GREP_HOOK=1 grep -rn "UmlView\|view::uml\|register(\"uml\"\|default_view: Some" crates/waml/src crates/waml-editor/src`
Expected: no `UmlView` / `view::uml` / `register("uml"` hits (the `default_view: Some(view_decl(...))` hits in `okf.rs` test helpers are unrelated test profiles and are fine).

- [ ] **Step 11: Gate + commit**

Run: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and the editor/vscode lint+build.
Expected: PASS. Clippy must be clean — retiring `UmlView` leaves no `dead_code`.

```bash
git add crates/waml/src/view.rs crates/waml/src/extension.rs crates/waml/src/profile.rs \
        crates/waml/src/okf.rs crates/waml/src/view/chain.rs \
        crates/waml-editor/src/folder_projection.rs \
        crates/waml-editor/tests/fixtures/packages/index.md
git add -A crates/waml/src/view/uml.rs
git commit -m "refactor(view): retire the UmlView middleware; box is a profile property"
```

---

## Self-Review

**Spec coverage:**
- Seam 0 (glyph is a property of the profile): Task 1 adds `ProfileDef.folder_icon` (okf→book, uml-domain→box).
- Seam 1 (`folder_row` stamps declared-profile glyph, else nothing): Task 2 Step 1.
- Seam 2 (bundle-root node stamps declared glyph, default book): Task 2 Step 3 (`declared_directory_glyph` + reworked `default_directory_icon`), replacing `FOLDER_ROW_ICON`'s job (Task 2 Step 2).
- Seam 3 (retire `UmlView`; drop `uml-domain` default_view; keep `UmlEditorExtension`): Task 4.
- Existing tests that invert/lose premise: all five named in the spec are handled — `the_packages_fixture...` (Task 2), `a_declared_package_under_a_chainless_parent...` (rename+invert, Task 2), `tree_row_icon_matches...` (Task 2), `the_root_node_draws_the_same_glyph...` (reframe, Task 2), `stamps_box_on_the_uml_domain_child_only` (deleted Task 2, assertion relocated to the new headless test Task 3).
- New tests: folder_row book/box/none (Task 3 Step 1), root books/boxes by declared top (Task 2 reframed test + Task 3 Step 2), cross-surface tri-glyph (Task 3 Step 3).
- Non-goals respected: no new icon assets, no DSL changes, no `docs/waml` content edits (only the `packages` test-fixture prose, which the spec's Testing section implicitly owns); no resolved/inherited-profile glyph rules; only the single `folder_icon` field.

**Extra churn found beyond the spec's key-file list (all handled so each unit gates green):** `root.rs`'s `rows_carry_the_icon_...` archive assertion (Task 2 Step 5); `folder_view.rs`'s two book-glyph tests (Task 2 Step 7); the `view.rs:16` re-export (Task 2 Step 2); `script_gate.rs` comment (Task 2 Step 4); `okf.rs`'s end-to-end test (Task 4 Step 5); `chain.rs`'s two `uml`-as-stand-in masking tests + owners-shape test (Task 4 Steps 6-7); `folder_projection.rs`'s `index_is_never_offered_as_maskable` + `stage_label`/`extension_label` uml arms (Task 4 Step 8).

**Type consistency:** `folder_icon: &'static str` set in Task 1, read as `def.folder_icon` in Task 2 (`IconId::new(def.folder_icon)`) and `declared_directory_glyph`. `waml::profile::profile(name) -> Option<ProfileDef>` used with `.and_then(crate::profile::profile)` / `.and_then(waml::profile::profile)`. `default_view` set to `None` for `uml-domain` only in Task 4 (kept `Some(["uml"])` through Tasks 1-3 so the `uml` stage stays coherent while still alive).

**Ordering invariant:** Task 1 adds the field (no behavior change, green). Task 2 changes behavior + updates all break-on-change tests + removes the const atomically (green). Task 3 is additive tests (green). Task 4 retires the middleware once the box no longer depends on it (green). No unit stamps a name before the field exists or removes a symbol before its last use is gone.
