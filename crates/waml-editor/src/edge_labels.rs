//! Pure relationship-label policy and terminal geometry for the native canvas.

use crate::{diagram_display::ResolvedDiagramDisplay, scene::SceneEdge};
use makepad_widgets::{dvec2, DVec2};
use waml::{
    adornment::{end_marker, End, Marker},
    model::AssocName,
};

/// How far along the route a terminal label steps, past whatever adornment sits
/// on the endpoint. Screen px, like the marker geometry it has to clear.
const HEAD_GAP: f64 = 4.0;

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

/// How far back along the route an endpoint adornment reaches, in screen px, so
/// a terminal label can step past it. Mirrors `canvas::geometry::marker_geometry`:
/// triangles and open arrows run one `marker_size` back from the tip, diamonds two.
pub fn marker_extent(marker: Marker, marker_size: f64) -> f64 {
    match marker {
        Marker::None => 0.0,
        Marker::OpenArrow | Marker::HollowTriangle => marker_size,
        Marker::HollowDiamond | Marker::FilledDiamond => 2.0 * marker_size,
    }
}

/// Labels each visible relationship end from the routed terminal segment, then
/// appends the optional relationship name at the route's middle point.
///
/// `marker_size` is the renderer's screen-space adornment size; terminal labels
/// step past the glyph actually drawn at their end rather than a fixed guess.
pub fn edge_end_labels(
    edge: &SceneEdge,
    display: &ResolvedDiagramDisplay,
    marker_size: f64,
) -> Vec<EdgeLabel> {
    let mut labels = Vec::new();
    if edge.kind.is_ended() && edge.points.len() >= 2 {
        for end in [End::From, End::To] {
            let end_data = match end {
                End::From => &edge.from_end,
                End::To => &edge.to_end,
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
                let (endpoint, open) = terminal_segment(edge, end);
                let navigable = match end {
                    End::From => edge.from_end.navigable,
                    End::To => edge.to_end.navigable,
                };
                let clearance =
                    marker_extent(end_marker(edge.kind, end, navigable), marker_size) + HEAD_GAP;
                labels.push(terminal_label(text, endpoint, open, clearance));
            }
        }
    }

    if display.show_labels {
        if let Some(name) = edge.name.as_ref().and_then(relationship_name) {
            if let Some(label) = mid_route_label(&edge.points, name.to_string()) {
                labels.push(label);
            }
        }
    }
    labels
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
const LABEL_GAP: f64 = 3.0;

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

fn terminal_segment(edge: &SceneEdge, end: End) -> ((f64, f64), (f64, f64)) {
    match end {
        End::From => (edge.points[0], edge.points[1]),
        End::To => {
            let last = edge.points.len() - 1;
            (edge.points[last], edge.points[last - 1])
        }
    }
}

/// Places one end's role/multiplicity text: `clearance` px inward along the
/// route (past the adornment on this endpoint), then `LABEL_GAP` px off the
/// stroke perpendicular to it.
///
/// The inward step alone is not enough — it walks the text along the line it is
/// meant to clear — so the perpendicular gap is what actually lifts it off, and
/// the alignment has to agree with that side or the box lands back on the line.
fn terminal_label(
    text: String,
    endpoint: (f64, f64),
    open: (f64, f64),
    clearance: f64,
) -> EdgeLabel {
    let dx = open.0 - endpoint.0;
    let dy = open.1 - endpoint.1;
    let length = dx.hypot(dy);
    let (dx, dy) = if length > f64::EPSILON {
        (dx / length, dy / length)
    } else {
        (0.0, -1.0)
    };
    let orientation = if dy.abs() > dx.abs() {
        Orientation::Vertical
    } else {
        Orientation::Horizontal
    };
    let (gap, align) = clear_of_route(orientation, Some(dx));
    EdgeLabel {
        text,
        anchor: endpoint,
        offset: dvec2(dx * clearance + gap.x, dy * clearance + gap.y),
        align,
    }
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

    /// Marker size used by the label tests; the class renderer passes its own.
    const MARKER: f64 = 8.0;

    #[test]
    fn terminal_geometry_uses_the_terminal_segment_open_space() {
        let labels = edge_end_labels(
            &edge(vec![(20.0, 10.0), (100.0, 10.0)]),
            &display(CardinalityVisibility::All),
            MARKER,
        );
        // Both ends anchor ON their endpoint and step INWARD along the route.
        assert_eq!(labels[0].anchor, (20.0, 10.0));
        assert!(labels[0].offset.x > 0.0);
        assert_eq!(labels[1].anchor, (100.0, 10.0));
        assert!(labels[1].offset.x < 0.0);
        // A horizontal route is cleared upward, not by sliding along itself, and
        // each box grows AWAY from its own endpoint so the small inward step is
        // not spent burying half the text under the node it just left.
        for label in &labels {
            assert_eq!(label.offset.y, -LABEL_GAP);
        }
        assert_eq!(labels[0].align, LabelAlign::AboveStart);
        assert_eq!(labels[1].align, LabelAlign::AboveEnd);
    }

    #[test]
    fn a_terminal_box_never_grows_back_over_its_own_endpoint() {
        // The regression this guards: `AboveCenter` at a terminal centres the
        // text on an anchor only a few px in from the node border, so half the
        // label lands on the card. Both ends must clear their own node.
        let size = dvec2(40.0, 10.0);
        let from = aligned_text_pos(dvec2(100.0, 50.0), size, LabelAlign::AboveStart);
        let to = aligned_text_pos(dvec2(300.0, 50.0), size, LabelAlign::AboveEnd);
        assert!(from.x >= 100.0, "from-end text starts at its anchor");
        assert!(to.x + size.x <= 300.0, "to-end text ends at its anchor");
    }

    #[test]
    fn a_terminal_label_steps_past_the_adornment_actually_drawn() {
        // `associates` with a navigable `to` end: bare diamond-free `from`, open
        // arrow at `to`. The adorned end has to step further in than the bare one.
        let mut edge = edge(vec![(20.0, 10.0), (100.0, 10.0)]);
        edge.to_end.navigable = Some(true);
        let labels = edge_end_labels(&edge, &display(CardinalityVisibility::All), MARKER);

        assert_eq!(labels[0].offset.x, HEAD_GAP, "bare end clears the gap only");
        assert_eq!(labels[1].offset.x, -(MARKER + HEAD_GAP), "past the arrow");
    }

    #[test]
    fn a_diamond_pushes_its_label_twice_as_far_as_an_arrow() {
        assert_eq!(marker_extent(Marker::None, MARKER), 0.0);
        assert_eq!(marker_extent(Marker::OpenArrow, MARKER), MARKER);
        assert_eq!(marker_extent(Marker::HollowTriangle, MARKER), MARKER);
        assert_eq!(marker_extent(Marker::HollowDiamond, MARKER), 2.0 * MARKER);
        assert_eq!(marker_extent(Marker::FilledDiamond, MARKER), 2.0 * MARKER);
    }

    #[test]
    fn a_vertical_route_takes_its_terminal_label_beside_the_line() {
        let labels = edge_end_labels(
            &edge(vec![(10.0, 20.0), (10.0, 100.0)]),
            &display(CardinalityVisibility::All),
            MARKER,
        );
        for label in &labels {
            assert_eq!(label.align, LabelAlign::Right);
            assert_eq!(label.offset.x, LABEL_GAP);
        }
        assert!(labels[0].offset.y > 0.0, "from end steps down the route");
        assert!(labels[1].offset.y < 0.0, "to end steps back up it");
    }

    #[test]
    fn association_reference_is_not_painted_as_relationship_name() {
        let mut edge = edge(vec![(20.0, 10.0), (100.0, 10.0)]);
        edge.name = Some(AssocName::Assoc("employment".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        assert!(edge_end_labels(&edge, &display, MARKER).is_empty());
    }

    #[test]
    fn straight_relationship_name_uses_segment_midpoint() {
        let mut edge = edge(vec![(0.0, 0.0), (100.0, 0.0)]);
        edge.name = Some(AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        let labels = edge_end_labels(&edge, &display, MARKER);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].anchor, (50.0, 0.0));
        assert_eq!(labels[0].offset, dvec2(0.0, -LABEL_GAP));
        assert_eq!(labels[0].align, LabelAlign::AboveCenter);
    }

    #[test]
    fn a_vertical_relationship_name_clears_the_line_sideways() {
        // Was hardcoded `Above` regardless of orientation, which centred the
        // name on a vertical stroke and drew the route through the glyphs.
        let mut edge = edge(vec![(0.0, 0.0), (0.0, 100.0)]);
        edge.name = Some(AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        let labels = edge_end_labels(&edge, &display, MARKER);

        assert_eq!(labels[0].align, LabelAlign::Right);
        assert_eq!(labels[0].offset, dvec2(LABEL_GAP, 0.0));
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
        let mut edge = edge(vec![(0.0, 0.0), (10.0, 0.0), (10.0, 90.0)]);
        edge.name = Some(AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        let labels = edge_end_labels(&edge, &display, MARKER);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].anchor, (10.0, 40.0));
        // The midpoint lands on the vertical leg, so the name clears sideways.
        assert_eq!(labels[0].offset, dvec2(LABEL_GAP, 0.0));
    }
}
