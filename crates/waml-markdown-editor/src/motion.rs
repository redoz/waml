//! One motion clock over stable-identity layout geometry.
//!
//! Every draw layer consumes the same interpolated snapshot, so text,
//! selection, diagnostics, images, caret, and IME can never disagree about
//! where they are in a frame.

use std::{collections::BTreeMap, sync::Arc};

use makepad_widgets::{animator::Ease, Play, Rect};
use waml_syntax::TextChange;

use crate::{
    input::ScrollAnchor,
    layout::{GeometryElementId, LayoutElementId, LayoutSnapshot},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MotionElementId {
    GlyphCluster(GeometryElementId),
    Block(LayoutElementId),
    EmbeddedBlock(LayoutElementId),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometryEntry {
    pub id: MotionElementId,
    pub rect: Rect,
    pub baseline: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionConfig {
    pub duration_seconds: f64,
    pub ease: Ease,
    pub max_changed_source_bytes: usize,
    pub max_changed_visible_elements: usize,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            duration_seconds: 0.100,
            ease: Ease::OutCubic,
            max_changed_source_bytes: 4096,
            max_changed_visible_elements: 256,
        }
    }
}

#[derive(Clone, Debug)]
pub enum LayoutChangeCause {
    LocalEdit { changes: Arc<[TextChange]> },
    ImageMeasurement(LayoutElementId),
    ViewportResize,
    InitialLoad,
    ExternalReplacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MotionCutReason {
    ReducedMotion,
    InitialLoad,
    ExternalReplacement,
    ViewportResize,
    SourceBudget,
    VisibleGeometryBudget,
    UnsafeIdentityMapping,
    OutsideViewport,
}

#[derive(Clone)]
pub struct MotionFrame {
    pub layout: Arc<LayoutSnapshot>,
    pub scroll_y: f64,
    pub progress: f64,
    pub active: bool,
    pub cut_reason: Option<MotionCutReason>,
}

/// Tolerance, in seconds, for landing exactly on the end of a transition.
const END_EPSILON: f64 = 1e-9;

#[derive(Clone)]
struct Transition {
    from: Arc<LayoutSnapshot>,
    to: Arc<LayoutSnapshot>,
    start: f64,
    duration: f64,
    ease: Ease,
}

#[derive(Default)]
pub struct MotionController {
    transition: Option<Transition>,
    current: Option<Arc<LayoutSnapshot>>,
    anchor: Option<ScrollAnchor>,
    viewport_height: f64,
    last_cut: Option<MotionCutReason>,
}

impl MotionController {
    pub fn new(viewport_height: f64) -> Self {
        Self {
            viewport_height,
            ..Self::default()
        }
    }

    pub fn set_viewport_height(&mut self, viewport_height: f64) {
        self.viewport_height = viewport_height;
    }

    /// Installs a new target. Only a local edit or an image remeasurement may
    /// animate; everything else cuts. An interrupted transition rebases from
    /// the frame currently on screen, never from the obsolete original.
    #[allow(clippy::too_many_arguments)]
    pub fn commit(
        &mut self,
        now: f64,
        previous: Option<Arc<LayoutSnapshot>>,
        target: Arc<LayoutSnapshot>,
        cause: LayoutChangeCause,
        reduced_motion: bool,
        anchor: Option<ScrollAnchor>,
        config: MotionConfig,
    ) -> MotionFrame {
        self.anchor = anchor;
        // An interrupted transition rebases from the frame that is on screen.
        let interrupted = self.transition.is_some().then(|| self.sample(now).layout);
        let previous = interrupted.or_else(|| self.current.clone()).or(previous);

        let cut = cut_reason(
            previous.as_deref(),
            &target,
            &cause,
            reduced_motion,
            &config,
        );
        self.last_cut = cut;
        if cut.is_some() || previous.is_none() {
            self.transition = None;
            self.current = Some(target.clone());
            return self.frame(target, 1.0, false, cut);
        }
        let from = previous.expect("an animating commit has a previous snapshot");
        self.transition = Some(Transition {
            from,
            to: target.clone(),
            start: now,
            duration: config.duration_seconds.max(0.0),
            ease: config.ease,
        });
        self.current = Some(target);
        self.sample(now)
    }

    /// Samples the frame for `now`. The returned snapshot is the only geometry
    /// any layer may draw this frame.
    pub fn sample(&mut self, now: f64) -> MotionFrame {
        let Some(transition) = self.transition.clone() else {
            let layout = self
                .current
                .clone()
                .expect("sample is only valid after a commit");
            return self.frame(layout, 1.0, false, self.last_cut);
        };
        // Frame clocks arrive as floating-point seconds, so a sample taken at
        // exactly the end can land a few ulps short. Treat that as ended rather
        // than leaving a transition running for one extra frame.
        let elapsed = (now - transition.start).max(0.0);
        let (mut ended, linear) = Play::Forward {
            duration: transition.duration,
        }
        .get_ended_time(elapsed);
        // Frame clocks arrive as floating-point seconds, so a sample taken at
        // exactly the end can land a few ulps short. Treat that as ended rather
        // than leaving a transition running for one more frame.
        ended |= elapsed + END_EPSILON >= transition.duration;
        let eased = transition.ease.map(linear);
        let layout = Arc::new(LayoutSnapshot::interpolate(
            &transition.from,
            &transition.to,
            eased,
        ));
        if ended {
            self.transition = None;
            self.current = Some(transition.to.clone());
            return self.frame(transition.to, 1.0, false, None);
        }
        self.frame(layout, eased, true, None)
    }

    /// Scroll follows the caret geometry of this very frame, so it can never
    /// drift from what the caret layer draws.
    fn frame(
        &self,
        layout: Arc<LayoutSnapshot>,
        progress: f64,
        active: bool,
        cut_reason: Option<MotionCutReason>,
    ) -> MotionFrame {
        let scroll_y = self
            .anchor
            .and_then(|anchor| {
                let caret = layout.source_to_point(anchor.position)?;
                Some(
                    (caret.rect.pos.y - anchor.viewport_y)
                        .clamp(0.0, layout.max_scroll_y(self.viewport_height)),
                )
            })
            .unwrap_or(0.0);
        MotionFrame {
            layout,
            scroll_y,
            progress,
            active,
            cut_reason,
        }
    }
}

/// Cut policy, evaluated in exactly this order. The first match is reported.
fn cut_reason(
    previous: Option<&LayoutSnapshot>,
    target: &LayoutSnapshot,
    cause: &LayoutChangeCause,
    reduced_motion: bool,
    config: &MotionConfig,
) -> Option<MotionCutReason> {
    if reduced_motion {
        return Some(MotionCutReason::ReducedMotion);
    }
    match cause {
        LayoutChangeCause::InitialLoad => return Some(MotionCutReason::InitialLoad),
        LayoutChangeCause::ExternalReplacement => {
            return Some(MotionCutReason::ExternalReplacement)
        }
        LayoutChangeCause::ViewportResize => return Some(MotionCutReason::ViewportResize),
        LayoutChangeCause::LocalEdit { changes } => {
            if changed_source_bytes(changes) > config.max_changed_source_bytes {
                return Some(MotionCutReason::SourceBudget);
            }
        }
        LayoutChangeCause::ImageMeasurement(_) => {}
    }
    let previous = previous?;
    let (Some(from), Some(to)) = (entries(previous), entries(target)) else {
        return Some(MotionCutReason::UnsafeIdentityMapping);
    };
    let changed = changed_visible_elements(&from, &to);
    if changed > config.max_changed_visible_elements {
        return Some(MotionCutReason::VisibleGeometryBudget);
    }
    if !from.keys().any(|id| to.contains_key(id)) {
        // Nothing that is on screen survives, so there is nothing to move.
        return Some(MotionCutReason::OutsideViewport);
    }
    if changed == 0 {
        return Some(MotionCutReason::OutsideViewport);
    }
    None
}

fn changed_source_bytes(changes: &[TextChange]) -> usize {
    changes
        .iter()
        .map(|change| {
            let range = change.old_range;
            range
                .end()
                .to_usize()
                .saturating_sub(range.start().to_usize())
                + change.replacement.len()
        })
        .sum()
}

/// Every new, deleted, or moved element counts once across the union of the
/// two visible windows.
fn changed_visible_elements(
    from: &BTreeMap<MotionElementId, GeometryEntry>,
    to: &BTreeMap<MotionElementId, GeometryEntry>,
) -> usize {
    let mut changed = 0;
    for (id, entry) in to {
        match from.get(id) {
            None => changed += 1,
            Some(previous) if previous.rect != entry.rect => changed += 1,
            Some(_) => {}
        }
    }
    changed += from.keys().filter(|id| !to.contains_key(id)).count();
    changed
}

/// The visible geometry of one snapshot, keyed by motion identity. `None` means
/// the map is unsafe because an identity repeats.
pub fn entries(snapshot: &LayoutSnapshot) -> Option<BTreeMap<MotionElementId, GeometryEntry>> {
    let mut entries = BTreeMap::new();
    let mut insert = |id: MotionElementId, rect: Rect, baseline: Option<f64>| {
        entries
            .insert(id, GeometryEntry { id, rect, baseline })
            .is_none()
    };
    for cluster in snapshot.glyph_clusters() {
        let baseline = cluster.glyphs.first().map(|glyph| glyph.baseline);
        if !insert(
            MotionElementId::GlyphCluster(cluster.id),
            cluster.rect,
            baseline,
        ) {
            return None;
        }
    }
    for block in snapshot.visible_blocks() {
        if !insert(MotionElementId::Block(block.id), block.rect, None) {
            return None;
        }
    }
    Some(entries)
}
