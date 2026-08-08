# UI Behavior and Architecture Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `docs/waml` the current, testable product contract for all user-visible workflows, and make its architecture views match the six-crate implementation and revisioned data flows on `origin/main`.

**Architecture:** Build one temporary traceability inventory first. Use that inventory to give each behavior and each document one owner. Keep shipped Given-When-Then contracts, the permanent semantic product-use-case model, planned behavior, runtime architecture, OKF trust metadata, and WAML language gaps separate. Reuse the existing WAML parser, formatter, and `waml::index_md::reindex_source` generator for all repository gates.

**Tech Stack:** Rust 2021, Clap, `waml` and `waml-cli`, OKF v0.2 Markdown frontmatter, WAML actor/use-case/class/sequence/activity documents and diagrams, Node.js 22 built-in test runner, GitHub Actions, VS Code/Vitest tests, Makepad native and WebAssembly editor tests.

## Global Constraints

- Work only in a git worktree made from `origin/main`. Never edit `C:\dev\waml` directly.
- Use ASD-STE100 Simplified Technical English in all user-facing text.
- Shipped behavior on `origin/main` is normative. Do not promote a planned control or a discrepancy to a shipped contract.
- Use only these goal status values: `done`, `partial`, `planned`, and `horizon`. Remove `implemented` and every `unverified` suffix.
- Derive goal status from shipped scenario coverage and evidence. Do not estimate status from a first reading.
- Use one normative scenario for shared native/browser behavior. Its verification target is a native Rust test; if that test is absent, keep source-evidenced shipped behavior and record a native verification gap.
- Use browser tests only for browser-only behavior or an explicit native/browser parity seam. A browser-only scenario without browser-specific automation keeps its source-evidenced shipped contract and records a browser verification gap.
- Keep the scenario form and field order exact: identifier, applicability, Given/And, When, Then/And, and evidence.
- Use semantic targets and observable results. Do not use Rust operations or fixed coordinates unless a coordinate value is the contract.
- Keep current bundle-relative document identities and typed WAML frontmatter.
- Use OKF v0.2 `sources`, `generated`, `verified`, `status`, and `stale_after` only for provenance, trust, review state, and freshness.
- Keep product goal status in the Markdown body as `**Status:**`. Never copy `done`, `partial`, `planned`, or `horizon` into OKF frontmatter.
- Preserve a legacy `timestamp` when an existing v0.1 document requires it. For a new v0.2 document, use `generated.at` and do not add `timestamp`.
- Do not add a second Markdown/WAML parser or formatter. A contract checker can scan canonical Markdown headings and fields only after `waml check` succeeds.
- Do not expand the WAML language in this work. Record notation gaps in the feature-gap ledger.
- Keep all GWT scenario bodies in `docs/waml/goals/**`. Product use-case documents link to scenario headings and never copy GWT lines.
- Do not add product-use-case layout geometry or renderer requirements. Specialized actor, use-case, and system-boundary rendering is separate user work.
- Every editor launch must use `run.ps1 -Title` with a short kebab-case value. Add a six-digit `-Color` value if two task windows are open at the same time.
- Use `rtk` before shell commands. Run all commands from the worktree root unless a step gives another directory.
- Each task ends with a focused commit. Do not combine unrelated work streams in one commit.

## Dependency and Ownership Map

Run Tasks 1 and 2 in order. After Task 2 freezes the inventory, start the automation and architecture lanes:

- Automation lane: Tasks 3 and 4 in order.
- Architecture lane: Tasks 13 and 14 in order.

Task 15 waits for Tasks 2 and 3 because both can edit `crates/waml-cli/tests/cli_e2e.rs`; execute Task 15 before authoring goal scenarios so the inventory distinguishes verified tests from gaps. Task 5 waits for Task 4. Tasks 6, 7, 8, 9, and 10 wait for Tasks 4, 5, and 15, then run in parallel. Task 11 waits for Tasks 2, 5, 6, 7, 8, 9, and 10. Task 12 waits for Tasks 6 through 11. Task 16 waits for Tasks 3, 4, 12, and 14. Tasks 17, 18, and 19 run in order after every lane is complete.

Concurrent agents must use these boundaries:

| Owner | Exclusive files while its lane runs |
| --- | --- |
| Inventory coordinator | `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.schema.json`, `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`, and the inventory reports created by later streams |
| Index CLI | `crates/waml-cli/src/main.rs`, `crates/waml-cli/src/commands.rs`, `crates/waml-cli/src/io.rs`, `crates/waml-cli/tests/cli_e2e.rs` |
| Contract checker | `scripts/check-waml-doc-contract.mjs`, `scripts/check-waml-doc-contract.test.mjs` |
| Contract and gap ledger | `docs/waml/documentation-contract.md`, `docs/waml/waml-feature-gaps.md`, `docs/superpowers/audits/reports/contract.md` |
| Read/shell goals | `docs/waml/goals/read-a-bundle/**`, `docs/superpowers/audits/reports/read-shell.md` |
| Author/trust goals | `docs/waml/goals/author-in-the-editor/**`, `docs/waml/goals/trust-the-content/**`, `docs/superpowers/audits/reports/author-trust.md` |
| Class/shared goals | `docs/waml/goals/uml/class/**`, `docs/waml/goals/uml/shared/**`, `docs/superpowers/audits/reports/class-shared.md` |
| Behavior goals | `docs/waml/goals/uml/activity/**`, `docs/waml/goals/uml/sequence/**`, `docs/waml/goals/uml/state-machine/**`, `docs/waml/goals/uml/use-case/**`, `docs/superpowers/audits/reports/behavior-diagrams.md` |
| Browser/tool goals | `docs/waml/goals/share-and-publish/**`, `docs/waml/goals/tooling-around-the-repo/**`, `docs/superpowers/audits/reports/browser-tooling.md` |
| Product use-case model | `docs/waml/use-cases/**`, `docs/waml/waml-feature-gaps.md` after Task 5 stops, and `docs/superpowers/audits/reports/use-cases.md` |
| Goal integrator | `docs/waml/goals/index.md`, `docs/waml/goals/root-goal.md`, `docs/waml/goals/mvp.md`, `docs/waml/goals/beyond-uml.md`, plus all goal `index.md` files after the five goal agents stop |
| Architecture | `docs/waml/architecture/**`, `docs/superpowers/audits/reports/architecture.md` |
| Evidence coordinator | Only the test files listed in Task 15 and `docs/superpowers/audits/reports/evidence.md` |
| CI/documentation gate | `.github/workflows/ci.yml`, `README.md` |
| Final integrator | `docs/waml/index.md` and every generated `index.md` after all other agents stop; it also deletes the exact temporary audit files in Task 19 |

No goal agent edits test files. No evidence agent changes scenario prose. The product-use-case agent does not edit goal files or tests. No agent runs the writing form of `waml index` until Task 17.

## Audit Inventory Contract

The inventory is JSON Lines. One line is one behavior. The line object has this exact shape:

```json
{
  "behavior_id": "BHV-WEB-001",
  "area": "browser boot",
  "workflow": "Open an exported site",
  "goal_document": "docs/waml/goals/share-and-publish/run-in-a-browser.md",
  "applicability": "browser",
  "state": "shipped",
  "implementation_evidence": [
    {
      "path": "crates/waml-editor/src/browser_boot.rs",
      "symbol": "select_browser_boot",
      "start_line": 48
    }
  ],
  "test_evidence": [
    {
      "path": "crates/waml-editor/src/browser_boot.rs",
      "test": "share_fragment_beats_api",
      "scenario_marker": false
    }
  ],
  "scenario_id": "WEB-001",
  "scenario_id_origin": "allocated",
  "verification_boundary": "browser",
  "verification_state": "verified",
  "verification_gap": null,
  "feature_gap_ids": ["FG-001"],
  "discrepancy": null,
  "notes": "A share fragment has priority over an API query."
}
```

Use these value rules:

- `behavior_id` matches `^BHV-[A-Z]+-[0-9]{3}$` and never changes after Task 2.
- `area` is one of: `bundle lifecycle`, `shell`, `navigation`, `tabs`, `Markdown reading`, `Markdown editing`, `session state`, `class diagram`, `activity diagram`, `sequence diagram`, `browser`, `CLI`, `LSP`, or `VS Code`.
- `applicability` is `shared`, `native`, or `browser`.
- `state` is `shipped`, `planned`, `unsupported`, or `discrepant`.
- A scenario identifier matches `^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$`: one or more uppercase segments followed by a numeric suffix. Preserve existing identifiers such as `SEQ-MSG-1`, `SEQ-ORD-1`, and `SEQ-FRAG-10` verbatim. Never normalize or renumber them.
- `scenario_id` and `scenario_id_origin` are required only for `shipped`. They are `null` for the other states. `scenario_id_origin` is `existing` for an identifier already present on `origin/main` and `allocated` for an identifier created by this audit.
- A newly `allocated` identifier matches `^[A-Z][A-Z0-9]*-[0-9]{3}$`. The coordinator allocates these three-digit identifiers in ascending order. The wider canonical grammar exists only to retain stable identifiers.
- `verification_boundary` is the required target, not proof that verification exists. It is `native` for `shared` and `native`. It is `browser` for `browser`.
- `verification_state` is `verified` or `gap` for `shipped` and `not_applicable` for all other states. `verified` requires a test that asserts the observable Then result at `verification_boundary`. `gap` keeps the shipped scenario when implementation or test evidence proves the behavior but the target-boundary test is absent or insufficient.
- `verification_gap` is a complete present-tense sentence when `verification_state` is `gap`; it is `null` otherwise. A missing test is a verification gap, not an implementation/documentation discrepancy.
- A `shipped` row has at least one implementation or test evidence object. A source-only shipped row is valid when `verification_state` is `gap`.
- Each evidence array contains objects with repository-relative paths. A test object names an exact test function or test title.
- `discrepancy` is a complete present-tense sentence for `discrepant`. It is `null` for other states.
- `feature_gap_ids` contains only identifiers from `docs/waml/waml-feature-gaps.md`.

Use these scenario prefixes and owners for newly allocated identifiers. The coordinator allocates three-digit numbers in ascending order and never renumbers an allocated identifier. Existing multi-segment identifiers keep their exact spelling even when the nearest owner prefix is `SEQ`:

| Prefix | Behavior owner |
| --- | --- |
| `BUNDLE` | start, recents, open, close, save, and export |
| `SHELL` | responsive shell, docks, splitters, overlays, popups, and theme |
| `NAV` | tree, folders, breadcrumbs, external links, reveal, and view history |
| `TAB` | preview tabs, pinned tabs, document switching, and presentation switching |
| `MDREAD` | Markdown reading and selection in read-only presentation |
| `MDEDIT` | Markdown editing, clipboard, multi-caret, and IME |
| `SESSION` | undo, redo, savepoints, dirty state, diagnostics, and status feedback |
| `CLASS` | class-diagram selection, tools, manipulation, properties, layout, conflicts, and solver feedback |
| `ACT` | activity rendering, hit testing, selection, and camera behavior |
| `SEQ` | sequence rendering, hit testing, selection, and camera behavior |
| `WEB` | browser boot, URL, share, site, download, API, and local serve behavior |
| `CLI` | command-line workflows |
| `LSP` | language-server workflows |
| `VSC` | VS Code integration workflows |

Each work-stream report has these exact headings: `# Changed files`, `# Scenario identifiers`, `# Evidence`, `# Verification gaps`, `# Open discrepancies`, and `# Feature gaps`. A report lists `None.` under a heading with no entries. A shipped scenario with `verification_state: gap` stays under `# Scenario identifiers` and is also listed under `# Verification gaps`; it never moves to `# Open discrepancies` solely because a test is absent.

## Product Use-Case Traceability Procedure

Run this procedure after `waml check` succeeds. It is a focused audit check,
not a second product parser or a new WAML feature.

1. Enumerate actor leaves from `docs/waml/use-cases/actors/*.md` and use-case
   leaves from `docs/waml/use-cases/workflows/*.md`. Exclude `index.md`.
2. Read the semantic-intention table under `# Evidence` in
   `docs/superpowers/audits/reports/use-cases.md`. Require one use-case leaf for
   each intention. Require one owning goal for each intention. If an intention
   has more than one owner, fail and reconcile the inventory owner before you
   continue.
3. For each use-case leaf, parse the `## Owning goal` and `## Scenarios`
   sections. Require exactly one owning-goal document link. Require every
   scenario link to target that same document and one exact `####` scenario
   heading.
4. Generate the fragment for each target heading with the repository
   `heading_slug` rule from `crates/waml-cli/src/lsp/query.rs`: trim the heading,
   remove its leading `#` characters, trim again, convert to lower case, split
   on white space, and join the parts with `-`. Do not remove punctuation. For
   example, `#### BUNDLE-001 — open a bundle` becomes
   `#bundle-001-—-open-a-bundle`. Require the authored fragment to equal this
   value. `waml check` removes fragments before path resolution, so its success
   does not satisfy this check.
5. Remove the frontmatter range from each actor and use-case leaf, then scan
   body lines with
   `(?i)^\s*(?:[-*>]\s*)?(?:\*\*|__)?(Given|When|Then|And)(?:\*\*|__)?(?:\s|$)`.
   Fail on every match. This catches plain, emphasized, list, and block-quote
   copies of a GWT body.
6. Parse each view into its frontmatter, `##` sections, `###` member groups,
   and member links. Require `type: Diagram`, `profile: uml-domain`, one
   `### External actors` group, and one named product-boundary group. Reject a
   parsed `## Layout` section. Because WAML layout records parse only in that
   section, this rejects coordinates, sizes, routes, row/column rules, relative
   ordering, frames, and all other layout statements without a keyword search.
7. Resolve all member links. Compare the set of actor links under external
   actor groups with the complete actor-leaf set. Compare the set of links
   under product-boundary groups with the complete use-case-leaf set. Both set
   differences must be empty. A leaf can occur in more than one view.
8. Under the report's `# Evidence` heading, record for each actor and use-case
   leaf its semantic intention or role, owning goal when applicable, scenario
   identifiers when applicable, and every containing view. Compare this table
   with the parsed sets above.

---

### Task 1: Create the audit schema and inventory native behavior

**Files:**
- Create: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.schema.json`
- Create: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`

**Interfaces:**
- Consumes: shipped code and tests on `origin/main`; the Audit Inventory Contract above.
- Produces: valid JSONL rows for all `shared` and `native` behavior; allocated scenario identifiers; exact evidence paths and test names.

- [ ] **Step 1: Write the JSON Schema before the data**

Use JSON Schema draft 2020-12. Set `additionalProperties` to `false`. Encode every enum and conditional rule from the Audit Inventory Contract. Require at least one implementation or test evidence object for `shipped`. Require target-boundary test evidence only when `verification_state` is `verified`; allow source-only shipped rows when they are explicit verification gaps.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "additionalProperties": false,
  "required": [
    "behavior_id", "area", "workflow", "goal_document", "applicability",
    "state", "implementation_evidence", "test_evidence", "scenario_id",
    "scenario_id_origin", "verification_boundary", "verification_state",
    "verification_gap", "feature_gap_ids", "discrepancy", "notes"
  ],
  "properties": {
    "behavior_id": { "type": "string", "pattern": "^BHV-[A-Z]+-[0-9]{3}$" },
    "area": {
      "enum": [
        "bundle lifecycle", "shell", "navigation", "tabs", "Markdown reading",
        "Markdown editing", "session state", "class diagram", "activity diagram",
        "sequence diagram", "browser", "CLI", "LSP", "VS Code"
      ]
    },
    "workflow": { "type": "string", "minLength": 1 },
    "goal_document": { "type": "string", "pattern": "^docs/waml/goals/.+\\.md$" },
    "applicability": { "enum": ["shared", "native", "browser"] },
    "state": { "enum": ["shipped", "planned", "unsupported", "discrepant"] },
    "implementation_evidence": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "symbol", "start_line"],
        "properties": {
          "path": { "type": "string", "minLength": 1 },
          "symbol": { "type": "string", "minLength": 1 },
          "start_line": { "type": "integer", "minimum": 1 }
        }
      }
    },
    "test_evidence": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "test", "scenario_marker"],
        "properties": {
          "path": { "type": "string", "minLength": 1 },
          "test": { "type": "string", "minLength": 1 },
          "scenario_marker": { "type": "boolean" }
        }
      }
    },
    "scenario_id": {
      "oneOf": [
        { "type": "string", "pattern": "^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$" },
        { "type": "null" }
      ]
    },
    "scenario_id_origin": {
      "oneOf": [
        { "enum": ["existing", "allocated"] },
        { "type": "null" }
      ]
    },
    "verification_boundary": { "enum": ["native", "browser"] },
    "verification_state": { "enum": ["verified", "gap", "not_applicable"] },
    "verification_gap": { "type": ["string", "null"] },
    "feature_gap_ids": {
      "type": "array",
      "uniqueItems": true,
      "items": { "type": "string", "pattern": "^FG-[0-9]{3}$" }
    },
    "discrepancy": { "type": ["string", "null"] },
    "notes": { "type": "string" }
  },
  "allOf": [
    {
      "if": { "properties": { "state": { "const": "shipped" } } },
      "then": {
        "properties": {
          "scenario_id": { "type": "string" },
          "scenario_id_origin": { "enum": ["existing", "allocated"] },
          "verification_state": { "enum": ["verified", "gap"] }
        },
        "anyOf": [
          { "properties": { "implementation_evidence": { "minItems": 1 } } },
          { "properties": { "test_evidence": { "minItems": 1 } } }
        ]
      },
      "else": {
        "properties": {
          "scenario_id": { "type": "null" },
          "scenario_id_origin": { "type": "null" },
          "verification_state": { "const": "not_applicable" },
          "verification_gap": { "type": "null" }
        }
      }
    },
    {
      "if": { "properties": { "applicability": { "enum": ["shared", "native"] } } },
      "then": { "properties": { "verification_boundary": { "const": "native" } } },
      "else": { "properties": { "verification_boundary": { "const": "browser" } } }
    },
    {
      "if": { "properties": { "state": { "const": "discrepant" } } },
      "then": { "properties": { "discrepancy": { "type": "string", "minLength": 1 } } },
      "else": { "properties": { "discrepancy": { "type": "null" } } }
    },
    {
      "if": { "properties": { "scenario_id_origin": { "const": "allocated" } } },
      "then": {
        "properties": {
          "scenario_id": { "type": "string", "pattern": "^[A-Z][A-Z0-9]*-[0-9]{3}$" }
        }
      }
    },
    {
      "if": { "properties": { "verification_state": { "const": "verified" } } },
      "then": {
        "properties": {
          "test_evidence": { "minItems": 1 },
          "verification_gap": { "type": "null" }
        }
      }
    },
    {
      "if": { "properties": { "verification_state": { "const": "gap" } } },
      "then": {
        "properties": {
          "state": { "const": "shipped" },
          "verification_gap": { "type": "string", "minLength": 1 }
        }
      },
      "else": { "properties": { "verification_gap": { "type": "null" } } }
    }
  ]
}
```

- [ ] **Step 2: Record the native audit baseline**

Run:

```powershell
rtk cargo test -p waml-editor -- --list
rtk cargo test -p waml-markdown-editor -- --list
rtk cargo test -p waml -- --list
rtk rg -n "#\[test\]|fn [a-zA-Z0-9_]+\(" crates/waml-editor/src/app/tests crates/waml-editor/src/editor_session/tests.rs crates/waml-editor/tests crates/waml-markdown-editor/tests
```

Expected: each command succeeds. The lists expose the native evidence names without changing files.

- [ ] **Step 3: Audit bundle, shell, navigation, and tab workflows**

Inspect these implementation owners and their adjacent tests:

```text
crates/waml-editor/src/start_screen.rs
crates/waml-editor/src/config.rs
crates/waml-editor/src/load.rs
crates/waml-editor/src/app/workspace.rs
crates/waml-editor/src/app/tests/shell.rs
crates/waml-editor/src/app/tests/menus.rs
crates/waml-editor/src/app/tests/navigation.rs
crates/waml-editor/src/app/tests/workspace.rs
crates/waml-editor/src/tree.rs
crates/waml-editor/src/tree_panel.rs
crates/waml-editor/src/navigation.rs
crates/waml-editor/src/doc_tabs.rs
crates/waml-editor/src/document_host.rs
crates/waml-editor/tests/view_history.rs
```

Add one inventory row for each observable branch. Include empty/start state, recent-item order and pinning, open and close outcomes, responsive shell modes, dock and overlay state, tree/folder navigation, breadcrumbs, external links, reveal, back/forward history, preview/permanent tabs, active-document changes, and presentation changes. Source evidence is sufficient to keep a row `shipped`. If no native test asserts the observable Then result, set `verification_state: "gap"`, explain the missing native assertion in `verification_gap`, and keep `test_evidence` empty or limited to the partial tests that actually exist.

- [ ] **Step 4: Audit Markdown and session-state workflows**

Inspect these owners and their tests:

```text
crates/waml-markdown-editor/src/session.rs
crates/waml-markdown-editor/src/input.rs
crates/waml-markdown-editor/src/widget.rs
crates/waml-markdown-editor/src/reading/
crates/waml-markdown-editor/tests/
crates/waml-editor/src/editor_session.rs
crates/waml-editor/src/editor_session/tests.rs
crates/waml-editor/src/native_save.rs
crates/waml-editor/src/api_save.rs
crates/waml-editor/tests/editor_history.rs
crates/waml-editor/tests/history_integration.rs
crates/waml-editor/tests/markdown_authority.rs
crates/waml-editor/tests/markdown_integration.rs
```

Add rows for reading, editing, selection, clipboard, multi-caret, IME begin/update/commit/cancel, undo, redo, savepoint identity, dirty state, save failure, diagnostics, quarantine display, and status feedback. Do not label a row `discrepant` only because automation is absent; use `verification_state: "gap"` while retaining its source-evidenced shipped contract.

- [ ] **Step 5: Audit diagram workflows**

Inspect these owners and their tests:

```text
crates/waml-editor/src/class_diagram_view.rs
crates/waml-editor/src/canvas/class/
crates/waml-editor/src/diagram_properties.rs
crates/waml-editor/src/inspector_panel.rs
crates/waml-editor/src/scene.rs
crates/waml-editor/src/behavior_doc_view.rs
crates/waml-editor/src/canvas/behavior/
crates/waml/tests/flow_solver_golden.rs
crates/waml/tests/interaction_solver_golden.rs
crates/waml/tests/incremental_analysis.rs
```

Add rows for class selection, tools, direct manipulation, properties, layout, conflicts, solver feedback, and each shipped activity/sequence rendering, hit-test, selection, and camera result. Add state-machine and use-case behavior when the audit finds a distinct visible branch.

- [ ] **Step 6: Validate the native inventory rules**

Run this exact PowerShell check:

```powershell
rtk proxy pwsh -NoProfile -Command '& { $rows = @(Get-Content "docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl" | ForEach-Object { $_ | ConvertFrom-Json }); if ($rows.Count -eq 0) { throw "inventory is empty" }; $canonical = "^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$"; $allocated = "^[A-Z][A-Z0-9]*-[0-9]{3}$"; if ($rows | Where-Object { $_.applicability -in @("shared","native") -and $_.verification_boundary -ne "native" }) { throw "native boundary mismatch" }; if ($rows | Where-Object { $_.state -eq "shipped" -and (-not $_.scenario_id -or $_.scenario_id -notmatch $canonical) }) { throw "shipped row has no canonical scenario" }; if ($rows | Where-Object { $_.scenario_id_origin -eq "allocated" -and $_.scenario_id -notmatch $allocated }) { throw "new scenario does not use three digits" }; if ($rows | Where-Object { $_.state -eq "shipped" -and $_.implementation_evidence.Count -eq 0 -and $_.test_evidence.Count -eq 0 }) { throw "shipped row has no evidence" }; if ($rows | Where-Object { $_.verification_state -eq "verified" -and $_.test_evidence.Count -eq 0 }) { throw "verified row has no test" }; if ($rows | Where-Object { $_.verification_state -eq "gap" -and -not $_.verification_gap }) { throw "verification gap has no reason" }; if ($rows | Where-Object { $_.state -ne "shipped" -and $_.verification_state -ne "not_applicable" }) { throw "non-shipped row has verification state" } }'
```

Expected: exit 0 with no output.

- [ ] **Step 7: Commit the native inventory**

```bash
git add docs/superpowers/audits/2026-08-08-ui-behavior-inventory.schema.json docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl
git commit -m "docs: inventory native behavior"
```

### Task 2: Complete and freeze browser, CLI, LSP, and VS Code inventory

**Files:**
- Modify: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`

**Interfaces:**
- Consumes: the schema and native rows from Task 1.
- Produces: a complete, sorted, immutable allocation of behavior and scenario identifiers for every interactive surface.

- [ ] **Step 1: Inventory browser boot, share, site, serve, and API behavior**

Inspect:

```text
crates/waml-editor/src/browser_boot.rs
crates/waml-editor/src/platform_browser.rs
crates/waml-editor/src/api_save.rs
crates/waml-cli/src/site.rs
crates/waml-cli/src/serve/
crates/waml-cli/tests/serve_e2e.rs
scripts/export-site-browser.test.mjs
scripts/serve-browser-check.mjs
scripts/inject-runtime-shell.test.mjs
scripts/package-web-artifact.test.mjs
scripts/verify-web-artifact.test.mjs
.github/workflows/pages.yml
```

Add browser rows for share-fragment priority, API and token boot, bundle URL boot, site boot configuration, failed fetch feedback, download/export, same-origin API access, rejected foreign origin, save conflicts, static site export, and artifact completeness. Use `verification_boundary: "browser"`. A host Rust test can prove URL selection, but it does not satisfy browser verification for visible browser behavior. Keep source-evidenced behavior `shipped` and set `verification_state: "gap"` until browser-specific evidence asserts its Then result.

- [ ] **Step 2: Inventory CLI, LSP, and VS Code behavior**

Inspect:

```text
crates/waml-cli/src/main.rs
crates/waml-cli/src/commands.rs
crates/waml-cli/src/lsp/
crates/waml-cli/tests/cli_e2e.rs
crates/waml-cli/tests/lsp_e2e.rs
editors/vscode/src/extension.ts
editors/vscode/src/serverPath.ts
editors/vscode/src/serverPath.test.ts
```

Include the shipped LSP capabilities from `server_capabilities()`: diagnostics, document symbols, document links, definitions, and semantic tokens. Include executable resolution, launch, restart, error, and deactivate workflows from the VS Code tests.

- [ ] **Step 3: Record unsupported, planned, and discrepant behavior**

For every implementation/documentation disagreement, create a `discrepant` row with a complete `discrepancy` sentence. For every visible control with no shipped implementation, create a `planned` or `unsupported` row. Do not allocate a scenario identifier to these rows. A missing or partial target-boundary test is not a disagreement: keep the source-evidenced row `shipped`, allocate or preserve its scenario, and record `verification_state: "gap"` plus `verification_gap`.

- [ ] **Step 4: Sort and freeze identifiers**

Sort JSONL lines by `behavior_id`. Check that each `scenario_id` is unique. Check the compatible canonical grammar, the three-digit rule for `allocated` identifiers, and the preserved spelling of every `existing` identifier. Check that all `shared` rows target `native` and all `browser` rows target `browser`. Check that every shipped gap has source or test evidence and a non-empty reason.

Run:

```powershell
rtk proxy pwsh -NoProfile -Command '& { $rows = @(Get-Content "docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl" | ForEach-Object { $_ | ConvertFrom-Json }); $canonical = "^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$"; $allocated = "^[A-Z][A-Z0-9]*-[0-9]{3}$"; $dup = $rows | Where-Object scenario_id | Group-Object scenario_id | Where-Object Count -gt 1; if ($dup) { throw ("duplicate scenarios: " + (($dup.Name) -join ", ")) }; if ($rows | Where-Object { $_.scenario_id -and $_.scenario_id -notmatch $canonical }) { throw "noncanonical scenario identifier" }; if ($rows | Where-Object { $_.scenario_id_origin -eq "allocated" -and $_.scenario_id -notmatch $allocated }) { throw "new scenario does not use three digits" }; if ($rows | Where-Object { $_.applicability -eq "shared" -and $_.verification_boundary -ne "native" }) { throw "shared row has wrong verification target" }; if ($rows | Where-Object { $_.applicability -eq "browser" -and $_.verification_boundary -ne "browser" }) { throw "browser row has wrong verification target" }; if ($rows | Where-Object { $_.state -eq "shipped" -and $_.implementation_evidence.Count -eq 0 -and $_.test_evidence.Count -eq 0 }) { throw "shipped row has no evidence" }; if ($rows | Where-Object { $_.verification_state -eq "verified" -and $_.test_evidence.Count -eq 0 }) { throw "verified row has no test" }; if ($rows | Where-Object { $_.verification_state -eq "gap" -and -not $_.verification_gap }) { throw "verification gap has no reason" } }'
```

Expected: exit 0 with no output.

- [ ] **Step 5: Commit the frozen inventory**

```bash
git add docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl
git commit -m "docs: freeze behavior traceability inventory"
```

### Task 3: Add deterministic generated-index checking to the existing CLI

**Files:**
- Modify: `crates/waml-cli/src/main.rs`
- Modify: `crates/waml-cli/src/commands.rs`
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-cli/tests/cli_e2e.rs`

**Interfaces:**
- Consumes: `io::read_physical_bundle(paths: &[PathBuf]) -> io::Result<PhysicalBundle>`, `commands::prepare`, and `waml::index_md::reindex_source(&SourceBundle) -> SourceBundle`.
- Produces: `waml index DIRECTORY [--check]`; `commands::plan_indexes(files: &[(String, String)]) -> Result<Vec<IndexChange>, String>`; `io::write_indexes(root: &Path, changes: &[IndexChange]) -> io::Result<()>`.

- [ ] **Step 1: Write failing CLI tests**

Add tests with these exact names:

```rust
#[test]
fn index_check_reports_each_stale_index_and_exits_one() {
    let dir = tmp();
    std::fs::write(dir.join("index.md"), "# Wrong\n").unwrap();
    std::fs::write(
        dir.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();

    let output = bin()
        .args(["index"])
        .arg(&dir)
        .arg("--check")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("index.md: generated index is stale"));
}

#[test]
fn index_write_reconciles_then_check_is_clean() {
    let dir = tmp();
    std::fs::write(
        dir.join("order.md"),
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )
    .unwrap();

    assert!(bin().args(["index"]).arg(&dir).status().unwrap().success());
    assert!(dir.join("index.md").is_file());
    assert!(bin()
        .args(["index"])
        .arg(&dir)
        .arg("--check")
        .status()
        .unwrap()
        .success());
}

#[test]
fn index_never_changes_non_index_documents() {
    let dir = tmp();
    let leaf = "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n";
    std::fs::write(dir.join("order.md"), leaf).unwrap();

    assert!(bin().args(["index"]).arg(&dir).status().unwrap().success());

    assert_eq!(std::fs::read_to_string(dir.join("order.md")).unwrap(), leaf);
}
```

Assert stderr uses `waml: {display path}: generated index is stale`. Assert write mode creates missing indexes, replaces stale indexes, removes only stale files whose basename equals `index.md`, and leaves every other file byte-identical.

Add direct `io.rs` unit cases for `../outside/index.md`, an absolute path, `nested/not-index.md`, an existing symlinked parent, and a symlinked `index.md`. Each case must return `InvalidInput` or `PermissionDenied` before a write or removal. On Windows, create symlinks only when the test process has permission; otherwise return early from only the two symlink cases. The traversal, absolute-path, and basename cases must always run.

- [ ] **Step 2: Run the tests to verify failure**

Run: `rtk cargo test -p waml-cli --test cli_e2e index_`

Expected: FAIL because `index` is not a Clap command.

- [ ] **Step 3: Add the exact planning interface**

Add this public type in `commands.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexChange {
    Upsert { path: String, rendered: String },
    Remove { path: String },
}

pub fn plan_indexes(files: &[(String, String)]) -> Result<Vec<IndexChange>, String>;
```

Implement `plan_indexes` with this deterministic algorithm:

```text
prepared := prepare(files); return its diagnostic error unchanged on failure
before := BTreeMap(path -> text) from prepared.source()
after := BTreeMap(path -> text) from reindex_source(prepared.source())
paths := sorted union of before.keys and after.keys
for path in paths:
    if the final slash-separated segment is not ASCII-case-insensitive "index.md": continue
    if before[path] == after[path]: continue
    if after[path] exists: append Upsert { path, rendered: after[path] }
    otherwise: append Remove { path }
return changes
```

Do not infer removals by scanning the physical directory. `Remove` comes only from an `index.md` that exists in the prepared input bundle and is absent from the reindexed output bundle. This keeps non-index files outside the change set by construction.

- [ ] **Step 4: Add the CLI and safe writer**

Add this Clap shape to `Command`:

```rust
/// Rebuild deterministic directory index documents.
Index {
    /// One directory that contains the Markdown bundle.
    path: PathBuf,
    /// Do not write; exit non-zero when an index differs.
    #[arg(long)]
    check: bool,
},
```

`--check` prints one file-specific line per `IndexChange` and exits 1. Write mode uses `PhysicalBundle.root`. Implement the writer with this exact containment algorithm; translate each `reject(...)` to an `io::Error` with `InvalidInput`, except a symlink escape uses `PermissionDenied`:

```text
resolve_index_target(root, relative, create_parents):
    canonical_root := canonicalize(root)
    components := components(Path(relative))
    reject when components is empty
    reject every Prefix, RootDir, or ParentDir component
    discard CurDir components; retain Normal components without string rewriting
    reject unless the final Normal component equals "index.md" ignoring ASCII case
    current := canonical_root
    for each retained directory component before the filename:
        candidate := current.join(component)
        if symlink_metadata(candidate) succeeds:
            reject with PermissionDenied when candidate is a symlink
            reject unless candidate is a directory
            resolved := canonicalize(candidate)
            reject with PermissionDenied unless resolved.starts_with(canonical_root)
            current := resolved
        else if the error is NotFound and create_parents:
            create_dir(candidate)
            resolved := canonicalize(candidate)
            reject with PermissionDenied unless resolved.starts_with(canonical_root)
            current := resolved
        else:
            return the filesystem error
    target := current.join(final component)
    reject unless target.starts_with(canonical_root)
    if symlink_metadata(target) succeeds:
        reject with PermissionDenied when target is a symlink
        resolved := canonicalize(target)
        reject with PermissionDenied unless resolved.starts_with(canonical_root)
    else if the error is not NotFound:
        return the filesystem error
    return target

write_indexes(root, changes):
    for change in the already sorted changes:
        if Upsert:
            target := resolve_index_target(root, path, true)
            reject when target exists and is not a regular file
            write(target, rendered.as_bytes())
        if Remove:
            target := resolve_index_target(root, path, false)
            if target does not exist: continue
            reject unless symlink_metadata(target).file_type is a regular file
            remove_file(target)
```

Never call `remove_dir`, never follow a symlink, and never construct a target with `root.join(relative)` before component validation. The CLI obtains all changes from `plan_indexes`; the writer independently validates every path so a future caller cannot bypass containment.

- [ ] **Step 5: Run focused and regression tests**

Run:

```powershell
rtk cargo test -p waml-cli --test cli_e2e index_
rtk cargo test -p waml-cli
rtk cargo test -p waml index_md
```

Expected: PASS. The index unit tests prove that `reindex_source` remains a fixpoint.

- [ ] **Step 6: Commit the index command**

```bash
git add crates/waml-cli/src/main.rs crates/waml-cli/src/commands.rs crates/waml-cli/src/io.rs crates/waml-cli/tests/cli_e2e.rs
git commit -m "feat(cli): check generated indexes"
```

### Task 4: Add the documentation contract checker

**Files:**
- Create: `scripts/check-waml-doc-contract.mjs`
- Create: `scripts/check-waml-doc-contract.test.mjs`

**Interfaces:**
- Consumes: canonical Markdown that already passes `waml check`; repository root and one `docs/waml` root.
- Produces: `checkDocsContract(docsRoot, repositoryRoot) -> Promise<string[]>`; a CLI that prints the document path, line, and reason, then exits 1 when errors exist.

- [ ] **Step 1: Write failing fixture tests**

Use temporary directories. Start the test file with this fixture helper and canonical document:

```javascript
import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { checkDocsContract } from "./check-waml-doc-contract.mjs";

async function check(files) {
  const repositoryRoot = await mkdtemp(join(tmpdir(), "waml-doc-contract-"));
  for (const [path, text] of Object.entries(files)) {
    const absolute = join(repositoryRoot, path);
    await mkdir(dirname(absolute), { recursive: true });
    await writeFile(absolute, text);
  }
  return checkDocsContract(join(repositoryRoot, "docs/waml"), repositoryRoot);
}

const canonical = `# Tabs

**Status:** done

#### TAB-001 — a new preview replaces the old preview

**Applies to:** shared

**Given** one document is open in the preview tab
**When** the reader selects a different document
**Then** the editor replaces the old preview

**Evidence:** \`crates/waml-editor/src/doc_tabs.rs::preview_replaces_preview\`
`;

test("accepts the canonical shipped scenario form", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs":
      "// Scenario: TAB-001\n#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.deepEqual(errors, []);
});

test("accepts stable multi-segment identifiers verbatim", async () => {
  for (const id of ["SEQ-MSG-1", "SEQ-ORD-1", "SEQ-FRAG-10"]) {
    const legacy = canonical.replaceAll("TAB-001", id);
    const errors = await check({
      "docs/waml/goals/sequence.md": legacy,
      "crates/waml-editor/src/doc_tabs.rs":
        `// Scenario: ${id}\n#[test]\nfn preview_replaces_preview() {}\n`,
    });
    assert.deepEqual(errors, [], id);
  }
});

test("accepts shipped source evidence with an explicit native verification gap", async () => {
  const sourceOnly = `${canonical}\n## Verification gaps\n\n- TAB-001 — target: native; No native test asserts the tab replacement result.\n`;
  const errors = await check({
    "docs/waml/goals/tabs.md": sourceOnly,
    "crates/waml-editor/src/doc_tabs.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.deepEqual(errors, []);
});

test("rejects source-only shipped evidence without a verification gap", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("Verification gaps")), true);
});

test("accepts browser source evidence with an explicit browser verification gap", async () => {
  const browser = canonical
    .replaceAll("TAB-001", "WEB-001")
    .replace("**Applies to:** shared", "**Applies to:** browser")
    .replace("crates/waml-editor/src/doc_tabs.rs", "crates/waml-editor/src/browser_boot.rs");
  const errors = await check({
    "docs/waml/goals/web.md": `${browser}\n## Verification gaps\n\n- WEB-001 — target: browser; No browser-specific test asserts the boot result.\n`,
    "crates/waml-editor/src/browser_boot.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.deepEqual(errors, []);
});

test("rejects a verification gap with the wrong target", async () => {
  const withWrongGap = `${canonical}\n## Verification gaps\n\n- TAB-001 — target: browser; No native test asserts the tab replacement result.\n`;
  const errors = await check({
    "docs/waml/goals/tabs.md": withWrongGap,
    "crates/waml-editor/src/doc_tabs.rs":
      "pub fn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("native test is absent")), true);
});

test("rejects duplicate scenario identifiers with both paths", async () => {
  const errors = await check({
    "docs/waml/goals/a.md": canonical,
    "docs/waml/goals/b.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs": "// Scenario: TAB-001\n",
  });
  assert.equal(errors.some((error) => error.includes("a.md") && error.includes("b.md")), true);
});

test("rejects shipped scenarios without applicability or evidence", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical
      .replace("**Applies to:** shared\n\n", "")
      .replace(/\n\*\*Evidence:\*\*.*\n/, "\n"),
  });
  assert.equal(errors.some((error) => error.includes("Applies to")), true);
  assert.equal(errors.some((error) => error.includes("Evidence")), true);
});

test("rejects implemented and unverified goal status text", async () => {
  const errors = await check({
    "docs/waml/goals/a.md": "# A\n\n**Status:** implemented\n",
    "docs/waml/goals/b.md": "# B\n\n**Status:** partial — unverified\n",
  });
  assert.equal(errors.length, 2);
});

test("rejects a shared scenario whose evidence is browser-only", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical.replace(
      "crates/waml-editor/src/doc_tabs.rs",
      "crates/waml-editor/src/browser_boot.rs",
    ),
    "crates/waml-editor/src/browser_boot.rs":
      "// Scenario: TAB-001\n#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("native")), true);
});

test("rejects a browser scenario whose evidence is not browser-specific or a parity seam", async () => {
  const errors = await check({
    "docs/waml/goals/web.md": canonical
      .replace("TAB-001", "WEB-001")
      .replace("**Applies to:** shared", "**Applies to:** browser"),
    "crates/waml-editor/src/doc_tabs.rs":
      "// Scenario: WEB-001\n#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("browser-specific")), true);
});

test("rejects a scenario identifier absent from its cited test file", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical,
    "crates/waml-editor/src/doc_tabs.rs": "#[test]\nfn preview_replaces_preview() {}\n",
  });
  assert.equal(errors.some((error) => error.includes("Scenario: TAB-001")), true);
});

test("rejects Given-When-Then text under planned or horizon behavior", async () => {
  const errors = await check({
    "docs/waml/goals/tabs.md": canonical.replace("# Tabs", "# Tabs\n\n## Planned behavior"),
    "crates/waml-editor/src/doc_tabs.rs": "// Scenario: TAB-001\n",
  });
  assert.equal(errors.some((error) => error.includes("planned")), true);
});
```

- [ ] **Step 2: Run tests to verify failure**

Run: `rtk node --test scripts/check-waml-doc-contract.test.mjs`

Expected: FAIL because the checker module does not exist.

- [ ] **Step 3: Implement the deterministic line-contract scanner**

Export `checkDocsContract(docsRoot, repositoryRoot) -> Promise<string[]>`. Use these exact regular expressions and constants:

```javascript
const ID_SOURCE = String.raw`[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+`;
const SCENARIO = new RegExp(`^#### (${ID_SOURCE}) — ([a-z].+)$`);
const GAP = new RegExp(`^- (${ID_SOURCE}) — target: (native|browser); (.+[.!?])$`);
const STATUS = /^\*\*Status:\*\* (done|partial|planned|horizon)$/;
const EVIDENCE_REF = /`([^`]+)`/g;
const REF = /^(?<path>[A-Za-z0-9._/-]+)(?:::(?<symbol>[A-Za-z0-9_ .:'"-]+)|:(?<line>[1-9][0-9]*))$/;
const NON_SHIPPED_SECTIONS = new Set([
  "Planned behavior",
  "Unsupported behavior",
  "Discrepancies",
]);
const BROWSER_TEST_PATHS = [
  /^crates\/waml-cli\/tests\/serve_e2e\.rs$/,
  /^scripts\/export-site-browser\.test\.mjs$/,
  /^scripts\/serve-browser-check\.mjs$/,
  /^scripts\/.*browser.*\.test\.mjs$/,
];
const BROWSER_IMPLEMENTATION_PATHS = [
  /^crates\/waml-editor\/src\/(browser_boot|platform_browser|api_save)\.rs$/,
  /^crates\/waml-cli\/src\/site\.rs$/,
  /^crates\/waml-cli\/src\/serve\//,
  /^scripts\//,
];
```

Implement the scanner in this order:

```text
walkMarkdown(directory):
    recursively read directory entries in locale-independent code-point order
    do not follow directory symlinks
    return only *.md files, sorted by repository-relative path with "/" separators

checkDocsContract(docsRoot, repositoryRoot):
    errors := []
    scenarios := Map<id, { document, line, applicability, refs, section }>
    gaps := Map<id, { document, line, target, reason }>
    for document in walkMarkdown(docsRoot):
        text := readFile(document, "utf8"); lines := text split on /\r?\n/
        section := ""
        scan each line from top to bottom:
            when line starts "## ", set section to its exact remaining text
            reject a **Status:** line unless it matches STATUS
            when section is "Verification gaps" and line starts "- ":
                require GAP; reject duplicate gap IDs; store the parsed gap
            when line starts "#### ":
                require SCENARIO, including the literal Unicode em dash
                reject when section is in NON_SHIPPED_SECTIONS
                block := nonblank lines until the next heading matching /^#{1,4} /
                parse block with parseScenarioBlock below
                reject a duplicate ID and name both document paths
                store the parsed scenario
        for each WAML file under docs/waml/architecture/views:
            inspect only the text between the first pair of exact "---" lines
            accept when that range contains /^sources:\s*$/
            otherwise require a relative Markdown link into ../concepts/implementation/
            resolve that link, reject escape from docsRoot, and require the linked
            concept's frontmatter range to contain /^sources:\s*$/

    for scenario in scenarios in ID order:
        target := scenario.applicability == "browser" ? "browser" : "native"
        result := inspectEvidence(scenario, repositoryRoot, target)
        append result.errors
        if result.hasTargetTest:
            reject a stale gap entry for scenario.id
        else:
            require one gap entry with the same ID, document, and target
            error text includes "Verification gaps" when it is absent
    reject every gap whose ID has no source-evidenced shipped scenario
    return errors sorted by normalized document path, line number, then reason
```

Parse each scenario block without a Markdown AST:

```text
parseScenarioBlock(nonblankLines):
    consume exactly one "**Applies to:** shared|native|browser"
    consume exactly one line beginning "**Given** "
    consume zero or more lines beginning "**And** "
    consume exactly one line beginning "**When** "
    consume exactly one line beginning "**Then** "
    consume zero or more lines beginning "**And** "
    consume exactly one line beginning "**Evidence:** "
    reject trailing nonblank lines before the next heading
    extract every backtick value with EVIDENCE_REF; require at least one
    require each value to match REF
```

Inspect evidence deterministically:

```text
inspectEvidence(scenario, repositoryRoot, target):
    hasSource := false; hasTargetTest := false; errors := []
    for reference in scenario.refs:
        reject absolute paths, backslashes, empty segments, ".", and ".."
        absolute := resolve(repositoryRoot, reference.path)
        realRoot := realpath(repositoryRoot); realFile := realpath(absolute)
        contained := relative(realRoot, realFile)
        reject when contained is empty only if the reference names the root itself
        reject when contained is absolute, equals "..", or starts with ".." + sep
        require realFile is a regular file; read it as UTF-8 and split on /\r?\n/
        for path:line: require line <= line count; set hasSource := true
        for path::symbol:
            require the exact symbol text occurs; use its first matching line
            window := that line plus the preceding 12 lines
            testCandidate := path contains "/tests/" or ".test."
                             or path matches BROWSER_TEST_PATHS
                             or window contains #[test], test(, or it(
            if not testCandidate: set hasSource := true; continue
            require one trimmed window line equals
                    "// Scenario: <scenario.id>" or "# Scenario: <scenario.id>"
            nativeTest := path starts "crates/" and path matches neither
                          BROWSER_TEST_PATHS nor BROWSER_IMPLEMENTATION_PATHS
            browserTest := path matches one of BROWSER_TEST_PATHS
            if target is native and nativeTest: hasTargetTest := true
            if target is browser and browserTest: hasTargetTest := true
    reject when hasSource is false and no valid marked test was found
    return { hasSource, hasTargetTest, errors }
```

A marked test at the wrong boundary is evidence that exists, but it does not set `hasTargetTest`; the owning document must retain a verification-gap item for the target boundary. When that item is absent, use `native test is absent; add an item under Verification gaps` for a native target and `browser-specific test is absent; add an item under Verification gaps` for a browser target. A missing marker on a cited test is an error, not source evidence. A `shared` scenario needs a native Rust test. A `browser` scenario needs a browser-specific test from `BROWSER_TEST_PATHS`; `browser_boot.rs`, `platform_browser.rs`, `api_save.rs`, `site.rs`, and `serve/` can be implementation evidence but do not by themselves close a browser verification gap. An explicit parity seam can cite both native and browser tests; its required target still follows `**Applies to:**`.

For CLI mode, resolve `repositoryRoot` as `process.cwd()` and `docsRoot` from the one positional argument. Print each sorted error as `path:line: reason`. Set `process.exitCode = 1` when errors are non-empty and `0` otherwise. Do not parse YAML values or Markdown syntax beyond the exact line contracts above; `waml check` owns those grammars.

- [ ] **Step 4: Run checker tests and the existing script suite**

Run:

```powershell
rtk node --test scripts/check-waml-doc-contract.test.mjs
rtk node --test "scripts/*.test.mjs"
```

Expected: PASS. The second command includes the new test file.

- [ ] **Step 5: Commit the checker**

```bash
git add scripts/check-waml-doc-contract.mjs scripts/check-waml-doc-contract.test.mjs
git commit -m "test(docs): enforce WAML contract shape"
```

### Task 5: Define OKF metadata, scenario policy, and WAML feature gaps

**Files:**
- Create: `docs/waml/documentation-contract.md`
- Create: `docs/waml/waml-feature-gaps.md`
- Create: `docs/superpowers/audits/reports/contract.md`

**Interfaces:**
- Consumes: the frozen inventory, OKF v0.2 in `docs/specs/OKF_SPEC.md`, and the scenario prefix table in this plan.
- Produces: one canonical metadata mapping; one linked ledger with `FG-001` through `FG-010`; rules that every goal stream follows.

- [ ] **Step 1: Create the documentation contract with typed v0.2 metadata**

Start `documentation-contract.md` with this exact frontmatter:

```yaml
---
type: Reference
title: Documentation Contract
description: The scenario, evidence, status, provenance, and freshness rules for docs/waml.
sources:
  - { id: okf-v02, resource: ../specs/OKF_SPEC.md, title: OKF v0.2 }
  - { id: approved-design, resource: ../superpowers/specs/2026-08-08-ui-behavior-architecture-docs-design.md, title: UI behavior and architecture documentation design }
generated: { by: process:docs-audit, at: 2026-08-08T00:00:00Z }
verified: { by: process:docs-contract-check, at: 2026-08-08T00:00:00Z }
status: stable
stale_after: 2026-11-08
---
```

Define the exact stable scenario form from the approved design. Define the compatible identifier grammar `^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$`, preserve all existing identifiers verbatim, and reserve `PREFIX-NNN` for newly allocated identifiers. Define all prefix owners. State that body `**Status:**` is product completion and frontmatter `status` is OKF lifecycle. State that absence of OKF `status` means `stable`. State that trust tier is derived from `verified` and is never stored. State that `generated.at` supersedes `timestamp`; preserve `timestamp` only for legacy input.

- [ ] **Step 2: Define planned, unsupported, and discrepant forms**

Use these exact body section headings:

```markdown
## Planned behavior
## Unsupported behavior
## Discrepancies
## Verification gaps
```

Each planned, unsupported, or discrepancy list item starts with its frozen `BHV-*` identifier. A planned item states the product intention and says that it has no passing acceptance scenario. An unsupported item states that `origin/main` does not support the workflow. A discrepancy item states the visible claim, the observed result, and its exact `path:line` evidence. A verification-gap item uses `- SCENARIO-ID — target: native|browser; Complete sentence.` and refers to a shipped GWT scenario in the same document. It records missing target-boundary automation without changing the scenario or product state.

- [ ] **Step 3: Create the feature-gap ledger with ten seeded entries**

Each entry must have these headings: `Problem`, `Minimal desired notation`, `Current workaround`, `Affected documents`, and `Kind`. Seed:

| ID | Minimal desired notation | Kind | Required affected documents |
| --- | --- | --- | --- |
| `FG-001` | scenario-level `platform` and `capability` predicates | semantics | `run-in-a-browser.md`, `documentation-contract.md` |
| `FG-002` | reusable typed gestures plus an input-consumed assertion | syntax | `edit-prose.md`, `draw-on-the-canvas.md` |
| `FG-003` | named view anchors plus `eventually` after one draw cycle | semantics | `fit-the-window.md`, `read-a-diagram.md` |
| `FG-004` | ordered collection and state assertions | semantics | `work-with-tabs.md`, `select-and-inspect.md` |
| `FG-005` | semantic text positions, multi-caret actions, and IME composition | syntax | `edit-prose.md` |
| `FG-006` | transaction groups and saved-state markers | semantics | `save-and-undo.md` |
| `FG-007` | semantic canvas targets and coordinate-space-aware drag paths | syntax | `interact-with-a-class-diagram.md`, `draw-on-the-canvas.md` |
| `FG-008` | hit target, tolerance, and z-order assertions | semantics | the four diagram-interaction goal documents |
| `FG-009` | component ports plus explicit asynchronous and compare-and-swap notation | syntax | `crate-ownership.md`, `editor-ownership.md`, `revisioned-edit-transaction.md` |
| `FG-010` | traceable links from scenario identifiers through product use cases to tests and evidence | tooling | `documentation-contract.md` and every goal document with scenarios; Task 11 adds the product-use-case links after those documents exist |

Use WAML links to each affected document. State that the ledger records opportunities and does not authorize language changes. Do not add stick-figure actors, ellipse use cases, system-boundary rendering, or specialized use-case layout as ledger entries. The user implements that view separately. Keep `FG-010` because WAML does not yet enforce complete scenario-to-use-case-to-test traceability.

- [ ] **Step 4: Write the contract report and validate**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/documentation-contract.md docs/waml/waml-feature-gaps.md
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: `waml check` succeeds. The contract checker can still report pre-existing `implemented` and `unverified` goal text; record those known findings in `reports/contract.md` and do not hide them.

- [ ] **Step 5: Commit the contract and ledger**

```bash
git add docs/waml/documentation-contract.md docs/waml/waml-feature-gaps.md docs/superpowers/audits/reports/contract.md
git commit -m "docs: define behavior contract metadata"
```

### Task 6: Update read, shell, navigation, and tab goals

**Files:**
- Modify: `docs/waml/goals/read-a-bundle/open-a-bundle.md`
- Modify: `docs/waml/goals/read-a-bundle/browse-the-tree.md`
- Modify: `docs/waml/goals/read-a-bundle/read-a-document.md`
- Modify: `docs/waml/goals/read-a-bundle/read-a-diagram.md`
- Modify: `docs/waml/goals/read-a-bundle/navigate-and-return.md`
- Modify: `docs/waml/goals/read-a-bundle/fit-the-window.md`
- Create: `docs/waml/goals/read-a-bundle/use-the-shell.md`
- Create: `docs/waml/goals/read-a-bundle/work-with-tabs.md`
- Modify: `docs/waml/goals/read-a-bundle/index.md`
- Create: `docs/superpowers/audits/reports/read-shell.md`

**Interfaces:**
- Consumes: frozen rows with `BUNDLE`, `SHELL`, `NAV`, `TAB`, and `MDREAD` scenario identifiers.
- Produces: one owner for each read workflow; no copied cross-cutting scenario.

- [ ] **Step 1: Place each inventory row in exactly one leaf**

Use this ownership:

```text
open-a-bundle.md       start, recents, folder/link open, close, and open failure
use-the-shell.md       docks, splitters, overlays, popups, theme, and non-width shell behavior
fit-the-window.md      responsive layout and narrow/wide viewport behavior only
browse-the-tree.md     tree, folder, reveal, and external-link behavior
navigate-and-return.md breadcrumbs and back/forward history
work-with-tabs.md      preview, permanent/pinned, switching, and presentation switching
read-a-document.md     Markdown presentation and read-only selection
read-a-diagram.md      behavior common to reading every diagram kind
```

If a related document needs a cross-cutting behavior, add a link to its owning leaf and do not repeat the scenario.

- [ ] **Step 2: Convert each shipped row to the canonical scenario form**

Use exact inventory identifiers and evidence. Example shape:

```markdown
#### TAB-001 — a new preview replaces the old preview

**Applies to:** shared

**Given** one document is open in the preview tab
**When** the reader selects a different document in the tree
**Then** the editor replaces the old preview with the new document

**Evidence:** `crates/waml-editor/src/doc_tabs.rs::open_preview_twice_replaces_the_single_preview_slot`
```

Use the exact frozen scenario and evidence values for all other rows. Do not invent evidence. For each shipped row with `verification_state: "gap"`, cite implementation evidence in the scenario and add its exact target and reason under `## Verification gaps` in the same leaf.

- [ ] **Step 3: Record non-shipped rows without GWT text**

Put planned, unsupported, and discrepant entries under the exact sections from `documentation-contract.md`. Use the frozen `BHV-*` identifier and evidence from the row.

- [ ] **Step 4: Derive each leaf status**

Set `done` only when every behavior needed by `Done when` is shipped and has a scenario. Set `partial` when shipped coverage exists but planned, unsupported, or discrepant rows prevent completion. Set `planned` or `horizon` only when no shipped contract exists.

- [ ] **Step 5: Update the subtree index and report**

Add the two new leaves to `read-a-bundle/index.md`. Keep the index to one H1, one description paragraph, and one member list. Fill every report heading.

- [ ] **Step 6: Validate and commit**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/goals/read-a-bundle
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: no error points at this subtree. Errors from other not-yet-updated subtrees are allowed only while their parallel agents run.

```bash
git add docs/waml/goals/read-a-bundle docs/superpowers/audits/reports/read-shell.md
git commit -m "docs: specify reading and shell behavior"
```

### Task 7: Update authoring, persistence, diagnostics, and trust goals

**Files:**
- Modify: `docs/waml/goals/author-in-the-editor/create-and-delete-documents.md`
- Modify: `docs/waml/goals/author-in-the-editor/start-from-a-template.md`
- Modify: `docs/waml/goals/author-in-the-editor/edit-prose.md`
- Modify: `docs/waml/goals/author-in-the-editor/edit-the-model.md`
- Modify: `docs/waml/goals/author-in-the-editor/draw-on-the-canvas.md`
- Modify: `docs/waml/goals/author-in-the-editor/author-with-the-keyboard.md`
- Modify: `docs/waml/goals/author-in-the-editor/arrange-a-diagram.md`
- Modify: `docs/waml/goals/author-in-the-editor/reduce-the-effort.md`
- Modify: `docs/waml/goals/author-in-the-editor/save-and-undo.md`
- Modify: `docs/waml/goals/author-in-the-editor/index.md`
- Modify: `docs/waml/goals/trust-the-content/round-trip-losslessly.md`
- Modify: `docs/waml/goals/trust-the-content/resolve-references.md`
- Modify: `docs/waml/goals/trust-the-content/report-every-problem.md`
- Modify: `docs/waml/goals/trust-the-content/keep-indexes-correct.md`
- Modify: `docs/waml/goals/trust-the-content/format-canonically.md`
- Modify: `docs/waml/goals/trust-the-content/index.md`
- Create: `docs/superpowers/audits/reports/author-trust.md`

**Interfaces:**
- Consumes: `MDEDIT` and `SESSION` rows, plus author/trust rows owned by existing goals.
- Produces: stable Markdown-edit and persistence contracts; explicit planned keyboard/canvas behavior; evidence-based trust statuses.

- [ ] **Step 1: Assign Markdown and session rows**

`edit-prose.md` owns text insertion, deletion, selection, clipboard, multi-caret, and IME. `save-and-undo.md` owns undo, redo, history grouping, savepoints, dirty state, save success/failure, and final-close protection. `report-every-problem.md` owns user-visible diagnostic and quarantine feedback. Link instead of copying a row between these leaves.

- [ ] **Step 2: Write shipped scenarios and non-shipped records**

Use the canonical form. Include `FG-005` links for text-position or IME workarounds and `FG-006` for savepoint/transaction workarounds. Keep keyboard-only and unimplemented canvas controls under `Planned behavior` unless the inventory has shipped evidence. A source-evidenced shipped row stays GWT and gets its exact `Verification gaps` item when its target-boundary test is absent.

- [ ] **Step 3: Correct trust claims from evidence**

Replace first-reading claims with exact test evidence from formatter, lossless round-trip, reference, incremental-analysis, and index tests. A quarantine scenario must say that an invalid document can be stale while unrelated projections remain current. Do not describe analysis as one global binary result.

- [ ] **Step 4: Update statuses, indexes, and report**

Remove every `— unverified`. Use only the four allowed body values. Keep both subtree indexes in generated-content form.

- [ ] **Step 5: Validate and commit**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/goals/author-in-the-editor docs/waml/goals/trust-the-content
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: no error points at either owned subtree.

```bash
git add docs/waml/goals/author-in-the-editor docs/waml/goals/trust-the-content docs/superpowers/audits/reports/author-trust.md
git commit -m "docs: specify authoring and trust behavior"
```

### Task 8: Update class-diagram and shared-diagram goals

**Files:**
- Modify: `docs/waml/goals/uml/class/feature-cut.md`
- Create: `docs/waml/goals/uml/class/interact-with-a-class-diagram.md`
- Modify: `docs/waml/goals/uml/class/index.md`
- Modify: `docs/waml/goals/uml/shared/select-and-inspect.md`
- Modify: `docs/waml/goals/uml/shared/solve-the-layout.md`
- Modify: `docs/waml/goals/uml/shared/route-the-edges.md`
- Modify: `docs/waml/goals/uml/shared/place-the-labels.md`
- Modify: `docs/waml/goals/uml/shared/keep-the-map-stable.md`
- Modify: `docs/waml/goals/uml/shared/theme-the-diagram.md`
- Modify: `docs/waml/goals/uml/shared/index.md`
- Create: `docs/superpowers/audits/reports/class-shared.md`

**Interfaces:**
- Consumes: all `CLASS` rows and shared diagram rows from the inventory.
- Produces: one interaction owner for class UI and one owner per common solver/selection/theme behavior.

- [ ] **Step 1: Separate language capability from UI behavior**

Keep language and model coverage in `feature-cut.md`. Put selection, tools, direct manipulation, properties, drag/place, conflict feedback, solver feedback, and camera behavior in `interact-with-a-class-diagram.md`.

- [ ] **Step 2: Keep cross-cutting ownership in shared leaves**

Use `select-and-inspect.md` for behavior common to more than one diagram kind. Use `solve-the-layout.md`, `route-the-edges.md`, `place-the-labels.md`, `keep-the-map-stable.md`, and `theme-the-diagram.md` for their named outputs. The class interaction document links to these contracts.

- [ ] **Step 3: Write scenarios with semantic canvas targets**

Describe a classifier, member, edge, handle, property field, or conflict item. Do not use a widget id or raw pointer coordinate. When current WAML cannot express the gesture, link `FG-007` or `FG-008` and describe the current prose workaround. For each shipped gap row, retain its scenario with implementation evidence and add the exact target and reason under `## Verification gaps`.

- [ ] **Step 4: Update statuses, indexes, report, and validate**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/goals/uml/class docs/waml/goals/uml/shared
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: no error points at the owned files.

```bash
git add docs/waml/goals/uml/class docs/waml/goals/uml/shared docs/superpowers/audits/reports/class-shared.md
git commit -m "docs: specify class diagram behavior"
```

### Task 9: Update activity, sequence, state-machine, and use-case goals

**Files:**
- Modify: `docs/waml/goals/uml/activity/feature-cut.md`
- Create: `docs/waml/goals/uml/activity/interact-with-an-activity-diagram.md`
- Modify: `docs/waml/goals/uml/activity/index.md`
- Modify: `docs/waml/goals/uml/sequence/feature-cut.md`
- Modify: `docs/waml/goals/uml/sequence/language.md`
- Create: `docs/waml/goals/uml/sequence/interact-with-a-sequence-diagram.md`
- Modify: `docs/waml/goals/uml/sequence/index.md`
- Modify: `docs/waml/goals/uml/state-machine/feature-cut.md`
- Create: `docs/waml/goals/uml/state-machine/interact-with-a-state-machine-diagram.md`
- Modify: `docs/waml/goals/uml/state-machine/index.md`
- Modify: `docs/waml/goals/uml/use-case/feature-cut.md`
- Create: `docs/waml/goals/uml/use-case/interact-with-a-use-case-diagram.md`
- Modify: `docs/waml/goals/uml/use-case/index.md`
- Create: `docs/superpowers/audits/reports/behavior-diagrams.md`

**Interfaces:**
- Consumes: `ACT` and `SEQ` rows plus any shipped state-machine/use-case rows.
- Produces: rendering, hit-test, selection, and camera contracts for each behavior-diagram kind; retained stable sequence-language scenarios. It does not produce the product-use-case model from Task 11.

- [ ] **Step 1: Normalize existing sequence scenarios**

Add `**Applies to:** shared` and exact evidence to every stable `SEQ-MSG-*`, `SEQ-ORD-*`, and `SEQ-FRAG-*` scenario. Keep identifiers such as `SEQ-MSG-1`, `SEQ-ORD-1`, and `SEQ-FRAG-10` byte-for-byte stable. They match the compatible canonical grammar and are not renumbered to three digits. Add `Scenario:` markers in Task 15, not here.

- [ ] **Step 2: Create one interaction leaf per kind**

Each interaction leaf owns only user-visible rendering, hit testing, selection, camera retention, and refresh behavior. The feature-cut leaf continues to own language/model coverage. `goals/uml/use-case/**` describes the use-case editor and renderer as a product feature. It does not contain the permanent actor and workflow documents under `docs/waml/use-cases/**`.

- [ ] **Step 3: Write shipped and non-shipped records**

Use `ACT-*` and `SEQ-*` identifiers from the inventory. For state machine and use case, write stable scenarios when the inventory has shipped implementation or test evidence. If target-boundary automation is absent, keep the shipped GWT and add its `Verification gaps` item. Use planned or unsupported records only when implementation is absent, and use discrepant records only when implementation and documentation disagree.

- [ ] **Step 4: Link WAML expression gaps**

Use `FG-003` for eventual draw-cycle results and `FG-008` for hit tolerance or z-order. Do not add test syntax to WAML in this task. Do not add a feature-gap entry for stick-figure actors, ellipse use cases, system-boundary rendering, or specialized use-case layout. The user implements that view separately, and this plan does not constrain its geometry.

- [ ] **Step 5: Update indexes, statuses, report, and validate**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/goals/uml/activity docs/waml/goals/uml/sequence docs/waml/goals/uml/state-machine docs/waml/goals/uml/use-case
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: no error points at the four owned subtrees.

```bash
git add docs/waml/goals/uml/activity docs/waml/goals/uml/sequence docs/waml/goals/uml/state-machine docs/waml/goals/uml/use-case docs/superpowers/audits/reports/behavior-diagrams.md
git commit -m "docs: specify behavior diagram workflows"
```

### Task 10: Update browser, share, CLI, LSP, and VS Code goals

**Files:**
- Modify: `docs/waml/goals/share-and-publish/share-a-link.md`
- Modify: `docs/waml/goals/share-and-publish/run-in-a-browser.md`
- Modify: `docs/waml/goals/share-and-publish/serve-locally.md`
- Modify: `docs/waml/goals/share-and-publish/publish-a-site.md`
- Modify: `docs/waml/goals/share-and-publish/export-a-bundle.md`
- Modify: `docs/waml/goals/share-and-publish/index.md`
- Modify: `docs/waml/goals/tooling-around-the-repo/command-line-tool.md`
- Modify: `docs/waml/goals/tooling-around-the-repo/language-server.md`
- Modify: `docs/waml/goals/tooling-around-the-repo/text-editor-integration.md`
- Modify: `docs/waml/goals/tooling-around-the-repo/index.md`
- Create: `docs/superpowers/audits/reports/browser-tooling.md`

**Interfaces:**
- Consumes: `WEB`, `CLI`, `LSP`, and `VSC` rows.
- Produces: browser-only and parity-seam contracts; complete LSP capability text; removal of the out-of-vocabulary `implemented` status.

- [ ] **Step 1: Write browser-specific contracts**

Cover boot-source priority, clean site boot, download/export, share, static site, local serve, token placement, same-origin API use, rejected foreign origins, API save, and visible error feedback. Use `**Applies to:** browser` only when native behavior does not own the contract. If source evidence proves shipped browser behavior but no browser-specific test asserts the Then result, retain the scenario and add its browser-target `Verification gaps` item.

- [ ] **Step 2: Write shared parity seams once**

For a shared workflow, keep one shared scenario with native evidence. Add a browser scenario only when it verifies the explicit parity seam, and cite both the native test and browser-specific test in the Evidence line.

- [ ] **Step 3: Correct CLI and LSP claims**

State that the editor and CLI share core services but expose different operations. State that the language server provides diagnostics, document symbols, document links, definitions, and semantic tokens. Cite `snapshot_queries_are_advertised_unicode_exact_and_revision_current_over_stdio` for the query capabilities.

- [ ] **Step 4: Correct statuses and add feature-gap links**

Replace `**Status:** implemented` in `serve-locally.md` with an evidence-derived allowed value. Link `FG-001` for platform/capability predicates and `FG-010` for evidence traceability.

- [ ] **Step 5: Update indexes, report, validate, and commit**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/goals/share-and-publish docs/waml/goals/tooling-around-the-repo
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: no error points at the two owned subtrees.

```bash
git add docs/waml/goals/share-and-publish docs/waml/goals/tooling-around-the-repo docs/superpowers/audits/reports/browser-tooling.md
git commit -m "docs: specify browser and tooling workflows"
```

### Task 11: Create the permanent semantic product-use-case model

**Files:**
- Create: `docs/waml/use-cases/index.md`
- Create: `docs/waml/use-cases/actors/index.md`
- Create: `docs/waml/use-cases/actors/*.md` actor leaves, one kebab-case file for each distinct external role in the frozen workflows
- Create: `docs/waml/use-cases/workflows/index.md`
- Create: `docs/waml/use-cases/workflows/*.md` use-case leaves, one kebab-case file for each distinct shipped workflow
- Create: `docs/waml/use-cases/views/index.md`
- Create: `docs/waml/use-cases/views/editor-workflows.md`
- Create: `docs/waml/use-cases/views/browser-and-publishing-workflows.md`
- Create: `docs/waml/use-cases/views/tooling-workflows.md`
- Modify: `docs/waml/waml-feature-gaps.md`
- Create: `docs/superpowers/audits/reports/use-cases.md`

**Interfaces:**
- Consumes: the frozen inventory, `documentation-contract.md`, the completed goal leaves from Tasks 6 through 10, and exact scenario headings in those leaves.
- Produces: typed actor and use-case documents, three semantic system-boundary diagrams, and a report that maps each use case to one goal and its shipped scenario identifiers.

- [ ] **Step 1: Derive the actor and workflow sets from the frozen contract**

Group rows by semantic user intention first. Do not include `goal_document` in the grouping key. After the intention groups exist, collect their goal owners. If one intention has more than one owner, stop Task 11 and reconcile one inventory owner before you create any leaf for that intention. Do not create duplicate leaves to preserve conflicting ownership. Create use cases only for intention groups that contain at least one shipped GWT scenario. Derive actors from users and external roles that cross the editor, browser/publishing, or tooling boundary. Do not make a platform, screen, widget, crate, or internal service an actor. Use lower-case kebab-case filenames. Record the actor-to-workflow mapping, the semantic-intention key, and the reconciled owner under `# Evidence` in `reports/use-cases.md` before authoring documents.

- [ ] **Step 2: Create typed actor documents**

Each actor leaf uses `type: uml.Actor`, one H1 that matches `title`, and a short responsibility statement. Use `## Relationships` only for a semantically valid specialization. The narrower actor declares the relationship to its broader parent:

```markdown
## Relationships

- specializes [Product user](./product-user.md)
```

Do not use `specializes` for job-title similarity. A child actor must be able to participate in every use case of its parent.

- [ ] **Step 3: Create typed use-case documents and traceability links**

Each workflow leaf uses `type: uml.UseCase`, one H1 that matches `title`, and these sections in this order: `## Owning goal`, `## Scenarios`, and `## Relationships`. The owning-goal section has exactly one document link. The scenario section links each frozen shipped scenario identifier to its heading in that same goal document. Generate each fragment with the repository `heading_slug` rule in the Product Use-Case Traceability Procedure. Do not copy the heading text as a new scenario body.

Use this structure for the open-bundle workflow, with the exact frozen identifier and title in place of the example identifier when they differ:

```markdown
---
type: uml.UseCase
title: Open a bundle
description: A reader opens a WAML bundle in the product.
---
# Open a bundle

## Owning goal

- [Open a bundle](../../goals/read-a-bundle/open-a-bundle.md)

## Scenarios

- [BUNDLE-001](../../goals/read-a-bundle/open-a-bundle.md#bundle-001-—-open-a-bundle)

## Relationships

- associates [Reader](../actors/reader.md)
```

Put each actor association in the use-case document and author it once. Use `includes` only when the target workflow always runs as a required part of the source workflow. Use `extends` from an optional or conditional workflow to its base workflow. Use `specializes` from a narrower use case to a broader use case only when it inherits the complete parent intention and result. Do not infer a relationship from shared evidence, a shared goal, or execution order alone.

- [ ] **Step 4: Create semantic system-boundary diagrams**

Each view uses `type: Diagram` and `profile: uml-domain`. Use only `## Members` groups. Put actors under `### External actors`. Put use cases under one named product boundary: `### WAML editor boundary`, `### WAML browser and publishing boundary`, or `### WAML tooling boundary`. A use case can occur in more than one view when it crosses boundaries, but it still has one leaf document and one owning goal. Record every actor-to-view and use-case-to-view membership under `# Evidence` in `reports/use-cases.md`.

Do not add a `## Layout` section. Do not specify rows, columns, frames, positions, sizes, routes, shape names, or actor placement. Specialized stick-figure, ellipse, and system-boundary rendering is separate user work and is not a dependency of this task.

- [ ] **Step 5: Validate semantic links and absence of copied contracts**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/use-cases
rtk cargo run -p waml-cli -- fmt --check docs/waml/use-cases
rtk rg -n "^type: uml\.(Actor|UseCase)$|^- (associates|includes|extends|specializes) \[" docs/waml/use-cases
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Then run the complete Product Use-Case Traceability Procedure. Expected: both WAML commands pass. The contract checker reports no error for `docs/waml/use-cases/**` or `waml-feature-gaps.md`; errors from an architecture lane that is still running are allowed. The type-and-relationship search lists every authored semantic element. The traceability procedure finds one owner per semantic intention, exact repository-compatible fragments, no GWT body copy, no parsed layout record, and no actor or use-case leaf outside the union of view members. Add `use-cases/index.md` and the workflow documents to the affected-document links for `FG-010` because automatic scenario-to-use-case-to-test completeness checking remains a WAML tooling opportunity.

- [ ] **Step 6: Commit the product-use-case model**

```bash
git add docs/waml/use-cases docs/waml/waml-feature-gaps.md docs/superpowers/audits/reports/use-cases.md
git commit -m "docs: model product use cases"
```

### Task 12: Integrate the goal tree and deduplicate cross-cutting scenarios

**Files:**
- Modify: `docs/waml/goals/index.md`
- Modify: `docs/waml/goals/root-goal.md`
- Modify: `docs/waml/goals/mvp.md`
- Modify: `docs/waml/goals/beyond-uml.md`
- Modify: `docs/waml/goals/uml/index.md`
- Modify: all goal-subtree `index.md` files after Tasks 6 through 10 stop
- Modify: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`

**Interfaces:**
- Consumes: all five goal reports, `reports/use-cases.md`, every frozen inventory row, and the completed product-use-case model from Task 11.
- Produces: one scenario owner per shipped behavior; explicit verification-gap records; evidence-derived aggregate statuses; final planned/unsupported/discrepant destinations; confirmed goal and scenario targets for every product use case.

- [ ] **Step 1: Reconcile every report against the inventory**

For each JSONL row, confirm that `goal_document` contains its shipped scenario or non-shipped record. For each shipped `verification_state: "gap"` row, also require the exact scenario identifier in that document's `## Verification gaps` section with the same target and reason. Task 15 has already set marker flags only for real tests; do not weaken or fabricate them during integration. Correct only ownership mistakes and duplicate prose. Do not renumber identifiers.

For each use case in `reports/use-cases.md`, confirm that its one owning-goal link resolves and that each linked scenario occurs once in that goal. Confirm that the use-case document contains no GWT body line. If a product-use-case link is wrong, return it to the Task 11 owner; the goal integrator does not edit `docs/waml/use-cases/**`.

- [ ] **Step 2: Remove duplicate contracts**

Search:

```powershell
rtk rg -n "^#### [A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+ — " docs/waml/goals
```

Expected: each identifier occurs once. If two leaves describe the same action/result, keep the scenario in the inventory owner and replace the other copy with a WAML link.

- [ ] **Step 3: Derive aggregate goal and MVP status**

Update `root-goal.md` and `mvp.md` from leaf results. Remove the first-reading and `unverified` instructions. Keep the status legend at exactly four values. Treat shipped source evidence as shipped behavior, not a discrepancy. List verification gaps separately and apply the documented completion policy consistently; never hide a gap by changing the product state. Do not mark the root `done` while an MVP leaf has planned, unsupported, or discrepant work that violates its completion condition.

- [ ] **Step 4: Keep every goal index generated-content-only**

Each goal `index.md` has one H1, one description paragraph, and one member list. Put all policy in `documentation-contract.md` or `root-goal.md`.

- [ ] **Step 5: Run the complete goal checks**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/goals
rtk node scripts/check-waml-doc-contract.mjs docs/waml
rtk rg -n "\*\*Status:\*\* (implemented|.*unverified)" docs/waml/goals
```

Expected: the first two commands pass for the goal tree. The final search returns no matches.

- [ ] **Step 6: Commit the integrated goal tree**

```bash
git add docs/waml/goals docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl
git commit -m "docs: integrate evidence-based goal tree"
```

### Task 13: Document six-crate and editor ownership

**Files:**
- Create: `docs/waml/architecture/concepts/implementation/index.md`
- Create: `docs/waml/architecture/concepts/implementation/waml-core-crate.md`
- Create: `docs/waml/architecture/concepts/implementation/waml-syntax-crate.md`
- Create: `docs/waml/architecture/concepts/implementation/waml-markdown-editor-crate.md`
- Create: `docs/waml/architecture/concepts/implementation/waml-editor-crate.md`
- Create: `docs/waml/architecture/concepts/implementation/waml-cli-crate.md`
- Create: `docs/waml/architecture/concepts/implementation/waml-ops-dto-crate.md`
- Create: `docs/waml/architecture/concepts/implementation/source-bundle.md`
- Create: `docs/waml/architecture/concepts/implementation/markdown-syntax.md`
- Create: `docs/waml/architecture/concepts/implementation/okf-analysis.md`
- Create: `docs/waml/architecture/concepts/implementation/uml-analysis.md`
- Create: `docs/waml/architecture/concepts/implementation/prepared-candidate.md`
- Create: `docs/waml/architecture/concepts/implementation/affected-analysis.md`
- Create: `docs/waml/architecture/concepts/implementation/app-shell.md`
- Create: `docs/waml/architecture/concepts/implementation/editor-session.md`
- Create: `docs/waml/architecture/concepts/implementation/document-host.md`
- Create: `docs/waml/architecture/concepts/implementation/markdown-editor.md`
- Create: `docs/waml/architecture/concepts/implementation/diagram-renderer.md`
- Create: `docs/waml/architecture/concepts/implementation/platform-adapter.md`
- Create: `docs/waml/architecture/views/crate-ownership.md`
- Create: `docs/waml/architecture/views/editor-ownership.md`
- Create: `docs/superpowers/audits/reports/architecture.md`

**Interfaces:**
- Consumes: workspace members from root `Cargo.toml`, exact path dependencies from the six crate manifests, and implementation symbols from `analysis.rs`, `editor_session.rs`, and `document_host.rs`.
- Produces: typed `uml.Class` concepts and two WAML dependency/ownership diagrams.

- [ ] **Step 1: Create one typed concept per crate and runtime owner**

Each concept uses `type: uml.Class`, a stable title, `stereotype: crate` or `stereotype: runtime`, and `sources` with exact repository paths. Use these crate responsibilities:

```text
waml-syntax            immutable Markdown green/red syntax and incremental reparse
waml                   SourceBundle, OKF/UML analysis, edits, projection, layout, index generation
waml-markdown-editor   WAML-owned Markdown reading/editing sessions, input, layout, and Makepad widget
waml-editor            app shell, EditorSession, document host, navigation, renderers, native/browser adapters
waml-ops-dto           serde wire contract for CLI semantic operations
waml-cli               check/fmt/index, query/mutation, share/site/export, serve/API, and LSP hosts
```

Represent source dependency paths as `depends` relationships. Do not infer a dependency that is absent from Cargo manifests.

- [ ] **Step 2: Build the crate ownership class diagram**

`crate-ownership.md` uses `type: Diagram`, `profile: uml-domain`, and the six crate concepts. Group the six members by syntax, core, presentation, and product surfaces. Add notes that `waml-editor` depends on `waml` and `waml-markdown-editor`, `waml-markdown-editor` depends on `waml-syntax`, `waml` depends on `waml-syntax`, `waml-ops-dto` depends on `waml`, and `waml-cli` depends on `waml` and `waml-ops-dto`.

- [ ] **Step 3: Build the editor ownership class diagram**

Use the runtime concepts and these exact sources:

```text
App shell           crates/waml-editor/src/app.rs::App
EditorSession       crates/waml-editor/src/editor_session.rs::EditorSession
Document host       crates/waml-editor/src/document_host.rs::DocumentHost
Navigation and tabs crates/waml-editor/src/navigation.rs and doc_tabs.rs
Markdown editor     crates/waml-markdown-editor/src/widget.rs and session.rs
Diagram renderers   class_diagram_view.rs and behavior_doc_view.rs
Platform adapters   native_save.rs, platform_browser.rs, browser_boot.rs, and api_save.rs
```

Use notes for ownership invariants. Link `FG-009` because WAML has no first-class component ports.

- [ ] **Step 4: Validate diagrams and report**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/architecture/concepts/implementation docs/waml/architecture/views/crate-ownership.md docs/waml/architecture/views/editor-ownership.md
```

Expected: PASS. All WAML member links resolve.

- [ ] **Step 5: Commit ownership architecture**

```bash
git add docs/waml/architecture/concepts/implementation docs/waml/architecture/views/crate-ownership.md docs/waml/architecture/views/editor-ownership.md docs/superpowers/audits/reports/architecture.md
git commit -m "docs: map crate and editor ownership"
```

### Task 14: Document preparation, incremental updates, and deployment surfaces

**Files:**
- Create: `docs/waml/architecture/views/preparation-pipeline.md`
- Create: `docs/waml/architecture/views/incremental-analysis.md`
- Create: `docs/waml/architecture/views/revisioned-edit-transaction.md`
- Create: `docs/waml/architecture/views/deployment-surfaces.md`
- Create: `docs/waml/architecture/overview.md`
- Modify: `docs/waml/architecture/index.md`
- Modify: `docs/waml/architecture/views/index.md`
- Modify: `docs/waml/architecture/views/system-context.md`
- Modify: `docs/waml/architecture/views/authoring-and-validation.md`
- Modify: `docs/waml/architecture/views/editing-round-trip.md`
- Modify: `docs/waml/architecture/views/web-delivery.md`
- Modify: `docs/waml/architecture/concepts/workflows/model-projection.md`
- Modify: `docs/waml/architecture/concepts/workflows/validation-and-diagnostics.md`
- Modify: `docs/waml/architecture/concepts/workflows/editor.md`
- Modify: `docs/waml/architecture/concepts/runtime/native-editor.md`
- Modify: `docs/waml/architecture/concepts/runtime/command-line-tool.md`
- Modify: `docs/waml/architecture/concepts/runtime/language-server.md`
- Modify: `docs/waml/architecture/concepts/runtime/native-web-delivery.md`
- Modify: `docs/superpowers/audits/reports/architecture.md`

**Interfaces:**
- Consumes: `SourceBundle`, `PreviousAnalyses`, `PreparedCandidate`, `PromotedMarkdownUpdate`, `AffectedAnalysis`, `prepare_candidate*`, `ExactSourceEdit`, `EditorSession::apply_with_preparer`, and `EditorSession::install_semantic_completion`.
- Produces: two revisioned sequence diagrams, one incremental activity diagram, one deployment activity diagram, and corrected stale claims.

- [ ] **Step 1: Move hand-written architecture guide text out of the generated index**

Move the current `Understand the model`, `Follow a workflow`, and `Run the product` guide to `architecture/overview.md`. Reduce `architecture/index.md` to one H1, one description paragraph, and a generated member list. Link the overview as a member.

- [ ] **Step 2: Write the preparation sequence**

`preparation-pipeline.md` uses `type: uml.Sequence`. Use lifelines for immutable `SourceBundle`, Markdown syntax/catalog analysis, OKF analysis/lowering, UML syntax/analysis/projection, `PreparedCandidate`, and the caller. Show `prepare_candidate(candidate_source, previous, candidate_revision)`. Show failure returning without mutating the live bundle. Show successful `PreparedCandidate` carrying source, OKF, UML, affected analysis, and revision.

- [ ] **Step 3: Write incremental analysis as an activity**

`incremental-analysis.md` uses `type: uml.Activity`. Use this flow:

```text
exact text changes -> validate base identity -> incremental reparse when valid
-> full document reparse when incremental recovery cannot apply
-> compute affected semantic closure -> analyze affected islands
-> quarantine malformed documents -> retain unrelated current projections
-> prepare candidate -> commit or reject
```

Add notes for per-island freshness and the fact that a quarantined document does not make every projection stale.

- [ ] **Step 4: Write the revisioned edit transaction sequence**

`revisioned-edit-transaction.md` uses `type: uml.Sequence`. Use lifelines for user action, app shell, `EditorSession`, immutable live snapshot, edit lowering, candidate preparation, and document/diagram hosts. Show both exact Markdown edits and semantic edits. Show compare of base revision, prepare-then-commit, atomic snapshot installation, affected view refresh, and reject with the old snapshot intact. Link `FG-009` for explicit compare-and-swap notation.

- [ ] **Step 5: Write deployment and user surfaces as an activity**

`deployment-surfaces.md` uses `type: uml.Activity`. Cover desktop, static WebAssembly, share link, exported site, local serve/API, CLI, LSP, and VS Code. Show which surfaces read a bundle, which can write, and which host the editor. Keep native and browser UI as builds of the same editor, but do not claim that CLI and editor expose the same operations.

- [ ] **Step 6: Correct all known stale claims**

Make these exact corrections:

- The LSP provides diagnostics, symbols, links, definitions, and semantic tokens.
- Analysis can quarantine malformed documents and keep unrelated projections current.
- Semantic edits prepare a candidate and commit atomically; they do not mutate and restore the live bundle.
- Projection includes Markdown syntax/catalog, OKF, UML syntax/analysis, and multiple view projections. It is not a plain-document/UML two-stage process.
- Editor and CLI share core services and expose different operations.

- [ ] **Step 7: Update architecture indexes and validate**

Add all new views to `architecture/views/index.md`. Add implementation concepts to the architecture package listing. Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml/architecture
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: no architecture-link, concept-resolution, metadata, or stale-claim error.

- [ ] **Step 8: Commit runtime architecture**

```bash
git add docs/waml/architecture docs/superpowers/audits/reports/architecture.md
git commit -m "docs: model revisioned runtime architecture"
```

### Task 15: Put scenario identifiers in their evidence tests

**Files:**
- Modify as cited: `crates/waml-editor/src/start_screen.rs`
- Modify as cited: `crates/waml-editor/src/config.rs`
- Modify as cited: `crates/waml-editor/src/doc_tabs.rs`
- Modify as cited: `crates/waml-editor/src/document_host.rs`
- Modify as cited: `crates/waml-editor/src/native_save.rs`
- Modify as cited: `crates/waml-editor/src/api_save.rs`
- Modify as cited: `crates/waml-editor/src/browser_boot.rs`
- Modify as cited: `crates/waml-editor/src/class_diagram_view.rs`
- Modify as cited: `crates/waml-editor/src/behavior_doc_view.rs`
- Modify as cited: `crates/waml-editor/src/canvas/behavior/mod.rs`
- Modify as cited: `crates/waml-editor/src/editor_session/tests.rs`
- Modify as cited: `crates/waml-editor/src/app/tests/menus.rs`
- Modify as cited: `crates/waml-editor/src/app/tests/navigation.rs`
- Modify as cited: `crates/waml-editor/src/app/tests/shell.rs`
- Modify as cited: `crates/waml-editor/src/app/tests/workspace.rs`
- Modify as cited: `crates/waml-editor/tests/editor_history.rs`
- Modify as cited: `crates/waml-editor/tests/history_integration.rs`
- Modify as cited: `crates/waml-editor/tests/markdown_authority.rs`
- Modify as cited: `crates/waml-editor/tests/markdown_integration.rs`
- Modify as cited: `crates/waml-editor/tests/view_history.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/document_ops.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/draw_layers.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/gutter.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/highlighting.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/layout_geometry.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/motion.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/presentation_constructs.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/presentation_layout.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/reading_model.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/reading_source_map.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/reading_widget_draw.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/unicode_ime.rs`
- Modify as cited: `crates/waml-markdown-editor/tests/widget_parity.rs`
- Modify as cited: `crates/waml/tests/incremental_analysis.rs`
- Modify as cited: `crates/waml/tests/flow_solver_golden.rs`
- Modify as cited: `crates/waml/tests/interaction_solver_golden.rs`
- Modify as cited: `crates/waml/tests/sequence_language_syntax.rs`
- Modify as cited: `crates/waml/tests/sequence_semantics.rs`
- Modify as cited: `crates/waml-cli/tests/cli_e2e.rs`
- Modify as cited: `crates/waml-cli/tests/lsp_e2e.rs`
- Modify as cited: `crates/waml-cli/tests/serve_e2e.rs`
- Modify as cited: `editors/vscode/src/serverPath.test.ts`
- Modify as cited: `scripts/export-site-browser.test.mjs`
- Modify as cited: `scripts/serve-browser-check.mjs`
- Modify: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`
- Create: `docs/superpowers/audits/reports/evidence.md`

**Interfaces:**
- Consumes: every frozen shipped inventory row and each proposed `test_evidence` object claimed to assert the observable Then result.
- Produces: a nearby comment that matches `Scenario: [A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+` in each valid cited test; marker flags set to `true` only for those tests; shipped source-evidenced gaps retained without fabricated markers.

- [ ] **Step 1: Add comments without changing test behavior**

For Rust, place the marker directly above `#[test]`:

```rust
// Scenario: SESSION-001
#[test]
fn savepoint_identity_tracks_undo_back_to_saved_state() {
```

For JavaScript/TypeScript, place it directly above the test call:

```javascript
// Scenario: WEB-001
test("an exported site boots and exports its model back", options, async (t) => {
```

One test can carry multiple adjacent `Scenario:` comments. Do not rename tests unless a name is factually wrong.

- [ ] **Step 2: Reject weak evidence rather than fabricating it**

If a proposed test does not assert the observable Then result, remove that object from `test_evidence` or retain it only as partial inventory evidence with `scenario_marker: false`. Keep at least one exact implementation evidence object. Set `verification_state: "gap"` and write the exact `verification_gap` reason; Tasks 6 through 10 will write the GWT scenario and its goal-document verification-gap item from this finalized row. Do not add a marker to an unrelated test. Change the row to `discrepant` only when the implementation's observable result disagrees with the intended GWT contract.

- [ ] **Step 3: Set inventory marker flags and write the report**

Set `scenario_marker: true` only for a verified adjacent marker. For a `verified` row, every test object used to satisfy the target boundary has a true marker. A `gap` row can have an empty `test_evidence` array or false markers for partial tests. The evidence report lists each scenario, implementation paths, test paths, target boundary, verification state, and gap reason.

- [ ] **Step 4: Run evidence and test gates**

Run:

```powershell
rtk cargo test -p waml-editor
rtk cargo test -p waml-markdown-editor
rtk cargo test -p waml-cli
rtk cargo test -p waml
rtk pnpm test
rtk proxy pwsh -NoProfile -Command '& { $rows = @(Get-Content "docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl" | ForEach-Object { $_ | ConvertFrom-Json }); if ($rows | Where-Object { $_.verification_state -eq "verified" -and ($_.test_evidence.Count -eq 0 -or -not ($_.test_evidence.scenario_marker -contains $true)) }) { throw "verified row has no marked test" }; if ($rows | Where-Object { $_.verification_state -eq "gap" -and (-not $_.verification_gap -or ($_.implementation_evidence.Count -eq 0 -and $_.test_evidence.Count -eq 0)) }) { throw "verification gap is not source-evidenced" } }'
```

Run `rtk pnpm test` from `editors/vscode`. Expected: PASS. The marker comments do not change behavior, every verified row has a marked real test, and every gap remains shipped and evidence-backed before goal authoring starts.

- [ ] **Step 5: Commit traceability markers**

```bash
git add crates editors/vscode/src/serverPath.test.ts scripts/export-site-browser.test.mjs scripts/serve-browser-check.mjs docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl docs/superpowers/audits/reports/evidence.md
git commit -m "test: link scenarios to evidence"
```

### Task 16: Add CI documentation gates and matching contributor commands

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`

**Interfaces:**
- Consumes: `waml check`, `waml fmt --check`, `waml index --check`, and `scripts/check-waml-doc-contract.mjs`.
- Produces: one cross-platform `WAML documentation contract` CI step and a `README.md` section with byte-identical commands.

- [ ] **Step 1: Add the CI step after Rust installation and before the general test step**

Use one named step so all four commands share the same checked-out tree:

```yaml
      - name: WAML documentation contract
        run: |
          cargo run -p waml-cli -- check docs/waml
          cargo run -p waml-cli -- fmt --check docs/waml
          cargo run -p waml-cli -- index docs/waml --check
          node scripts/check-waml-doc-contract.mjs docs/waml
```

The CLI and checker output already include a path and reason. Do not wrap failures in generic messages.

- [ ] **Step 2: Add the same commands to README development instructions**

Create a `### Documentation contract` subsection under `## Develop`. Copy the four commands in the same order and spelling. Explain that `cargo run -p waml-cli -- index docs/waml` rewrites generated indexes when the check reports stale files.

- [ ] **Step 3: Run the commands locally**

Run the four commands exactly as shown in CI.

Expected: PASS with no rewritten files.

- [ ] **Step 4: Commit CI and contributor instructions**

```bash
git add .github/workflows/ci.yml README.md
git commit -m "ci: gate WAML documentation contract"
```

### Task 17: Reconcile generated indexes and perform cross-tree integration

**Files:**
- Modify: `docs/waml/index.md`
- Modify: every generated `docs/waml/**/index.md`
- Modify: goal or architecture leaf documents only when integration finds a broken owner link
- Modify: exact `docs/waml/use-cases/actors/*.md`, `workflows/*.md`, or `views/*.md` leaves only when integration finds broken traceability or view membership
- Modify: all `docs/superpowers/audits/reports/*.md`
- Modify: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`

**Interfaces:**
- Consumes: all stream reports, complete scenario markers, the permanent product-use-case model, and the existing `reindex_source`-backed CLI.
- Produces: deterministic indexes, one cross-tree source of truth, no duplicate scenario or stale architecture claim, complete product-use-case links, and durable verification-gap records for every source-evidenced scenario without target-boundary automation.

- [ ] **Step 1: Link the contract, gaps, goals, and architecture from the root**

Ensure `docs/waml/index.md` lists `documentation-contract.md`, `waml-feature-gaps.md`, `goals/`, `use-cases/`, `architecture/`, and `user.md` through the generated model. Keep all explanatory prose in leaf documents.

- [ ] **Step 2: Run the generator in write mode once**

Run:

```powershell
rtk cargo run -p waml-cli -- index docs/waml
rtk cargo run -p waml-cli -- index docs/waml --check
```

Expected: the first command reports only generated index upserts/removals. The second exits 0 and reports no stale index. Review every removal before staging; only `index.md` paths are valid.

- [ ] **Step 3: Cross-check every inventory row and report**

Confirm:

- Every row has one owning goal document.
- Every shipped row has one unique scenario, applicable value, and implementation or test evidence.
- Every `verified` row has a target-boundary test and a true adjacent scenario marker.
- Every `gap` row has a complete reason and a matching `## Verification gaps` item in its owning goal document; it is not classified as discrepant solely because automation is absent.
- Every planned, unsupported, or discrepant row has one non-GWT record.
- Every report scenario occurs in its declared changed file.
- Every report discrepancy occurs in the inventory and goal tree.
- Every report feature gap occurs in the ledger and links back to an affected document.
- Every `uml.UseCase` document has one owning-goal link and links every shipped scenario assigned to that workflow in `reports/use-cases.md`.
- Every linked scenario occurs exactly once in its owning goal, and its fragment equals the repository slug of that heading.
- No plain, emphasized, list, or block-quote GWT body line occurs under `docs/waml/use-cases/**`.
- Every actor association occurs once, and every semantic relationship target resolves.
- The union of external-actor view members equals the complete actor-leaf set.
- The union of product-boundary view members equals the complete shipped use-case-leaf set.
- Parsed use-case views contain no `## Layout` section or layout record.

Run the complete Product Use-Case Traceability Procedure after these checks. Do not rely on `waml check` for fragments, view coverage, or copied GWT bodies.

- [ ] **Step 4: Check stale architecture claims and concept resolution**

Run:

```powershell
rtk rg -n "diagnostics-only|two stages|mutate.*restore|same operations|global binary" docs/waml/architecture
rtk cargo run -p waml-cli -- check docs/waml/architecture
```

Expected: the search returns no stale claim. The architecture check passes and resolves every diagram member.

- [ ] **Step 5: Commit integrated indexes**

```bash
git add docs/waml docs/superpowers/audits
git commit -m "docs: reconcile WAML contract indexes"
```

### Task 18: Verify native and browser boundaries

**Files:**
- Modify only when a check finds a real mismatch: the owning goal document, its exact evidence test, the inventory row, and its stream report
- Modify with each goal or scenario correction: the corresponding `docs/waml/use-cases/workflows/*.md` leaf and `docs/superpowers/audits/reports/use-cases.md`
- Modify when the corrected workflow changes a role or boundary: its exact `docs/waml/use-cases/actors/*.md` leaf and affected `docs/waml/use-cases/views/*.md` view
- Do not commit: `target/docs-contract-start.png`
- Do not commit: `target/docs-contract-native.png`

**Interfaces:**
- Consumes: final scenarios, their test boundaries, the integrated product-use-case links, and `reports/use-cases.md`.
- Produces: passing available suites, target-boundary verification for verified scenarios, durable gap records for unautomated shipped scenarios, corrected product-use-case traceability, and screenshots for manual review of remaining native surfaces.

- [ ] **Step 1: Run the normative native suites**

Run:

```powershell
rtk cargo test -p waml-editor
rtk cargo test -p waml-markdown-editor
rtk cargo test -p waml
```

Expected: PASS. These suites supply target-boundary evidence for rows marked `verified`. A shipped scenario not asserted by these suites remains `verification_state: "gap"`; the passing suite does not close it by association.

- [ ] **Step 2: Inspect the start and recent workflow in a titled window**

Run:

```powershell
rtk proxy pwsh -File .\run.ps1 -Empty -Title docs-contract-start
rtk proxy pwsh -File scripts/capture-window.ps1 -Out target/docs-contract-start.png -Process waml-editor
```

Expected: the titled window shows the start screen. The screenshot uses native pixels. Close the window before the next launch.

- [ ] **Step 3: Inspect navigation, tabs, Markdown, and diagrams in a titled window**

Run:

```powershell
rtk proxy pwsh -File .\run.ps1 crates/waml-editor/tests/fixtures/mini -Title docs-contract-native
rtk proxy pwsh -File scripts/capture-window.ps1 -Out target/docs-contract-native.png -Process waml-editor
```

Expected: the titled window opens the staged mini fixture. Exercise only inventory rows marked as manual review. Record an observed implementation/GWT mismatch as `discrepant`; do not silently edit the claimed behavior. Manual confirmation does not become automated test evidence, so a row without a target-boundary test stays a verification gap.

- [ ] **Step 4: Run browser-only host and artifact tests**

Run:

```powershell
rtk cargo test -p waml-editor browser_boot
rtk cargo test -p waml-cli --test serve_e2e
rtk node --test "scripts/*.test.mjs"
```

Expected: PASS. A headed exported-site test can report SKIP only when its documented external binary or Playwright prerequisite is absent. A source-evidenced scenario that depends on that skipped result remains shipped with `verification_state: "gap"`; the gap reason names the skipped or absent prerequisite. Use `discrepant` only if an executed check observes behavior that conflicts with the contract.

- [ ] **Step 5: Run the explicit browser/API check when its prerequisites exist**

Run the repository-documented form:

```powershell
rtk node scripts/serve-browser-check.mjs target/release/waml.exe crates/waml-editor/tests/fixtures/mini
```

Expected: `serve-browser-check: PASS`. If the executable or Chromium prerequisite is absent, build or provide it when the repository supports that action. Otherwise retain affected source-evidenced scenarios as shipped gaps and name the missing prerequisite. Do not convert them to discrepancies.

- [ ] **Step 6: Reconcile and revalidate product-use-case traceability**

After each correction to a goal, scenario identifier, scenario title, workflow owner, actor role, or system boundary, update the corresponding use-case leaf, fragment, view membership, and `reports/use-cases.md` row in the same change. Run the complete Product Use-Case Traceability Procedure after each correction. Expected: one owner per semantic intention, exact fragment-to-heading matches, no copied GWT body, no parsed layout record, and complete actor/use-case view membership.

- [ ] **Step 7: Commit only evidence and traceability corrections**

If no mismatch exists, do not make an empty commit. If a mismatch exists, stage only the owner document, exact test, inventory row, stream report, corresponding product-use-case files, and `reports/use-cases.md`, then commit:

```bash
git commit -m "docs: reconcile verification evidence"
```

### Task 19: Run final validation and remove temporary coordination files

**Files:**
- Delete: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.schema.json`
- Delete: `docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl`
- Delete: `docs/superpowers/audits/reports/contract.md`
- Delete: `docs/superpowers/audits/reports/read-shell.md`
- Delete: `docs/superpowers/audits/reports/author-trust.md`
- Delete: `docs/superpowers/audits/reports/class-shared.md`
- Delete: `docs/superpowers/audits/reports/behavior-diagrams.md`
- Delete: `docs/superpowers/audits/reports/browser-tooling.md`
- Delete: `docs/superpowers/audits/reports/use-cases.md`
- Delete: `docs/superpowers/audits/reports/architecture.md`
- Delete: `docs/superpowers/audits/reports/evidence.md`

**Interfaces:**
- Consumes: a fully reconciled inventory and reports whose information now lives in `docs/waml` and evidence markers.
- Produces: the final product source of truth with no parallel audit source; all local and CI-equivalent gates green.

- [ ] **Step 1: Prove the inventory is fully discharged before deletion**

Run:

```powershell
rtk proxy pwsh -NoProfile -Command '& { $rows = @(Get-Content "docs/superpowers/audits/2026-08-08-ui-behavior-inventory.jsonl" | ForEach-Object { $_ | ConvertFrom-Json }); $canonical = "^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$"; $allocated = "^[A-Z][A-Z0-9]*-[0-9]{3}$"; foreach ($row in $rows) { $body = Get-Content -Raw $row.goal_document; if ($row.state -eq "shipped") { if (-not $row.scenario_id -or $row.scenario_id -notmatch $canonical) { throw ($row.behavior_id + " has no canonical scenario") }; if ($row.scenario_id_origin -eq "allocated" -and $row.scenario_id -notmatch $allocated) { throw ($row.behavior_id + " has an invalid allocated scenario") }; if ($row.implementation_evidence.Count -eq 0 -and $row.test_evidence.Count -eq 0) { throw ($row.behavior_id + " has no shipped evidence") }; if (-not $body.Contains($row.scenario_id)) { throw ($row.behavior_id + " scenario is absent from " + $row.goal_document) }; if ($row.verification_state -eq "verified") { if ($row.test_evidence.Count -eq 0 -or -not ($row.test_evidence.scenario_marker -contains $true)) { throw ($row.behavior_id + " verified evidence is not marked") } } elseif ($row.verification_state -eq "gap") { if (-not $row.verification_gap) { throw ($row.behavior_id + " has no verification-gap reason") }; $gapPattern = "(?m)^- " + [regex]::Escape($row.scenario_id) + " — target: " + [regex]::Escape($row.verification_boundary) + "; "; if ($body -notmatch $gapPattern) { throw ($row.behavior_id + " gap is absent from " + $row.goal_document) } } else { throw ($row.behavior_id + " has invalid shipped verification state") } } else { if ($row.verification_state -ne "not_applicable") { throw ($row.behavior_id + " has invalid non-shipped verification state") }; if (-not $body.Contains($row.behavior_id)) { throw ($row.behavior_id + " record is absent from " + $row.goal_document) } } } }'
```

Expected: exit 0 with no output.

Run the complete Product Use-Case Traceability Procedure before you delete the inventory or `reports/use-cases.md`. Expected: the report, semantic-intention owners, scenario fragments, GWT-body scan, parsed layout state, and complete view-member sets agree.

- [ ] **Step 2: Run documentation gates exactly as CI runs them**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml
rtk cargo run -p waml-cli -- fmt --check docs/waml
rtk cargo run -p waml-cli -- index docs/waml --check
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: all four commands pass. No command changes a file.

- [ ] **Step 3: Run repository regression gates**

Run:

```powershell
rtk cargo fmt --check
rtk cargo test --workspace
rtk cargo test --workspace --doc
rtk cargo clippy --workspace --all-targets --all-features -- -D warnings
rtk node --test "scripts/*.test.mjs"
rtk pnpm build
rtk pnpm test
rtk pnpm lint
```

Run the final three commands from `editors/vscode`. Expected: PASS.

- [ ] **Step 4: Run final cross-tree searches**

Run:

```powershell
rtk rg -n "\*\*Status:\*\* (implemented|.*unverified)" docs/waml
rtk rg -n "^#### [A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+ — " docs/waml/goals
rtk rg -n "(?i)^\s*(?:[-*>]\s*)?(?:\*\*|__)?(Given|When|Then|And)(?:\*\*|__)?(?:\s|$)" docs/waml/use-cases
rtk git diff --check
```

Expected: the forbidden-status and complete copied-GWT searches return no matches. The scenario list contains unique identifiers already accepted by the checker. The Product Use-Case Traceability Procedure has already proved parsed layout absence and complete view coverage. `git diff --check` returns no error.

- [ ] **Step 5: Delete the temporary inventory and reports**

Delete only the exact files listed in this task. Do not delete `docs/waml/documentation-contract.md` or `docs/waml/waml-feature-gaps.md`. The traceability source of truth is now the goal tree, architecture tree, test markers, and ledger.

- [ ] **Step 6: Re-run the documentation gates after deletion**

Run:

```powershell
rtk cargo run -p waml-cli -- check docs/waml
rtk cargo run -p waml-cli -- fmt --check docs/waml
rtk cargo run -p waml-cli -- index docs/waml --check
rtk node scripts/check-waml-doc-contract.mjs docs/waml
```

Expected: PASS. Nothing under `docs/waml` depended on the temporary audit paths.

Repeat steps 1 and 3 through 7 of the Product Use-Case Traceability Procedure directly from `docs/waml/use-cases/**`. Skip only the report-input and report-comparison steps because Task 19 deleted the report. Expected: goal ownership, fragments, GWT-body absence, parsed layout absence, and complete actor/use-case view membership remain valid without audit scaffolding.

- [ ] **Step 7: Commit cleanup and final state**

```bash
git add -u docs/superpowers/audits
git commit -m "docs: remove behavior audit scaffolding"
```

- [ ] **Step 8: Record the final review result**

The final reviewer must state all five results explicitly: no unowned behavior, no duplicate contract, no stale architecture claim, no orphan or copied product-use-case contract, and no unresolved validation error. If any result is false, do not merge until the owning task corrects it.
