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
}
