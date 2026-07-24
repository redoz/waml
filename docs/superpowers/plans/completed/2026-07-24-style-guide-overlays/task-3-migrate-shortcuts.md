# Task 3: Migrate `ShortcutsOverlay` onto `OverlayShell`

**Files:**
- Modify: `crates/waml-editor/src/shortcuts_overlay.rs`
- (No `app.rs` change — its `toggle_shortcuts_overlay` / `set_shortcuts_overlay` / `overlay_action` calls are preserved verbatim by keeping the same method names.)

**Interfaces:**
- Consumes (Task 2): `OverlayShell`, `OverlayShellAction`, `OverlayShellPass`.
- Preserves (App relies on these unchanged): `ShortcutsOverlay::visible(&self) -> bool`, `set_visible(&mut self, cx: &mut Cx, bool)`, `overlay_action(&self, &Actions) -> Option<ShortcutsOverlayAction>`, and `pub const BINDINGS`.

**Goal:** delete the bespoke scrim/panel/edge/dismiss code and route everything through the embedded shell, proving the abstraction against pre-existing behavior. The overlay now paints over the caption band too (the shell's window-overlay draw) — an intended upgrade over the old body-flow draw.

**First per-pid visual verify lands here** (Trap 3) — the headless gate cannot catch a dead node or blank text.

---

- [ ] **Step 1: Update the `script_mod!` DSL**

In `shortcuts_overlay.rs`, replace the `mod.widgets.ShortcutsOverlay` default body so the scrim/panel/edge move under a nested `shell:` block and `draw_key`/`draw_desc`/`draw_title` stay. Keep the imports (`use mod.atlas`, `use mod.fonts`, etc.):

```rust
    mod.widgets.ShortcutsOverlay = set_type_default() do mod.widgets.ShortcutsOverlayBase{
        width: Fill
        height: Fill
        shell: {
            panel_width: 360.0
            draw_scrim +: { color: atlas.scrim }
            draw_panel +: { color: atlas.surface }
            draw_edge +: { color: atlas.frame_hi }
            draw_thumb +: { color: atlas.frame_lo }
        }
        draw_title +: {
            color: atlas.text
            text_style: fonts.text_title
        }
        draw_key +: {
            color: atlas.text
            text_style: fonts.text_heading
        }
        draw_desc +: {
            color: atlas.text_dim
            text_style: fonts.text_body
        }
    }
```

- [ ] **Step 2: Replace the struct fields**

Delete `draw_scrim`, `draw_panel`, `draw_edge`, `visible`, and `panel_rect` fields and the `PANEL_W` const. Add the embedded shell. Keep `draw_title`/`draw_key`/`draw_desc`, `uid`, `source`, `walk`, `layout`. New field set:

```rust
#[derive(Script, ScriptHook, Widget)]
pub struct ShortcutsOverlay {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[live]
    shell: crate::overlay_shell::OverlayShell,

    #[redraw]
    #[live]
    draw_title: DrawText,
    #[redraw]
    #[live]
    draw_key: DrawText,
    #[redraw]
    #[live]
    draw_desc: DrawText,
}
```

Keep the layout constants used for content: `PANEL_PAD`, `TITLE_H`, `ROW_H`, `KEY_COL_W` (the shell now owns the outer pad, but these still lay out the rows within the content column — leave them as-is).

- [ ] **Step 3: Replace `Widget::draw_walk` with the seam**

```rust
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        let h = self.content_height();
        if let Some(pass) = self.shell.begin(cx, h) {
            self.draw_rows(cx, pass.origin, pass.width);
            self.shell.end(cx);
        }
        DrawStep::done()
    }
```

- [ ] **Step 4: Replace `Widget::handle_event`**

```rust
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let OverlayShellAction::Dismissed = self.shell.handle_event(cx, event) {
            cx.widget_action(self.widget_uid(), ShortcutsOverlayAction::Dismissed);
        }
    }
```

Add `use crate::overlay_shell::{OverlayShell, OverlayShellAction};` near the top (alongside `use makepad_widgets::*;`).

- [ ] **Step 5: Add `content_height` + `draw_rows`, rewrite the inherent methods**

Replace the old inherent `impl` (the `draw_walk` body's title/rows loop moves into `draw_rows`; `visible`/`set_visible` delegate to the shell):

```rust
impl ShortcutsOverlay {
    /// Content height the shell needs to size + scroll the panel.
    fn content_height(&self) -> f64 {
        TITLE_H + BINDINGS.len() as f64 * ROW_H
    }

    /// Draw the title + key/desc rows relative to the shell-provided origin.
    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        self.draw_title.draw_abs(cx, origin, "Shortcuts");
        let mut y = origin.y + TITLE_H;
        for (key, desc) in BINDINGS {
            self.draw_key.draw_abs(cx, dvec2(origin.x, y), key);
            self.draw_desc
                .draw_abs(cx, dvec2(origin.x + KEY_COL_W, y), desc);
            y += ROW_H;
        }
    }

    pub fn visible(&self) -> bool {
        self.shell.is_open()
    }

    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.shell.set_open(cx, visible);
    }

    /// Convenience reader for `App`, mirroring `ToolDock::dock_action`.
    pub fn overlay_action(&self, actions: &Actions) -> Option<ShortcutsOverlayAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            ShortcutsOverlayAction::None => None,
            action => Some(action),
        }
    }
}
```

Delete the now-unused `PANEL_PAD` if the compiler flags it (dead_code is a hard error); keep the ones still referenced (`TITLE_H`, `ROW_H`, `KEY_COL_W`). Remove `KEY_COL_W`'s siblings only if unused.

- [ ] **Step 6: Run the preserved test + gate**

Run: `cargo test -p waml-editor --lib shortcuts_overlay`
Expected: PASS — `bindings_list_is_non_empty_and_has_the_toggle_and_close_keys` unchanged and green.

Run: `cargo clippy -p waml-editor -- -D warnings`
Expected: clean (no leftover dead constants/fields).

- [ ] **Step 7: Per-pid visual verify (Trap 3 — mandatory)**

Build + launch the WORKTREE's own copy, capture the pid, screenshot BY pid in a single PowerShell call, then stop that pid only. Do NOT screenshot/kill by name.

```powershell
$p = Start-Process -PassThru pwsh -ArgumentList '-File','scripts/run-native.ps1'
# ...wait for the window, then in ONE call capture by $p.Id and save a PNG, then:
Stop-Process -Id $p.Id -Confirm:$false
```

Confirm: press `?` (or the tool-dock Shortcuts button) → the overlay draws a scrim + centered panel with the **"Shortcuts" title and all key/desc rows visible** (blank text = the namespace/registration trap — investigate before proceeding), now covering the caption band too; `Esc` closes it; a click on the scrim (outside the panel) closes it; a click inside the panel does not.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/shortcuts_overlay.rs
git commit -m "refactor(shortcuts): migrate ShortcutsOverlay onto shared OverlayShell"
```

Verification: full gate `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build` green; visual verify passed.
