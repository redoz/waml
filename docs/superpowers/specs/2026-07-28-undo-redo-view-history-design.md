# Undo, redo, and view history

**Date:** 2026-07-28
**Status:** Approved design

## Context

This is a follow-on to
`2026-07-28-document-header-logical-navigation-design.md`. That design creates
one logical navigation path for tree rows, breadcrumbs, and Markdown links, but
explicitly leaves navigation history and back/forward controls out of scope.

The editor has two kinds of history with different meanings:

- Model history changes authored WAML state through Undo and Redo.
- View history changes which logical editor location is visible through Back
  and Forward.

These histories must cooperate without becoming one history. Undoing an edit
may reveal the editor where that edit occurred, but Back must never undo model
content and Undo must never merely traverse previously viewed tabs.

The existing architecture already provides useful choke points:

- `ViewOutcome::edit` submits one `PendingEdit` for one user action.
- `PendingEdit` wraps an `EditBatch`; a batch contains one or more operations.
- `EditorSession::apply` transactionally lowers a batch into a candidate
  `SourceBundle`, reparses it, and commits it only after validation succeeds.
- `DocumentHost::transition` owns document opening, preview replacement, tab
  activation, promotion, closing, and active-view synchronization.
- `OpenTabs` provides one shared replaceable preview slot for every document
  kind.

`SourceBundle` uses copy-on-write document text through `Arc<String>`. That
makes a bundle clone inexpensive until a shared document is changed, but a
changed document is still copied in full. Bundle snapshots therefore remain a
good transactional implementation detail, not the primary user-facing undo
representation.

## Goals

- Make every successful user edit reversible as one atomic transaction.
- Preserve batches containing multiple operations as one Undo step.
- Reveal the logical editor location affected by Undo and Redo.
- Keep Undo/Redo global to the currently open model.
- Preserve Undo/Redo across saves and compute dirty state relative to the
  exact saved history state.
- Retain fine-grained recent typing and interaction history while allowing
  compatible older entries to collapse.
- Add Back and Forward controls to the shared document header.
- Record preview replacement, manual tab switching, explicit logical
  navigation, active-tab closing, and Undo/Redo-driven editor switching.
- Restore logical view locations without restoring historical tab-strip state.
- Preserve selection, caret, fragment, scroll, and viewport context where the
  active view can supply it.
- Keep all history transitions transactional and bounded.

## Non-goals

- A branching Undo tree after a new edit.
- Restoring historical pinned tabs, tab ordering, or closed-tab state.
- Recording every caret movement, selection change, scroll, diagram pan, or
  zoom as a separate view-history entry.
- Recording project-tree expansion, navigator scope/filter state, dock state,
  popup state, or external browser navigation.
- Persisting either history across closing and reopening a model.
- A history palette or timeline UI.
- Undo/Redo toolbar buttons. The first UI surface is keyboard commands and
  command enablement; Back/Forward receives explicit header buttons.
- Replacing `SourceBundle` with a rope or piece table.

## Decisions

### Two independent linear histories

The open editor session owns one linear model history. The application shell
owns one linear view history.

Both use conventional branch invalidation:

- Undoing creates a Redo branch.
- A successful new edit after Undo clears Redo.
- Going Back creates a Forward branch.
- A new recorded navigation after Back clears Forward.

Keeping either discarded future would require a branching-history interface,
which is outside this design.

Undo/Redo never consumes a Back/Forward entry. Back/Forward never applies a
model edit. A visible editor switch caused by Undo or Redo is nevertheless a
normal recorded view transition, so Back can return the user to the location
they occupied before invoking the model-history command.

### Reversible semantic edit batches

The operation pipeline becomes reversible. Applying a `PendingEdit` produces
both the next source state and another `PendingEdit` that restores the prior
state:

```rust
pub struct AppliedEdit {
    pub source: SourceBundle,
    pub inverse: PendingEdit,
}

pub trait EditBatch: sealed::Sealed {
    fn apply_reversible(
        &self,
        context: EditContext<'_>,
    ) -> Result<AppliedEdit, EditError>;
}
```

This design names the method `apply_reversible`. Its contract is fixed:
successful application returns a validated candidate and an executable
inverse; failure returns neither.

For a batch containing multiple operations:

1. Start from a copy-on-write candidate source.
2. Before applying each operation, inspect the candidate's current authored
   state and construct that operation's inverse.
3. Apply the forward operation to the candidate.
4. Continue against the newly updated candidate.
5. Reverse the collected inverse-operation order.
6. Return the candidate plus one inverse batch.

The inverse need not use only public forward-operation variants. Private
restoration operations may carry before-images that do not belong in the
public editing API. Applying such an inverse must itself produce the reciprocal
forward edit. This symmetry lets Undo and Redo use the same mechanism.

Examples:

- A set operation captures the value it replaces.
- A rename captures the old and new identities.
- A remove captures the complete removed authored value and its ordering.
- A cascading classifier or directory removal captures every authored item and
  relationship required for exact restoration.
- A text-range replacement captures the replaced slice and the inserted range,
  not an entire unchanged document.

The information retained for a destructive operation is irreducible: lossless
Undo must retain the content that was removed.

### Transactional failure behavior

Forward edits, Undo, and Redo all use the same candidate-first validation:

- No session field changes until the entire batch applies and reparses.
- A failure leaves the model, revision, dirty state, and both model-history
  stacks unchanged.
- An inverse-generation failure is an edit failure, even if the forward
  mutation could otherwise be lowered.
- Undo/Redo failure leaves the popped entry on its original stack and reports a
  concise status-bar error.

An inverse failing after its corresponding forward edit succeeded indicates an
implementation invariant violation. It is still handled without partial state
mutation.

### Model-history entries

Each committed user action carries presentation, merge, state, and location
metadata:

```rust
pub struct EditHistoryEntry {
    pub edit: PendingEdit,
    pub label: String,
    pub merge_key: Option<EditMergeKey>,
    pub from_state: HistoryStateId,
    pub to_state: HistoryStateId,
    pub target_location: ViewLocation,
    pub reciprocal_location: ViewLocation,
}
```

The stack stores the edit to execute in its current direction. Applying it
returns the reciprocal edit, which is pushed to the opposite stack with the
state and location direction reversed.

For a normal edit:

1. Capture the active logical location before application.
2. Apply the forward batch and obtain its inverse.
3. Capture the resulting logical location.
4. Push the inverse entry onto Undo.
5. Clear Redo.

For Undo:

1. Peek the Undo entry.
2. Transactionally apply its inverse.
3. Push the returned reciprocal edit onto Redo.
4. Synchronize every affected model projection and open view.
5. Restore the entry's target location in the resulting model.
6. Report `Undid <label>`.

Redo is the same process with the stacks and locations reversed and reports
`Redid <label>`.

Location restoration happens after model application. This is required for an
Undo that recreates a deleted subject or restores the pre-rename identity.

### Global model history with visible edit origins

Undo/Redo is global to the open model rather than local to a tab. WAML
operations can rename references, remove cascades, reorder directories, and
otherwise affect several authored documents in one transaction; assigning
such an edit to one tab-local stack would make ownership ambiguous.

A global command must not silently change content in an unrelated hidden
editor. Every history entry therefore carries before/after logical locations.
Undo reveals the before-location; Redo reveals the after-location. The location
also includes the affected selection or caret when available.

If revealing the location activates a different document, that activation is
recorded in view history. The user can press Back to return to the editor they
occupied before Undo or Redo.

### Recent atomic fidelity and older compaction

Every submitted user action initially remains its own history entry. The newest
64 model-history entries form an atomic tail and are never compacted.

This means typing `Customerr` and immediately invoking Undo removes only the
last `r` when each insertion was submitted atomically. Pointer-driven
interactions likewise retain their most recent atomic steps.

Outside the atomic tail, adjacent entries may collapse only when all of these
conditions hold:

- They have the same non-empty `EditMergeKey`.
- They target the same model, logical document, editing control, and semantic
  operation kind.
- Text ranges are contiguous when the operation is textual.
- No focus, selection, navigation, savepoint, or explicit command boundary
  separates them.
- Neither entry is structural or destructive.

Compaction composes the stored reversible edits in execution order and keeps
the oldest before-location, newest after-location, and a combined label.
Compaction changes Undo granularity only after entries leave the atomic tail; it
does not discard the data required for Redo.

The model history retains at most 1,024 entries after compaction. When the
limit is exceeded, complete oldest entries are removed. The boundary constants
are named internal policy constants and can be tuned from measured usage
without changing history semantics.

### Savepoints and dirty state

Each distinct model state receives an opaque `HistoryStateId`. The session
tracks:

```rust
current_state: HistoryStateId
saved_state: HistoryStateId
```

The model is dirty exactly when those IDs differ.

Saving does not clear Undo or Redo. A successful save sets
`saved_state = current_state`, subject to the existing stale-save guard: a save
of an older revision cannot mark a newer current state clean.

Undoing back to the exact saved state clears the dirty indicator. Undoing past
it or redoing away from it marks the model dirty. A new edit made after Undo
may discard a Redo branch containing the saved state; the current state remains
dirty because its ID differs from the retained saved-state ID.

Compaction never crosses a reachable saved-state boundary. Dropping history
older than the bounded retention limit may make a saved state unreachable, but
does not change the dirty comparison.

Opening, closing, replacing, or creating a model clears Undo, Redo, Back, and
Forward and establishes the loaded state as the new saved state. Theme
rehydration does not clear history.

### Logical view locations

View history stores stable logical locations, never widget references or
transient `LiveId`s:

```rust
pub struct ViewLocation {
    pub document: DocumentLocator,
    pub anchor: ViewAnchor,
}

pub enum ViewAnchor {
    None,
    Text {
        caret: TextPosition,
        selection: Option<TextRange>,
        scroll_y: f64,
    },
    Markdown {
        fragment: Option<String>,
        scroll_y: f64,
    },
    Diagram {
        selection: Option<ElementLocator>,
        viewport: DiagramViewport,
    },
}
```

`DocumentLocator` carries the logical concept identity plus the document/view
kind needed to distinguish, for example, a classifier preview from its source
view. It must contain enough information for the existing document factories
to reopen the same logical view after its preview tab has been replaced.

Views expose two narrow operations:

- capture their current `ViewAnchor`;
- restore a compatible `ViewAnchor` after activation and layout.

Unsupported anchor variants degrade to `ViewAnchor::None`; document activation
still succeeds.

### Recording view history

`ViewHistory` is a bounded linear sequence plus a cursor. It retains at most
256 logical locations.

A location is recorded for:

- opening a different preview document;
- replacing the shared preview slot with a different logical document;
- manually activating another tab;
- selecting a document through the tab switcher;
- explicit tree, breadcrumb, or Markdown document navigation;
- explicit same-document fragment or semantic-anchor navigation;
- closing the active tab when another logical document becomes active;
- an editor switch performed to reveal an Undo or Redo result.

The following do not create entries:

- promoting or pinning the already-active preview;
- closing an inactive tab;
- reactivating the exact current location;
- passive reconciliation after a model edit;
- caret, selection, scroll, pan, or zoom changes by themselves;
- Back or Forward traversal itself.

Before leaving a location, the shell refreshes the current entry with the
view's latest anchor. This preserves the last caret, selection, fragment,
scroll, or viewport without generating entries for every local movement.

Recording a new location after Back truncates the Forward branch. Consecutive
equal locations are deduplicated.

### Traversing view history

Back and Forward resolve the target `DocumentLocator` against the current
model, activate it through `DocumentHost`, and restore its anchor after the
view is ready.

Restoration affects only the logical view:

- An already-open matching tab is activated.
- A location whose preview was replaced or whose tab was closed is reopened in
  the current shared preview slot.
- Existing pinned tabs, tab order, preview/persistent flags, and unrelated
  closed-tab state are not restored.

History traversal uses an explicit transition cause such as
`HistoryTraversal`. `DocumentHost` still performs its normal activation and
synchronization, while the shell suppresses creation of a reciprocal history
entry and does not clear the opposite branch.

If a target no longer resolves because its subject was deleted, traversal
continues in the requested direction until it finds the next resolvable entry.
Skipped locations are retained because a later Undo may restore their subject.
If no target resolves, the current view remains unchanged and the status bar
reports that no earlier or later available location exists.

### Document-header controls

The shared `DocumentHeader` from the preceding design gains leading Back and
Forward icon buttons before the breadcrumb.

- The controls remain mounted in every active-document header to avoid layout
  movement as history availability changes.
- Each control is disabled when no resolvable target exists in its direction.
- The start screen has no document header or history controls.
- Button activation routes through the application-owned `ViewHistory`; the
  widget never mutates tabs directly.
- Tooltips identify the command and may include the target document title when
  it resolves cheaply.

Navigation controls become a third contributor to header visibility alongside
the breadcrumb and right-dock toggle. Consequently every active document has a
compact header; only the start screen collapses the shared header completely.

Undo and Redo use conventional platform shortcuts:

- Undo: `Cmd+Z` on macOS and `Ctrl+Z` elsewhere.
- Redo: `Cmd+Shift+Z` on macOS; both `Ctrl+Shift+Z` and `Ctrl+Y` elsewhere.

Back and Forward support their header buttons first. Platform-appropriate
keyboard and mouse-side-button bindings may be added through the same command
path without changing history semantics.

### Central command flow

User-originated model edits continue to enter through one application command:

```text
active DocView
  -> ViewOutcome::edit
  -> App edit command with HistoryPresentation
  -> EditorSession reversible application
  -> model-history update
  -> DocumentHost/session synchronization
```

All view changes enter through one navigation command:

```text
tree / breadcrumb / Markdown / preview / tab / Undo reveal
  -> logical ViewLocation
  -> App navigation command with transition cause
  -> DocumentHost transition
  -> anchor restoration
  -> view-history update when the cause is recordable
```

Direct calls to `DocumentHost::transition` from popup, tab, and view-outcome
handlers are migrated to this application command so recording policy cannot
be bypassed accidentally.

## Interaction examples

### Preview replacement and Back

1. `Customer` is active in the shared preview slot.
2. The user previews `Order`; `Order` replaces `Customer`.
3. Back resolves `Customer` and reopens it in the current preview slot.
4. Forward resolves and previews `Order` again.
5. No pinned tab or tab ordering is restored or removed.

### Manual switching and Undo reveal

1. The user edits `Customer` in editor A.
2. They manually activate editor B and make another edit.
3. They activate editor C.
4. Undo applies B's inverse and activates B at the affected selection.
5. That C-to-B switch is recorded in view history.
6. Back returns to C without redoing B's content.

### Redo invalidation

1. The model moves through states A, B, and C.
2. Undo returns from C to B and makes C available through Redo.
3. A new edit creates state D from B.
4. Redo is cleared because the operation that recreated C was derived for B,
   not D.

### Savepoint traversal

1. State B is saved.
2. An edit creates C and marks the model dirty.
3. Undo returns to B and clears the dirty indicator.
4. Another Undo reaches A and marks the model dirty again.
5. Redo returns to B and clears it again.

### Recent typing fidelity

1. Atomic text operations produce `Customerr`.
2. The final insert remains within the 64-entry atomic tail.
3. Undo removes only the last `r`.
4. Much older contiguous inserts with the same merge key may later collapse
   into a larger Undo step.

## Testing

### Reversible-operation tests

- Every OKF and UML operation round-trips authored source through forward then
  inverse application.
- Applying inverse then its reciprocal reproduces the exact forward source.
- Multi-operation batches collect inverses in reverse order.
- Rename, reorder, remove, cascading remove, import, placement, and relationship
  operations restore exact authored data and ordering.
- A failure at any batch index leaves source and inverse output uncommitted.
- Private restoration operations cannot bypass validation.

### Editor-session history tests

- One `PendingEdit` containing several operations creates one Undo entry.
- Undo and Redo move reciprocal edits between stacks.
- A new successful edit clears Redo; a failed edit does not.
- History is global across logical documents.
- Undo and Redo return the correct before/after view location and label.
- The newest 64 entries stay atomic.
- Only compatible older entries compact, and no compaction crosses a selection,
  navigation, structural, or savepoint boundary.
- Retention drops only complete oldest entries.
- Opening or replacing a model clears both model-history stacks.

### Save and dirty-state tests

- Saving preserves Undo and Redo.
- Undoing to the saved state is clean.
- Undoing past or redoing away from the saved state is dirty.
- A stale save cannot mark a newer state clean.
- Discarding a Redo branch containing the saved state remains dirty.
- Theme rehydration preserves histories.

### View-history policy tests

- Preview replacement, manual tab activation, tab-switcher activation, active
  close, explicit navigation, and Undo/Redo reveal record locations.
- Promotion, inactive close, passive synchronization, and repeated activation
  do not.
- Back and Forward traversal do not add entries.
- New navigation after Back clears Forward.
- The current entry captures its latest anchor on departure.
- Consecutive equal locations are deduplicated.
- The 256-entry bound removes complete oldest locations.

### Restoration tests

- Back activates an already-open matching tab.
- Back to a replaced or closed preview reopens it in the shared preview slot.
- Restoration never changes unrelated pinned tabs, ordering, or persistence.
- Source caret/selection, Markdown fragment/scroll, and diagram
  selection/viewport restore when supported.
- Unsupported anchors degrade to document-only activation.
- Deleted targets are skipped without being removed from history.
- Undo restoring a deleted subject can make its retained view-history locations
  resolvable again.

### UI and command tests

- Back and Forward buttons reflect resolvable history availability.
- Header button activation uses the application navigation command.
- Every active document keeps a stable compact header; the start screen does
  not.
- Undo/Redo shortcuts map to the same commands as any future menu surface.
- Status feedback names the undone or redone action.
- Undo/Redo-driven editor switching creates a Back destination.

## Implementation boundaries

The implementation should introduce focused units rather than adding stack and
recording policy directly to `app.rs`:

- `waml::edit`: reversible edit contract and reciprocal `PendingEdit`s.
- OKF/UML/compat operation modules: concrete inverse generation.
- `editor_history`: model-history stacks, state IDs, compaction, limits, and
  savepoint semantics.
- `view_history`: logical locations, cursor, recording policy, traversal, and
  limits.
- `DocumentHost`: capture/restore anchor delegation and transition execution.
- `App`: command orchestration, status feedback, and synchronization only.
- `DocumentHeader`: disabled/enabled Back and Forward presentation only.

No unit besides the application command layer may coordinate both histories.
This keeps model reversibility independently testable and prevents widgets from
bypassing navigation-recording policy.
