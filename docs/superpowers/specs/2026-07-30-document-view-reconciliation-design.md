# Document View Reconciliation Design

## Purpose

Keep each open document tab consistent with the current analyzed model.
Preserve the temporary state of a compatible live view.
Replace a live view only when its document identity or view type changes.

## Problem

After a model edit, the editor prepares each open document again.
`DocumentHost::reconcile_documents` currently installs every prepared view.
It does this even when the tab ID and view type do not change.

This replacement removes temporary per-tab state.
For a class diagram, this state includes the properties mode and expanded cards.
The replacement starts in canvas mode.
The shared properties widget can remain visible.
The replacement then ignores later property actions because it is not in properties mode.

Other views also contain temporary state.
The reconciliation rule must apply to all document views.

## Requirements

- Keep the current view when the tab ID and view identity are unchanged.
- Update the tab title and presentation from the prepared document.
- Refresh a retained view with `after_session_change`.
- Replace the view when the tab ID or view identity changes.
- For an active replacement, deactivate the old view.
- Activate and fully synchronize the new active view.
- Do not treat passive model reconciliation as user navigation.
- Preserve tab order, active-tab selection, and preview or persistent state.

## View Identity Contract

Add a stable view-identity value to the `DocView` contract.
Each concrete view returns its identity.

The identity describes the live surface and its fixed configuration.
The initial set covers:

- Class diagram
- Behavior flow
- Behavior interaction
- Classifier preview with its navigation category
- Generic OKF Markdown
- Source Markdown

The host uses the view identity only to decide if it can retain a live view.
The host does not copy implementation-specific state.

## Reconciliation

For each prepared document:

1. Match it with the open tab at the same position.
2. Keep the current preview or persistent state.
3. Compare the prepared tab ID and view identity with the current values.
4. If both values match:
   - Update the tab title and presentation.
   - Keep the current view object.
   - Discard the prepared view.
5. If either value differs:
   - Remove the old view.
   - Install the prepared tab and view.
   - Update the active tab ID when necessary.

The reconciliation result records whether the active view was replaced.

After reconciliation:

- If the active view was retained, call `after_session_change`.
- If the active view was replaced, call `on_deactivate` on the removed view.
- Then call `on_activate` and `sync` on the replacement.

Inactive replacement views do not use lifecycle hooks.
They receive a full synchronization when the user activates their tab.

## Error and Missing-Document Behavior

A missing prepared document keeps the current tab and view.
This matches the current behavior.
The change does not add automatic tab closure.

The host must not retain a view when its identity is different.
This rule prevents a diagram view from representing a behavior or preview document.

## Tests

Add host-level tests for these rules:

- Compatible reconciliation keeps the same live view and its temporary state.
- Compatible reconciliation updates title and presentation.
- An incompatible active replacement calls deactivate, activate, and full synchronization.
- An inactive replacement does not change the shared body.

Add an application-level regression test:

1. Open a class diagram.
2. Open the diagram properties panel.
3. Change one property.
4. Do not close the panel.
5. Change a second property.
6. Confirm that both edits change the session model.
7. Confirm that the second value is present in the source text.
8. Confirm that the properties panel remains open.

The existing single-view and single-action-batch property tests remain.
They do not replace the application-level test.

## Out of Scope

- Restoring temporary state after a real view-identity change.
- Closing tabs whose documents no longer resolve.
- Changing view-history behavior.
- Changing the persistence or save workflow.
