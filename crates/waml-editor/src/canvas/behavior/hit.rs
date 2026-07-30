//! Behavior-canvas hit-testing (spec §5.1). `Empty` scenes hit nothing;
//! `Flow` scenes check nodes topmost-first, then routed edges within a
//! tolerance band. `Interaction` scenes check, in priority order: messages,
//! then activation bars (resolving to their lifeline), then lifeline
//! heads/stems within a tolerance band, then fragment frame borders.

use super::scene::BehaviorScene;

/// World-space distance from `p` to segment `a`-`b`.
const EDGE_TOLERANCE: f64 = 6.0;
const LIFELINE_TOLERANCE: f64 = 6.0;
const FRAGMENT_BORDER_TOLERANCE: f64 = 6.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BehaviorTarget {
    FlowNode(String),
    FlowEdge(String),
    Lifeline(String),
    Message(String),
    Fragment(String),
}

fn point_in_rect(p: (f64, f64), rect: waml::solve::Rect) -> bool {
    p.0 >= rect.x && p.0 <= rect.x + rect.w && p.1 >= rect.y && p.1 <= rect.y + rect.h
}

fn distance_to_segment(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f64::EPSILON {
        return (p.0 - a.0).hypot(p.1 - a.1);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq).clamp(0.0, 1.0);
    let (cx, cy) = (a.0 + dx * t, a.1 + dy * t);
    (p.0 - cx).hypot(p.1 - cy)
}

fn distance_to_polyline(p: (f64, f64), points: &[(f64, f64)]) -> f64 {
    points
        .windows(2)
        .map(|seg| distance_to_segment(p, seg[0], seg[1]))
        .fold(f64::INFINITY, f64::min)
}

/// World-space distance from `p` to the rectangle's boundary (its four sides,
/// walked as a closed polyline) -- true near the border, large in the
/// interior, unlike `point_in_rect`.
fn distance_to_rect_border(p: (f64, f64), rect: waml::solve::Rect) -> f64 {
    let corners = [
        (rect.x, rect.y),
        (rect.x + rect.w, rect.y),
        (rect.x + rect.w, rect.y + rect.h),
        (rect.x, rect.y + rect.h),
        (rect.x, rect.y),
    ];
    distance_to_polyline(p, &corners)
}

pub(crate) fn hit_test(scene: &BehaviorScene, world: (f64, f64)) -> Option<BehaviorTarget> {
    match scene {
        BehaviorScene::Empty { .. } => None,
        BehaviorScene::Flow { nodes, edges, .. } => {
            for node in nodes.iter().rev() {
                if point_in_rect(world, node.rect) {
                    return Some(BehaviorTarget::FlowNode(node.key.clone()));
                }
            }
            edges
                .iter()
                .find(|edge| distance_to_polyline(world, &edge.points) <= EDGE_TOLERANCE)
                .map(|edge| BehaviorTarget::FlowEdge(edge.key.clone()))
        }
        BehaviorScene::Interaction {
            lifelines,
            activations,
            messages,
            fragments,
        } => {
            for message in messages {
                let hit = match message.self_loop {
                    Some(rect) => distance_to_rect_border(world, rect) <= EDGE_TOLERANCE,
                    None => {
                        distance_to_segment(
                            world,
                            (message.from_x, message.y),
                            (message.to_x, message.y),
                        ) <= EDGE_TOLERANCE
                    }
                } || message
                    .label_rect
                    .is_some_and(|rect| point_in_rect(world, rect));
                if hit {
                    return Some(BehaviorTarget::Message(message.id.clone()));
                }
            }
            for activation in activations {
                if point_in_rect(world, activation.rect) {
                    return Some(BehaviorTarget::Lifeline(activation.lifeline.clone()));
                }
            }
            for lifeline in lifelines {
                let on_head = point_in_rect(world, lifeline.head);
                let on_stem = distance_to_segment(
                    world,
                    (lifeline.stem_x, lifeline.stem_top),
                    (lifeline.stem_x, lifeline.stem_bottom),
                ) <= LIFELINE_TOLERANCE;
                if on_head || on_stem {
                    return Some(BehaviorTarget::Lifeline(lifeline.id.clone()));
                }
            }
            // Innermost-first (`solve::interaction` pushes each fragment only
            // after walking its children), so the topmost-drawn frame wins.
            fragments
                .iter()
                .find(|fragment| {
                    distance_to_rect_border(world, fragment.rect) <= FRAGMENT_BORDER_TOLERANCE
                })
                .map(|fragment| BehaviorTarget::Fragment(fragment.id.clone()))
        }
    }
}

/// The world-space bounding rect of one target's geometry, or `None` when the
/// target is not in `scene` (a stale selection). Drives Fit to Selection.
pub(crate) fn target_bounds(
    scene: &BehaviorScene,
    target: &BehaviorTarget,
) -> Option<waml::solve::Rect> {
    match (scene, target) {
        (BehaviorScene::Empty { .. }, _) => None,
        (BehaviorScene::Flow { nodes, .. }, BehaviorTarget::FlowNode(key)) => {
            nodes.iter().find(|n| &n.key == key).map(|n| n.rect)
        }
        (BehaviorScene::Flow { edges, .. }, BehaviorTarget::FlowEdge(key)) => edges
            .iter()
            .find(|e| &e.key == key)
            .and_then(|e| polyline_bounds(&e.points)),
        (BehaviorScene::Interaction { lifelines, .. }, BehaviorTarget::Lifeline(id)) => lifelines
            .iter()
            .find(|l| &l.id == id)
            .map(|l| waml::solve::Rect {
                x: l.head.x.min(l.stem_x),
                y: l.head.y.min(l.stem_top),
                w: (l.head.x + l.head.w).max(l.stem_x) - l.head.x.min(l.stem_x),
                h: (l.head.y + l.head.h).max(l.stem_bottom) - l.head.y.min(l.stem_top),
            }),
        (BehaviorScene::Interaction { messages, .. }, BehaviorTarget::Message(id)) => messages
            .iter()
            .find(|m| &m.id == id)
            .and_then(|m| match m.self_loop {
                Some(rect) => Some(rect),
                None => polyline_bounds(&[(m.from_x, m.y), (m.to_x, m.y)]),
            }),
        (BehaviorScene::Interaction { fragments, .. }, BehaviorTarget::Fragment(id)) => {
            fragments.iter().find(|f| &f.id == id).map(|f| f.rect)
        }
        // A target from the other scene family: not in this scene.
        (BehaviorScene::Flow { .. }, _) | (BehaviorScene::Interaction { .. }, _) => None,
    }
}

fn polyline_bounds(points: &[(f64, f64)]) -> Option<waml::solve::Rect> {
    let (first, rest) = points.split_first()?;
    let (mut min_x, mut min_y) = *first;
    let (mut max_x, mut max_y) = *first;
    for (x, y) in rest {
        min_x = min_x.min(*x);
        min_y = min_y.min(*y);
        max_x = max_x.max(*x);
        max_y = max_y.max(*y);
    }
    Some(waml::solve::Rect {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
    })
}

#[cfg(test)]
mod tests {
    use super::super::scene::{FlowEdgeGeo, FlowNodeGeo};
    use super::*;
    use waml::model::FlowNodeKind;
    use waml::solve::Rect;

    #[test]
    fn empty_scene_hits_nothing() {
        let scene = BehaviorScene::Empty {
            message: "No renderable elements".into(),
        };
        assert_eq!(hit_test(&scene, (10.0, 10.0)), None);
    }

    fn node(key: &str, rect: Rect) -> FlowNodeGeo {
        FlowNodeGeo {
            key: key.into(),
            kind: FlowNodeKind::Plain,
            rect,
            title: key.into(),
            lines: Vec::new(),
            type_name: None,
            refines: false,
        }
    }

    #[test]
    fn flow_hit_prefers_node_over_edge_under_it() {
        let scene = BehaviorScene::Flow {
            nodes: vec![node(
                "n1",
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 40.0,
                },
            )],
            edges: vec![FlowEdgeGeo {
                key: "e1".into(),
                points: vec![(-50.0, 20.0), (150.0, 20.0)],
                label: None,
            }],
            off_page: Vec::new(),
            groups: Vec::new(),
        };
        // (50, 20) sits inside the node rect AND on the route -- node wins.
        assert_eq!(
            hit_test(&scene, (50.0, 20.0)),
            Some(BehaviorTarget::FlowNode("n1".into()))
        );
        // Off the node but still on the route -> edge.
        assert_eq!(
            hit_test(&scene, (140.0, 20.0)),
            Some(BehaviorTarget::FlowEdge("e1".into()))
        );
    }

    #[test]
    fn flow_edge_hits_within_tolerance_band() {
        let scene = BehaviorScene::Flow {
            nodes: Vec::new(),
            edges: vec![FlowEdgeGeo {
                key: "e1".into(),
                points: vec![(0.0, 0.0), (100.0, 0.0)],
                label: None,
            }],
            off_page: Vec::new(),
            groups: Vec::new(),
        };
        assert_eq!(
            hit_test(&scene, (50.0, 4.0)),
            Some(BehaviorTarget::FlowEdge("e1".into()))
        );
        assert_eq!(hit_test(&scene, (50.0, 20.0)), None);
    }

    use super::super::scene::{ActivationGeo, FragmentGeo, LifelineGeo, MessageGeo};
    use crate::node_style::AccentBucket;
    use waml::model::{FragmentKind, MessageVerb};

    fn lifeline(id: &str, stem_x: f64) -> LifelineGeo {
        LifelineGeo {
            id: id.into(),
            head: Rect {
                x: stem_x - 20.0,
                y: 0.0,
                w: 40.0,
                h: 20.0,
            },
            stem_x,
            stem_top: 20.0,
            stem_bottom: 200.0,
            destroyed: false,
            label: id.into(),
            bucket: AccentBucket::None,
        }
    }

    fn fragment(id: &str, rect: Rect) -> FragmentGeo {
        FragmentGeo {
            id: id.into(),
            kind: FragmentKind::Alt,
            rect,
            depth: 0,
            operands: Vec::new(),
        }
    }

    #[test]
    fn message_beats_enclosing_fragment() {
        let scene = BehaviorScene::Interaction {
            lifelines: vec![lifeline("a", 0.0), lifeline("b", 100.0)],
            activations: Vec::new(),
            messages: vec![MessageGeo {
                id: "m0".into(),
                verb: MessageVerb::Calls,
                from_x: 0.0,
                to_x: 100.0,
                y: 50.0,
                self_loop: None,
                label: None,
                label_rect: None,
            }],
            fragments: vec![fragment(
                "f0",
                Rect {
                    x: -10.0,
                    y: 10.0,
                    w: 120.0,
                    h: 80.0,
                },
            )],
        };
        // (50, 50) sits on the message line AND well inside the fragment's
        // interior (far from its border) -- message wins.
        assert_eq!(
            hit_test(&scene, (50.0, 50.0)),
            Some(BehaviorTarget::Message("m0".into()))
        );
    }

    #[test]
    fn activation_bar_resolves_to_its_lifeline() {
        let scene = BehaviorScene::Interaction {
            lifelines: vec![lifeline("a", 0.0)],
            activations: vec![ActivationGeo {
                lifeline: "a".into(),
                rect: Rect {
                    x: -6.0,
                    y: 40.0,
                    w: 12.0,
                    h: 60.0,
                },
                depth: 0,
                unclosed: false,
            }],
            messages: Vec::new(),
            fragments: Vec::new(),
        };
        assert_eq!(
            hit_test(&scene, (0.0, 60.0)),
            Some(BehaviorTarget::Lifeline("a".into()))
        );
    }

    #[test]
    fn fragment_border_hits_fragment_but_interior_empty_space_does_not() {
        let scene = BehaviorScene::Interaction {
            lifelines: Vec::new(),
            activations: Vec::new(),
            messages: Vec::new(),
            fragments: vec![fragment(
                "f0",
                Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 100.0,
                },
            )],
        };
        // On the top border.
        assert_eq!(
            hit_test(&scene, (50.0, 0.0)),
            Some(BehaviorTarget::Fragment("f0".into()))
        );
        // Deep in the empty interior, far from every side.
        assert_eq!(hit_test(&scene, (50.0, 50.0)), None);
    }
}
