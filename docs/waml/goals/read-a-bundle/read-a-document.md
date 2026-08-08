# Read a Document

**Goal:** A reader reads a Markdown document as formatted, read-only content.

**Why:** Formatted content is easier to read than Markdown source.

**Done when:** A document opens in read-only Markdown presentation, the visible
content shows the current document at the reader's current position, and an
external replacement maps the selection and scroll anchor without retained
motion.

**Status:** done
**MVP:** yes

## Shipped behavior

#### NATIVE-022 — a Markdown document opens in read-only presentation

**Applies to:** shared

**Given** the open bundle contains a Markdown document
**When** the reader opens that document
**Then** the editor shows the document as formatted Markdown
**And** text input does not change the document source

**Evidence:** `crates/waml-editor/src/generic_okf_view.rs:34` `crates/waml-editor/src/source_view.rs:236`

#### NATIVE-045 — external replacement maps the reading position

**Applies to:** shared

**Given** a reader has a selection and scroll position in a Markdown document
**When** an external replacement changes that document
**Then** the editor maps the selection to the replacement and keeps the scroll position
**And** the editor stops retained layout motion

**Evidence:** `crates/waml-editor/tests/markdown_integration.rs::external_replacement_maps_selection_and_scroll_and_cuts_motion`

#### NATIVE-061 — the reading surface shows the current document and position

**Applies to:** shared

**Given** a Markdown document is open in reading presentation
**When** the reader views the document
**Then** the reading surface shows the current document at the reader's current position

**Evidence:** `crates/waml-markdown-editor/src/widget.rs::draw_walk_with_session`

## Verification gaps

- NATIVE-022 — target: native; The cited native integration test proves that read-only input is rejected and source text is unchanged, but it does not assert rendered Markdown presentation.
- NATIVE-061 — target: native; No native test checks the full visible reading surface.

## Notes

- Preview and permanent tab behavior is owned by
  [Work with Tabs](./work-with-tabs.md).
- Diagram presentation is owned by [Read a Diagram](./read-a-diagram.md) and the
  goal for each diagram kind.
