//! Dev-only harness for authoring the tree/doc-tab kind glyphs (`icons.rs`).
//!
//! Renders every icon at a few real display sizes plus a large zoom cell, on
//! the Atlas surface, with a light/dark toggle (Space). It pulls the SHARED
//! shader source via `#[path]` (no lib split), so editing `icons.rs` while this
//! is running hot-reloads the DSL and the glyphs update live.
//!
//! Run: `cargo run -p waml-editor --bin icon_harness`
//! Not wired into the shipping editor.

use makepad_widgets::*;

#[path = "../theme_atlas.rs"]
mod theme_atlas;
#[path = "../icons.rs"]
mod icons;

use icons::TreeIcons;

script_mod! {
    use mod.prelude.widgets.*
    use mod.atlas

    mod.widgets.IconGridBase = #(IconGrid::register_widget(vm))

    mod.widgets.IconGrid = set_type_default() do mod.widgets.IconGridBase{
        width: Fill
        height: Fill
        draw_bg +: { color: atlas.field_bg }
    }

    startup() do #(IconHarness::script_component(vm)){
        ui: Root{
            Window{
                window.inner_size: vec2(1180, 800)
                window.title: "waml icon harness"
                body +: {
                    mod.widgets.IconGrid{}
                }
            }
        }
    }
}

/// The icon proof-grid widget: one row per glyph, several real sizes plus a
/// zoom cell. Draws everything absolutely from the walked rect.
#[derive(Script, ScriptHook, Widget)]
pub struct IconGrid {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[redraw]
    #[live]
    draw_bg: DrawColor,
    #[live]
    icons: TreeIcons,
    /// Toggled by Space; swaps the backdrop so glyph contrast is checked on
    /// both Atlas modes without a rebuild.
    #[rust]
    dark: bool,
}

/// Real display sizes to prove per icon. The tree/doc-tabs draw at 14px; the
/// neighbours flank it so hinting drift across sizes is visible at a glance.
const SIZES: [f64; 3] = [14.0, 16.0, 20.0];
const ROW_H: f64 = 88.0;
const ZOOM: f64 = 72.0;
const PAD: f64 = 28.0;
/// Horizontal stride between the two icon columns.
const COL_W: f64 = 380.0;

impl Widget for IconGrid {
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);

        let ox = (rect.pos.x + PAD).round();
        let oy = (rect.pos.y + PAD).round();

        // Three columns so the (now 23) glyphs fit a reasonable window height.
        let all = self.icons.labeled_mut();
        let per_col = all.len().div_ceil(3);
        for (i, (_name, icon)) in all.into_iter().enumerate() {
            let col = i / per_col;
            let row = i % per_col;
            let col_x = (ox + col as f64 * COL_W).round();
            let zoom_x = (col_x + 220.0).round();
            let row_top = oy + row as f64 * ROW_H;
            // Small sizes: baseline-aligned along the top band of the row.
            let mut x = col_x;
            for &sz in SIZES.iter() {
                let y = (row_top + (ZOOM - sz) * 0.5).round();
                icon.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(x.round(), y),
                        size: dvec2(sz, sz),
                    },
                );
                x += 44.0;
            }
            // Zoom cell.
            icon.draw_abs(
                cx,
                Rect {
                    pos: dvec2(zoom_x, row_top.round()),
                    size: dvec2(ZOOM, ZOOM),
                },
            );
        }
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if let Event::KeyDown(ke) = event {
            if ke.key_code == KeyCode::Space {
                self.dark = !self.dark;
                self.draw_bg.color = if self.dark {
                    vec4(0.055, 0.078, 0.11, 1.0)
                } else {
                    vec4(1.0, 1.0, 1.0, 1.0)
                };
                self.draw_bg.redraw(cx);
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct IconHarness {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for IconHarness {}

impl AppMain for IconHarness {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        crate::icons::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

app_main!(IconHarness);
