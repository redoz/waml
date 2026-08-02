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
- Bundle Envelope v1 — one versioned, nonce-delimited codec replacing
  headerless Markdown splitting — is specified in
  `docs/superpowers/plans/2026-07-31-bundle-envelope-v1.md` and is a
  prerequisite for [Export a Bundle](./export-a-bundle.md) and
  [Serve Locally](./serve-locally.md). Whether it has landed is the first thing
  to check when auditing this row.
- Link length is the practical ceiling, and this bundle is growing. If a
  full-bundle link exceeds what a browser or a chat client accepts, the goal
  needs a leaf for a hosted or chunked form.
- Corruption reporting is the unverified half.
