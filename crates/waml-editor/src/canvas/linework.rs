//! Shared linework policy for every canvas.
//!
//! CAD mode draws ONE constant screen-space hairline at every zoom; `Scaled` is
//! the legacy branch that grows linework with the camera for looks. The class
//! canvas reads `class::render::LineworkMetrics`; the behavior canvas reads
//! `BehaviorLineworkMetrics` below. Both fork on the SAME mode, so a future
//! setting flips both canvases at once.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::canvas) enum LineworkMode {
    Cad,
    // Retained as the legacy branch for future comparison or setting.
    #[allow(dead_code)]
    Scaled,
}

pub(in crate::canvas) const DEFAULT_LINEWORK_MODE: LineworkMode = LineworkMode::Cad;

/// Behavior-canvas linework: routes, lifeline stems, message strokes, arrow
/// heads and markers.
///
/// The behavior renderers author every stroke width and marker size as a world
/// constant and then hand it to one of the two methods here, so the mode -- not
/// the call site -- decides whether the camera touches it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::canvas) struct BehaviorLineworkMetrics {
    /// Multiplier on every authored stroke width and marker size.
    stroke_scale: f64,
    /// Floor a stroke may not drop below, in the same space as the result.
    min_thickness: f64,
}

impl BehaviorLineworkMetrics {
    pub(in crate::canvas) fn for_zoom(mode: LineworkMode, zoom: f64) -> Self {
        debug_assert!(zoom.is_finite() && zoom > 0.0);
        match mode {
            // CAD: the authored value IS the drawn value. The floor only guards
            // a degenerate sub-pixel author, never the camera.
            LineworkMode::Cad => Self {
                stroke_scale: 1.0,
                min_thickness: 1.0,
            },
            LineworkMode::Scaled => Self {
                stroke_scale: zoom.max(0.3),
                min_thickness: 1.4,
            },
        }
    }

    /// A stroke width: scaled by the mode, then floored so a route never
    /// vanishes.
    pub(in crate::canvas) fn thickness(&self, authored: f64) -> f64 {
        (authored * self.stroke_scale).max(self.min_thickness)
    }

    /// A marker or arrow-head extent. No floor -- these are sized off the
    /// stroke they terminate, and a floor here would freeze a fat glyph on a
    /// shrinking route (the same trap the class canvas's floored shadow was).
    pub(in crate::canvas) fn glyph(&self, authored: f64) -> f64 {
        authored * self.stroke_scale
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
    fn cad_behavior_linework_is_constant_across_zoom() {
        for zoom in [MIN_ZOOM, 0.25, 1.0, 4.0, MAX_ZOOM] {
            let metrics = BehaviorLineworkMetrics::for_zoom(LineworkMode::Cad, zoom);
            assert_eq!(metrics.thickness(2.0), 2.0);
            assert_eq!(metrics.thickness(1.4), 1.4);
            assert_eq!(metrics.thickness(1.2), 1.2);
            assert_eq!(metrics.glyph(9.0), 9.0);
            assert_eq!(metrics.glyph(10.0), 10.0);
        }
    }

    #[test]
    fn scaled_behavior_linework_follows_the_camera() {
        let low = BehaviorLineworkMetrics::for_zoom(LineworkMode::Scaled, 0.25);
        let high = BehaviorLineworkMetrics::for_zoom(LineworkMode::Scaled, 4.0);
        assert!(low.glyph(9.0) < high.glyph(9.0));
        assert_eq!(high.thickness(2.0), 8.0);
        // The floor holds a far-out route visible instead of letting it thin to
        // nothing.
        assert_eq!(low.thickness(2.0), 1.4);
    }

    #[test]
    fn modes_match_at_baseline_zoom() {
        let cad = BehaviorLineworkMetrics::for_zoom(LineworkMode::Cad, 1.0);
        let scaled = BehaviorLineworkMetrics::for_zoom(LineworkMode::Scaled, 1.0);
        for authored in [1.4, 2.0, 2.8, 4.0] {
            assert_eq!(cad.thickness(authored), scaled.thickness(authored));
        }
        assert_eq!(cad.glyph(9.0), scaled.glyph(9.0));
        // Below `Scaled`'s floor the two part company even at zoom 1: the
        // fragment border is authored at 1.2, and CAD draws exactly that.
        assert_eq!(cad.thickness(1.2), 1.2);
        assert_eq!(scaled.thickness(1.2), 1.4);
    }

    #[test]
    fn metrics_are_positive_and_finite_at_zoom_bounds() {
        for mode in [LineworkMode::Cad, LineworkMode::Scaled] {
            for zoom in [MIN_ZOOM, MAX_ZOOM] {
                let metrics = BehaviorLineworkMetrics::for_zoom(mode, zoom);
                assert!(metrics.thickness(1.4).is_finite() && metrics.thickness(1.4) > 0.0);
                assert!(metrics.glyph(9.0).is_finite() && metrics.glyph(9.0) > 0.0);
            }
        }
    }
}
