//! Stateless row-drawing helpers for the tree panel. Each function takes the
//! row's absolute `Rect` (from `TreeLayout` once Task 6 lands, or built by the
//! caller in the meantime) rather than reaching into `Cx` for geometry, so the
//! pure formulas here stay unit-testable without a `Cx`.

use crate::icons::{Icon, IconSet};
use crate::tree_layout::ROW_HEIGHT;
use crate::tree_panel::DrawChevron;
use makepad_widgets::*;

const ICON_SIZE: f64 = 14.0;
const ICON_LEFT_MARGIN: f64 = 20.0;
// The fold-chevron box's size and left margin live in `tree_layout`, which
// positions it: drawing reads that rect rather than keeping a second copy of
// the numbers here.
/// Per-depth x step, shared by the glyph column and the label. The fork's
/// label step was `indent_width` (10.0 in its DSL) PLUS the per-depth margins
/// its `indent_walk` tacked on (`left: depth*1.0`, `right: depth*4.0`), so the
/// visible step was 15px, not 10. Both columns step by that one number here:
/// give them different steps and the gap between glyph and text drifts 5px per
/// level, until around depth 3 the label lands on top of the glyph.
const DEPTH_INDENT: f64 = 15.0;

/// Left edge of a row's glyph and of its label, at depth 0. The fork placed
/// labels at `padding.left: 38.0`; reproduce that so the two columns sit
/// exactly where they always did.
const ICON_LEFT: f64 = ICON_LEFT_MARGIN;
pub const LABEL_LEFT: f64 = 38.0;

fn indent_x(row_x: f64, left: f64, depth: usize, scale: f64) -> f64 {
    row_x + (left + depth as f64 * DEPTH_INDENT) * scale
}

fn label_x(row_x: f64, depth: usize, scale: f64) -> f64 {
    indent_x(row_x, LABEL_LEFT, depth, scale)
}

/// Draw the provider-supplied row-leading glyph at `rect.pos`.
///
/// `scale` is the fold amount the core is drawing this row at (1.0
/// at rest, shrinking to 0 as an ancestor folder closes): the glyph shrinks and
/// fades with it, so the row's hand-drawn marks dissolve together with the
/// widget-drawn label rather than staying full-size over a collapsing row.
///
/// The draw position is rounded to whole device pixels before `draw_abs` so the
/// SDF glyph's thin strokes land pixel-aligned; a subpixel `x`/`y` would soften
/// them.
pub fn row_icon(
    cx: &mut Cx2d,
    icons: &mut IconSet,
    icon: Icon,
    rect: Rect,
    depth: usize,
    color: Vec4,
    scale: f64,
) {
    let size = ICON_SIZE * scale;
    // Scaled exactly like the label's x: mid-collapse both columns have to
    // squeeze toward the row's left edge at the same rate, or the glyph drifts
    // out from under its text for the length of the fold.
    let x = indent_x(rect.pos.x, ICON_LEFT, depth, scale).round();
    let y = (rect.pos.y + (ROW_HEIGHT * scale - size) / 2.0).round();
    icons.draw(
        cx,
        icon,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(size, size),
        },
        fade(color, scale),
    );
}

/// Draw the fold chevron into `rect`, rotated by `open` (0 collapsed / 1
/// expanded) and faded by `scale` (see `row_icon`).
///
/// `rect` is `TreeLayout::chevron_rect` verbatim -- the SAME rect the press
/// path hit-tests. Recomputing the position from the row here is what would
/// let the drawn chevron and its click target drift apart, which is the whole
/// reason the geometry lives in the core.
///
/// Only the origin is rounded, matching `row_icon`: the chevron is a 1.3px
/// stroke and a subpixel origin would smear it.
pub fn row_chevron(cx: &mut Cx2d, draw: &mut DrawChevron, rect: Rect, open: f32, scale: f64) {
    let chevron_rect = Rect {
        pos: dvec2(rect.pos.x.round(), rect.pos.y.round()),
        size: rect.size,
    };
    draw.open = open;
    draw.fade = scale as f32;
    draw.draw_abs(cx, chevron_rect);
}

/// Draw the degraded-chain marker: a small solid dot at the row's right edge,
/// for a directory row whose declared `view:` chain fell back to the root
/// view. Purely additive to `row_icon`/`row_chevron` -- no hit test reads this
/// rect, it is presentation only.
pub fn row_diag_marker(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, scale: f64) {
    let width = rect.size.x;
    if !width.is_finite() {
        return;
    }
    let size = 6.0 * scale;
    let x = (rect.pos.x + width - size - 10.0).round();
    let y = (rect.pos.y + (ROW_HEIGHT * scale - size) / 2.0).round();
    draw.draw_abs(
        cx,
        Rect {
            pos: dvec2(x, y),
            size: dvec2(size, size),
        },
    );
}

/// Paint the active-row highlight over the row at `rect.pos`, spanning the full
/// tree width and the row's current (folded) height. Translucent, so it drops
/// over the already-drawn row (bg + label) without hiding the text. Drawn before
/// the glyph so the icon stays on top.
///
/// Used for both the selection tint and the new hover tint -- hence the
/// neutral name rather than the fork's `draw_row_highlight`.
pub fn row_fill(cx: &mut Cx2d, draw: &mut DrawColor, rect: Rect, scale: f64) {
    let width = rect.size.x;
    if !width.is_finite() {
        return;
    }
    // The pen's colour is the theme's (or, for the reveal pulse, the caller's);
    // put it back after the fade so repeated rows don't compound the multiply.
    let color = draw.color;
    draw.color = fade(color, scale);
    draw.draw_abs(
        cx,
        Rect {
            pos: dvec2(rect.pos.x, rect.pos.y),
            size: dvec2(width, ROW_HEIGHT * scale),
        },
    );
    draw.color = color;
}

/// Draw a row's label. Previously the fork widget's job; ours now.
///
/// Vertically centred in the row's SCALED band and faded with it, so a label
/// dissolves with its row mid-collapse instead of standing at full ink.
///
pub fn row_label(
    cx: &mut Cx2d,
    draw: &mut DrawText,
    rect: Rect,
    depth: usize,
    text: &str,
    scale: f64,
) {
    let color = draw.color;
    draw.color = fade(color, scale);
    let size = draw
        .layout(cx, 0.0, 0.0, None, false, Align::default(), text)
        .size_in_lpxs;
    let x = label_x(rect.pos.x, depth, scale).round();
    let y = (rect.pos.y + (rect.size.y - size.height as f64) / 2.0).round();
    draw.draw_abs(cx, dvec2(x, y), text);
    draw.color = color;
}

/// `color` with its alpha scaled by the row's fold amount.
pub fn fade(color: Vec4, scale: f64) -> Vec4 {
    vec4(color.x, color.y, color.z, color.w * scale as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_scales_alpha_only() {
        let color = vec4(0.2, 0.4, 0.6, 0.8);
        let faded = fade(color, 0.5);
        assert_eq!((faded.x, faded.y, faded.z), (0.2, 0.4, 0.6));
        assert!((faded.w - 0.4).abs() < 1e-6);
    }

    #[test]
    fn label_starts_past_the_glyph_column() {
        // The fork sat labels at padding.left 38, stepping 15px per depth.
        assert_eq!(label_x(0.0, 0, 1.0), 38.0);
        assert_eq!(label_x(0.0, 2, 1.0), 68.0);
        // Mid-collapse the whole column shrinks with the row.
        assert_eq!(label_x(0.0, 0, 0.5), 19.0);
    }

    /// Regression: the glyph column stepped 15px per depth while the label
    /// stepped 10, so the gap closed 5px a level and the text landed on the
    /// glyph around depth 3.
    #[test]
    fn the_glyph_to_label_gap_is_the_same_at_every_depth() {
        let gap = |depth| label_x(0.0, depth, 1.0) - indent_x(0.0, ICON_LEFT, depth, 1.0);
        let at_root = gap(0);
        assert!(at_root > 0.0, "label sits right of the glyph");
        for depth in 1..8 {
            assert_eq!(gap(depth), at_root, "gap drifted by depth {depth}");
        }
    }

    #[test]
    fn both_columns_shrink_together_mid_fold() {
        for scale in [1.0, 0.6, 0.25] {
            let icon = indent_x(0.0, ICON_LEFT, 3, scale);
            let label = label_x(0.0, 3, scale);
            assert!(
                (label - icon - (LABEL_LEFT - ICON_LEFT) * scale).abs() < 1e-9,
                "columns diverged at scale {scale}"
            );
        }
    }
}
