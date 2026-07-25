//! Per-window agent marker: a faint colour wash across the title row plus a
//! right-floated badge chip, so several concurrently-running `waml-editor`
//! windows can be told apart by eye. Mounted zero-width as the first child of
//! `title_row`; drawing is purely optional (both badge and tint are `None`
//! unless `--title`/`--color` were passed on the command line).

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.AgentMarkBase = #(AgentMark::register_widget(vm))

    mod.widgets.AgentMark = set_type_default() do mod.widgets.AgentMarkBase{
        width: 0.0
        height: Fill
        wash_base: atlas.field_bg
        chip_fallback: atlas.selection
        ink_fallback: atlas.text
        draw_wash +: { color: #0000 }
        draw_chip +: {
            color: #0000
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 4.0)
                sdf.fill(self.color)
                return sdf.result
            }
        }
        draw_label +: {
            color: #FFF
            text_style: fonts.text_caption
        }
    }
}

/// Blend fraction for the title-row wash: subtle enough to leave burger/model
/// text at full legibility, strong enough to read at a glance.
const WASH_AMOUNT: f32 = 0.15;
const CHIP_PAD_X: f64 = 6.0;
const CHIP_PAD_Y: f64 = 2.0;
const CHIP_RIGHT_GAP: f64 = 6.0;

/// Pick black or white ink for a chip of colour `fill`, by Rec.709 luma.
pub fn label_ink(fill: Vec4) -> Vec4 {
    let luma = 0.2126 * fill.x + 0.7152 * fill.y + 0.0722 * fill.z;
    if luma > 0.55 {
        vec4(0.06, 0.08, 0.11, 1.0)
    } else {
        vec4(0.98, 0.98, 0.99, 1.0)
    }
}

/// Linearly blend `base` toward `tint` by `amount`; always fully opaque (this
/// replaces a chrome fill, not a translucent overlay).
pub fn wash(base: Vec4, tint: Vec4, amount: f32) -> Vec4 {
    vec4(
        base.x + (tint.x - base.x) * amount,
        base.y + (tint.y - base.y) * amount,
        base.z + (tint.z - base.z) * amount,
        1.0,
    )
}

#[derive(Script, ScriptHook, Widget)]
pub struct AgentMark {
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
    draw_wash: DrawColor,
    #[redraw]
    #[live]
    draw_chip: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,

    #[live]
    wash_base: Vec4,
    #[live]
    chip_fallback: Vec4,
    #[live]
    ink_fallback: Vec4,

    /// Badge text from `--title`; `None` draws nothing.
    #[rust]
    badge: Option<String>,
    /// Tint from `--color`; `None` draws nothing.
    #[rust]
    tint: Option<Vec4>,
    /// The title row's own width in pixels, pushed each frame by `App`
    /// (this widget is mounted zero-width, so it has no width of its own to
    /// draw across).
    #[rust]
    row_w: f64,
}

impl Widget for AgentMark {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
        // Inert: this widget must never claim clicks meant for the caption
        // row's siblings (menu button, model-name label, drag region).
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        let anchor = cx.walk_turtle(walk);

        if self.row_w <= 0.0 || (self.badge.is_none() && self.tint.is_none()) {
            return DrawStep::done();
        }

        let row = Rect {
            pos: anchor.pos,
            size: dvec2(self.row_w, anchor.size.y),
        };

        if let Some(tint) = self.tint {
            self.draw_wash.color = wash(self.wash_base, tint, WASH_AMOUNT);
            self.draw_wash.draw_abs(cx, row);
        }

        if let Some(text) = self.badge.clone() {
            let fill = self.tint.unwrap_or(self.chip_fallback);
            let text_w = self
                .draw_label
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &text)
                .size_in_lpxs
                .width as f64;
            let chip_w = text_w + CHIP_PAD_X * 2.0;
            let chip_h = row.size.y - CHIP_PAD_Y * 2.0;
            let chip = Rect {
                pos: dvec2(
                    row.pos.x + row.size.x - chip_w - CHIP_RIGHT_GAP,
                    row.pos.y + CHIP_PAD_Y,
                ),
                size: dvec2(chip_w, chip_h),
            };

            self.draw_chip.color = fill;
            self.draw_chip.draw_abs(cx, chip);

            self.draw_label.color = if self.tint.is_some() {
                label_ink(fill)
            } else {
                self.ink_fallback
            };
            let text_y = chip.pos.y + chip.size.y * 0.5 - 6.0;
            self.draw_label
                .draw_abs(cx, dvec2(chip.pos.x + CHIP_PAD_X, text_y), &text);
        }

        DrawStep::done()
    }
}

impl AgentMark {
    /// Push the parsed `--title`/`--color` values in; called at startup and
    /// again after theme reload (which wipes `#[rust]` fields).
    pub fn set_marks(&mut self, cx: &mut Cx, badge: Option<String>, tint: Option<Vec4>) {
        self.badge = badge;
        self.tint = tint;
        self.redraw(cx);
    }

    /// Push the title row's measured width; called every frame by `App`.
    pub fn set_row_width(&mut self, cx: &mut Cx, px: f64) {
        self.row_w = px;
        self.redraw(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_ink_is_near_white_on_a_dark_fill() {
        let ink = label_ink(vec4(0.05, 0.05, 0.10, 1.0));
        assert!(ink.x > 0.9 && ink.y > 0.9 && ink.z > 0.9, "got {ink:?}");
    }

    #[test]
    fn label_ink_is_near_black_on_a_light_fill() {
        let ink = label_ink(vec4(0.95, 0.95, 0.90, 1.0));
        assert!(ink.x < 0.2 && ink.y < 0.2 && ink.z < 0.2, "got {ink:?}");
    }

    #[test]
    fn label_ink_weights_green_over_blue() {
        // Pure green is perceptually bright (0.7152 luma) -> dark ink.
        // Pure blue is perceptually dark (0.0722 luma) -> light ink.
        assert!(label_ink(vec4(0.0, 1.0, 0.0, 1.0)).x < 0.5);
        assert!(label_ink(vec4(0.0, 0.0, 1.0, 1.0)).x > 0.5);
    }

    #[test]
    fn label_ink_is_always_opaque() {
        assert_eq!(label_ink(vec4(0.5, 0.5, 0.5, 1.0)).w, 1.0);
    }

    #[test]
    fn wash_at_zero_is_the_base() {
        let base = vec4(1.0, 1.0, 1.0, 1.0);
        let got = wash(base, vec4(1.0, 0.0, 0.0, 1.0), 0.0);
        assert!((got.x - 1.0).abs() < 1e-6);
        assert!((got.y - 1.0).abs() < 1e-6);
        assert!((got.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn wash_at_one_is_the_tint() {
        let got = wash(vec4(1.0, 1.0, 1.0, 1.0), vec4(0.2, 0.4, 0.6, 1.0), 1.0);
        assert!((got.x - 0.2).abs() < 1e-6);
        assert!((got.y - 0.4).abs() < 1e-6);
        assert!((got.z - 0.6).abs() < 1e-6);
    }

    #[test]
    fn wash_interpolates_linearly_and_stays_opaque() {
        let got = wash(vec4(1.0, 1.0, 1.0, 1.0), vec4(0.0, 0.0, 0.0, 1.0), 0.15);
        assert!((got.x - 0.85).abs() < 1e-6, "got {got:?}");
        assert_eq!(
            got.w, 1.0,
            "wash must stay opaque: it replaces a chrome fill"
        );
    }
}
