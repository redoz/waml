# Keep Indexes Correct

**Goal:** Each directory index agrees with the package that it describes.

**Why:** The index is the map for the reader. An index that is not current
hides documents that exist and shows documents that do not exist.

**Done when:** To add, remove, rename, or move a document corrects each index
that the change touches, in the same transaction. A check can show that no
index in the bundle is out of date.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The library has a function that makes each directory index again from the
  model, and golden tests use it. No product code calls it. Thus a person
  maintains the indexes in this bundle by hand.
- An index has one H1, one optional description paragraph, and one flat list of
  members. The regeneration removes all other content.
- The index of the architecture package has hand-written sections at this time.
  A regeneration removes them. To correct that index is part of this goal.
