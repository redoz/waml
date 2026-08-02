use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::geometry::segment_quad;
use crate::canvas::primitives::{
    edge_point_to_screen, fill_rect, font_raster_size, world_rect_to_screen,
};
use crate::edge_labels::{marker_extent, HEAD_GAP, LABEL_GAP};
use makepad_widgets::*;
use waml::adornment::{end_marker, End};
use waml::solve::label::LabelSlot;

const LABEL_PAD: f64 = 3.0;

/// Screen-space thickness of a leader line: a hairline regardless of zoom, so
/// it reads as a pointer rather than a route.
const LEADER_THICKNESS: f64 = 1.0;

/// Below this drawn size edge text is an unreadable smear, so it is skipped.
///
/// It is a CUTOFF, never a floor: the solver's non-overlap guarantee is stated
/// over world rects, and the drawn text only stays inside its projected screen
/// rect while it scales with the zoom. Flooring the font instead drew text
/// larger than its own box, overflowing the chip and re-overlapping the
/// neighbouring labels and cards the placement stage exists to avoid.
const MIN_LEGIBLE_PX: f64 = 5.0;

/// World-space font size for edge text, matching `LabelConfig::font_size` --
/// what the solver measured the label boxes with.
const WORLD_FONT_SIZE: f64 = 8.0;

/// World-space clearance the solver already lifted each label off its route by,
/// matching `LabelConfig::gap`. The screen-space top-up tops THIS projected gap
/// up to `LABEL_GAP`, so the two must agree (asserted in the tests below).
const WORLD_GAP: f64 = 3.0;

pub(super) fn draw_edge_labels(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    let viewport = snapshot.viewport;
    // Edge text is annotation, not content: it reads well below the card type
    // scale, and at 11 the multiplicity/role chips out-shouted the cards.
    let world_target = WORLD_FONT_SIZE * viewport.camera.zoom;
    if world_target < MIN_LEGIBLE_PX {
        return;
    }
    let target_size = world_target as f32;
    let font_size = font_raster_size(target_size);
    draws.edge_label.text_style.font_size = font_size;
    draws.edge_label.font_scale = target_size / font_size;

    for label in &snapshot.scene.labels {
        let screen = world_rect_to_screen(viewport, label.rect);
        let center = dvec2(
            label.rect.x + label.rect.w * 0.5,
            label.rect.y + label.rect.h * 0.5,
        );
        let nudge = snapshot
            .scene
            .edges
            .get(label.edge)
            .map(|edge| {
                screen_clearance(
                    center,
                    label.attach,
                    edge,
                    label.slot,
                    viewport.camera.zoom,
                    snapshot.linework.marker_size,
                )
            })
            .unwrap_or_default();
        let leader_end = label.leader.map(|leader| {
            edge_point_to_screen(&viewport.camera, viewport.view_rect.pos, leader[1])
        });
        let (pos, leader_end) = nudged(screen.pos, leader_end, nudge);
        if let (Some(leader), Some(end)) = (label.leader, leader_end) {
            let start = edge_point_to_screen(&viewport.camera, viewport.view_rect.pos, leader[0]);
            for bar in leader_bars(start, end, LEADER_THICKNESS) {
                draws.edge.draw_abs(cx, bar);
            }
        }
        fill_rect(
            cx,
            draws.edge_label_bg,
            Rect {
                pos: pos - dvec2(LABEL_PAD, LABEL_PAD),
                size: screen.size + dvec2(LABEL_PAD * 2.0, LABEL_PAD * 2.0),
            },
            draws.edge_label_bg.color,
        );
        draws.edge_label.draw_abs(cx, pos, &label.text);
    }
}

/// The bars that draw one leader, as an orthogonal L: along x from the route
/// anchor, then along y into the label.
///
/// It cannot be ONE quad. The `EdgeLine` pen FILLS the rect it is handed (a
/// per-segment AABB collapses to the bar itself only for an axis-aligned
/// segment), and the ring search that finds a leader's target samples 16
/// angles, so nearly every leader is diagonal -- one quad painted a solid
/// opaque block from anchor to label, covering exactly the content the
/// placement stage exists to keep clear. Two axis-aligned legs are hairlines by
/// construction, and match the orthogonal routing the edges themselves use.
/// A degenerate leg is dropped, so an axis-aligned leader is a single bar.
fn leader_bars(start: DVec2, end: DVec2, thickness: f64) -> Vec<Rect> {
    let corner = dvec2(end.x, start.y);
    [(start, corner), (corner, end)]
        .into_iter()
        .filter(|(a, b)| (a.x - b.x).abs() > thickness || (a.y - b.y).abs() > thickness)
        .map(|(a, b)| segment_quad(a, b, thickness))
        .collect()
}

/// Apply the screen-space clearance top-up to everything that belongs to one
/// label: the box's top-left AND, when there is one, the leader line's far end.
///
/// They move TOGETHER on purpose. The leader points at the box as drawn, and
/// nudging only the box left the leader ending on the un-nudged world rect --
/// visibly short of (or inside) the box it points at wherever the nudge bites.
fn nudged(pos: DVec2, leader_end: Option<DVec2>, nudge: DVec2) -> (DVec2, Option<DVec2>) {
    (pos + nudge, leader_end.map(|end| end + nudge))
}

/// Screen-space top-up on a world-space placement.
///
/// Placement is world-space, but strokes and endpoint adornments are drawn at a
/// FIXED screen size, so at zoom < 1 the solver's world gap projects to less
/// than it needs to be and the text lands on the arrowhead it was supposed to
/// step past. Two corrections, both in screen px:
///
/// * perpendicular to the route: top the projected gap back up to `LABEL_GAP`.
///   Every label gets this, mid-route names included -- the stroke they ride
///   alongside is screen-width at any zoom;
/// * along the route, for a TERMINAL label only: step past the adornment drawn
///   at that end (`marker_extent` + `HEAD_GAP`), which has no world size at all.
///   Only by however much of that the label does not ALREADY clear: a candidate
///   that slid down the route is past the arrowhead on its own, and pushing it
///   further would shove it out of the world rect the solver reserved for it.
fn screen_clearance(
    center: DVec2,
    attach: (f64, f64),
    edge: &crate::scene::SceneEdge,
    slot: LabelSlot,
    zoom: f64,
    marker_size: f64,
) -> DVec2 {
    // World and screen axes differ only by a positive scale and a translation,
    // so a unit WORLD direction is a valid screen direction too, and the side
    // the solver picked reads the same in both spaces.
    let Some(axis) = route_axis(&edge.points, attach, slot) else {
        return dvec2(0.0, 0.0);
    };
    let perp = (LABEL_GAP - WORLD_GAP * zoom).max(0.0);
    let along = match slot {
        LabelSlot::MidRoute => 0.0,
        LabelSlot::TerminalFrom | LabelSlot::TerminalTo => {
            let (endpoint, end, navigable) = if slot == LabelSlot::TerminalFrom {
                (edge.points[0], End::From, edge.from_end.navigable)
            } else {
                (
                    edge.points[edge.points.len() - 1],
                    End::To,
                    edge.to_end.navigable,
                )
            };
            let needed = marker_extent(end_marker(edge.kind, end, navigable), marker_size)
                + HEAD_GAP
                // Ground the step in what the slide already bought, in the same
                // screen px the adornment is drawn in.
                - (attach.0 - endpoint.0).hypot(attach.1 - endpoint.1) * zoom;
            needed.max(0.0)
        }
    };
    let normal = dvec2(-axis.y, axis.x);
    let offset = center - dvec2(attach.0, attach.1);
    let side = offset.x * normal.x + offset.y * normal.y;
    let sign = if side < 0.0 { -1.0 } else { 1.0 };
    axis * along + normal * (sign * perp)
}

/// Unit direction of the route leg the label clears against.
///
/// For a terminal it leads AWAY from the end the label belongs to, so it doubles
/// as the direction the along-route step pushes. A mid-route label sits on an
/// arbitrary leg, so its axis is the direction of the segment its attach point
/// landed on -- the only thing that says which way "perpendicular to the stroke"
/// points there. `None` for a degenerate polyline.
fn route_axis(points: &[(f64, f64)], attach: (f64, f64), slot: LabelSlot) -> Option<DVec2> {
    if points.len() < 2 {
        return None;
    }
    let (a, b) = match slot {
        LabelSlot::TerminalFrom => (points[0], points[1]),
        LabelSlot::TerminalTo => (points[points.len() - 1], points[points.len() - 2]),
        LabelSlot::MidRoute => nearest_segment(points, attach)?,
    };
    unit(a, b)
}

/// The segment of `points` the attach point lies on, as an ordered
/// `(start, end)` pair. Solver attach points sit exactly ON the route, so this
/// is a lookup; taking the NEAREST segment simply disambiguates a corner and
/// keeps a rounded coordinate from falling through.
fn nearest_segment(points: &[(f64, f64)], attach: (f64, f64)) -> Option<((f64, f64), (f64, f64))> {
    let mut best: Option<(f64, usize)> = None;
    for (index, segment) in points.windows(2).enumerate() {
        let (a, b) = (segment[0], segment[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let length_sq = dx * dx + dy * dy;
        if length_sq <= f64::EPSILON {
            continue;
        }
        let t = (((attach.0 - a.0) * dx + (attach.1 - a.1) * dy) / length_sq).clamp(0.0, 1.0);
        let distance = (a.0 + dx * t - attach.0).hypot(a.1 + dy * t - attach.1);
        if best.map(|(d, _)| distance < d).unwrap_or(true) {
            best = Some((distance, index));
        }
    }
    best.map(|(_, index)| (points[index], points[index + 1]))
}

/// Unit vector from `a` to `b`, or `None` when the two coincide.
fn unit(a: (f64, f64), b: (f64, f64)) -> Option<DVec2> {
    let d = dvec2(b.0 - a.0, b.1 - a.1);
    let length = d.x.hypot(d.y);
    if length <= f64::EPSILON {
        return None;
    }
    Some(d / length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_drawn_font_never_outgrows_its_projected_box() {
        // Flooring the size made the text bigger than the solver's world rect
        // projected by the zoom, so it spilled out of its chip and back over the
        // neighbours the placement stage had just cleared.
        let cfg = waml::solve::label::LabelConfig::default();
        for zoom in [0.2_f64, 0.4, 0.625, 1.0, 3.0] {
            let world_target = WORLD_FONT_SIZE * zoom;
            if world_target < MIN_LEGIBLE_PX {
                continue;
            }
            let world_box = waml::solve::label::measure("customer {1}", &cfg);
            assert!(
                world_target <= world_box.h * zoom + 1e-9,
                "text outgrew its box at zoom {zoom}"
            );
        }
        assert_eq!(
            WORLD_FONT_SIZE, cfg.font_size,
            "the drawn world size must be the size the solver measured with"
        );
    }

    fn edge() -> crate::scene::SceneEdge {
        use waml::model::{RelEnd, RelationshipKind};
        use waml::solve::Rect as WRect;
        crate::scene::SceneEdge {
            source: WRect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            target: WRect {
                x: 200.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            kind: RelationshipKind::Aggregates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            points: vec![(20.0, 10.0), (200.0, 10.0)],
        }
    }

    #[test]
    fn a_terminal_label_steps_past_its_adornment_in_screen_px() {
        // The diamond at the `from` end is drawn at a FIXED screen size, so the
        // step past it cannot be a world-space gap: it must be the same number
        // of screen px at every zoom.
        let center = dvec2(40.0, 0.0);
        let attach = (20.0, 10.0);
        for zoom in [0.25_f64, 1.0, 3.0] {
            let nudge =
                screen_clearance(center, attach, &edge(), LabelSlot::TerminalFrom, zoom, 8.0);
            assert_eq!(
                nudge.x,
                2.0 * 8.0 + HEAD_GAP,
                "must clear the diamond at zoom {zoom}"
            );
        }
    }

    #[test]
    fn a_zoomed_out_label_is_pushed_further_off_the_stroke() {
        // The solver's world gap projects to less than LABEL_GAP at zoom < 1;
        // the top-up moves the box further onto the side it already chose (up,
        // here, since its centre is above the attach point).
        let center = dvec2(40.0, 0.0);
        let attach = (20.0, 10.0);
        let out = screen_clearance(center, attach, &edge(), LabelSlot::TerminalFrom, 0.25, 8.0);
        assert!(out.y < 0.0, "pushed up, away from the stroke: {out:?}");
        let at_one = screen_clearance(center, attach, &edge(), LabelSlot::TerminalFrom, 1.0, 8.0);
        assert_eq!(at_one.y, 0.0, "no top-up needed at zoom 1");
    }

    #[test]
    fn the_perpendicular_top_up_only_bites_when_zoomed_out() {
        assert_eq!((LABEL_GAP - WORLD_GAP * 1.0).max(0.0), 0.0);
        assert!((LABEL_GAP - WORLD_GAP * 0.25).max(0.0) > 0.0);
        // Never negative: zooming IN must not pull the label back onto the line.
        assert_eq!((LABEL_GAP - WORLD_GAP * 4.0).max(0.0), 0.0);
        // The top-up tops up the gap the SOLVER used; the two are independent
        // constants in two crates, so they have to be pinned to each other.
        assert_eq!(
            WORLD_GAP,
            waml::solve::label::LabelConfig::default().gap,
            "the assumed world gap must be the gap the solver lifted by"
        );
    }

    #[test]
    fn a_mid_route_name_is_also_lifted_off_the_stroke() {
        // The stroke is drawn screen-width whatever the zoom, so a relationship
        // name riding a mid-route leg needs the same screen gap a terminal gets.
        let attach = (110.0, 10.0);
        let center = dvec2(110.0, 0.0);
        let out = screen_clearance(center, attach, &edge(), LabelSlot::MidRoute, 0.25, 8.0);
        assert!(out.y < 0.0, "lifted off the horizontal stroke: {out:?}");
        assert_eq!(out.y, -(LABEL_GAP - WORLD_GAP * 0.25));
        // Nothing to step past mid-route: the adornments are at the ends.
        assert_eq!(out.x, 0.0);
    }

    #[test]
    fn a_mid_route_name_on_a_vertical_leg_steps_aside() {
        let mut edge = edge();
        edge.points = vec![(20.0, 10.0), (100.0, 10.0), (100.0, 200.0)];
        let attach = (100.0, 120.0);
        let center = dvec2(140.0, 120.0);
        let out = screen_clearance(center, attach, &edge, LabelSlot::MidRoute, 0.25, 8.0);
        assert_eq!(out.y, 0.0, "a vertical leg clears sideways: {out:?}");
        assert!(out.x > 0.0, "pushed right, away from the stroke: {out:?}");
    }

    #[test]
    fn a_slid_terminal_label_is_not_pushed_past_its_own_box() {
        // A candidate that already slid down the route is clear of the diamond
        // on its own; stepping it a further 20px would walk it out of the world
        // rect the solver reserved, and out of its non-overlap guarantee.
        let attach = (100.0, 10.0);
        let center = dvec2(120.0, 0.0);
        let out = screen_clearance(center, attach, &edge(), LabelSlot::TerminalFrom, 1.0, 8.0);
        assert_eq!(out.x, 0.0, "already past the adornment: {out:?}");
        // Partly slid: only the shortfall is made up.
        let attach = (25.0, 10.0);
        let out = screen_clearance(center, attach, &edge(), LabelSlot::TerminalFrom, 1.0, 8.0);
        assert_eq!(out.x, 2.0 * 8.0 + HEAD_GAP - 5.0);
    }

    #[test]
    fn a_diagonal_leader_draws_hairlines_not_a_filled_block() {
        // The edge pen FILLS its quad, so a single AABB from anchor to label
        // painted a solid opaque rectangle over everything between them.
        let start = dvec2(100.0, 100.0);
        let end = dvec2(180.0, 40.0);
        let bars = leader_bars(start, end, LEADER_THICKNESS);
        assert_eq!(bars.len(), 2, "an L, not one quad: {bars:?}");
        for bar in &bars {
            assert!(
                bar.size.x <= LEADER_THICKNESS || bar.size.y <= LEADER_THICKNESS,
                "every leg must be a hairline: {bar:?}"
            );
        }
        // The L runs anchor -> corner -> label, so the legs meet and the far
        // leg reaches the label end.
        assert!(bars[0].pos.x <= start.x.min(end.x) + LEADER_THICKNESS);
        assert!(bars[1].pos.y <= end.y.max(start.y));
        assert!(bars[1].pos.y + bars[1].size.y >= end.y.min(start.y));
    }

    #[test]
    fn an_axis_aligned_leader_is_a_single_bar() {
        let bars = leader_bars(dvec2(0.0, 10.0), dvec2(60.0, 10.0), LEADER_THICKNESS);
        assert_eq!(bars.len(), 1, "no degenerate second leg: {bars:?}");
        assert_eq!(bars[0].size.y, LEADER_THICKNESS);
    }

    #[test]
    fn a_leader_follows_its_box_s_screen_clearance() {
        // The leader was drawn to the UN-nudged world rect while the box moved,
        // so at zoom < 1 it stopped short of (or ended inside) its own box.
        let pos = dvec2(100.0, 50.0);
        let end = dvec2(100.0, 60.0);
        let nudge = dvec2(0.0, 6.0);
        let (moved_pos, moved_end) = nudged(pos, Some(end), nudge);
        assert_eq!(
            moved_pos - pos,
            moved_end.expect("a leader end") - end,
            "box and leader end must move by the same offset"
        );
        assert_eq!(
            nudged(pos, None, nudge).1,
            None,
            "no leader, nothing to move"
        );
    }
}
