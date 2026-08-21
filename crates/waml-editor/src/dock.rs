//! Pure state model for the docked/collapsible Model + Inspector panels. Holds
//! the `DockState` enum, its transition table (`next`), the open/close motion
//! curve (`DockMotion`), and the slot/center width arithmetic that makes Pin
//! shrink the center. No makepad types here, so it is unit-tested standalone.
//!
//! Both panels are binary: a panel is either a `Flag` (nothing drawn) or a
//! `Pinned` column that shrinks the center. The old floating `Peek` state and
//! its auto-collapse timer were deleted along with the `peek_layer` that hosted
//! them.

/// Which visual state a dock panel is in. Replaces the panels' old separate
/// `collapsed` / `pinned` / `folded` bools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DockState {
    /// Resting: a thin sideways-label strip at the body edge, no body drawn.
    #[default]
    Flag,
    /// Docked column: consumes layout width, the center shrinks, sticky.
    Pinned,
}

/// A user/pointer event that may transition a `DockState`. See the plan's
/// authoritative transition table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockEvent {
    /// A shell dock toggle (the caption tree button or document-header
    /// right-dock button). Moves Flag <-> Pinned.
    Toggle,
    /// The responsive shell forced a dock open, idempotently. Drives any state to
    /// `Pinned` and never collapses.
    Open,
    /// The shell forced the panel shut because the active view declares no such
    /// dock (`BodyChrome.right_dock == None` -- the last tab was closed, or a
    /// model with no diagrams was opened). The mirror of `Open`: it drives ANY
    /// state to `Flag` and never expands. Not a user gesture -- with the flag
    /// spine and pin button gone, the caption toggle is the only affordance,
    /// and it is hidden in exactly this case, so a still-`Pinned` column would
    /// strand on screen with no way to close it.
    Close,
}

/// The transition table.
pub fn next(state: DockState, ev: DockEvent) -> DockState {
    use DockEvent::*;
    use DockState::*;
    match (state, ev) {
        (Flag, Toggle) => Pinned,
        (Pinned, Toggle) => Flag,
        (_, Open) => Pinned,
        (_, Close) => Flag,
    }
}

/// Apply `ev` to `state` in place. Returns `true` when the state changed --
/// the caller's cue to redraw. The single shim behind both dock panels'
/// `apply_dock` (they were byte-identical copies before).
pub fn apply(state: &mut DockState, ev: DockEvent) -> bool {
    let next = next(*state, ev);
    if next == *state {
        return false;
    }
    *state = next;
    true
}

/// The layout width a panel's slot reserves in the `flow: Right` dock row.
/// Only `Pinned` reserves a column and thereby shrinks the center; `Flag`
/// reserves nothing (a collapsed panel draws zero pixels and must not carve a
/// window-bg gutter down the edge). Test seam: the shell animates its slots
/// through `responsive_layout`, which interpolates the same widths.
#[cfg(test)]
pub fn slot_width(state: DockState, body_w: f64) -> f64 {
    match state {
        DockState::Flag => 0.0,
        DockState::Pinned => body_w,
    }
}

pub const DOCK_MOTION_SECS: f64 = 0.180;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockMotion {
    value: f64,
    from: f64,
    target: f64,
    started_at: f64,
    active: bool,
}

impl Default for DockMotion {
    fn default() -> Self {
        Self::new(0.0)
    }
}

impl DockMotion {
    pub fn new(value: f64) -> Self {
        let value = value.clamp(0.0, 1.0);
        Self {
            value,
            from: value,
            target: value,
            started_at: 0.0,
            active: false,
        }
    }

    pub fn request(&mut self, target: f64, now: f64) {
        self.sample(now);
        let target = target.clamp(0.0, 1.0);
        if target == self.target {
            return;
        }
        self.from = self.value;
        self.target = target;
        self.started_at = now;
        self.active = self.from != self.target;
    }

    pub fn sample(&mut self, now: f64) -> f64 {
        if !self.active {
            return self.value;
        }
        let t = ((now - self.started_at) / DOCK_MOTION_SECS).clamp(0.0, 1.0);
        let u = 1.0 - t;
        let eased = 1.0 - u * u * u;
        self.value = self.from + (self.target - self.from) * eased;
        if t >= 1.0 {
            self.value = self.target;
            self.active = false;
        }
        self.value
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

pub fn presentation_visible(value: f64) -> bool {
    value > 0.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResponsiveDockLayout {
    pub left_slot: f64,
    pub right_slot: f64,
    pub tree_body: f64,
    pub inspector_body: f64,
}

pub fn responsive_layout(
    narrow: bool,
    viewport_w: f64,
    tree_value: f64,
    inspector_value: f64,
    tree_w: f64,
    inspector_w: f64,
) -> ResponsiveDockLayout {
    let cap = viewport_w.max(0.0);
    let tree_target = if narrow { tree_w.min(cap) } else { tree_w };
    let inspector_target = if narrow {
        inspector_w.min(cap)
    } else {
        inspector_w
    };
    let tree_body = tree_target * tree_value.clamp(0.0, 1.0);
    let inspector_body = inspector_target * inspector_value.clamp(0.0, 1.0);
    ResponsiveDockLayout {
        left_slot: if narrow { 0.0 } else { tree_body },
        right_slot: if narrow { 0.0 } else { inspector_body },
        tree_body,
        inspector_body,
    }
}

pub fn narrow_entry_states(tree: DockState, inspector: DockState) -> (DockState, DockState) {
    if tree == DockState::Pinned && inspector == DockState::Pinned {
        (tree, DockState::Flag)
    } else {
        (tree, inspector)
    }
}

/// In narrow mode the inspector floats above the center column, so its host
/// starts below a visible document header. Wide mode remains a full-height
/// reserved column beside the center.
pub fn narrow_inspector_top(narrow: bool, header_height: f64) -> f64 {
    if narrow {
        header_height.max(0.0)
    } else {
        0.0
    }
}

/// Which window edge a dock panel is anchored to. The Model tree docks left,
/// the Inspector docks right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockEdge {
    Left,
    Right,
}

pub fn narrow_toggle_states(
    tree: DockState,
    inspector: DockState,
    edge: DockEdge,
) -> (DockState, DockState) {
    match edge {
        DockEdge::Left => {
            let next_tree = next(tree, DockEvent::Toggle);
            if next_tree == DockState::Pinned {
                (next_tree, DockState::Flag)
            } else {
                (next_tree, inspector)
            }
        }
        DockEdge::Right => {
            let next_inspector = next(inspector, DockEvent::Toggle);
            if next_inspector == DockState::Pinned {
                (DockState::Flag, next_inspector)
            } else {
                (tree, next_inspector)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn narrow_inspector_starts_below_only_a_visible_header() {
        assert_eq!(narrow_inspector_top(true, 30.0), 30.0);
        assert_eq!(narrow_inspector_top(true, 0.0), 0.0);
        assert_eq!(narrow_inspector_top(false, 30.0), 0.0);
    }

    #[test]
    fn wide_and_narrow_layout_use_the_same_final_widths() {
        let wide = responsive_layout(false, 900.0, 1.0, 1.0, 280.0, 320.0);
        assert_eq!(
            wide,
            ResponsiveDockLayout {
                left_slot: 280.0,
                right_slot: 320.0,
                tree_body: 280.0,
                inspector_body: 320.0,
            }
        );
        let narrow = responsive_layout(true, 390.0, 1.0, 0.0, 280.0, 320.0);
        assert_eq!(
            narrow,
            ResponsiveDockLayout {
                left_slot: 0.0,
                right_slot: 0.0,
                tree_body: 280.0,
                inspector_body: 0.0,
            }
        );
    }

    #[test]
    fn narrow_body_width_is_capped_to_the_viewport() {
        let layout = responsive_layout(true, 240.0, 1.0, 0.0, 280.0, 320.0);
        assert_eq!(layout.tree_body, 240.0);
    }

    #[test]
    fn wide_layout_interpolates_slots_and_bodies_together() {
        assert_eq!(
            responsive_layout(false, 900.0, 0.5, 0.25, 280.0, 320.0),
            ResponsiveDockLayout {
                left_slot: 140.0,
                right_slot: 80.0,
                tree_body: 140.0,
                inspector_body: 80.0,
            }
        );
    }

    #[test]
    fn narrow_layout_keeps_slots_zero_and_animates_capped_bodies() {
        assert_eq!(
            responsive_layout(true, 240.0, 0.5, 1.0, 280.0, 320.0),
            ResponsiveDockLayout {
                left_slot: 0.0,
                right_slot: 0.0,
                tree_body: 120.0,
                inspector_body: 240.0,
            }
        );
    }

    #[test]
    fn panel_content_stays_visible_until_motion_reaches_zero() {
        assert!(presentation_visible(1.0));
        assert!(presentation_visible(0.001));
        assert!(!presentation_visible(0.0));
    }

    #[test]
    fn dock_motion_has_exact_endpoints_and_completes_at_180_ms() {
        let mut motion = DockMotion::new(0.0);
        assert_eq!(motion.value(), 0.0);
        motion.request(1.0, 0.0);
        assert_eq!(motion.sample(0.0), 0.0);
        assert!(motion.sample(0.179) < 1.0);
        assert_eq!(motion.sample(0.180), 1.0);
        assert!(!motion.is_active());
    }

    #[test]
    fn dock_motion_is_monotonic_with_cubic_ease_out() {
        let mut motion = DockMotion::new(0.0);
        motion.request(1.0, 0.0);
        let samples = [0.0, 0.03, 0.06, 0.09, 0.12, 0.15, 0.18].map(|time| motion.sample(time));
        assert!(samples.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(samples[0], 0.0);
        assert_eq!(samples[6], 1.0);
        assert!(samples[3] > 0.5, "ease-out must lead linear interpolation");
    }

    #[test]
    fn reversing_motion_starts_from_the_sampled_in_flight_value() {
        let mut motion = DockMotion::new(0.0);
        motion.request(1.0, 0.0);
        let before_reverse = motion.sample(0.09);
        motion.request(0.0, 0.09);
        assert_eq!(motion.value(), before_reverse);
        assert!(motion.sample(0.10) < before_reverse);
        assert_eq!(motion.sample(0.27), 0.0);
    }

    #[test]
    fn repeated_target_requests_do_not_restart_motion() {
        let mut motion = DockMotion::new(0.0);
        motion.request(1.0, 0.0);
        let at_sixty_ms = motion.sample(0.06);
        motion.request(1.0, 0.09);
        assert!(motion.value() > at_sixty_ms);
        assert_eq!(motion.sample(0.18), 1.0);
    }

    #[test]
    fn entering_narrow_with_two_open_docks_keeps_tree() {
        assert_eq!(
            narrow_entry_states(DockState::Pinned, DockState::Pinned),
            (DockState::Pinned, DockState::Flag)
        );
        assert_eq!(
            narrow_entry_states(DockState::Flag, DockState::Pinned),
            (DockState::Flag, DockState::Pinned)
        );
    }

    #[test]
    fn narrow_toggles_are_mutually_exclusive() {
        assert_eq!(
            narrow_toggle_states(DockState::Flag, DockState::Pinned, DockEdge::Left),
            (DockState::Pinned, DockState::Flag)
        );
        assert_eq!(
            narrow_toggle_states(DockState::Pinned, DockState::Flag, DockEdge::Right),
            (DockState::Flag, DockState::Pinned)
        );
        assert_eq!(
            narrow_toggle_states(DockState::Pinned, DockState::Flag, DockEdge::Left),
            (DockState::Flag, DockState::Flag)
        );
    }

    #[test]
    fn toggle_flips_between_flag_and_pinned() {
        use DockEvent::*;
        use DockState::*;
        assert_eq!(next(Flag, Toggle), Pinned);
        assert_eq!(next(Pinned, Toggle), Flag);
    }

    #[test]
    fn open_is_idempotent_and_never_collapses() {
        use DockEvent::*;
        use DockState::*;
        // Request-only: a view can ask for its panel, never for its collapse,
        // so a user who closed it isn't fought by the next click. Every state
        // lands on Pinned, including Pinned itself (a no-op, so no redraw).
        assert_eq!(next(Flag, Open), Pinned);
        assert_eq!(next(Pinned, Open), Pinned);
    }

    #[test]
    fn close_is_idempotent_and_never_opens() {
        use DockEvent::*;
        use DockState::*;
        // The mirror of `Open`: the shell forces a panel shut when the active
        // view declares no such dock (the last tab closed, or a model with no
        // diagrams opened). Every state lands on Flag, including Flag itself (a
        // no-op, so no redraw).
        assert_eq!(next(Pinned, Close), Flag);
        assert_eq!(next(Flag, Close), Flag);
    }

    #[test]
    fn apply_reports_only_real_changes() {
        let mut s = DockState::default();
        assert_eq!(s, DockState::Flag);
        assert!(apply(&mut s, DockEvent::Toggle));
        assert_eq!(s, DockState::Pinned);
        assert!(!apply(&mut s, DockEvent::Open));
        assert!(apply(&mut s, DockEvent::Close));
        assert_eq!(s, DockState::Flag);
    }

    #[test]
    fn slot_width_only_pinned_reserves_body() {
        assert_eq!(slot_width(DockState::Flag, 280.0), 0.0);
        assert_eq!(slot_width(DockState::Pinned, 280.0), 280.0);
    }

    #[test]
    fn pinning_shrinks_center_by_exactly_slot_delta() {
        // The center is Fill = total - left_slot - right_slot. Pinning the left
        // Model panel must shrink the center by exactly its body width.
        let total = 1280.0;
        let right = slot_width(DockState::Flag, 320.0); // inspector at rest
        let center_flag = total - slot_width(DockState::Flag, 280.0) - right;
        let center_pinned = total - slot_width(DockState::Pinned, 280.0) - right;
        assert_eq!(center_flag - center_pinned, 280.0);
    }
}
