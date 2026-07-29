//! Shared viewport and pan/zoom mechanics. `local` coordinates are
//! relative to the canvas rect's top-left; the widget adds the rect origin.

use makepad_widgets::{DVec2, Rect as ViewRect};
use waml::solve::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Camera {
    pub(crate) pan_x: f64,
    pub(crate) pan_y: f64,
    pub(crate) zoom: f64,
}

/// Zoom is clamped to this range to avoid degenerate transforms.
pub(crate) const MIN_ZOOM: f64 = 0.05;
pub(crate) const MAX_ZOOM: f64 = 20.0;

impl Camera {
    /// World (diagram-pixel) point -> canvas-local point.
    pub(crate) fn world_to_local(&self, wx: f64, wy: f64) -> (f64, f64) {
        ((wx - self.pan_x) * self.zoom, (wy - self.pan_y) * self.zoom)
    }

    /// Canvas-local point -> world point.
    pub(crate) fn local_to_world(&self, lx: f64, ly: f64) -> (f64, f64) {
        (lx / self.zoom + self.pan_x, ly / self.zoom + self.pan_y)
    }

    /// Multiply zoom by `factor`, keeping the world point under `(local_x, local_y)` fixed.
    pub(crate) fn zoom_at(&mut self, local_x: f64, local_y: f64, factor: f64) {
        let (wx, wy) = self.local_to_world(local_x, local_y);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan_x = wx - local_x / self.zoom;
        self.pan_y = wy - local_y / self.zoom;
    }

    /// Fit `bbox` centered in a `viewport_w` x `viewport_h` canvas with `pad` px inset.
    pub(crate) fn fit(bbox: Rect, viewport_w: f64, viewport_h: f64, pad: f64) -> Camera {
        let avail_w = (viewport_w - 2.0 * pad).max(1.0);
        let avail_h = (viewport_h - 2.0 * pad).max(1.0);
        let zoom = if bbox.w > 0.0 && bbox.h > 0.0 {
            (avail_w / bbox.w)
                .min(avail_h / bbox.h)
                .clamp(MIN_ZOOM, MAX_ZOOM)
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

impl Default for Camera {
    fn default() -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

pub(crate) const FIT_PAD: f64 = 48.0;
pub(crate) const ZOOM_STEP: f64 = 1.2;
pub(crate) const CAMERA_SECS: f64 = 0.22;
pub(crate) const CAMERA_TICK: f64 = 1.0 / 144.0;
const MIN_PINCH_SPREAD: f64 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum InitialFit {
    None,
    ScenePending,
    Scene(waml::solve::Rect),
    FocusPending,
    Focus(waml::solve::Rect),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TimerCommand {
    Keep,
    StartInterval(f64),
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewportEffects {
    pub(crate) redraw: bool,
    pub(crate) camera_timer: TimerCommand,
}

impl ViewportEffects {
    fn unchanged() -> Self {
        Self {
            redraw: false,
            camera_timer: TimerCommand::Keep,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewportSnapshot {
    pub(crate) camera: Camera,
    pub(crate) view_rect: ViewRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct TouchPair {
    pub(crate) a: u64,
    pub(crate) b: u64,
    pub(crate) spread: f64,
    pub(crate) midpoint_abs: DVec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PanOrigin {
    down_abs: DVec2,
    pan_x: f64,
    pan_y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct CameraTween {
    from: Camera,
    to: Camera,
    t: f64,
}

pub(crate) struct ViewportController {
    camera: Camera,
    view_rect: ViewRect,
    initial_fit: InitialFit,
    pan: Option<PanOrigin>,
    release_down_abs: Option<DVec2>,
    pinch: Option<TouchPair>,
    tween: Option<CameraTween>,
    tween_last_time: f64,
}

impl Default for ViewportController {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            view_rect: ViewRect::default(),
            initial_fit: InitialFit::None,
            pan: None,
            release_down_abs: None,
            pinch: None,
            tween: None,
            tween_last_time: 0.0,
        }
    }
}

impl ViewportController {
    pub(crate) fn camera(&self) -> Camera {
        self.camera
    }

    pub(crate) fn snapshot(&self) -> ViewportSnapshot {
        ViewportSnapshot {
            camera: self.camera,
            view_rect: self.view_rect,
        }
    }

    pub(crate) fn restore_camera(&mut self, camera: Camera) -> Camera {
        let restored = Camera {
            pan_x: if camera.pan_x.is_finite() {
                camera.pan_x
            } else {
                0.0
            },
            pan_y: if camera.pan_y.is_finite() {
                camera.pan_y
            } else {
                0.0
            },
            zoom: if camera.zoom.is_finite() {
                camera.zoom.clamp(MIN_ZOOM, MAX_ZOOM)
            } else {
                1.0
            },
        };
        self.camera = restored;
        self.initial_fit = InitialFit::None;
        self.tween = None;
        restored
    }

    pub(crate) fn set_view_rect(&mut self, rect: ViewRect) {
        self.view_rect = rect;
    }

    pub(crate) fn request_initial_fit(&mut self, fit: InitialFit) {
        self.initial_fit = fit;
    }

    pub(crate) fn apply_initial_fit(&mut self) -> bool {
        if self.view_rect.size.x <= 0.0 || self.view_rect.size.y <= 0.0 {
            return false;
        }
        let camera = match self.initial_fit {
            InitialFit::None => return false,
            InitialFit::ScenePending | InitialFit::FocusPending => return false,
            InitialFit::Scene(bounds) => {
                fit_scene_camera(bounds, self.view_rect.size.x, self.view_rect.size.y)
            }
            InitialFit::Focus(bounds) => Camera {
                pan_x: bounds.x + bounds.w * 0.5 - self.view_rect.size.x * 0.5,
                pan_y: bounds.y + bounds.h * 0.5 - self.view_rect.size.y * 0.5,
                zoom: 1.0,
            },
        };
        self.camera = camera;
        self.initial_fit = InitialFit::None;
        true
    }

    pub(crate) fn retain_for_scene_update(
        &mut self,
        replacement_bounds: Option<waml::solve::Rect>,
    ) -> ViewportEffects {
        self.initial_fit = match (self.initial_fit, replacement_bounds) {
            (InitialFit::None, _) => InitialFit::None,
            (InitialFit::ScenePending | InitialFit::Scene(_), None) => InitialFit::ScenePending,
            (InitialFit::ScenePending | InitialFit::Scene(_), Some(bounds)) => {
                InitialFit::Scene(bounds)
            }
            (InitialFit::FocusPending | InitialFit::Focus(_), None) => InitialFit::FocusPending,
            (InitialFit::FocusPending | InitialFit::Focus(_), Some(bounds)) => {
                InitialFit::Focus(bounds)
            }
        };
        self.tween = None;
        self.tween_last_time = 0.0;
        ViewportEffects {
            redraw: false,
            camera_timer: TimerCommand::Stop,
        }
    }

    pub(crate) fn begin_pan(&mut self, abs: DVec2) {
        self.pan = Some(PanOrigin {
            down_abs: abs,
            pan_x: self.camera.pan_x,
            pan_y: self.camera.pan_y,
        });
        self.release_down_abs = Some(abs);
    }

    pub(crate) fn pan_down_abs(&self) -> Option<DVec2> {
        self.release_down_abs
    }

    pub(crate) fn pan_to(&mut self, abs: DVec2) -> bool {
        let Some(origin) = self.pan else {
            return false;
        };
        let delta = abs - origin.down_abs;
        let before = self.camera;
        self.camera.pan_x = origin.pan_x - delta.x / self.camera.zoom;
        self.camera.pan_y = origin.pan_y - delta.y / self.camera.zoom;
        self.camera != before
    }

    pub(crate) fn end_pan(&mut self) {
        self.pan = None;
        self.release_down_abs = None;
    }

    pub(crate) fn apply_scroll_zoom(&mut self, abs: DVec2, factor: f64) -> ViewportEffects {
        self.tween = None;
        self.tween_last_time = 0.0;
        self.camera.zoom_at(
            abs.x - self.view_rect.pos.x,
            abs.y - self.view_rect.pos.y,
            factor,
        );
        ViewportEffects {
            redraw: true,
            camera_timer: TimerCommand::Stop,
        }
    }

    pub(crate) fn apply_pinch_sample(&mut self, sample: TouchPair) -> ViewportEffects {
        let mut effects = ViewportEffects::unchanged();
        match self.pinch {
            Some(previous) if previous.a == sample.a && previous.b == sample.b => {
                if let Some(factor) = pinch_factor(previous.spread, sample.spread) {
                    self.camera.zoom_at(
                        sample.midpoint_abs.x - self.view_rect.pos.x,
                        sample.midpoint_abs.y - self.view_rect.pos.y,
                        factor,
                    );
                }
                let travel = sample.midpoint_abs - previous.midpoint_abs;
                self.camera.pan_x -= travel.x / self.camera.zoom;
                self.camera.pan_y -= travel.y / self.camera.zoom;
                effects.redraw = true;
            }
            _ => {
                self.end_pan();
                self.tween = None;
                self.tween_last_time = 0.0;
                effects.camera_timer = TimerCommand::Stop;
            }
        }
        self.pinch = Some(sample);
        effects
    }

    pub(crate) fn end_pinch(&mut self) -> bool {
        self.pinch.take().is_some()
    }

    pub(crate) fn zoom_step(&mut self, factor: f64) -> ViewportEffects {
        let mut target = self.tween.map(|tween| tween.to).unwrap_or(self.camera);
        target.zoom_at(
            self.view_rect.size.x * 0.5,
            self.view_rect.size.y * 0.5,
            factor,
        );
        self.glide_to(target)
    }

    pub(crate) fn fit_to_bounds(&mut self, bounds: Option<waml::solve::Rect>) -> ViewportEffects {
        let Some(bounds) = bounds else {
            return ViewportEffects::unchanged();
        };
        if self.view_rect.size.x <= 0.0 || self.view_rect.size.y <= 0.0 {
            return ViewportEffects::unchanged();
        }
        self.initial_fit = InitialFit::None;
        self.glide_to(fit_scene_camera(
            bounds,
            self.view_rect.size.x,
            self.view_rect.size.y,
        ))
    }

    pub(crate) fn glide_to(&mut self, target: Camera) -> ViewportEffects {
        if target == self.camera {
            return self.cancel_glide();
        }
        let camera_timer = if self.tween.is_none() {
            TimerCommand::StartInterval(CAMERA_TICK)
        } else {
            TimerCommand::Keep
        };
        self.tween = Some(CameraTween {
            from: self.camera,
            to: target,
            t: 0.0,
        });
        self.tween_last_time = 0.0;
        ViewportEffects {
            redraw: true,
            camera_timer,
        }
    }

    pub(crate) fn cancel_glide(&mut self) -> ViewportEffects {
        self.tween = None;
        self.tween_last_time = 0.0;
        ViewportEffects {
            redraw: false,
            camera_timer: TimerCommand::Stop,
        }
    }

    pub(crate) fn tick_camera(&mut self, now: f64) -> ViewportEffects {
        let Some(mut tween) = self.tween else {
            self.tween_last_time = 0.0;
            return ViewportEffects {
                redraw: false,
                camera_timer: TimerCommand::Stop,
            };
        };
        let dt = if self.tween_last_time == 0.0 || now <= self.tween_last_time {
            CAMERA_TICK
        } else {
            now - self.tween_last_time
        };
        self.tween_last_time = now;
        tween.t = (tween.t + dt / CAMERA_SECS).min(1.0);
        self.camera = lerp_camera(tween.from, tween.to, self.view_rect.size, ease_out(tween.t));
        if tween.t >= 1.0 {
            self.tween = None;
            self.tween_last_time = 0.0;
            ViewportEffects {
                redraw: true,
                camera_timer: TimerCommand::Stop,
            }
        } else {
            self.tween = Some(tween);
            ViewportEffects {
                redraw: true,
                camera_timer: TimerCommand::Keep,
            }
        }
    }

    pub(crate) fn set_transient_camera(&mut self, camera: Camera) {
        self.camera = camera;
    }
}

pub(crate) fn ease_out(t: f64) -> f64 {
    let u = 1.0 - t.clamp(0.0, 1.0);
    1.0 - u * u * u
}

fn lerp_camera(from: Camera, to: Camera, view: DVec2, e: f64) -> Camera {
    if e <= 0.0 {
        return from;
    }
    if e >= 1.0 {
        return to;
    }
    let (half_w, half_h) = (view.x * 0.5, view.y * 0.5);
    let a = from.local_to_world(half_w, half_h);
    let b = to.local_to_world(half_w, half_h);
    let zoom = (from.zoom.ln() + (to.zoom.ln() - from.zoom.ln()) * e).exp();
    let (wx, wy) = (a.0 + (b.0 - a.0) * e, a.1 + (b.1 - a.1) * e);
    Camera {
        pan_x: wx - half_w / zoom,
        pan_y: wy - half_h / zoom,
        zoom,
    }
}

fn fit_scene_camera(bbox: waml::solve::Rect, viewport_w: f64, viewport_h: f64) -> Camera {
    Camera::fit(bbox, viewport_w, viewport_h, FIT_PAD)
}

fn pinch_factor(prev_spread: f64, spread: f64) -> Option<f64> {
    if prev_spread < MIN_PINCH_SPREAD || spread < MIN_PINCH_SPREAD {
        return None;
    }
    Some(spread / prev_spread)
}

#[cfg(test)]
mod tests {
    use super::*;
    use makepad_widgets::{dvec2, Rect as ViewRect};

    fn approx(a: (f64, f64), b: (f64, f64)) {
        assert!(
            (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9,
            "{a:?} != {b:?}"
        );
    }

    #[test]
    fn world_local_round_trip() {
        let cam = Camera {
            pan_x: 30.0,
            pan_y: -10.0,
            zoom: 2.0,
        };
        let local = cam.world_to_local(100.0, 50.0);
        approx(local, ((100.0 - 30.0) * 2.0, (50.0 - -10.0) * 2.0));
        approx(cam.local_to_world(local.0, local.1), (100.0, 50.0));
    }

    #[test]
    fn zoom_at_keeps_point_under_cursor_fixed() {
        let mut cam = Camera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        };
        let before = cam.local_to_world(400.0, 300.0);
        cam.zoom_at(400.0, 300.0, 1.5);
        let after = cam.local_to_world(400.0, 300.0);
        approx(before, after);
        assert!((cam.zoom - 1.5).abs() < 1e-9);
    }

    #[test]
    fn zoom_at_clamps_to_bounds() {
        let mut cam = Camera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        };
        cam.zoom_at(0.0, 0.0, 1000.0);
        assert_eq!(cam.zoom, MAX_ZOOM);
        cam.zoom_at(0.0, 0.0, 0.0001);
        assert_eq!(cam.zoom, MIN_ZOOM);
    }

    #[test]
    fn fit_centers_bbox_in_viewport() {
        let bbox = Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        };
        let cam = Camera::fit(bbox, 800.0, 600.0, 40.0);
        // Limiting axis: width. zoom = (800-80)/200 = 3.6.
        assert!((cam.zoom - 3.6).abs() < 1e-9);
        // The bbox center maps to the viewport center.
        let center = cam.world_to_local(100.0, 50.0);
        approx(center, (400.0, 300.0));
    }

    #[test]
    fn fit_of_empty_viewport_stays_positive() {
        let bbox = Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        };
        let cam = Camera::fit(bbox, 0.0, 0.0, 40.0);
        assert!(cam.zoom >= MIN_ZOOM);
    }

    #[test]
    fn pan_is_owned_by_the_viewport() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(100.0, 50.0),
            size: dvec2(800.0, 600.0),
        });
        viewport.begin_pan(dvec2(300.0, 200.0));
        viewport.pan_to(dvec2(360.0, 230.0));
        assert_eq!(viewport.camera().pan_x, -60.0);
        assert_eq!(viewport.camera().pan_y, -30.0);
    }

    #[test]
    fn camera_tick_lands_exactly_on_target_and_stops() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        let target = Camera {
            pan_x: 20.0,
            pan_y: 30.0,
            zoom: 2.0,
        };
        assert_eq!(
            viewport.glide_to(target).camera_timer,
            TimerCommand::StartInterval(CAMERA_TICK),
        );
        viewport.tick_camera(10.0);
        let effects = viewport.tick_camera(10.0 + CAMERA_SECS);
        assert_eq!(viewport.camera(), target);
        assert_eq!(effects.camera_timer, TimerCommand::Stop);
    }

    #[test]
    fn pinch_rejects_degenerate_spread_and_keeps_the_fixed_point() {
        assert_eq!(pinch_factor(4.0, 8.0), None);
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(50.0, 25.0),
            size: dvec2(800.0, 600.0),
        });
        let before = viewport.camera().local_to_world(400.0, 300.0);
        viewport.apply_pinch_sample(TouchPair {
            a: 1,
            b: 2,
            spread: 100.0,
            midpoint_abs: dvec2(450.0, 325.0),
        });
        viewport.apply_pinch_sample(TouchPair {
            a: 1,
            b: 2,
            spread: 150.0,
            midpoint_abs: dvec2(450.0, 325.0),
        });
        let after = viewport.camera().local_to_world(400.0, 300.0);
        approx(before, after);
    }

    #[test]
    fn pinch_factor_has_a_safe_spread_threshold() {
        assert_eq!(pinch_factor(100.0, 200.0), Some(2.0));
        assert_eq!(pinch_factor(200.0, 100.0), Some(0.5));
        assert_eq!(pinch_factor(MIN_PINCH_SPREAD - 0.1, 200.0), None);
        assert!(pinch_factor(MIN_PINCH_SPREAD, MIN_PINCH_SPREAD).is_some());
    }

    #[test]
    fn fit_uses_the_shared_pad() {
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: 200.0,
            h: 100.0,
        };
        assert_eq!(FIT_PAD, 48.0);
        assert_eq!(
            fit_scene_camera(bounds, 800.0, 600.0),
            Camera::fit(bounds, 800.0, 600.0, FIT_PAD),
        );
    }

    #[test]
    fn zoom_step_is_a_symmetric_pair() {
        let mut camera = Camera::default();
        camera.zoom_at(400.0, 300.0, ZOOM_STEP);
        camera.zoom_at(400.0, 300.0, 1.0 / ZOOM_STEP);
        assert!((camera.zoom - 1.0).abs() < 1e-9);
        assert!(camera.pan_x.abs() < 1e-9 && camera.pan_y.abs() < 1e-9);
    }

    #[test]
    fn glide_interpolation_holds_the_viewport_centre() {
        let view = dvec2(1280.0, 880.0);
        let from = Camera {
            pan_x: -120.0,
            pan_y: 40.0,
            zoom: 0.8,
        };
        let mut to = from;
        to.zoom_at(view.x * 0.5, view.y * 0.5, ZOOM_STEP);
        let anchor = from.local_to_world(view.x * 0.5, view.y * 0.5);
        for step in 0..=10 {
            let mid = lerp_camera(from, to, view, step as f64 / 10.0);
            approx(mid.local_to_world(view.x * 0.5, view.y * 0.5), anchor);
        }
        assert_eq!(lerp_camera(from, to, view, 1.0), to);
    }

    #[test]
    fn scene_update_cancels_a_glide_before_a_late_timer_tick() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        viewport.glide_to(Camera {
            pan_x: 200.0,
            pan_y: 100.0,
            zoom: 2.0,
        });
        viewport.tick_camera(10.0);
        let camera_at_update = viewport.camera();

        let effects = viewport.retain_for_scene_update(Some(Rect {
            x: 500.0,
            y: 400.0,
            w: 200.0,
            h: 100.0,
        }));
        assert_eq!(effects.camera_timer, TimerCommand::Stop);

        let late_tick = viewport.tick_camera(20.0);
        assert_eq!(late_tick.camera_timer, TimerCommand::Stop);
        assert_eq!(viewport.camera(), camera_at_update);
    }

    #[test]
    fn scene_update_retargets_a_pending_initial_fit() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        let scene_a = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 100.0,
        };
        let scene_b = Rect {
            x: 1000.0,
            y: 700.0,
            w: 400.0,
            h: 300.0,
        };
        viewport.request_initial_fit(InitialFit::Scene(scene_a));

        viewport.retain_for_scene_update(Some(scene_b));
        assert!(viewport.apply_initial_fit());
        assert_eq!(viewport.camera(), fit_scene_camera(scene_b, 800.0, 600.0),);
    }

    #[test]
    fn scene_update_preserves_a_settled_camera_without_scheduling_a_fit() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        let settled = Camera {
            pan_x: 30.0,
            pan_y: -20.0,
            zoom: 1.5,
        };
        viewport.set_transient_camera(settled);

        let effects = viewport.retain_for_scene_update(Some(Rect {
            x: 500.0,
            y: 400.0,
            w: 200.0,
            h: 100.0,
        }));

        assert_eq!(effects.camera_timer, TimerCommand::Stop);
        assert!(!viewport.apply_initial_fit());
        assert_eq!(viewport.camera(), settled);
    }

    #[test]
    fn empty_scene_then_populated_update_fits_on_the_first_valid_draw() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        viewport.request_initial_fit(InitialFit::ScenePending);

        assert!(!viewport.apply_initial_fit());
        assert_eq!(viewport.camera(), Camera::default());

        let scene_b = Rect {
            x: 1000.0,
            y: 700.0,
            w: 400.0,
            h: 300.0,
        };
        viewport.retain_for_scene_update(Some(scene_b));
        assert!(viewport.apply_initial_fit());
        assert_eq!(viewport.camera(), fit_scene_camera(scene_b, 800.0, 600.0),);
    }

    #[test]
    fn empty_focus_then_populated_update_retains_focus_fit_semantics() {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(ViewRect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        viewport.request_initial_fit(InitialFit::FocusPending);

        assert!(!viewport.apply_initial_fit());
        assert!(!viewport.apply_initial_fit());

        let focus_b = Rect {
            x: 1000.0,
            y: 700.0,
            w: 400.0,
            h: 300.0,
        };
        viewport.retain_for_scene_update(Some(focus_b));
        assert!(viewport.apply_initial_fit());
        assert_eq!(
            viewport.camera(),
            Camera {
                pan_x: 800.0,
                pan_y: 550.0,
                zoom: 1.0,
            },
        );
    }

    #[test]
    fn restoring_a_camera_clamps_zoom_and_rejects_non_finite_coordinates() {
        let mut viewport = ViewportController::default();

        assert_eq!(
            viewport.restore_camera(Camera {
                pan_x: f64::NAN,
                pan_y: f64::INFINITY,
                zoom: MAX_ZOOM * 2.0,
            }),
            Camera {
                pan_x: 0.0,
                pan_y: 0.0,
                zoom: MAX_ZOOM,
            }
        );
        assert_eq!(
            viewport.restore_camera(Camera {
                pan_x: 12.0,
                pan_y: -8.0,
                zoom: 0.0,
            }),
            Camera {
                pan_x: 12.0,
                pan_y: -8.0,
                zoom: MIN_ZOOM,
            }
        );
    }
}
