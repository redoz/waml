# Keep Indexes Correct

**Goal:** Every directory index matches the package it describes.

**Why:** The index is the reader's map. A stale index hides documents that
exist and promises documents that do not.

**Done when:** Adding, removing, renaming, or moving a document updates every
affected index in the same transaction, and a check can prove that no index in
the bundle is stale.

**Status:** partial — unverified
**MVP:** yes

## Notes

- `waml::index_md::reindex_source` rebuilds every directory index from the
  model and is exercised by golden tests, but no product code calls it. Indexes
  in this bundle are therefore hand-maintained today.
- An index carries exactly an H1, an optional description paragraph, and a flat
  member list. Anything else is drift that reindexing will discard.
- `docs/waml/architecture/index.md` is currently drifted — it carries
  hand-written prose sections. Repairing it is part of this goal.
