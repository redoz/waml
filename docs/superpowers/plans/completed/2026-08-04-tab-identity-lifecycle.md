# Tab Identity & Lifecycle Fixes

**Context.** Investigation of the issues.md P2 item "Tab and navigation state" found three real defects in the waml-editor tab/navigation layer (a fourth suspected defect — first-matching-tab promotion — is already fixed and covered by a regression test at `crates/waml-editor/src/document_host.rs:631`). First, `DocumentHost::reconcile_documents` (`crates/waml-editor/src/document_host.rs:366-405`) silently retains a stale live tab and view when a document's locator no longer resolves after a rename or deletion (`let Some(prepared) = prepared else { continue; }` at ~377). Second, there is no per-tab anchor cache: manually activating a tab always arrives with `ViewAnchor::None`, so `ClassDiagramView::sync` (`crates/waml-editor/src/class_diagram_view.rs:448-469`) clears selection and camera; the departing tab's anchor goes to view history (`crates/waml-editor/src/app/navigation.rs:406`) but the arriving tab's last anchor is stored nowhere. Third, Back/Forward commits traversal immediately (`crates/waml-editor/src/app/navigation.rs:531`) while the anchor restore is deferred to the next Draw via `pending_anchor_restore` (`navigation.rs:440-443`, applied at `navigation.rs:340-357`, driven from `crates/waml-editor/src/app/event.rs:141`); a rapid second traversal refreshes the departing entry from a stale pre-restore anchor (`navigation.rs:419-423`) and overwrites the first pending restore (`navigation.rs:440`). The tasks below fix these in dependency order: tombstones first (4), then the anchor cache (1), then traversal generation guarding (3). Existing headless harnesses (`ProbeView` in `document_host.rs` cfg(test), and `app/tests/navigation.rs`) suffice; no new library seam is required.

**Verification.** After each task: `cargo test -p waml-editor`

### Task 1: Tombstone tabs whose locator no longer resolves (Defect 4)

**Files:**
- `crates/waml-editor/src/document_host.rs`
- `crates/waml-editor/src/doc_tabs.rs`

**Steps:**
1. Add a `resolved: bool` field (default `true`) to `DocTab` (`doc_tabs.rs:135`). Prefer the flag over a `TabState` enum — the only unresolved state is "tombstoned", and a bool keeps `OpenTabs` equality/clone semantics untouched.
2. In `DocumentHost::reconcile_documents` (`document_host.rs:366-405`), replace the silent `let Some(prepared) = prepared else { continue; }` retention: when `prepared` is `None`, set `self.tabs.tabs[index].resolved = false` and continue. When `prepared` is `Some(..)` (locator resolves again, e.g. after undo), set `resolved = true` on both the compatible-retain path and the replacement path — revival happens naturally through the existing prepared-document flow.
3. In `DocumentHost::sync_active` (`document_host.rs`, the method containing the existing no-active-view branch at ~242-246), treat an active tab with `resolved == false` the same as "no active view": skip `sync_from_session` and disable canvas interaction via the existing `set_canvas_interaction_enabled(cx, false)` / `set_behavior_canvas_interaction_enabled(cx, false)` calls, so a tombstoned tab cannot be interacted with.
4. In `DocTabs::set_tabs` (`doc_tabs.rs`), render tombstoned tabs dimmed (reuse the existing dim/inactive color path for the title) so the user can see the document is gone. Closing a tombstoned tab must keep working via the untouched `DocumentCommand::Close` path.

**Tests** (in the `cfg(test)` module of `document_host.rs`, using `ProbeView` at ~line 476):
- Tombstone round-trip: open two documents, run `after_session_change` with `prepared = [Some(..), None]`; assert the second tab is retained but `resolved == false` and, when active, `sync_active` disables interaction. Then re-run with `prepared = [Some(..), Some(..)]` (the undo case); assert `resolved == true` again and the view syncs.
- Closing a tombstoned tab: tombstone a tab, `transition(.., DocumentCommand::Close(id))`; assert the tab and its view are removed and the host stays consistent.

### Task 2: Per-tab anchor cache in DocumentHost (Defect 1)

**Files:**
- `crates/waml-editor/src/document_host.rs`

**Steps:**
1. Add `anchors: HashMap<LiveId, ViewAnchor>` to `DocumentHost`. Hosting the cache here (not on `App`) covers every `DocumentCommand::Activate` path, including ones that bypass `app/navigation.rs` (`app/actions.rs:242`, `app/actions.rs:744`, `transition_document` at `navigation.rs:372`, `open_view_source` at `navigation.rs:394`).
2. In `finish_transition` (`document_host.rs:249`): before calling `on_deactivate` on the old view, capture its anchor with `view.capture_anchor(&body)` and store it under `old_active` (both the removed-view branch and the still-registered branch). After `sync_active` runs for the new tab, if `anchors` holds an entry for `new_active`, restore it via the existing `restore_anchor` path (same mechanics as `restore_active_anchor`, using `session.snapshot()`).
3. Restore order matters: `sync_from_session` (e.g. `ClassDiagramView::sync`, `class_diagram_view.rs:448-469`) clears selection, so the cached-anchor restore must run after `sync_active`, mirroring how `restore_location_with_asset_host` restores after transition.
4. Do not let the cached anchor clobber an explicit incoming anchor: `restore_location_with_asset_host` already calls `restore_active_anchor` with the location's anchor after `transition`; ensure the cache restore in `finish_transition` is skipped or harmlessly overwritten in that flow (only apply the cached anchor when the caller supplied `ViewAnchor::None`, or simply let the explicit restore run last — pick whichever reads cleaner, but assert the explicit-anchor path still wins in a test).
5. Evict cache entries for closed tabs in `reconcile_registry` (`document_host.rs:41-53`): remove `anchors` entries for every stale id alongside the view removal. Also clear the map in `replace_tabs_for_session`.

**Tests** (extend `crates/waml-editor/app/tests/navigation.rs`, headless harness `navigation_app_with_active_order` at ~line 639):
- Switch A -> B -> A with a selection/camera set on A before departing; after returning to A, assert `capture_anchor` reports the preserved selection key and camera rather than the cleared default.
- Explicit anchor wins: navigate to A with a concrete `ViewAnchor` while a different cached anchor exists for A; assert the explicit anchor is the one in effect.
- Eviction: close A, reopen it; assert the stale cached anchor was not applied.

### Task 3: Generation-guard the deferred history-traversal restore (Defect 3)

**Files:**
- `crates/waml-editor/src/app/navigation.rs`
- `crates/waml-editor/src/app/mod.rs` (or wherever the `App` fields `pending_anchor_restore` live)

**Steps:**
1. Add `generation: u64` to `PendingAnchorRestore` (`navigation.rs:9-13`) and a monotonically increasing `anchor_restore_generation: u64` counter field on `App`. When `transition_to_location` sets `pending_anchor_restore` (`navigation.rs:440-443`), increment the counter and stamp the new generation.
2. In the `TransitionCause::HistoryTraversal` branch (`navigation.rs:419-423`): skip `refresh_current(departing)` when `self.pending_anchor_restore` is `Some` and its `document` matches the departing document — the departing view's anchor is pre-restore stale, and refreshing history with it corrupts the entry the first traversal was about to restore.
3. In `apply_pending_anchor_restore` (`navigation.rs:340-357`): after restoring, only call `view_history.refresh_current` when the pending entry's `generation` still equals `self.anchor_restore_generation` (i.e. no newer traversal superseded it). The restore itself may still run if the active tab's locator matches (existing guard at :345-350), but a superseded generation must not refresh history. This is option (a): traversal stays responsive; the stale refresh is suppressed instead of blocking the second traversal.
4. Leave `traverse_view_history`'s immediate `commit_traversal` (`navigation.rs:531`) as is — the guard makes the deferred restore safe against it.

**Tests** (extend `crates/waml-editor/app/tests/navigation.rs`):
- Two Back traversals with no intervening Draw: build a 3-entry history with distinct anchors, issue Back twice, then assert the intermediate history entry's anchor was not overwritten with a default/stale anchor (inspect via the harness's history accessor or a `#[cfg(test)]` probe).
- After pumping a Draw (the harness's event pump that drives `apply_pending_anchor_restore` via `app/event.rs:141`), assert the final entry's refresh reflects the actually-restored anchor and that the second pending restore (latest generation) was the one applied.
