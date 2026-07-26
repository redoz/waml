# Fixed Font Raster Ladder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the canvas font-size LRU with a deterministic fixed geometric raster-size ladder.

**Architecture:** A pure `font_raster_size(target_size) -> f32` selector owns the policy. Drawing continues to preserve exact visual size with `font_scale = target_size / raster_size`; no mutable cache state or timer participates.

**Tech Stack:** Rust 2021, Makepad `DrawText`, built-in Rust tests.

## Global Constraints

- Work only in the `font-size-lru` worktree.
- Do not change `node_design_editor.rs`.
- Preserve direct target sizes at or below 32 points.
- Use ladder sizes `32, 40, 50, 63, 79, 99, 124, 155, 194, 243, 304`.

---

### Task 1: Pure raster-size policy

**Files:**
- Modify: `crates/waml-editor/src/canvas.rs`
- Test: `crates/waml-editor/src/canvas.rs`

**Interfaces:**
- Consumes: a target visual font size in points.
- Produces: `fn font_raster_size(target_size: f32) -> f32`.

- [ ] **Step 1: Write failing selection tests**

Test that sizes at or below 32 remain direct, values above 32 choose the nearest
ladder rung, exact midpoints resolve upward, and values beyond the ladder choose
304.

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
cargo test -p waml-editor canvas::tests::font_raster_size
```

Expected: compilation failure because `font_raster_size` does not exist.

- [ ] **Step 3: Implement the selector and simplify drawing state**

Add the fixed ladder and nearest-rung selector. Remove `zoom_dwell_timer`,
`is_zooming`, `font_size_lru`, cache counters, `note_zooming`,
`pick_font_size`, timer routing, and cache logging. Replace call sites with the
pure selector.

- [ ] **Step 4: Revert unused node-design-editor changes**

Remove the four added `font_scale = 1.0` assignments from
`node_design_editor.rs`, leaving that file identical to `HEAD`.

- [ ] **Step 5: Verify GREEN and regressions**

Run:

```powershell
cargo fmt --all
cargo test -p waml-editor
cargo clippy -p waml-editor --all-targets -- -D warnings
git diff --check
```

Expected: all commands exit successfully.
