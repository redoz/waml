# Folder glyph by declared profile

## Problem

Every folder row in a WAML bundle draws the **book** glyph. `RootView::folder_row`
(`crates/waml/src/view/root.rs`) stamps a hardcoded `FOLDER_ROW_ICON = "book"` on
every folder it mints, so an entire documentation tree reads as a shelf of books and
the book glyph carries no information.

Worse, the **box** glyph (a UML package) only appears when the parent directory's
resolved view chain happens to run the `uml` middleware stage (`UmlView`), which
decorates the *listing* rather than the folder itself. A `uml-domain` package whose
parent declares no `view: uml` gets no box — it falls back to the generic book. In
the shipped docs there are no boxes at all.

The book glyph should mean something specific, not "a folder exists here."

## Model

A folder's glyph is decided by the profile it **declares in its own frontmatter**
(the raw `index.profile`, *not* the inherited `resolved_profile`):

| Directory declares            | Glyph    | Meaning                          |
| ----------------------------- | -------- | -------------------------------- |
| nothing (interior folder)     | `folder` | just a subdirectory              |
| `profile: okf`                | `book`   | a nested OKF bundle root         |
| `profile: uml-domain`         | `box`    | a UML package                    |
| nothing, **but the bundle top** | `book`   | the opened top is an OKF root    |

Book becomes a **bundle-root marker**, box a **package marker**, and the plain folder
glyph the default for ordinary subdirectories. Book/box reappear wherever a directory
declares its own profile, so nesting an OKF bundle (`profile: okf`) inside a package
re-books at that boundary, and a sub-package (`profile: uml-domain`) inside a package
keeps its box. Declaration — not resolved/inherited profile — drives the glyph, so a
plain folder inside a package draws the plain folder glyph, and a declared sub-package
inside a package still draws a box.

### Worked examples

```
docs/waml/            book    (opened top, declares nothing)
  goals/              folder
    uml/              folder  (a doc folder named "uml"; declares nothing)
  use-cases/          folder

packages/             box     (declares uml-domain)
  billing/            box     (declares uml-domain)
  notes/              folder  (declares nothing)
  handbook/           book    (declares okf -- nested OKF bundle)
```

## Design

### The glyph is a property of the profile

Add a `folder_icon: &'static str` field to `ProfileDef`
(`crates/waml/src/profile.rs`):

- the shipped `okf` profile -> `"book"`
- the `uml` extension's `uml-domain` profile -> `"box"`

Core stays profile-agnostic: `folder_row` asks a declared profile for its
`folder_icon` rather than knowing package kinds itself. The `uml` extension keeps
ownership of "uml-domain draws a box" because it owns the `uml-domain` `ProfileDef`
in `uml_profiles()`. The editor icon table already carries `book`, `box`, and
`folder` (`crates/waml-editor/src/extension_editor.rs`,
`crates/waml-editor/src/icons.rs`), so no new icon assets are needed.

### Seam 1 — `RootView::folder_row`

`crates/waml/src/view/root.rs`. Replace the unconditional `FOLDER_ROW_ICON` stamp
with a lookup on the child's **own declared** profile:

- `index(child).profile` is `Some(p)` -> stamp `profile(p).folder_icon`
  (`"book"` for `okf`, `"box"` for `uml-domain`).
- `index(child).profile` is `None` -> stamp **nothing**. `resolve_icon` degrades an
  un-stamped `Folder` row to `Icon::Folder`
  (`crates/waml-editor/src/extension_editor.rs`), so plain folders draw the plain
  folder glyph with no explicit stamp.

An unknown/foreign profile name resolves no `ProfileDef`; treat it as "no
folder_icon" and degrade to the plain folder glyph (it is not a shipped root kind).

### Seam 2 — the bundle-root node

The one directory in a surface that no parent listing produced — the tree's root
node, and any surface that shows a directory it did not receive as a projected row.
Keyed on `parent.is_none()`. Stamp its **own declared** profile's `folder_icon`,
defaulting to `"book"` (the `okf` glyph) when it declares none. This replaces the
job the `FOLDER_ROW_ICON` constant did for the root node. The undeclared-defaults-to-
book rule is the *only* place an undeclared directory books; every other undeclared
directory is interior and degrades to the plain folder glyph.

### Seam 3 — retire the `UmlView` icon stamp

`crates/waml/src/view/uml.rs`. `folder_row` now stamps `"box"` on declared
`uml-domain` folders directly, independent of any view chain, so the `UmlView`
decorator's icon override is redundant and no longer the mechanism boxes depend on.

- Remove the `UmlView` projection stage and its `uml` middleware registration.
- Drop `uml-domain`'s `default_view` to `None` (`uml_profiles()`), matching `okf`:
  a `uml-domain` folder with no `view:` now resolves the root-only chain, and its
  box comes from the folder glyph, not a stage.
- `UmlEditorExtension` **stays** — it still contributes `("box", Icon::Box)` to the
  icon table, which `folder_row` now relies on.

`UmlView` is a pure icon decorator today (its `resolve` declines, `apply`/`surface`
forward, it mints no rows), so removing it changes nothing but the icon path.

## Testing

Existing tests that invert or lose their premise:

- `the_packages_fixture_draws_a_box_for_its_declared_package`
  (`crates/waml-editor/src/tree.rs`) — `Notes` expectation Book -> **Folder**;
  `Billing` stays Box.
- `a_declared_package_under_a_chainless_parent_draws_the_plain_folder_glyph`
  (`crates/waml-editor/src/tree.rs`) — **inverts**: the declared package now draws a
  **box** with no `uml` stage in the parent chain. Rename and rewrite the assertion;
  this is the "no boxes at all" regression being fixed.
- `tree_row_icon_matches_the_folder_row_icon_for_the_same_directory`
  (`crates/waml-editor/src/tree.rs`) — `Docs` expectation Book -> **Folder**; `Pkg`
  stays Box (now independent of `view: uml`). Keep the tree==folder-view equality.
- `the_root_node_draws_the_same_glyph_as_its_directory_children`
  (`crates/waml-editor/src/tree.rs`) — **premise dies** (root=book, plain children=
  folder no longer match). Reframe to: the root node draws the OKF bundle-root glyph
  (`book`) for an undeclared top, and its declared-profile glyph otherwise.
- `stamps_box_on_the_uml_domain_child_only` (`crates/waml/src/view/uml.rs`) — the
  box now comes from `folder_row`; relocate this assertion to a `root.rs` /
  folder-view test and delete the `UmlView`-specific test with the stage.

New tests to add:

- `folder_row` stamps `book` for a child declaring `profile: okf`, `box` for one
  declaring `profile: uml-domain`, and nothing (degrades to `Folder`) for a child
  declaring no profile.
- The bundle-root node books for an undeclared top and boxes for a top declaring
  `uml-domain` (the `packages` fixture root).
- Cross-surface: tree row icon == folder-view row icon for the same directory across
  all three glyphs.

## Non-goals

- No new icon assets, no DSL changes, no doc-content edits. `docs/waml` renders one
  book at the top and plain folders throughout with no content change.
- No resolved/inherited-profile glyph rules. Declaration alone drives the glyph; a
  plain folder under a package is a folder, not a box.
- No general "profile registry contributes arbitrary glyphs" surface beyond the
  single `folder_icon` field the two shipped profiles need.
