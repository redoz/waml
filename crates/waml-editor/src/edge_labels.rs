//! Pure relationship-label policy and terminal geometry for the native canvas.

use crate::{
    diagram_display::ResolvedDiagramDisplay,
    scene::{attribute_cardinality_text, SceneEdge},
};
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
            let cardinality =
                attribute_cardinality_text(end_data.multiplicity.as_ref(), display.cardinality);
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
        if let Some(name) = edge.name.as_ref().map(relationship_name) {
            if let Some(&(x, y)) = edge.points.get(edge.points.len() / 2) {
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

fn relationship_name(name: &AssocName) -> &str {
    match name {
        AssocName::Label(name) | AssocName::Assoc(name) => name,
    }
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
}
