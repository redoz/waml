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
- `docs/superpowers/plans/2026-07-28-document-header-logical-navigation.md`
  routes tree rows, breadcrumb segments, and rendered Markdown links through
  one logical navigation policy covering documents, directories, and fragments
  — one policy rather than three call sites is what makes this goal finishable.
- `2026-07-31-breadcrumb-tree-reveal.md` makes a breadcrumb click reveal its
  node in the tree without navigating or toggling folders.
- `2026-07-30-document-view-reconciliation.md` keeps a live document view
  compatible across model revisions, which is what stops a navigation from
  landing on a stale view.
- The unverified part is position fidelity: returning to the *document* is
  known to work, returning to the *position* is not confirmed.
