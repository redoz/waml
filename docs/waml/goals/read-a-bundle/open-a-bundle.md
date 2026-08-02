# Open a Bundle

**Goal:** A reader opens a bundle from a folder, from a recent entry, or from a
share link.

**Why:** All other functions need an open bundle.

**Done when:** The three entry paths give the same loaded state. A bundle that
does not load causes a message that gives the reason. The window is not empty.
The list of recent bundles stays after a restart.

**Status:** done — unverified
**MVP:** yes

## Notes

- A start screen shows the recent bundles. A reader can pin an entry. A pinned
  entry stays in the first position.
- The web form loads a bundle from a share link. The reader does not install
  software and does not make an account. Refer to [Share a
  Link](../share-and-publish/share-a-link.md).
- Failure messages are weak. A bundle with incorrect content must cause a
  message that gives the name of the file and the reason. Verify this before
  you accept the status above.
