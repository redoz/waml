#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LineworkMode {
    Cad,
    Scaled,
}

pub(super) const DEFAULT_LINEWORK_MODE: LineworkMode = LineworkMode::Cad;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LineworkMetrics {
    pub(super) frame_stroke_scale: f32,
    pub(super) group_stroke_width: f32,
    pub(super) group_dash_period: f32,
    pub(super) divider_thickness: f64,
    pub(super) edge_thickness: f64,
    pub(super) marker_size: f64,
    pub(super) nub_size: f64,
}

impl LineworkMetrics {
    pub(super) fn for_zoom(mode: LineworkMode, zoom: f64) -> Self {
        debug_assert!(zoom.is_finite() && zoom > 0.0);
        match mode {
            LineworkMode::Cad => Self {
                frame_stroke_scale: (1.0 / zoom) as f32,
                group_stroke_width: 1.0,
                group_dash_period: 6.0,
                divider_thickness: 1.0,
                edge_thickness: 3.0,
                marker_size: 10.0,
                nub_size: 6.0,
            },
            LineworkMode::Scaled => Self {
                frame_stroke_scale: 1.0,
                group_stroke_width: 1.0,
                group_dash_period: (6.0 * zoom).clamp(3.0, 18.0) as f32,
                divider_thickness: zoom.max(1.0),
                edge_thickness: (3.0 * zoom).max(1.8),
                marker_size: (10.0 * zoom).max(4.0),
                nub_size: 6.0 * zoom,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::viewport::{MAX_ZOOM, MIN_ZOOM};

    #[test]
    fn default_linework_mode_is_cad() {
        assert_eq!(DEFAULT_LINEWORK_MODE, LineworkMode::Cad);
    }

    #[test]
    fn cad_metrics_are_screen_space_at_supported_zooms() {
        for zoom in [MIN_ZOOM, 0.25, 1.0, 4.0, MAX_ZOOM] {
            let metrics = LineworkMetrics::for_zoom(LineworkMode::Cad, zoom);

            assert!(((metrics.frame_stroke_scale as f64 * zoom) - 1.0).abs() <= 1e-6);
            assert_eq!(metrics.group_stroke_width, 1.0);
            assert_eq!(metrics.group_dash_period, 6.0);
            assert_eq!(metrics.divider_thickness, 1.0);
            assert_eq!(metrics.edge_thickness, 3.0);
            assert_eq!(metrics.marker_size, 10.0);
            assert_eq!(metrics.nub_size, 6.0);
        }
    }

    #[test]
    fn scaled_metrics_at_quarter_zoom_obey_minima() {
        let metrics = LineworkMetrics::for_zoom(LineworkMode::Scaled, 0.25);

        assert_eq!(metrics.frame_stroke_scale, 1.0);
        assert_eq!(metrics.group_stroke_width, 1.0);
        assert_eq!(metrics.group_dash_period, 3.0);
        assert_eq!(metrics.divider_thickness, 1.0);
        assert_eq!(metrics.edge_thickness, 1.8);
        assert_eq!(metrics.marker_size, 4.0);
        assert_eq!(metrics.nub_size, 1.5);
    }

    #[test]
    fn scaled_metrics_at_double_zoom_scale_linework() {
        let metrics = LineworkMetrics::for_zoom(LineworkMode::Scaled, 2.0);

        assert_eq!(metrics.frame_stroke_scale, 1.0);
        assert_eq!(metrics.group_stroke_width, 1.0);
        assert_eq!(metrics.group_dash_period, 12.0);
        assert_eq!(metrics.divider_thickness, 2.0);
        assert_eq!(metrics.edge_thickness, 6.0);
        assert_eq!(metrics.marker_size, 20.0);
        assert_eq!(metrics.nub_size, 12.0);
    }

    #[test]
    fn scaled_dash_period_is_capped() {
        assert_eq!(
            LineworkMetrics::for_zoom(LineworkMode::Scaled, 20.0).group_dash_period,
            18.0
        );
    }

    #[test]
    fn modes_match_at_baseline_zoom() {
        assert_eq!(
            LineworkMetrics::for_zoom(LineworkMode::Cad, 1.0),
            LineworkMetrics::for_zoom(LineworkMode::Scaled, 1.0)
        );
    }

    #[test]
    fn metrics_are_positive_and_finite_at_zoom_bounds() {
        for mode in [LineworkMode::Cad, LineworkMode::Scaled] {
            for zoom in [MIN_ZOOM, MAX_ZOOM] {
                let metrics = LineworkMetrics::for_zoom(mode, zoom);
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
}
