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

- The accepted dialect is CommonMark 0.31.2 plus five GFM extensions —
  tables, task lists, strikethrough, autolinks, and footnotes — plus WAML
  frontmatter and WAML section islands. Anything outside that is not Markdown
  this tool claims to handle.
- One lossless, revisioned syntax tree is the incremental authority for that
  dialect. The editor, the analysis, the canvas, persistence, navigation, and
  the language server all read the same revision. A surface holding its own
  copy of the text is how an editor and a diagram come to disagree about what
  the document says.
- The editing model is transactional over immutable snapshots: an edit is an
  exact transaction against a known revision, not a mutation of a buffer.
  Selections are Unicode-safe, input methods work, layout handles variable
  metrics, and the viewport is virtualized so document length does not decide
  frame time.
- Source is presented, not exposed. Syntax stays visible, typography carries
  meaning, embedded blocks render in place, and geometry changes animate over
  one shared, deterministic duration rather than snapping.
- Table *parsing* is in the dialect above. Table *editing* — moving between
  cells, adding a row, keeping the pipes aligned — is not, and deserves its own
  leaf. Editing a table as raw pipes is technically sufficient and practically
  miserable.
- Byte-fidelity on untouched regions is owned by [Round-Trip
  Losslessly](../trust-the-content/round-trip-losslessly.md), not here.
- This is the single most likely place for the tree to be wrong in either
  direction. Presentation work has landed; how much of the editing foundation
  is real is the first thing an audit should settle.
