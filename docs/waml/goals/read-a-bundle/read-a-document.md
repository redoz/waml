# Read a Document

**Goal:** A reader reads an OKF document as formatted prose, not as source.

**Why:** The bundle is Markdown. A documentation tool that shows Markdown
source is a text editor.

**Done when:** Every construct this bundle uses — headings, paragraphs, lists,
links, inline code, code blocks, tables, emphasis — renders correctly in the
native and web forms, and a document with no recognised type still renders as
prose.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The OKF bundle on disk is the semantic root. UML is a selective projection
  over it, not the other way around. That ordering is why an unrecognised type
  degrades to prose rather than to nothing, and it is the structural reason
  this tool can grow past UML at all.
- Tables are the likely gap. This goal tree uses them heavily, which makes them
  MVP-blocking by the dogfood bar.
- A source view exists for reading the underlying text on demand. That is a
  convenience, not the reading path.
