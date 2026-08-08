# Save and Undo

**Goal:** An author does not lose work and can go back one edit at a time.

**Why:** A save or history failure can destroy work and remove trust in the
editor.

**Done when:** Accepted edits enter one revision, undo and redo are reciprocal,
savepoints control dirty state, failed final saves keep the document open, and
successful saves permit close.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-027 — an accepted source edit advances once

**Applies to:** native

**Given** a source edit applies to the current document revision
**When** the editor accepts the edit
**Then** the session advances exactly once
**And** the installed semantic result uses the accepted source revision

**Evidence:** `crates/waml-editor/src/editor_session/tests.rs::accepted_source_edit_advances_once_and_promotes_the_same_syntax_arc`

#### NATIVE-028 — undo and redo exchange reciprocal history entries

**Applies to:** native

**Given** one accepted edit is in the document history
**When** the author undoes the edit and then redoes it
**Then** undo restores the earlier document and creates the reciprocal redo entry
**And** redo restores the edited document and creates the reciprocal undo entry

**Evidence:** `crates/waml-editor/tests/editor_history.rs::one_edit_produces_undo_and_undo_redo_swap_reciprocals`

#### NATIVE-029 — undo to a savepoint clears dirty state

**Applies to:** native

**Given** the document history has a saved-state marker and a later edit
**When** the author undoes back to the saved history entry
**Then** the document matches the saved-state identity and is not dirty

**Evidence:** `crates/waml-editor/tests/editor_history.rs::savepoint_identity_tracks_undo_back_to_saved_state`

#### NATIVE-030 — a failed final save keeps dirty work open

**Applies to:** native

**Given** a dirty document cannot complete its final save
**When** the author tries to close the editor
**Then** the editor does not quit
**And** the document remains dirty

**Evidence:** `crates/waml-editor/src/native_save.rs:111`

#### NATIVE-031 — a successful bundle open clears the save error

**Applies to:** native

**Given** the editor shows an error from an earlier save
**When** the author successfully opens a valid bundle
**Then** the visible save error is cleared

**Evidence:** `crates/waml-editor/src/app/tests/workspace.rs::successful_bundle_open_clears_the_visible_save_error`

#### NATIVE-048 — a new edit after undo clears redo history

**Applies to:** native

**Given** the author has undone an edit and redo is available
**When** the author makes a new edit
**Then** the new edit becomes the current history entry and redo is unavailable

**Evidence:** `crates/waml-editor/tests/editor_history.rs::new_edit_after_undo_clears_redo`

#### NATIVE-049 — an invalid proposal does not change session history

**Applies to:** native

**Given** a source proposal is stale or invalid for the current session
**When** the editor evaluates the proposal
**Then** the current document snapshot stays unchanged
**And** undo and redo history stay unchanged

**Evidence:** `crates/waml-editor/src/editor_session/tests.rs::stale_or_invalid_source_proposals_do_not_mutate_snapshot_or_history`

#### NATIVE-056 — a failed save blocks close and keeps document state

**Applies to:** native

**Given** an open document has state that needs a final save
**When** the final save fails during close
**Then** the document stays open with its document state intact

**Evidence:** `crates/waml-editor/src/app/tests/workspace.rs::failed_save_blocks_close_and_keeps_document_state`

#### NATIVE-057 — a successful save permits close and clears document state

**Applies to:** native

**Given** an open document can complete its final save
**When** the author closes the document
**Then** the editor closes it and clears its document state

**Evidence:** `crates/waml-editor/src/app/tests/workspace.rs::successful_save_allows_close_and_clears_document_state`

#### NATIVE-062 — an accepted source proposal becomes current

**Applies to:** native

**Given** a revisioned source proposal is valid for the current session
**When** the editor promotes the proposal
**Then** the proposed source becomes the current session document

**Evidence:** `crates/waml-editor/src/editor_session/tests.rs::accepted_source_edit_advances_once_and_promotes_the_same_syntax_arc`

## Verification gaps

- NATIVE-030 — target: native; The test asserts quit prevention after a final save failure, but it does not assert that the document remains dirty.

## Notes

- The scenarios describe savepoints and edit transactions in prose.
  [FG-006](../../waml-feature-gaps.md#fg-006--transaction-groups-and-saved-states)
  records this WAML expression gap.
- Back and forward restore view positions. They do not change content and are
  owned by [Navigate and Return](../read-a-bundle/navigate-and-return.md).
- [Edit Prose](./edit-prose.md) owns text input and selection behavior.
