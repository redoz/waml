# Read a Document

**Goal:** A reader reads an OKF document as formatted text, not as source.

**Why:** The bundle contains Markdown. A documentation tool that shows Markdown
source gives no more help than a text editor.

**Done when:** Each construct that this bundle uses draws correctly in the
native form and in the web form. The constructs are headings, paragraphs,
lists, links, inline code, code blocks, tables, and emphasis. A document with
an unknown type draws as formatted text.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The OKF bundle on disk is the semantic root. UML is a selective projection
  above it. This order is the reason that a document with an unknown type draws
  as text and not as an error. It is also the reason that this tool can operate
  on content that is not UML.
- Tables are the probable defect. This goal tree uses many tables. Thus the bar
  needs correct tables.
- A source view shows the text of the document when the reader asks for it.
  That view is an option. It is not the usual path to read a document.
