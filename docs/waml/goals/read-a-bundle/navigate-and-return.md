# Navigate and Return

**Goal:** A reader sees the current document path and returns to earlier view
positions.

**Why:** A reader needs location context and a reliable return path while the
reader moves through a bundle.

**Done when:** The editor creates breadcrumbs for documents and directories,
and Back and Forward restore documents and their view anchors.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-013 — breadcrumbs show the document and directory path

**Applies to:** native

**Given** a document or directory is active
**When** the editor presents its breadcrumb
**Then** the breadcrumb contains the labels and targets from the bundle root to the active item

**Evidence:** `crates/waml-editor/src/navigation.rs::breadcrumb_for` `crates/waml-editor/src/navigation.rs::breadcrumb_for_directory`

#### NATIVE-016 — back and forward restore document anchors

**Applies to:** native

**Given** the reader moved between documents with stored view anchors
**When** the reader uses Back or Forward
**Then** the editor restores the selected history document and its view anchor

**Evidence:** `crates/waml-editor/src/app/tests/navigation.rs::manual_and_preview_transitions_follow_back_and_forward_history` `crates/waml-editor/tests/history_integration.rs::back_and_forward_restore_view_anchors_without_tab_metadata`

## Verification gaps

- NATIVE-013 — target: native; No native test asserts breadcrumb labels and targets.

## Notes

- In-bundle document links are owned by [Open a Bundle](./open-a-bundle.md).
- External links and tree reveal are owned by
  [Browse the Tree](./browse-the-tree.md).
