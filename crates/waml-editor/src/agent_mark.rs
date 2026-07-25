//! Per-window agent marker: a right-floated pill in the title row, so several
//! concurrently-running `waml-editor` windows can be told apart by eye.
//! Mounted zero-width as the first child of `title_row`; drawing is purely
//! optional (both badge and tint are `None` unless `--title`/`--color` were
//! passed on the command line).
//!
//! The pill is the ONLY mark. A first cut also washed the whole title row 15%
//! toward the tint; that read as a broken theme rather than as a marker, so the
//! row keeps its plain `atlas.field_bg` and `--color` now only fills the pill.

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
        chip_fallback: atlas.selection
        ink_fallback: atlas.text
        draw_chip +: {
            color: #0000
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 2.5)
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

/// Chip padding and its gap from the row's right edge.
const CHIP_PAD_X: f64 = 6.0;
const CHIP_PAD_Y: f64 = 3.0;
const CHIP_RIGHT_GAP: f64 = 6.0;

/// Inner width of a `--color`-only pill, which has no text to size it. Keeps
/// that flag useful on its own now that the title-row wash is gone.
const SWATCH_W: f64 = 14.0;

/// Reference string for the pill's VERTICAL band: a cap and a descender. The
/// band is measured from this rather than from the badge text, so every
/// window's pill is the same height whatever letters its badge happens to use,
/// and a `--color`-only swatch matches a titled pill exactly.
const BAND_REF: &str = "Hg";

/// The glyph band as a fraction of the font size, above and below the
/// baseline: cap height and lowercase descender depth for IBM Plex Sans.
///
/// The laid-out LINE BOX is NOT usable here. For `fonts.text_caption` it
/// measures 16lpx while the glyphs occupy only the bottom 10 of it (the role's
/// `asc`/`desc` em fudges pad the box and push the glyphs down inside it), so
/// sizing the pill on the box gave a 22px pill with 10px of dead space above
/// the word and 2px below. Deriving the band from the baseline instead makes
/// the pill hug the word and puts the word in its middle.
const CAP_RATIO: f64 = 0.72;
const DESC_RATIO: f64 = 0.21;

/// Pick black or white ink for a chip of colour `fill`, by Rec.709 luma.
pub fn label_ink(fill: Vec4) -> Vec4 {
    let luma = 0.2126 * fill.x + 0.7152 * fill.y + 0.0722 * fill.z;
    if luma > 0.55 {
        vec4(0.06, 0.08, 0.11, 1.0)
    } else {
        vec4(0.98, 0.98, 0.99, 1.0)
    }
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
    draw_chip: DrawColor,
    #[redraw]
    #[live]
    draw_label: DrawText,

    #[live]
    chip_fallback: Vec4,
    #[live]
    ink_fallback: Vec4,

    /// Badge text from `--title`; `None` draws nothing.
    #[rust]
    badge: Option<String>,
    /// Tint from `--color`; `None` falls back to `chip_fallback`.
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

        let text = self.badge.clone().unwrap_or_default();

        // Vertical band, from the reference string's baseline. `band_top` is
        // measured DOWN from the text box's top-left, which is what
        // `DrawText::draw_abs` takes, so seating the text is
        // `chip_top + pad - band_top`.
        let refbox = self
            .draw_label
            .layout(cx, 0.0, 0.0, None, false, Align::default(), BAND_REF);
        let (band_top, band_h) = match refbox.rows.first().and_then(|r| {
            r.glyphs
                .first()
                .map(|g| (r.origin_in_lpxs.y as f64, g.font_size_in_lpxs as f64))
        }) {
            Some((baseline, font_size)) => (
                baseline - CAP_RATIO * font_size,
                (CAP_RATIO + DESC_RATIO) * font_size,
            ),
            // No glyphs laid out (font not yet loaded): fall back to the line
            // box rather than drawing a zero-height pill.
            None => (0.0, refbox.size_in_lpxs.height as f64),
        };

        let inner_w = if text.is_empty() {
            SWATCH_W
        } else {
            self.draw_label
                .layout(cx, 0.0, 0.0, None, false, Align::default(), &text)
                .size_in_lpxs
                .width as f64
        };

        let chip_w = inner_w + CHIP_PAD_X * 2.0;
        let chip_h = band_h + CHIP_PAD_Y * 2.0;
        let chip = Rect {
            pos: dvec2(
                row.pos.x + row.size.x - chip_w - CHIP_RIGHT_GAP,
                // Centred in the row. The chip is sized from the glyph band, so
                // centring the chip centres the word with it -- which is what
                // makes the pill read as seated rather than floating high.
                (row.pos.y + (row.size.y - chip_h) * 0.5).round(),
            ),
            size: dvec2(chip_w, chip_h),
        };

        let fill = self.tint.unwrap_or(self.chip_fallback);
        self.draw_chip.color = fill;
        self.draw_chip.draw_abs(cx, chip);

        if !text.is_empty() {
            self.draw_label.color = if self.tint.is_some() {
                label_ink(fill)
            } else {
                self.ink_fallback
            };
            self.draw_label.draw_abs(
                cx,
                dvec2(
                    chip.pos.x + CHIP_PAD_X,
                    (chip.pos.y + CHIP_PAD_Y - band_top).round(),
                ),
                &text,
            );
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

}
