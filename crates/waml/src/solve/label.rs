//! World-space edge label placement.

use super::sizing::{self, Font};
use super::wire::{Rect, Size};

/// The face edge labels are drawn in. The renderer's `target_size` is
/// `8.0 * zoom`, so 8.0 is the world-space size and both agree at zoom 1.
const LABEL_FONT: Font = Font::Sans;

/// World-space height of a group's title strip, measured from the top of the
/// group's rect. Treated as a hard obstacle by `place_labels`: a label may
/// not sit on top of the title text. The group's INTERIOR below this band is
/// deliberately not an obstacle -- a group legitimately contains edges and
/// their labels.
pub const GROUP_TITLE_BAND: f64 = 32.0;

/// Tunables for label geometry, in world units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelConfig {
    /// World-space font size for label text.
    pub font_size: f64,
    /// Clearance between a route and the label riding alongside it.
    pub gap: f64,
    /// Extra room reserved between the two terminal labels of one edge, so
    /// they do not touch in the middle when the gap is sized to hold both.
    pub slack: f64,
}

impl Default for LabelConfig {
    fn default() -> Self {
        LabelConfig {
            font_size: 8.0,
            gap: 3.0,
            slack: 24.0,
        }
    }
}

/// World-space box a label's text occupies. Height is a full line height even
/// for empty text -- a zero-height rect is invisible to every collision test,
/// which would silently stop an empty label from acting as an obstacle.
pub fn measure(text: &str, cfg: &LabelConfig) -> Size {
    Size {
        w: sizing::text_width(text, cfg.font_size, LABEL_FONT),
        h: sizing::line_height(cfg.font_size, LABEL_FONT),
    }
}

/// Which label on an edge this is. The two terminals carry role/multiplicity
/// text and belong to one end; the mid-route label carries the relationship
/// name and belongs to the whole route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LabelSlot {
    TerminalFrom,
    TerminalTo,
    MidRoute,
}

/// One label to place: which edge (index into the route list), which slot, and
/// the already-composed text. Text composition is display policy and stays with
/// the frontend; the solver only ever sees the final string.
#[derive(Debug, Clone, PartialEq)]
pub struct LabelRequest {
    pub edge: usize,
    pub slot: LabelSlot,
    pub text: String,
}

/// One possible position for a label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    /// World box the text would occupy.
    pub rect: Rect,
    /// Point on the route this candidate hangs off, for the leader-line stage
    /// and for the renderer's own head clearance.
    pub attach: (f64, f64),
    /// True when this is the preferred side (above a horizontal run, right of
    /// a vertical one). Used to break near-ties so labels do not flip sides
    /// between two layouts that score the same.
    pub side_is_canonical: bool,
    /// How far this slid from the slot's ideal position, in world units.
    pub slide_cost: f64,
}

/// Fraction-of-band slide positions offered to a terminal label, measured from
/// its own endpoint. Bounded: slid too far and the text stops reading as
/// belonging to that end.
const TERMINAL_SLIDES: [f64; 4] = [0.0, 0.15, 0.30, 0.45];
/// Arc-length positions offered to a mid-route label, as a fraction of total
/// route length. Centred on the true middle.
const MID_SLIDES: [f64; 5] = [0.50, 0.42, 0.58, 0.35, 0.65];

pub fn candidates(
    points: &[(f64, f64)],
    slot: LabelSlot,
    size: Size,
    cfg: &LabelConfig,
) -> Vec<Candidate> {
    let total = polyline_length(points);
    if points.len() < 2 || total <= f64::EPSILON {
        return Vec::new();
    }

    let mut out = Vec::new();
    let slides: &[f64] = match slot {
        LabelSlot::MidRoute => &MID_SLIDES,
        _ => &TERMINAL_SLIDES,
    };

    for &s in slides {
        // Terminal slots measure their fraction from their own end of the
        // route; mid-route measures from the start.
        let t = match slot {
            LabelSlot::TerminalFrom => s,
            LabelSlot::TerminalTo => 1.0 - s,
            LabelSlot::MidRoute => s,
        };
        let Some((attach, tangent)) = point_at_fraction(points, t) else {
            continue;
        };
        let horizontal = tangent.0.abs() >= tangent.1.abs();
        for canonical in [true, false] {
            let rect = rect_for(attach, tangent, horizontal, canonical, size, slot, cfg);
            out.push(Candidate {
                rect,
                attach,
                side_is_canonical: canonical,
                // Cost is the DISTANCE SLID from the slot's ideal position, not
                // the raw fraction: terminal fractions already measure from
                // their own end, but mid-route ones are absolute, so the ideal
                // (the centre) has to be subtracted off.
                slide_cost: match slot {
                    LabelSlot::MidRoute => (s - 0.5).abs() * total,
                    _ => s * total,
                },
            });
        }
    }
    out
}

/// Sum of segment lengths of a polyline.
fn polyline_length(points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|segment| (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1))
        .sum()
}

/// Walks the polyline's arc length to `t * total` (t in `[0, 1]`) and returns
/// the point there plus the UNIT tangent of the segment it landed on. `None`
/// for a degenerate (empty, single-point, or zero-length) polyline.
fn point_at_fraction(points: &[(f64, f64)], t: f64) -> Option<((f64, f64), (f64, f64))> {
    let total = polyline_length(points);
    if points.len() < 2 || total <= f64::EPSILON {
        return None;
    }
    let t = t.clamp(0.0, 1.0);
    let mut remaining = t * total;
    for segment in points.windows(2) {
        let dx = segment[1].0 - segment[0].0;
        let dy = segment[1].1 - segment[0].1;
        let length = dx.hypot(dy);
        if length <= f64::EPSILON {
            continue;
        }
        if remaining <= length {
            let fraction = remaining / length;
            let point = (segment[0].0 + dx * fraction, segment[0].1 + dy * fraction);
            let tangent = (dx / length, dy / length);
            return Some((point, tangent));
        }
        remaining -= length;
    }
    // Rounding can leave a hair of remaining length after the loop; land on
    // the final segment's endpoint with its tangent.
    let last = points.windows(2).last()?;
    let dx = last[1].0 - last[0].0;
    let dy = last[1].1 - last[0].1;
    let length = dx.hypot(dy);
    if length <= f64::EPSILON {
        return None;
    }
    Some((last[1], (dx / length, dy / length)))
}

/// World box for one candidate: lifted `cfg.gap` off the stroke on the chosen
/// side, and grown along the route axis so a terminal label never grows back
/// over its own endpoint (and the card sitting there).
///
/// Mirrors `LabelAlign` in `waml-editor/src/edge_labels.rs::aligned_text_pos`:
/// a horizontal route lifts the text above or below it; a vertical route steps
/// the text aside to the right or left.
fn rect_for(
    attach: (f64, f64),
    tangent: (f64, f64),
    horizontal: bool,
    canonical: bool,
    size: Size,
    slot: LabelSlot,
    cfg: &LabelConfig,
) -> Rect {
    let (ax, ay) = attach;
    if horizontal {
        // Canonical side is above the stroke; the other candidate offers below.
        let y = if canonical {
            ay - cfg.gap - size.h
        } else {
            ay + cfg.gap
        };
        // Grow AWAY from the label's own endpoint, which means consulting the
        // tangent's sign: on a right-to-left route both terminal labels would
        // otherwise grow back over their own endpoint's card.
        let forward = tangent.0 >= 0.0;
        let x = match slot {
            LabelSlot::TerminalFrom => {
                if forward {
                    ax
                } else {
                    ax - size.w
                }
            }
            LabelSlot::TerminalTo => {
                if forward {
                    ax - size.w
                } else {
                    ax
                }
            }
            LabelSlot::MidRoute => ax - size.w * 0.5,
        };
        Rect {
            x,
            y,
            w: size.w,
            h: size.h,
        }
    } else {
        // Canonical side is right of the stroke; the other candidate offers left.
        let x = if canonical {
            ax + cfg.gap
        } else {
            ax - cfg.gap - size.w
        };
        let y = match slot {
            LabelSlot::TerminalFrom => {
                if tangent.1 >= 0.0 {
                    ay
                } else {
                    ay - size.h
                }
            }
            LabelSlot::TerminalTo => {
                if tangent.1 >= 0.0 {
                    ay - size.h
                } else {
                    ay
                }
            }
            LabelSlot::MidRoute => ay - size.h * 0.5,
        };
        Rect {
            x,
            y,
            w: size.w,
            h: size.h,
        }
    }
}

/// What a label may not sit on (`hard`) and what merely costs it (`soft`).
///
/// Node cards and already-placed labels are hard: text under a card is
/// invisible and text on text is unreadable. Group *title bands* are hard, but
/// group interiors are NOT -- a group box is a large translucent container that
/// legitimately holds edges and labels, so treating its whole rect as solid
/// would forbid every label inside a group.
///
/// Foreign edge strokes are soft, and `place` derives them from the route list
/// it is given rather than from this struct: a label's OWN stroke is neither
/// hard nor soft (the perpendicular gap already clears it), and only `place`
/// knows which route each request belongs to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Obstacles {
    pub hard: Vec<Rect>,
}

/// True when `rect` overlaps any hard obstacle. Abutting exactly is NOT a
/// collision -- that is the common case when a spacing floor put a box right at
/// the gap, and rejecting it would throw away the best candidate.
pub fn collides(rect: Rect, hard: &[Rect]) -> bool {
    hard.iter().any(|o| {
        rect.x < o.x + o.w && o.x < rect.x + rect.w && rect.y < o.y + o.h && o.y < rect.y + rect.h
    })
}

/// How many soft segments cross `rect`.
pub fn soft_crossings(rect: Rect, soft: &[[(f64, f64); 2]]) -> usize {
    soft.iter()
        .filter(|s| segment_hits_rect(s[0], s[1], rect))
        .count()
}

/// How many FOREIGN route segments cross `rect`. `own` is the index of the
/// route the label belongs to; its own stroke is skipped, because the
/// perpendicular gap already clears it and charging a label for its own bend
/// would flip it to a worse side for no reason.
fn foreign_crossings(rect: Rect, routes: &[Vec<(f64, f64)>], own: usize) -> usize {
    routes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != own)
        .flat_map(|(_, points)| points.windows(2))
        .filter(|s| segment_hits_rect(s[0], s[1], rect))
        .count()
}

/// How far off-axis a segment may be and still count as axis-aligned. A
/// GEOMETRIC tolerance, deliberately not `f64::EPSILON`: route and geometry
/// arithmetic leaves sub-pixel drift on nominally orthogonal segments, and at
/// machine epsilon every such segment falls through to the diagonal arm, so
/// `W_CROSSING` would be silently skipped and a label would sit on a foreign
/// stroke.
const AXIS_TOLERANCE: f64 = 1e-6;

/// True when the axis-aligned segment `a`-`b` crosses `rect`. The router only
/// ever produces orthogonal routes, so a diagonal segment honestly returns
/// `false` rather than pretending to handle a case that cannot occur.
fn segment_hits_rect(a: (f64, f64), b: (f64, f64), rect: Rect) -> bool {
    let x_min = rect.x;
    let x_max = rect.x + rect.w;
    let y_min = rect.y;
    let y_max = rect.y + rect.h;
    if (a.0 - b.0).abs() <= AXIS_TOLERANCE {
        // Vertical segment: constant x, varying y.
        let x = a.0;
        let (y0, y1) = if a.1 <= b.1 { (a.1, b.1) } else { (b.1, a.1) };
        x >= x_min && x <= x_max && y0 <= y_max && y1 >= y_min
    } else if (a.1 - b.1).abs() <= AXIS_TOLERANCE {
        // Horizontal segment: constant y, varying x.
        let y = a.1;
        let (x0, x1) = if a.0 <= b.0 { (a.0, b.0) } else { (b.0, a.0) };
        y >= y_min && y <= y_max && x0 <= x_max && x1 >= x_min
    } else {
        false
    }
}

/// Weight on how far a candidate slid from its slot's ideal position.
const W_SLIDE: f64 = 1.0;
/// Weight on taking the non-canonical side. Small: it only breaks near-ties,
/// so a genuinely better position still wins.
const W_SIDE: f64 = 8.0;
/// Weight per foreign stroke crossed.
const W_CROSSING: f64 = 40.0;

/// A label the solver found a home for.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PlacedLabel {
    pub edge: usize,
    pub slot: LabelSlot,
    pub text: String,
    pub rect: Rect,
    pub attach: (f64, f64),
    /// Set when this label could not be placed beside its route and was
    /// pushed out to free space instead: `[attach, nearest point on rect]`.
    /// `None` for every label placed normally by `place`.
    pub leader: Option<[(f64, f64); 2]>,
}

/// Result of a placement pass. `unplaced` is not an error: it is the input to
/// the reroute and leader-line stages, and its length is the instrumentation
/// that says whether those stages are earning their complexity.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    pub placed: Vec<PlacedLabel>,
    pub unplaced: Vec<LabelRequest>,
}

pub fn place(
    routes: &[Vec<(f64, f64)>],
    requests: &[LabelRequest],
    obstacles: &Obstacles,
    cfg: &LabelConfig,
) -> Placement {
    let mut out = Placement::default();
    // Already-placed labels become hard obstacles for later ones, which is what
    // stops labels landing on each other.
    let mut hard = obstacles.hard.clone();

    // Single pass. A retry would be a no-op: `hard` only ever GROWS (each
    // placement pushes its rect and nothing is ever removed), so a request that
    // collided on every candidate collides again on any later pass.
    for request in requests {
        if !try_place(request, routes, &mut hard, cfg, &mut out) {
            out.unplaced.push(request.clone());
        }
    }
    out
}

/// Last-resort placement for a request `place` could not fit: the best-scoring
/// candidate IGNORING hard obstacles. Overlapping text beats a label that
/// silently vanishes from the diagram, so the caller uses this rather than
/// dropping the label. `None` only for a degenerate route with no candidates at
/// all.
pub fn fallback(
    routes: &[Vec<(f64, f64)>],
    request: &LabelRequest,
    cfg: &LabelConfig,
) -> Option<PlacedLabel> {
    let points = routes.get(request.edge)?;
    let size = measure(&request.text, cfg);
    let mut best: Option<(f64, Candidate)> = None;
    for c in candidates(points, request.slot, size, cfg) {
        let score = score_candidate(&c, routes, request.edge);
        if best.as_ref().map(|(b, _)| score < *b).unwrap_or(true) {
            best = Some((score, c));
        }
    }
    best.map(|(_, c)| PlacedLabel {
        edge: request.edge,
        slot: request.slot,
        text: request.text.clone(),
        rect: c.rect,
        attach: c.attach,
        leader: None,
    })
}

fn score_candidate(c: &Candidate, routes: &[Vec<(f64, f64)>], own: usize) -> f64 {
    W_SLIDE * c.slide_cost
        + if c.side_is_canonical { 0.0 } else { W_SIDE }
        + W_CROSSING * foreign_crossings(c.rect, routes, own) as f64
}

/// Score a single request against every candidate and take the best that has no
/// hard collision. Returns false when nothing fits.
fn try_place(
    request: &LabelRequest,
    routes: &[Vec<(f64, f64)>],
    hard: &mut Vec<Rect>,
    cfg: &LabelConfig,
    out: &mut Placement,
) -> bool {
    let Some(points) = routes.get(request.edge) else {
        return false;
    };
    let size = measure(&request.text, cfg);
    let mut best: Option<(f64, Candidate)> = None;
    for c in candidates(points, request.slot, size, cfg) {
        if collides(c.rect, hard) {
            continue;
        }
        let score = score_candidate(&c, routes, request.edge);
        // Strict `<` keeps the FIRST candidate of a tie, and candidate order is
        // deterministic, so ties resolve the same way every run.
        if best.as_ref().map(|(b, _)| score < *b).unwrap_or(true) {
            best = Some((score, c));
        }
    }
    match best {
        Some((_, c)) => {
            hard.push(c.rect);
            out.placed.push(PlacedLabel {
                edge: request.edge,
                slot: request.slot,
                text: request.text.clone(),
                rect: c.rect,
                attach: c.attach,
                leader: None,
            });
            true
        }
        None => false,
    }
}

/// Ring count for the leader search. Each ring steps out by two label heights,
/// so this reaches well past any realistic diagram's bounding box -- which is
/// what makes the search total.
const MAX_LEADER_RINGS: usize = 64;
/// Positions sampled per ring. Fixed count in fixed angular order, so the
/// search is deterministic.
const LEADER_STEPS: usize = 16;

/// The un-displaced attach point a slot would have used, ignoring collisions:
/// the first candidate's `attach`. Every candidate for a given slot shares the
/// same attach point (the slide only moves the rect, not the anchor along the
/// route it was measured from at the un-displaced position), so any candidate
/// would do; the first is simplest.
fn ideal_anchor(points: &[(f64, f64)], slot: LabelSlot) -> Option<(f64, f64)> {
    let slides: &[f64] = match slot {
        LabelSlot::MidRoute => &MID_SLIDES,
        _ => &TERMINAL_SLIDES,
    };
    let t = match slot {
        LabelSlot::TerminalFrom => slides[0],
        LabelSlot::TerminalTo => 1.0 - slides[0],
        LabelSlot::MidRoute => slides[0],
    };
    point_at_fraction(points, t).map(|(attach, _)| attach)
}

/// The point on `rect`'s border closest to `from`, so a leader line meets the
/// label box rather than aiming at its centre.
fn nearest_edge_point(rect: Rect, from: (f64, f64)) -> (f64, f64) {
    let x = from.0.clamp(rect.x, rect.x + rect.w);
    let y = from.1.clamp(rect.y, rect.y + rect.h);
    // If `from` is already inside or on the rect, `x`/`y` land inside it;
    // snap to the nearest border instead of leaving the point interior.
    let dist_left = (x - rect.x).abs();
    let dist_right = (rect.x + rect.w - x).abs();
    let dist_top = (y - rect.y).abs();
    let dist_bottom = (rect.y + rect.h - y).abs();
    let inside = x > rect.x && x < rect.x + rect.w && y > rect.y && y < rect.y + rect.h;
    if !inside {
        return (x, y);
    }
    let min = dist_left.min(dist_right).min(dist_top).min(dist_bottom);
    if min == dist_left {
        (rect.x, y)
    } else if min == dist_right {
        (rect.x + rect.w, y)
    } else if min == dist_top {
        (x, rect.y)
    } else {
        (x, rect.y + rect.h)
    }
}

/// Place every request, falling back to a leader line for any that will not fit
/// beside their route.
///
/// This is TOTAL for any request whose route resolves: obstacles are finite,
/// so an expanding ring always reaches empty space outside the content
/// bounding box. That is what makes leader lines a complete strategy rather
/// than one more thing that can fail. A request whose `edge` does not resolve
/// to a route, or whose route is too degenerate to yield an anchor, comes back
/// in `unplaced` -- there is no route for a leader to attach to.
pub fn place_with_leaders(
    routes: &[Vec<(f64, f64)>],
    requests: &[LabelRequest],
    obstacles: &Obstacles,
    cfg: &LabelConfig,
) -> Placement {
    let mut out = place(routes, requests, obstacles, cfg);
    let residue = std::mem::take(&mut out.unplaced);
    let mut hard: Vec<Rect> = obstacles
        .hard
        .iter()
        .copied()
        .chain(out.placed.iter().map(|p| p.rect))
        .collect();

    for request in residue {
        let size = measure(&request.text, cfg);
        let Some(points) = routes.get(request.edge) else {
            out.unplaced.push(request);
            continue;
        };
        let Some(anchor) = ideal_anchor(points, request.slot) else {
            out.unplaced.push(request);
            continue;
        };

        let mut found = None;
        'rings: for ring in 1..=MAX_LEADER_RINGS {
            let radius = ring as f64 * size.h * 2.0;
            for step in 0..LEADER_STEPS {
                let angle = std::f64::consts::TAU * step as f64 / LEADER_STEPS as f64;
                let rect = Rect {
                    x: anchor.0 + radius * angle.cos() - size.w * 0.5,
                    y: anchor.1 + radius * angle.sin() - size.h * 0.5,
                    w: size.w,
                    h: size.h,
                };
                if !collides(rect, &hard) {
                    found = Some(rect);
                    break 'rings;
                }
            }
        }

        match found {
            Some(rect) => {
                hard.push(rect);
                out.placed.push(PlacedLabel {
                    edge: request.edge,
                    slot: request.slot,
                    text: request.text.clone(),
                    rect,
                    attach: anchor,
                    leader: Some([anchor, nearest_edge_point(rect, anchor)]),
                });
            }
            None => out.unplaced.push(request),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_measures_wider_than_it_is_tall_and_scales_with_text() {
        let cfg = LabelConfig::default();
        let short = measure("1", &cfg);
        let long = measure("settledBy {0..*}", &cfg);
        assert!(long.w > short.w, "more text is wider");
        assert_eq!(short.h, long.h, "one line is one line height");
        assert!(short.h > 0.0);
    }

    #[test]
    fn an_empty_label_still_has_height() {
        // A zero-height rect would be invisible to every collision test, so an
        // empty label would silently stop being an obstacle.
        let m = measure("", &LabelConfig::default());
        assert_eq!(m.w, 0.0);
        assert!(m.h > 0.0);
    }

    #[test]
    fn terminal_candidates_stay_near_their_own_endpoint() {
        let cfg = LabelConfig::default();
        let size = Size { w: 40.0, h: 10.0 };
        let route = [(0.0, 0.0), (200.0, 0.0)];

        let from = candidates(&route, LabelSlot::TerminalFrom, size, &cfg);
        let to = candidates(&route, LabelSlot::TerminalTo, size, &cfg);

        assert!(!from.is_empty() && !to.is_empty());
        // Each terminal set clusters at its OWN end, never past the middle.
        for c in &from {
            assert!(c.attach.0 < 100.0, "from candidates stay in the near half");
        }
        for c in &to {
            assert!(c.attach.0 > 100.0, "to candidates stay in the far half");
        }
    }

    #[test]
    fn every_candidate_clears_the_stroke_by_the_gap() {
        let cfg = LabelConfig::default();
        let size = Size { w: 40.0, h: 10.0 };
        // Horizontal route along y = 50: no candidate rect may touch that line.
        for c in candidates(
            &[(0.0, 50.0), (200.0, 50.0)],
            LabelSlot::MidRoute,
            size,
            &cfg,
        ) {
            let clears_above = c.rect.y + c.rect.h <= 50.0 - cfg.gap + 1e-9;
            let clears_below = c.rect.y >= 50.0 + cfg.gap - 1e-9;
            assert!(
                clears_above || clears_below,
                "rect sits on the stroke: {:?}",
                c.rect
            );
        }
    }

    #[test]
    fn both_sides_of_the_stroke_are_offered() {
        let cfg = LabelConfig::default();
        let size = Size { w: 40.0, h: 10.0 };
        let cs = candidates(
            &[(0.0, 50.0), (200.0, 50.0)],
            LabelSlot::MidRoute,
            size,
            &cfg,
        );
        assert!(cs.iter().any(|c| c.rect.y < 50.0), "some candidate above");
        assert!(cs.iter().any(|c| c.rect.y > 50.0), "some candidate below");
        assert!(
            cs.iter().any(|c| c.side_is_canonical),
            "canonical side offered"
        );
    }

    #[test]
    fn candidate_generation_is_deterministic() {
        let cfg = LabelConfig::default();
        let size = Size { w: 40.0, h: 10.0 };
        let route = [(0.0, 0.0), (60.0, 0.0), (60.0, 90.0)];
        let a = candidates(&route, LabelSlot::MidRoute, size, &cfg);
        let b = candidates(&route, LabelSlot::MidRoute, size, &cfg);
        assert_eq!(a, b);
    }

    #[test]
    fn a_degenerate_route_yields_no_candidates() {
        let cfg = LabelConfig::default();
        let size = Size { w: 40.0, h: 10.0 };
        assert!(candidates(&[], LabelSlot::MidRoute, size, &cfg).is_empty());
        assert!(candidates(&[(1.0, 1.0)], LabelSlot::MidRoute, size, &cfg).is_empty());
        assert!(candidates(&[(1.0, 1.0), (1.0, 1.0)], LabelSlot::MidRoute, size, &cfg).is_empty());
    }

    #[test]
    fn terminal_labels_grow_away_from_their_endpoint_on_a_right_to_left_route() {
        // Target left of source: the route runs -x, so a `from` label must grow
        // LEFT off its endpoint and a `to` label must grow RIGHT off its own.
        // Growing the other way puts both boxes back over their own card.
        let cfg = LabelConfig::default();
        let size = Size { w: 40.0, h: 10.0 };
        let route = [(300.0, 50.0), (0.0, 50.0)];

        for c in candidates(&route, LabelSlot::TerminalFrom, size, &cfg) {
            assert!(
                c.rect.x + c.rect.w <= c.attach.0 + 1e-9,
                "from label grew back over its endpoint: {:?}",
                c.rect
            );
        }
        for c in candidates(&route, LabelSlot::TerminalTo, size, &cfg) {
            assert!(
                c.rect.x >= c.attach.0 - 1e-9,
                "to label grew back over its endpoint: {:?}",
                c.rect
            );
        }
    }

    #[test]
    fn a_nearly_axis_aligned_segment_still_counts_as_a_crossing() {
        // Route arithmetic leaves sub-pixel drift on nominally orthogonal
        // segments; at machine epsilon those fell through to the diagonal arm
        // and the crossing term was silently skipped.
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        assert!(segment_hits_rect((-10.0, 25.0), (110.0, 25.0 + 1e-9), rect));
        assert!(segment_hits_rect((50.0, -10.0), (50.0 + 1e-9, 60.0), rect));
    }

    #[test]
    fn an_unplaceable_label_still_gets_a_last_resort_position() {
        // Nothing fits, but the label must not vanish from the diagram.
        let reqs = LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        };
        let placed = fallback(&route_pair(), &reqs, &LabelConfig::default())
            .expect("a real route always yields some candidate");
        assert_eq!(placed.text, "places");
        assert!(placed.rect.h > 0.0);
    }

    #[test]
    fn a_rect_overlapping_a_card_is_rejected() {
        let card = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        assert!(collides(
            Rect {
                x: 90.0,
                y: 40.0,
                w: 30.0,
                h: 20.0
            },
            &[card]
        ));
        assert!(!collides(
            Rect {
                x: 101.0,
                y: 0.0,
                w: 30.0,
                h: 20.0
            },
            &[card]
        ));
    }

    #[test]
    fn touching_edges_do_not_count_as_a_collision() {
        // Exactly abutting is the common case when a floor put a box right at
        // the gap. Treating it as a collision would reject the best candidate.
        let card = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        assert!(!collides(
            Rect {
                x: 100.0,
                y: 0.0,
                w: 30.0,
                h: 20.0
            },
            &[card]
        ));
    }

    #[test]
    fn soft_crossings_are_counted_not_fatal() {
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let through = [(-10.0, 25.0), (110.0, 25.0)];
        let clear = [(-10.0, 200.0), (110.0, 200.0)];
        assert_eq!(soft_crossings(rect, &[through, clear]), 1);
    }

    #[test]
    fn a_label_is_never_charged_for_its_own_stroke() {
        // Two identical polylines: whichever one owns the label, exactly one
        // crossing is charged -- the OTHER one. A bent route must not pay
        // W_CROSSING for running through its own label's box.
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            w: 100.0,
            h: 50.0,
        };
        let routes = vec![
            vec![(-10.0, 25.0), (110.0, 25.0)],
            vec![(-10.0, 25.0), (110.0, 25.0)],
        ];
        assert_eq!(foreign_crossings(rect, &routes, 0), 1);
        assert_eq!(foreign_crossings(rect, &routes, 1), 1);
        assert_eq!(foreign_crossings(rect, &routes[..1], 0), 0);
    }

    #[test]
    fn a_foreign_stroke_still_pushes_a_label_to_the_other_side() {
        // The soft term now comes from the route list itself, so a neighbouring
        // route running through the canonical side must still flip the label.
        let routes = vec![
            vec![(0.0, 50.0), (300.0, 50.0)],
            vec![(0.0, 45.0), (300.0, 45.0)],
        ];
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let out = place(
            &routes,
            &reqs,
            &Obstacles::default(),
            &LabelConfig::default(),
        );
        assert!(
            out.placed[0].rect.y > 50.0,
            "foreign stroke above should push the label below: {:?}",
            out.placed[0].rect
        );
    }

    fn route_pair() -> Vec<Vec<(f64, f64)>> {
        vec![vec![(0.0, 50.0), (300.0, 50.0)]]
    }

    #[test]
    fn two_labels_on_one_route_never_overlap_each_other() {
        let reqs = vec![
            LabelRequest {
                edge: 0,
                slot: LabelSlot::TerminalFrom,
                text: "order {1}".into(),
            },
            LabelRequest {
                edge: 0,
                slot: LabelSlot::TerminalTo,
                text: "customer {1}".into(),
            },
        ];
        let out = place(
            &route_pair(),
            &reqs,
            &Obstacles::default(),
            &LabelConfig::default(),
        );
        assert_eq!(out.placed.len(), 2, "both placed: {:?}", out.unplaced);
        assert!(!collides(out.placed[0].rect, &[out.placed[1].rect]));
    }

    #[test]
    fn a_label_never_lands_on_a_card() {
        let obstacles = Obstacles {
            hard: vec![Rect {
                x: 0.0,
                y: 0.0,
                w: 140.0,
                h: 100.0,
            }],
        };
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::TerminalFrom,
            text: "order {1}".into(),
        }];
        let out = place(&route_pair(), &reqs, &obstacles, &LabelConfig::default());
        for p in &out.placed {
            assert!(
                !collides(p.rect, &obstacles.hard),
                "placed on a card: {:?}",
                p.rect
            );
        }
    }

    #[test]
    fn an_impossible_label_is_reported_not_silently_dropped() {
        // One enormous obstacle covering the whole route: nothing can be placed,
        // and the caller must be able to tell.
        let obstacles = Obstacles {
            hard: vec![Rect {
                x: -500.0,
                y: -500.0,
                w: 2000.0,
                h: 2000.0,
            }],
        };
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let out = place(&route_pair(), &reqs, &obstacles, &LabelConfig::default());
        assert!(out.placed.is_empty());
        assert_eq!(
            out.unplaced, reqs,
            "unplaceable labels come back to the caller"
        );
    }

    #[test]
    fn placement_is_deterministic() {
        let reqs = vec![
            LabelRequest {
                edge: 0,
                slot: LabelSlot::TerminalFrom,
                text: "a".into(),
            },
            LabelRequest {
                edge: 0,
                slot: LabelSlot::TerminalTo,
                text: "b".into(),
            },
            LabelRequest {
                edge: 0,
                slot: LabelSlot::MidRoute,
                text: "c".into(),
            },
        ];
        let cfg = LabelConfig::default();
        let one = place(&route_pair(), &reqs, &Obstacles::default(), &cfg);
        let two = place(&route_pair(), &reqs, &Obstacles::default(), &cfg);
        assert_eq!(one.placed, two.placed);
    }

    #[test]
    fn the_canonical_side_wins_an_otherwise_tied_choice() {
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let out = place(
            &route_pair(),
            &reqs,
            &Obstacles::default(),
            &LabelConfig::default(),
        );
        // Open space both sides: the label must take the canonical one (above a
        // horizontal run) rather than picking arbitrarily.
        assert!(out.placed[0].rect.y < 50.0, "should sit above the route");
    }

    #[test]
    fn a_label_with_nowhere_to_go_gets_a_leader_into_free_space() {
        let obstacles = Obstacles {
            hard: vec![Rect {
                x: -200.0,
                y: -200.0,
                w: 800.0,
                h: 400.0,
            }],
        };
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let out = place_with_leaders(&route_pair(), &reqs, &obstacles, &LabelConfig::default());

        assert!(out.unplaced.is_empty(), "leader lines make placement total");
        let placed = &out.placed[0];
        assert!(
            !collides(placed.rect, &obstacles.hard),
            "leader target must be free"
        );
        let leader = placed.leader.expect("a displaced label carries a leader");
        assert_eq!(leader[0], placed.attach, "leader starts on the route");
    }

    #[test]
    fn a_label_that_fits_normally_gets_no_leader() {
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let out = place_with_leaders(
            &route_pair(),
            &reqs,
            &Obstacles::default(),
            &LabelConfig::default(),
        );
        assert!(out.placed[0].leader.is_none());
    }

    #[test]
    fn the_leader_search_terminates_on_a_hostile_scene() {
        // Obstacles cannot cover the whole plane, so an expanding ring always
        // finds free space eventually. This pins that the search actually
        // exploits it.
        let obstacles = Obstacles {
            hard: (0..50)
                .map(|i| Rect {
                    x: i as f64 * 40.0,
                    y: 0.0,
                    w: 40.0,
                    h: 400.0,
                })
                .collect(),
        };
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "x".into(),
        }];
        let out = place_with_leaders(&route_pair(), &reqs, &obstacles, &LabelConfig::default());
        assert_eq!(out.placed.len(), 1);
        assert!(out.unplaced.is_empty());
    }

    #[test]
    fn an_unobstructed_mid_route_label_stays_at_the_middle() {
        let reqs = vec![LabelRequest {
            edge: 0,
            slot: LabelSlot::MidRoute,
            text: "places".into(),
        }];
        let out = place(
            &route_pair(),
            &reqs,
            &Obstacles::default(),
            &LabelConfig::default(),
        );
        // Nothing to slide away from, so the label must attach at the route's
        // midpoint (x = 150 on a 0..300 run) — not at the first MID_SLIDES entry
        // that happens to be listed.
        let rect = out.placed[0].rect;
        let centre = rect.x + rect.w / 2.0;
        assert!(
            (centre - 150.0).abs() < 1.0,
            "expected the midpoint, got centre {centre} (rect {rect:?})"
        );
    }
}
