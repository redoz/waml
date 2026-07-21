//! FPS-heat meter: samples framerate during a user interaction and maps it to a
//! green->amber->red "heat" colour plus an eased 0..1 strength. Pulled out of the
//! wordmark widget (`logo.rs`) so the logo stays a dumb renderer -- it just takes
//! `color`/`strength` via `LogoMark::set_heat`. `App` owns one `FpsMeter`,
//! forwards raw events to `on_event`, and pushes the result to the logo whenever
//! it reports a change.
//!
//! Metering spans an interaction: a primary pointer press..release (a natural
//! span), plus a short decaying tail after each mouse-wheel scroll (which has no
//! release event). While metering, next-frame dt feeds an EMA-smoothed
//! framerate; when idle it schedules no frames, so it costs nothing at rest.

use makepad_widgets::*;

// Heat-strength ease-in/out duration (seconds): `strength` ramps 0->1 as metering
// engages, 1->0 as it releases -- smooths the enable/disable so the tint fades.
const METER_SECS: f64 = 0.2;

// FPS smoothing time constant (seconds) for the exponential moving average --
// small enough to react to load, large enough to steady the per-frame jitter.
const FPS_TAU: f64 = 0.15;

// Tail (seconds) the meter stays live after the last mouse-wheel scroll. Scroll
// has no release event like a press does, so each scroll re-extends this window
// and the meter eases out once wheel input goes quiet (e.g. after zooming).
const SCROLL_METER_TAIL: f64 = 0.4;

/// App-owned framerate meter. Not a widget: it holds no drawing, just the
/// sampling/easing state and its own frame tick. See the module docs.
#[derive(Default)]
pub struct FpsMeter {
    // Interaction-span flag: true across a primary pointer press..release.
    metering: bool,
    // Deadline (seconds_since_app_start) until which scroll-driven metering stays
    // live; re-extended per scroll. Effective metering = `metering || now < this`.
    meter_until: f64,
    // Eased 0..1 heat strength handed to the logo (ramps toward 1 while metering,
    // toward 0 otherwise).
    strength: f32,
    // EMA-smoothed framerate (Hz), sampled from next-frame dt while metering.
    fps: f32,
    // Heat colour (green->amber->red) mapped from `fps`.
    color: [f32; 3],
    // Set on a metering rising edge: skips the first fps sample so a stale dt
    // (idle gap before the interaction) can't flash red.
    skip_fps_sample: bool,
    // Timestamp of the last processed frame, for dt.
    last_time: f64,
    next_frame: NextFrame,
}

impl FpsMeter {
    /// Current heat colour (green->amber->red).
    pub fn color(&self) -> [f32; 3] {
        self.color
    }

    /// Current eased 0..1 heat strength.
    pub fn strength(&self) -> f32 {
        self.strength
    }

    /// Feed a raw event. Detects interaction spans (primary press/release plus the
    /// scroll tail) and advances the sampler on its own next-frame tick. Returns
    /// `true` when `color`/`strength` may have changed and the caller should push
    /// them to the logo (and redraw it).
    pub fn on_event(&mut self, cx: &mut Cx, event: &Event) -> bool {
        match event {
            Event::MouseDown(e) if e.button.is_primary() => {
                self.set_span(cx, true);
                true
            }
            Event::MouseUp(e) if e.button.is_primary() => {
                self.set_span(cx, false);
                true
            }
            // Mouse-wheel zoom has no release; pulse a decaying tail so the meter
            // stays live while scroll input keeps arriving.
            Event::Scroll(_) => {
                self.pulse(cx);
                true
            }
            _ => {
                if let Some(ne) = self.next_frame.is_event(event) {
                    self.tick(cx, ne.time);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Toggle the press-span flag. On a rising edge (nothing currently metering)
    /// it primes the sampler; then arms the frame loop so the strength eases in
    /// and (on release) back out.
    fn set_span(&mut self, cx: &mut Cx, on: bool) {
        if on && !self.is_live(cx) {
            self.prime(cx);
        }
        self.metering = on;
        self.next_frame = cx.new_next_frame();
    }

    /// Extend the scroll tail. Re-armed per scroll; primes on a rising edge like
    /// `set_span`.
    fn pulse(&mut self, cx: &mut Cx) {
        if !self.is_live(cx) {
            self.prime(cx);
        }
        self.meter_until = cx.seconds_since_app_start() + SCROLL_METER_TAIL;
        self.next_frame = cx.new_next_frame();
    }

    // Reset the dt clock and skip the first fps sample (a stale idle-gap dt would
    // flash red before the EMA settles).
    fn prime(&mut self, cx: &Cx) {
        self.last_time = cx.seconds_since_app_start();
        self.skip_fps_sample = true;
    }

    fn is_live(&self, cx: &Cx) -> bool {
        self.metering || cx.seconds_since_app_start() < self.meter_until
    }

    fn tick(&mut self, cx: &mut Cx, time: f64) {
        let dt = (time - self.last_time).max(0.0);
        self.last_time = time;
        let active = self.metering || time < self.meter_until;

        // Ease strength toward the metering target.
        let target = if active { 1.0 } else { 0.0 };
        let step = (dt / METER_SECS) as f32;
        if self.strength < target {
            self.strength = (self.strength + step).min(target);
        } else if self.strength > target {
            self.strength = (self.strength - step).max(target);
        }

        // Sample framerate only while metering. Skip the first frame after enable
        // (its dt is a stale idle gap -> false red flash).
        if active {
            if self.skip_fps_sample {
                self.skip_fps_sample = false;
            } else {
                let cdt = dt.clamp(0.001, 0.5);
                let inst_fps = 1.0 / cdt;
                let alpha = 1.0 - (-cdt / FPS_TAU).exp();
                if self.fps == 0.0 {
                    // Seed on the first real sample instead of blending from 0: an
                    // EMA blend here would compute ~= alpha*inst_fps (~6fps),
                    // reading as a false-low red flash before it climbs to
                    // steady-state over ~FPS_TAU. `fps` is always >= 1/0.5 = 2 once
                    // sampling (cdt clamped to <=0.5), so 0.0 unambiguously means
                    // "unseeded".
                    self.fps = inst_fps as f32;
                } else {
                    self.fps = (self.fps as f64 + alpha * (inst_fps - self.fps as f64)) as f32;
                }
                self.color = Self::heat_color(self.fps);
            }
        }

        // Keep the loop armed while metering OR the strength is still easing out;
        // true idle (no meter, faded out) schedules no frames.
        if active || self.strength > 0.0 {
            self.next_frame = cx.new_next_frame();
        }
    }

    /// Map a smoothed framerate (Hz) to a heat colour: green at >=60, amber at
    /// 30, red at <=15, lerped piecewise between. Playful, not calibrated.
    fn heat_color(fps: f32) -> [f32; 3] {
        const GREEN: [f32; 3] = [0.235, 0.745, 0.353];
        const AMBER: [f32; 3] = [0.902, 0.588, 0.078];
        const RED: [f32; 3] = [0.922, 0.275, 0.471];
        fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
            let t = t.clamp(0.0, 1.0);
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        }
        if fps >= 60.0 {
            GREEN
        } else if fps >= 30.0 {
            lerp(AMBER, GREEN, (fps - 30.0) / 30.0)
        } else if fps >= 15.0 {
            lerp(RED, AMBER, (fps - 15.0) / 15.0)
        } else {
            RED
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FpsMeter;

    const GREEN: [f32; 3] = [0.235, 0.745, 0.353];
    const AMBER: [f32; 3] = [0.902, 0.588, 0.078];
    const RED: [f32; 3] = [0.922, 0.275, 0.471];

    fn approx_eq(a: [f32; 3], b: [f32; 3]) -> bool {
        (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4 && (a[2] - b[2]).abs() < 1e-4
    }

    // Anchors: `heat_color` clamps flat to GREEN at/above 60fps and flat to RED
    // at/below 15fps (the `>= 60.0` / final `else` arms).
    #[test]
    fn anchors_green_at_and_above_60() {
        assert!(approx_eq(FpsMeter::heat_color(60.0), GREEN));
        assert!(approx_eq(FpsMeter::heat_color(120.0), GREEN));
    }

    #[test]
    fn anchors_red_at_and_below_15() {
        assert!(approx_eq(FpsMeter::heat_color(15.0), RED));
        assert!(approx_eq(FpsMeter::heat_color(5.0), RED));
        assert!(approx_eq(FpsMeter::heat_color(0.0), RED));
    }

    // Amber anchor at the 30fps boundary. The branch order in `heat_color`
    // checks `fps >= 30.0` before `fps >= 15.0`, so 30.0 itself is owned by
    // the amber->green lerp: `lerp(AMBER, GREEN, (30-30)/30)` = AMBER exactly.
    // The red->amber lerp's far end agrees with the same value --
    // `lerp(RED, AMBER, (30-15)/15)` = AMBER -- so the ramp is continuous
    // there even though only one branch is ever actually evaluated at 30.0.
    #[test]
    fn amber_at_30_boundary() {
        assert!(approx_eq(FpsMeter::heat_color(30.0), AMBER));
        // Just below the boundary (still the red->amber branch): confirms the
        // two pieces meet at the same colour instead of stepping.
        assert!(approx_eq(FpsMeter::heat_color(29.999), AMBER));
    }

    // Sweep the whole ramp: every channel must stay in [0,1] (guaranteed by
    // `lerp`'s `t.clamp(0.0, 1.0)` plus endpoints that are themselves in
    // range), and the green channel must never decrease as fps rises --
    // RED.g=0.275 < AMBER.g=0.588 < GREEN.g=0.745, and each lerp interpolates
    // straight between adjacent anchors, so this holds across the whole
    // sweep. (The red and blue channels do NOT hold this property -- e.g.
    // blue dips from RED.b=0.471 down to AMBER.b=0.078 then rises back to
    // GREEN.b=0.353 -- so only the green channel's monotonicity is asserted.)
    #[test]
    fn ramp_is_in_range_and_green_channel_nondecreasing() {
        let mut prev_green = f32::MIN;
        let mut fps = 0.0f32;
        while fps <= 90.0 {
            let c = FpsMeter::heat_color(fps);
            for &ch in &c {
                assert!((0.0..=1.0).contains(&ch), "channel out of [0,1] at fps={fps}: {c:?}");
            }
            assert!(
                c[1] + 1e-6 >= prev_green,
                "green channel decreased at fps={fps}: {} < {prev_green}",
                c[1]
            );
            prev_green = c[1];
            fps += 0.5;
        }
    }
}
