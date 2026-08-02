# Browse the Tree

**Goal:** A reader sees the bundle's structure and opens documents from it.

**Why:** A bundle is a package forest. Without a tree the reader can only reach
documents that something else already linked.

**Done when:** The tree shows every document and package in the bundle, a
single click opens the shared preview tab, a double click makes that tab
permanent, and the tree can be hidden.

**Status:** done — unverified
**MVP:** yes

## Notes

- Preview-tab behavior is settled: a double click on an open preview tab makes
  that tab permanent in place, never duplicating it and never reverting it.
- A narrow window moves the tree above the view; a wide window puts it at the
  side.
- Tree search and filtering do not exist. Not needed for the bar at this bundle
  size; revisit if a bundle outgrows a screenful.
