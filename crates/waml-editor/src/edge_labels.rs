//! Pure relationship-label policy and terminal geometry for the native canvas.

use crate::{diagram_display::ResolvedDiagramDisplay, scene::SceneEdge};
use makepad_widgets::{dvec2, DVec2};
use waml::{adornment::End, model::AssocName};

const TERMINAL_OFFSET: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelAlign {
    Left,
    Right,
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EdgeLabel {
    pub text: String,
    pub anchor: (f64, f64),
    pub align: LabelAlign,
}

/// Labels each visible relationship end from the routed terminal segment, then
/// appends the optional relationship name at the route's middle point.
pub fn edge_end_labels(edge: &SceneEdge, display: &ResolvedDiagramDisplay) -> Vec<EdgeLabel> {
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
                labels.push(terminal_label(text, endpoint, open));
            }
        }
    }

    if display.show_labels {
        if let Some(name) = edge.name.as_ref().and_then(relationship_name) {
            if let Some((x, y)) = polyline_midpoint(&edge.points) {
                labels.push(EdgeLabel {
                    text: name.to_string(),
                    anchor: (x, y - TERMINAL_OFFSET),
                    align: LabelAlign::Above,
                });
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
    match midpoint_orientation(points) {
        Orientation::Vertical => Some(EdgeLabel {
            text,
            anchor: (x + TERMINAL_OFFSET, y),
            align: LabelAlign::Right,
        }),
        Orientation::Horizontal => Some(EdgeLabel {
            text,
            anchor: (x, y - LABEL_GAP),
            align: LabelAlign::Above,
        }),
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
    match align {
        LabelAlign::Left => dvec2(anchor.x - size.x, anchor.y),
        LabelAlign::Right => anchor,
        LabelAlign::Above => dvec2(anchor.x - size.x * 0.5, anchor.y - size.y),
        LabelAlign::Below => dvec2(anchor.x - size.x * 0.5, anchor.y),
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

fn terminal_label(text: String, endpoint: (f64, f64), open: (f64, f64)) -> EdgeLabel {
    let dx = open.0 - endpoint.0;
    let dy = open.1 - endpoint.1;
    let length = dx.hypot(dy);
    let (dx, dy) = if length > f64::EPSILON {
        (dx / length, dy / length)
    } else {
        (0.0, -1.0)
    };
    let align = if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            LabelAlign::Right
        } else {
            LabelAlign::Left
        }
    } else if dy >= 0.0 {
        LabelAlign::Below
    } else {
        LabelAlign::Above
    };
    EdgeLabel {
        text,
        anchor: (
            endpoint.0 + dx * TERMINAL_OFFSET,
            endpoint.1 + dy * TERMINAL_OFFSET,
        ),
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

    #[test]
    fn terminal_geometry_uses_the_terminal_segment_open_space() {
        let labels = edge_end_labels(
            &edge(vec![(20.0, 10.0), (100.0, 10.0)]),
            &display(CardinalityVisibility::All),
        );
        assert_eq!(labels[0].align, LabelAlign::Right);
        assert!(labels[0].anchor.0 > 20.0);
        assert_eq!(labels[1].align, LabelAlign::Left);
        assert!(labels[1].anchor.0 < 100.0);
    }

    #[test]
    fn association_reference_is_not_painted_as_relationship_name() {
        let mut edge = edge(vec![(20.0, 10.0), (100.0, 10.0)]);
        edge.name = Some(AssocName::Assoc("employment".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        assert!(edge_end_labels(&edge, &display).is_empty());
    }

    #[test]
    fn straight_relationship_name_uses_segment_midpoint() {
        let mut edge = edge(vec![(0.0, 0.0), (100.0, 0.0)]);
        edge.name = Some(AssocName::Label("places".into()));
        let mut display = display(CardinalityVisibility::Off);
        display.show_roles = false;
        display.show_cardinality = false;
        display.show_labels = true;

        let labels = edge_end_labels(&edge, &display);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].anchor, (50.0, -TERMINAL_OFFSET));
    }

    #[test]
    fn mid_route_label_anchors_at_the_route_midpoint() {
        let label = mid_route_label(&[(0.0, 0.0), (100.0, 0.0)], "guard".into()).unwrap();
        assert_eq!(label.text, "guard");
        // Lifted clear of a horizontal route, not centred on it.
        assert_eq!(label.anchor, (50.0, -LABEL_GAP));
        assert_eq!(label.align, LabelAlign::Above);
    }

    #[test]
    fn a_vertical_route_takes_its_label_beside_the_line() {
        // `Above` on a vertical segment centres the glyphs on the stroke, so
        // the route runs through the text. Step out to the right instead.
        let label = mid_route_label(&[(0.0, 0.0), (0.0, 100.0)], "else".into()).unwrap();
        assert_eq!(label.anchor, (TERMINAL_OFFSET, 50.0));
        assert_eq!(label.align, LabelAlign::Right);
    }

    #[test]
    fn orientation_follows_the_segment_the_midpoint_lands_on() {
        // The midpoint of this bend falls on the long vertical leg, even though
        // the route starts out horizontal.
        let label =
            mid_route_label(&[(0.0, 0.0), (10.0, 0.0), (10.0, 90.0)], "x".into()).unwrap();
        assert_eq!(label.align, LabelAlign::Right);
    }

    #[test]
    fn text_alignment_keeps_each_label_in_its_declared_open_direction() {
        let anchor = dvec2(100.0, 100.0);
        let size = dvec2(24.0, 10.0);
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::Left),
            dvec2(76.0, 100.0)
        );
        assert_eq!(aligned_text_pos(anchor, size, LabelAlign::Right), anchor);
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::Above),
            dvec2(88.0, 90.0)
        );
        assert_eq!(
            aligned_text_pos(anchor, size, LabelAlign::Below),
            dvec2(88.0, 100.0)
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

        let labels = edge_end_labels(&edge, &display);

        assert_eq!(labels.len(), 1);
        assert_eq!(labels[0].anchor, (10.0, 40.0 - TERMINAL_OFFSET));
    }
}
