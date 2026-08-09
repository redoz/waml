# Language Server

**Goal:** A text editor gets diagnostics, document structure, navigation, and
semantic highlighting for WAML documents.

**Why:** An author who prefers a different editor must not lose access to WAML.
The language server gives that access.

**Done when:** The server publishes diagnostics and answers document-symbol,
document-link, definition, and full-document semantic-token requests from the
current live buffer. Completion remains planned.

**Status:** partial
**MVP:** no

## Shipped behavior

#### LSP-001 — publish diagnostics for an open document

**Applies to:** native

**Given** an editor opens a WAML document with an unresolved target
**When** the language server analyzes the open document over stdio
**Then** it publishes the positioned diagnostic to the editor

**Evidence:** `crates/waml-cli/tests/lsp_e2e.rs::publishes_diagnostics_for_unresolved_target_over_stdio`

#### LSP-002 — answer document-symbol requests

**Applies to:** native

**Given** an open WAML document contains a Unicode-named element
**When** the editor requests document symbols
**Then** the server returns the current document symbol with an exact Unicode range

**Evidence:** `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`

#### LSP-003 — answer document-link requests

**Applies to:** native

**Given** an open WAML document contains a resolvable document link
**When** the editor requests document links
**Then** the server returns the resolved target URI

**Evidence:** `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`

#### LSP-004 — answer cross-document definition requests

**Applies to:** native

**Given** an open WAML document refers to an element in another document
**When** the editor requests the definition
**Then** the server returns the definition URI in the other document

**Evidence:** `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`

#### LSP-005 — answer full-document semantic-token requests

**Applies to:** native

**Given** an open WAML document contains semantic tokens
**When** the editor requests full-document semantic tokens
**Then** the server returns tokens with ranges in the advertised UTF-16 legend

**Evidence:** `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`

## Planned behavior

- BHV-LSP-006 — Completion of element names, relationship names, relationship kinds, and link targets has no passing acceptance scenario.

## Discrepancies

- BHV-LSP-007 — The earlier goal text said that the server could answer from stale disk content. The current live-buffer result is implemented at `crates/waml-cli/src/lsp/server.rs:223` and is observed after an edit by `crates/waml-cli/tests/lsp_e2e.rs::snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio`.

## Notes

- The language server is not diagnostics-only. It provides diagnostics,
  document symbols, document links, definitions, and semantic tokens.
- The query acceptance test changes an open document before it checks symbols,
  links, definitions, and tokens. The results use that current revision.
