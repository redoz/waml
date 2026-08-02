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

- This is the most heavily planned area in the repository. Four plans cover it,
  in dependency order:
  - `2026-07-31-markdown-syntax-platform.md` — one lossless, revisioned
    `waml-syntax` tree as the incremental authority for CommonMark 0.31.2, the
    five named GFM extensions, WAML frontmatter, and WAML section islands.
  - `2026-07-31-markdown-editor-foundation.md` — a standalone WAML-owned
    editing crate: immutable revisioned snapshots, exact edit transactions,
    Unicode-safe selections, input method support, variable-metric layout, and
    viewport virtualization.
  - `2026-07-31-markdown-editor-integration.md` — replacing the read-only
    source surface, and making analysis, canvas, persistence, navigation,
    assets, and the language server consume one revision.
  - `2026-07-31-markdown-presentation-motion.md` — visible syntax, semantic
    typography, embedded blocks, and a shared deterministic 100 ms geometry
    transition. This one has landed commits against tasks 1 through 8.
- `2026-07-31-markdown-bracket-activation.md` covers nested link and image
  labels with CommonMark bracket activation.
- Because the GFM extensions are named in the syntax platform plan, table
  *parsing* is scoped. Table *editing* ergonomics are not, and still deserve a
  leaf once the integration plan lands.
- Byte-fidelity on untouched regions is owned by [Round-Trip
  Losslessly](../trust-the-content/round-trip-losslessly.md), not here.
- The status above is `partial` on the strength of those landed motion tasks.
  The audit should establish how much of foundation and integration is real
  before anyone plans more work here — this is the single most likely place for
  the tree to be wrong in either direction.
