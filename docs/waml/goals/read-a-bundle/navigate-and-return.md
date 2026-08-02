# Navigate and Return

**Goal:** A reader follows a link and comes back.

**Why:** A bundle is a graph. Reading one is a walk, and a walk without a way
back is a maze.

**Done when:** Clicking a link in prose or a node in a diagram opens the target,
back returns to the exact previous position including scroll and selection, and
forward returns again.

**Status:** done — unverified
**MVP:** yes

## Notes

- Navigation history is bounded, so a long session cannot grow it without
  limit.
- Tree rows, breadcrumb segments, and rendered Markdown links all resolve
  through one navigation policy, covering documents, directories, and fragments
  within a document. Three call sites with three behaviors is how "click a
  link" becomes untestable; one policy is what makes this goal finishable.
- Revealing is not navigating. A breadcrumb click shows where the current
  document sits in the tree. It does not open anything and does not toggle a
  folder open or closed.
- A live view survives a model revision when the revision is compatible with
  it. When it is not, the view is torn down and rebuilt through the full
  lifecycle rather than patched — a half-reconciled view is how navigation
  lands on stale content.
- The unverified part is position fidelity: returning to the *document* is
  known to work, returning to the *position* is not confirmed.
