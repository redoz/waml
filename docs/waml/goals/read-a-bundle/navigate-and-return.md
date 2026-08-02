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
- The unverified part is position fidelity: returning to the *document* is
  known to work, returning to the *position* is not confirmed.
