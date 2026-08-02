use super::{primitives::ClassDrawResources, RenderSnapshot};
use crate::canvas::primitives::{fill_rect, font_raster_size, world_rect_to_screen};
use crate::edge_labels::{marker_extent, HEAD_GAP, LABEL_GAP};
use makepad_widgets::*;
use waml::adornment::{end_marker, End};
use waml::solve::label::LabelSlot;

const LABEL_PAD: f64 = 3.0;

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
        let pos = screen.pos + nudge;
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

/// Screen-space top-up on a world-space placement.
///
/// Placement is world-space, but strokes and endpoint adornments are drawn at a
/// FIXED screen size, so at zoom < 1 the solver's world gap projects to less
/// than it needs to be and the text lands on the arrowhead it was supposed to
/// step past. Two corrections, both in screen px:
///
/// * perpendicular to the route: top the projected gap back up to `LABEL_GAP`;
/// * along the route, for a TERMINAL label only: step past the adornment drawn
///   at that end (`marker_extent` + `HEAD_GAP`), which has no world size at all.
fn screen_clearance(
    center: DVec2,
    attach: (f64, f64),
    edge: &crate::scene::SceneEdge,
    slot: LabelSlot,
    zoom: f64,
    marker_size: f64,
) -> DVec2 {
    let Some(inward) = route_direction(&edge.points, slot) else {
        return dvec2(0.0, 0.0);
    };
    // World and screen axes differ only by a positive scale and a translation,
    // so a unit WORLD direction is a valid screen direction too, and the side
    // the solver picked reads the same in both spaces.
    let perp = (LABEL_GAP - LABEL_GAP * zoom).max(0.0);
    let (end, navigable) = match slot {
        LabelSlot::TerminalFrom => (End::From, edge.from_end.navigable),
        _ => (End::To, edge.to_end.navigable),
    };
    let along = marker_extent(end_marker(edge.kind, end, navigable), marker_size) + HEAD_GAP;
    let normal = dvec2(-inward.y, inward.x);
    let offset = center - dvec2(attach.0, attach.1);
    let side = offset.x * normal.x + offset.y * normal.y;
    let sign = if side < 0.0 { -1.0 } else { 1.0 };
    inward * along + normal * (sign * perp)
}

/// Unit route direction leading AWAY from the end a TERMINAL label belongs to.
/// `None` for a mid-route label (which sits on an arbitrary leg of the route,
/// so the terminal segment says nothing about the axis it clears on) or a
/// degenerate polyline.
fn route_direction(points: &[(f64, f64)], slot: LabelSlot) -> Option<DVec2> {
    if points.len() < 2 {
        return None;
    }
    let (a, b) = match slot {
        LabelSlot::TerminalFrom => (points[0], points[1]),
        LabelSlot::TerminalTo => (points[points.len() - 1], points[points.len() - 2]),
        LabelSlot::MidRoute => return None,
    };
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
        assert_eq!((LABEL_GAP - LABEL_GAP * 1.0).max(0.0), 0.0);
        assert!((LABEL_GAP - LABEL_GAP * 0.25).max(0.0) > 0.0);
        // Never negative: zooming IN must not pull the label back onto the line.
        assert_eq!((LABEL_GAP - LABEL_GAP * 4.0).max(0.0), 0.0);
    }
}
