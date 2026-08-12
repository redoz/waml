# Command-Line Tool

**Goal:** A person validates, formats, queries, and changes a bundle from a
shell or a build step.

**Why:** A continuous integration job cannot open a window. A repository that
keeps documentation as source needs commands that operate without a window.

**Done when:** The tool validates, formats, queries, bundles, and changes WAML
content. Failures return a non-zero status and positioned diagnostics when
applicable. Batch changes do not write partial results.

**Status:** done
**MVP:** no

## Shipped behavior

#### CLI-001 — validation reports positioned errors and exits non-zero

**Applies to:** native

**Given** a bundle contains an analysis error
**When** the author runs the check command
**Then** the command reports a positioned diagnostic and returns a non-zero status

**Evidence:** `crates/waml-cli/tests/cli_e2e.rs::check_reports_malformed_claimed_uml_from_parser_analysis`

#### CLI-002 — formatting is canonical and idempotent

**Applies to:** native

**Given** a bundle is not in canonical format
**When** the author formats it two times
**Then** the second format pass changes no byte

**Evidence:** `crates/waml-cli/tests/cli_e2e.rs::fmt_canonical_output_is_idempotent`

#### CLI-004 — add an attribute with the direct command

**Applies to:** native

**Given** a bundle contains a target that has no requested attribute
**When** the author runs the direct attribute-add command
**Then** the command writes the attribute and refuses a duplicate attribute

**Evidence:** `crates/waml-cli/tests/cli_e2e.rs::attr_add_writes_the_file` and `crates/waml-cli/tests/cli_e2e.rs::duplicate_attr_exits_1`

#### CLI-005 — apply an NDJSON batch atomically

**Applies to:** native

**Given** an NDJSON operation batch contains a later invalid operation
**When** the author applies the batch
**Then** the command reports failure and writes no part of the batch

**Evidence:** `crates/waml-cli/tests/cli_e2e.rs::apply_late_multi_file_failure_writes_nothing`

#### CLI-006 — show a resolved classifier

**Applies to:** native

**Given** a bundle contains a classifier
**When** the author queries that classifier with the show command
**Then** the command returns the resolved classifier

**Evidence:** `crates/waml-cli/tests/cli_e2e.rs::show_json_and_refs_share_prepared_referrer_results`

#### CLI-007 — list classifier referrers

**Applies to:** native

**Given** other bundle elements refer to a classifier
**When** the author runs the refs query for that classifier
**Then** the command returns its resolved referrers

**Evidence:** `crates/waml-cli/tests/cli_e2e.rs::show_json_and_refs_share_prepared_referrer_results`

#### CLI-008 — list classifiers with an optional type filter

**Applies to:** native

**Given** a bundle contains classifiers of one or more types
**When** the author runs the list command with an optional type filter
**Then** the command returns the matching classifiers

**Evidence:** `crates/waml-cli/src/main.rs:1075`

#### CLI-009 — bundle a directory as JSON or TypeScript

**Applies to:** native

**Given** a directory contains WAML Markdown files
**When** the author runs the bundle command with JSON or TypeScript output
**Then** the command writes the requested bundle artifact

**Evidence:** `crates/waml-cli/src/main.rs:988` and `crates/waml-cli/src/commands.rs:148`

#### CLI-010 — format check rejects noncanonical content

**Applies to:** native

**Given** a bundle is not in canonical format
**When** the author runs `fmt --check`
**Then** the command returns a non-zero status without formatting the bundle

**Evidence:** `crates/waml/src/fmt.rs::plan_fmt` and `crates/waml-cli/src/main.rs::is not formatted`

#### CLI-011 — direct commands change nodes, values, and relationships

**Applies to:** native

**Given** a bundle has a valid direct node, value, or relationship change
**When** the author runs the corresponding mutation command
**Then** the command applies that change to the bundle

**Evidence:** `crates/waml-cli/src/main.rs:726`, `crates/waml-cli/src/main.rs:575`, `crates/waml-cli/src/main.rs:660`, and `crates/waml-cli/src/main.rs:675`

## Verification gaps

- CLI-008 — target: native; The List command is source-evidenced, but no targeted CLI E2E test asserts its output and type filtering.
- CLI-009 — target: native; The Bundle command is source-evidenced, but no targeted CLI E2E test asserts JSON and TypeScript artifact output.
- CLI-010 — target: native; The Fmt command source sets a non-zero check result for changed files, but no CLI E2E asserts the noncanonical fmt --check exit.
- CLI-011 — target: native; The mutation dispatch is source-evidenced, but the cited CLI E2E covers only the separate attribute-add row.

## Notes

- The editor and CLI use the same core analysis and edit services. They expose
  different operations and do not claim command-for-control parity.
- The share command is owned by [Share a Link](../share-and-publish/share-a-link.md#cli-003-—-create-a-share-fragment-or-url).
