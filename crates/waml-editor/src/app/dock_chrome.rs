//! Everything `App` remembers about the two docked columns and the caption
//! chrome that tracks them.
//!
//! [`crate::dock`] is the pure state model -- the `Flag`/`Pinned` transition
//! table, the motion curve, the slot arithmetic -- and knows nothing about a
//! running editor. This module is the layer above it: the *session's* copy of
//! that model, the widths it was seeded with, the responsive mode it is in, and
//! the last-applied values that keep `sync_dock_slots` from rewriting widget
//! walks it has already written. It holds makepad types (`NextFrame`, `Vec4`),
//! so it cannot live in `dock.rs`, and it holds no widget refs, so every method
//! here is testable without a `Cx`.
//!
//! # Invariants
//!
//! * **`widths` is the truth; `layout` is derived.** `widths` is what the user
//!   dragged and what `.waml/editor.json` persists. `layout` is what
//!   [`DockChrome::layout_for`] computes from it each frame -- never the other
//!   way round. `layout` is stored ONLY as the change guard for the walk
//!   writes; anything that rebuilds the widget tree behind our back must call
//!   [`DockChrome::invalidate_layout`] so the next sync writes unconditionally.
//! * **`rubber` is gesture state, never a width.** It is non-zero only for the
//!   span of a splitter drag that has snapped its panel shut, and
//!   [`DockChrome::release_drag`] zeroes it the moment the finger lifts. It is
//!   never persisted, and narrow mode -- which reserves no slots and hides the
//!   splitters -- never applies it.
//! * **The motion values are sampled, not stored twice.** `tree_motion` and
//!   `inspector_motion` are driven once per sync by
//!   [`DockChrome::drive_motions`], whose return value is the only reading the
//!   caller should use for that frame; `presentation_visible` and `layout_for`
//!   must agree, and they can only agree if both come off the same sample.
//! * **The "last applied" fields start deliberately out of range** so the first
//!   sync of a session always writes: `tree_btn_slot_w` starts negative (no
//!   real slot width is), `seam_break` starts `None`, `layout` starts at
//!   `Default` (all four columns zero).
//! * **The tree column starts open.** `tree_motion` seeds at `1.0`, not `0.0`:
//!   a session opens with the model column already presented, and seeding it
//!   shut would animate it open on the first frame.

use super::*;

/// Footprint of one slot-carried caption control: the DSL `width` (30) plus its
/// 2px margin. The burger and the tree toggle are both this wide and ride the
/// slot in that order -- the toggle ending on the split, the burger just past
/// it -- so the slot is sized against the toggle's copy of it (see
/// [`tree_toggle_layout`]).
pub(super) const TREE_BTN_W: f64 = 32.0;

/// `tab_row`'s x within the caption, as the DSL declares it: `title_row`'s 2px
/// left padding and the 44px logo. The burger is NOT part of the lead any more
/// -- it moved into the row, onto the slot. Only used before the row has a drawn
/// rect to measure (see `App::sync_dock_slots`).
pub(super) const DEFAULT_TAB_ROW_LEAD_W: f64 = 46.0;

pub(super) const NARROW_ENTER_W: f64 = 640.0;
pub(super) const NARROW_EXIT_W: f64 = 680.0;

/// Width a dock panel's BODY draws at inside a host of `host_w`: the host
/// width less the splitter strip that shares the host with it. Floored at zero
/// so a mid-animation host narrower than the strip cannot go negative.
///
/// This is the spec's stated consequence of mounting the splitter inside the
/// panel host: the stored/persisted width is the whole column, and the body is
/// `SPLITTER_W` narrower than that number.
pub(super) fn panel_body_w(host_w: f64) -> f64 {
    (host_w - crate::panel_splitter::SPLITTER_W).max(0.0)
}

/// Responsive mode for the next frame, with hysteresis: the two thresholds are
/// deliberately apart so a window parked on the boundary cannot oscillate.
pub(super) fn next_narrow(narrow: bool, viewport_w: f64) -> bool {
    if narrow {
        viewport_w <= NARROW_EXIT_W
    } else {
        viewport_w < NARROW_ENTER_W
    }
}

/// Where the tree-column toggle sits, as `(visible, slot_w)`.
///
/// There is ONE toggle and it lives in the caption's tab row. `tree_btn_slot`
/// is what moves it: an empty runtime-sized spacer leading the row, with the
/// button as its next sibling.
///
/// The slot is a WIDTH rather than a flag because a flag is what made this
/// jerk. Open, the button ends where the tree column does, so the history pair
/// after it starts on the column's right edge; collapsed, the slot closes to
/// nothing and the button leads the row. Both ends are the same continuous
/// number, so nothing jumps at the handoff -- the arrangement this replaced
/// faded a second, panel-docked button in at the end of the collapse and added
/// its 32px in a single frame.
///
/// `lead_w` is what the logo costs before the row's turtle starts; the
/// slot only makes up the difference to the column's edge. Lerped off the
/// RESERVATION rather than the animating body, which keeps the run monotonic (a
/// body-derived target dips below the button's own width while the column is
/// narrower than `lead_w`).
///
/// Narrow is the exception: the panel floats instead of reserving, so the column
/// has no edge to sit on and the button just leads the row throughout.
pub(super) fn tree_toggle_layout(
    mounted: bool,
    narrow: bool,
    tree_body: f64,
    tree_w: f64,
    lead_w: f64,
) -> (bool, f64) {
    if !mounted {
        return (false, 0.0);
    }
    if narrow {
        return (true, 0.0);
    }
    // `tree_w` is the column's LIVE reserved width, not the compile-time
    // default: a drag that resizes the column moves body and reservation
    // together, so `progress` stays 1 and the button tracks the edge without
    // sliding. Only a snap (collapse/reopen), which animates the body against a
    // fixed reservation, runs `progress` between the ends.
    let progress = if tree_w > 0.0 {
        (tree_body / tree_w).clamp(0.0, 1.0)
    } else {
        0.0
    };
    // The toggle's own footprint comes off the target: the slot leads it, so it
    // is `lead_w + slot + TREE_BTN_W` that has to land on the column's edge --
    // the toggle sits flush with the split, and the burger trailing it is the
    // first thing right of the split.
    let open_w = (tree_w - lead_w - TREE_BTN_W).max(0.0);
    (true, open_w * progress)
}

/// Session state for the docked Model + Inspector columns and the caption
/// chrome that tracks them. See the module docs for the invariants that hold
/// over these fields together.
pub(super) struct DockChrome {
    /// Responsive mode: narrow floats the panels over the center instead of
    /// reserving slots for them. Driven only by [`Self::reconcile_narrow`].
    narrow: bool,
    /// User-dragged widths of the two columns, seeded from the open project's
    /// `.waml/editor.json` and persisted back on drag release.
    widths: crate::project_settings::DockWidths,
    /// Last layout pushed into the slot/host/body walks -- a change guard, not
    /// a source of truth.
    layout: ResponsiveDockLayout,
    /// Springy give, in px, currently shown by a collapsed-but-still-held
    /// panel: `(tree, inspector)`.
    rubber: (f64, f64),
    tree_motion: DockMotion,
    inspector_motion: DockMotion,
    /// Last-applied `tree_btn_slot` width. Written every frame of a dock
    /// motion, so it carries its own guard rather than riding `layout`'s.
    tree_btn_slot_w: f64,
    /// Last-applied `ChromeSeam` break. Change-guard only -- the seam owns the
    /// value it draws with.
    seam_break: Option<(f64, f64, Vec4)>,
    /// Whether a model is open, and so whether the tree-column toggle should be
    /// showing at all. The gate above [`Self::tree_toggle_layout`], so the
    /// open/close paths do not have to know how the button is seated.
    toggle_mounted: bool,
}

impl Default for DockChrome {
    fn default() -> Self {
        Self {
            narrow: false,
            widths: crate::project_settings::DockWidths::default(),
            layout: ResponsiveDockLayout::default(),
            rubber: (0.0, 0.0),
            // The model column opens presented; see the module invariants.
            tree_motion: DockMotion::new(1.0),
            inspector_motion: DockMotion::default(),
            // Negative so the first sync always writes.
            tree_btn_slot_w: -1.0,
            seam_break: None,
            toggle_mounted: false,
        }
    }
}

impl DockChrome {
    pub(super) fn is_narrow(&self) -> bool {
        self.narrow
    }

    /// Reconcile the responsive mode against the live viewport, returning
    /// `true` when the mode actually flipped (and so when the caller owes the
    /// widget-side half of the transition).
    pub(super) fn reconcile_narrow(&mut self, viewport_w: f64) -> bool {
        let next = next_narrow(self.narrow, viewport_w);
        if next == self.narrow {
            return false;
        }
        self.narrow = next;
        true
    }

    /// Pin the responsive mode without a window resize. Test-only: production
    /// reaches narrow mode exclusively through [`Self::reconcile_narrow`], so
    /// the mode can never disagree with the viewport that produced it.
    #[cfg(test)]
    pub(super) fn force_narrow(&mut self, narrow: bool) {
        self.narrow = narrow;
    }

    pub(super) fn widths(&self) -> crate::project_settings::DockWidths {
        self.widths
    }

    pub(super) fn set_widths(&mut self, widths: crate::project_settings::DockWidths) {
        self.widths = widths;
    }

    /// Column widths are per-project state: closing the project returns them to
    /// the defaults the next one starts from unless it has its own.
    pub(super) fn reset_widths(&mut self) {
        self.widths = crate::project_settings::DockWidths::default();
    }

    pub(super) fn layout(&self) -> ResponsiveDockLayout {
        self.layout
    }

    /// Drive both open/close animations to `now` and hand back the values this
    /// frame must be built from. The single sample point -- see the module
    /// invariants.
    pub(super) fn drive_motions(
        &mut self,
        tree_pinned: bool,
        inspector_pinned: bool,
        now: f64,
    ) -> (f64, f64) {
        self.tree_motion
            .request(if tree_pinned { 1.0 } else { 0.0 }, now);
        self.inspector_motion
            .request(if inspector_pinned { 1.0 } else { 0.0 }, now);
        (self.tree_motion.value(), self.inspector_motion.value())
    }

    pub(super) fn motions_active(&self) -> bool {
        self.tree_motion.is_active() || self.inspector_motion.is_active()
    }

    /// Seat both animations at their end states without running them. Test-only
    /// -- production drives them through [`Self::drive_motions`].
    #[cfg(test)]
    pub(super) fn seat_motions(&mut self, tree_pinned: bool, inspector_pinned: bool) {
        self.tree_motion = DockMotion::new(if tree_pinned { 1.0 } else { 0.0 });
        self.inspector_motion = DockMotion::new(if inspector_pinned { 1.0 } else { 0.0 });
    }

    /// The layout the columns should draw at for this viewport, off the motion
    /// values last handed out by [`Self::drive_motions`], with the drag's
    /// springy give folded in.
    ///
    /// The give is applied to the SLOT as well as the body so the sliver pushes
    /// the canvas rather than floating over it, keeping the drag physically
    /// honest. Narrow mode reserves no slots and hides the splitters, so it has
    /// no give.
    pub(super) fn layout_for(&self, viewport_w: f64) -> ResponsiveDockLayout {
        let mut layout = crate::dock::responsive_layout(
            self.narrow,
            viewport_w,
            self.tree_motion.value(),
            self.inspector_motion.value(),
            self.widths.tree_w,
            self.widths.inspector_w,
        );
        if !self.narrow {
            let (tree_rubber, inspector_rubber) = self.rubber;
            layout.tree_body = crate::splitter::with_rubber(layout.tree_body, tree_rubber);
            layout.left_slot = crate::splitter::with_rubber(layout.left_slot, tree_rubber);
            layout.inspector_body =
                crate::splitter::with_rubber(layout.inspector_body, inspector_rubber);
            layout.right_slot = crate::splitter::with_rubber(layout.right_slot, inspector_rubber);
        }
        layout
    }

    /// Record `layout` as applied, returning `true` when it differs from what
    /// the walks already carry (and so when the caller must write them).
    pub(super) fn commit_layout(&mut self, layout: ResponsiveDockLayout) -> bool {
        if layout == self.layout {
            return false;
        }
        self.layout = layout;
        true
    }

    /// Forget the applied layout, so the next sync writes the walks
    /// unconditionally. Called wherever the widget tree is rebuilt underneath
    /// us (a session swap, a theme live-edit reload) and the walks we think we
    /// wrote are gone.
    pub(super) fn invalidate_layout(&mut self) {
        self.layout = ResponsiveDockLayout::default();
    }

    /// Record `w` as the applied `tree_btn_slot` width, returning `true` when
    /// it moved far enough to be worth a write. Sub-pixel motion frames are
    /// dropped here rather than at the call site.
    pub(super) fn commit_tree_slot_w(&mut self, w: f64) -> bool {
        if (w - self.tree_btn_slot_w).abs() <= 0.01 {
            return false;
        }
        self.tree_btn_slot_w = w;
        true
    }

    /// Record `span` as the applied `ChromeSeam` break, returning `true` when
    /// it changed. Compared with a half-pixel tolerance on the ends, since the
    /// span is measured off drawn card rects and settles a frame late.
    pub(super) fn commit_seam_break(&mut self, span: Option<(f64, f64, Vec4)>) -> bool {
        let changed = match (span, self.seam_break) {
            (None, None) => false,
            (Some((a0, a1, ac)), Some((b0, b1, bc))) => {
                (a0 - b0).abs() > 0.5 || (a1 - b1).abs() > 0.5 || ac != bc
            }
            _ => true,
        };
        if !changed {
            return false;
        }
        self.seam_break = span;
        true
    }

    pub(super) fn set_toggle_mounted(&mut self, mounted: bool) {
        self.toggle_mounted = mounted;
    }

    /// Where the tree-column toggle sits this frame, as `(visible, slot_w)`.
    /// Reads the LIVE reserved width off `widths`, not the compile-time
    /// default; see [`tree_toggle_layout`].
    pub(super) fn tree_toggle_layout(&self, lead_w: f64) -> (bool, f64) {
        tree_toggle_layout(
            self.toggle_mounted,
            self.narrow,
            self.layout.tree_body,
            self.widths.tree_w,
            lead_w,
        )
    }

    /// One splitter drag frame's outcome, folded into the widths and the give.
    /// Returns the dock transition the outcome implies, if any -- collapse and
    /// reopen go through the ordinary `DockEvent` path so `DockMotion` animates
    /// the snap and `DockState` stays the single source of truth.
    pub(super) fn apply_drag(
        &mut self,
        edge: crate::dock::DockEdge,
        outcome: crate::splitter::DragOutcome,
    ) -> Option<crate::dock::DockEvent> {
        use crate::dock::{DockEdge, DockEvent};
        use crate::splitter::DragOutcome;

        let set_width = |widths: &mut crate::project_settings::DockWidths, w: f64| match edge {
            DockEdge::Left => widths.tree_w = w,
            DockEdge::Right => widths.inspector_w = w,
        };
        let set_rubber = |rubber: &mut (f64, f64), r: f64| match edge {
            DockEdge::Left => rubber.0 = r,
            DockEdge::Right => rubber.1 = r,
        };
        match outcome {
            DragOutcome::Width(w) => {
                set_width(&mut self.widths, w);
                set_rubber(&mut self.rubber, 0.0);
                None
            }
            DragOutcome::Collapse { rubber } => {
                set_rubber(&mut self.rubber, rubber);
                Some(DockEvent::Close)
            }
            DragOutcome::Reopen(w) => {
                set_width(&mut self.widths, w);
                set_rubber(&mut self.rubber, 0.0);
                Some(DockEvent::Open)
            }
        }
    }

    /// The spring lets go with the finger: whatever sliver was being held out
    /// springs back flush, animated by the same `DockMotion`.
    pub(super) fn release_drag(&mut self) {
        self.rubber = (0.0, 0.0);
    }

    /// The give currently held out on each side, as `(tree, inspector)`.
    #[cfg(test)]
    pub(super) fn rubber(&self) -> (f64, f64) {
        self.rubber
    }
}
