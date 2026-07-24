# Task 4: Fonts overlay + wordmark row + one-open-at-a-time

**Files:**
- Create: `crates/waml-editor/src/fonts_overlay.rs`
- Modify: `crates/waml-editor/src/main.rs` (`mod fonts_overlay;`)
- Modify: `crates/waml-editor/src/app.rs` (menu row, `LogoCommand`, mount,
  registration, mutual-exclusion helpers)

**Interfaces:**
- Consumes (Tasks 2-3): `OverlayShell`, `OverlayShellAction`, the seam.
- Produces: `FontsOverlay` widget with `visible(&self) -> bool` /
  `set_visible(&mut self, cx: &mut Cx, bool)` / `overlay_action(&self, &Actions)
  -> Option<FontsOverlayAction>` (the `ShortcutsOverlay` API, so App drives it the
  same way), and `pub const ROLES`.
- Produces on `App`: `close_page_overlays(&mut self, cx)` +
  `open_page_overlay(&mut self, cx, which)` — the one-open-at-a-time seam covering
  the shortcuts overlay AND every page.

**Goal:** first style-guide page. Each row shows a `mod.fonts` role's live sample
(rendered in the REAL token so it can't drift) + its spec line.

---

- [ ] **Step 1: Register the module** — add `mod fonts_overlay;` in `main.rs`
  (near the other overlay/page modules).

- [ ] **Step 2: The widget + its `script_mod!`**

Create `crates/waml-editor/src/fonts_overlay.rs`. Wire each sample field to its
REAL role token (this is what makes the page a live style-guide). Keep the block a
SINGLE `set_type_default() do ...{ }` object-literal (README trap #1); the `shell:`
sub-block supplies the shared chrome colors exactly like Task 3's migration.

```rust
//! Fonts style-guide overlay: one row per `mod.fonts` role, each showing the
//! role name, a pangram rendered in the REAL role token (wired below, so it
//! tracks any edit to the scale), and the role's spec line. Rides the shared
//! `OverlayShell` (scrim/panel/scroll/dismiss); provides content only.

use crate::overlay_shell::{OverlayShell, OverlayShellAction};
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.fonts
    use mod.widgets.*
    use mod.text.*

    mod.widgets.FontsOverlayBase = #(FontsOverlay::register_widget(vm))

    mod.widgets.FontsOverlay = set_type_default() do mod.widgets.FontsOverlayBase{
        width: Fill
        height: Fill
        shell: {
            panel_width: 460.0
            draw_scrim +: { color: atlas.scrim }
            draw_panel +: { color: atlas.surface }
            draw_edge +: { color: atlas.frame_hi }
            draw_thumb +: { color: atlas.frame_lo }
        }
        draw_role +: { color: atlas.text_dim, text_style: fonts.text_label }
        draw_spec +: { color: atlas.text_dim, text_style: fonts.text_mono }
        // Samples wired to the real role tokens.
        draw_sample_title +:   { color: atlas.text, text_style: fonts.text_title }
        draw_sample_heading +: { color: atlas.text, text_style: fonts.text_heading }
        draw_sample_body +:    { color: atlas.text, text_style: fonts.text_body }
        draw_sample_label +:   { color: atlas.text, text_style: fonts.text_label }
        draw_sample_menu +:    { color: atlas.text, text_style: fonts.text_menu }
        draw_sample_eyebrow +: { color: atlas.text, text_style: fonts.text_eyebrow }
        draw_sample_mono +:    { color: atlas.text, text_style: fonts.text_mono }
    }
}
```

- [ ] **Step 3: Action + struct**

```rust
#[derive(Clone, Debug, Default)]
pub enum FontsOverlayAction {
    #[default]
    None,
    Dismissed,
}

#[derive(Script, ScriptHook, Widget)]
pub struct FontsOverlay {
    #[uid] uid: WidgetUid,
    #[source] source: ScriptObjectRef,
    #[walk] walk: Walk,
    #[layout] layout: Layout,

    #[live] shell: OverlayShell,

    #[redraw] #[live] draw_role: DrawText,
    #[redraw] #[live] draw_spec: DrawText,
    #[redraw] #[live] draw_sample_title: DrawText,
    #[redraw] #[live] draw_sample_heading: DrawText,
    #[redraw] #[live] draw_sample_body: DrawText,
    #[redraw] #[live] draw_sample_label: DrawText,
    #[redraw] #[live] draw_sample_menu: DrawText,
    #[redraw] #[live] draw_sample_eyebrow: DrawText,
    #[redraw] #[live] draw_sample_mono: DrawText,
}
```

- [ ] **Step 4: The role table + shared sample**

```rust
/// Shared preview string (a pangram exercises ascenders/descenders/caps).
const SAMPLE: &str = "The five boxing wizards jump quickly — 0123456789";

/// (role name, spec line). ORDER matches the `draw_sample_*` match in `draw_rows`
/// AND `mod.fonts`'s scale order. The coverage test locks this to the 7 roles.
pub const ROLES: [(&str, &str); 7] = [
    ("Title",   "IBM Plex Sans Condensed SemiBold · 16px · 1.1"),
    ("Heading", "IBM Plex Sans SemiBold · 13px · 1.2"),
    ("Body",    "IBM Plex Sans Regular · 12px · 1.2"),
    ("Label",   "IBM Plex Sans Medium · 11px · 1.2"),
    ("Menu",    "IBM Plex Sans Regular · 10px · 1.2"),
    ("Eyebrow", "IBM Plex Sans SemiBold · 10px · 1.2"),
    ("Mono",    "IBM Plex Mono Regular · 11px · 1.2"),
];

const ROW_H: f64 = 64.0;
```

- [ ] **Step 5: `Widget` impl (seam) + inherent methods**

```rust
impl Widget for FontsOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let OverlayShellAction::Dismissed = self.shell.handle_event(cx, event) {
            cx.widget_action(self.widget_uid(), FontsOverlayAction::Dismissed);
        }
    }
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, _walk: Walk) -> DrawStep {
        let h = self.content_height();
        if let Some(pass) = self.shell.begin(cx, h) {
            self.draw_rows(cx, pass.origin, pass.width);
            self.shell.end(cx);
        }
        DrawStep::done()
    }
}

impl FontsOverlay {
    fn content_height(&self) -> f64 { ROLES.len() as f64 * ROW_H }

    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        for (i, (name, spec)) in ROLES.iter().enumerate() {
            let y = origin.y + i as f64 * ROW_H;
            self.draw_role.draw_abs(cx, dvec2(origin.x, y), name);
            self.draw_spec.draw_abs(cx, dvec2(origin.x, y + ROW_H - 16.0), spec);
            let sy = dvec2(origin.x, y + 20.0);
            // 7 distinct DrawText fields carry the 7 styles; pick per role index.
            match i {
                0 => self.draw_sample_title.draw_abs(cx, sy, SAMPLE),
                1 => self.draw_sample_heading.draw_abs(cx, sy, SAMPLE),
                2 => self.draw_sample_body.draw_abs(cx, sy, SAMPLE),
                3 => self.draw_sample_label.draw_abs(cx, sy, SAMPLE),
                4 => self.draw_sample_menu.draw_abs(cx, sy, SAMPLE),
                5 => self.draw_sample_eyebrow.draw_abs(cx, sy, SAMPLE),
                _ => self.draw_sample_mono.draw_abs(cx, sy, SAMPLE),
            }
        }
    }

    pub fn visible(&self) -> bool { self.shell.is_open() }
    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) { self.shell.set_open(cx, visible); }
    pub fn overlay_action(&self, actions: &Actions) -> Option<FontsOverlayAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() { FontsOverlayAction::None => None, a => Some(a) }
    }
}
```

- [ ] **Step 6: Coverage test (drift guard) + logo-command test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_table_covers_the_7_mod_fonts_roles() {
        // The 7 role tokens in mod.fonts, in scale order. If fonts.rs gains/loses
        // a role, this list + ROLES must move together.
        const CANON: [&str; 7] = ["Title","Heading","Body","Label","Menu","Eyebrow","Mono"];
        assert_eq!(ROLES.len(), 7);
        for (i, (name, _)) in ROLES.iter().enumerate() {
            assert_eq!(*name, CANON[i], "role {i} drifted from the mod.fonts scale");
        }
    }
}
```

- [ ] **Step 7: Wire into `app.rs`**

  1. **Import** — in `App`'s `script_mod!` block (top of `app.rs`, alongside
     `use mod.widgets.ShortcutsOverlay`): `use mod.widgets.FontsOverlay`.
  2. **Mount** — in the body `Overlay` flow (next to `shortcuts_overlay :=` ~L341):
     `fonts_overlay := FontsOverlay{}`.
  3. **Register** — in `App::script_mod` (the list ending ~L1915), add
     `crate::fonts_overlay::script_mod(vm);` right after
     `crate::shortcuts_overlay::script_mod(vm);` and BEFORE `self::script_mod(vm)`
     (registration-order trap — a page registered after the App block is a dead
     node).
  4. **Menu row** — in `logo_menu_items()`, add a **Fonts** row before the Exit
     danger row: `id: live_id!(fonts)`, `label: "Fonts"`, `icon: Icon::Paintbrush`
     (a catalog glyph; NOTE: this makes `Paintbrush` a used icon — Task 5's Icons
     table must list it), `danger: false`, `enabled: true`.
  5. **Command** — add `Fonts` to `LogoCommand`; `logo_command_for(live_id!(fonts))
     => Some(LogoCommand::Fonts)`; update the existing mapping test.
  6. **Handler + mutual exclusion** — add the helpers on `App`:
     ```rust
     /// Close the shortcuts overlay AND every style-guide page. Every open path
     /// calls this first, so exactly one overlay is ever visible.
     fn close_page_overlays(&mut self, cx: &mut Cx) {
         self.set_shortcuts_overlay(cx, false);
         if let Some(mut o) = self.ui.widget(cx, ids!(fonts_overlay))
             .borrow_mut::<crate::fonts_overlay::FontsOverlay>() { o.set_visible(cx, false); }
         // Tasks 5-6 extend this with icons_overlay / colors_overlay.
     }
     ```
     In the `logo_closed` `Invoked` match (~L1116), `LogoCommand::Fonts =>` closes
     all overlays then shows the fonts overlay:
     ```rust
     LogoCommand::Fonts => {
         self.close_page_overlays(cx);
         if let Some(mut o) = self.ui.widget(cx, ids!(fonts_overlay))
             .borrow_mut::<crate::fonts_overlay::FontsOverlay>() { o.set_visible(cx, true); }
     }
     ```
  7. **Dismiss** — mirror the shortcuts dismiss (~L1699): read
     `fonts_overlay.overlay_action(actions)`; on `Dismissed`, `set_visible(cx,
     false)`. Also extend the `Escape` hotkey path (~L1963) to call
     `close_page_overlays(cx)` so Esc closes a page too.

- [ ] **Step 8: Gate + per-pid visual verify (mandatory, Trap 3)**

  - `cargo test -p waml-editor` green (coverage + logo-command tests).
  - Full gate `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`.
  - Build+launch the WORKTREE's own `scripts/run-native.ps1`; capture the spawned
    pid; screenshot BY that pid in ONE PowerShell call; `Stop-Process -Id $pid`.
    NEVER by-name. Confirm (light + dark via `T`): the wordmark menu shows a
    **Fonts** row; clicking it opens the overlay; all 7 samples render IN their
    real styles (sizes visibly differ — blank/uniform text = the namespace or
    registration trap, investigate before proceeding); spec lines read correctly;
    Esc and scrim-click close it; opening Fonts while Shortcuts is open closes
    Shortcuts (and vice-versa).

- [ ] **Step 9: Commit** — `git commit -m "feat(editor): Fonts style-guide overlay + wordmark row"`
