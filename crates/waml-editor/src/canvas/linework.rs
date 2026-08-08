//! Shared linework policy for every canvas.
//!
//! There is ONE policy: CAD. Every stroke is a constant screen-space hairline
//! at every zoom, so a diagram reads the same far out as it does close in. The
//! class canvas reads `class::render::LineworkMetrics`; the behavior canvas
//! reads `BehaviorLineworkMetrics` below.
//!
//! Screen-space width is only half of "sharp": a hairline still smears across
//! two device rows when its rect lands on a fractional device pixel, which is
//! exactly what a fractional camera offset produces. Every screen-space stroke
//! must therefore be snapped to the device grid before it is drawn -- see
//! `crate::canvas::primitives::snap_stroke_rect`.

/// Behavior-canvas linework: routes, lifeline stems, message strokes, arrow
/// heads and markers.
///
/// The behavior renderers author every stroke width and marker size as a world
/// constant and hand it to one of the two methods here, so this type -- not the
/// call site -- owns the policy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::canvas) struct BehaviorLineworkMetrics {
    /// Floor a stroke may not drop below. Guards a degenerate sub-pixel author,
    /// never the camera.
    min_thickness: f64,
    /// `AccentFrame::stroke_scale` for the lifeline-head surface -- the inverse
    /// zoom that cancels the shader's `zoom` term, so the frame holds one
    /// screen-space hairline.
    pub(in crate::canvas) frame_stroke_scale: f32,
    /// `AccentFrame::screen_space`: always 1.0 on a canvas, dropping the
    /// zoom-driven stroke-alpha lift and shadow floor that only the non-canvas
    /// consumers (popups, which use `zoom` as a compactness knob) want.
    pub(in crate::canvas) frame_screen_space: f32,
}

impl BehaviorLineworkMetrics {
    pub(in crate::canvas) fn for_zoom(zoom: f64) -> Self {
        debug_assert!(zoom.is_finite() && zoom > 0.0);
        Self {
            min_thickness: 1.0,
            frame_stroke_scale: (1.0 / zoom) as f32,
            frame_screen_space: 1.0,
        }
    }

    /// A stroke width: the authored value IS the drawn value, floored so a
    /// route never vanishes.
    pub(in crate::canvas) fn thickness(&self, authored: f64) -> f64 {
        authored.max(self.min_thickness)
    }

    /// A marker or arrow-head extent. No floor -- these are sized off the
    /// stroke they terminate, and a floor here would freeze a fat glyph on a
    /// shrinking route.
    pub(in crate::canvas) fn glyph(&self, authored: f64) -> f64 {
        authored
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::viewport::{MAX_ZOOM, MIN_ZOOM};

    #[test]
    fn behavior_linework_is_constant_across_zoom() {
        for zoom in [MIN_ZOOM, 0.25, 1.0, 4.0, MAX_ZOOM] {
            let metrics = BehaviorLineworkMetrics::for_zoom(zoom);
            assert_eq!(metrics.thickness(2.0), 2.0);
            assert_eq!(metrics.thickness(1.4), 1.4);
            assert_eq!(metrics.thickness(1.2), 1.2);
            assert_eq!(metrics.glyph(9.0), 9.0);
            assert_eq!(metrics.glyph(10.0), 10.0);
        }
    }

    #[test]
    fn frame_stroke_scale_cancels_the_camera() {
        for zoom in [MIN_ZOOM, 0.25, 1.0, 4.0, MAX_ZOOM] {
            let metrics = BehaviorLineworkMetrics::for_zoom(zoom);
            assert!(((metrics.frame_stroke_scale as f64 * zoom) - 1.0).abs() <= 1e-6);
            assert_eq!(metrics.frame_screen_space, 1.0);
        }
    }

    #[test]
    fn metrics_are_positive_and_finite_at_zoom_bounds() {
        for zoom in [MIN_ZOOM, MAX_ZOOM] {
            let metrics = BehaviorLineworkMetrics::for_zoom(zoom);
            assert!(metrics.thickness(1.4).is_finite() && metrics.thickness(1.4) > 0.0);
            assert!(metrics.glyph(9.0).is_finite() && metrics.glyph(9.0) > 0.0);
        }
    }
}
