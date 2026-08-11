//! Headless zoom ladder shared by the reading view and the raw-markdown
//! source editor. See `docs/superpowers/specs/2026-08-11-viewer-font-size-control-design.md`.

/// Discrete zoom rungs, spec §The ladder.
pub(crate) const ZOOM_LADDER: [u32; 10] = [50, 67, 75, 90, 100, 110, 125, 150, 175, 200];

/// Default zoom percent (100%).
pub(crate) const ZOOM_DEFAULT: u32 = 100;

/// Which surface a zoom target applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZoomTarget {
    Reading,
    Source,
}

/// Snap any percent to the closest rung on the ladder. Ties resolve downward
/// (the lower rung wins on an exact midpoint).
pub(crate) fn nearest_rung(percent: u32) -> u32 {
    let mut best = ZOOM_LADDER[0];
    let mut best_dist = best.abs_diff(percent);
    for &rung in ZOOM_LADDER.iter().skip(1) {
        let dist = rung.abs_diff(percent);
        if dist < best_dist {
            best = rung;
            best_dist = dist;
        }
    }
    best
}

/// The ladder index of a rung value, snapping first.
fn ladder_index(percent: u32) -> usize {
    let snapped = nearest_rung(percent);
    ZOOM_LADDER.iter().position(|&r| r == snapped).unwrap_or(0)
}

/// Step to the next rung up, saturating at the top of the ladder.
pub(crate) fn zoom_in(percent: u32) -> u32 {
    let idx = ladder_index(percent);
    let next = (idx + 1).min(ZOOM_LADDER.len() - 1);
    ZOOM_LADDER[next]
}

/// Step to the next rung down, saturating at the bottom of the ladder.
pub(crate) fn zoom_out(percent: u32) -> u32 {
    let idx = ladder_index(percent);
    let next = idx.saturating_sub(1);
    ZOOM_LADDER[next]
}

/// Convert a zoom percent into a scale multiplier.
pub(crate) fn scale(percent: u32) -> f64 {
    percent as f64 / 100.0
}

/// A wheel event's worth of scroll delta needed before it steps one rung on
/// the ladder. Trackpad two-finger scrolls arrive as many small deltas, so
/// this is sized for those.
///
/// A physical wheel notch is far larger than one step -- Windows delivers
/// `WHEEL_DELTA` (120) per detent (`win32_window.rs`'s `WM_MOUSEWHEEL`), and
/// the web backend is the same order -- so `add` also CLAMPS to one rung per
/// event. Without that clamp a single detent walks three rungs at once and
/// the control is unusable with a mouse, while a precise trackpad behaves;
/// that asymmetry is the bug this pairing prevents.
const WHEEL_STEP: f64 = 40.0;

/// Accumulates signed wheel deltas from `Event::Scroll` into whole-rung zoom
/// steps. `add` returns the signed count of rungs to step: negative for
/// zoom-IN (wheel up / negative `scroll.y`), positive for zoom-OUT. A
/// direction change (the running total and the new delta disagree in sign)
/// discards the stale residual first, so a quick reversal doesn't carry over
/// spurious momentum from the other direction.
#[derive(Default)]
pub(crate) struct WheelAccumulator {
    accumulated: f64,
}

impl WheelAccumulator {
    pub(crate) fn add(&mut self, delta: f64) -> i32 {
        if delta == 0.0 {
            return 0;
        }
        if self.accumulated != 0.0 && self.accumulated.signum() != delta.signum() {
            self.accumulated = 0.0;
        }
        self.accumulated += delta;
        if self.accumulated.abs() < WHEEL_STEP {
            return 0;
        }
        // One rung per event, however big the delta: a mouse detent (120 on
        // Windows) and a coalesced free-spin (the fork sums adjacent wheel
        // messages and clamps at i16::MAX) must not walk the whole ladder in
        // one frame. The residual is dropped rather than carried so a fast
        // spin cannot bank steps for later.
        let direction = self.accumulated.signum();
        self.accumulated = 0.0;
        direction as i32
    }

    /// Drops any part-way accumulation. Called when the zoom target changes,
    /// so a sub-threshold nudge in one surface cannot step the ladder in a
    /// different one.
    pub(crate) fn reset(&mut self) {
        self.accumulated = 0.0;
    }
}

/// The live zoom percent per target. **Memory is the truth; the config file is
/// write-through persistence, seeded once per target on first use.**
///
/// Reading the file back on every command instead would be wrong twice over:
/// each step costs a synchronous read (plus a rewrite), and because
/// `config::set_*_zoom` is deliberately best-effort -- it logs and swallows a
/// write failure -- a read-only or full disk would leave the applied scale and
/// the persisted value permanently one rung apart, with each further press
/// re-deriving from the stale file. The UI must not depend on the write
/// landing.
#[derive(Default)]
pub(crate) struct ZoomState {
    reading: Option<u32>,
    source: Option<u32>,
}

impl ZoomState {
    pub(crate) fn get(&mut self, target: ZoomTarget) -> u32 {
        match target {
            ZoomTarget::Reading => *self
                .reading
                .get_or_insert_with(crate::config::reading_zoom),
            ZoomTarget::Source => *self.source.get_or_insert_with(crate::config::source_zoom),
        }
    }

    pub(crate) fn set(&mut self, target: ZoomTarget, percent: u32) {
        match target {
            ZoomTarget::Reading => {
                self.reading = Some(percent);
                crate::config::set_reading_zoom(percent);
            }
            ZoomTarget::Source => {
                self.source = Some(percent);
                crate::config::set_source_zoom(percent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stepping_walks_the_ladder_and_saturates() {
        assert_eq!(zoom_in(100), 110);
        assert_eq!(zoom_out(100), 90);
        assert_eq!(zoom_in(200), 200);
        assert_eq!(zoom_out(50), 50);
        // stepping from an off-ladder value snaps first, then steps
        assert_eq!(zoom_in(105), 110);
        assert_eq!(zoom_out(105), 90);
    }

    #[test]
    fn nearest_rung_snaps_off_ladder_and_absurd_values() {
        assert_eq!(nearest_rung(100), 100);
        assert_eq!(nearest_rung(103), 100);
        assert_eq!(nearest_rung(118), 125);
        assert_eq!(nearest_rung(0), 50);
        assert_eq!(nearest_rung(u32::MAX), 200);
    }

    #[test]
    fn scale_is_percent_over_100() {
        assert_eq!(scale(100), 1.0);
        assert_eq!(scale(50), 0.5);
        assert_eq!(scale(175), 1.75);
    }

    #[test]
    fn accumulator_steps_once_per_threshold_and_resets_on_direction_change() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.add(-15.0), 0);
        assert_eq!(acc.add(-30.0), -1); // crossed 40 upward-zoom
        assert_eq!(acc.add(10.0), 0); // direction change resets, no step
        assert_eq!(acc.add(35.0), 1);
    }

    #[test]
    fn one_mouse_detent_steps_exactly_one_rung() {
        // Windows sends WHEEL_DELTA (120) per detent; the web backend is the
        // same order of magnitude. Three rungs per notch made the control
        // unusable with a mouse while a trackpad behaved.
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.add(-120.0), -1);
        assert_eq!(acc.add(-120.0), -1);
        assert_eq!(acc.add(120.0), 1);
    }

    #[test]
    fn a_coalesced_free_spin_still_steps_one_rung() {
        // The fork sums adjacent wheel messages into one event, clamped at
        // i16::MAX -- that must not walk the whole ladder in a single frame.
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.add(-32767.0), -1);
    }

    #[test]
    fn a_step_banks_no_residual_for_the_next_event() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.add(-79.0), -1);
        // The leftover 39 is dropped, so the next small nudge is below the
        // threshold on its own.
        assert_eq!(acc.add(-20.0), 0);
    }

    #[test]
    fn reset_discards_a_part_way_accumulation() {
        let mut acc = WheelAccumulator::default();
        assert_eq!(acc.add(-35.0), 0);
        acc.reset();
        assert_eq!(acc.add(-20.0), 0);
    }
}
