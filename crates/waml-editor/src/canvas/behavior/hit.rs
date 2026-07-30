//! Behavior-canvas hit-testing (spec §5.1). `Empty` scenes hit nothing;
//! `Flow` scenes check nodes topmost-first, then routed edges within a
//! tolerance band. `Interaction` targets land in Task 8.

use super::scene::BehaviorScene;

/// World-space distance from `p` to segment `a`-`b`.
const EDGE_TOLERANCE: f64 = 6.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BehaviorTarget {
    FlowNode(String),
    FlowEdge(String),
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
    }
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
}
