# Keep Indexes Correct

**Goal:** Each generated directory index agrees with its package.

**Why:** A stale index hides documents that exist and lists documents that do
not exist.

**Done when:** A structural change regenerates each affected index in the same
transaction, and a check reports no stale generated index.

**Status:** partial
**MVP:** yes

## Notes

- `crates/waml/src/index_md.rs::reindex_source` regenerates index source from
  the current package forest.
- `crates/waml/src/index_md.rs::reindex_bundle_creates_index_for_each_directory`
  checks index creation for each directory.
- `crates/waml/tests/golden.rs::nested_packages_round_trip_through_reindex`
  checks nested package output through regeneration.
- The evidence proves library behavior. It does not prove that every editor
  structure change regenerates affected indexes, so the goal stays partial.
- Invalid source proposals and session history are owned by
  [Save and Undo](../author-in-the-editor/save-and-undo.md).
