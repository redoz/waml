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

- A generic OKF view exists for untyped documents, so an unknown type degrades
  to prose rather than to nothing.
- Tables are the likely gap. This goal tree uses them heavily, which makes them
  MVP-blocking by the dogfood bar.
- A source view exists for reading the underlying text on demand. That is a
  convenience, not the reading path.
