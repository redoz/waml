# ProjectTree Header Turtle Migration Implementation Plan

> **For agentic workers:** Execute tasks top to bottom. Each task is self-contained, ends with a build + test + commit cycle, and leaves `main`-quality green state. This is layout/render code with **zero** unit-test coverage of pixels — do NOT fabricate tests that assert glyph positions; the harness has no such capability. The verification loop per task is `cargo build -p waml-editor` (or `check`) succeeds, `cargo test -p waml-editor` stays green (309+ tests — a regression guard on the untouched tree/icon/note logic), and a `git commit`. Where a task changes pure testable logic, add a real unit test; none of the three tasks below does, so none adds one. Visual parity is verified manually (see each task's "Manual verify").

**Goal:** Kill the absolute-positioning hit-rect bug class for the two interactive glyph controls in `ProjectTree`'s header (collapse + pin) by making them real shared `IconButton` DSL children of a `flow`-laid `View`, exactly as `ToolDock` and `Inspector::element_bar` already do. The composite immediate-mode draws (scope-title trigger, search field, type chip, notes) stay immediate-mode with their existing panel-anchored geometry.

**Architecture:** `crates/waml-editor/src/tree_panel.rs` is a `#[deref] View` hybrid widget: the `View` carries the Atlas HUD frame `draw_bg` and lays out DSL children (`header`, `note_band`, `file_tree`); its `draw_walk` additionally hand-draws the header composites + row glyphs immediate-mode. Today the header is an empty 64px spacer and the collapse/pin glyphs are `IconSet::draw_abs`'d over the panel, hit-tested in `handle_event` against cached `collapse_rect`/`pin_rect` through a `hit_off` translate. This plan replaces the two glyphs with `collapse_btn`/`pin_btn` `IconButton` children inside a real `header` `View` (`flow: Down` → `title_row` + `search_row`), driven per-draw via `set_icon`/`set_active` and read via `IconButton::clicked` from `Event::Actions` — mirroring `inspector_panel.rs:290-313` / `tool_dock.rs:163-186`. The `IconButton` widgets own their own `view.area()` hit-test, so they need no `hit_off`. The composites keep their current `self.view.area().rect(cx)`-anchored math (byte-identical output) — moving them to per-row turtle rects would shift them by the panel's 6px padding, breaking visual parity for no functional gain, so it is deliberately not done.

**Tech Stack:** Rust, makepad (`redoz` fork), `makepad_widgets`. Shared widgets: `crate::icon_button::IconButton` (32×32, `icon_size: 16`, `set_icon`/`set_active`/`clicked`, accent-wash on hover/active). Build/test via cargo. Windows 11 / PowerShell (commands below are shell-agnostic cargo/git invocations).

## Global Constraints

- **Never hand-draw an `IconButton` in a manual `begin_turtle`/`draw_abs`.** Buttons MUST be `flow`-laid DSL children of a `View`. This is what killed the earlier abs-turtle-overlay WIP.
- **`clippy -D warnings` promotes rustc `dead_code` to a hard error.** A `#[rust]` field whose last reader is removed must be removed in the *same* task, or the build breaks. `pin_rect`/`collapse_rect` are read only in `handle_event` and written only in `draw_walk`; both go in Task 2, so the fields go in Task 2 too.
- **Preserve current rendered pixels** for the composite draws (title/search/chip/notes). They keep anchoring off `self.view.area().rect(cx)` + the existing `PAD`/`TITLE_ROW_H`/`HEADER_H` constants. Do not re-anchor them off the new row rects — that shifts them down/right by the panel's `padding: 6.0` and violates parity.
- **`hit_off` stays** for the remaining composite rects (`title_rect`/`search_rect`/`chip_rect`): they are still hand-drawn pre-alignment and events arrive post-alignment (`makepad-aligned-parent-hit-rect-offset`). The panel is left-aligned so `hit_off ≈ 0`, but keep the translate per convention. The two `IconButton` children need no `hit_off` — they hit-test their own `view.area()`; that is the entire point of the migration.
- **No behavior change** to `ScopeRequest` / `Query` / `FilterRequest` seams, the glass/opacity easing (`PanelGlass`), the `Elsewhere`/`Empty`/`Browse`/`Results` notes, `note_band_height`, or the row glyphs.
- **`Flow::Right` cross-axis (y) align is per-child; main-axis (x) is whole-block** (`makepad-turtle-align-shifts-whole-block`). The glyph cluster's vertical centering is per-child (`align:{y:0.5}`); horizontal packing is the block.
- **Stay in the worktree** `C:\dev\waml\.worktrees\tree-filter-select` (verified `git rev-parse --show-toplevel` = `C:/dev/waml/.worktrees/tree-filter-select`). Do not edit main's checkout. `run-native` builds `$PSScriptRoot`'s checkout — launch the worktree's own copy for any visual check.

---

### Task 1: Restructure the header DSL into a real `flow: Down` View with `IconButton` children

Turn the empty `header` spacer into a `flow: Down` `View` holding a `title_row` (`flow: Right`, Fill spacer + `collapse_btn` + `pin_btn`) and a `search_row` (empty spacer reserving the lower band). Import the `IconButton` ref-ext trait. This task adds structure only: the old abs glyph draws and old `collapse_rect`/`pin_rect` hit-tests still fully drive collapse/pin; the new buttons are declared but inert (no icon set, clicks unread), so behavior is unchanged and the build stays green.

**Files:**
- Modify `crates/waml-editor/src/tree_panel.rs:12-18` (add `use` for `IconButtonWidgetRefExt`).
- Modify `crates/waml-editor/src/tree_panel.rs:90-97` (replace the `header` spacer DSL with the two-row structure).

**Interfaces:**
- Consumes: `crate::icon_button::IconButtonWidgetRefExt` (derive-generated trait exposing `WidgetRef::as_icon_button() -> IconButtonRef`); `mod.widgets.IconButton` (DSL widget, default 32×32 / `icon_size: 16`, registered in `icon_button.rs`).
- Produces: DSL node paths `header.title_row.collapse_btn` and `header.title_row.pin_btn` (`IconButton`), addressable via `self.view.widget(cx, ids!(...))`.

Steps:

- [ ] Add the ref-ext import. Change `crates/waml-editor/src/tree_panel.rs:12-18` from:

```rust
use crate::icons::Icon;
use crate::icons::IconSet;
use crate::nav::NavView;
use crate::panel_glass::PanelGlass;
use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::HashMap;
```

to:

```rust
use crate::icon_button::IconButtonWidgetRefExt;
use crate::icons::Icon;
use crate::icons::IconSet;
use crate::nav::NavView;
use crate::panel_glass::PanelGlass;
use crate::tree::{ProjectTree as ProjectTreeData, TreeKind, TreeNode};
use makepad_widgets::*;
use std::collections::HashMap;
```

- [ ] Replace the `header` spacer DSL. Change `crates/waml-editor/src/tree_panel.rs:90-97` from:

```rust
        // Header band: an empty spacer reserving the top strip; the title
        // trigger, collapse/pin glyphs, and (Task 9) the search row + type chip
        // are all hand-drawn immediate-mode in `draw_walk`, same hybrid as the
        // inspector.
        header := View {
            width: Fill
            height: 64.0
        }
```

to:

```rust
        // Header band: a real `flow: Down` container. `title_row` hosts the two
        // interactive glyph controls as shared `IconButton` children (packed
        // right behind a Fill spacer); the scope-title trigger is still drawn
        // immediate-mode over the row's left. `search_row` is an empty spacer
        // reserving the lower band, over which the search field + type chip are
        // drawn immediate-mode -- the same hybrid `inspector::element_bar` uses.
        // 34 + 30 = 64 keeps the body's top position and `note_band` unchanged.
        header := View {
            width: Fill
            height: 64.0
            flow: Down
            title_row := View {
                width: Fill
                height: 34.0
                flow: Right
                align: Align{y: 0.5}
                padding: Inset{left: 10.0, right: 10.0}
                spacing: 6.0
                // Fill spacer pushes the glyph cluster to the right edge; the
                // scope-title trigger is drawn abs into the leading space.
                title_spacer := View { width: Fill, height: Fill }
                collapse_btn := IconButton {}
                pin_btn := IconButton {}
            }
            search_row := View {
                width: Fill
                height: 30.0
            }
        }
```

- [ ] Build. `cargo build -p waml-editor` — expect success (the new `IconButton` children resolve via the DSL `use mod.widgets.*`; the ref-ext import is now consumed nowhere yet, so expect a single `unused import` warning — that is fine for a non-`-D` build and is resolved in Task 2). To keep the tree strictly warning-clean at this checkpoint, add `#[allow(unused_imports)]` above the new `use` line only if `cargo clippy` is gated; otherwise leave it — Task 2 consumes it. Prefer leaving it and NOT gating clippy on this commit.
- [ ] Test. `cargo test -p waml-editor` — expect `test result: ok.` with the full pre-existing count (309+), unchanged (this task touches only DSL layout, no logic).
- [ ] Manual verify (optional at this checkpoint): launch the worktree's own `run-native` (dedicated pid, screenshot by that pid per `screenshot-verify-hits-user-editor` — never grab the user's editor, never kill-all). The header still renders identically (old abs glyphs still draw; new buttons are icon-less/transparent). Collapse/pin still work via the old hit path.
- [ ] Commit. `git add -A && git commit` with message:

```
refactor(tree): header DSL becomes flow:Down View with IconButton children

Turn the empty 64px header spacer into a real flow:Down View holding a
title_row (Fill spacer + collapse_btn/pin_btn IconButtons) and an empty
search_row band. Import IconButtonWidgetRefExt. Structure only -- the old
abs glyph draws and collapse_rect/pin_rect hit-tests still drive collapse
and pin; the new buttons are inert until the next commit. Mirrors
tool_dock / inspector::element_bar.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
```

---

### Task 2: Swap collapse/pin onto the `IconButton` children (draw + event) and drop the abs glyphs + cached rects

Atomically replace the two absolute glyph controls. In `draw_walk`, drive `collapse_btn`/`pin_btn` via `set_icon`/`set_active` and delete the two `IconSet::draw_abs` blocks (which also wrote `pin_rect`/`collapse_rect`). In `handle_event`, read `collapse_btn.clicked`/`pin_btn.clicked` from `Event::Actions` and delete the `pin_rect`/`collapse_rect` `contains(p)` branches. Remove the now-unused `pin_rect`/`collapse_rect` fields in the same commit (dead_code gate). Done atomically so collapse/pin stay functional with no double-toggle and no dead interim. The composite title/search/chip draws and their `hit_off` path are untouched.

**Files:**
- Modify `crates/waml-editor/src/tree_panel.rs:388-391` (remove `collapse_rect`/`pin_rect` fields).
- Modify `crates/waml-editor/src/tree_panel.rs:545-575` (drive the two buttons before layout).
- Modify `crates/waml-editor/src/tree_panel.rs:604-629` (delete the abs pin/collapse glyph blocks; keep `cy` + `dim` + the title trigger).
- Modify `crates/waml-editor/src/tree_panel.rs:800-835` (remove the two `contains` branches).
- Modify `crates/waml-editor/src/tree_panel.rs:864-888` (add the two button-click reads to the existing `Event::Actions` arm).

**Interfaces:**
- Consumes: `IconButtonRef::set_icon(&self, cx, Icon)`, `set_active(&self, cx, bool)`, `clicked(&self, &Actions) -> bool` (from `icon_button.rs:212-241`); `Icon::{ListExpand, ListCollapse, Pin, PinOff}`; `PanelGlass::pinned` / `PanelGlass::toggle_pin`.
- Produces: no new public API. Removes `#[rust] pin_rect: Rect` and `#[rust] collapse_rect: Rect` from `ProjectTree`.

Steps:

- [ ] Remove the two fields. Delete `crates/waml-editor/src/tree_panel.rs:388-391`:

```rust
    #[rust]
    collapse_rect: Rect,
    #[rust]
    pin_rect: Rect,
```

(Leave `header_rect` and `title_rect` — `header_rect` still anchors `hit_off` + `body_top`; `title_rect` is still the scope-trigger hit rect.)

- [ ] Drive the two buttons at the top of `draw_walk`. In `crates/waml-editor/src/tree_panel.rs`, immediately after the `note_band` visibility block (currently ends at line 558) and before `let mut walk = walk;` (line 560), insert:

```rust
        // Sync the two interactive glyph controls onto their shared `IconButton`
        // children before the header View lays them out: collapse shows the fold
        // chevron (reusing the inspector's `ListCollapse`/`ListExpand`), pin
        // shows Pin/PinOff and reads lit while the panel is pinned (matches the
        // inspector). Their clicks are read in `handle_event` from `Event::Actions`
        // -- they own their own `view.area()` hit-test, so no `hit_off`.
        let collapse_btn = self.view.widget(cx, ids!(header.title_row.collapse_btn));
        collapse_btn.as_icon_button().set_icon(
            cx,
            if self.collapsed {
                Icon::ListExpand
            } else {
                Icon::ListCollapse
            },
        );
        let pin_btn = self.view.widget(cx, ids!(header.title_row.pin_btn));
        pin_btn
            .as_icon_button()
            .set_icon(cx, if self.panel.pinned { Icon::Pin } else { Icon::PinOff });
        pin_btn.as_icon_button().set_active(cx, self.panel.pinned);
```

- [ ] Delete the abs glyph blocks. In `draw_walk`, remove `crates/waml-editor/src/tree_panel.rs:604-629` (the right-cluster comment, the `pin` block, and the `collapse` block) — from the line:

```rust
        // Right cluster, right -> left: pin, then the fold chevron (reusing
```

through and including:

```rust
        let dc = self.icons.get(collapse_icon);
        dc.color = dim;
        dc.draw_abs(cx, collapse);
```

Keep the two lines above it (`let cy = rect.pos.y + TITLE_ROW_H * 0.5;` and `let dim = self.draw_dim.color;`) — `cy` still positions the title trigger and `dim` still tints the magnifier/chip/notes — and keep everything from the `// Scope-title trigger` comment onward unchanged.

- [ ] Read the button clicks. In `handle_event`, extend the existing `if let Event::Actions(actions) = event {` block (currently at line 864, guarding the `file_tree.file_clicked` read) by inserting the two reads at the top of the block, immediately after the `{`:

```rust
        if let Event::Actions(actions) = event {
            // The two glyph controls are `IconButton` children now; read their
            // clicks here instead of hit-testing cached rects. `self.view.
            // handle_event` above already drove the children so these actions
            // are present.
            if self
                .view
                .widget(cx, ids!(header.title_row.collapse_btn))
                .as_icon_button()
                .clicked(actions)
            {
                self.collapsed = !self.collapsed;
                self.view.redraw(cx);
            }
            if self
                .view
                .widget(cx, ids!(header.title_row.pin_btn))
                .as_icon_button()
                .clicked(actions)
            {
                self.panel.toggle_pin(cx);
                self.view.redraw(cx);
            }
            if let Some(id) = file_tree.file_clicked(actions) {
```

(The rest of the block — the `file_clicked` body — is unchanged; you are only inserting the two `if` blocks ahead of the existing `if let Some(id) = file_tree.file_clicked(actions) {` line.)

- [ ] Remove the two `contains` branches. In the `Hit::FingerUp` arm, delete `crates/waml-editor/src/tree_panel.rs:803-812`:

```rust
                if self.pin_rect.contains(p) {
                    self.panel.toggle_pin(cx);
                    self.view.redraw(cx);
                    return;
                }
                if self.collapse_rect.contains(p) {
                    self.collapsed = !self.collapsed;
                    self.view.redraw(cx);
                    return;
                }
```

Leave the `let p = fe.abs - hit_off;` line and the `title_rect`/`search_rect`/`chip_rect` branches that follow — they still use `hit_off`.

- [ ] Build. `cargo build -p waml-editor` — expect success. Confirm no `dead_code` complaint: `cargo clippy -p waml-editor -- -D warnings` should pass (the `IconButtonWidgetRefExt` import is now consumed; `pin_rect`/`collapse_rect` are gone; `cy`/`dim` are still read).
- [ ] Test. `cargo test -p waml-editor` — expect the full pre-existing count green, unchanged.
- [ ] Manual verify (dedicated pid per `screenshot-verify-hits-user-editor`): collapse toggles the body (header stays, frame hugs); pin toggles the glass and the pin button reads lit; both now show the shared `IconButton` hover wash. Expect the glyph cluster to sit ~6px lower and read as 32px accent-wash buttons (the intended shared-button look) rather than the old bare 16px glyphs — this is the accepted visual change; title/search/chip are byte-identical. Confirm the scope-title trigger, search field, and type chip (filter `SelectFlyout`) still open.
- [ ] Commit. `git add -A && git commit` with message:

```
feat(tree): collapse/pin become shared IconButton children

Drive collapse_btn/pin_btn via set_icon/set_active each draw_walk and read
their clicks from Event::Actions, dropping the two IconSet::draw_abs glyph
blocks and the pin_rect/collapse_rect contains() branches + fields. The
buttons own their view.area() hit-test, so no hit_off for them. Composite
title/search/chip draws and their hit_off path are untouched (byte-identical).
Mirrors inspector_panel.rs:290-313 / tool_dock.rs:163-186.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
```

---

### Task 3: Refresh stale doc comments and run the full verification pass

Documentation-only + verification. Update the module/struct comments that still describe the collapse/pin controls as "hand-drawn immediate-mode" so the source matches the shipped hybrid. No behavior change. Close with a full clippy + test pass and a final manual parity check.

**Files:**
- Modify `crates/waml-editor/src/tree_panel.rs:1-10` (module doc: note the header's two `IconButton` glyph controls).
- Modify `crates/waml-editor/src/tree_panel.rs:334-341` (struct comment above `draw_title`/`draw_dim` — the "glyph tint source" note is now only for the composites).

**Interfaces:**
- Consumes: nothing new.
- Produces: nothing new. Comment-only edits.

Steps:

- [ ] Update the module doc. In `crates/waml-editor/src/tree_panel.rs:1-10`, append a sentence to the module-level `//!` block describing the header as a real `flow: Down` View whose collapse/pin controls are shared `IconButton` children while the scope-title trigger, search field, and type chip stay immediate-mode. Concretely, change the final paragraph:

```rust
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.
```

to:

```rust
//! Structure mirrors studio's `DesktopFileTree` / `FlatFileTree`, minus the
//! filter page and git-status dots.
//!
//! The header is a real `flow: Down` `View`: its collapse + pin controls are
//! shared `IconButton` children (they own their own hover/click/`view.area()`
//! hit-test), while the scope-title trigger, search field, and type chip stay
//! immediate-mode hand-drawn over the header band -- the same hybrid the
//! inspector's `element_bar` uses.
```

- [ ] Update the `draw_title`/`draw_dim` field comment. In `crates/waml-editor/src/tree_panel.rs:334-341`, change:

```rust
    // Header band ink (Task 8). `draw_title` is the scope-title label;
    // `draw_dim` is everything subdued (the `⌄`, glyph tint source).
    #[redraw]
    #[live]
    draw_title: DrawText,
```

to:

```rust
    // Header band ink. `draw_title` is the scope-title label; `draw_dim` is
    // everything subdued (the `⌄`, plus the search/chip/note tint source). The
    // collapse/pin glyph tint now lives in the `IconButton` children, not here.
    #[redraw]
    #[live]
    draw_title: DrawText,
```

- [ ] Build + full lint. `cargo build -p waml-editor` then `cargo clippy -p waml-editor -- -D warnings` — expect both clean (comment-only changes cannot introduce dead code).
- [ ] Test. `cargo test -p waml-editor` — expect the full pre-existing count green.
- [ ] Manual verify (final parity pass, dedicated pid): header renders correctly; every click target lands — scope-title trigger opens the scope dropdown; collapse toggles the body; pin toggles glass + reads lit; search field takes focus and edits (caret, backspace, escape); type chip opens the filter `SelectFlyout`; the shipped per-kind filter dropdown still lands. Collapsed: header stays, body hides, frame hugs the header.
- [ ] Commit. `git add -A && git commit` with message:

```
docs(tree): header comments describe the IconButton hybrid

Module + field comments now say collapse/pin are shared IconButton children
and the composite title/search/chip stay immediate-mode. Comment-only;
full clippy + test pass.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>
```
