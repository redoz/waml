# Share a Link

**Goal:** A bundle becomes a link. That link makes the same bundle again in the
browser of a reader.

**Why:** This is the second half of the MVP bar. Without it, the tool is
private.

**Done when:** A link from this bundle opens it again with the same content. A
link that is incomplete or damaged causes a message. The reader installs no
software and makes no account.

**Status:** done — unverified
**MVP:** yes

## Notes

- The library packs a bundle and the architecture documents the round trip.
- A bundle becomes one envelope with a version and a delimiter that uses a
  nonce. To split a packed bundle is unambiguous: the delimiter cannot occur in
  the content, and the version gives the format of the remainder. The previous
  form had no header and calculated the boundaries. Thus a document with
  specific characters could split itself.
- Each surface that packs or unpacks a bundle uses that one codec. The surfaces
  include [Export a Bundle](./export-a-bundle.md) and [Serve
  Locally](./serve-locally.md). A second packing path gives a second set of
  defects.
- The length of a link is the practical limit, and this bundle becomes larger.
  If a full bundle makes a link that a browser or a message tool does not
  accept, this goal needs a separate goal for a hosted form or a form with
  parts.
- The message for a damaged link is not verified.
