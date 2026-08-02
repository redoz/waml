# Edit Prose

**Goal:** An author writes the Markdown body of a document in the editor.

**Why:** Most of a documentation bundle is prose. This is the single largest
gap between the product today and the dogfood bar.

**Done when:** An author can write and revise every construct this bundle uses
— headings, paragraphs, lists, links, inline code, code blocks, tables,
emphasis — with a caret, a selection, keyboard navigation, and clipboard, and
the result round-trips byte-for-byte through the parser when unchanged.

**Status:** partial — unverified
**MVP:** yes

## Notes

- A Markdown editing crate and Markdown hosts exist. How complete the editing
  surface is — selection, clipboard, multi-line editing, undo granularity — is
  the unverified part and the most likely source of MVP-blocking work.
- Table editing deserves its own leaf once this is audited. Editing a Markdown
  table as raw pipes is technically sufficient and practically miserable.
- Byte-fidelity on untouched regions is owned by [Round-Trip
  Losslessly](../trust-the-content/round-trip-losslessly.md), not here.
