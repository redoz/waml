//! SVG-vs-SDF glyph compare harness (see `docs/design/svg-sdf-harness.md`).
//!
//! ONE icon (`pin`), three columns, sizes 56/24/18/16/14 px stacked:
//!  - Col A: the Lucide SVG (`pin.svg`) via `DrawSvg`, on a transparency checker.
//!  - Col B: the `icons.rs` SDF (`IconPin`), on the same checker.
//!  - Col C: overlay diff -- SVG red + SDF blue on a dark tile, so agreement
//!    reads purple, SVG-only red, SDF-only blue (an impartial judge).
//!
//! Note: `DrawSvg` renders the Lucide *stroke* paths as a solid fill, while the
//! SDF is authored as a thin stroke outline -- the overlay honestly exposes that
//! fill-vs-outline gap.
//!
//! Run: `cargo run -p waml-editor --bin logo_harness`
//! No hot-reload in bare `cargo run` -- edit `icons.rs`, rebuild, relaunch.
//! Shader errors surface at GPU runtime in stdout as `[E] ...icons.rs:LINE`.

use makepad_widgets::*;

// Pulled in by path (the editor crate has no lib target).
#[path = "../logo.rs"]
mod logo;
#[path = "../theme_atlas.rs"]
mod theme_atlas;
#[path = "../icons.rs"]
mod icons;

use icons::IconSet;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*
    use mod.draw
    use mod.atlas

    // Transparency checkerboard behind the SVG / SDF columns. Fixed 8px cell so
    // both columns share one grid regardless of glyph size.
    mod.draw.CheckerTile = mod.draw.DrawColor{
        pixel: fn() {
            let px = self.pos * self.rect_size
            let cell = 8.0
            let g = mod(floor(px.x / cell) + floor(px.y / cell), 2.0)
            return vec4(mix(vec3(0.87), vec3(0.72), g), 1.0)
        }
    }
    // Dark tile behind the overlay column (so additive-ish red/blue read).
    mod.draw.DarkTile = mod.draw.DrawColor{
        pixel: fn() { return vec4(0.10, 0.10, 0.13, 1.0) }
    }
    // Solid chip painted on the exact glyph rect in col B, so the icon's own
    // rect_size (the bounds the SDF is drawn into) is visible behind it.
    mod.draw.IconRectTile = mod.draw.DrawColor{
        pixel: fn() { return vec4(0.16, 0.17, 0.20, 1.0) }
    }

    mod.widgets.CompareProbeBase = #(CompareProbe::register_widget(vm))
    mod.widgets.CompareProbe = set_type_default() do mod.widgets.CompareProbeBase{
        width: Fill
        height: Fill
        draw_check: mod.draw.CheckerTile{}
        draw_dark: mod.draw.DarkTile{}
        draw_iconbg: mod.draw.IconRectTile{}
        draw_svg: mod.draw.DrawSvg{ svg: crate_resource("self:resources/icons/moon.svg") }
    }

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                pass.clear_color: vec4(0.90, 0.90, 0.90, 1.0)
                window.inner_size: vec2(760, 640)
                window.title: "moon: SVG vs SDF vs diff"
                body +: {
                    padding: 32
                    flow: Down
                    mod.widgets.CompareProbe{}
                }
            }
        }
    }
}

/// Display sizes (px), stacked top-to-bottom in every column. First row is a
/// large zoom so shader detail reads; the rest are real display sizes.
const SIZES: [f64; 5] = [150.0, 24.0, 18.0, 16.0, 14.0];
/// Padding around each glyph inside its background tile.
const PAD: f64 = 8.0;
const ROW_GAP: f64 = 16.0;
const COL_GAP: f64 = 190.0;

// Atlas accent (#1496dc) -- the SDF's normal tint in column B.
const ACCENT: Vec4f = Vec4f { x: 0.078, y: 0.588, z: 0.863, w: 1.0 };
// Overlay tints (column C). Semi-transparent so overlap blends toward purple.
const OVL_SVG: Vec4f = Vec4f { x: 0.92, y: 0.20, z: 0.22, w: 0.62 };
const OVL_SDF: Vec4f = Vec4f { x: 0.28, y: 0.48, z: 1.0, w: 0.62 };
// Column A svg tint (near-black on the light checker).
const INK: Vec4f = Vec4f { x: 0.12, y: 0.12, z: 0.12, w: 1.0 };

/// Draws `pin` three ways per size row: SVG (A), SDF (B), overlay diff (C).
#[derive(Script, ScriptHook, Widget)]
pub struct CompareProbe {
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
    draw_check: DrawColor,
    #[live]
    draw_dark: DrawColor,
    #[live]
    draw_iconbg: DrawColor,
    #[live]
    draw_svg: DrawSvg,
    #[live]
    icons: IconSet,
}

impl CompareProbe {
    /// Tile rect for column `col` (0..3), row origin `y`, glyph size `sz`.
    /// Tile hugs the glyph (sz + padding) so big/small rows stay compact.
    fn tile(&self, ox: f64, col: f64, y: f64, sz: f64) -> Rect {
        let t = sz + 2.0 * PAD;
        Rect {
            pos: dvec2((ox + col * COL_GAP).round(), y.round()),
            size: dvec2(t, t),
        }
    }
    /// Glyph rect inset by `PAD` inside a tile -- identical across columns so
    /// A/B/C overlay pixel-for-pixel.
    fn glyph(tile: Rect, sz: f64) -> Rect {
        Rect {
            pos: dvec2(tile.pos.x + PAD, tile.pos.y + PAD),
            size: dvec2(sz, sz),
        }
    }
}

impl Widget for CompareProbe {
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        let ox = (rect.pos.x + 8.0).round();
        let mut y = (rect.pos.y + 8.0).round();

        for &sz in SIZES.iter() {
            // --- Col A: SVG on checker ---
            let a = self.tile(ox, 0.0, y, sz);
            self.draw_check.draw_abs(cx, a);
            self.draw_svg.color = INK;
            self.draw_svg.draw_abs(cx, Self::glyph(a, sz));

            // --- Col B: SDF on a solid chip sized to the glyph rect ---
            let b = self.tile(ox, 1.0, y, sz);
            self.draw_check.draw_abs(cx, b);
            self.draw_iconbg.draw_abs(cx, Self::glyph(b, sz));
            self.icons.moon.color = ACCENT;
            self.icons.moon.draw_abs(cx, Self::glyph(b, sz));

            // --- Col C: overlay diff on dark ---
            let c = self.tile(ox, 2.0, y, sz);
            self.draw_dark.draw_abs(cx, c);
            let g = Self::glyph(c, sz);
            self.draw_svg.color = OVL_SVG;
            self.draw_svg.draw_abs(cx, g);
            self.icons.moon.color = OVL_SDF;
            self.icons.moon.draw_abs(cx, g);

            y += sz + 2.0 * PAD + ROW_GAP;
        }
        DrawStep::done()
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    // First-frame kick: the SDF DrawQuad bg doesn't paint until its area is
    // invalidated, so force one redraw once the UI is up.
    #[rust]
    kick: NextFrame,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.kick = cx.new_next_frame();
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        crate::theme_atlas::script_mod(vm);
        crate::logo::script_mod(vm);
        crate::icons::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if self.kick.is_event(event).is_some() {
            self.ui.redraw(cx);
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
