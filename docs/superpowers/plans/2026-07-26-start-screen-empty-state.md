# In-Canvas Start Screen Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the no-model launcher card with a compact in-canvas start screen over a full-width, subdued WAML wordmark.

**Architecture:** Keep `StartScreen` as the event and FlatList owner, but replace its card tree with two overlay layers: a non-interactive `View` that reuses `mod.draw.LogoMark`, and a centered compact content column. Pure helpers calculate the responsive wordmark size and cap copied recent rows so layout behavior is unit-testable without a live Makepad context.

**Tech Stack:** Rust, Makepad `script_mod!` UI DSL, Makepad widget tests, Cargo.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\start-screen-empty-state` on `codex/start-screen-empty-state`.
- Reuse the existing six-segment `mod.draw.LogoMark`; do not duplicate its geometry.
- Keep the background logo's segment order `HI, LO, LO, MID, MID, HI`.
- Render no launcher card, border, subtitle, START heading, or keyboard-shortcut badges.
- Show no more than five recent documents.
- Keep each recent age on the model-name line and right-align all ages to one column.
- Preserve recent-row open and pin interactions.

---

### Task 1: Lock down responsive sizing and recent-count behavior

**Files:**
- Modify: `crates/waml-editor/src/start_screen.rs`

**Interfaces:**
- Consumes: `DVec2`, `RecentRow`, and the rows passed to `StartScreen::set_recents`.
- Produces: `backdrop_logo_size(available: DVec2) -> DVec2`, `cap_recent_rows(rows: Vec<RecentRow>) -> Vec<RecentRow>`, and `MAX_RECENTS: usize`.

- [x] **Step 1: Write failing unit tests**

Add tests that require a five-item maximum and a responsive logo that preserves the existing `1.749` aspect ratio while fitting inside horizontal and vertical margins:

```rust
#[test]
fn recent_rows_are_capped_at_five() {
    let rows = (0..7).map(|i| row(&format!("/{i}"))).collect();
    let capped = cap_recent_rows(rows);
    assert_eq!(capped.len(), 5);
    assert_eq!(capped[4].path, "/4");
}

#[test]
fn backdrop_logo_is_nearly_full_width_and_preserves_aspect() {
    let size = backdrop_logo_size(dvec2(1536.0, 958.0));
    assert_eq!(size.x, 1440.0);
    assert!((size.x / size.y - LOGO_ASPECT).abs() < 0.0001);
}

#[test]
fn backdrop_logo_shrinks_to_fit_short_viewports() {
    let size = backdrop_logo_size(dvec2(1200.0, 500.0));
    assert!(size.x <= 1104.0);
    assert!(size.y <= 404.0);
    assert!((size.x / size.y - LOGO_ASPECT).abs() < 0.0001);
}
```

- [x] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
rtk cargo test -p waml-editor start_screen::tests
```

Expected: compilation fails because `cap_recent_rows`, `backdrop_logo_size`, and `LOGO_ASPECT` do not exist.

- [x] **Step 3: Implement the pure helpers**

Add:

```rust
const MAX_RECENTS: usize = 5;
const LOGO_ASPECT: f64 = 1.749;
const LOGO_MARGIN: f64 = 48.0;

fn cap_recent_rows(mut rows: Vec<RecentRow>) -> Vec<RecentRow> {
    rows.truncate(MAX_RECENTS);
    rows
}

fn backdrop_logo_size(available: DVec2) -> DVec2 {
    let max_width = (available.x - LOGO_MARGIN * 2.0).max(0.0);
    let max_height = (available.y - LOGO_MARGIN * 2.0).max(0.0);
    let width = max_width.min(max_height * LOGO_ASPECT);
    dvec2(width, width / LOGO_ASPECT)
}
```

Call `cap_recent_rows` from `set_recents`.

- [x] **Step 4: Run the focused tests and verify GREEN**

Run:

```powershell
rtk cargo test -p waml-editor start_screen::tests
```

Expected: all `start_screen::tests` pass.

### Task 2: Replace the launcher card with the in-canvas composition

**Files:**
- Modify: `crates/waml-editor/src/start_screen.rs`
- Modify: `crates/waml-editor/src/recent_row.rs`
- Modify: `crates/waml-editor/src/app.rs`

**Interfaces:**
- Consumes: `mod.draw.LogoMark`, `backdrop_logo_size`, `RecentRowView::ROW_HEIGHT`, `ActionLink`, and the existing FlatList row event routing.
- Produces: `recent_list_height(row_count: usize) -> f64`, `foreground_width(available_width: f64) -> f64`, and a two-layer overlay start screen with a responsive background wordmark and a compact five-row foreground list.

- [x] **Step 1: Write failing recent-list height behavior tests**

Add tests for visible list-height behavior without locking the row-height
constant itself:

```rust
#[test]
fn empty_recent_list_reserves_one_placeholder_row() {
    assert_eq!(recent_list_height(0), RecentRowView::ROW_HEIGHT);
}

#[test]
fn recent_list_reserves_one_height_per_row() {
    assert_eq!(recent_list_height(3), 3.0 * RecentRowView::ROW_HEIGHT);
}

#[test]
fn recent_list_never_reserves_more_than_five_rows() {
    assert_eq!(recent_list_height(8), 5.0 * RecentRowView::ROW_HEIGHT);
}

#[test]
fn foreground_uses_compact_width_when_space_allows() {
    assert_eq!(foreground_width(1280.0), 440.0);
}

#[test]
fn foreground_keeps_safe_margins_in_narrow_viewports() {
    assert_eq!(foreground_width(400.0), 352.0);
    assert_eq!(foreground_width(20.0), 0.0);
}
```

- [x] **Step 2: Run the focused tests and verify RED**

Run:

```powershell
rtk cargo test -p waml-editor start_screen::tests
```

Expected: compilation fails because `recent_list_height` does not exist.

- [x] **Step 3: Implement and verify the recent-list height helper**

Add:

```rust
fn recent_list_height(row_count: usize) -> f64 {
    row_count.clamp(1, MAX_RECENTS) as f64 * RecentRowView::ROW_HEIGHT
}

fn foreground_width(available_width: f64) -> f64 {
    (available_width - 48.0).max(0.0).min(440.0)
}
```

Run:

```powershell
rtk cargo test -p waml-editor start_screen::tests
```

Expected: all `start_screen::tests` pass.

- [x] **Step 4: Implement the compact row**

In `recent_row.rs`, set the root row to a fixed `48.0` height, reduce horizontal padding/spacing, keep `when` inside `titlerow`, and retain the same text-role tokens:

```rust
width: Fill
height: 48.0
padding: Inset{left: 0.0, right: 0.0, top: 2.0, bottom: 2.0}
spacing: 8.0
```

Set:

```rust
pub const ROW_HEIGHT: f64 = 48.0;
```

- [x] **Step 5: Replace the start-screen DSL tree**

Change the root to `flow: Overlay`. Add a full-window background host containing:

```rust
backdrop_logo := View {
    show_bg: true
    draw_bg: mod.draw.LogoMark {
        fade: 0.07
    }
}
```

Add a second full-window centered vertical-scroll host with a responsive
`Fit` content column, capped at `440.0` wide with safe narrow-window margins,
containing only:

```text
Create a new model
Open a model

RECENT
[up to five compact rows]
```

Remove the card surface/frame, animated splash widget, subtitle, divider, START eyebrow, two-column body, and list border. Keep the existing action-link IDs and FlatList IDs so event routing remains unchanged.

- [x] **Step 6: Size the background and list hosts during draw**

At the start of `StartScreen::draw_walk`, use:

```rust
let available = cx.peek_walk_turtle(walk).size;
let logo_size = backdrop_logo_size(available);
```

Push `logo_size` into `backdrop_logo.walk.width/height`. Set the transparent recent-list host height to:

```rust
let list_height = recent_list_height(self.rows.len());
```

Push `foreground_width(available.x)` into the content column width.

Delete `seat_subtitle_baseline` and the old five-row framed-box sizing.

- [x] **Step 7: Update stale comments and verify GREEN**

Update `app.rs` and module comments so they describe the five-item in-canvas launcher rather than a scrollable dialog card.

Run:

```powershell
rtk cargo test -p waml-editor
rtk cargo build -p waml-editor
```

Expected: `602+` tests pass and the editor builds with no new warnings.

- [x] **Step 8: Launch and visually verify**

Run the editor with no model argument, then capture it:

```powershell
rtk cargo run -p waml-editor --bin waml-editor
pwsh -File scripts/capture-window.ps1 -Out start-screen.png -Process waml-editor
```

Verify:

- The WAML mark spans almost the full canvas width without stretching.
- Both outer segments remain visible and the mark stays subdued.
- The foreground has no card, subtitle, START label, or shortcut badges.
- Exactly five or fewer recent rows render.
- Ages align with model-name baselines.
- New/Open and recent-row interactions still work.

- [ ] **Step 9: Commit**

```powershell
rtk git add docs/superpowers/plans/2026-07-26-start-screen-empty-state.md crates/waml-editor/src/start_screen.rs crates/waml-editor/src/recent_row.rs crates/waml-editor/src/app.rs
rtk git commit -m "feat: redesign empty start screen"
```
