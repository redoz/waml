# Browse the Tree

**Goal:** A reader sees the bundle structure, reveals nested items, and opens
external links without changing the document workspace.

**Why:** The tree gives direct access to documents and packages that have no
incoming document link.

**Done when:** The editor builds the tree from bundle content, opens the folders
that reveal a nested item, keeps open folders after a refresh, and sends an
external link to the browser adapter once.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-010 — bundle content builds the project tree

**Applies to:** native

**Given** a bundle is open
**When** the editor presents the project tree
**Then** the tree contains the project structure from the bundle

**Evidence:** `crates/waml-editor/src/tree.rs::build_tree`

#### NATIVE-011 — reveal opens the folders above a nested item

**Applies to:** native

**Given** a nested tree item is inside closed folders
**When** the reader reveals that item in the tree
**Then** the tree opens the folders that contain the item

**Evidence:** `crates/waml-editor/src/tree_panel.rs::reveal_path`

#### NATIVE-012 — a tree refresh keeps open folders

**Applies to:** native

**Given** the reader has open folders in the project tree
**When** refreshed bundle data changes the tree
**Then** the folders that still exist remain open

**Evidence:** `crates/waml-editor/src/tree_panel.rs::reconcile_open_directories`

#### NATIVE-015 — an external link opens once outside the workspace

**Applies to:** native

**Given** a document contains an external web link
**When** the reader follows the link
**Then** the browser adapter opens the external URL once

**Evidence:** `crates/waml-editor/src/app/tests/navigation.rs::navigation_external_target_invokes_only_the_browser_adapter_once`

## Verification gaps

- NATIVE-010 — target: native; No native test asserts the complete visible tree construction.
- NATIVE-011 — target: native; No direct native assertion covers reveal expansion.
- NATIVE-012 — target: native; No native test checks tree refresh and retained open folders.

## Notes

- Preview and permanent tab behavior is owned by
  [Work with Tabs](./work-with-tabs.md).
- Responsive tree placement is owned by [Fit the Window](./fit-the-window.md).
