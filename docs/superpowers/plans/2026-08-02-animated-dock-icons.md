# Animated Dock Icons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the five requested Lucide glyphs, show action-specific open/close glyphs on both dock toggles, and animate the existing left and right dock widths over 180 ms without changing final layout or responsive behavior.

**Architecture:** Keep `DockState` as the logical `Flag`/`Pinned` authority. Add a pure `DockMotion` presentation model in `dock.rs`; `App` owns one motion per panel, samples them in `sync_dock_slots`, requests `NextFrame` events while either motion is active, and converts normalized values into body and desktop-slot widths. The tree and inspector widgets get an independent presentation-visible flag so their contents remain drawable until a close animation reaches zero.

**Tech Stack:** Rust, Makepad widgets and `NextFrame`, the existing SDF `IconSet`, Lucide SVG source, Cargo unit tests, and the existing PowerShell window-capture script.

## Global Constraints

- The transition duration is exactly `0.180` seconds for both panels.
- Use cubic ease-out interpolation: `1.0 - (1.0 - t)^3` for clamped `t` in `0.0..=1.0`.
- Keep final panel widths unchanged: project tree `PROJECT_TREE_W`, inspector `INSPECTOR_W`.
- Keep logical dock state binary for these controls: `Flag` or `Pinned`.
- Keep narrow-mode center slots at zero width and cap animated panel bodies to the available viewport.
- Keep narrow-mode mutual exclusion and outside-click dismissal unchanged.
- Keep panel content visible until the closing presentation value reaches exactly `0.0`.
- Show the action a click will perform: an `Open` glyph for `Flag`, and a `Close` glyph for `Pinned`.
- Preserve the catalog order invariant: draw DSL field, `IconSet` field, `IconSet::get`, `Icon` variant, `Icon::ALL`, and `Icon::label` must use the same order.
- Do not modify or commit unrelated work. The feature executes in a clean isolated worktree created from the current `main` tip.
- Prefix shell commands with `rtk`, as required by `RTK.md`.
- Use ASD-STE100 Simplified Technical English in new comments and documentation.

---

## File Structure

- Modify `crates/waml-editor/src/icons.rs`: add the five Lucide SDF drawings and extend every ordered catalog mapping.
- Modify `crates/waml-editor/src/icons_overlay.rs`: list the new glyphs in the icon reference and keep drift guards accurate.
- Modify `crates/waml-editor/src/dock.rs`: own the pure normalized motion model, presentation visibility predicate, and progress-based responsive width arithmetic.
- Modify `crates/waml-editor/src/tree_panel.rs`: separate draw visibility from logical dock state.
- Modify `crates/waml-editor/src/inspector_panel.rs`: mirror the tree panel's presentation-visible contract.
- Modify `crates/waml-editor/src/document_header.rs`: let `App` replace the visible right-dock glyph without changing whether the control exists.
- Modify `crates/waml-editor/src/app.rs`: own both motion values and their frame request, select toggle glyphs, and apply animated geometry.
- Modify `crates/waml-editor/src/icons_overlay.rs` tests, `icons.rs` tests, `dock.rs` tests, `document_header.rs` tests, and `app.rs` tests in place. No new test crate is required.
- Do not modify `crates/waml-editor/src/bin/icon_harness.rs`: it already iterates `Icon::ALL`, so extending `ALL` is the harness integration.

---

### Task 1: Add the five Lucide glyphs to the shared catalog

**Files:**
- Modify: `crates/waml-editor/src/icons.rs:3634-3684, 3760-3803, 3806-4034, 4040-4157, 4170-4525, 4528-4592`
- Modify: `crates/waml-editor/src/icons_overlay.rs:20-118, 274-350`
- Verify without editing: `crates/waml-editor/src/bin/icon_harness.rs`

**Interfaces:**
- Consumes: existing `IconSet`, `Icon::ALL`, `Icon::label`, `ICON_GROUPS`, and `scripts/gen-icon.py`.
- Produces: `Icon::{FolderTree, PanelLeftOpen, PanelLeftClose, PanelRightOpen, PanelRightClose}` and matching `DrawColor` fields.

- [ ] **Step 1: Add failing catalog-order and label tests**

Replace the fixed-count test and extend the tail-order test in `icons.rs` with these exact assertions:

```rust
#[test]
fn icon_all_has_117_entries() {
    assert_eq!(Icon::ALL.len(), 117);
}

#[test]
fn dock_action_glyphs_follow_catalog_order_and_lucide_slugs() {
    assert_eq!(
        &Icon::ALL[112..],
        &[
            Icon::FolderTree,
            Icon::PanelLeftOpen,
            Icon::PanelLeftClose,
            Icon::PanelRightOpen,
            Icon::PanelRightClose,
        ]
    );
    assert_eq!(Icon::FolderTree.label(), "folder-tree");
    assert_eq!(Icon::PanelLeftOpen.label(), "panel-left-open");
    assert_eq!(Icon::PanelLeftClose.label(), "panel-left-close");
    assert_eq!(Icon::PanelRightOpen.label(), "panel-right-open");
    assert_eq!(Icon::PanelRightClose.label(), "panel-right-close");
}
```

Keep the existing assertions for indices `0..=111`; they protect the established order.

- [ ] **Step 2: Run the new tests and confirm that the variants are absent**

Run:

```powershell
rtk cargo test -p waml-editor icons::tests::dock_action_glyphs_follow_catalog_order_and_lucide_slugs --lib
```

Expected: compilation fails with missing `Icon` variants such as `FolderTree` and `PanelLeftOpen`.

- [ ] **Step 3: Generate the five SDF bodies from the official Lucide SVGs**

Use a temporary directory outside the repository. The generator prints Rust/Makepad DSL and does not edit source files.

```powershell
rtk pwsh -NoProfile -Command '$dockGlyphDir="C:\tmp\waml-lucide-dock-icons"; New-Item -ItemType Directory -Force $dockGlyphDir | Out-Null; "folder-tree","panel-left-open","panel-left-close","panel-right-open","panel-right-close" | ForEach-Object { Invoke-WebRequest -UseBasicParsing -Uri ("https://raw.githubusercontent.com/lucide-icons/lucide/main/icons/" + $_ + ".svg") -OutFile (Join-Path $dockGlyphDir ($_.ToString() + ".svg")) }'
rtk python scripts/gen-icon.py C:\tmp\waml-lucide-dock-icons\folder-tree.svg
rtk python scripts/gen-icon.py C:\tmp\waml-lucide-dock-icons\panel-left-open.svg
rtk python scripts/gen-icon.py C:\tmp\waml-lucide-dock-icons\panel-left-close.svg
rtk python scripts/gen-icon.py C:\tmp\waml-lucide-dock-icons\panel-right-open.svg
rtk python scripts/gen-icon.py C:\tmp\waml-lucide-dock-icons\panel-right-close.svg
```

Append five `mod.draw.DrawColor` definitions after the existing `IconPanelRight` drawing. Name them `IconFolderTree`, `IconPanelLeftOpen`, `IconPanelLeftClose`, `IconPanelRightOpen`, and `IconPanelRightClose`, in that order. Each definition uses `pixel: fn()`, starts with `let s = self.rect_size.x`, and then contains the full stdout from its matching generator command. The generator output is the exact code for every path command. Do not hand-adjust coordinates, and keep only one `let s = self.rect_size.x` in each function.

- [ ] **Step 4: Extend every ordered catalog layer**

Append the same five items after `panel_right` / `PanelRight` in all six ordered locations:

```rust
// IconSet DSL
folder_tree: mod.draw.IconFolderTree{ color: atlas.accent }
panel_left_open: mod.draw.IconPanelLeftOpen{ color: atlas.accent }
panel_left_close: mod.draw.IconPanelLeftClose{ color: atlas.accent }
panel_right_open: mod.draw.IconPanelRightOpen{ color: atlas.accent }
panel_right_close: mod.draw.IconPanelRightClose{ color: atlas.accent }

// IconSet fields
#[live]
pub folder_tree: DrawColor,
#[live]
pub panel_left_open: DrawColor,
#[live]
pub panel_left_close: DrawColor,
#[live]
pub panel_right_open: DrawColor,
#[live]
pub panel_right_close: DrawColor,

// IconSet::get
Icon::FolderTree => &mut self.folder_tree,
Icon::PanelLeftOpen => &mut self.panel_left_open,
Icon::PanelLeftClose => &mut self.panel_left_close,
Icon::PanelRightOpen => &mut self.panel_right_open,
Icon::PanelRightClose => &mut self.panel_right_close,

// Icon enum and Icon::ALL
FolderTree,
PanelLeftOpen,
PanelLeftClose,
PanelRightOpen,
PanelRightClose,

// Icon::label
Icon::FolderTree => "folder-tree",
Icon::PanelLeftOpen => "panel-left-open",
Icon::PanelLeftClose => "panel-left-close",
Icon::PanelRightOpen => "panel-right-open",
Icon::PanelRightClose => "panel-right-close",
```

Change `pub const ALL: [Icon; 112]` to `pub const ALL: [Icon; 117]`.

- [ ] **Step 5: Update the overlay group and its unwired allow-list**

In `TREE PANEL / DOCUMENT TABS`, replace the two static toggle descriptions and add `FolderTree`:

```rust
ie!(FolderTree, "Project tree hierarchy catalog glyph"),
ie!(PanelLeftOpen, "Open the project tree"),
ie!(PanelLeftClose, "Close the project tree"),
ie!(PanelRightOpen, "Open the inspector"),
ie!(PanelRightClose, "Close the inspector"),
```

Keep `PanelRight` listed because document chrome still uses it as the existing right-dock availability marker. Move `PanelLeft` to `CATALOG ONLY` with purpose `"Retired static left-dock toggle"`. Extend the allow-list because `FolderTree` is catalog-only in this change and `PanelLeft` loses its UI call site:

```rust
const UNWIRED_BUT_LISTED: &[Icon] = &[
    Icon::PinOff,
    Icon::InspectionPanel,
    Icon::ListTree,
    Icon::FolderTree,
    Icon::PanelLeft,
];
```

- [ ] **Step 6: Run catalog, overlay, and harness checks**

Run:

```powershell
rtk cargo test -p waml-editor icons::tests --lib
rtk cargo test -p waml-editor icons_overlay::drift --lib
rtk cargo build -p waml-editor --bin icon_harness
```

Expected: all tests pass and the harness binary builds with 117 catalog entries.

- [ ] **Step 7: Commit the catalog change**

Stage only the two feature files:

```powershell
rtk git add crates/waml-editor/src/icons.rs crates/waml-editor/src/icons_overlay.rs
rtk git commit -m "feat(editor): add dock action icons"
```

---

### Task 2: Add a pure reversible dock-motion model and animated layout arithmetic

**Files:**
- Modify: `crates/waml-editor/src/dock.rs:84-140, 271-385`

**Interfaces:**
- Consumes: `DockState` and fixed body widths supplied by the app.
- Produces: `DOCK_MOTION_SECS: f64`, `DockMotion::{new, request, sample, value, is_active}`, `presentation_visible(f64) -> bool`, and a progress-based `responsive_layout`.

- [ ] **Step 1: Write failing motion tests**

Add these tests to `dock.rs`:

```rust
#[test]
fn dock_motion_has_exact_endpoints_and_completes_at_180_ms() {
    let mut motion = DockMotion::new(0.0);
    assert_eq!(motion.value(), 0.0);
    motion.request(1.0, 0.0);
    assert_eq!(motion.sample(0.0), 0.0);
    assert!(motion.sample(0.179) < 1.0);
    assert_eq!(motion.sample(0.180), 1.0);
    assert!(!motion.is_active());
}

#[test]
fn dock_motion_is_monotonic_with_cubic_ease_out() {
    let mut motion = DockMotion::new(0.0);
    motion.request(1.0, 0.0);
    let samples = [0.0, 0.03, 0.06, 0.09, 0.12, 0.15, 0.18]
        .map(|time| motion.sample(time));
    assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(samples[0], 0.0);
    assert_eq!(samples[6], 1.0);
    assert!(samples[3] > 0.5, "ease-out must lead linear interpolation");
}

#[test]
fn reversing_motion_starts_from_the_sampled_in_flight_value() {
    let mut motion = DockMotion::new(0.0);
    motion.request(1.0, 0.0);
    let before_reverse = motion.sample(0.09);
    motion.request(0.0, 0.09);
    assert_eq!(motion.value(), before_reverse);
    assert!(motion.sample(0.10) < before_reverse);
    assert_eq!(motion.sample(0.27), 0.0);
}

#[test]
fn repeated_target_requests_do_not_restart_motion() {
    let mut motion = DockMotion::new(0.0);
    motion.request(1.0, 0.0);
    let at_sixty_ms = motion.sample(0.06);
    motion.request(1.0, 0.09);
    assert!(motion.value() > at_sixty_ms);
    assert_eq!(motion.sample(0.18), 1.0);
}
```

- [ ] **Step 2: Replace responsive-layout state tests with progress tests**

Change `responsive_layout` test calls from `DockState` arguments to normalized values and add these cases:

```rust
#[test]
fn wide_layout_interpolates_slots_and_bodies_together() {
    assert_eq!(
        responsive_layout(false, 900.0, 0.5, 0.25, 280.0, 320.0),
        ResponsiveDockLayout {
            left_slot: 140.0,
            right_slot: 80.0,
            tree_body: 140.0,
            inspector_body: 80.0,
        }
    );
}

#[test]
fn narrow_layout_keeps_slots_zero_and_animates_capped_bodies() {
    assert_eq!(
        responsive_layout(true, 240.0, 0.5, 1.0, 280.0, 320.0),
        ResponsiveDockLayout {
            left_slot: 0.0,
            right_slot: 0.0,
            tree_body: 120.0,
            inspector_body: 240.0,
        }
    );
}

#[test]
fn panel_content_stays_visible_until_motion_reaches_zero() {
    assert!(presentation_visible(1.0));
    assert!(presentation_visible(0.001));
    assert!(!presentation_visible(0.0));
}
```

Keep final-width coverage by passing `1.0` for expanded and `0.0` for collapsed in the existing wide/narrow tests.

- [ ] **Step 3: Run the focused tests and confirm failure**

Run:

```powershell
rtk cargo test -p waml-editor dock::tests --lib
```

Expected: compilation fails because `DockMotion`, `presentation_visible`, and the new `responsive_layout` signature do not exist.

- [ ] **Step 4: Implement `DockMotion`**

Add this pure model before `ResponsiveDockLayout`:

```rust
pub const DOCK_MOTION_SECS: f64 = 0.180;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockMotion {
    value: f64,
    from: f64,
    target: f64,
    started_at: f64,
    active: bool,
}

impl Default for DockMotion {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl DockMotion {
    pub fn new(value: f64) -> Self {
        let value = value.clamp(0.0, 1.0);
        Self {
            value,
            from: value,
            target: value,
            started_at: 0.0,
            active: false,
        }
    }

    pub fn request(&mut self, target: f64, now: f64) {
        self.sample(now);
        let target = target.clamp(0.0, 1.0);
        if target == self.target {
            return;
        }
        self.from = self.value;
        self.target = target;
        self.started_at = now;
        self.active = self.from != self.target;
    }

    pub fn sample(&mut self, now: f64) -> f64 {
        if !self.active {
            return self.value;
        }
        let t = ((now - self.started_at) / DOCK_MOTION_SECS).clamp(0.0, 1.0);
        let u = 1.0 - t;
        let eased = 1.0 - u * u * u;
        self.value = self.from + (self.target - self.from) * eased;
        if t >= 1.0 {
            self.value = self.target;
            self.active = false;
        }
        self.value
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

pub fn presentation_visible(value: f64) -> bool {
    value > 0.0
}
```

The `request` method samples before it compares targets. This is the key reversal rule: a target change starts from the current eased value, while a repeated target does not reset `started_at`.

- [ ] **Step 5: Make responsive layout consume presentation values**

Replace the `tree: DockState` and `inspector: DockState` parameters with `tree_value: f64` and `inspector_value: f64`. Use this body:

```rust
pub fn responsive_layout(
    narrow: bool,
    viewport_w: f64,
    tree_value: f64,
    inspector_value: f64,
    tree_w: f64,
    inspector_w: f64,
) -> ResponsiveDockLayout {
    let cap = viewport_w.max(0.0);
    let tree_target = if narrow { tree_w.min(cap) } else { tree_w };
    let inspector_target = if narrow { inspector_w.min(cap) } else { inspector_w };
    let tree_body = tree_target * tree_value.clamp(0.0, 1.0);
    let inspector_body = inspector_target * inspector_value.clamp(0.0, 1.0);
    ResponsiveDockLayout {
        left_slot: if narrow { 0.0 } else { tree_body },
        right_slot: if narrow { 0.0 } else { inspector_body },
        tree_body,
        inspector_body,
    }
}
```

Do not change `slot_width`, `narrow_entry_states`, `narrow_toggle_states`, or the state transition table.

- [ ] **Step 6: Run the dock tests**

Run:

```powershell
rtk cargo test -p waml-editor dock::tests --lib
```

Expected: all dock tests pass.

- [ ] **Step 7: Commit the pure model**

```powershell
rtk git add crates/waml-editor/src/dock.rs
rtk git commit -m "feat(editor): model reversible dock motion"
```

---

### Task 3: Keep panel contents drawable through close animation

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:455-480, 851-897, 1073-1119`
- Modify: `crates/waml-editor/src/inspector_panel.rs:357-376, 566-604, 991-1039`

**Interfaces:**
- Consumes: `dock::presentation_visible(motion.value())` from Task 2.
- Produces: `ProjectTree::set_presentation_visible(&mut self, &mut Cx, bool)` and `Inspector::set_presentation_visible(&mut self, &mut Cx, bool)`.

- [ ] **Step 1: Add independent presentation-visible state to both widgets**

Add this field to `ProjectTree`, next to `dock`. It starts visible because the tree starts `Pinned`:

```rust
#[rust(true)]
presentation_visible: bool,
```

Add the same field to `Inspector`, but use the collapsed default:

```rust
#[rust]
presentation_visible: bool,
```

- [ ] **Step 2: Change the draw gates**

In both `draw_walk` methods, replace:

```rust
if !crate::dock::body_visible(self.dock) {
```

with:

```rust
if !self.presentation_visible {
```

Keep each existing zero-walk branch unchanged. It must still stamp an empty hit area after the close reaches zero.

- [ ] **Step 3: Add idempotent setters**

Add the same method shape to both widget impl blocks:

```rust
pub fn set_presentation_visible(&mut self, cx: &mut Cx, visible: bool) {
    if self.presentation_visible == visible {
        return;
    }
    self.presentation_visible = visible;
    self.view.redraw(cx);
}
```

Do not change `dock_state`, `toggle_dock`, `open_dock`, or `close_dock`. Logical state and draw lifetime must remain separate.

- [ ] **Step 4: Run both panel test modules**

Run:

```powershell
rtk cargo test -p waml-editor tree_panel::tests --lib
rtk cargo test -p waml-editor inspector_panel::tests --lib
```

Expected: all existing tests pass. The Task 2 `panel_content_stays_visible_until_motion_reaches_zero` test proves the value-to-visibility boundary used by `App`.

- [ ] **Step 5: Commit the panel-visibility change**

Inspect the diff and stage only the two files owned by this task.

```powershell
rtk git diff -- crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/inspector_panel.rs
rtk git add -p crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/inspector_panel.rs
rtk git diff --cached --check
rtk git commit -m "feat(editor): retain dock content during close"
```

---

### Task 4: Select open and close glyphs from logical dock state

**Files:**
- Modify: `crates/waml-editor/src/document_header.rs:389-417, 431-460, 628-746`
- Modify: `crates/waml-editor/src/app.rs:1-84, 2202-2216, 3027-3050`

**Interfaces:**
- Consumes: the five `Icon` variants from Task 1 and `DockEdge` / `DockState` from `dock.rs`.
- Produces: `dock_toggle_icon(DockEdge, DockState) -> Icon` and `DocumentHeader::set_right_dock_icon(&mut self, &mut Cx, Icon)`.

- [ ] **Step 1: Write failing glyph-selection tests in `app.rs`**

Add `dock_toggle_icon` to the test module's existing `use super::{...}` list, import `DockEdge`, and add:

```rust
#[test]
fn dock_toggle_glyphs_show_the_next_action() {
    assert_eq!(
        dock_toggle_icon(DockEdge::Left, DockState::Flag),
        Icon::PanelLeftOpen
    );
    assert_eq!(
        dock_toggle_icon(DockEdge::Left, DockState::Pinned),
        Icon::PanelLeftClose
    );
    assert_eq!(
        dock_toggle_icon(DockEdge::Right, DockState::Flag),
        Icon::PanelRightOpen
    );
    assert_eq!(
        dock_toggle_icon(DockEdge::Right, DockState::Pinned),
        Icon::PanelRightClose
    );
}
```

- [ ] **Step 2: Run the test and confirm failure**

Run:

```powershell
rtk cargo test -p waml-editor app::tests::dock_toggle_glyphs_show_the_next_action --lib
```

Expected: compilation fails because `dock_toggle_icon` does not exist.

- [ ] **Step 3: Add the pure glyph selector near the other app helpers**

```rust
fn dock_toggle_icon(edge: crate::dock::DockEdge, state: DockState) -> crate::icons::Icon {
    use crate::dock::DockEdge;
    use crate::icons::Icon;

    match (edge, state == DockState::Flag) {
        (DockEdge::Left, true) => Icon::PanelLeftOpen,
        (DockEdge::Left, false) => Icon::PanelLeftClose,
        (DockEdge::Right, true) => Icon::PanelRightOpen,
        (DockEdge::Right, false) => Icon::PanelRightClose,
    }
}
```

This also gives the compatible `Peek` state a close glyph because clicking it collapses the panel.

- [ ] **Step 4: Add a right-button glyph setter to `DocumentHeader`**

Add this method after `set_right_dock`:

```rust
pub fn set_right_dock_icon(&mut self, cx: &mut Cx, icon: Icon) {
    let Some(current) = self.state.right_dock.as_mut() else {
        return;
    };
    if *current == icon {
        return;
    }
    *current = icon;
    self.view
        .widget(cx, ids!(right_button))
        .as_icon_button()
        .set_icon(cx, icon);
}
```

Add a state-transition test to the existing document-header test that first calls `set_right_dock(..., Some(Icon::PanelRight))`, then calls `set_right_dock_icon(..., Icon::PanelRightOpen)` and `PanelRightClose`, and checks `test_right_dock()` after each call. Also call the setter when no right dock exists and assert that it stays `None`.

- [ ] **Step 5: Set a correct initial left glyph**

In `show_editor`, replace `Icon::PanelLeft` with `Icon::PanelLeftClose`, because the project tree starts pinned. Update the nearby DSL comment to say that runtime synchronization switches between `PanelLeftOpen` and `PanelLeftClose`.

Do not replace `Icon::PanelRight` in document-view chrome producers. Those values declare that the active document has a right dock; `App::sync_dock_slots` owns the action glyph shown on the mounted control.

- [ ] **Step 6: Run focused tests**

```powershell
rtk cargo test -p waml-editor app::tests::dock_toggle_glyphs_show_the_next_action --lib
rtk cargo test -p waml-editor document_header::tests --lib
rtk cargo test -p waml-editor icons_overlay::drift::table_covers_exactly_the_used_icons --lib
```

Expected: all tests pass. The overlay drift test may require moving the now-unwired old `PanelLeft` row exactly as described in Task 1.

- [ ] **Step 7: Commit glyph-state behavior**

```powershell
rtk git add crates/waml-editor/src/app.rs crates/waml-editor/src/document_header.rs crates/waml-editor/src/icons_overlay.rs
rtk git commit -m "feat(editor): show dock toggle actions"
```

---

### Task 5: Integrate app-owned animation and frame scheduling

**Files:**
- Modify: `crates/waml-editor/src/app.rs:665-745, 1566-1651, 2237-2279, 2935-3024, 3690-3850`

**Interfaces:**
- Consumes: `DockMotion`, progress-based `responsive_layout`, `presentation_visible`, the two panel setters, `dock_toggle_icon`, and `DocumentHeader::set_right_dock_icon`.
- Produces: animated writes to `left_slot`, `right_slot`, `tree_host`, and `inspector_host`; a `NextFrame` chain only while motion is active.

- [ ] **Step 1: Add app-owned presentation state**

Add these fields after `dock_layout`:

```rust
#[rust(DockMotion::new(1.0))]
tree_motion: DockMotion,
#[rust]
inspector_motion: DockMotion,
#[rust]
dock_next_frame: NextFrame,
```

Import `DockMotion` with the existing dock imports. The tree starts at `1.0` to match `ProjectTree`'s initial `Pinned` state; the inspector uses `DockMotion::default()` and starts at `0.0` to match `Flag`.

- [ ] **Step 2: Retarget and sample motion inside `sync_dock_slots`**

After responsive-mode reconciliation and after reading `(tree_state, inspector_state)`, add:

```rust
let now = cx.seconds_since_app_start();
self.tree_motion.request(
    if tree_state == DockState::Pinned { 1.0 } else { 0.0 },
    now,
);
self.inspector_motion.request(
    if inspector_state == DockState::Pinned { 1.0 } else { 0.0 },
    now,
);
let tree_value = self.tree_motion.value();
let inspector_value = self.inspector_motion.value();
```

`request` samples at `now`, including for a repeated target, so do not call `sample` a second time.

Pass `tree_value` and `inspector_value` to `responsive_layout` instead of the logical states.

- [ ] **Step 3: Push presentation visibility before geometry redraw**

Before or next to the host-width writes, update both widgets:

```rust
if let Some(mut panel) = self
    .ui
    .widget(cx, ids!(project_tree))
    .borrow_mut::<crate::tree_panel::ProjectTree>()
{
    panel.set_presentation_visible(cx, crate::dock::presentation_visible(tree_value));
}
if let Some(mut panel) = self
    .ui
    .widget(cx, ids!(inspector))
    .borrow_mut::<crate::inspector_panel::Inspector>()
{
    panel.set_presentation_visible(
        cx,
        crate::dock::presentation_visible(inspector_value),
    );
}
```

At an in-flight close, logical state is already `Flag`, but the positive presentation value keeps the widget on its expanded draw path. At exact zero, the existing zero-walk path takes control.

- [ ] **Step 4: Push dynamic glyphs and preserve active styling**

Replace the current left-button update with:

```rust
let tree_button = self.ui.widget(cx, ids!(tree_btn)).as_icon_button();
tree_button.set_icon(
    cx,
    dock_toggle_icon(crate::dock::DockEdge::Left, tree_state),
);
tree_button.set_active(cx, tree_state == DockState::Pinned);
```

Extend the existing document-header update:

```rust
header.set_right_dock_icon(
    cx,
    dock_toggle_icon(crate::dock::DockEdge::Right, inspector_state),
);
header.set_right_dock_active(cx, inspector_state == DockState::Pinned);
```

The icon changes from logical state immediately, before animation completion. A reversal therefore changes the glyph immediately and does not wait for width to reach an endpoint.

- [ ] **Step 5: Request frames only while motion is active**

At the end of `sync_dock_slots`, before `sync_tree_gap`, add:

```rust
if self.tree_motion.is_active() || self.inspector_motion.is_active() {
    self.dock_next_frame = cx.new_next_frame();
}
```

The main event handler already calls `sync_dock_slots(cx)` for every event, including the requested `NextFrame`. The stored token wakes the next event; `request(..., now)` advances each active motion and schedules the next token until both motions complete.

- [ ] **Step 6: Update mounted layout tests for deterministic endpoints**

The mounted dock helpers currently change widget `DockState` and call `sync_dock_slots`. Before assertions that expect final widths, set the corresponding motions to endpoints so tests do not depend on wall-clock progress:

```rust
app.tree_motion = DockMotion::new(if tree == DockState::Pinned { 1.0 } else { 0.0 });
app.inspector_motion =
    DockMotion::new(if inspector == DockState::Pinned { 1.0 } else { 0.0 });
```

Keep the existing wide and narrow geometry assertions. They prove that the final layout has not changed. Add assertions to the mounted wide test that `app.dock_layout.left_slot` and `tree_body` match at the expanded endpoint, and that `right_slot` and `inspector_body` match at the expanded endpoint.

- [ ] **Step 7: Run app, dock, panel, and document-header tests**

```powershell
rtk cargo test -p waml-editor dock::tests --lib
rtk cargo test -p waml-editor app::tests --lib
rtk cargo test -p waml-editor tree_panel::tests --lib
rtk cargo test -p waml-editor inspector_panel::tests --lib
rtk cargo test -p waml-editor document_header::tests --lib
```

Expected: all tests pass, including the existing responsive layout, mounted shell, outside-click, narrow mutual-exclusion, and document-header tests.

- [ ] **Step 8: Commit app integration without unrelated hunks**

```powershell
rtk git diff -- crates/waml-editor/src/app.rs
rtk git add crates/waml-editor/src/app.rs
rtk git diff --cached --check
rtk git commit -m "feat(editor): animate dock panel widths"
```

---

### Task 6: Run full verification and inspect the live motion

**Files:**
- Verify: all files changed in Tasks 1-5
- Create only if a screenshot is needed for review: `shot.png` in the worktree root

**Interfaces:**
- Consumes: the complete feature.
- Produces: test, build, icon-harness, and live-window evidence for integration.

- [ ] **Step 1: Format and inspect the feature diff**

```powershell
rtk cargo fmt --all -- --check
rtk git diff --check
rtk git status --short
```

Expected: formatting and diff checks pass, and the isolated worktree is clean between feature commits.

- [ ] **Step 2: Run the complete editor library suite**

```powershell
rtk cargo test -p waml-editor --lib
```

Expected: all `waml-editor` library tests pass.

- [ ] **Step 3: Build the editor and icon harness**

```powershell
rtk cargo build -p waml-editor --bin waml-editor
rtk cargo build -p waml-editor --bin icon_harness
```

Expected: both binaries build without warnings introduced by this feature.

- [ ] **Step 4: Inspect all five catalog glyphs in the icon harness**

Run the harness in a visible terminal:

```powershell
rtk cargo run -p waml-editor --bin icon_harness
```

Confirm that `folder-tree`, `panel-left-open`, `panel-left-close`, `panel-right-open`, and `panel-right-close` all render at 18 px without clipping, missing strokes, or field-order swaps. Close the harness after inspection.

- [ ] **Step 5: Inspect the editor animation and reversal**

Run:

```powershell
rtk cargo run -p waml-editor --bin waml-editor
```

Check this exact sequence in both wide and narrow windows:

1. Click the left toggle. Confirm that the glyph changes immediately from close to open and that the tree and center widths animate for 180 ms.
2. Click again during motion. Confirm that width reverses from its current position without a jump.
3. Repeat for the right inspector. Confirm that right open/close glyphs follow logical target state.
4. In narrow mode, open the left panel and then the right panel. Confirm that the left closes while the right opens, center slots stay at zero, and body widths stay within the viewport.
5. Close each panel. Confirm that content remains visible while width shrinks and disappears only at zero.
6. Resize the window during motion and cross the responsive breakpoint. Confirm that the current motion continues from its sampled value and final widths remain correct.

Capture evidence when useful:

```powershell
rtk pwsh -File scripts/capture-window.ps1 -Out shot.png -Process waml-editor
```

- [ ] **Step 6: Review commits and integrate**

```powershell
rtk git log --oneline --decorate -6
rtk git status --short
```

Expected feature commits, in order:

1. `feat(editor): add dock action icons`
2. `feat(editor): model reversible dock motion`
3. `feat(editor): retain dock content during close`
4. `feat(editor): show dock toggle actions`
5. `feat(editor): animate dock panel widths`

Use the repository's branch-integration workflow only after all tests, builds, and live checks pass.
