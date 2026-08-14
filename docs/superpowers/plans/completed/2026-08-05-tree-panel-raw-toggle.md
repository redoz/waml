# Tree-panel projected/raw toggle — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the inert per-folder "View raw" affordance with one session-wide Projected/Raw switch that both the tree panel and every open folder tab obey, and make the tree panel a real projection of the folder-view middleware chain instead of a raw OKF member listing.

**Architecture:** A new makepad-free module `crates/waml-editor/src/folder_projection.rs` owns the single middleware registry, the `ViewMode` enum, and one `project_rows` function. `FolderView` (the folder surface) and `build_tree` (the tree seam) both call it, so the two surfaces cannot disagree about what a directory contains. `TreeNode.key` becomes a `waml::view::row::RowId`, so selection and expansion survive the chain re-run a mode flip triggers. `DocumentHost` gains an explicit view-replace command so an already-open folder tab can be re-run in place — the capability whose absence made "View raw" inert. The mode lives in memory on `App` and is never persisted.

**Tech Stack:** Rust (workspace crates `waml`, `waml-editor`), makepad (fork pin, `makepad-widgets`), the `script_mod!` widget-registration DSL, Python 3 for `scripts/gen-icon.py`.

## Global Constraints

- **The implementer has no window and cannot verify anything visually.** Every visual check in this plan is written as DEFERRED and collected in the table at the foot. A task's own proof is always a headless test.
- **The full gate, run for every task, on its own:**
  ```
  cargo fmt --all --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  cd editors/vscode && pnpm build && pnpm test && pnpm lint   # build FIRST
  ```
- **Known baseline failure:** `-p waml-syntax --test properties` has one failing proptest on `origin/main` already. It is pre-existing and NOT caused by this work. Do not attempt to fix it; do not let it block a commit.
- **`clippy -D warnings` promotes `dead_code` to a hard error.** A half-landed type with no consumer fails the gate. Every `#[allow(dead_code)]` must name a concrete unlanded consumer, and must sit on the specific item — never on the enclosing struct or impl. The commit that lands the named consumer removes the allow in the same commit.
- **Headless-crate boundary:** `waml` and `waml-syntax` must not depend on the editor, on makepad, or on a window. Nothing in this plan adds an editor dependency to `waml`; the only `waml` change is widening one visibility.
- **Hiding is presentational and NEVER a permission boundary.** No code and no comment written by this plan may imply a hidden file is protected, restricted, or secured.
- **Commit messages:** conventional-commit subject plus a body explaining WHY. **No Claude co-author trailer** — the user considers it advertising.
- **Mode is not persisted.** `.waml/settings.json` is untouched by this plan. Every launch starts Projected.
- Work from `origin/main` at `7102c279` or later.

## File Structure

| File | Change | Responsibility after this plan |
|---|---|---|
| `crates/waml-editor/src/folder_projection.rs` | **create** (Task 4) | `ViewMode`, `core_registry()`, `chain_for()`, `project_rows()`. Makepad-free. The one place a folder's rows come from. |
| `crates/waml-editor/src/folder_view.rs` | modify | The folder surface's `DocView`. Loses `build_raw`, the `raw` field, and its private `run`/`core_registry`; delegates to `folder_projection`. |
| `crates/waml-editor/src/folder_list.rs` | modify | The folder-surface widget. Loses `raw_link`, `raw_banner`, `set_raw`, `raw_requested`, `RawRequested`. |
| `crates/waml-editor/src/folder_documents.rs` | modify | Loses `open_raw`; `open` gains a `ViewMode`. |
| `crates/waml-editor/src/documents.rs` | modify | Loses `open_folder_raw`; `open_folder` gains a `ViewMode`. |
| `crates/waml-editor/src/navigation.rs` | modify | Loses `NavigationTarget::DirectoryRaw`. |
| `crates/waml-editor/src/app/navigation.rs` | modify | Loses the `DirectoryRaw` route; passes the app's `ViewMode`. |
| `crates/waml-editor/src/document_host.rs` | modify | Gains `DocumentCommand::ReopenInPlace` — the explicit view-replace path. |
| `crates/waml-editor/src/tree.rs` | modify | `build_tree` becomes a projection; `TreeNode.key` becomes a `RowId`. |
| `crates/waml-editor/src/tree_panel.rs` | modify | Gains the toggle `IconButton`; keys its maps on `RowId::key_string()`. |
| `crates/waml-editor/src/nav.rs` | modify | Follows the `TreeNode` shape change. |
| `crates/waml-editor/src/icons.rs` | modify | Four new catalog glyphs. |
| `crates/waml-editor/src/icons_overlay.rs` | modify | Four new `ICON_GROUPS` rows; `UNWIRED_BUT_LISTED` churn. |
| `crates/waml-editor/src/app.rs` | modify | Holds `view_mode: ViewMode` in memory. |
| `crates/waml/src/view/root.rs` | modify | `ROOT_VIEW_OWNER` widens from `pub(crate)` to `pub`. |
| `resources/icons/{square-code,square-library,book,box}.svg` | **create** | Lucide sources for `scripts/gen-icon.py`. |

---

### Task 1: Catalogue the `book` and `box` glyphs

Two glyphs with no call site, catalogued ahead of one — exactly the existing `FileCodeCorner` / `Plus` precedent. Done first because it is independent of every other task and proves the six-list catalog ritual before the toggle's own glyphs depend on it.

**Files:**
- Create: `resources/icons/book.svg`
- Create: `resources/icons/box.svg`
- Modify: `crates/waml-editor/src/icons.rs` (six lists + one count)
- Modify: `crates/waml-editor/src/icons_overlay.rs` (`ICON_GROUPS`, `UNWIRED_BUT_LISTED` ~line 291)

**Interfaces:**
- Produces: `Icon::Book`, `Icon::Box`, `IconSet::book`, `IconSet::r#box`, DSL names `mod.draw.IconBook` / `mod.draw.IconBox`, labels `"book"` / `"box"`.

- [ ] **Step 1: Write the two Lucide sources**

`resources/icons/book.svg`:

```xml
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
  <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
</svg>
```

`resources/icons/box.svg`:

```xml
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
  <path d="m3.3 7 8.7 5 8.7-5" />
  <path d="M12 22V12" />
</svg>
```

- [ ] **Step 2: Generate each glyph body, one at a time**

`scripts/gen-all-icons.py` is STALE — **never run it**. Run the per-glyph generator twice:

```bash
python scripts/gen-icon.py resources/icons/book.svg
python scripts/gen-icon.py resources/icons/box.svg
```

Each prints a `mod.draw.Icon<Name> = mod.draw.DrawColor{ ... }` DSL body. Paste each verbatim into the `script_mod!` block of `crates/waml-editor/src/icons.rs`, appended after the last existing `mod.draw.Icon*` shader declaration, in the order `IconBook` then `IconBox`.

- [ ] **Step 3: Wire both glyphs into all six catalog lists, in the same order, appended at the end**

The catalog has six positional lists and one count. Order must be identical in every one. Append `Book` then `Box` after the current last entry (`Plus`) in each:

1. The `IconSet` DSL block (`icons.rs` ~line 3966, after `plus: ...`):
```rust
        book: mod.draw.IconBook{ color: atlas.accent }
        r#box: mod.draw.IconBox{ color: atlas.accent }
```
Note: `box` is a reserved word; the field is `r#box` in Rust. If the script DSL rejects `r#box` as a field name, use the plain identifier `box` in the DSL block and `r#box` only on the Rust side — the DSL name must match the Rust field's *snake_case* spelling that the `IconSet` drift test derives (see Step 5; run the test and follow what it reports).

2. The `IconSet` struct fields (`icons.rs`, after `pub plus: DrawColor,`):
```rust
    #[live]
    pub book: DrawColor,
    #[live]
    pub r#box: DrawColor,
```

3. The `Icon` enum (`icons.rs` ~line 4604, after `Plus,`):
```rust
    Book,
    Box,
```

4. `IconSet::get`'s match (`icons.rs` ~line 4341, after the `Icon::Plus` arm):
```rust
            Icon::Book => &mut self.book,
            Icon::Box => &mut self.r#box,
```

5. `Icon::ALL` (`icons.rs` ~line 4610), after `Icon::Plus,`:
```rust
        Icon::Book,
        Icon::Box,
```
and bump the array length in the same edit: `pub const ALL: [Icon; 121]` becomes `pub const ALL: [Icon; 123]`.

6. `Icon::label`'s match (`icons.rs` ~line 4736), after the `Icon::Plus` arm:
```rust
            Icon::Book => "book",
            Icon::Box => "box",
```

- [ ] **Step 4: Update the count assertion**

In `icons.rs`'s test module (~line 4869):

```rust
        assert_eq!(Icon::ALL.len(), 123);
```

- [ ] **Step 5: Add the overlay rows and the unwired entries**

In `crates/waml-editor/src/icons_overlay.rs`, append to the trailing "CATALOG ONLY" group in `ICON_GROUPS` (if no such group exists under that exact title, add the two rows to the group whose title reads closest to catalog-only and keep them adjacent):

```rust
            ie!(Book, "Catalogued ahead of a call site"),
            ie!(Box, "Catalogued ahead of a call site"),
```

And in `UNWIRED_BUT_LISTED` (~line 291), after `Icon::FileBraces,`:

```rust
    // Catalogued ahead of a call site, same as FileCodeCorner / Plus above.
    Icon::Book,
    Icon::Box,
```

- [ ] **Step 6: Run the three catalog guards plus the full gate**

Run: `cargo test -p waml-editor icons`
Expected: PASS — including `every_table_glyph_is_unique`, `unwired_catalog_glyphs_are_still_listed`, the enum/field/DSL/`get` drift tests, and the count assertion.

Then the full gate (see Global Constraints).

Visual verification of how the two glyphs actually draw is DEFERRED to the human (row V1 in the deferred table).

- [ ] **Step 7: Commit**

```bash
git add resources/icons/book.svg resources/icons/box.svg \
        crates/waml-editor/src/icons.rs crates/waml-editor/src/icons_overlay.rs
git commit -m "feat(icons): catalogue the book and box glyphs

The catalog is add-only and glyphs are pruned deliberately, so a glyph
lands in the reference before its call site rather than after. These two
have no consumer yet and join UNWIRED_BUT_LISTED next to the existing
FileCodeCorner / Plus precedent."
```

---

### Task 2: Catalogue the `square-code` and `square-library` glyphs

The toggle's two state glyphs. They land here with no call site (and therefore in `UNWIRED_BUT_LISTED`) so the catalog work is separated from the widget work; Task 10 removes them from that list in the same commit that gives them a call site.

**Files:**
- Create: `resources/icons/square-code.svg`
- Create: `resources/icons/square-library.svg`
- Modify: `crates/waml-editor/src/icons.rs`
- Modify: `crates/waml-editor/src/icons_overlay.rs`

**Interfaces:**
- Consumes: the six-list catalog shape established in Task 1; `Icon::ALL` is `[Icon; 123]` on entry.
- Produces: `Icon::SquareCode`, `Icon::SquareLibrary`, labels `"square-code"` / `"square-library"`. Task 9 sets these on the toggle button.

- [ ] **Step 1: Write the two Lucide sources**

`resources/icons/square-code.svg`:

```xml
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="m10 9-3 3 3 3" />
  <path d="m14 15 3-3-3-3" />
  <rect x="3" y="3" width="18" height="18" rx="2" />
</svg>
```

`resources/icons/square-library.svg`:

```xml
<svg
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <path d="M7 7v10" />
  <path d="M11 7v10" />
  <path d="m15 7 2 10" />
  <rect x="3" y="3" width="18" height="18" rx="2" />
</svg>
```

- [ ] **Step 2: Generate both glyph bodies**

```bash
python scripts/gen-icon.py resources/icons/square-code.svg
python scripts/gen-icon.py resources/icons/square-library.svg
```

Paste each printed `mod.draw.IconSquareCode = ...` / `mod.draw.IconSquareLibrary = ...` body verbatim into `icons.rs`'s `script_mod!` block, after `IconBox` from Task 1.

- [ ] **Step 3: Wire both into all six lists, appended after `Box`**

DSL block:
```rust
        square_code: mod.draw.IconSquareCode{ color: atlas.accent }
        square_library: mod.draw.IconSquareLibrary{ color: atlas.accent }
```

Struct fields:
```rust
    #[live]
    pub square_code: DrawColor,
    #[live]
    pub square_library: DrawColor,
```

Enum:
```rust
    SquareCode,
    SquareLibrary,
```

`IconSet::get`:
```rust
            Icon::SquareCode => &mut self.square_code,
            Icon::SquareLibrary => &mut self.square_library,
```

`Icon::ALL` — append the two entries and bump the length to `[Icon; 125]`:
```rust
        Icon::SquareCode,
        Icon::SquareLibrary,
```

`Icon::label`:
```rust
            Icon::SquareCode => "square-code",
            Icon::SquareLibrary => "square-library",
```

- [ ] **Step 4: Update the count assertion**

```rust
        assert_eq!(Icon::ALL.len(), 125);
```

- [ ] **Step 5: Add overlay rows and the temporary unwired entries**

In `ICON_GROUPS`, add to the `"TREE PANEL / DOCUMENT TABS"` group (this is where their call site will be):

```rust
            ie!(SquareLibrary, "Tree/folder view is projected (chain running)"),
            ie!(SquareCode, "Tree/folder view is raw (chain bypassed)"),
```

In `UNWIRED_BUT_LISTED`, add — with a comment that names the task removing them, because these two are temporary, unlike Task 1's:

```rust
    // Removed from this list by the tree-panel projected/raw toggle, which is
    // their call site. Temporary: they are catalogued a commit ahead of it.
    Icon::SquareCode,
    Icon::SquareLibrary,
```

- [ ] **Step 6: Run the guards and the full gate**

Run: `cargo test -p waml-editor icons`
Expected: PASS.

Then the full gate. Visual verification of the two glyphs is DEFERRED (row V1).

- [ ] **Step 7: Commit**

```bash
git add resources/icons/square-code.svg resources/icons/square-library.svg \
        crates/waml-editor/src/icons.rs crates/waml-editor/src/icons_overlay.rs
git commit -m "feat(icons): catalogue square-code and square-library

The two state glyphs of the coming tree-panel projected/raw toggle. They
land a commit ahead of their call site so the six-list catalog ritual is
one reviewable change and the widget work is another; the toggle's own
commit takes them back out of UNWIRED_BUT_LISTED."
```

---

### Task 3: Delete the per-folder "View raw" affordance and the `DirectoryRaw` route

Pure deletion. The affordance never worked: `folder_documents::open_raw` reuses `folder_document_tab_id(directory)`, and `DocumentHost::apply_command` (`document_host.rs:71`) inserts the incoming view only when the tab is not already open, so opening raw from a folder's own tab built the raw `FolderView` and discarded it. Removing the affordance removes the defect; Task 5 lands the missing capability and Task 4 restores raw as a mode.

Delete, do not deprecate. Keeping two independent raw controls gives four combinations and a tab that can disagree with the tree.

**Files:**
- Modify: `crates/waml-editor/src/folder_list.rs` (DSL `raw_banner` ~line 118 and `raw_link` ~line 142; `FolderListViewAction::RawRequested`; the `raw_link_area` hit block ~line 367; `raw_requested`; `set_raw`; both `WidgetRef` forwarders ~line 731)
- Modify: `crates/waml-editor/src/folder_view.rs` (the `raw` field, `build_raw`, the `set_raw` call in `sync`, the `raw_requested` branch in `handle`, the `build_raw_bypasses_the_declared_chain_and_its_diagnostics` test)
- Modify: `crates/waml-editor/src/folder_documents.rs` (`open_raw`)
- Modify: `crates/waml-editor/src/documents.rs` (`open_folder_raw` ~line 112)
- Modify: `crates/waml-editor/src/navigation.rs` (`NavigationTarget::DirectoryRaw`, ~line 22)
- Modify: `crates/waml-editor/src/app/navigation.rs` (the `DirectoryRaw` arm, ~line 329)
- Modify: `crates/waml-editor/src/tree_panel.rs` (the `DirectoryRaw` arm of `reveal_path`'s match, ~line 589)

**Interfaces:**
- Produces: `NavigationTarget` with exactly three variants (`Document`, `Directory`, `ExternalUrl`). `FolderView` with no `raw` field. `waml::view::chain::Chain::raw()` keeps existing in `waml` with no editor consumer for one commit — that is fine, `waml` is a library crate and its public API is not subject to `dead_code`.

- [ ] **Step 1: Delete the widget half**

In `folder_list.rs`:
- Delete the `raw_banner := View { ... }` block and the `raw_link := View { ... }` block from the `script_mod!` DSL, including their preceding comments.
- Delete `RawRequested` from `enum FolderListViewAction` and its arm from every exhaustive match over that enum (there is one in `row_opened`, ~line 634 — remove the `| FolderListViewAction::RawRequested` alternative from its `None` arm).
- Delete the `raw_link_area` block at the head of `Widget::handle_event` (~line 366) in its entirety, including the `let raw_link_area = ...` binding and the whole `match event.hits(cx, raw_link_area) { ... }`.
- Delete `pub fn raw_requested` and `pub fn set_raw` from the inherent impl and from the `WidgetRef` extension impl (~lines 731 and 735).

- [ ] **Step 2: Delete the view-model half**

In `folder_view.rs`:
- Delete the `raw: bool` field from `struct FolderView` and its doc comment.
- Delete `pub fn build_raw` entirely.
- In `FolderView::build`, drop `raw: false` from the struct literal.
- In `sync`, delete `body.folder_list().set_raw(cx, self.raw);`.
- In `handle`, delete the whole `} else if !self.raw && body.folder_list().raw_requested(actions) {` branch and its body, so the chain reads `if let Some(index) = body.folder_list().row_opened(actions) { ... } else if let Some(index) = body.folder_list().enter_pressed(actions) {`.
- Delete the test `build_raw_bypasses_the_declared_chain_and_its_diagnostics`. Task 4 replaces its coverage with a mode-based equivalent.

- [ ] **Step 3: Delete the provider and route**

- `folder_documents.rs`: delete `pub fn open_raw` and its doc comment.
- `documents.rs`: delete `pub fn open_folder_raw` and its doc comment (~line 110).
- `navigation.rs`: delete the `DirectoryRaw { address: String },` variant and its doc comment from `enum NavigationTarget`.
- `app/navigation.rs`: delete the whole `crate::navigation::NavigationTarget::DirectoryRaw { address } => { ... }` arm.
- `tree_panel.rs` (~line 589): collapse the two-pattern arm to one:

```rust
            NavigationTarget::Directory { address } => {
                node.is_directory && node.key == address.as_str()
            }
```

- [ ] **Step 4: Compile and let the compiler find the rest**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. If it names any remaining `DirectoryRaw` / `open_raw` / `raw_requested` / `set_raw` reference (including in `app/tests/`), delete that reference too; there must be zero left. Confirm with:

```bash
git grep -n "DirectoryRaw\|open_raw\|build_raw\|raw_requested\|raw_link\|raw_banner" -- crates editors
```
Expected: no output.

- [ ] **Step 5: Run the full gate**

Then commit.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src
git commit -m "refactor(folder-view): delete the inert per-folder View raw affordance

open_raw deliberately shared folder_document_tab_id, and apply_command
only inserts an incoming view when the tab is not already open, so
clicking View raw from a folder's own tab built the raw FolderView and
threw it away. Verified in a window: the affordance drew, the click
landed, nothing changed.

A session-wide switch replaces it. Keeping both would give four
combinations and a tab that can disagree with the tree, so the
affordance, its banner, open_raw, and the DirectoryRaw route all go
rather than being worked around."
```

---

### Task 4: `folder_projection.rs` — one `ViewMode`, one registry, one row source

The file-heavy half of the tree change, landed with only `FolderView` as its consumer so the tree rewiring is a separate, reviewable commit. Extracting `project_rows` is what makes the spec's "tree children equal the folder view's rows" testable rather than hoped for: after this task there is exactly one function that answers "what rows does this directory have".

**Files:**
- Create: `crates/waml-editor/src/folder_projection.rs`
- Modify: `crates/waml-editor/src/lib.rs` (add `pub mod folder_projection;` next to `pub mod folder_view;` — match the surrounding declaration style and alphabetical position)
- Modify: `crates/waml-editor/src/folder_view.rs` (delete `core_registry` and `FolderView::run`; `build` takes a `ViewMode`)
- Modify: `crates/waml-editor/src/folder_documents.rs` (`open` takes a `ViewMode`)
- Modify: `crates/waml-editor/src/documents.rs` (`open_folder` takes a `ViewMode`)
- Modify: `crates/waml-editor/src/app.rs` (new `#[rust] view_mode: ViewMode` field)
- Modify: `crates/waml-editor/src/app/navigation.rs` (pass `self.view_mode`)
- Modify: `crates/waml-editor/src/tree.rs` (its one `crate::folder_view::core_registry()` call moves to `crate::folder_projection::core_registry()`)

**Interfaces:**
- Consumes: `waml::view::chain::{Chain, ChainLimits, MiddlewareRegistry}`, `waml::view::projection::ProjectionCtx`, `waml::view::row::Row`, `waml::diagnostic::Diagnostic`, `waml::analysis::OkfAnalysis`.
- Produces, all `pub` in `crate::folder_projection`:
  - `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)] pub enum ViewMode { #[default] Projected, Raw }`
  - `pub fn core_registry() -> MiddlewareRegistry`
  - `pub fn chain_for(analysis: &waml::analysis::OkfAnalysis, directory: &str, mode: ViewMode) -> (Chain, Vec<Diagnostic>)`
  - `pub fn project_rows(analysis: &waml::analysis::OkfAnalysis, directory: &str, mode: ViewMode, limits: ChainLimits) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)>`
  - `FolderView::build(analysis, directory, limits, mode)` — a fourth parameter, appended.
  - `folder_documents::open(analysis, directory, limits, mode)`, `documents::open_folder(okf, directory, limits, mode)` — likewise.
  - `App.view_mode` — read by Task 9/10.

- [ ] **Step 1: Write the failing test**

Create `crates/waml-editor/src/folder_projection.rs` with only its test module first, so the test names the API before it exists:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    fn analysis(
        pairs: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> waml::analysis::PreparedCandidate {
        let source = SourceBundle::try_from_pairs(pairs).unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    fn hidden_bundle() -> waml::analysis::PreparedCandidate {
        analysis([
            (
                "index.md",
                "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n* [Orders](orders.md)\n* [References](references/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("references/index.md", "# References\n"),
        ])
    }

    #[test]
    fn projected_runs_the_declared_chain_and_raw_bypasses_it() {
        let prepared = hidden_bundle();
        let limits = ChainLimits::default();

        let (_, projected, diagnostics) =
            project_rows(prepared.okf(), "/", ViewMode::Projected, limits).unwrap();
        assert!(
            diagnostics.is_empty(),
            "a correctly authored `view: hide` must not be diagnosed: {diagnostics:?}"
        );
        assert_eq!(
            projected.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
            vec!["Orders"],
        );

        let (_, raw, raw_diagnostics) =
            project_rows(prepared.okf(), "/", ViewMode::Raw, limits).unwrap();
        assert!(raw_diagnostics.is_empty(), "raw never builds the declared chain");
        assert_eq!(
            raw.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
            vec!["Orders", "References"],
            "raw is presentational reachability, not a permission decision",
        );
    }

    #[test]
    fn raw_never_diagnoses_a_declared_chain_it_does_not_build() {
        let prepared = analysis([
            ("index.md", "---\nview: nonexistent\n---\n# Root\n\n* [Orders](orders.md)\n"),
            ("orders.md", "# Orders\n"),
        ]);
        let limits = ChainLimits::default();

        let (_, _, declared) =
            project_rows(prepared.okf(), "/", ViewMode::Projected, limits).unwrap();
        assert!(!declared.is_empty(), "an unknown middleware name diagnoses");

        let (_, _, raw) = project_rows(prepared.okf(), "/", ViewMode::Raw, limits).unwrap();
        assert!(raw.is_empty());
    }

    #[test]
    fn a_missing_directory_yields_none_rather_than_panicking() {
        let prepared = hidden_bundle();
        assert!(
            project_rows(prepared.okf(), "/missing", ViewMode::Projected, ChainLimits::default())
                .is_none()
        );
    }

    #[test]
    fn raw_mode_owns_every_row_through_the_root_view() {
        let prepared = hidden_bundle();
        let (_, rows, _) =
            project_rows(prepared.okf(), "/", ViewMode::Raw, ChainLimits::default()).unwrap();
        assert!(
            rows.iter()
                .all(|row| row.id.owner.as_str() == waml::view::root::ROOT_VIEW_OWNER),
            "in Raw the chain is [index], so RootView owns every row",
        );
    }
}
```

Note: the last test needs `ROOT_VIEW_OWNER` to be public. Task 7 does that widening; if it has not landed yet, **omit `raw_mode_owns_every_row_through_the_root_view` from this task** and add it in Task 7 instead. Do not widen the visibility here — a `waml` change with no consumer would be a second concern in this commit.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor folder_projection`
Expected: FAIL to compile — `cannot find function project_rows`, `cannot find type ViewMode`.

- [ ] **Step 3: Write the module**

Prepend to `crates/waml-editor/src/folder_projection.rs`:

```rust
//! Where a folder's rows come from — for BOTH surfaces that show them.
//!
//! The folder surface (`folder_view.rs`) and the tree seam (`tree.rs`) run
//! the same chain, against the same registry, in the same mode. Two row
//! sources that disagree are invisible: the tree lists a child the folder
//! view does not, or marks a folder degraded that opens clean, and the gate
//! is green either way.
//!
//! Deliberately makepad-free, like `tree.rs`, so both consumers can depend on
//! it and its behaviour is unit-testable with no window.

use waml::diagnostic::Diagnostic;
use waml::okf::Directory;
use waml::view::chain::{Chain, ChainLimits, MiddlewareRegistry};
use waml::view::projection::ProjectionCtx;
use waml::view::row::Row;

/// The session-wide projected/raw switch, held in memory on `App` and read by
/// every surface that lists a folder's contents.
///
/// NOT persisted, and `.waml/settings.json` never sees it: raw is a deliberate
/// act, not a preference, so every launch starts `Projected` and an author's
/// declared `view:` is what a reader sees unless they ask otherwise.
///
/// `Raw` is presentational reachability and performs no access check. Nothing
/// in waml treats a row a chain declined to emit as protected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Projected,
    Raw,
}

/// The middleware registry every folder-listing path in the editor resolves
/// against: the core extension's `index` and `hide`.
///
/// One function because two construction sites that disagree are invisible --
/// a folder resolves fine in one and reports `unknown view middleware` in the
/// other, with the gate green either way. Cheap enough to build per call;
/// nothing here caches across frames.
pub fn core_registry() -> MiddlewareRegistry {
    MiddlewareRegistry::from_extensions(&[&waml::extension::CoreExt])
        .expect("the core extension registers a conflict-free name table")
}

/// The chain `directory` runs under `mode`, plus any build-level diagnostics
/// (unknown middleware name, bad params) the declared chain produced.
///
/// `Raw` pins the chain to `Chain::raw()` -- the identity listing -- and never
/// builds the declared chain at all, which is why it never diagnoses one.
pub fn chain_for(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mode: ViewMode,
) -> (Chain, Vec<Diagnostic>) {
    match mode {
        ViewMode::Projected => analysis
            .bundle
            .resolved_view(directory, &core_registry()),
        ViewMode::Raw => (Chain::raw(), Vec::new()),
    }
}

/// Run `directory`'s chain for `mode` and hand back the chain itself, its
/// rows, and every diagnostic (build-level and run-level) it produced.
///
/// The chain comes back with the rows because a later gesture must call
/// `Chain::apply` against the exact stages that minted the `RowId`s it
/// addresses; rebuilding from `directory` alone could resolve a different
/// stage set if the bundle changed underneath.
///
/// `None` means `directory` is not in the bundle at all.
pub fn project_rows(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mode: ViewMode,
    limits: ChainLimits,
) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)> {
    let dir: Directory = analysis.bundle.directory(directory)?.clone();
    let (chain, mut diagnostics) = chain_for(analysis, directory, mode);
    // A middleware's params ARE the folder's own index frontmatter -- `hide`
    // reads its globs from here, and `Chain::build` validated them against
    // this same map. Passing an empty one makes every param-taking stage fail
    // its own declaration check and trip the whole-chain fallback.
    let params = analysis
        .bundle
        .index(directory)
        .map(|index| index.extra.clone())
        .unwrap_or_default();
    let descend = |_: &Directory| Chain::default();
    let ctx = ProjectionCtx {
        dir: &dir,
        bundle: &analysis.bundle,
        params: &params,
        descend: &descend,
    };
    let outcome = chain.run(&ctx, limits);
    diagnostics.extend(outcome.diagnostics);
    Some((chain, outcome.rows, diagnostics))
}
```

Add `pub mod folder_projection;` to `crates/waml-editor/src/lib.rs`.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p waml-editor folder_projection`
Expected: PASS.

- [ ] **Step 5: Rewire `FolderView` onto it**

In `folder_view.rs`:
- Delete `pub fn core_registry()` and `fn run(...)` entirely.
- Rewrite `FolderView::build`:

```rust
    /// Resolve `directory`'s rows for `mode` and hold the chain that produced
    /// them. `Raw` bypasses the declared chain; `Projected` runs it.
    pub fn build(
        analysis: &waml::analysis::OkfAnalysis,
        directory: &str,
        limits: ChainLimits,
        mode: crate::folder_projection::ViewMode,
    ) -> Option<FolderView> {
        let (chain, rows, diagnostics) =
            crate::folder_projection::project_rows(analysis, directory, mode, limits)?;
        Some(FolderView {
            directory: directory.to_string(),
            rows,
            chain,
            diagnostics,
        })
    }
```
- Fix `folder_view.rs`'s own tests: every `FolderView::build(prepared.okf(), "/", ChainLimits::default())` gains a fourth argument `crate::folder_projection::ViewMode::Projected`. In the test `the_tree_and_the_folder_view_agree_on_whether_a_chain_degraded`, replace `core_registry()` with `crate::folder_projection::core_registry()`.

- [ ] **Step 6: Rewire the callers**

- `folder_documents.rs::open` gains `mode: crate::folder_projection::ViewMode` as its last parameter and forwards it to `FolderView::build`. Update its own test to pass `ViewMode::Projected`.
- `documents.rs::open_folder` gains the same last parameter and forwards it.
- `tree.rs`: change `let registry = crate::folder_view::core_registry();` to `let registry = crate::folder_projection::core_registry();`.
- `app.rs`: add the field, near `chain_limits` (~line 706):

```rust
    /// The session-wide projected/raw switch. In memory only -- NOT persisted,
    /// and `.waml/settings.json` never sees it, so every launch starts
    /// `Projected` and the author's declared `view:` is the default a reader
    /// gets. Read by both the tree seam and every folder tab, so the two can
    /// never disagree about what a directory contains.
    #[rust]
    view_mode: crate::folder_projection::ViewMode,
```
- `app/navigation.rs`: the `Directory` arm's `crate::documents::open_folder(...)` call gains `self.view_mode` as its last argument.

- [ ] **Step 7: Run the full gate**

`cargo clippy --workspace --all-targets -- -D warnings` will name every remaining call site with the wrong arity — fix each by passing the mode the caller already has (`self.view_mode` in `App`, `ViewMode::Projected` in tests).

Then the whole gate.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(folder-view): one row source for both folder surfaces

The folder surface and the tree seam each answered \"what rows does this
directory have\" separately, and the tree never ran the chain at all. Two
answers that disagree are invisible: the tree lists a child the folder
view does not, and the gate is green.

folder_projection::project_rows is now the single answer, parameterised
by a ViewMode the session owns rather than by a per-tab raw flag. Raw is
presentational reachability -- it performs no access check."
```

---

### Task 5: `DocumentHost::ReopenInPlace` — the explicit view-replace path

`apply_command` (`document_host.rs:71`) inserts the incoming view only when `already_open` is false. That is deliberate for ordinary navigation — re-opening a document tab must not blow away a markdown editor's scroll position or an in-flight edit — but it leaves no way to swap the view of a tab that is already open, which is exactly what a mode flip needs. Add the capability rather than loosening `Open`.

**Files:**
- Modify: `crates/waml-editor/src/document_host.rs` (`enum DocumentCommand`, `apply_command`)

**Interfaces:**
- Produces: `DocumentCommand::ReopenInPlace { document: OpenDocument }` — replaces the view registered for `document.tab_id` when that tab is open; a no-op when it is not. Consumed by Task 10.

- [ ] **Step 1: Write the failing test**

Add to `document_host.rs`'s test module. If the module has an existing helper that builds an `OpenDocument` with a stub `DocView`, reuse it verbatim rather than writing a second one; the sketch below assumes helpers `host()` and `doc(tab_id, marker)` in the shape the surrounding tests already use, and `view_identity(host, tab_id)` reading back `DocViewIdentity`.

```rust
    /// The capability whose absence made the old per-folder "View raw" inert:
    /// `Open` keeps the existing view whenever the tab id is already open, so
    /// re-navigating a folder tab built the new view and discarded it. A mode
    /// flip must swap the view of a tab that stays put, in place, without
    /// closing and reopening it (which would lose tab order and selection).
    #[test]
    fn reopen_in_place_replaces_the_view_of_an_already_open_tab() {
        let mut host = host();
        let first = doc(TAB, DocViewIdentity::Folder);
        let tab_id = first.tab_id;
        host.apply_command(DocumentCommand::Open { document: first, persistent: false });
        assert_eq!(host.tabs().len(), 1);

        let replacement = doc(TAB, DocViewIdentity::Source);
        host.apply_command(DocumentCommand::ReopenInPlace { document: replacement });

        assert_eq!(host.tabs().len(), 1, "no second tab for the same document");
        assert_eq!(host.tabs()[0].id, tab_id, "the tab keeps its identity");
        assert_eq!(
            view_identity(&host, tab_id),
            Some(DocViewIdentity::Source),
            "the view was replaced, not discarded",
        );
    }

    #[test]
    fn reopen_in_place_on_a_closed_tab_opens_nothing() {
        let mut host = host();
        host.apply_command(DocumentCommand::ReopenInPlace {
            document: doc(TAB, DocViewIdentity::Folder),
        });
        assert!(host.tabs().is_empty(), "ReopenInPlace never opens a tab");
    }
```

Adapt the helper names to what the file already provides. If `apply_command` is private and the surrounding tests drive the host through a public wrapper (`transition`), use that wrapper instead — do not widen visibility for a test.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p waml-editor document_host`
Expected: FAIL to compile — `no variant named ReopenInPlace`.

- [ ] **Step 3: Add the variant and its arm**

In `enum DocumentCommand`:

```rust
    /// Swap the view behind an ALREADY-OPEN tab, leaving tab order,
    /// selection, and the active tab untouched. A no-op when `document`'s tab
    /// is not open.
    ///
    /// Distinct from `Open`, which deliberately keeps the existing view when
    /// the tab id is already open: an ordinary re-navigation must not discard
    /// a markdown editor's scroll position or an in-flight edit. Only a caller
    /// that KNOWS the view must change -- a session-wide projected/raw flip --
    /// asks for this.
    ReopenInPlace { document: OpenDocument },
```

In `apply_command`'s match, after the `Open` arm:

```rust
            DocumentCommand::ReopenInPlace { document } => {
                let tab_id = document.tab_id;
                if self.tabs.tabs.iter().any(|tab| tab.id == tab_id) {
                    let (_tab, view) = document.into_tab(true);
                    self.views.insert(tab_id, view);
                }
            }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p waml-editor document_host`
Expected: PASS.

- [ ] **Step 5: Run the full gate, then commit**

```bash
git add crates/waml-editor/src/document_host.rs
git commit -m "feat(document-host): add an explicit view-replace command

apply_command inserts an incoming view only when the tab is not already
open, which is right for ordinary navigation -- re-navigating must not
discard a markdown editor's scroll or an in-flight edit -- but leaves no
way to swap the view of a tab that stays put. That gap is what made the
old per-folder View raw silently do nothing.

ReopenInPlace names the intent instead of loosening Open, so only a
caller that knows the view must change gets the replacing behaviour."
```

---

### Task 6: `build_tree` projects the chain

The tree stops listing OKF members directly: a directory's children become the chain's rows for that directory, in the chain's order, with the chain's labels. `TreeNode.key` deliberately stays a `String` here so this commit is about the row source alone; Task 7 changes the identity type and takes the ripple.

This closes the folder-view spec's own unmet checklist item — a `hide: ["**"]` folder showed no rows in its folder view and still listed every hidden child in the tree.

**Files:**
- Modify: `crates/waml-editor/src/tree.rs` (`build_tree` and its tests)
- Modify: `crates/waml-editor/src/nav.rs` (`view`, `packages` — pass the new args through)
- Modify: `crates/waml-editor/src/navigation.rs` (`breadcrumbs`' `build_tree` call, ~line 118)
- Modify: `crates/waml-editor/src/app/navigation.rs` (`refresh_nav` passes mode + limits into `nav::view` / `nav::packages`)

**Interfaces:**
- Consumes: `crate::folder_projection::{ViewMode, core_registry, project_rows}`, `waml::view::chain::ChainLimits`, `waml::view::row::{Row, RowTarget}`.
- Produces:
  - `build_tree(okf, uml_analysis, root_fallback, mode: ViewMode, limits: ChainLimits) -> ProjectTree`
  - `nav::view(okf, uml, state, mode, limits) -> NavView`
  - `nav::packages(okf, uml, mode, limits) -> Vec<PackageRow>`
  - `navigation::breadcrumbs(...)` keeps its signature and passes `ViewMode::Projected` with `ChainLimits::default()` internally — breadcrumbs describe the authored structure, not the current session mode. Say so in a comment.

- [ ] **Step 1: Write the failing tests**

Add to `tree.rs`'s test module:

```rust
    fn hidden() -> waml::analysis::PreparedCandidate {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\n* [Sales](sales/)\n",
            ),
            (
                "sales/index.md",
                "---\nview: hide\nhide: [\"**\"]\n---\n# Sales\n\n* [Order](./order.md)\n",
            ),
            ("sales/order.md", "# Order\n"),
        ])
        .unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    /// The folder-view spec's own checklist item that did not hold: an opaque
    /// folder showed no rows in its folder view and still listed every hidden
    /// child in the tree.
    #[test]
    fn an_opaque_folder_has_no_tree_children_projected_and_all_of_them_raw() {
        let prepared = hidden();
        let limits = waml::view::chain::ChainLimits::default();

        let projected = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            crate::folder_projection::ViewMode::Projected,
            limits,
        );
        let sales = &projected.roots[0].children[0];
        assert!(
            sales.children.is_empty(),
            "hide: [\"**\"] leaves nothing for the tree to list",
        );

        let raw = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            crate::folder_projection::ViewMode::Raw,
            limits,
        );
        let sales = &raw.roots[0].children[0];
        assert_eq!(
            sales.children.iter().map(|row| row.title.as_str()).collect::<Vec<_>>(),
            vec!["Order"],
            "raw bypasses the chain, so the row is reachable again",
        );
    }

    /// The tree and the folder surface must never disagree about what a
    /// directory contains -- they read the same projection now, so this is a
    /// regression fence, not an aspiration.
    #[test]
    fn tree_children_equal_the_folder_views_rows_row_for_row_in_both_modes() {
        let prepared = hidden();
        let limits = waml::view::chain::ChainLimits::default();
        for mode in [
            crate::folder_projection::ViewMode::Projected,
            crate::folder_projection::ViewMode::Raw,
        ] {
            let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", mode, limits);
            let sales = &tree.roots[0].children[0];
            let (_, rows, _) =
                crate::folder_projection::project_rows(prepared.okf(), "/sales", mode, limits)
                    .unwrap();
            assert_eq!(
                sales.children.iter().map(|node| node.title.as_str()).collect::<Vec<_>>(),
                rows.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
                "{mode:?}: tree children must be the chain's rows, in the chain's order",
            );
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor tree::`
Expected: FAIL to compile — `build_tree` takes 3 arguments, 5 supplied.

- [ ] **Step 3: Rewrite `build_tree`'s child derivation**

Replace the body of `directory_node` between `let mut children = Vec::new();` and the `Some(TreeNode { ... })` construction. The member walk, the `seen` set, and both fallback loops all go: rows come from the chain now, and ownership is total (a row no middleware claims is emitted by the root view with a real member href), so nothing can be orphaned.

```rust
        let mut children = Vec::new();
        // Children ARE the chain's rows for this directory, in the chain's
        // order, carrying the chain's labels -- not the OKF member list. The
        // tree and the folder surface therefore cannot disagree about what a
        // directory contains. `project_rows` returning None means the
        // directory left the bundle underneath us; an empty child list is the
        // honest answer, not a panic.
        let projected = crate::folder_projection::project_rows(okf, address.as_str(), mode, limits);
        for row in projected.iter().flat_map(|(_, rows, _)| rows.iter()) {
            match &row.target {
                waml::view::row::RowTarget::Folder(child_address) => {
                    let Some(child) = waml::okf::DirectoryAddress::parse(child_address) else {
                        continue;
                    };
                    if let Some(mut node) =
                        directory_node(okf, uml_analysis, &child, root_fallback, mode, limits)
                    {
                        // The chain owns the label; a middleware may relabel a
                        // folder row, and the tree must show what it said.
                        node.title = row.label.clone();
                        children.push(node);
                    }
                }
                waml::view::row::RowTarget::Concept(concept_id) => {
                    if let Some(mut node) = concept_node(concept_id) {
                        node.title = row.label.clone();
                        children.push(node);
                    }
                }
                // No file behind it, so nothing to open by concept id or
                // address. It still gets a row: dropping it would make the
                // tree disagree with the folder view about what is there.
                waml::view::row::RowTarget::Virtual => {
                    children.push(TreeNode {
                        key: row.id.path.as_str().to_string(),
                        title: row.label.clone(),
                        kind: NavCategory::OkfDocument,
                        presentation: DocumentPresentation {
                            icon: Icon::FileText,
                            accent: None,
                            category: NavCategory::OkfDocument,
                        },
                        is_directory: false,
                        openable: false,
                        concept_id: None,
                        can_edit_classifier: false,
                        can_delete_classifier: false,
                        view_degraded: false,
                        children: Vec::new(),
                    });
                }
            }
        }
```

Thread the two new parameters through both `build_tree` and its inner `directory_node`:

```rust
pub fn build_tree(
    okf: &waml::analysis::OkfAnalysis,
    uml_analysis: &waml::uml::Analysis,
    root_fallback: &str,
    mode: crate::folder_projection::ViewMode,
    limits: waml::view::chain::ChainLimits,
) -> ProjectTree {
```

and `fn directory_node(okf, uml_analysis, address, root_fallback, mode, limits) -> Option<TreeNode>`, forwarding at the recursive call and at the root call site.

Leave the `view_degraded` computation exactly as it is — it comes from `resolved_view` against `core_registry()` and must stay the same registry the folder surface uses, or the tree marks a folder degraded that opens fine. Update only the module path (`crate::folder_projection::core_registry()`), which Task 4 already did.

- [ ] **Step 4: Update the existing `tree.rs` tests**

Every existing `build_tree(...)` call in the test module gains `crate::folder_projection::ViewMode::Projected, waml::view::chain::ChainLimits::default()`. `navigator_uses_okf_directories_and_authored_index_order` should still pass unchanged in its assertions: an identity chain reproduces the plain listing row for row, including order, which is the folder-view spec's own guarantee. If it does not, that is a real regression in the chain — investigate before touching the assertion.

- [ ] **Step 5: Thread the parameters through the three callers**

- `nav.rs`: `pub fn view(okf, uml, state, mode, limits)` and `pub fn packages(okf, uml, mode, limits)`; forward to `build_tree`. Update `nav.rs`'s own tests.
- `navigation.rs::breadcrumbs` (~line 118): pass a fixed pair, with the reason:
```rust
    // Breadcrumbs describe the authored structure a reader navigated through,
    // so they read the declared chain regardless of the session's mode.
    let tree = build_tree(
        okf,
        uml_analysis,
        "Untitled",
        crate::folder_projection::ViewMode::Projected,
        waml::view::chain::ChainLimits::default(),
    );
```
- `app/navigation.rs::refresh_nav`: pass `self.view_mode, self.chain_limits` to both `crate::nav::view` and `crate::nav::packages`. Update `app/tests/mod.rs`'s `crate::nav::view(...)` call (~line 56) the same way.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-editor tree:: nav:: navigation::`
Expected: PASS.

- [ ] **Step 7: Run the full gate, then commit**

Visual verification that a hidden file disappears from the tree in Projected and reappears in Raw is DEFERRED (row V2).

```bash
git add crates/waml-editor/src
git commit -m "feat(tree): a directory's children are the chain's rows

build_tree listed real OKF members and borrowed the registry only to
compute the degraded marker, so a hide: [\"**\"] folder showed no rows in
its folder view and still listed every hidden child in the tree -- the
folder-view spec's own checklist item did not hold.

Occlusion-filtering the member list would have been cheaper and is
indistinguishable while index and hide are the only middleware, but it
diverges the moment a middleware reorders, relabels, or mints rows, and
then the two surfaces disagree with no way for a reader to tell which is
right. Full projection has no such failure mode."
```

---

### Task 7: `TreeNode.key` becomes a `RowId`

`RowId` is stable across a re-projection, so selection and expansion survive the chain re-run a mode flip triggers. A file address is not: a middleware that relabels or mints rows has no file to key on.

This is the wiring-heavy half of the tree change. The row source is already correct after Task 6, so this commit changes one type and follows the compiler.

**Files:**
- Modify: `crates/waml/src/view/root.rs` (widen `ROOT_VIEW_OWNER` to `pub`)
- Modify: `crates/waml-editor/src/tree.rs` (`TreeNode.key`, new `address` field, `key_string`)
- Modify: `crates/waml-editor/src/tree_panel.rs` (`id_to_key`, `chevron_rects`, `open_directories`, `selected_key`, `reveal_key`, `pending_scroll_key`, `reveal_path`, `row_navigation` call sites, test fixtures)
- Modify: `crates/waml-editor/src/nav.rs` (`find_node`, `scope_node`, `packages`)

**Interfaces:**
- Consumes: `waml::view::row::{RowId, RowPath, ViewId}`, `waml::view::root::ROOT_VIEW_OWNER`.
- Produces:
  - `TreeNode { pub key: RowId, pub address: Option<String>, pub concept_id: Option<String>, ... }` — `address` is `Some(directory address)` for a directory row, `None` otherwise. `concept_id` is unchanged.
  - `pub fn key_string(key: &RowId) -> String` in `tree.rs` — the flat string the panel's `HashMap<LiveId, String>` and `chevron_rects` key on. Format: `format!("{}\u{1}{}", key.owner, key.path)`. A `\u{1}` separator because neither a `ViewId` nor a `RowPath` segment can contain it, so the encoding is injective.

- [ ] **Step 1: Widen `ROOT_VIEW_OWNER`**

`crates/waml/src/view/root.rs:22`:

```rust
/// The reserved `ViewId` of the terminal stage. Public because a host that
/// pins a chain to `[index]` (the raw OKF layer) must be able to say so:
/// every row in such a chain is owned by this id.
pub const ROOT_VIEW_OWNER: &str = "index";
```

Confirm `pub mod root;` (or an equivalent re-export) makes `waml::view::root::ROOT_VIEW_OWNER` reachable from `waml-editor`; if `root` is `pub(crate)`, re-export the constant from `crates/waml/src/view/mod.rs` as `pub use root::ROOT_VIEW_OWNER;` rather than widening the whole module.

- [ ] **Step 2: Write the failing tests**

In `tree.rs`'s test module:

```rust
    /// In Raw the chain is [index], so the root view owns every row and
    /// RootView::resolve answers every path -- Raw is today's listing.
    #[test]
    fn raw_rows_are_owned_by_the_root_view() {
        let prepared = hidden();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            crate::folder_projection::ViewMode::Raw,
            waml::view::chain::ChainLimits::default(),
        );
        let sales = &tree.roots[0].children[0];
        assert!(
            sales
                .children
                .iter()
                .all(|node| node.key.owner.as_str() == waml::view::root::ROOT_VIEW_OWNER),
        );
    }

    /// A RowId minted by a middleware while Projected does not resolve in
    /// Raw, because its owner is not in the raw chain. Expansion or selection
    /// sitting on such a row falls back to the nearest resolvable prefix --
    /// at worst the folder. That is the existing Unresolved rule, not a new
    /// one, and it must NOT panic or vanish.
    #[test]
    fn a_row_id_whose_owner_is_absent_from_the_raw_chain_falls_back_to_a_prefix() {
        let prepared = hidden();
        let limits = waml::view::chain::ChainLimits::default();
        let (chain, _, _) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            crate::folder_projection::ViewMode::Raw,
            limits,
        )
        .unwrap();
        let dir = prepared.okf().bundle.directory("/sales").unwrap().clone();
        let params = prepared
            .okf()
            .bundle
            .index("/sales")
            .map(|index| index.extra.clone())
            .unwrap_or_default();
        let descend = |_: &waml::okf::Directory| waml::view::chain::Chain::default();
        let ctx = waml::view::projection::ProjectionCtx {
            dir: &dir,
            bundle: &prepared.okf().bundle,
            params: &params,
            descend: &descend,
        };
        let stranger = waml::view::row::RowId {
            owner: waml::view::row::ViewId::new("group-by-tag"),
            path: waml::view::row::RowPath::parse("synthesized/leaf").unwrap(),
        };
        let rows = chain.resolve(&ctx, &stranger);
        assert!(
            rows.is_ok(),
            "an unresolvable RowId falls back to the folder's own listing",
        );
    }

    /// The panel keys its maps on a flat string; the encoding must not let two
    /// distinct RowIds collide.
    #[test]
    fn key_string_is_injective_across_owner_and_path() {
        let a = waml::view::row::RowId {
            owner: waml::view::row::ViewId::new("a/b"),
            path: waml::view::row::RowPath::parse("c").unwrap(),
        };
        let b = waml::view::row::RowId {
            owner: waml::view::row::ViewId::new("a"),
            path: waml::view::row::RowPath::parse("b/c").unwrap(),
        };
        assert_ne!(key_string(&a), key_string(&b));
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-editor tree::`
Expected: FAIL to compile — `key_string` not found, `node.key.owner` on a `String`.

- [ ] **Step 4: Change the type and add the helper**

In `tree.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// The projected row's identity, NOT a file address. Stable across a
    /// re-projection, so selection and expansion survive the chain re-run a
    /// mode flip triggers; a file address is not, because a middleware may
    /// relabel or mint rows with no file behind them.
    pub key: waml::view::row::RowId,
    /// Directory rows only: the real OKF address this row expands into.
    /// `None` for concept and virtual rows.
    pub address: Option<String>,
    pub title: String,
    // ... the remaining fields are unchanged
}

/// The flat string the tree panel keys its `LiveId` maps and cached chevron
/// rects on. `\u{1}` separates the two halves: neither a `ViewId` nor a
/// `RowPath` segment can contain it, so distinct `RowId`s never collide.
pub fn key_string(key: &waml::view::row::RowId) -> String {
    format!("{}\u{1}{}", key.owner, key.path)
}
```

Build each `TreeNode`'s `key` from the row it came from (`row.id.clone()`), and set `address` from `RowTarget::Folder(address)`. The bundle root node, which no row emits, gets:

```rust
            key: waml::view::row::RowId {
                owner: waml::view::row::ViewId::new(waml::view::root::ROOT_VIEW_OWNER),
                path: waml::view::row::RowPath::parse("/")
                    .unwrap_or_else(|_| waml::view::row::RowPath::parse("root").expect("literal")),
            },
            address: Some(address.as_str().to_string()),
```
`RowPath::parse` rejects a leading `/`, so the root's path is the literal `"root"` — use `RowPath::parse("root").expect("a non-empty single segment parses")` directly and drop the fallback. Say in a comment that the bundle root is not a projected row and therefore mints its own id.

- [ ] **Step 5: Follow the compiler through the panel**

In `tree_panel.rs`, every place that stored a key as a `String` keeps storing a `String` — but the value becomes `crate::tree::key_string(&node.key)`. Change:
- `id_to_key: HashMap<LiveId, String>` — populate with `key_string(&node.key)`.
- `chevron_rects`, `open_directories`, `selected_key`, `reveal_key`, `pending_scroll_key` — unchanged types, values now come from `key_string`.
- `reveal_path`'s `Directory { address }` arm: `node.is_directory && node.address.as_deref() == Some(address.as_str())`, and it returns `key_string(&node.key)`.
- `row_navigation(key, ...)`: its `is_directory` branch built a `NavigationTarget::Directory { address: key.to_owned() }` from the key. That is no longer an address. Change its first parameter to `address: Option<&str>` and the branch to:
```rust
    if is_directory {
        let address = address?;
        return Some(NavigationIntent::Resolved {
            target: NavigationTarget::Directory { address: address.to_owned() },
            disposition: OpenDisposition::Preview,
        });
    }
```
  and at the call site, look the node up by `key_string` to recover its `address`. If the panel currently has no node lookup by key, add a private `fn node_for_key(&self, key: &str) -> Option<&TreeNode>` that walks `self.tree.roots` comparing `key_string(&node.key)`; the existing `reveal_path` walk is the shape to copy. Update `row_navigation_uses_the_row_key_for_directory_addresses` (~line 1608) to pass the address, and rename it to `row_navigation_uses_the_row_address_for_directories`.
- Test fixtures in `tree_panel.rs` (e.g. ~line 1573) construct `TreeNode` literally: give each a `key: RowId { owner: ViewId::new(waml::view::root::ROOT_VIEW_OWNER), path: RowPath::parse("...").unwrap() }` and an `address`. Add a small private test helper `fn node_key(path: &str) -> waml::view::row::RowId` to avoid repeating the literal seven times.

In `nav.rs`: `find_node(nodes, key: &str)` compares `key_string(&n.key) == key`; but `scope_node` / `scoped_roots` / `packages` are called with a *directory address* (`NavState::scope`, default `"/"`). Change `find_node` to `find_directory(nodes, address: &str)` comparing `n.address.as_deref() == Some(address)`, and update all three callers plus `nav.rs`'s tests. `PackageRow.key` keeps its `String` type and takes the directory address.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-editor`
Expected: PASS.

- [ ] **Step 7: Run the full gate, then commit**

```bash
git add crates/waml crates/waml-editor/src
git commit -m "refactor(tree): key tree rows on RowId, not a file address

A RowId is stable across a re-projection, so selection and expansion
survive the chain re-run a mode flip triggers. A file address is not: a
middleware may relabel a row or mint one with no file behind it, and
there is then nothing to key on.

Directory rows carry their real OKF address in a separate field, since
that is what navigation opens; the key answers identity and the address
answers location, and conflating them is what made this change
necessary."
```

---

### Task 8: Tree rows carry `caps` / `child_caps`

The tree's `can_edit_classifier` / `can_delete_classifier` are classifier-shaped guesses derived from the document type. The chain already declares, per row, what its owner will accept. Carrying the declared capabilities instead means an affordance the tree draws is one `apply` will honour.

Neither existing bool has a production consumer today (only `tree.rs` writes them and a `tree_panel.rs` test fixture sets them), so this is a clean swap, not a menu rewrite.

**Files:**
- Modify: `crates/waml-editor/src/tree.rs`
- Modify: `crates/waml-editor/src/tree_panel.rs` (test fixtures only)

**Interfaces:**
- Consumes: `waml::view::row::{RowCaps, ChildCaps}`.
- Produces: `TreeNode { pub caps: RowCaps, pub child_caps: ChildCaps, ... }`; `can_edit_classifier` and `can_delete_classifier` are gone from `TreeNode` (they stay on `DocumentCapabilities` in `document.rs`, which is a different type with its own consumers — do not touch it).

- [ ] **Step 1: Write the failing test**

In `tree.rs`'s test module:

```rust
    /// Capabilities are advisory and `apply` remains the authority, but the
    /// invariant the chain spec states is that a DECLARED capability must not
    /// yield Unsupported. Carrying the row's own declaration is what lets the
    /// tree gate an affordance on something apply will honour, instead of on
    /// a guess made from the document type.
    #[test]
    fn tree_rows_carry_the_projected_rows_declared_capabilities() {
        let prepared = mixed_prepared();
        let limits = waml::view::chain::ChainLimits::default();
        let mode = crate::folder_projection::ViewMode::Projected;
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", mode, limits);
        let sales = &tree.roots[0].children[0];
        let (_, rows, _) =
            crate::folder_projection::project_rows(prepared.okf(), "/sales", mode, limits).unwrap();

        assert_eq!(sales.children.len(), rows.len());
        for (node, row) in sales.children.iter().zip(rows.iter()) {
            assert_eq!(node.caps, row.caps);
            assert_eq!(node.child_caps, row.child_caps);
        }
    }
```

`mixed_prepared()` is a small helper returning the `PreparedCandidate` behind the existing `mixed()` fixture; if `mixed()` already returns the analyses rather than the candidate, add `fn mixed_prepared() -> waml::analysis::PreparedCandidate` alongside it with the same source pairs rather than reshaping the existing helper.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p waml-editor tree::tree_rows_carry`
Expected: FAIL to compile — no field `caps` on `TreeNode`.

- [ ] **Step 3: Swap the fields**

In `tree.rs`'s `TreeNode`, replace:

```rust
    pub can_edit_classifier: bool,
    pub can_delete_classifier: bool,
```

with:

```rust
    /// What the row's OWNING chain stage declares it will accept for this row
    /// (rename, delete, move out) and for the rows beneath it (reorder,
    /// insert, accept a move in).
    ///
    /// Advisory, for affordances only -- `Chain::apply` remains the authority.
    /// A middleware may under-declare and still accept an op; the invariant
    /// that matters is the converse, that a declared capability must not yield
    /// Unsupported.
    pub caps: waml::view::row::RowCaps,
    pub child_caps: waml::view::row::ChildCaps,
```

Populate both from the row each node came from (`row.caps`, `row.child_caps`). Directory nodes built by the recursive `directory_node` call take theirs from the folder row that produced them, alongside the `title` and `address` assignment Task 6/7 already do there. The bundle root, which no row emits, takes `RowCaps::default()` / `ChildCaps::default()`. Virtual rows take theirs from the row too.

`concept_node` no longer needs `crate::documents::describe`'s capabilities — check whether `describe` is still used for `presentation` (it is) and keep that call, dropping only the two capability reads.

- [ ] **Step 4: Update the panel's test fixtures**

`tree_panel.rs` ~line 1573 sets `can_edit_classifier: is_classifier, can_delete_classifier: is_classifier`. Replace with:

```rust
            caps: waml::view::row::RowCaps {
                rename: is_classifier,
                delete: is_classifier,
                move_out: false,
            },
            child_caps: waml::view::row::ChildCaps::default(),
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p waml-editor`
Expected: PASS.

- [ ] **Step 6: Run the full gate, then commit**

```bash
git add crates/waml-editor/src
git commit -m "refactor(tree): carry the row's declared caps instead of guessing

can_edit_classifier and can_delete_classifier were derived from the
document type -- a guess about what an edit would do, made in a different
place from the code that would do it. The chain already declares, per
row, what its owner accepts, and the chain spec's invariant is that a
declared capability must not yield Unsupported.

Carrying the declaration means an affordance the tree draws is one apply
will honour. Capabilities stay advisory; apply remains the authority."
```

---

### Task 9: The tree panel's projected/raw toggle button

One `IconButton` in the tree panel, whose glyph shows the CURRENT state rather than the action it would perform: `SquareLibrary` for Projected (the curated shelf the author's `view:` describes), `SquareCode` for Raw (the files as they sit on disk, chain bypassed).

Panel-side only: the button, its action, and its glyph state. Task 10 makes the flip do anything.

**Two traps, both real:**
1. The panel owns no `IconButton` children today (`tree_panel.rs:915` — collapse and expand both arrive from the caption bar). This is the first one back.
2. A child widget is silently dead and invisible unless its `script_mod!` registers **before** its consumer; there is no glob import in `app.rs`. **Verify, do not assume:** `app.rs` already calls `crate::icon_button::script_mod(vm)` before `crate::tree_panel::script_mod(vm)` (~lines 1155 and 1172). Confirm that ordering still holds before writing the DSL, and if it does not, move `icon_button` above `tree_panel` in the same commit.

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs` (`script_mod!` DSL, `ProjectTreeAction`, `handle_event`, a `set_view_mode` setter)
- Modify: `crates/waml-editor/src/icons_overlay.rs` (remove the two temporary `UNWIRED_BUT_LISTED` entries — the button IS their call site)

**Interfaces:**
- Consumes: `crate::icon_button::{IconButtonAction, IconButtonWidgetRefExt}`, `crate::icons::Icon`, `crate::folder_projection::ViewMode`.
- Produces:
  - `ProjectTreeAction::ToggleViewMode` — emitted on a click.
  - `ProjectTree::set_view_mode(&mut self, cx: &mut Cx, mode: ViewMode)` — sets the button's glyph and its `active` state. Called by Task 10.

- [ ] **Step 1: Verify the registration order**

Run:
```bash
grep -n "icon_button::script_mod\|tree_panel::script_mod" crates/waml-editor/src/app.rs
```
Expected: `icon_button::script_mod` appears on a LOWER line number than `tree_panel::script_mod`. If not, reorder and say why in the commit body.

- [ ] **Step 2: Write the failing test**

In `tree_panel.rs`'s test module, using the existing `mounted_project_tree_test_context()` harness:

```rust
    /// The glyph shows the CURRENT state, not the action the button would
    /// perform: a reader looking at the panel must be able to tell whether
    /// what they see is the author's declared view or the raw listing.
    #[test]
    fn the_toggle_glyph_reports_the_current_mode() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();

        panel.set_view_mode(&mut cx, crate::folder_projection::ViewMode::Projected);
        assert_eq!(panel.view_mode, crate::folder_projection::ViewMode::Projected);
        assert_eq!(panel.view_mode_icon(), crate::icons::Icon::SquareLibrary);

        panel.set_view_mode(&mut cx, crate::folder_projection::ViewMode::Raw);
        assert_eq!(panel.view_mode, crate::folder_projection::ViewMode::Raw);
        assert_eq!(panel.view_mode_icon(), crate::icons::Icon::SquareCode);
    }

    /// The button must actually be mounted and queryable. An unregistered or
    /// misnamed child instantiates a dead, unqueryable node -- invisible
    /// glyph, no-op set_icon, green gate -- so assert the query resolves.
    #[test]
    fn the_toggle_button_is_a_live_mounted_child() {
        let (mut cx, mut panel, _) = mounted_project_tree_test_context();
        panel.set_view_mode(&mut cx, crate::folder_projection::ViewMode::Raw);
        assert!(
            !panel.view.icon_button(&mut cx, ids!(view_mode_btn)).is_empty(),
            "view_mode_btn did not resolve; check script_mod registration order",
        );
    }
```

If `WidgetRef` has no `is_empty()` in this makepad fork, assert instead that `panel.view.icon_button(&mut cx, ids!(view_mode_btn)).borrow::<crate::icon_button::IconButton>().is_some()` — copy whatever shape another mounted-child test in the repo already uses (`document_header.rs`'s tests are the closest precedent).

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p waml-editor tree_panel::`
Expected: FAIL to compile — no method `set_view_mode`.

- [ ] **Step 4: Mount the button in the DSL**

In `tree_panel.rs`'s `script_mod!` block, add `use mod.widgets.IconButton` alongside the existing `use mod.widgets.*` if the block does not already resolve it, and insert a control strip above `tree_scroll` inside `mod.widgets.ProjectTree`:

```
        // The panel's only control. It owns no other IconButton children --
        // collapse and expand both arrive from the caption bar -- so this
        // strip exists solely to seat it.
        control_strip := View {
            width: Fill
            height: Fit
            flow: Right
            align: { x: 1.0 }
            padding: Inset{left: 6.0, right: 6.0, top: 6.0, bottom: 2.0}
            view_mode_btn := IconButton{ width: 28.0 height: 28.0 icon_size: 16.0 }
        }
```

- [ ] **Step 5: Add the state, the setter, and the action**

In `struct ProjectTree` (the widget), next to the other `#[rust]` fields:

```rust
    /// The mode this panel is currently DISPLAYING, pushed by the app. The
    /// panel never decides it -- it reports a click and redraws what it is
    /// told, so the tree and the folder tabs can never disagree.
    #[rust]
    view_mode: crate::folder_projection::ViewMode,
```

Inherent impl:

```rust
    /// The glyph for the CURRENT state -- `SquareLibrary` when the declared
    /// chain is running, `SquareCode` when it is bypassed. Not the action the
    /// button would perform: a reader must be able to read the panel and know
    /// what they are looking at.
    pub fn view_mode_icon(&self) -> Icon {
        match self.view_mode {
            crate::folder_projection::ViewMode::Projected => Icon::SquareLibrary,
            crate::folder_projection::ViewMode::Raw => Icon::SquareCode,
        }
    }

    pub fn set_view_mode(&mut self, cx: &mut Cx, mode: crate::folder_projection::ViewMode) {
        self.view_mode = mode;
        let icon = self.view_mode_icon();
        let button = self.view.icon_button(cx, ids!(view_mode_btn));
        button.set_icon(cx, icon);
        // Raw is the deliberate, non-default state, so it reads lit.
        button.set_active(cx, matches!(mode, crate::folder_projection::ViewMode::Raw));
    }
```

Add `use crate::icon_button::IconButtonWidgetRefExt;` to the file's imports.

`ProjectTreeAction` gains:

```rust
    /// The projected/raw toggle was clicked. The panel does not flip its own
    /// mode: `App` owns the session-wide switch and pushes the new mode back
    /// via `set_view_mode`, so the tree and every open folder tab move
    /// together or not at all.
    ToggleViewMode,
```

In `Widget::handle_event`'s `Event::Actions(actions)` block, before the `file_tree.folder_clicked` branch:

```rust
            if self.view.icon_button(cx, ids!(view_mode_btn)).clicked(actions) {
                cx.widget_action(uid, ProjectTreeAction::ToggleViewMode);
            }
```

Update the module's head comment: the "The panel owns no `IconButton` children any more" note at ~line 915 and the file-header note at ~line 13 are now false. Replace both with a sentence saying the panel owns exactly one, the projected/raw toggle, and that collapse/expand still come from the caption bar.

- [ ] **Step 6: Un-list the two glyphs**

In `icons_overlay.rs`, delete `Icon::SquareCode` and `Icon::SquareLibrary` (and their temporary comment) from `UNWIRED_BUT_LISTED`. They now have a call site, so guard 2 is satisfied and guard 3 would fail if they stayed.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p waml-editor tree_panel:: icons`
Expected: PASS — including the three icon guards.

- [ ] **Step 8: Run the full gate, then commit**

Visual verification of the two glyphs, their lit states, and the button's placement is DEFERRED (row V3).

```bash
git add crates/waml-editor/src
git commit -m "feat(tree-panel): add the projected/raw toggle button

The glyph shows the current state rather than the action -- a reader must
be able to look at the panel and know whether what they see is the
author's declared view or the raw listing, which a
what-this-button-would-do glyph cannot say.

The panel does not flip its own mode. App owns the session-wide switch
and pushes the result back, so the tree and every open folder tab move
together or not at all; a panel-local flag would let them disagree.

First IconButton child the panel has owned; icon_button::script_mod
already registers ahead of tree_panel::script_mod, so the child resolves."
```

---

### Task 10: Flip the session mode and re-run every open folder tab

The last wire. `App` owns the switch; a flip rebuilds the tree projection and re-runs every open folder tab in place — same tab, view swapped. Concept tabs are untouched.

**Files:**
- Modify: `crates/waml-editor/src/app/actions.rs` (handle `ProjectTreeAction::ToggleViewMode`)
- Modify: `crates/waml-editor/src/app/navigation.rs` (push the mode into the panel from `refresh_nav`; add `refresh_folder_tabs`)

**Interfaces:**
- Consumes: `App.view_mode` (Task 4), `ProjectTree::set_view_mode` and `ProjectTreeAction::ToggleViewMode` (Task 9), `DocumentCommand::ReopenInPlace` (Task 5), `documents::open_folder` (Task 4), `DocViewIdentity::Folder`.
- Produces: `App::toggle_view_mode(&mut self, cx: &mut Cx)`, `App::refresh_folder_tabs(&mut self, cx: &mut Cx)`.

- [ ] **Step 1: Write the failing test**

In `crates/waml-editor/src/app/tests/` (add to the module where tree/navigation app tests already live — `navigation.rs` is the closest fit):

```rust
    /// A flip re-runs every open folder tab IN PLACE -- same tab, view
    /// swapped -- and leaves concept tabs alone. Opening a second tab for the
    /// same folder, or leaving the old view behind, are both the defect the
    /// old "View raw" had.
    #[test]
    fn a_mode_flip_re_runs_open_folder_tabs_in_place() {
        let (mut cx, mut app) = app_with_bundle(&[
            (
                "index.md",
                "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n* [Orders](orders.md)\n* [References](references/)\n",
            ),
            ("orders.md", "# Orders\n"),
            ("references/index.md", "# References\n"),
        ]);

        app.navigate_with(
            &mut cx,
            NavigationIntent::Resolved {
                target: NavigationTarget::Directory { address: "/".into() },
                disposition: OpenDisposition::Persistent,
            },
        );
        let tabs_before = app.documents.tabs().len();
        let tab_id = app.documents.active_id();

        app.toggle_view_mode(&mut cx);

        assert_eq!(app.view_mode, crate::folder_projection::ViewMode::Raw);
        assert_eq!(app.documents.tabs().len(), tabs_before, "no second tab");
        assert_eq!(app.documents.active_id(), tab_id, "the tab keeps its identity");

        app.toggle_view_mode(&mut cx);
        assert_eq!(app.view_mode, crate::folder_projection::ViewMode::Projected);
        assert_eq!(app.documents.tabs().len(), tabs_before);
    }

    /// Mode is a session fact, not a preference. Nothing writes it anywhere.
    #[test]
    fn the_mode_starts_projected_and_is_never_persisted() {
        let (mut cx, mut app) = app_with_bundle(&[("index.md", "# Root\n")]);
        assert_eq!(app.view_mode, crate::folder_projection::ViewMode::Projected);
        app.toggle_view_mode(&mut cx);
        // The settings type has no field for it, by construction. This
        // assertion is the fence: adding one must break a test, not pass
        // silently.
        let settings = crate::project_settings::ProjectSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert!(!json.contains("view_mode"), "the mode must not reach settings");
    }
```

Adapt `app_with_bundle` to whatever fixture constructor `app/tests/` already provides (`app/tests/mod.rs` has the harness); do not add a second one. If `ProjectSettings` is not `Serialize`, drop the second test's JSON assertion and instead assert that `git grep -n "view_mode" -- crates/waml-editor/src/project_settings.rs` is empty — as a comment in the test naming the invariant, not as code.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor app::tests`
Expected: FAIL to compile — no method `toggle_view_mode`.

- [ ] **Step 3: Handle the action**

In `app/actions.rs`, next to the existing `ProjectTreeAction` handling (follow the shape of the `Navigate` / `ContextMenu` dispatch already there):

```rust
            crate::tree_panel::ProjectTreeAction::ToggleViewMode => {
                self.toggle_view_mode(cx);
            }
```

- [ ] **Step 4: Implement the flip**

In `app/navigation.rs`, in the same `impl App` block as `refresh_nav`:

```rust
    /// Flip the session-wide projected/raw switch.
    ///
    /// Both surfaces read the same flag, so there is no state in which the
    /// tree and a folder view disagree about what a directory contains. The
    /// flag lives in memory only: raw is a deliberate act, not a preference,
    /// so it is never written to `.waml/settings.json` and every launch
    /// starts Projected.
    ///
    /// This is presentational. A row the declared chain does not emit is not
    /// protected by anything; raw simply asks for the identity listing.
    pub(super) fn toggle_view_mode(&mut self, cx: &mut Cx) {
        self.view_mode = match self.view_mode {
            crate::folder_projection::ViewMode::Projected => {
                crate::folder_projection::ViewMode::Raw
            }
            crate::folder_projection::ViewMode::Raw => {
                crate::folder_projection::ViewMode::Projected
            }
        };
        self.refresh_nav(cx, false);
        self.refresh_folder_tabs(cx);
        cx.redraw_all();
    }

    /// Re-run every OPEN folder tab under the current mode, in place -- same
    /// tab, view swapped. Concept tabs are untouched: a mode is about how a
    /// container lists its contents, and a concept has none.
    ///
    /// `ReopenInPlace` rather than `Open` because `Open` keeps the existing
    /// view whenever the tab id is already open; that is exactly what made
    /// the old per-folder "View raw" build a view and throw it away.
    pub(super) fn refresh_folder_tabs(&mut self, cx: &mut Cx) {
        let folders: Vec<String> = self
            .documents
            .tabs()
            .iter()
            .filter(|tab| tab.presentation.category == crate::document::NavCategory::Directory)
            .map(|tab| tab.concept_id.clone())
            .collect();
        for directory in folders {
            let Some(document) = crate::documents::open_folder(
                self.session.okf_analysis(),
                &directory,
                self.chain_limits,
                self.view_mode,
            ) else {
                // The directory left the bundle underneath us. Leaving the
                // stale view up is wrong, but so is silently closing a tab
                // the user opened; the next model refresh reconciles it.
                continue;
            };
            self.documents.transition(
                cx,
                &self.ui,
                &self.session,
                DocumentCommand::ReopenInPlace { document },
            );
        }
    }
```

`tab.concept_id` is the field `folder_documents::open` sets to the directory address; confirm the accessor's real name on `DocTab` and use it. If `DocTab` exposes a `locator()` instead, match on `DocumentLocator`'s directory variant rather than on `presentation.category` — a locator match is the sturdier filter and should be preferred if one exists.

- [ ] **Step 5: Push the mode into the panel**

In `refresh_nav`, inside the `if let Some(mut panel) = ...` block, after `set_view_with_fold_reset`:

```rust
            panel.set_view_mode(cx, self.view_mode);
```

This is the only place the panel learns the mode, so the button can never show a state the app is not in.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p waml-editor`
Expected: PASS.

- [ ] **Step 7: Run the full gate**

Then confirm the deletion from Task 3 is still total and no raw affordance crept back:

```bash
git grep -n "DirectoryRaw\|open_raw\|build_raw\|raw_requested" -- crates editors
```
Expected: no output.

Visual verification that an open folder tab changes content in place on a flip rather than opening a second tab is DEFERRED (row V4).

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src
git commit -m "feat(app): flip the session view mode and re-run open folder tabs

One switch, read by both the tree seam and every folder tab, so there is
no state in which the two disagree about what a directory contains. A
flip re-runs each open folder tab in place through ReopenInPlace --
concept tabs are untouched, and no folder gets a second tab.

The mode is in memory only. Raw is a deliberate act, not a preference:
every launch starts Projected so an author's declared view is what a
reader sees unless they ask otherwise. Nothing here is a permission
check."
```

---

## Deferred visual verification

The implementer has no window. Every item below is stated in its task as deferred and must be checked by the human before sign-off. A green gate is not evidence for any of them.

| # | Task | What to look at | Pass condition |
|---|---|---|---|
| V1 | 1, 2 | The icons style-guide overlay (burger menu → Icons reference). | `book`, `box`, `square-code`, `square-library` each draw a legible Lucide glyph at 18px, correctly weighted and not clipped by the viewport. If a stroke clips, revisit `A` / `B` / `STROKE_W` per-glyph in `scripts/gen-icon.py` as its header describes. |
| V2 | 6 | A bundle whose folder declares `view: hide` with a matching `hide:` glob, tree panel open. | The hidden child is absent from the tree in Projected and present in Raw, and the tree agrees row-for-row with that folder's own tab. |
| V3 | 9 | The tree panel's top-right control strip. | Exactly one button; `SquareLibrary` at rest in Projected, `SquareCode` and lit in Raw; hover lights the wash; the Hand cursor appears and clears. The button does not shove the tree rows or clip at the panel's narrowest dragged width. |
| V4 | 10 | An open folder tab for a `hide:` folder; click the toggle. | The tab's content changes in place. No second tab appears, the tab keeps its position in the strip, and it stays active. Flipping back restores the projected listing. |
| V5 | 7, 10 | Expand several folders and select a row, then flip the mode twice. | Expansion and selection survive for rows the raw chain still owns; a selection on a row the raw chain does not own falls back to the nearest folder rather than vanishing, and nothing panics. |

## Self-review notes

- **Spec coverage.** The mode (Tasks 4, 10); deletion of "View raw" / banner / `open_raw` / `DirectoryRaw` (Task 3); tree-as-projection (Task 6); `TreeNode.key` as `RowId` (Task 7); virtual rows degrading rather than panicking (Task 6, `RowTarget::Virtual` arm and `openable: false`); `caps` / `child_caps` replacing the classifier bools (Task 8); `view_degraded` unchanged and on the same registry (Task 6, Step 3, plus the existing `the_tree_and_the_folder_view_agree_on_whether_a_chain_degraded` test); the toggle and its glyph semantics (Task 9); four icons with the six-list ritual and the `UNWIRED_BUT_LISTED` churn (Tasks 1, 2, 9); flipping with tabs open and the `DocumentHost` view-replace path (Tasks 5, 10); every headless test the spec lists.
- **One spec item is explicitly deferred, not implemented:** "Tree edits — rename, delete, drag — route through `Chain::apply`". The tree panel has no rename, delete, or drag gesture today (its only edit surface is the context menu, which routes through the node menu), so there is nothing to reroute. Task 8 lands the capability data those gestures would gate on. Building the gestures themselves is a separate change and is deliberately out of scope here.
