use crate::canvas::viewport::{Camera, ViewportSnapshot};
use makepad_widgets::*;

const FONT_RASTER_SIZES: &[f32] = &[
    32.0, 40.0, 50.0, 63.0, 79.0, 99.0, 124.0, 155.0, 194.0, 243.0, 304.0,
];

pub(in crate::canvas) fn font_raster_size(target_size: f32) -> f32 {
    // Below the ladder, snap to whole points instead of passing the zoomed size
    // through raw. A continuous size means every zoom value asks the atlas for
    // a font size it has never rasterized -- and card body text lives entirely
    // in this range, so a zoom notch (or any frame of a zoom glide) paid a full
    // re-rasterization of every glyph on screen. Whole-point rungs bound that to
    // 28 cached sizes, and the <=0.5pt remainder rides `font_scale` as usual.
    if target_size <= FONT_RASTER_SIZES[0] {
        return target_size.max(4.0).round();
    }

    FONT_RASTER_SIZES
        .iter()
        .copied()
        .min_by(|a, b| {
            (target_size - *a)
                .abs()
                .total_cmp(&(target_size - *b).abs())
                .then_with(|| b.total_cmp(a))
        })
        .unwrap_or(target_size)
}

pub(in crate::canvas) fn fill_rect(cx: &mut Cx2d, rule: &mut DrawColor, rect: Rect, color: Vec4) {
    if rect.size.x <= 0.5 || rect.size.y <= 0.5 {
        return;
    }
    rule.color = color;
    rule.draw_abs(cx, rect);
}

/// Snap one axis of a stroke band onto the DEVICE pixel grid: an integer
/// number of whole pixels, starting on a pixel boundary.
///
/// A CAD hairline is only crisp if it lands on the grid. `center`/`extent` are
/// in lpx, so a 2.0-lpx stem whose centre happens to fall mid-pixel smears
/// across three pixels at partial coverage -- reading LIGHTER and blurrier than
/// the identical stroke one lifeline over that happened to land on a boundary.
/// That per-element lottery is what makes constant-width linework look
/// inconsistent.
fn snap_band(center: f64, extent: f64, dpi: f64) -> (f64, f64) {
    let px = (extent * dpi).round().max(1.0);
    let start = (center * dpi - px * 0.5).round();
    (start / dpi, px / dpi)
}

/// The quad for an axis-aligned stroke from `a` to `b`, `thickness` lpx wide,
/// snapped to whole device pixels on both axes. A diagonal pair is treated as
/// whichever axis it runs longest along (routes and messages are orthogonal).
pub(in crate::canvas) fn stroke_quad(cx: &Cx2d, a: DVec2, b: DVec2, thickness: f64) -> Rect {
    let dpi = cx.current_dpi_factor();
    if (a.x - b.x).abs() >= (a.y - b.y).abs() {
        let (y, h) = snap_band((a.y + b.y) * 0.5, thickness, dpi);
        let (x0, x1) = (a.x.min(b.x) * dpi, a.x.max(b.x) * dpi);
        let (x0, x1) = (x0.round(), x1.round());
        Rect {
            pos: dvec2(x0 / dpi, y),
            size: dvec2(((x1 - x0).max(1.0)) / dpi, h),
        }
    } else {
        let (x, w) = snap_band((a.x + b.x) * 0.5, thickness, dpi);
        let (y0, y1) = (a.y.min(b.y) * dpi, a.y.max(b.y) * dpi);
        let (y0, y1) = (y0.round(), y1.round());
        Rect {
            pos: dvec2(x, y0 / dpi),
            size: dvec2(w, ((y1 - y0).max(1.0)) / dpi),
        }
    }
}

/// Pull a rect's edges onto the device pixel grid, for a shape whose OUTLINE is
/// stroked by an SDF pen rather than drawn as quads (`stroke_quad` cannot help
/// there). Same reason: a border on a fractional edge renders as two dim rows
/// instead of one solid one.
pub(in crate::canvas) fn snap_rect(cx: &Cx2d, rect: Rect) -> Rect {
    let dpi = cx.current_dpi_factor();
    let x0 = (rect.pos.x * dpi).round();
    let y0 = (rect.pos.y * dpi).round();
    let x1 = ((rect.pos.x + rect.size.x) * dpi).round().max(x0 + 1.0);
    let y1 = ((rect.pos.y + rect.size.y) * dpi).round().max(y0 + 1.0);
    Rect {
        pos: dvec2(x0 / dpi, y0 / dpi),
        size: dvec2((x1 - x0) / dpi, (y1 - y0) / dpi),
    }
}

/// A stroke width in whole device pixels, so an SDF pen's `stroke_w` cannot
/// land on a half pixel.
pub(in crate::canvas) fn snap_stroke_width(cx: &Cx2d, thickness: f64) -> f64 {
    let dpi = cx.current_dpi_factor();
    (thickness * dpi).round().max(1.0) / dpi
}

pub(in crate::canvas) fn world_rect_to_screen(
    viewport: ViewportSnapshot,
    rect: waml::solve::Rect,
) -> Rect {
    let (lx, ly) = viewport.camera.world_to_local(rect.x, rect.y);
    Rect {
        pos: dvec2(viewport.view_rect.pos.x + lx, viewport.view_rect.pos.y + ly),
        size: dvec2(rect.w * viewport.camera.zoom, rect.h * viewport.camera.zoom),
    }
}

pub(in crate::canvas) fn edge_point_to_screen(
    camera: &Camera,
    rect_pos: DVec2,
    p: (f64, f64),
) -> DVec2 {
    let (lx, ly) = camera.world_to_local(p.0, p.1);
    dvec2(rect_pos.x + lx, rect_pos.y + ly)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever subpixel phase a stroke arrives at, it must leave on the grid:
    /// a whole number of device pixels starting on a boundary. Without this two
    /// identical 2.0-lpx strokes render at different apparent weights depending
    /// on where their element happened to land.
    #[test]
    fn a_snapped_band_lands_on_whole_device_pixels() {
        for dpi in [1.0, 1.5, 2.0] {
            for center in [10.0, 10.2, 10.5, 10.7, 11.0] {
                for extent in [1.0, 1.4, 2.0, 3.0] {
                    let (start, size) = snap_band(center, extent, dpi);
                    let start_px = start * dpi;
                    let size_px = size * dpi;
                    assert!((start_px - start_px.round()).abs() <= 1e-9);
                    assert!((size_px - size_px.round()).abs() <= 1e-9);
                    assert!(size_px >= 1.0, "a stroke may never round away entirely");
                    // Still centred on the band it was asked for, to within the
                    // half pixel the snap is allowed to move it.
                    assert!((start + size * 0.5 - center).abs() <= 0.5 / dpi + 1e-9);
                }
            }
        }
    }

    #[test]
    fn font_raster_size_snaps_small_text_to_whole_points() {
        assert_eq!(font_raster_size(4.0), 4.0);
        assert_eq!(font_raster_size(17.25), 17.0);
        assert_eq!(font_raster_size(17.75), 18.0);
        assert_eq!(font_raster_size(32.0), 32.0);
    }

    #[test]
    fn font_raster_size_never_falls_below_the_four_point_floor() {
        assert_eq!(font_raster_size(0.0), 4.0);
        assert_eq!(font_raster_size(1.2), 4.0);
    }

    #[test]
    fn a_zoom_sweep_lands_on_a_bounded_set_of_raster_sizes() {
        // The point of the ladder: a continuous zoom range must not produce a
        // continuous set of font sizes, or every frame re-rasterizes.
        let mut zoom = 0.05_f32;
        let mut sizes = std::collections::BTreeSet::new();
        while zoom <= 20.0 {
            for base_pt in [10.0_f32, 12.0, 14.0] {
                sizes.insert(font_raster_size(base_pt * zoom).to_bits());
            }
            zoom *= 1.01;
        }
        assert!(
            sizes.len() < 64,
            "a full zoom sweep should reuse rungs, got {} distinct sizes",
            sizes.len()
        );
    }

    #[test]
    fn font_raster_size_selects_the_nearest_ladder_rung() {
        assert_eq!(font_raster_size(33.0), 32.0);
        assert_eq!(font_raster_size(39.0), 40.0);
        assert_eq!(font_raster_size(61.0), 63.0);
        assert_eq!(font_raster_size(100.0), 99.0);
    }

    #[test]
    fn font_raster_size_resolves_midpoints_upward_and_caps_at_the_largest_rung() {
        assert_eq!(font_raster_size(36.0), 40.0);
        assert_eq!(font_raster_size(44.0), 40.0);
        assert_eq!(font_raster_size(45.0), 50.0);
        assert_eq!(font_raster_size(400.0), 304.0);
    }
}
