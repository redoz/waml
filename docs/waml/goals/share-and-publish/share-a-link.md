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
- A bundle packs into one versioned, nonce-delimited envelope. Splitting a
  packed bundle is unambiguous by construction: the delimiter cannot occur in
  content, and the version says how to read what follows. The older headerless
  form guessed at boundaries, which meant a document containing the wrong
  characters could split itself.
- Every surface that packs or unpacks a bundle uses that one codec —
  [Export a Bundle](./export-a-bundle.md) and
  [Serve Locally](./serve-locally.md) included. A second packing path is a
  second set of corruption bugs.
- Link length is the practical ceiling, and this bundle is growing. If a
  full-bundle link exceeds what a browser or a chat client accepts, the goal
  needs a leaf for a hosted or chunked form.
- Corruption reporting is the unverified half.
