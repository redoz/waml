//! Relationship-label TEXT policy for the native canvas: composes label
//! strings from the display toggles and reads back world-space placements the
//! solver computed. Geometry (where a label sits, which candidates it can
//! slide to, what it must avoid) lives in `waml::solve::label` now; this
//! module only decides WHAT text each edge wants labeled, plus the
//! flow-canvas mid-route helper (spec §2.6), which has not yet moved to
//! solver placement.

use crate::{diagram_display::ResolvedDiagramDisplay, scene::SceneEdge};
use makepad_widgets::{dvec2, DVec2};
use waml::{
    adornment::{End, Marker},
    model::AssocName,
    solve::label::{LabelRequest, LabelSlot},
};

/// How far along the route a terminal label steps, past whatever adornment sits
/// on the endpoint. Screen px, like the marker geometry it has to clear.
pub const HEAD_GAP: f64 = 4.0;

/// How far back along the route an endpoint adornment reaches, in screen px, so
/// a terminal label can step past it. Mirrors `canvas::geometry::marker_geometry`:
/// triangles and open arrows run one `marker_size` back from the tip, diamonds two.
///
/// Placement itself is world-space now, but adornments are drawn at a FIXED
/// screen size, so the clearance that keeps text off them cannot be projected --
/// a world-space gap collapses onto the arrowhead exactly when you zoom out.
pub fn marker_extent(marker: Marker, marker_size: f64) -> f64 {
    match marker {
        Marker::None => 0.0,
        Marker::OpenArrow | Marker::HollowTriangle => marker_size,
        Marker::HollowDiamond | Marker::FilledDiamond => 2.0 * marker_size,
    }
}

/// Where a label's box sits relative to its anchor.
///
/// Clearance off the stroke is perpendicular, so a horizontal route lifts its
/// text and a vertical route steps it aside. But lifting alone is not enough at
/// a terminal: a label centred on an anchor a few px in from a node border has
/// half its width buried under the card. So the horizontal cases also say which
/// way the box GROWS — away from the endpoint it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelAlign {
    /// Beside a vertical route, growing right and down from the anchor.
    Right,
    /// Lifted over a horizontal route, growing rightward (a `from` terminal).
    AboveStart,
    /// Lifted over a horizontal route, centred (a mid-route name).
    AboveCenter,
    /// Lifted over a horizontal route, growing leftward (a `to` terminal).
    AboveEnd,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabel {
    pub text: String,
    /// Point on the route the label hangs off, in world coordinates.
    pub anchor: (f64, f64),
    /// Clearance to add AFTER projecting `anchor`, in screen px. Adornments and
    /// stroke widths are zoom-invariant, so the nudge that keeps text off them
    /// cannot live in world units — a world-space offset collapses onto the line
    /// exactly when you zoom out, which is when the crowding is worst.
    pub offset: DVec2,
    pub align: LabelAlign,
}

/// Composes the label text each visible edge wants (terminal role/multiplicity
/// text at either end, plus an optional relationship name at the route's
/// middle), under the diagram's display toggles. Geometry — where each ends up,
/// which candidates it can slide to, what it must avoid — is the solver's job
/// (`waml::solve::label::place`); this only decides WHAT text exists and which
/// slot it belongs to.
pub fn label_requests(edges: &[SceneEdge], display: &ResolvedDiagramDisplay) -> Vec<LabelRequest> {
    let mut requests = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        push_requests(
            index,
            edge.kind,
            edge.name.as_ref(),
            &edge.from_end,
            &edge.to_end,
            display,
            &mut requests,
        );
    }
    requests
}

/// The same requests, composed from MODEL edges instead of scene edges, so the
/// pre-solve pass that sizes connected gaps to hold their terminal labels
/// (`connected_label_widths` -> `constrain::compile`) can be fed before any
/// geometry exists.
/// `edge` indexes into the slice given here, matching the `(BoxId, BoxId)` edge
/// list the solver is handed.
pub fn model_label_requests(
    edges: &[&waml::model::Edge],
    display: &ResolvedDiagramDisplay,
) -> Vec<LabelRequest> {
    let mut requests = Vec::new();
    for (index, edge) in edges.iter().enumerate() {
        push_requests(
            index,
            edge.kind,
            edge.name.as_ref(),
            &edge.from_end,
            &edge.to_end,
            display,
            &mut requests,
        );
    }
    requests
}

#[allow(clippy::too_many_arguments)]
fn push_requests(
    index: usize,
    kind: waml::model::RelationshipKind,
    name: Option<&AssocName>,
    from_end: &waml::model::RelEnd,
    to_end: &waml::model::RelEnd,
    display: &ResolvedDiagramDisplay,
    requests: &mut Vec<LabelRequest>,
) {
    if kind.is_ended() {
        for end in [End::From, End::To] {
            let end_data = match end {
                End::From => from_end,
                End::To => to_end,
            };
            let cardinality = display
                .show_cardinality
                .then(|| {
                    end_data
                        .multiplicity
                        .as_ref()
                        .map(|multiplicity| format!("{{{}}}", multiplicity.as_str()))
                })
                .flatten();
            let role = display
                .show_roles
                .then_some(end_data.role.as_deref())
                .flatten();
            let text = match (role, cardinality) {
                (Some(role), Some(cardinality)) => Some(format!("{role} {cardinality}")),
                (Some(role), None) => Some(role.to_string()),
                (None, Some(cardinality)) => Some(cardinality),
                (None, None) => None,
            };
            if let Some(text) = text {
                let slot = match end {
                    End::From => LabelSlot::TerminalFrom,
                    End::To => LabelSlot::TerminalTo,
                };
                requests.push(LabelRequest {
                    edge: index,
                    slot,
                    text,
                });
            }
        }
    }

    if display.show_labels {
        if let Some(name) = name.and_then(relationship_name) {
            requests.push(LabelRequest {
                edge: index,
                slot: LabelSlot::MidRoute,
                text: name.to_string(),
            });
        }
    }
}

/// Mid-route label anchor for a plain polyline (kind-agnostic; the class path
/// keeps its `SceneEdge`-typed entry points above). Used by the flow renderer
/// (spec §2.6) for guard/effect/carried-type text, without duplicating the
/// arc-length midpoint math.
pub fn mid_route_label(points: &[(f64, f64)], text: String) -> Option<EdgeLabel> {
    let (x, y) = polyline_midpoint(points)?;
    // Which side the text clears the route on depends on how the route runs
    // THERE: `Above` lifts it off a horizontal segment, but on a vertical one it
    // centres the text on the line and the stroke runs straight through the
    // glyphs. Step a vertical segment's label out to the right instead.
    let (offset, align) = clear_of_route(midpoint_orientation(points), None);
    Some(EdgeLabel {
        text,
        anchor: (x, y),
        offset,
        align,
    })
}

/// Which way a label steps to sit beside a route running in `orientation`, and
/// the alignment that keeps its box on that side. The gap is perpendicular to
/// the stroke: pushing a label ALONG the line only slides it to another part of
/// the same line.
///
/// `growth` is the route tangent's x component for a TERMINAL label, telling the
/// box to grow away from the endpoint it sits at; `None` centres it, which is
/// what a mid-route name wants.
fn clear_of_route(orientation: Orientation, growth: Option<f64>) -> (DVec2, LabelAlign) {
    match orientation {
        Orientation::Vertical => (dvec2(LABEL_GAP, 0.0), LabelAlign::Right),
        Orientation::Horizontal => {
            let align = match growth {
                None => LabelAlign::AboveCenter,
                Some(dx) if dx >= 0.0 => LabelAlign::AboveStart,
                Some(_) => LabelAlign::AboveEnd,
            };
            (dvec2(0.0, -LABEL_GAP), align)
        }
    }
}

/// Clearance between a route and the label riding alongside it, in world units.
pub const LABEL_GAP: f64 = 3.0;

enum Orientation {
    Horizontal,
    Vertical,
}

/// Orientation of the segment the arc-length midpoint falls on. Ties (a
/// zero-length or perfectly diagonal segment) read as horizontal, which is the
/// resting case for these orthogonal routes.
fn midpoint_orientation(points: &[(f64, f64)]) -> Orientation {
    let Some(segment) = midpoint_segment(points) else {
        return Orientation::Horizontal;
    };
    let dx = (segment[1].0 - segment[0].0).abs();
    let dy = (segment[1].1 - segment[0].1).abs();
    if dy > dx {
        Orientation::Vertical
    } else {
        Orientation::Horizontal
    }
}

fn midpoint_segment(points: &[(f64, f64)]) -> Option<[(f64, f64); 2]> {
    let total: f64 = points
        .windows(2)
        .map(|segment| (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1))
        .sum();
    if total <= f64::EPSILON {
        return None;
    }
    let mut remaining = total / 2.0;
    for segment in points.windows(2) {
        let length = (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1);
        if length <= f64::EPSILON {
            continue;
        }
        if remaining <= length {
            return Some([segment[0], segment[1]]);
        }
        remaining -= length;
    }
    points
        .windows(2)
        .last()
        .map(|segment| [segment[0], segment[1]])
}

/// Top-left corner at which to DRAW a label of `size`, given its anchor and
/// which way it aligns. Shared by the class and behavior canvases so a label
/// clears its route the same way on both.
pub fn aligned_text_pos(anchor: DVec2, size: DVec2, align: LabelAlign) -> DVec2 {
    // Every `Above*` case lifts by the full text height, so the gap is measured
    // from the BOTTOM of the glyphs rather than from their top-left corner.
    match align {
        LabelAlign::Right => anchor,
        LabelAlign::AboveStart => dvec2(anchor.x, anchor.y - size.y),
        LabelAlign::AboveCenter => dvec2(anchor.x - size.x * 0.5, anchor.y - size.y),
        LabelAlign::AboveEnd => dvec2(anchor.x - size.x, anchor.y - size.y),
    }
}

fn relationship_name(name: &AssocName) -> Option<&str> {
    match name {
        AssocName::Label(name) => Some(name),
        AssocName::Assoc(_) => None,
    }
}

fn polyline_midpoint(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let first = *points.first()?;
    let total: f64 = points
        .windows(2)
        .map(|segment| (segment[1].0 - segment[0].0).hypot(segment[1].1 - segment[0].1))
        .sum();
    if total <= f64::EPSILON {
        return Some(first);
    }

    let mut remaining = total / 2.0;
    for segment in points.windows(2) {
        let dx = segment[1].0 - segment[0].0;
        let dy = segment[1].1 - segment[0].1;
        let length = dx.hypot(dy);
        if length <= f64::EPSILON {
            continue;
        }
        if remaining <= length {
            let fraction = remaining / length;
            return Some((segment[0].0 + dx * fraction, segment[0].1 + dy * fraction));
        }
        remaining -= length;
    }
    points.last().copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_display::ResolvedDiagramDisplay;
    use crate::scene::SceneEdge;
    use waml::{
        model::{CardinalityVisibility, RelEnd, RelationshipKind},
        multiplicity::Multiplicity,
        solve::Rect,
    };

    fn display(cardinality: CardinalityVisibility) -> ResolvedDiagramDisplay {
        ResolvedDiagramDisplay {
            cardinality,
            ..Default::default()
        }
    }

    fn edge(points: Vec<(f64, f64)>) -> SceneEdge {
        SceneEdge {
            source: Rect {
                x: 0.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            target: Rect {
                x: 100.0,
                y: 0.0,
                w: 20.0,
                h: 20.0,
            },
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd {
                multiplicity: Multiplicity::parse("1"),
                ..Default::default()
            },
            to_end: RelEnd {
                multiplicity: Multiplicity::parse("0..*"),
                ..Default::default()
            },
            points,
        }
    }

    #[test]
    fn requests_follow_the_display_switches_and_carry_slot_identity() {
        let mut edge = edge(vec![(20.0, 10.0), (100.0, 10.0)]);
        edge.from_end.role = Some("orders".into());
        edge.name = Some(AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = false;
        assert!(label_requests(&[edge.clone()], &display).is_empty());

        display.show_roles = true;
        display.show_labels = true;
        let reqs = label_requests(&[edge], &display);
        let slots: Vec<_> = reqs.iter().map(|r| r.slot).collect();
        assert_eq!(slots, vec![LabelSlot::TerminalFrom, LabelSlot::MidRoute]);
        assert_eq!(reqs[0].text, "orders");
        assert_eq!(reqs[1].text, "places");
    }

    #[test]
    fn association_reference_is_not_painted_as_relationship_name() {
        let mut edge = edge(vec![(20.0, 10.0), (100.0, 10.0)]);
        edge.name = Some(AssocName::Assoc("employment".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        assert!(label_requests(&[edge], &display).is_empty());
    }

    #[test]
    fn mid_route_label_anchors_at_the_route_midpoint() {
        let label = mid_route_label(&[(0.0, 0.0), (100.0, 0.0)], "guard".into()).unwrap();
        assert_eq!(label.text, "guard");
        // Lifted clear of a horizontal route, not centred on it.
        // Anchored ON the route; the lift is screen-space clearance, so it stays
        // a constant distance off the stroke at every zoom.
        assert_eq!(label.anchor, (50.0, 0.0));
        assert_eq!(label.offset, dvec2(0.0, -LABEL_GAP));
        assert_eq!(label.align, LabelAlign::AboveCenter);
    }

    #[test]
    fn a_vertical_route_takes_its_label_beside_the_line() {
        // `Above` on a vertical segment centres the glyphs on the stroke, so
        // the route runs through the text. Step out to the right instead.
        let label = mid_route_label(&[(0.0, 0.0), (0.0, 100.0)], "else".into()).unwrap();
        assert_eq!(label.anchor, (0.0, 50.0));
        assert_eq!(label.offset, dvec2(LABEL_GAP, 0.0));
        assert_eq!(label.align, LabelAlign::Right);
    }

    #[test]
    fn orientation_follows_the_segment_the_midpoint_lands_on() {
        // The midpoint of this bend falls on the long vertical leg, even though
        // the route starts out horizontal.
        let label = mid_route_label(&[(0.0, 0.0), (10.0, 0.0), (10.0, 90.0)], "x".into()).unwrap();
        assert_eq!(label.align, LabelAlign::Right);
    }

    #[test]
    fn text_alignment_keeps_each_label_in_its_declared_open_direction() {
        let anchor = dvec2(100.0, 100.0);
        let size = dvec2(24.0, 10.0);
        assert_eq!(aligned_text_pos(anchor, size, LabelAlign::Right), anchor);
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::AboveCenter),
            dvec2(88.0, 90.0)
        );
    }

    #[test]
    fn bent_relationship_name_uses_polyline_arc_length_midpoint() {
        // The mid-route geometry is now the flow canvas's helper directly;
        // `label_requests` only emits the request, so exercise the geometry via
        // `mid_route_label` on the same bent polyline.
        let label =
            mid_route_label(&[(0.0, 0.0), (10.0, 0.0), (10.0, 90.0)], "places".into()).unwrap();
        assert_eq!(label.anchor, (10.0, 40.0));
        // The midpoint lands on the vertical leg, so the name clears sideways.
        assert_eq!(label.offset, dvec2(LABEL_GAP, 0.0));
    }
}
