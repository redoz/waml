# Work with Tabs

**Goal:** A reader uses one reusable preview and keeps selected documents in
permanent tabs.

**Why:** Preview tabs support quick reading, and permanent tabs keep important
documents available.

**Done when:** A new preview replaces the old preview, promotion keeps a tab,
reopening a permanent tab does not duplicate it, close selects the specified
fallback, and preview replacement leaves only the new preview open.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-017 — a new preview replaces the old preview

**Applies to:** native

**Given** one document is open in the preview tab
**When** the reader opens a different document in preview
**Then** the editor replaces the old preview with the new document

**Evidence:** `crates/waml-editor/src/doc_tabs.rs::open_preview_twice_replaces_the_single_preview_slot`

#### NATIVE-018 — promotion keeps a preview as a permanent tab

**Applies to:** native

**Given** one document is open in the preview tab
**When** the reader promotes that preview and opens another preview
**Then** the promoted document remains in a permanent tab
**And** the new document occupies a new preview tab

**Evidence:** `crates/waml-editor/src/doc_tabs.rs::promote_then_open_preview_keeps_the_promoted_tab_and_adds_a_fresh_preview`

#### NATIVE-019 — reopening a permanent tab does not duplicate it

**Applies to:** native

**Given** a document is open in a permanent tab
**When** the reader opens that document again
**Then** the editor focuses the existing tab without adding a duplicate

**Evidence:** `crates/waml-editor/src/doc_tabs.rs::reopening_a_promoted_tab_focuses_it_instead_of_duplicating`

#### NATIVE-020 — closing the active tab selects the fallback tab

**Applies to:** native

**Given** the active document has neighboring tabs
**When** the reader closes the active tab
**Then** the editor activates the right tab, then the left tab, then the first remaining tab as available

**Evidence:** `crates/waml-editor/src/doc_tabs.rs::close_activates_right_adjacent_then_left_then_first_tab`

#### NATIVE-021 — preview replacement leaves only the new preview open

**Applies to:** native

**Given** one document is open in preview
**When** the reader opens a different document in preview
**Then** only one preview remains open and it shows the new document

**Evidence:** `crates/waml-editor/src/document_host.rs::prepared_preview_replacement_drops_the_old_live_view`

## Notes

- The current prose workaround for ordered tabs and selected state is recorded
  in [FG-004](../../waml-feature-gaps.md#fg-004--ordered-collections-and-states).
