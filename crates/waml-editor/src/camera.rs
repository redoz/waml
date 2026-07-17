//! Pan/zoom camera. Pure math — no makepad types. `local` coordinates are
//! relative to the canvas rect's top-left; the widget adds the rect origin.

use waml::solve::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

/// Zoom is clamped to this range to avoid degenerate transforms.
pub const MIN_ZOOM: f64 = 0.05;
pub const MAX_ZOOM: f64 = 20.0;

impl Camera {
    /// World (diagram-pixel) point -> canvas-local point.
    pub fn world_to_local(&self, wx: f64, wy: f64) -> (f64, f64) {
        ((wx - self.pan_x) * self.zoom, (wy - self.pan_y) * self.zoom)
    }

    /// Canvas-local point -> world point.
    pub fn local_to_world(&self, lx: f64, ly: f64) -> (f64, f64) {
        (lx / self.zoom + self.pan_x, ly / self.zoom + self.pan_y)
    }

    /// Multiply zoom by `factor`, keeping the world point under `(local_x, local_y)` fixed.
    pub fn zoom_at(&mut self, local_x: f64, local_y: f64, factor: f64) {
        let (wx, wy) = self.local_to_world(local_x, local_y);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan_x = wx - local_x / self.zoom;
        self.pan_y = wy - local_y / self.zoom;
    }

    /// Fit `bbox` centered in a `viewport_w` x `viewport_h` canvas with `pad` px inset.
    pub fn fit(bbox: Rect, viewport_w: f64, viewport_h: f64, pad: f64) -> Camera {
        let avail_w = (viewport_w - 2.0 * pad).max(1.0);
        let avail_h = (viewport_h - 2.0 * pad).max(1.0);
        let zoom = if bbox.w > 0.0 && bbox.h > 0.0 {
            (avail_w / bbox.w).min(avail_h / bbox.h).clamp(MIN_ZOOM, MAX_ZOOM)
        } else {
            1.0_f64.clamp(MIN_ZOOM, MAX_ZOOM)
        };
        let (cx, cy) = (bbox.x + bbox.w * 0.5, bbox.y + bbox.h * 0.5);
        Camera {
            pan_x: cx - viewport_w * 0.5 / zoom,
            pan_y: cy - viewport_h * 0.5 / zoom,
            zoom,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: (f64, f64), b: (f64, f64)) {
        assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{a:?} != {b:?}");
    }

    #[test]
    fn world_local_round_trip() {
        let cam = Camera { pan_x: 30.0, pan_y: -10.0, zoom: 2.0 };
        let local = cam.world_to_local(100.0, 50.0);
        approx(local, ((100.0 - 30.0) * 2.0, (50.0 - -10.0) * 2.0));
        approx(cam.local_to_world(local.0, local.1), (100.0, 50.0));
    }

    #[test]
    fn zoom_at_keeps_point_under_cursor_fixed() {
        let mut cam = Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        let before = cam.local_to_world(400.0, 300.0);
        cam.zoom_at(400.0, 300.0, 1.5);
        let after = cam.local_to_world(400.0, 300.0);
        approx(before, after);
        assert!((cam.zoom - 1.5).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_clamps_to_bounds() {
        let mut cam = Camera { pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
        cam.zoom_at(0.0, 0.0, 1000.0);
        assert_eq!(cam.zoom, MAX_ZOOM);
        cam.zoom_at(0.0, 0.0, 0.0001);
        assert_eq!(cam.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_centers_bbox_in_viewport() {
        let bbox = Rect { x: 0.0, y: 0.0, w: 200.0, h: 100.0 };
        let cam = Camera::fit(bbox, 800.0, 600.0, 40.0);
        // Limiting axis: width. zoom = (800-80)/200 = 3.6.
        assert!((cam.zoom - 3.6).abs() < 1e-9);
        // The bbox center maps to the viewport center.
        let center = cam.world_to_local(100.0, 50.0);
        approx(center, (400.0, 300.0));
    }

    #[test]
    fn fit_of_empty_viewport_stays_positive() {
        let bbox = Rect { x: 0.0, y: 0.0, w: 200.0, h: 100.0 };
        let cam = Camera::fit(bbox, 0.0, 0.0, 40.0);
        assert!(cam.zoom >= MIN_ZOOM);
    }
}
