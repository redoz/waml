# Implementation Plans

This directory is a **worklist, not an archive**. Anything sitting at the top
level is a claim that work is outstanding. If that stops being true, the file
moves — see [When a plan moves](#when-a-plan-moves).

Last triaged: **2026-08-21** (audit finding A39, "planning hygiene").

## Layout

| Path | What lives here |
| --- | --- |
| `*.md` | **Active plans.** Outstanding, partially done, or deliberately deferred work. Every one carries a verdict in the table below. |
| `<date>-<slug>/` | An active plan too big for one file: a `README.md` task index plus `task-N-*.md`, one task each. Treated exactly like a top-level `*.md`. |
| `completed/` | Plans whose work **landed on `main`**. Never deleted — history and rationale stay readable. Also holds directory-form plans. |
| `drafts/` | Plans not yet approved for implementation. Nothing here is scheduled. |
| `notes/` | Findings and verification records that are not plans and were never meant to be executed. |
| `../specs/` | The design document a plan implements. A plan usually names its spec near the top. |

`implement-plan` takes a **top-level** path only — never `drafts/`, never
`completed/`, never a nested file inside a plan directory (hand it the
directory instead).

## When a plan moves

Move a plan to `completed/` with `git mv` (never `rm`, never copy — history
follows the file) as soon as **its artifacts exist on `main`**. That is the
whole test. Not "the branch is green", not "the PR is open": the named files,
types and commands the plan describes are in the tree on `main`.

Three things routinely go wrong and are worth naming:

- **The `implement-plan` runner archives for you; a hand-run does not.** Plans
  implemented by hand, in `local` mode, or in `dry-run` mode stay put unless
  someone moves them. That is how this directory reached 74 active plans while
  63 of them had already shipped.
- **Checkbox state is not progress.** The runner records progress in git
  `Plan-Tasks` commit trailers, not by ticking `- [ ]`. Of the 63 plans
  archived in the A39 pass, all but a handful still had every box unticked.
  Judge by artifacts, never by checkboxes.
- **A plan that dies still moves — but not to `completed/`.** `completed/`
  means *landed*. Abandoned and superseded plans stay at the top level with a
  dated `## Status` section stating why and what replaced them, so nobody
  re-executes them. They are still listed below, marked, and they are not
  outstanding work.

If you land only part of a plan, do not move it. Add a dated `## Status`
section directly under the title saying precisely what landed (with a file path
or commit SHA) and what did not.

## Active plans

11 plans plus 1 plan directory. **Only three carry outstanding work**
(`PARTIAL`/`HORIZON`); the rest are kept for rationale and are marked so they
do not read as in-flight.

| Plan | Verdict | Evidence |
| --- | --- | --- |
| `2026-08-04-issue-28-guard-single-authority.md` | **PARTIAL** | Task A subsumed by `completed/2026-08-04-frontmatter-yaml-alignment.md`. **B, C, D outstanding:** no `resolution` field on `BracketMatch` (`crates/waml-syntax/src/markdown/inline.rs:262`); two rival `frontmatter_value` extractors (`crates/waml/src/okf/lower.rs:508`, `crates/waml/src/uml/lower.rs:909`); debug oracle still compares island counts (`crates/waml-syntax/src/incremental.rs:334`). |
| `2026-08-02-root-folder-toggle.md` | **PARTIAL** | No-reset half landed (`crates/waml-editor/src/app/tests/navigation.rs:1525`). The toggle-open-and-closed goal was overtaken by `completed/2026-08-05-folder-view-middleware.md`; needs a product decision before any code. |
| `2026-08-02-web-text-shader-boot.md` | **PARTIAL** | Edits the makepad fork, not this repo — unverifiable from here. The boot number it targets already moved (31–38 s → ~9 s → ~1.7 s) via `completed/2026-08-02-web-batched-shader-link.md`. The `DrawTextSlug` migration itself is unconfirmed. |
| `2026-08-05-atproto-collab.md` | **HORIZON** | Unstarted: no `crates/waml-collab/`, no `automerge` dependency. **MVP-scale:** 13 tasks, a new crate, ~1.1 MB added to the wasm artifact, and OAuth deliberately excluded at a self-estimated ~40% of total work. CRDT choice (automerge) is correct — makepad's wasm is not wasm-bindgen based, so JS CRDTs are not available. |
| `2026-08-04-issue-triage-index.md` | **PARTIAL (index)** | Landing-order index, not a plan. 15 of 16 findings closed; issue 28 remains. Archive it when issue 28 closes. |
| `2026-07-12-straighten-edges-shared-band.md` | **ABANDONED** | Never started. Target files (`RelEdge.svelte`, `floating.ts`) deleted by `eae57286 refactor: retire legacy web and WASM stack`. |
| `2026-07-15-diagram-properties-panel-v2.md` | **SUPERSEDED** | Never started. Replaced by `completed/2026-07-26-native-diagram-properties.md`. |
| `2026-07-16-diagram-display-controls-refresh.md` | **SUPERSEDED** | Never started as written; its model goal landed natively (`show_type` in `crates/waml-editor/src/diagram_display.rs:6`; `attributeDetail` gone). |
| `2026-07-16-diagram-properties-body-cleanup.md` | **ABANDONED** | Never started. `DiagramPropertiesBody.svelte` deleted by `eae57286`. |
| `2026-07-16-model-navigator-switcher-redesign.md` | **ABANDONED** | Never started. `TopBar`/`NavigatorPanel` deleted by `eae57286`. |
| `2026-07-17-prose-solved-diagram-rendering.md` | **SUPERSEDED** | Never started. Replaced by `completed/2026-07-17-makepad-diagram-viewer.md` + `completed/2026-07-22-orthogonal-edge-router.md`. |
| `2026-07-17-ontology-substrate-seam-slice1/` | **ABANDONED** | Started, then reverted; survives only as the tag `archive/ontology-substrate-seam-slice1`. Its wire-projection premise died with the TypeScript frontend. |

The six web-era `ABANDONED`/`SUPERSEDED` entries share one cause: they target
`packages/web`, `packages/core`, `packages/okf` or `crates/waml-wasm`, all
deleted on 2026-07-28 by `eae57286 refactor: retire legacy web and WASM stack`
(called for by `completed/2026-07-27-first-class-okf-documents.md`). Any plan
older than that date naming a `.svelte` file or a `packages/` path is dead by
construction — check that before reading further into it.

## Other directories

- `drafts/` — `2026-08-08-source-as-navigation.md`. Not duplicated by any
  active plan; overlaps `completed/2026-08-08-surface-routed-navigation.md`
  and `completed/2026-07-25-view-right-dock-seam.md`, so re-read it against
  what shipped before promoting it.
- `notes/` — `2026-07-15-tsify-spike-findings.md` (stale: tsify went with the
  TypeScript stack in `eae57286`, kept as a record of why) and
  `2026-07-24-recents-pinning-verification.md` (a verification record for
  `completed/2026-07-24-recents-pinning-vs-rows.md`).

Neither directory was reorganised in the A39 pass.
