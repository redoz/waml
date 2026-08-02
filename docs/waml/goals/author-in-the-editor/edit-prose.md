# Edit Prose

**Goal:** An author writes the Markdown body of a document in the editor.

**Why:** Most of a documentation bundle is text. This is the largest difference
between the product today and the MVP bar.

**Done when:** An author writes and changes each construct that this bundle
uses, with a caret, a selection, keyboard navigation, and the clipboard. The
constructs are headings, paragraphs, lists, links, inline code, code blocks,
tables, and emphasis. An unchanged document keeps the same bytes through the
parser.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The accepted dialect is CommonMark 0.31.2, five GFM extensions, WAML
  frontmatter, and WAML section islands. The five extensions are tables, task
  lists, strikethrough, autolinks, and footnotes. This tool does not accept
  other Markdown dialects.
- One lossless syntax tree with revisions is the incremental authority for that
  dialect. The editor, the analysis, the canvas, the persistence, the
  navigation, and the language server read the same revision. If one surface
  keeps its own copy of the text, the editor and the diagram show different
  content.
- The edit model uses transactions above immutable snapshots. An edit is an
  exact transaction against a known revision. An edit is not a change to a
  buffer. Selections are safe for Unicode. Input methods operate. The layout
  accepts variable metrics. The viewport uses virtualization, thus the length
  of the document does not control the frame time.
- The tool presents the source. It does not show raw source only. Syntax stays
  visible, typography shows meaning, embedded blocks draw in position, and a
  change of geometry uses one shared duration. A change of geometry does not
  occur in one step.
- The dialect above includes tables. It does not include the controls to edit a
  table, such as movement between cells, addition of a row, and alignment of
  the pipe characters. Those controls need a separate goal. To edit a table as
  raw pipe characters is sufficient in theory and difficult in practice.
- [Round-Trip Losslessly](../trust-the-content/round-trip-losslessly.md)
  controls byte accuracy in the parts that the author does not touch. This goal
  does not.
- This goal is the most probable error in the tree, in either direction. The
  presentation work operates. An audit must find how much of the edit
  foundation operates before a person plans more work here.
