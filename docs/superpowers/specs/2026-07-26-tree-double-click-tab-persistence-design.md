# Tree Double-Click Tab Persistence

## Goal

Tree items that open documents use consistent preview semantics:

- A single click opens or focuses the item in the shared preview slot.
- A double click opens or focuses the item and makes its tab persistent.
- Double-clicking an already open item is idempotent and leaves it persistent.
- Classifiers and diagrams follow the same rules.
- Folder double-clicks remain unchanged.

The initially selected diagram also starts as a preview.

## Input flow

Makepad's `FingerDownEvent` already supplies `tap_count`, but `FileTree`
reduces every file press to `FileClicked(LiveId)` and does not carry the count
into that action. `ProjectTree` will retain the latest primary file-area
`tap_count` until it handles the corresponding `FileClicked` action.

`ProjectTree` will emit separate single- and double-click document actions.
The application will resolve the item key and title, open or focus its preview
tab, and promote the resulting tab for a double click.

No Makepad API or dependency revision is required.

## Tab model

The existing single preview slot is shared by classifiers, diagrams, and source
tabs. Diagram tabs receive IDs derived from their diagram keys, just as
classifier and source tab IDs are derived from their keys. This prevents
duplicates while allowing multiple diagrams to remain open after promotion.

Opening an item follows these rules:

1. If its tab already exists, activate it without changing persistent tabs
   back into previews.
2. Otherwise, replace the current preview slot or append a new preview when no
   preview exists.
3. For a double click, promote the returned tab ID.

Model startup opens the selected diagram through this preview path rather than
creating a special permanent diagram base. Diagram tree selection and the
diagram switcher also use the same preview-opening operation.

## State and view synchronization

When a preview slot is replaced, its old cached document view is removed before
the active tab is synchronized. Existing persistent tabs retain their cached
views. Opening an already open item reuses its stable tab identity and view.

The tab strip, active view, tree highlight, inspector, and diagram switcher are
refreshed through the existing application synchronization methods.

## Edge cases

- An unknown tree key remains a no-op.
- A double click whose first click focused an already open tab still promotes
  that same tab.
- Repeated double clicks remain safe because promotion is idempotent.
- Closing all tabs remains supported; selecting any document creates a new
  preview slot.
- Folder behavior is not changed.

## Testing

Unit tests will cover:

- Tap-count classification into single- and double-click document actions.
- Startup diagrams are previews.
- A diagram preview is replaced by the next preview.
- Promoted diagrams coexist and use distinct stable IDs.
- Reopening promoted classifier and diagram tabs does not duplicate or demote
  them.
- Double-click promotion is idempotent.

Focused editor tests will run first, followed by the broader workspace test
suite appropriate to the affected crates.
