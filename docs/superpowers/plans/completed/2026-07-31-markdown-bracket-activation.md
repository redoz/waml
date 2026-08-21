# Markdown Bracket Activation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Parse nested Markdown link and image labels with CommonMark bracket activation while preserving exact source and semantic reference ownership.

**Architecture:** Keep the current destination, reference, semantic annotation, and green-node builders. Add a bracket scan that records opener position, image status, and active status; match each closing bracket with the nearest active opener, deactivate earlier link openers after a successful link, and keep image openers active.

**Tech Stack:** Rust, `waml-syntax` green/red trees, Cargo integration tests.

## Global Constraints

- Preserve exact source spelling and ranges.
- Do not add Task 8 scheduling behavior.
- Preserve the unrelated `crates/waml-editor/src/app.rs` change.
- Use ASD-STE100 Simplified Technical English.

---

### Task 1: Nested link and image bracket activation

**Files:**
- Modify: `crates/waml-syntax/src/markdown/inline.rs`
- Test: `crates/waml-syntax/tests/markdown_inlines.rs`
- Modify: `.superpowers/sdd/2026-07-31-markdown-syntax-platform/task-3-report.md`

**Interfaces:**
- Consumes: `parse_inlines`, `parse_link`, `inline_destination`, reference definitions, semantic link annotations.
- Produces: explicit bracket opener matching and correctly nested `Link`/`Image` green nodes.

- [x] **Step 1: Write failing nested bracket tests**

Add real-parser regressions for:

```text
[outer [inner](/x)](/y)
![outer [inner](/x)][img]

[img]: /image
```

Assert exact source, node containment, ordered destinations, and the reference backlink owner for `img`.

- [x] **Step 2: Verify the tests fail**

Run:

```text
rtk cargo test -p waml-syntax --test markdown_inlines nested
```

Expected: the current first-`]` parser does not produce the required node nesting and destinations.

- [x] **Step 3: Implement bracket opener records**

Add a private opener record with source position, image status, and active status. Scan labels left-to-right, match each `]` with the nearest eligible opener, and return the successful link/image spans. After a successful link, deactivate earlier link openers; do not deactivate image openers.

- [x] **Step 4: Build nodes from matched spans**

Use the existing destination/reference parsing and semantic annotation paths. Parse each matched label range with the successful nested spans, so links cannot contain active links but images can contain links.

- [x] **Step 5: Verify focused and full suites**

Run:

```text
rtk cargo test -p waml-syntax --test markdown_inlines
rtk cargo test -p waml-syntax
rtk cargo fmt --check
rtk git diff --check
```

Expected: all commands exit with code `0`.

- [x] **Step 6: Report and commit**

Append the red/green evidence and final command output to the Task 3 report. Stage only Task 3 files and commit with a terse Conventional Commits message.
