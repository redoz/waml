# Breadcrumb tree reveal

**Date:** 2026-07-31
**Status:** Approved design

## Context

Breadcrumb segments currently emit the same navigation action as tree rows and
Markdown links. A directory breadcrumb therefore toggles that folder in the
project tree. This can close the folder that contains the active document, even
when the tree is hidden. A breadcrumb is more useful as a locator for the
logical hierarchy that it describes.

## Goals

- Make every breadcrumb segment locate its logical node in the project tree.
- Keep breadcrumb activation separate from document and directory navigation.
- Open the project tree and keep it open after activation.
- Expand only the target node's ancestor directories.
- Select the target row and smoothly scroll it into view.
- Pulse the revealed row so that the pointer-to-tree relationship is clear.
- Preserve normal tree-row and Markdown-link navigation.

## Non-goals

- Changing the breadcrumb labels, layout, elision, or hit rectangles.
- Changing tree-row or Markdown-link behavior.
- Changing the clicked directory's own fold state.
- Adding a general-purpose application animation framework.
- Changing the pinned Makepad dependency.

## Design

### A separate header action

`DocumentHeaderAction` replaces its breadcrumb `Navigate` variant with a
`RevealInTree(NavigationTarget)` variant. Segment hit testing and hover feedback
remain unchanged. The application handles this action in the existing document
header action stage, but does not pass it to `handle_navigation_intent`.

Breadcrumb targets are canonical logical document or directory targets. If a
future breadcrumb contains another target type, the reveal request fails
without changing the document or tree.

### Application-owned dock coordination

The application asks the project tree to accept the target first. It changes
dock state only when the tree accepts the reveal. In wide mode, the tree moves
to `DockState::Pinned`. In narrow mode, the tree moves to `Pinned` and the
inspector moves to `Flag`, which preserves the existing one-open-side-panel
rule. If the tree is already pinned, its state does not change.

The tree stays open after the reveal. The reveal does not change the active
document, tab history, navigation history, navigator scope, or status message.

### Tree-owned reveal state

`ProjectTree` owns one reveal operation:

```rust
pub fn reveal_target(
    &mut self,
    cx: &mut Cx,
    target: &NavigationTarget,
) -> bool;
```

The operation maps a document target to its concept key and a directory target
to its address. It rejects targets that do not exist in the current canonical
tree.

For a valid target, the tree:

1. Finds each ancestor directory in the canonical tree.
2. Opens closed ancestors without animation.
3. Leaves the target directory's own open state unchanged.
4. Sets the selected key to the target key.
5. Records a pending scroll and pulse for that key.
6. Requests a redraw.

Opening ancestors without fold animation makes the target row available in the
next draw. It also avoids a race between fold animation and row geometry.

The selected row remains selected until normal document-shell synchronization
selects a later active document or another breadcrumb reveal selects another
row.

### Scroll and pulse

During tree drawing, the existing selected-row overlay records an area for the
pending reveal key. After the file tree completes that draw, `ProjectTree`
sends Makepad's existing `scroll_focus_nav` trigger from the row area to the
file-tree area. The file tree then uses its existing smooth
`ScrollBars::scroll_into_view` path. No Makepad API or dependency revision is
required.

The tree draws a second transient overlay on the revealed row. A local
`NextFrame` loop reduces its strength over a short fixed interval. The pulse is
a brief accent wash over the normal selection highlight. It does not move or
resize the row. A repeated reveal restarts the pulse.

## Failure behavior

- A missing target leaves dock, folds, selection, and active document
  unchanged.
- A non-tree target is ignored.
- If the row is not produced after the requested redraw, the pending scroll
  expires and does not loop.
- Repeated activation is safe and restarts the scroll and pulse.

## Testing

### Header tests

- A breadcrumb hit emits `RevealInTree` with the original target.
- Existing history and right-dock actions are unchanged.

### Tree tests

- Revealing a nested document opens all ancestor directories.
- Revealing a directory opens its parents but preserves its own fold state.
- Revealing a target selects its key and records one pending scroll.
- Missing and unsupported targets do not mutate tree state.
- A repeated reveal restarts the pulse.

### Application tests

- Breadcrumb reveal does not activate a document or add view history.
- Wide reveal pins the tree.
- Narrow reveal pins the tree and closes the inspector.
- Tree-row and Markdown directory activation still use the shared toggle path.

### Runtime verification

- A deeply nested crumb opens the tree, reveals the row in the viewport, and
  shows a short pulse.
- The target folder does not expand or collapse unless it is an ancestor.
