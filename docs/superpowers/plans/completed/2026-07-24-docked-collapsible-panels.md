# Docked, collapsible panels (Model + Inspector) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the floating translucent Model (tree) + Inspector panels with docked panels that collapse to a thin vertical edge flag, driven by one `DockState` enum (Flag / Peek / Pinned).

**Architecture:** Approach A — the body inner goes from `flow: Overlay` to a `flow: Right` split (`dock_row`: `left_slot` / `center_stack` / `right_slot`) with a `peek_layer` Overlay sibling. The `center_stack` is `Fill`, so widening a slot on **Pin** shrinks the center automatically (no margin math). The real panel widgets render in `peek_layer` (an Overlay, zero layout cost) so a **Peek** body overlaps the center without consuming width and keeps real hit rects (no `draw_abs`). Each slot is a bg-less reservation spacer whose width the app drives from the panel's `DockState`; that reservation is the only thing that shrinks the center. `DockState`, its transition table, and the auto-collapse timer live in a pure, unit-tested `dock.rs` module.

**Tech Stack:** Rust, makepad (redoz fork), the project's `script_mod!` DSL, SDF glyph shaders (`icons.rs`), `PanelGlass` NextFrame dt easing.

## Global Constraints

- This is a git worktree at `C:\dev\waml\.claude\worktrees\edge-fillet-corner-sdf`. Confirm with `git rev-parse --show-toplevel` before editing. Read/Write/Edit take ABSOLUTE paths; a main-root path edits MAIN, not this worktree. Build and inspect ONLY this worktree's copy.
- Every task must end green under the FULL gate: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`. Do not commit a task until the gate passes.
- `DockState` replaces the panels' separate `collapsed` / `pinned` / `folded` bools. The transition table below is authoritative; encode it as a pure `dock::next` fn with a unit test.
- Do NOT use `draw_abs` for the peek body — the dedicated `peek_layer` Overlay only (aligned-parent hit-rect offset bug → dead clicks; memory `makepad-aligned-parent-hit-rect-offset`).
- Pin flips the WHOLE layout mode (overlay peek → docked column that shrinks center), not just opacity.
- Icon add: preserve the invariant `enum == field == DSL == get == ALL == label` order across EVERY parallel list in `icons.rs`, append the two new glyphs at the END (after `Group`), and bump every count. Keep unused catalog glyphs (memory `keep-unused-catalog-icons`).
- Manual visual verification is per-pid screenshot ONLY. Never kill-all / `Stop-Process` by name — it kills the user's own live editor (memory `screenshot-verify-hits-user-editor`).
- `script_mod` namespaces (`mod.X`) must be created by ONE object-literal assignment, never field-by-field (memory `script-mod-namespace-object-literal`). Register new widgets BEFORE their consumers in `App::script_mod` (memory `iconbutton-child-needs-script-mod-order`).

**Transition table (authoritative):**

| From    | Event (trigger)                                | To      |
|---------|------------------------------------------------|---------|
| Flag    | `FlagActivate` (hover flag, or click flag)     | Peek    |
| Peek    | `PointerLeft` (leaves flag AND body, ~600ms)   | Flag    |
| Peek    | `PinToggle` (click header pin)                 | Pinned  |
| Pinned  | `PinToggle` (click header pin = unpin)         | Flag    |
| Pinned  | `Collapse` (click header collapse)             | Flag    |
| any     | (unlisted event)                               | (unchanged) |

Pinned never auto-collapses: `PointerLeft` is a no-op in Pinned.

---

### Task 1: Port `list-tree` + `inspection-panel` glyphs into `icons.rs`

Adds the two flag glyphs to the SDF catalog, growing it from 92 → 94, preserving the `enum == field == DSL == get == ALL == label` order invariant (append after `Group`).

**Files:**
- Create: `resources/icons/list-tree.svg`, `resources/icons/inspection-panel.svg` (copies of the Lucide sources)
- Modify: `crates/waml-editor/src/icons.rs` (6 parallel lists + count + tests)

**Interfaces:**
- Produces: `Icon::ListTree`, `Icon::InspectionPanel` (used by the flag widget in Tasks 4–5); `IconSet` fields `list_tree`, `inspection_panel`.

- [ ] **Step 1: Copy the two Lucide SVGs into the repo**

```bash
cp c:/dev/vendor/lucide-icons/icons/list-tree.svg        crates/../resources/icons/list-tree.svg
cp c:/dev/vendor/lucide-icons/icons/inspection-panel.svg resources/icons/inspection-panel.svg
```

(The canonical location is `resources/icons/` at the repo root — same dir as `package.svg` referenced by the existing shaders' `Faithful port of resources/icons/…` comments.)

- [ ] **Step 2: Bump the count tests to fail first (TDD red)**

In `crates/waml-editor/src/icons.rs`, the `#[cfg(test)] mod tests` block, change the count expectations:

```rust
    #[test]
    fn icon_all_has_94_entries() {
        assert_eq!(Icon::ALL.len(), 94);
    }
```
Rename `icon_all_has_92_entries` → `icon_all_has_94_entries`, `92` → `94`. In `icon_all_is_in_field_order_at_the_edges` append:
```rust
        assert_eq!(Icon::ALL[92], Icon::ListTree);
        assert_eq!(Icon::ALL[93], Icon::InspectionPanel);
```
In `icon_labels_are_unique_and_nonempty` change `assert_eq!(seen.len(), 92);` → `94`. Add a new test:
```rust
    #[test]
    fn flag_glyphs_present_with_lucide_slugs() {
        assert_eq!(Icon::ListTree.label(), "list-tree");
        assert_eq!(Icon::InspectionPanel.label(), "inspection-panel");
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p waml-editor --lib icons::tests 2>&1 | tail -30`
Expected: FAIL — `no variant named ListTree`, and the count assertions mismatch (or a compile error because the variants don't exist yet).

- [ ] **Step 4: Add the two SDF shader bodies (DSL)**

In `icons.rs`, inside the `script_mod!` block, immediately BEFORE the `mod.widgets.IconSetBase = …` line (i.e. as the last two `mod.draw.Icon*` shaders, keeping the append-at-end order), paste:

```
    // List tree: three rows + an L-shaped tree connector.
    // Faithful port of resources/icons/list-tree.svg via scripts/gen-icon.py.
    mod.draw.IconListTree = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.3333, s * 0.2083)
            sdf.line_to(s * 0.8750, s * 0.2083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5417, s * 0.5000)
            sdf.line_to(s * 0.8750, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.5417, s * 0.7917)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.4167)
            sdf.arc_to(s * 0.2083, s * 0.4167, s * 0.0833, 3.1416, 1.5708)
            sdf.line_to(s * 0.3333, s * 0.5000)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.1250, s * 0.2083)
            sdf.line_to(s * 0.1250, s * 0.7083)
            sdf.arc_to(s * 0.2083, s * 0.7083, s * 0.0833, 3.1416, 1.5708)
            sdf.line_to(s * 0.3333, s * 0.7917)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }

    // Inspection panel: rounded rect with a dot in each corner.
    // Faithful port of resources/icons/inspection-panel.svg via scripts/gen-icon.py.
    mod.draw.IconInspectionPanel = mod.draw.DrawColor{
        pixel: fn() {
            let s = self.rect_size.x
            let w = s * 0.068
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.move_to(s * 0.2083, s * 0.1250)
            sdf.line_to(s * 0.7917, s * 0.1250)
            sdf.arc_to(s * 0.7917, s * 0.2083, s * 0.0833, -1.5708, 0.0000)
            sdf.line_to(s * 0.8750, s * 0.7917)
            sdf.arc_to(s * 0.7917, s * 0.7917, s * 0.0833, 0.0000, 1.5708)
            sdf.line_to(s * 0.2083, s * 0.8750)
            sdf.arc_to(s * 0.2083, s * 0.7917, s * 0.0833, 1.5708, 3.1416)
            sdf.line_to(s * 0.1250, s * 0.2083)
            sdf.arc_to(s * 0.2083, s * 0.2083, s * 0.0833, 3.1416, 4.7124)
            sdf.close_path()
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.2917)
            sdf.line_to(s * 0.2921, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.2917)
            sdf.line_to(s * 0.7088, s * 0.2917)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.2917, s * 0.7083)
            sdf.line_to(s * 0.2921, s * 0.7083)
            sdf.stroke(self.color, w)
            sdf.move_to(s * 0.7083, s * 0.7083)
            sdf.line_to(s * 0.7088, s * 0.7083)
            sdf.stroke(self.color, w)
            return sdf.result
        }
    }
```

- [ ] **Step 5: Register the shaders in the `IconSet` DSL object literal**

In the `mod.widgets.IconSet = set_type_default() do mod.widgets.IconSetBase{ … }` block, AFTER the last field `group: mod.draw.IconGroup{ color: atlas.accent }`, append:

```
        list_tree: mod.draw.IconListTree{ color: atlas.accent }
        inspection_panel: mod.draw.IconInspectionPanel{ color: atlas.accent }
```

- [ ] **Step 6: Add the two `IconSet` struct fields**

In `pub struct IconSet`, after `pub group: DrawColor,` append:

```rust
    #[live]
    pub list_tree: DrawColor,
    #[live]
    pub inspection_panel: DrawColor,
```

- [ ] **Step 7: Add the two `get()` match arms**

In `IconSet::get`, after `Icon::Group => &mut self.group,` append:

```rust
            Icon::ListTree => &mut self.list_tree,
            Icon::InspectionPanel => &mut self.inspection_panel,
```

- [ ] **Step 8: Add the two `Icon` enum variants**

In `pub enum Icon`, after `Group,` append:

```rust
    ListTree,
    InspectionPanel,
```

- [ ] **Step 9: Grow `ALL` (count + entries)**

Change `pub const ALL: [Icon; 92]` → `pub const ALL: [Icon; 94]`. After `Icon::Group,` (the last array entry) append:

```rust
        Icon::ListTree,
        Icon::InspectionPanel,
```

- [ ] **Step 10: Add the two `label()` match arms**

In `Icon::label`, after `Icon::Group => "group",` append:

```rust
            Icon::ListTree => "list-tree",
            Icon::InspectionPanel => "inspection-panel",
```

- [ ] **Step 11: Run the icon tests — expect PASS**

Run: `cargo test -p waml-editor --lib icons::tests 2>&1 | tail -30`
Expected: PASS (94 entries, unique labels, new `flag_glyphs_present_with_lucide_slugs` green).

- [ ] **Step 12: Sanity-check the glyphs in the harness (visual, optional but recommended)**

Run: `cargo run -p waml-editor --bin icon_harness` and confirm `list-tree` + `inspection-panel` render as crisp last two cells at 14/16/20px. Close it (per-pid; do not kill-all).

- [ ] **Step 13: Full gate + commit**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green.
```bash
git add resources/icons/list-tree.svg resources/icons/inspection-panel.svg crates/waml-editor/src/icons.rs
git commit -m "feat(icons): add list-tree + inspection-panel flag glyphs (catalog 92 -> 94)"
```

---

### Task 2: `dock.rs` — pure `DockState` model, transition fn, peek timer, slot geometry

A dependency-free module holding the enum, the transition table as a pure fn, the auto-collapse timer as a pure dt fn, and the slot/center width arithmetic — all unit tested without a live `Cx`. No UI wiring yet.

**Files:**
- Create: `crates/waml-editor/src/dock.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod dock;`)

**Interfaces:**
- Produces:
  - `pub enum DockState { Flag, Peek, Pinned }` (derives `Clone, Copy, Debug, PartialEq, Eq, Default`; `#[default] Flag`)
  - `pub enum DockEvent { FlagActivate, PointerLeft, PinToggle, Collapse }`
  - `pub fn next(state: DockState, ev: DockEvent) -> DockState`
  - `pub const FLAG_W: f64 = 28.0;`
  - `pub fn slot_width(state: DockState, body_w: f64) -> f64` — `Flag`/`Peek` → `FLAG_W`; `Pinned` → `FLAG_W + body_w`
  - `pub fn body_visible(state: DockState) -> bool` — `false` for `Flag`, else `true`
  - `pub const PEEK_COLLAPSE_SECS: f64 = 0.6;`
  - `pub struct PeekTimer { armed: bool, elapsed: f64 }` with `arm(&mut self)`, `cancel(&mut self)`, `pub fn advance(&mut self, dt: f64) -> bool` (returns `true` exactly once when the armed elapsed crosses `PEEK_COLLAPSE_SECS`; auto-cancels on fire), `pub fn is_armed(&self) -> bool`.

- [ ] **Step 1: Write the failing test file**

Create `crates/waml-editor/src/dock.rs`:

```rust
//! Pure state model for the docked/collapsible Model + Inspector panels. Holds
//! the `DockState` enum, its transition table (`next`), the peek auto-collapse
//! timer (`PeekTimer`, a pure dt function — same NextFrame dt pattern as
//! `panel_glass`, but testable without a live `Cx`), and the slot/center width
//! arithmetic that makes Pin shrink the center. No makepad types here.

/// Which visual state a dock panel is in. Replaces the panels' old separate
/// `collapsed` / `pinned` / `folded` bools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DockState {
    /// Resting: a thin sideways-label strip at the body edge, no body drawn.
    #[default]
    Flag,
    /// Unpinned + expanded: body floats over the (frozen) center via
    /// `peek_layer`; auto-collapses back to `Flag` after `PEEK_COLLAPSE_SECS`.
    Peek,
    /// Docked column: consumes layout width, the center shrinks, sticky.
    Pinned,
}

/// A user/pointer event that may transition a `DockState`. See the plan's
/// authoritative transition table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockEvent {
    /// Hover or click the flag strip.
    FlagActivate,
    /// Pointer left the flag AND the body for `PEEK_COLLAPSE_SECS` (peek only).
    PointerLeft,
    /// Header pin button: Peek -> Pinned, or Pinned -> Flag (unpin).
    PinToggle,
    /// Header collapse button: Pinned -> Flag.
    Collapse,
}

/// The transition table. Any unlisted (state, event) pair is a no-op (returns
/// the state unchanged) — notably `PointerLeft` in `Pinned` (docked columns
/// never auto-collapse).
pub fn next(state: DockState, ev: DockEvent) -> DockState {
    use DockEvent::*;
    use DockState::*;
    match (state, ev) {
        (Flag, FlagActivate) => Peek,
        (Peek, PointerLeft) => Flag,
        (Peek, PinToggle) => Pinned,
        (Pinned, PinToggle) => Flag,
        (Pinned, Collapse) => Flag,
        (s, _) => s,
    }
}

/// Flag-strip width (px). The slot always reserves at least this much, so the
/// flag never occludes the canvas corner.
pub const FLAG_W: f64 = 28.0;

/// The layout width a panel's slot reserves in the `flow: Right` dock row.
/// Only `Pinned` reserves the full column and thereby shrinks the center;
/// `Flag`/`Peek` reserve just the flag spine.
pub fn slot_width(state: DockState, body_w: f64) -> f64 {
    match state {
        DockState::Flag | DockState::Peek => FLAG_W,
        DockState::Pinned => FLAG_W + body_w,
    }
}

/// Whether the panel body (frame + contents) draws at all. `Flag` draws only
/// the strip.
pub fn body_visible(state: DockState) -> bool {
    !matches!(state, DockState::Flag)
}

/// Seconds an unpinned peek lingers after the pointer leaves before collapsing.
pub const PEEK_COLLAPSE_SECS: f64 = 0.6;

/// Auto-collapse timer for `Peek`. Pure dt accumulator — the caller arms it
/// when the pointer leaves the flag+body, cancels it when the pointer returns
/// (or the panel pins), and calls `advance(dt)` each armed frame. Testable
/// without a `Cx`.
#[derive(Default)]
pub struct PeekTimer {
    armed: bool,
    elapsed: f64,
}

impl PeekTimer {
    /// Start (or restart) the countdown from zero.
    pub fn arm(&mut self) {
        self.armed = true;
        self.elapsed = 0.0;
    }

    /// Stop the countdown (pointer returned, or panel left Peek).
    pub fn cancel(&mut self) {
        self.armed = false;
        self.elapsed = 0.0;
    }

    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Accumulate `dt` seconds. Returns `true` exactly once, on the frame the
    /// elapsed time first reaches `PEEK_COLLAPSE_SECS`; the timer then
    /// auto-cancels so it won't fire again until re-armed. A no-op (returns
    /// `false`) while unarmed.
    pub fn advance(&mut self, dt: f64) -> bool {
        if !self.armed {
            return false;
        }
        self.elapsed += dt.max(0.0);
        if self.elapsed >= PEEK_COLLAPSE_SECS {
            self.cancel();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_table_matches_spec() {
        use DockEvent::*;
        use DockState::*;
        // Flag: only FlagActivate advances (to Peek).
        assert_eq!(next(Flag, FlagActivate), Peek);
        assert_eq!(next(Flag, PointerLeft), Flag);
        assert_eq!(next(Flag, PinToggle), Flag);
        assert_eq!(next(Flag, Collapse), Flag);
        // Peek: PointerLeft -> Flag, PinToggle -> Pinned.
        assert_eq!(next(Peek, PointerLeft), Flag);
        assert_eq!(next(Peek, PinToggle), Pinned);
        assert_eq!(next(Peek, FlagActivate), Peek);
        assert_eq!(next(Peek, Collapse), Peek);
        // Pinned: PinToggle (unpin) and Collapse both -> Flag; never auto-collapses.
        assert_eq!(next(Pinned, PinToggle), Flag);
        assert_eq!(next(Pinned, Collapse), Flag);
        assert_eq!(next(Pinned, PointerLeft), Pinned);
        assert_eq!(next(Pinned, FlagActivate), Pinned);
    }

    #[test]
    fn full_cycle_flag_peek_pinned_flag() {
        let mut s = DockState::default();
        assert_eq!(s, DockState::Flag);
        s = next(s, DockEvent::FlagActivate);
        assert_eq!(s, DockState::Peek);
        s = next(s, DockEvent::PinToggle);
        assert_eq!(s, DockState::Pinned);
        s = next(s, DockEvent::Collapse);
        assert_eq!(s, DockState::Flag);
    }

    #[test]
    fn slot_width_only_pinned_reserves_body() {
        assert_eq!(slot_width(DockState::Flag, 280.0), FLAG_W);
        assert_eq!(slot_width(DockState::Peek, 280.0), FLAG_W);
        assert_eq!(slot_width(DockState::Pinned, 280.0), FLAG_W + 280.0);
    }

    #[test]
    fn body_visible_only_when_expanded() {
        assert!(!body_visible(DockState::Flag));
        assert!(body_visible(DockState::Peek));
        assert!(body_visible(DockState::Pinned));
    }

    #[test]
    fn pinning_shrinks_center_by_exactly_slot_delta() {
        // The center is Fill = total - left_slot - right_slot. Pinning the left
        // Model panel must shrink the center by exactly its body width.
        let total = 1280.0;
        let right = slot_width(DockState::Flag, 320.0); // inspector at rest
        let center_flag = total - slot_width(DockState::Flag, 280.0) - right;
        let center_pinned = total - slot_width(DockState::Pinned, 280.0) - right;
        assert_eq!(center_flag - center_pinned, 280.0);
    }

    #[test]
    fn peek_timer_fires_once_after_threshold() {
        let mut t = PeekTimer::default();
        assert!(!t.advance(1.0)); // unarmed: no-op
        t.arm();
        assert!(!t.advance(0.3)); // 0.3 < 0.6
        assert!(!t.advance(0.2)); // 0.5 < 0.6
        assert!(t.advance(0.2)); // 0.7 >= 0.6 -> fire
        assert!(!t.is_armed()); // auto-cancelled
        assert!(!t.advance(1.0)); // stays fired-off until re-armed
    }

    #[test]
    fn peek_timer_cancel_prevents_fire() {
        let mut t = PeekTimer::default();
        t.arm();
        assert!(!t.advance(0.5));
        t.cancel();
        assert!(!t.is_armed());
        assert!(!t.advance(1.0));
    }

    #[test]
    fn peek_timer_rearm_restarts_countdown() {
        let mut t = PeekTimer::default();
        t.arm();
        assert!(!t.advance(0.5));
        t.arm(); // pointer left again -> restart
        assert!(!t.advance(0.3)); // only 0.3 since re-arm
        assert!(t.advance(0.4)); // 0.7 -> fire
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/waml-editor/src/main.rs`, add `mod dock;` in alphabetical position (between `mod diagram_switcher;` and `mod doc_tabs;`).

- [ ] **Step 3: Run the dock tests — expect PASS**

Run: `cargo test -p waml-editor --lib dock::tests 2>&1 | tail -30`
Expected: PASS (8 tests).

- [ ] **Step 4: Full gate + commit**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
```bash
git add crates/waml-editor/src/dock.rs crates/waml-editor/src/main.rs
git commit -m "feat(dock): pure DockState model + transition table + peek timer"
```

---

### Task 3: Restructure the body DSL to the `flow: Right` dock split

Replaces the body's inner `flow: Overlay` View (`app.rs` ~195–286) with `dock_body` (Overlay) holding `dock_row` (Right: `left_slot` / `center_stack` / `right_slot`) plus a `peek_layer` (Overlay) sibling. The panels move into `peek_layer` wrappers; the aux floaters move into `center_stack`; the hard-coded `margin left:304` / `right:344` are retired to `12`. Panels keep their OLD internals this task (interim: they render as always-visible columns in the overlay — visually close to today's floats). Wire the DockState behavior in Tasks 4–5.

**Files:**
- Modify: `crates/waml-editor/src/app.rs` (the `body +: { … }` DSL block, ~167–303)

**Interfaces:**
- Produces DSL ids consumed by later tasks: `dock_body`, `dock_row`, `left_slot`, `center_stack`, `right_slot`, `peek_layer`, `left_peek_wrap`, `right_peek_wrap`. `project_tree` / `inspector` / `canvas` / `source_view` / `tool_dock` / `constraint_toggle` / `conflict_badge` / `selection_toolbar` ids are preserved (only their parentage moves).

- [ ] **Step 1: Replace the inner body Overlay View with the dock split**

In `crates/waml-editor/src/app.rs`, replace the entire inner `View{ … flow: Overlay … canvas … inspector … selection_toolbar … }` block (currently lines ~195–286, the child of `main_column` that holds `canvas` through `selection_toolbar`) with:

```
                    // Body: a docked split. `dock_row` is flow:Right so a
                    // pinned slot shrinks the Fill `center_stack` automatically
                    // (no margin math). `peek_layer` is an Overlay sibling above
                    // it, holding the real panel widgets: a Peek body overlaps
                    // the center at zero layout cost and keeps real hit rects
                    // (no draw_abs — see the aligned-parent hit-rect bug). The
                    // slots are bg-less reservation spacers whose width the app
                    // drives from each panel's DockState (see `sync_dock_slots`).
                    dock_body := View{
                        width: Fill
                        height: Fill
                        flow: Overlay
                        dock_row := View{
                            width: Fill
                            height: Fill
                            flow: Right
                            // Left (Model) reservation spacer. No bg, no content:
                            // its only job is to reserve width so the center
                            // shrinks when the Model panel pins. Width set at
                            // runtime by `sync_dock_slots` (28 at rest, 28+280
                            // pinned). Starts at 28 (Flag).
                            left_slot := View{ width: 28.0, height: Fill }
                            // Center: canvas base + aux HUD floaters. Fill, so it
                            // takes whatever the slots leave. Overlay so each
                            // floater wrapper gets the full center rect and parks
                            // itself by `align`; wrappers carry no bg and grab no
                            // pointer events over empty area, so the canvas keeps
                            // pan/zoom in the gaps.
                            center_stack := View{
                                width: Fill
                                height: Fill
                                flow: Overlay
                                canvas := GraphCanvas{
                                    width: Fill
                                    height: Fill
                                }
                                source_view := SolidView{
                                    width: Fill
                                    height: Fill
                                    visible: false
                                    draw_bg.color: atlas.canvas_ground
                                }
                                // Tool dock: left edge of the CENTER, vertically
                                // centered. Anchors to the real center rect now,
                                // so it auto-tracks dock state (retired margin:304).
                                tool_dock_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.0, y: 0.5}
                                    tool_dock := ToolDock{
                                        width: 48.0
                                        height: 308.0
                                        margin: Inset{left: 12.0}
                                    }
                                }
                                // Constraint-veil toggle: top-left of center.
                                constraint_toggle_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.0, y: 0.0}
                                    constraint_toggle := ConstraintToggle{
                                        width: 110.0
                                        height: 36.0
                                        margin: Inset{left: 12.0, top: 12.0}
                                    }
                                }
                                // Conflict counter: top-right of center.
                                conflict_badge_wrap := View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 1.0, y: 0.0}
                                    conflict_badge := ConflictBadge{
                                        margin: Inset{right: 12.0, top: 14.0}
                                        visible: false
                                    }
                                }
                                // Selection toolbar: bottom, centered.
                                View{
                                    width: Fill
                                    height: Fill
                                    align: Align{x: 0.5, y: 1.0}
                                    selection_toolbar := SelectionToolbar{
                                        width: Fit
                                        height: 44.0
                                        margin: Inset{bottom: 12.0}
                                    }
                                }
                            }
                            // Right (Inspector) reservation spacer. Starts 28 (Flag).
                            right_slot := View{ width: 28.0, height: Fill }
                        }
                        // Peek/pinned bodies. Overlay above `dock_row`, so a peek
                        // overhangs the center without shrinking it. The wraps are
                        // edge-aligned and bg-less; each panel draws its flag spine
                        // flush at the window edge and its body just inside it.
                        peek_layer := View{
                            width: Fill
                            height: Fill
                            flow: Overlay
                            left_peek_wrap := View{
                                width: Fill
                                height: Fill
                                align: Align{x: 0.0, y: 0.0}
                                project_tree := ProjectTree{
                                    width: 280.0
                                    height: Fill
                                    margin: Inset{top: 12.0, bottom: 12.0}
                                }
                            }
                            right_peek_wrap := View{
                                width: Fill
                                height: Fill
                                align: Align{x: 1.0, y: 0.0}
                                inspector := Inspector{
                                    width: 320.0
                                    height: Fill
                                    margin: Inset{top: 12.0, bottom: 12.0}
                                }
                            }
                        }
                    }
```

Note: `project_tree`/`inspector` no longer carry `margin left:12`/`right:12` — the flag spine sits at the window edge; Task 4/5 give them a left/right margin of `dock::FLAG_W` so the body clears the spine. Leave the top/bottom `12` margins.

- [ ] **Step 2: Build to confirm the DSL parses and ids resolve**

Run: `cargo build -p waml-editor 2>&1 | tail -30`
Expected: builds. (All `ids!(canvas)`, `ids!(project_tree)`, `ids!(inspector)`, `ids!(tool_dock)`, `ids!(conflict_badge)`, etc. still resolve — makepad finds a widget by id regardless of nesting depth.)

- [ ] **Step 3: Run the workspace tests**

Run: `cargo test -p waml-editor 2>&1 | tail -20`
Expected: PASS — no behavior changed yet; the restructure is structural.

- [ ] **Step 4: Manual smoke (per-pid, do not kill-all)**

Run: `pwsh scripts/run-native.ps1 -Fixture <a diagram fixture>` (or launch empty). Confirm the editor opens, the canvas renders, and the tree + inspector still show (as overlay columns) and remain interactive. Screenshot by the launched process's pid; close that pid only.

- [ ] **Step 5: Full gate + commit**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
```bash
git add crates/waml-editor/src/app.rs
git commit -m "refactor(app): body Overlay -> flow:Right dock split (slots + center_stack + peek_layer)"
```

---

### Task 4: Model (tree) panel adopts `DockState` — flag render, peek timer, left-slot sync

`ProjectTree` replaces its `collapsed` bool with a `dock: DockState`, runs the peek timer, renders the flag strip in `Flag`/`Peek` vs the full column body in `Peek`/`Pinned`, repoints the header pin/collapse buttons to `PinToggle`/`Collapse`, treats the flag strip as one hit target (`FlagActivate` on hover/click), and drives `PanelGlass` opacity for Peek. The app reads the tree's `slot_width()` and applies it to `left_slot`, and gives `project_tree` a left margin of `FLAG_W`. The inspector is untouched this task (still old-style; it keeps floating as a column — fixed in Task 5).

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs`
- Modify: `crates/waml-editor/src/app.rs` (add `sync_dock_slots` + call it; set `project_tree` left margin)

**Interfaces:**
- Consumes: `dock::{DockState, DockEvent, PeekTimer, next, slot_width, body_visible, FLAG_W}`; `Icon::ListTree` (Task 1); `PanelGlass` (still has `pinned`/`toggle_pin` — used here as `force_opaque` via the pub field, removed in Task 6).
- Produces: `ProjectTree::dock_state() -> DockState`, `ProjectTree::slot_width() -> f64` (read by the app).

- [ ] **Step 1: Swap the `collapsed` field for `dock` + `peek_timer` and add a flag rect**

In `tree_panel.rs`, in `pub struct ProjectTree`, replace:
```rust
    /// Panel-local body fold: hides the `FileTree` body, header stays.
    #[rust]
    collapsed: bool,
```
with:
```rust
    /// The dock visual state (Flag / Peek / Pinned), replacing the old
    /// `collapsed` bool and `panel.pinned`. Owned here; the app reads
    /// `slot_width()` to drive the left reservation slot.
    #[rust]
    dock: crate::dock::DockState,
    /// Auto-collapse countdown for Peek (armed when the pointer leaves the flag
    /// AND body; cancelled when it returns or the panel pins).
    #[rust]
    peek_timer: crate::dock::PeekTimer,
    /// The flag strip's on-screen rect, captured in `draw_walk`, for hover/click
    /// hit-testing (geometric containment, like `PanelGlass`).
    #[rust]
    flag_rect: Rect,
```
Add the imports at the top: `use crate::dock::{DockEvent, DockState};` (near the other `use crate::…` lines).

- [ ] **Step 2: Add the flag glyph + rotated-label constants and a `draw_flag` helper**

Near the other geometry consts in `tree_panel.rs`, add:
```rust
// Flag strip (rest state): a `dock::FLAG_W`-wide spine at the body edge with a
// glyph near the top and a sideways "Model" label below it in accent ink.
const FLAG_ICON_SIZE: f64 = 16.0;
const FLAG_ICON_TOP: f64 = 12.0;
```
Add a free function (below `draw_row_highlight`):
```rust
/// Draw the flag strip for the Model panel into `rect` (the `FLAG_W`-wide spine
/// at the panel's left edge). Icon near the top; the label runs downward. Text
/// rotation is unreliable in the fork's `DrawText`, so the label is a per-glyph
/// vertical stack (one char per line, ~13px advance) — swap for a real 90deg
/// rotated `DrawText` here if/when the fork supports it (impl decision).
fn draw_model_flag(
    cx: &mut Cx2d,
    icons: &mut IconSet,
    draw_dim: &mut DrawText,
    accent: Vec4,
    rect: Rect,
) {
    let cx_center = rect.pos.x + rect.size.x * 0.5;
    let icon_rect = Rect {
        pos: dvec2((cx_center - FLAG_ICON_SIZE * 0.5).round(), (rect.pos.y + FLAG_ICON_TOP).round()),
        size: dvec2(FLAG_ICON_SIZE, FLAG_ICON_SIZE),
    };
    icons.draw(cx, Icon::ListTree, icon_rect, accent);
    draw_dim.color = accent;
    let mut y = rect.pos.y + FLAG_ICON_TOP + FLAG_ICON_SIZE + 8.0;
    for ch in "Model".chars() {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        let w = draw_dim
            .layout(cx, 0.0, 0.0, None, false, Align::default(), s)
            .size_in_lpxs
            .width as f64;
        draw_dim.draw_abs(cx, dvec2((cx_center - w * 0.5).round(), y.round()), s);
        y += 13.0;
    }
}
```

- [ ] **Step 3: Rewrite `draw_walk` to branch on `dock`**

In `ProjectTree::draw_walk`, at the very top, add the flag-vs-body branch. Replace the opening lines (which set `ft_widget` visibility off `self.collapsed`) so that:

- When `dock == Flag` (`!dock::body_visible`): set the frame walk width to `dock::FLAG_W`, hide the header + file_tree + note_band children, draw only the frame bg + `draw_model_flag`, capture `flag_rect`, and return early. Concretely, at the top of `draw_walk`:
```rust
        // Flag rest state: draw only the thin spine (glyph + sideways label),
        // no header/body. Capture the flag rect for hover/click hit-testing.
        if !crate::dock::body_visible(self.dock) {
            let mut fw = walk;
            fw.width = Size::Fixed(crate::dock::FLAG_W);
            self.view.file_tree(cx, ids!(file_tree)).set_visible(cx, false);
            self.view.view(cx, ids!(header)).set_visible(cx, false);
            self.view.view(cx, ids!(note_band)).set_visible(cx, false);
            self.panel.pinned = false;
            self.panel.draw(cx, &mut self.view.draw_bg);
            let _ = self.view.draw_walk(cx, scope, fw);
            let rect = self.view.area().rect(cx);
            self.flag_rect = rect;
            let accent = self.draw_title.color;
            draw_model_flag(cx, &mut self.icons, &mut self.draw_dim, accent, rect);
            return DrawStep::done();
        }
        // Expanded (Peek or Pinned): restore the header + body children.
        self.view.view(cx, ids!(header)).set_visible(cx, true);
```
Then in the remaining (expanded) body path, DELETE the old `let ft_widget = …; ft_widget.set_visible(cx, !self.collapsed);` line and replace `self.collapsed` usages:
  - `note_band_height(self.nav_tag, self.collapsed)` → `note_band_height(self.nav_tag, false)`.
  - The `if self.collapsed { walk.height = Size::Fit … }` block: DELETE (a Peek/Pinned panel always draws its full body; the frame no longer hugs the header).
  - The pin/collapse button `set_icon` block: repoint (Step 4).
  - `self.panel.pinned = matches!(self.dock, DockState::Pinned);` — set BEFORE `self.panel.draw(...)` so Pinned forces opaque and Peek eases with hover.
  - The `if self.collapsed { self.search_rect = …default } else { … }` and the trailing `if !self.collapsed { … note … }` blocks: drop the `collapsed` guard (always run the `else`/body branch, since an expanded panel always shows the search row + notes).

- [ ] **Step 4: Repoint the header pin/collapse glyphs to DockState**

Replace the `collapse_btn`/`pin_btn` `set_icon` block in `draw_walk` with:
```rust
        // Collapse button: always the "collapse" chevron (it only appears in an
        // expanded panel and always sends the panel to Flag).
        self.view
            .widget(cx, ids!(header.title_row.collapse_btn))
            .as_icon_button()
            .set_icon(cx, Icon::ListCollapse);
        // Pin button: lit + `Pin` glyph while Pinned, `PinOff` while Peek.
        let pinned = matches!(self.dock, DockState::Pinned);
        let pin_btn = self.view.widget(cx, ids!(header.title_row.pin_btn));
        pin_btn.as_icon_button().set_icon(cx, if pinned { Icon::Pin } else { Icon::PinOff });
        pin_btn.as_icon_button().set_active(cx, pinned);
```

- [ ] **Step 5: Rewrite `handle_event` — flag hover/click, peek timer, header transitions**

In `ProjectTree::handle_event`:
- Replace the two `Event::Actions` button handlers (which flipped `self.collapsed` / called `self.panel.toggle_pin`) with DockState transitions:
```rust
            if self
                .view
                .widget(cx, ids!(header.title_row.collapse_btn))
                .as_icon_button()
                .clicked(actions)
            {
                self.apply_dock(cx, DockEvent::Collapse);
            }
            if self
                .view
                .widget(cx, ids!(header.title_row.pin_btn))
                .as_icon_button()
                .clicked(actions)
            {
                self.apply_dock(cx, DockEvent::PinToggle);
            }
```
- Add flag hover/click + peek-leave tracking. After the existing `self.panel.handle_event(...)` easing call, add:
```rust
        // Flag interaction + peek auto-collapse. The panel's on-screen rect is
        // the flag spine (Flag) or the full column (Peek/Pinned); use geometric
        // containment (like PanelGlass), not Hit::FingerHover* (inner children
        // claim it first).
        if let Event::MouseMove(e) = event {
            match self.dock {
                DockState::Flag => {
                    if self.flag_rect.contains(e.abs) {
                        self.apply_dock(cx, DockEvent::FlagActivate);
                    }
                }
                DockState::Peek => {
                    let inside = self.view.area().rect(cx).contains(e.abs);
                    if inside {
                        self.peek_timer.cancel();
                    } else if !self.peek_timer.is_armed() {
                        self.peek_timer.arm();
                        self.arm_frame(cx);
                    }
                }
                DockState::Pinned => {}
            }
        }
        if let Some(ne) = self.dock_frame.is_event(event) {
            let dt = if self.dock_last_time == 0.0 { 0.0 } else { ne.time - self.dock_last_time };
            self.dock_last_time = ne.time;
            if self.peek_timer.advance(dt) {
                self.dock_last_time = 0.0;
                self.apply_dock(cx, DockEvent::PointerLeft);
            } else if self.peek_timer.is_armed() {
                self.dock_frame = cx.new_next_frame();
            } else {
                self.dock_last_time = 0.0;
            }
        }
        // Also accept a primary click on the flag as FlagActivate.
        if let Hit::FingerUp(fe) = event.hits(cx, self.view.area()) {
            if fe.is_primary_hit() && self.dock == DockState::Flag && self.flag_rect.contains(fe.abs) {
                self.apply_dock(cx, DockEvent::FlagActivate);
            }
        }
```
- Add the `dock_frame` / `dock_last_time` fields to the struct (a dedicated NextFrame/clock for the peek timer, separate from `PanelGlass`'s):
```rust
    #[rust]
    dock_frame: NextFrame,
    #[rust]
    dock_last_time: f64,
```

- [ ] **Step 6: Add `apply_dock`, `arm_frame`, `dock_state`, `slot_width` helpers**

In `impl ProjectTree`:
```rust
    /// Apply a dock event: transition, (re)arm/cancel the peek timer, and
    /// redraw. No-op if the state is unchanged.
    fn apply_dock(&mut self, cx: &mut Cx, ev: DockEvent) {
        let next = crate::dock::next(self.dock, ev);
        if next == self.dock {
            return;
        }
        self.dock = next;
        // Entering Peek arms nothing yet (pointer is over the panel); leaving
        // Peek cancels any pending countdown.
        if self.dock != DockState::Peek {
            self.peek_timer.cancel();
        }
        self.view.redraw(cx);
    }

    fn arm_frame(&mut self, cx: &mut Cx) {
        self.dock_last_time = 0.0;
        self.dock_frame = cx.new_next_frame();
    }

    /// The current dock state (read by the app to size the slot).
    pub fn dock_state(&self) -> DockState {
        self.dock
    }

    /// The layout width the app must reserve in the left slot for this panel.
    pub fn slot_width(&self) -> f64 {
        crate::dock::slot_width(self.dock, 280.0)
    }
```

- [ ] **Step 7: App — reserve the flag spine on the tree body + sync the left slot**

In `app.rs`, give `project_tree` a left margin of the flag width so its body clears the spine. In the DSL from Task 3, change:
```
                                project_tree := ProjectTree{
                                    width: 280.0
                                    height: Fill
                                    margin: Inset{top: 12.0, bottom: 12.0}
                                }
```
to add `left: 28.0`:
```
                                    margin: Inset{left: 28.0, top: 12.0, bottom: 12.0}
```
Add a `sync_dock_slots` method on `App`:
```rust
    /// Push each panel's DockState-driven slot width onto its reservation slot,
    /// so a pinned panel shrinks the Fill center. Cheap: `apply_over` only on a
    /// change (tracked in `dock_slot_w`). Called each `handle_event`.
    fn sync_dock_slots(&mut self, cx: &mut Cx) {
        let lw = self
            .ui
            .widget(cx, ids!(project_tree))
            .borrow::<crate::tree_panel::ProjectTree>()
            .map(|p| p.slot_width())
            .unwrap_or(crate::dock::FLAG_W);
        if (lw - self.dock_slot_w.0).abs() > 0.5 {
            self.dock_slot_w.0 = lw;
            self.ui.widget(cx, ids!(left_slot)).apply_over(cx, live!{ width: (lw) });
        }
        // right_slot is synced in Task 5 once the inspector owns a DockState.
    }
```
Add the tracking field to `struct App`:
```rust
    /// Last-applied (left, right) dock slot widths, so `sync_dock_slots` only
    /// `apply_over`s on a real change.
    #[rust]
    dock_slot_w: (f64, f64),
```
Call `self.sync_dock_slots(cx);` at the end of `App::handle_event` (after the `self.ui.handle_event(...)` / match-event dispatch, so it runs every frame including NextFrame). If `App` uses `MatchEvent`, add the call in `handle_event` just before returning.

- [ ] **Step 8: Build + smoke the Model panel**

Run: `cargo build -p waml-editor 2>&1 | tail -30` → builds.
Run the editor (per-pid). Verify: at rest the left edge shows a 28px "Model" flag (list-tree glyph + sideways label); hovering it peeks the tree over the canvas; moving the pointer away collapses it after ~0.6s; clicking the header pin docks it (canvas visibly shrinks from the left); pin again (or collapse) returns to the flag. The inspector still floats as before (Task 5).

- [ ] **Step 9: Full gate + commit**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
```bash
git add crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app.rs
git commit -m "feat(dock): Model tree panel flag/peek/pinned via DockState + left-slot sync"
```

---

### Task 5: Inspector panel adopts `DockState` — flag render, peek timer, right-slot sync

Mirror of Task 4 for the `Inspector`: replace `folded` + `panel.pinned`/`toggle_pin` with a `dock: DockState` + `peek_timer`, render the `inspection-panel` flag with a sideways "Inspector" label, repoint the header fold/pin buttons to `Collapse`/`PinToggle`, run the peek timer, and drive the right slot. After this task NEITHER panel calls `PanelGlass::toggle_pin` (both set `panel.pinned` directly from `dock == Pinned`), which unblocks the panel_glass cleanup in Task 6.

**Files:**
- Modify: `crates/waml-editor/src/inspector_panel.rs`
- Modify: `crates/waml-editor/src/app.rs` (`sync_dock_slots` right slot; `inspector` right margin)

**Interfaces:**
- Consumes: `dock::*`, `Icon::InspectionPanel`.
- Produces: `Inspector::dock_state() -> DockState`, `Inspector::slot_width() -> f64`.

- [ ] **Step 1: Swap `folded` for `dock` + timer + flag rect fields**

In `inspector_panel.rs`, in `pub struct Inspector`, replace:
```rust
    /// Manual body fold. `true` hides the body even when a subject is selected;
    /// `Subject::None` collapses regardless. Toggled by the fold-caret button.
    #[rust]
    folded: bool,
```
with:
```rust
    /// Dock visual state (Flag / Peek / Pinned), replacing `folded` and
    /// `panel.pinned`. The app reads `slot_width()` for the right slot.
    #[rust]
    dock: crate::dock::DockState,
    #[rust]
    peek_timer: crate::dock::PeekTimer,
    #[rust]
    flag_rect: Rect,
    #[rust]
    dock_frame: NextFrame,
    #[rust]
    dock_last_time: f64,
```
Add `use crate::dock::{DockEvent, DockState};` near the top imports.

- [ ] **Step 2: Add the Inspector flag renderer**

Add a free fn in `inspector_panel.rs` (mirrors `draw_model_flag`, label "Inspector", glyph `Icon::InspectionPanel`):
```rust
const FLAG_ICON_SIZE: f64 = 16.0;
const FLAG_ICON_TOP: f64 = 12.0;

/// Draw the Inspector flag spine into `rect`. Per-glyph vertical label (text
/// rotation is unreliable in the fork's DrawText — swap for a rotated DrawText
/// if the fork gains it).
fn draw_inspector_flag(
    cx: &mut Cx2d,
    icons: &mut IconSet,
    draw_dim: &mut DrawText,
    accent: Vec4,
    rect: Rect,
) {
    let cxr = rect.pos.x + rect.size.x * 0.5;
    icons.draw(
        cx,
        Icon::InspectionPanel,
        Rect {
            pos: dvec2((cxr - FLAG_ICON_SIZE * 0.5).round(), (rect.pos.y + FLAG_ICON_TOP).round()),
            size: dvec2(FLAG_ICON_SIZE, FLAG_ICON_SIZE),
        },
        accent,
    );
    draw_dim.color = accent;
    let mut y = rect.pos.y + FLAG_ICON_TOP + FLAG_ICON_SIZE + 8.0;
    for ch in "Inspector".chars() {
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        let w = draw_dim
            .layout(cx, 0.0, 0.0, None, false, Align::default(), s)
            .size_in_lpxs
            .width as f64;
        draw_dim.draw_abs(cx, dvec2((cxr - w * 0.5).round(), y.round()), s);
        y += 13.0;
    }
}
```
The inspector already has a `draw_dim: DrawText` field to reuse (accent read from `draw_title.color` = `atlas.text`; use `self.draw_glyph.color` for the accent tint, which is `atlas.accent`).

- [ ] **Step 3: Branch `draw_walk` on `dock`**

At the top of `Inspector::draw_walk`, before the existing `let collapsed = …` line, add the flag branch:
```rust
        if !crate::dock::body_visible(self.dock) {
            let mut fw = walk;
            fw.width = Size::Fixed(crate::dock::FLAG_W);
            self.view.view(cx, ids!(element_bar)).set_visible(cx, false);
            self.view.view(cx, ids!(body)).set_visible(cx, false);
            self.panel.pinned = false;
            self.panel.draw(cx, &mut self.view.draw_bg);
            let _ = self.view.draw_walk(cx, scope, fw);
            let rect = self.view.area().rect(cx);
            self.flag_rect = rect;
            let accent = self.draw_glyph.color;
            draw_inspector_flag(cx, &mut self.icons, &mut self.draw_dim, accent, rect);
            return DrawStep::done();
        }
        self.view.view(cx, ids!(element_bar)).set_visible(cx, true);
```
Then replace the existing `let collapsed = self.proj.is_none() || self.folded;` with `let collapsed = self.proj.is_none();` (Flag is handled above; a Peek/Pinned panel with no subject still just shows the empty picker bar). Set `self.panel.pinned = matches!(self.dock, DockState::Pinned);` right before the existing `self.panel.draw(cx, &mut self.view.draw_bg);` call in the body path.

- [ ] **Step 4: Repoint the fold/pin buttons + `sync_bar_buttons`**

In `sync_bar_buttons`, drop the `folded` reference: the fold button always shows `Icon::ListCollapse` (it only appears in an expanded panel and always sends to Flag); the pin reflects `matches!(self.dock, DockState::Pinned)`:
```rust
    fn sync_bar_buttons(&mut self, cx: &mut Cx) {
        let pinned = matches!(self.dock, DockState::Pinned);
        let vis = self.show_picker;
        let fold = self.view.widget(cx, ids!(element_bar.fold_btn));
        fold.set_visible(cx, vis);
        fold.as_icon_button().set_icon(cx, Icon::ListCollapse);
        let pin = self.view.widget(cx, ids!(element_bar.pin_btn));
        pin.set_visible(cx, vis);
        pin.as_icon_button().set_icon(cx, if pinned { Icon::Pin } else { Icon::PinOff });
        pin.as_icon_button().set_active(cx, pinned);
    }
```
In `handle_event`, replace the pin/fold `Event::Actions` handlers:
```rust
            if self.view.widget(cx, ids!(element_bar.pin_btn)).as_icon_button().clicked(actions) {
                self.apply_dock(cx, DockEvent::PinToggle);
                self.sync_bar_buttons(cx);
            }
            if self.view.widget(cx, ids!(element_bar.fold_btn)).as_icon_button().clicked(actions) {
                self.apply_dock(cx, DockEvent::Collapse);
                self.sync_bar_buttons(cx);
            }
```
And in `set_subject`, replace `self.folded = false;` with nothing (subject changes no longer force-unfold; dock state is independent of subject).

- [ ] **Step 5: Add the flag hover/click + peek timer to `handle_event`**

Same shape as Task 4 Step 5 (using `self.view.area().rect(cx)` for the Peek-inside test and `self.flag_rect` for Flag). Insert after the existing `self.panel.handle_event(...)` easing call:
```rust
        if let Event::MouseMove(e) = event {
            match self.dock {
                DockState::Flag => {
                    if self.flag_rect.contains(e.abs) {
                        self.apply_dock(cx, DockEvent::FlagActivate);
                    }
                }
                DockState::Peek => {
                    if self.view.area().rect(cx).contains(e.abs) {
                        self.peek_timer.cancel();
                    } else if !self.peek_timer.is_armed() {
                        self.peek_timer.arm();
                        self.arm_frame(cx);
                    }
                }
                DockState::Pinned => {}
            }
        }
        if let Some(ne) = self.dock_frame.is_event(event) {
            let dt = if self.dock_last_time == 0.0 { 0.0 } else { ne.time - self.dock_last_time };
            self.dock_last_time = ne.time;
            if self.peek_timer.advance(dt) {
                self.dock_last_time = 0.0;
                self.apply_dock(cx, DockEvent::PointerLeft);
            } else if self.peek_timer.is_armed() {
                self.dock_frame = cx.new_next_frame();
            } else {
                self.dock_last_time = 0.0;
            }
        }
        if let Hit::FingerUp(fe) = event.hits(cx, self.view.area()) {
            if fe.is_primary_hit() && self.dock == DockState::Flag && self.flag_rect.contains(fe.abs) {
                self.apply_dock(cx, DockEvent::FlagActivate);
            }
        }
```
Note: the inspector's existing body hit-testing uses `hits_with_capture_overload(...)`; keep that block as-is — the added `event.hits(cx, self.view.area())` FingerUp check for the flag is only consulted while `dock == Flag` (body hidden), so there is no double-handling.

- [ ] **Step 6: Add `apply_dock` / `arm_frame` / `dock_state` / `slot_width`**

In `impl Inspector` (identical shape to Task 4 Step 6, with `sync_bar_buttons` after a state change and body width 320):
```rust
    fn apply_dock(&mut self, cx: &mut Cx, ev: DockEvent) {
        let next = crate::dock::next(self.dock, ev);
        if next == self.dock {
            return;
        }
        self.dock = next;
        if self.dock != DockState::Peek {
            self.peek_timer.cancel();
        }
        self.sync_bar_buttons(cx);
        self.view.redraw(cx);
    }
    fn arm_frame(&mut self, cx: &mut Cx) {
        self.dock_last_time = 0.0;
        self.dock_frame = cx.new_next_frame();
    }
    pub fn dock_state(&self) -> crate::dock::DockState {
        self.dock
    }
    pub fn slot_width(&self) -> f64 {
        crate::dock::slot_width(self.dock, 320.0)
    }
```

- [ ] **Step 7: App — reserve the flag spine on the inspector body + sync the right slot**

In `app.rs`, add a right margin of the flag width to `inspector` (Task 3 DSL):
```
                                    margin: Inset{right: 28.0, top: 12.0, bottom: 12.0}
```
Extend `sync_dock_slots` to drive `right_slot`:
```rust
        let rw = self
            .ui
            .widget(cx, ids!(inspector))
            .borrow::<crate::inspector_panel::Inspector>()
            .map(|p| p.slot_width())
            .unwrap_or(crate::dock::FLAG_W);
        if (rw - self.dock_slot_w.1).abs() > 0.5 {
            self.dock_slot_w.1 = rw;
            self.ui.widget(cx, ids!(right_slot)).apply_over(cx, live!{ width: (rw) });
        }
```

- [ ] **Step 8: Build + smoke both panels**

Run: `cargo build -p waml-editor 2>&1 | tail -30` → builds.
Run editor (per-pid). Verify the Inspector flag on the right edge (inspection-panel glyph + sideways "Inspector"), peek + auto-collapse, pin shrinks the canvas from the right, and BOTH panels pinned at once (canvas squeezed on both sides). Close by pid.

- [ ] **Step 9: Full gate + commit**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
```bash
git add crates/waml-editor/src/inspector_panel.rs crates/waml-editor/src/app.rs
git commit -m "feat(dock): Inspector panel flag/peek/pinned via DockState + right-slot sync"
```

---

### Task 6: `panel_glass` cleanup + retire the now-false module docs + final visual verify

`PanelGlass` sheds the pin concept: rename `pinned` → `force_opaque` (set each draw from `dock == Pinned`) and delete `toggle_pin` (now unused). Refresh the three module docs that still claim the panels "float over the graph canvas (app `flow: Overlay`)". Close out with the mandatory manual visual pass.

**Files:**
- Modify: `crates/waml-editor/src/panel_glass.rs`
- Modify: `crates/waml-editor/src/tree_panel.rs`, `crates/waml-editor/src/inspector_panel.rs` (rename the one `panel.pinned` write; refresh module docs)

**Interfaces:**
- Consumes: nothing new.
- Produces: `PanelGlass { pub hovered, pub force_opaque, … }` (no `pinned`, no `toggle_pin`).

- [ ] **Step 1: Rename the field + retarget `target()`, delete `toggle_pin`**

In `panel_glass.rs`:
- Rename the field:
```rust
    /// Force the interior fill fully opaque regardless of hover (the panel sets
    /// this from `dock == Pinned`). A Peek body eases with hover only.
    pub force_opaque: bool,
```
- In `target()`, `if self.hovered || self.pinned` → `if self.hovered || self.force_opaque`.
- DELETE the `pub fn toggle_pin(&mut self, cx: &mut Cx) { … }` method entirely.

- [ ] **Step 2: Update the two write sites**

In `tree_panel.rs` and `inspector_panel.rs`, change every `self.panel.pinned = …` write (added in Tasks 4–5) and the Flag-branch `self.panel.pinned = false;` to `self.panel.force_opaque = …` / `self.panel.force_opaque = false;`. Grep to confirm none remain: `rg 'panel\.pinned|toggle_pin' crates/waml-editor/src`.

- [ ] **Step 3: Refresh the now-false module docs**

- `panel_glass.rs` header: the panels no longer "float over the graph canvas (app `flow: Overlay`)". Reword to: the shared translucency machine now eases only the **Peek** body's interior fill (Pinned forces opaque via `force_opaque`; Flag draws no body), the panels living in `peek_layer` over the docked `flow: Right` split.
- `tree_panel.rs` header + the `panel` field doc: replace "float over the graph canvas" language with the docked/flag model (Flag spine at the edge, Peek overlay, Pinned docked column that shrinks the center).
- `inspector_panel.rs` header: same reword; drop the "float"/"Overlay body" phrasing.

- [ ] **Step 4: Build + tests**

Run: `cargo build -p waml-editor 2>&1 | tail -20 && cargo test -p waml-editor 2>&1 | tail -20`
Expected: builds clean (no `dead_code` warning for a leftover `toggle_pin` — the gate promotes `dead_code` to a hard error; memory `gate-clippy-deny-warnings-blocks-dead-code`), tests pass.

- [ ] **Step 5: Mandatory manual visual pass (per-pid; never kill-all)**

Run the native editor on a diagram fixture (`pwsh scripts/run-native.ps1 -Fixture <fixture>`). Capture screenshots BY the launched process's pid. Confirm the spec's checklist:
  1. **Flag legible** — both edges show a crisp 28px flag (glyph + sideways "Model"/"Inspector" in accent ink); no canvas-corner occlusion.
  2. **Peek** — hovering/clicking a flag slides its body out over the (unshifted) canvas; leaving both flag and body auto-collapses after ~0.6s; the flag spine stays visible as the peek's spine.
  3. **Pin reflow** — clicking header pin docks the column; the canvas/active tab reflows into the reduced width (center shrinks by exactly the body width).
  4. **Both panels pinned at once** — canvas squeezed on both sides, no overlap, aux floaters (tool dock, constraint toggle, conflict badge, selection toolbar) still anchored to the (now narrower) center rect.
Close ONLY the launched pid (`Stop-Process -Id <pid>`), never by name.

- [ ] **Step 6: Full gate + commit**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
```bash
git add crates/waml-editor/src/panel_glass.rs crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/inspector_panel.rs
git commit -m "refactor(panel_glass): drop pin -> force_opaque (Peek-only ease); retire float-era docs"
```

---

## Notes for the implementer

- **`apply_over` width**: `self.ui.widget(cx, ids!(left_slot)).apply_over(cx, live!{ width: (lw) })` sets a View's walk width at runtime (`lw: f64`). If the fork's `live!` macro rejects a bare `f64` for `width`, wrap as `Size::Fixed(lw)` or use the `(#lw)` binding form the codebase already uses for runtime values — check a nearby `apply_over` call for the exact spelling.
- **Flag label rotation**: both flag renderers use a per-glyph vertical stack (one char per line) because the fork's `DrawText` rotation is unproven. If a rotated `DrawText` lands, replace the char loop with a single rotated `draw_abs` — the rect and accent tint are already computed. This is a deliberate impl decision, not a gap.
- **Peek NextFrame vs PanelGlass NextFrame**: the panels keep `PanelGlass`'s own `NextFrame` for opacity easing and add a SEPARATE `dock_frame`/`dock_last_time` for the peek countdown, so the two dt loops don't stomp each other's `last_time`.
- **Do not** widen the panel body in the slot for Peek — that would shrink the center (Pinned behavior). Peek keeps `slot_width == FLAG_W`; only the `peek_layer` body grows.
- **Flag hover wash**: the spec asks for a subtle accent wash on flag hover (the `icon_button` idiom). Because hover fires `FlagActivate` and immediately promotes Flag → Peek, a lingering hovered-flag state barely exists, so the flag renderers above omit a dedicated wash and let the HUD frame bg carry the "pressable" read. If, in the visual pass, the flag reads as inert, add the `icon_button` wash: paint a faint `atlas.accent @16%` rounded fill behind the glyph in `draw_model_flag`/`draw_inspector_flag` (same premultiplied-accent shader the `IconButton` `draw_bg` uses), gated on a short hover-hold before promoting to Peek. This is a visual-polish decision to make at the screenshot step, not a blocker.
- **Aux-anchor regression guard**: the spec calls for a guard on the retired `margin left:304 / right:344`. A pure Rust unit test can't read DSL margin literals, so the guard is: (a) the `pinning_shrinks_center_by_exactly_slot_delta` test (Task 2) proves the center math, and (b) Task 6's visual pass confirms the aux floaters ride the narrowed center when both panels pin. Before committing Task 3, `rg '304|344' crates/waml-editor/src/app.rs` must return nothing in the body DSL.
