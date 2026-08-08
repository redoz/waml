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

# Documentation Contract

This contract defines the product behavior records in `docs/waml`. Shipped
behavior on `origin/main` is normative. Each behavior has one owning goal
document. Related documents link to that owner and do not copy its scenario.

## Metadata and status

Use OKF v0.2 fields for provenance, trust review, lifecycle, and freshness.
Use `sources` for implementation, test, or normative-document provenance. Use
`generated` for a derived document and `verified` for its recorded review.
Use `stale_after` as an absolute review date.

The Markdown body field `**Status:**` states product completion. It has only
these values: `done`, `partial`, `planned`, and `horizon`. Its value follows
shipped scenario coverage and evidence.

The frontmatter `status` field states the OKF lifecycle. It has the independent
values `draft`, `stable`, and `deprecated`. If frontmatter has no `status`, its
value is `stable`. Do not copy a body product status into frontmatter.

Trust tier is derived from `verified` and is never stored. For new v0.2
documents, `generated.at` records the generation time and supersedes
`timestamp`. Preserve `timestamp` only when a legacy v0.1 input requires it.

## Stable scenarios

A stable scenario has this exact field order: identifier, applicability,
Given and optional And state, When action, Then and optional And result, and
evidence. The heading form is `#### SCENARIO-ID — lower-case behavior`. The
following field form follows the heading:

```markdown
**Applies to:** shared|native|browser

**Given** one observable state
**And** one additional state, when required
**When** one user action
**Then** one observable result
**And** one additional result, when required

**Evidence:** `repository-relative-path::exact_symbol` or `repository-relative-path:line`
```

Use active voice, present tense, semantic targets, and observable results. Do
not expose Rust operations or fixed coordinates unless a coordinate is the
contract. A shared scenario has one normative native verification target. A
browser scenario has a browser verification target. A browser test is for
browser-only behavior or an explicit native/browser parity seam.

The compatible identifier grammar is
`^[A-Z][A-Z0-9]*(?:-[A-Z][A-Z0-9]*)*-[0-9]+$`. Preserve every existing
identifier verbatim, including multi-segment identifiers such as `SEQ-MSG-1`.
Reserve `PREFIX-NNN` for newly allocated identifiers. An allocated identifier
uses its owner prefix, a three-digit ascending number, and the grammar
`^[A-Z][A-Z0-9]*-[0-9]{3}$`. The wider grammar keeps stable existing identifiers;
it does not permit their renumbering.

| Prefix | Behavior owner |
| --- | --- |
| `BUNDLE` | Start, recents, open, close, save, and export. |
| `SHELL` | Responsive shell, docks, splitters, overlays, popups, and theme. |
| `NAV` | Tree, folders, breadcrumbs, external links, reveal, and view history. |
| `TAB` | Preview tabs, pinned tabs, document switching, and presentation switching. |
| `MDREAD` | Markdown reading and selection in read-only presentation. |
| `MDEDIT` | Markdown editing, clipboard, multi-caret, and IME. |
| `SESSION` | Undo, redo, savepoints, dirty state, diagnostics, and status feedback. |
| `CLASS` | Class-diagram selection, tools, manipulation, properties, layout, conflicts, and solver feedback. |
| `ACT` | Activity rendering, hit testing, selection, and camera behavior. |
| `SEQ` | Sequence rendering, hit testing, selection, and camera behavior. |
| `WEB` | Browser boot, URL, share, site, download, API, and local serve behavior. |
| `CLI` | Command-line workflows. |
| `LSP` | Language-server workflows. |
| `VSC` | VS Code integration workflows. |

## Planned behavior

Each list item starts with its frozen `BHV-*` identifier. It states the product
intention and states that it has no passing acceptance scenario.

## Unsupported behavior

Each list item starts with its frozen `BHV-*` identifier. It states that
`origin/main` does not support the workflow.

## Discrepancies

Each list item starts with its frozen `BHV-*` identifier. It states the visible
claim, the observed result, and exact `path:line` evidence.

## Verification gaps

A verification-gap item has this form: `- SCENARIO-ID — target: native|browser;
Complete sentence.`

It refers to a shipped GWT scenario in the same document. It records missing
target-boundary automation without changing the scenario or product state. A
missing target-boundary test is a verification gap, not a discrepancy.

## Evidence and traceability

Evidence uses an exact repository-relative source or test reference. A marked
test verifies the observable Then result at the scenario target boundary. A
source-evidenced shipped scenario without sufficient target-boundary automation
remains shipped and has a verification gap.

Each shipped scenario identifier links through one product use case to its
test and other evidence. The permanent product-use-case layer supplies those
links after the goal documents exist. This traceability need is recorded in
[FG-010](./waml-feature-gaps.md).
