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
        shell +: {
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

#[derive(Clone, Debug, Default)]
pub enum FontsOverlayAction {
    #[default]
    None,
    Dismissed,
}

/// Shared preview string (a pangram exercises ascenders/descenders/caps).
const SAMPLE: &str = "The five boxing wizards jump quickly — 0123456789";

/// (role name, spec line). ORDER matches the `draw_sample_*` match in `draw_rows`
/// AND `mod.fonts`'s scale order. The coverage test locks this to the 7 roles.
pub const ROLES: [(&str, &str); 7] = [
    ("Title", "IBM Plex Sans Condensed SemiBold · 16px · 1.1"),
    ("Heading", "IBM Plex Sans SemiBold · 13px · 1.2"),
    ("Body", "IBM Plex Sans Regular · 12px · 1.2"),
    ("Label", "IBM Plex Sans Medium · 11px · 1.2"),
    ("Menu", "IBM Plex Sans Regular · 10px · 1.2"),
    ("Eyebrow", "IBM Plex Sans SemiBold · 10px · 1.2"),
    ("Mono", "IBM Plex Mono Regular · 11px · 1.2"),
];

const ROW_H: f64 = 64.0;

#[derive(Script, ScriptHook, Widget)]
pub struct FontsOverlay {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[live]
    shell: OverlayShell,

    #[redraw]
    #[live]
    draw_role: DrawText,
    #[redraw]
    #[live]
    draw_spec: DrawText,
    #[redraw]
    #[live]
    draw_sample_title: DrawText,
    #[redraw]
    #[live]
    draw_sample_heading: DrawText,
    #[redraw]
    #[live]
    draw_sample_body: DrawText,
    #[redraw]
    #[live]
    draw_sample_label: DrawText,
    #[redraw]
    #[live]
    draw_sample_menu: DrawText,
    #[redraw]
    #[live]
    draw_sample_eyebrow: DrawText,
    #[redraw]
    #[live]
    draw_sample_mono: DrawText,
}

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
    fn content_height(&self) -> f64 {
        ROLES.len() as f64 * ROW_H
    }

    fn draw_rows(&mut self, cx: &mut Cx2d, origin: DVec2, _width: f64) {
        for (i, (name, spec)) in ROLES.iter().enumerate() {
            let y = origin.y + i as f64 * ROW_H;
            self.draw_role.draw_abs(cx, dvec2(origin.x, y), name);
            self.draw_spec
                .draw_abs(cx, dvec2(origin.x, y + ROW_H - 16.0), spec);
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

    // Mirrors `ShortcutsOverlay`'s API shape; not yet consumed here (App drives
    // this page purely through `set_visible`/`overlay_action`), kept public for
    // parity and future toggle-style callers.
    #[allow(dead_code)]
    pub fn visible(&self) -> bool {
        self.shell.is_open()
    }

    pub fn set_visible(&mut self, cx: &mut Cx, visible: bool) {
        self.shell.set_open(cx, visible);
    }

    pub fn overlay_action(&self, actions: &Actions) -> Option<FontsOverlayAction> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.cast() {
            FontsOverlayAction::None => None,
            a => Some(a),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roles_table_covers_the_7_mod_fonts_roles() {
        // The 7 role tokens in mod.fonts, in scale order. If fonts.rs gains/loses
        // a role, this list + ROLES must move together.
        const CANON: [&str; 7] = [
            "Title", "Heading", "Body", "Label", "Menu", "Eyebrow", "Mono",
        ];
        assert_eq!(ROLES.len(), 7);
        for (i, (name, _)) in ROLES.iter().enumerate() {
            assert_eq!(*name, CANON[i], "role {i} drifted from the mod.fonts scale");
        }
    }
}
