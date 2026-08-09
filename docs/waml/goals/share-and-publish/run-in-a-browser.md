# Run in a Browser

**Goal:** The same editor operates as a web artifact and shows the same views
as the desktop form.

**Why:** There is one application with two delivery forms. A separate web
viewer is a second product to keep correct.

**Done when:** The web form draws each view that the native form draws. The web
form starts in a time that a reader accepts. A failure causes a message. The
canvas is not empty.

**Status:** partial
**MVP:** yes

## Shipped behavior

#### BROWSER-005 — a failed bundle URL boot shows a visible reason

**Applies to:** browser

**Given** a browser URL names a bundle that cannot be fetched or decoded
**When** the reader opens the URL
**Then** the browser shows a visible reason for the boot failure

**Evidence:** `crates/waml-editor/src/browser_boot.rs:136`, `crates/waml-editor/src/app.rs:902`, and `crates/waml-editor/src/app.rs:1080`

## Unsupported behavior

- BHV-BRW-013 — `origin/main` does not render all diagonal content that the native editor renders.

## Discrepancies

- BHV-BRW-022 — The goal says that a browser failure causes a message. A non-success API boot response only logs the error and returns at `crates/waml-editor/src/app.rs:902` and `crates/waml-editor/src/browser_boot.rs:184`.

## Verification gaps

- BROWSER-005 — target: browser; Host tests cover message construction, but no headed browser test observes the action error for a failed bundle URL boot.

## Notes

- The web build does not use threads because the publication service does not
  provide cross-origin isolation headers.
- [FG-001](../../waml-feature-gaps.md#fg-001--platform-and-capability-predicates)
  records the missing typed platform and capability predicates. The
  `Applies to` field is the current workaround.
- [FG-010](../../waml-feature-gaps.md#fg-010--scenario-to-evidence-traceability)
  records the manual scenario-to-evidence traceability seam.
