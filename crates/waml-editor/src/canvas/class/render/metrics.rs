//! Class-canvas linework. One policy, CAD: every value below is a constant in
//! SCREEN space, so linework holds its weight at any zoom
//! (`crate::canvas::linework`).
//!
//! Widths now come from `canvas::pen::Pen`; this type is a shrinking shim and
//! is deleted in the pen migration.

use crate::canvas::pen::Pen;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in super::super) struct LineworkMetrics {
    pub(super) frame_stroke_scale: f32,
    /// Always 1.0: the frame drops the zoom-driven compensations that only the
    /// non-canvas `AccentFrame` consumers want.
    pub(super) frame_screen_space: f32,
    pub(super) group_stroke_width: f32,
    pub(super) group_dash_period: f32,
    pub(super) divider_thickness: f64,
    pub(super) edge_thickness: f64,
    pub(super) marker_size: f64,
    pub(super) nub_size: f64,
}

impl LineworkMetrics {
    pub(in super::super) fn for_zoom(zoom: f64) -> Self {
        debug_assert!(zoom.is_finite() && zoom > 0.0);
        Self {
            frame_stroke_scale: (1.0 / zoom) as f32,
            frame_screen_space: 1.0,
            group_stroke_width: Pen::HAIRLINE.width() as f32,
            group_dash_period: 6.0,
            divider_thickness: Pen::HAIRLINE.width(),
            edge_thickness: Pen::REGULAR.width(),
            marker_size: 10.0,
            nub_size: 6.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::viewport::{MAX_ZOOM, MIN_ZOOM};

    #[test]
    fn metrics_are_screen_space_at_supported_zooms() {
        for zoom in [MIN_ZOOM, 0.25, 1.0, 4.0, MAX_ZOOM] {
            let metrics = LineworkMetrics::for_zoom(zoom);

            assert!(((metrics.frame_stroke_scale as f64 * zoom) - 1.0).abs() <= 1e-6);
            assert_eq!(metrics.frame_screen_space, 1.0);
            assert_eq!(metrics.group_stroke_width, Pen::HAIRLINE.width() as f32);
            assert_eq!(metrics.group_dash_period, 6.0);
            assert_eq!(metrics.divider_thickness, Pen::HAIRLINE.width());
            assert_eq!(metrics.edge_thickness, Pen::REGULAR.width());
            assert_eq!(metrics.marker_size, 10.0);
            assert_eq!(metrics.nub_size, 6.0);
        }
    }

    #[test]
    fn edge_dependents_hold_across_zoom() {
        let low = LineworkMetrics::for_zoom(0.25);
        let high = LineworkMetrics::for_zoom(4.0);
        assert_eq!(low.edge_thickness * 2.0, high.edge_thickness * 2.0);
        assert_eq!(low.edge_thickness * 0.5, high.edge_thickness * 0.5);
        assert_eq!(low.marker_size, high.marker_size);
    }

    #[test]
    fn metrics_are_positive_and_finite_at_zoom_bounds() {
        for zoom in [MIN_ZOOM, MAX_ZOOM] {
            let metrics = LineworkMetrics::for_zoom(zoom);
            assert!(metrics.frame_stroke_scale.is_finite() && metrics.frame_stroke_scale > 0.0);
            assert!(metrics.group_stroke_width.is_finite() && metrics.group_stroke_width > 0.0);
            assert!(metrics.group_dash_period.is_finite() && metrics.group_dash_period > 0.0);
            assert!(metrics.divider_thickness.is_finite() && metrics.divider_thickness > 0.0);
            assert!(metrics.edge_thickness.is_finite() && metrics.edge_thickness > 0.0);
            assert!(metrics.marker_size.is_finite() && metrics.marker_size > 0.0);
            assert!(metrics.nub_size.is_finite() && metrics.nub_size > 0.0);
        }
    }
}
