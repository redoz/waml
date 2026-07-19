//! `WamlButton`: the reusable Atlas HUD action button. A white vertical-gradient
//! fill ringed by the same source-bright accent frame as `AccentFrame`, thickening
//! on hover, with a press "ripple" that wipes a thick accent border out from the
//! click origin plus a brief glow flare.
//!
//! Immediate-mode *component*, same convention as `tool_dock`/`start_screen`:
//! the parent owns layout + hit-testing. It positions the button with `draw_at`,
//! then drives the press animation with `press`/`tick`/`release`. The shader
//! lives here (moved from `frame`); the frame material still matches
//! `AccentFrame`. The `Widget` impl is intentionally event-passive -- parents call
//! the inherent methods, so a stray tree route can never double-fire a press.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*

    // The Atlas HUD button (see `docs/design/hud-button-mock.html`): a white
    // vertical-gradient fill ringed by the same source-bright frame as `AccentFrame`.
    // Sharp corners (`sdf.rect`). Animated knobs are per-draw uniforms, driven by
    // the caller via `set_uniform` (same as canvas `draw_node`'s `zoom`):
    //   hover  0..1  -- frame brightens/thickens on pointer hover
    //   flare  0..1  -- press "glow" flash: fill lifts briefly toward the accent
    //   reveal 0..1  -- press ripple: a thick solid-accent border wipes in
    //                   radially from the click origin (`ox`,`oy`), the mock's
    //                   clip-path reveal.
    mod.draw.WamlButton = mod.draw.DrawColor{
        accent: uniform(atlas.accent)
        border_hi: uniform(atlas.frame_hi)
        border_lo: uniform(atlas.frame_lo)
        // Fill gradient stops. Light: field_bg #xffffff -> surface faint-cool,
        // i.e. the mock's white -> .74. Dark: lifted slate -> slate, so the
        // button reads as dark glass instead of a white slab.
        fill_hi: uniform(atlas.field_bg)
        fill_lo: uniform(atlas.surface)
        hover: uniform(0.0)
        flare: uniform(0.0)
        reveal: uniform(0.0)
        ox: uniform(0.5)
        oy: uniform(0.5)
        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let inset = 1.5
            let w = self.rect_size.x - inset * 2.0
            let h = self.rect_size.y - inset * 2.0
            // Vertical fill from the two themed stops (mock: white .92 -> .74);
            // the press flare lifts it briefly toward a bright accent tint.
            let fill = mix(mix(self.fill_hi, self.fill_lo, self.pos.y), mix(self.fill_hi, self.accent, 0.14), self.flare * 0.5)
            sdf.rect(inset, inset, w, h)
            sdf.fill_keep(fill)
            // Rest frame: the source-bright 150deg fade, thickening on hover.
            let dir = vec2(0.5, 0.8660254)
            let span = 1.3660254
            let t = clamp((self.pos.x * dir.x + self.pos.y * dir.y) / span, 0.0, 1.0)
            sdf.stroke(mix(self.border_hi, self.border_lo, t), mix(1.2, 1.8, self.hover))
            // Press ripple: thick solid-accent border, masked to the growing
            // radius around the click origin.
            let d = length(self.pos - vec2(self.ox, self.oy))
            let wipe = 1.0 - step(self.reveal * 1.7, d)
            sdf.rect(inset, inset, w, h)
            sdf.stroke(vec4(self.accent.x, self.accent.y, self.accent.z, wipe), 2.6)
            return sdf.result
        }
    }

    mod.widgets.WamlButtonBase = #(WamlButton::register_widget(vm))

    mod.widgets.WamlButton = set_type_default() do mod.widgets.WamlButtonBase{
        width: Fill
        height: Fill
        // Transparent `color` -- the shader paints the whole fill.
        draw_bg: mod.draw.WamlButton{ color: #x00000000 }
        // Uppercased at the call site (mock uses uppercase letterspaced mono; we
        // reuse the shared bold sans to stay clear of the fork's inline-font
        // empty-family quirk).
        draw_label +: {
            color: atlas.text
            text_style: theme.font_bold{font_size: 11 line_spacing: 1.2}
        }
    }
}

// Press ripple/flare timings (seconds): the accent border wipes in over
// `REVEAL_SECS`; the glow flare decays over `FLARE_SECS`.
const REVEAL_SECS: f64 = 0.14;
const FLARE_SECS: f64 = 0.45;
// Label placement inside the button: inset from the left edge, vertically
// centered (the `8.0` is half the ~16px cap height of the label font).
const LABEL_PAD_X: f64 = 16.0;
const LABEL_HALF_H: f64 = 8.0;

#[derive(Script, ScriptHook, Widget)]
pub struct WamlButton {
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
    #[redraw]
    #[live]
    draw_label: DrawText,

    // Press ripple/flare state (see module docs). `pressed` gates the next-frame
    // loop; `org` is the click point in 0..1 button space; `reveal`/`flare` are
    // the animated uniforms fed to the shader.
    #[rust]
    pressed: bool,
    #[rust]
    org: DVec2,
    #[rust]
    start: f64,
    #[rust]
    reveal: f32,
    #[rust]
    flare: f32,
    #[rust]
    next_frame: NextFrame,
}

impl Widget for WamlButton {
    // Event-passive: parents drive this component through the inherent methods
    // below (`press`/`tick`/`release`/`draw_at`), so a stray tree route can
    // never double-handle a press.
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    // Provided for completeness (DSL-tree placement): draw filling the walk
    // rect, labelless. Immediate-mode parents call `draw_at` instead.
    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let rect = cx.walk_turtle(walk);
        self.draw_at(cx, rect, "", false);
        DrawStep::done()
    }
}

impl WamlButton {
    /// Begin the press ripple from `abs` (an absolute pointer position) within
    /// `rect` (the button's last-drawn bounds). Starts a next-frame loop; call
    /// `tick` each frame and `release` on FingerUp.
    pub fn press(&mut self, cx: &mut Cx, rect: Rect, abs: DVec2, time: f64) {
        self.pressed = true;
        self.org = dvec2(
            ((abs.x - rect.pos.x) / rect.size.x).clamp(0.0, 1.0),
            ((abs.y - rect.pos.y) / rect.size.y).clamp(0.0, 1.0),
        );
        self.start = time;
        self.reveal = 0.0;
        self.flare = 1.0;
        self.next_frame = cx.new_next_frame();
        self.draw_bg.redraw(cx);
    }

    /// Advance the ripple/flare if `event` is our scheduled next frame. Returns
    /// true when it consumed the frame (the parent can stop there).
    pub fn tick(&mut self, cx: &mut Cx, event: &Event) -> bool {
        if let Some(ne) = self.next_frame.is_event(event) {
            if self.pressed {
                let elapsed = ne.time - self.start;
                self.reveal = (elapsed / REVEAL_SECS).clamp(0.0, 1.0) as f32;
                self.flare = (1.0 - (elapsed / FLARE_SECS).clamp(0.0, 1.0)) as f32;
                self.next_frame = cx.new_next_frame();
                self.draw_bg.redraw(cx);
                return true;
            }
        }
        false
    }

    /// End the press (FingerUp / cancel), clearing the ripple. Returns true if a
    /// press was actually in progress.
    pub fn release(&mut self, cx: &mut Cx) -> bool {
        if self.pressed {
            self.pressed = false;
            self.reveal = 0.0;
            self.flare = 0.0;
            self.draw_bg.redraw(cx);
            true
        } else {
            false
        }
    }

    /// Draw the button filling `rect` with `label` (drawn as given -- the caller
    /// uppercases). `hovered` thickens the accent frame.
    pub fn draw_at(&mut self, cx: &mut Cx2d, rect: Rect, label: &str, hovered: bool) {
        self.draw_bg.set_uniform(cx, live_id!(hover), &[if hovered { 1.0 } else { 0.0 }]);
        self.draw_bg.set_uniform(cx, live_id!(reveal), &[self.reveal]);
        self.draw_bg.set_uniform(cx, live_id!(flare), &[self.flare]);
        self.draw_bg.set_uniform(cx, live_id!(ox), &[self.org.x as f32]);
        self.draw_bg.set_uniform(cx, live_id!(oy), &[self.org.y as f32]);
        self.draw_bg.draw_abs(cx, rect);
        self.draw_label.draw_abs(
            cx,
            dvec2(rect.pos.x + LABEL_PAD_X, rect.pos.y + rect.size.y * 0.5 - LABEL_HALF_H),
            label,
        );
    }
}
