# Recent Row Typography Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Recent item typography smaller and restore visible space below its path without changing the row pitch or interactions.

**Architecture:** Keep the row layout change inside `RecentRowView`, but define its 10/9 px styles in the centralized `mod.fonts` scale because the chrome typography gate rejects ad-hoc sizes. Extend the Fonts style-guide overlay and its role-coverage test; the user approved the TDD configuration exception for the live-design spacing itself because the repository has no runtime layout harness.

**Tech Stack:** Rust, Makepad `script_mod!` live design, Cargo tests

## Global Constraints

- Keep the Recent row at 48 px tall.
- Render the title at 10 px and the relative time/path at 9 px.
- Vertically center the time against the title.
- Leave shared typography tokens and row interactions unchanged.

---

### Task 1: Compact the Recent row

**Files:**
- Modify: `crates/waml-editor/src/recent_row.rs:77-114`
- Modify: `crates/waml-editor/src/fonts.rs:1-113`
- Modify: `crates/waml-editor/src/fonts_overlay.rs:30-199`
- Modify: `crates/waml-editor/src/script_gate.rs:90-120`

**Interfaces:**
- Consumes: the existing centralized typography scale and `RecentRowView` live-design hierarchy.
- Produces: `fonts.text_compact_label`, `fonts.text_micro`, and an unchanged `RecentRowView` public API and `ROW_HEIGHT`.

- [ ] **Step 1: Establish the clean baseline**

Run:

```powershell
rtk cargo test -p waml-editor
rtk cargo build -p waml-editor
```

Expected: both commands exit 0 before the live design changes.

- [ ] **Step 2: Add the compact centralized roles**

Add `fonts.text_compact_label` as IBM Plex Sans Medium 10 px and `fonts.text_micro` as IBM Plex Sans Regular 9 px. Add both to the Fonts overlay, update its coverage test from eight to ten roles, and update the script namespace gate's exact key set from eight to ten.

- [ ] **Step 3: Apply the minimal live-design change**

Update only the existing declarations:

```rust
textcol := View {
    width: Fill
    height: Fit
    flow: Down
    spacing: 0.0

    titlerow := View {
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        title := Label {
            width: Fill
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: fonts.text_compact_label
            }
        }
        when := Label {
            text: ""
            draw_text +: {
                color: atlas.text_dim
                text_style: fonts.text_micro
            }
        }
    }

    path := Label {
        text: ""
        margin: Inset{bottom: 4.0}
        draw_text +: {
            color: atlas.text_dim
            text_style: fonts.text_micro
        }
    }
}
```

- [ ] **Step 4: Verify the crate**

Run:

```powershell
rtk cargo test -p waml-editor
rtk cargo build -p waml-editor
```

Expected: all commands exit 0 with no failing tests.

- [ ] **Step 5: Verify the native rendering**

Launch the worktree build, open the start screen with at least one Recent item, and capture it:

```powershell
rtk cargo run -p waml-editor
pwsh -File scripts/capture-window.ps1 -Out recent-row-typography.png -Process waml-editor
```

Confirm that the title is visibly larger than the metadata, the time is centered against the title, the path has space below it, and the row remains 48 px tall.

- [ ] **Step 6: Commit**

```powershell
rtk git add crates/waml-editor/src/fonts.rs crates/waml-editor/src/fonts_overlay.rs crates/waml-editor/src/recent_row.rs crates/waml-editor/src/script_gate.rs docs/superpowers/specs/2026-07-28-recent-row-typography-design.md docs/superpowers/plans/2026-07-28-recent-row-typography.md
rtk git commit -m "fix(recents): tighten row typography"
```
