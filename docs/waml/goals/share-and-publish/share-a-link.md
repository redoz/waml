# Share a Link

**Goal:** A bundle packs into a link, and that link rebuilds the same bundle in
a reader's browser.

**Why:** This is the second half of the dogfood bar. Without it the tool is
private.

**Done when:** A link produced from this bundle reopens it with identical
content, a link that is truncated or corrupt says so, and no installation or
account is needed to read one.

**Status:** done — unverified
**MVP:** yes

## Notes

- Share packing and the bundle envelope exist in the library, with a share
  round-trip documented as a workflow.
- Link length is the practical ceiling, and this bundle is growing. If a
  full-bundle link exceeds what a browser or a chat client accepts, the goal
  needs a leaf for a hosted or chunked form.
- Corruption reporting is the unverified half.
