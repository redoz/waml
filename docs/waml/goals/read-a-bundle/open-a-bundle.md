# Open a Bundle

**Goal:** A reader gets a bundle on screen from a folder, a recent entry, or a
share link.

**Why:** Everything else in the product is downstream of a bundle being open.

**Done when:** All three entry paths land on the same loaded state, a bundle
that fails to load says why instead of showing an empty window, and the recent
list survives a restart.

**Status:** done — unverified
**MVP:** yes

## Notes

- A start screen lists recent bundles. A reader can pin an entry to hold it in
  first position.
- The web form loads a bundle from a share link with no installation and no
  account. See [Share a Link](../share-and-publish/share-a-link.md).
- Failure reporting on load is the thin part: a malformed bundle should name
  the file and the reason. Audit before trusting the `done` above.
