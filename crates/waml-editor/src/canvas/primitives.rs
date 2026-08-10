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
