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
        // Full text_style (font_family included) -- a partial/color-only
        // override renders nothing (see canvas.rs).
        draw_label +: { text_style: theme.font_regular{ font_size: 9.0 } }
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
    draw_label: DrawText,
    #[live]
    icons: TreeIcons,
    /// Toggled by Space; swaps the backdrop so glyph contrast is checked on
    /// both Atlas modes without a rebuild.
    #[rust]
    dark: bool,
    /// Mouse-wheel scroll offset (the grid is taller than the window once the
    /// full Lucide set is present), plus the clamp bound recomputed each draw.
    #[rust]
    scroll_y: f64,
    #[rust]
    max_scroll: f64,
}

/// Real display sizes to prove per icon. The tree/doc-tabs draw at 14px; the
/// neighbours flank it so hinting drift across sizes is visible at a glance.
const SIZES: [f64; 3] = [14.0, 16.0, 20.0];
const ROW_H: f64 = 98.0;
const ZOOM: f64 = 72.0;
const PAD: f64 = 28.0;
/// Horizontal stride between the two icon columns.
const COL_W: f64 = 380.0;

impl Widget for IconGrid {
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_bg.draw_abs(cx, rect);

        // Three columns; the grid is taller than the window once the full
        // Lucide set is present, so it scrolls (mouse wheel, see handle_event).
        let all = self.icons.labeled_mut();
        let per_col = all.len().div_ceil(3);
        let content_h = 2.0 * PAD + per_col as f64 * ROW_H;
        self.max_scroll = (content_h - rect.size.y).max(0.0);
        self.scroll_y = self.scroll_y.clamp(0.0, self.max_scroll);

        let ox = (rect.pos.x + PAD).round();
        let oy = (rect.pos.y + PAD - self.scroll_y).round();

        // Label ink flipped for contrast against whichever backdrop is active.
        self.draw_label.color = if self.dark {
            vec4(0.66, 0.74, 0.82, 1.0)
        } else {
            vec4(0.34, 0.41, 0.49, 1.0)
        };
        for (i, (name, icon)) in all.into_iter().enumerate() {
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
            // Icon name under the small-size band.
            self.draw_label
                .draw_abs(cx, dvec2(col_x, (row_top + ZOOM + 6.0).round()), name);
        }
        DrawStep::done()
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        match event {
            Event::KeyDown(ke) if ke.key_code == KeyCode::Space => {
                self.dark = !self.dark;
                self.draw_bg.color = if self.dark {
                    vec4(0.055, 0.078, 0.11, 1.0)
                } else {
                    vec4(1.0, 1.0, 1.0, 1.0)
                };
                self.draw_bg.redraw(cx);
            }
            Event::Scroll(e) => {
                let prev = self.scroll_y;
                self.scroll_y = (self.scroll_y + e.scroll.y).clamp(0.0, self.max_scroll);
                if self.scroll_y != prev {
                    self.draw_bg.redraw(cx);
                }
            }
            _ => {}
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
