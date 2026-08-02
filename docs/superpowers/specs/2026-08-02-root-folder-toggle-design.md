# Root Folder Toggle Design

## Goal

Make the root directory row in the project tree behave like every other directory row. Each activation changes the root between open and closed.

## Current Behavior

The project tree emits a directory navigation intent when a folder row is activated. `App::navigate_with` sends non-root directory targets to `ProjectTree::toggle_directory`, but it handles `/` separately. The root branch resets navigation state and refreshes the tree, so it does not change the root folder's open state.

## Design

Remove the root-only navigation branch from `App::navigate_with`. Send all directory targets, including `/`, through `ProjectTree::toggle_directory`.

The existing tree widget remains the owner of directory open state. No new action type, source marker, or duplicate toggle path is added.

## Behavior

- Activating an open root directory closes it.
- Activating a closed root directory opens it.
- Root activation does not change the navigation scope, query, filter, active document, or dock state.
- Non-root directory behavior does not change.
- Directory targets from other entry points use the same toggle path.

## Error Handling

`ProjectTree::toggle_directory` keeps its current behavior. It returns `false` and makes no change when the directory address is not present in the active tree.

## Testing

Update the application-level root navigation regression test so it starts with a mounted root directory and verifies both transitions: open to closed, then closed to open. The test also verifies that scope, query, filter, active document, and dock state remain unchanged.

Run the focused editor test first, then the full `waml-editor` library test suite and the project formatting check.
