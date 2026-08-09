# Share a Link

**Goal:** A bundle becomes a link. That link makes the same bundle again in the
browser of a reader.

**Why:** This is the second half of the MVP bar. Without it, the tool is
private.

**Done when:** A link from this bundle opens it again with the same content. A
link that is incomplete or damaged causes a message. The reader installs no
software and makes no account.

**Status:** done
**MVP:** yes

## Shipped behavior

#### CLI-003 — create a share fragment or URL

**Applies to:** native

**Given** an author has a WAML bundle
**When** the author runs the share command
**Then** the command returns a share fragment or a share URL that opens the bundle in a browser

**Evidence:** `crates/waml-cli/src/main.rs:358`

#### BROWSER-015 — a damaged share link shows an error and the start screen

**Applies to:** browser

**Given** a browser URL contains a damaged share-link model
**When** the reader opens the URL
**Then** the browser shows the start screen and a visible reason for the failure

**Evidence:** `crates/waml-editor/src/app.rs:836`

## Verification gaps

- CLI-003 — target: native; The browser E2E invokes waml share --fragment-only as one step, but no native CLI test asserts share fragment or share URL output.
- BROWSER-015 — target: browser; The wasm startup path calls report_action_error after share decoding fails, but no headed browser test observes the start screen and visible action error.

## Notes

- The `CLI-003` browser test is partial cross-surface evidence. It creates a
  fragment with the native CLI and opens the changed model in a browser, but it
  does not satisfy the native CLI verification boundary.
- Browser boot-source priority belongs to [Export a Bundle](./export-a-bundle.md).
  This document does not copy that contract.
