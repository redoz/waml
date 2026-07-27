//! Pure relationship-label policy and terminal geometry for the native canvas.

use crate::{diagram_display::ResolvedDiagramDisplay, scene::SceneEdge};
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
