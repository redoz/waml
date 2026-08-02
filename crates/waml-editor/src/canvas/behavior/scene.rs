//! `BehaviorScene`: what `BehaviorSurface` draws (spec §5.3). `Flow` landed in
//! Task 7; `Interaction` lands here.

use crate::node_style::AccentBucket;
use waml::model::{FlowNodeKind, FragmentKind, MessageKind};
use waml::solve::{Rect, SolvedGroup};

/// One solved flow node's drawable geometry (spec §4.1).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowNodeGeo {
    pub key: String,
    pub kind: FlowNodeKind,
    pub rect: Rect,
    pub title: String,
    /// `entry:`/`do:`/`exit:` body lines (state-machine flavor only).
    pub lines: Vec<String>,
    /// Resolved `:Type` line for an `Object` node.
    pub type_name: Option<String>,
    /// Whether this `Plain` node refines another flow document (footer glyph).
    pub refines: bool,
}

/// One solved flow edge's drawable geometry. The arrowhead always lands at
/// `points.last()` -- `Route.source`/`Route.target` (and therefore these
/// points) are already the TRUE direction regardless of ranking reversal.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowEdgeGeo {
    pub key: String,
    pub points: Vec<(f64, f64)>,
    /// `trigger [guard] / effect`, `else`, or the carried-type text.
    pub label: Option<String>,
}

/// A dangling cross-document edge, rendered as a short outbound stub.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowOffPageGeo {
    pub edge_key: String,
    pub points: Vec<(f64, f64)>,
    pub target_title: String,
}

/// One solved lifeline's drawable geometry (spec §4.2). `bucket` is the
/// referenced classifier's accent bucket, resolved once at scene-build time
/// (`AccentBucket::Unknown` when the lifeline carries no resolvable `ref_`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LifelineGeo {
    pub id: String,
    pub head: Rect,
    pub stem_x: f64,
    pub stem_top: f64,
    pub stem_bottom: f64,
    pub destroyed: bool,
    /// The classifier the lifeline is typed by -- the bottom line of a named
    /// head, the only line of an anonymous one.
    pub label: String,
    /// The instance name (`as checkout`), when the author gave one. UML would
    /// draw the pair as `checkout : Order`; the head stacks them instead.
    pub instance: Option<String>,
    pub bucket: AccentBucket,
}

/// One solved activation bar's drawable geometry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActivationGeo {
    pub lifeline: String,
    pub rect: Rect,
    pub depth: u8,
    pub unclosed: bool,
}

/// One solved message's drawable geometry.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MessageGeo {
    pub id: String,
    pub verb: MessageKind,
    pub from_x: f64,
    pub to_x: f64,
    pub y: f64,
    pub self_loop: Option<Rect>,
    pub label: Option<String>,
    pub label_rect: Option<Rect>,
}

/// One operand of a solved fragment.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OperandGeo {
    pub divider_y: Option<f64>,
    pub guard_text: String,
    pub guard_rect: Rect,
}

/// One solved combined-fragment frame.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FragmentGeo {
    pub id: String,
    pub kind: FragmentKind,
    pub rect: Rect,
    pub depth: u8,
    pub operands: Vec<OperandGeo>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum BehaviorScene {
    Empty {
        message: String,
    },
    Flow {
        nodes: Vec<FlowNodeGeo>,
        edges: Vec<FlowEdgeGeo>,
        off_page: Vec<FlowOffPageGeo>,
        groups: Vec<SolvedGroup>,
    },
    Interaction {
        lifelines: Vec<LifelineGeo>,
        activations: Vec<ActivationGeo>,
        messages: Vec<MessageGeo>,
        fragments: Vec<FragmentGeo>,
    },
}

impl BehaviorScene {
    /// How many elements the status bar counts for this scene: flow nodes for a
    /// flow, participant lifelines for an interaction, nothing for `Empty`.
    ///
    /// `App::sync_statusbar` needs this because it previously only ever read
    /// `ClassDiagramSurface`, which left a behavior document showing whatever
    /// count the last class diagram had.
    pub(crate) fn element_count(&self) -> usize {
        match self {
            BehaviorScene::Empty { .. } => 0,
            BehaviorScene::Flow { nodes, .. } => nodes.len(),
            BehaviorScene::Interaction { lifelines, .. } => lifelines.len(),
        }
    }

    /// The world-space bounding box of everything this scene draws, or `None`
    /// for an `Empty` scene (mirrors `crate::scene::bounding_box`). Drives the
    /// load-time fit and the view bar's Fit to Size (spec §4, Task 6).
    pub(crate) fn bounds(&self) -> Option<Rect> {
        let mut points: Vec<(f64, f64)> = Vec::new();
        match self {
            BehaviorScene::Empty { .. } => {}
            BehaviorScene::Flow {
                nodes,
                edges,
                off_page,
                groups,
            } => {
                for node in nodes {
                    points.push((node.rect.x, node.rect.y));
                    points.push((node.rect.x + node.rect.w, node.rect.y + node.rect.h));
                }
                for group in groups {
                    points.push((group.rect.x, group.rect.y));
                    points.push((group.rect.x + group.rect.w, group.rect.y + group.rect.h));
                }
                for point in edges
                    .iter()
                    .flat_map(|e| e.points.iter())
                    .chain(off_page.iter().flat_map(|s| s.points.iter()))
                {
                    points.push(*point);
                }
            }
            BehaviorScene::Interaction {
                lifelines,
                activations,
                messages,
                fragments,
            } => {
                for lifeline in lifelines {
                    points.push((lifeline.head.x, lifeline.head.y));
                    points.push((
                        lifeline.head.x + lifeline.head.w,
                        lifeline.head.y + lifeline.head.h,
                    ));
                    points.push((lifeline.stem_x, lifeline.stem_top));
                    points.push((lifeline.stem_x, lifeline.stem_bottom));
                }
                for rect in activations
                    .iter()
                    .map(|a| a.rect)
                    .chain(fragments.iter().map(|f| f.rect))
                    .chain(messages.iter().filter_map(|m| m.self_loop))
                    .chain(messages.iter().filter_map(|m| m.label_rect))
                {
                    points.push((rect.x, rect.y));
                    points.push((rect.x + rect.w, rect.y + rect.h));
                }
                for message in messages {
                    points.push((message.from_x, message.y));
                    points.push((message.to_x, message.y));
                }
            }
        }
        let (first, rest) = points.split_first()?;
        let (mut min_x, mut min_y) = *first;
        let (mut max_x, mut max_y) = *first;
        for (x, y) in rest {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x);
            max_y = max_y.max(*y);
        }
        Some(Rect {
            x: min_x,
            y: min_y,
            w: max_x - min_x,
            h: max_y - min_y,
        })
    }
}

impl Default for BehaviorScene {
    fn default() -> Self {
        BehaviorScene::Empty {
            message: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_scene_has_no_bounds() {
        assert_eq!(
            BehaviorScene::Empty {
                message: "x".into()
            }
            .bounds(),
            None
        );
    }

    #[test]
    fn flow_bounds_cover_nodes_and_route_points() {
        let scene = BehaviorScene::Flow {
            nodes: vec![FlowNodeGeo {
                key: "n1".into(),
                kind: FlowNodeKind::Plain,
                rect: Rect {
                    x: 10.0,
                    y: 20.0,
                    w: 100.0,
                    h: 40.0,
                },
                title: "n1".into(),
                lines: Vec::new(),
                type_name: None,
                refines: false,
            }],
            edges: vec![FlowEdgeGeo {
                key: "e1".into(),
                points: vec![(-30.0, 60.0), (60.0, 200.0)],
                label: None,
            }],
            off_page: Vec::new(),
            groups: Vec::new(),
        };
        assert_eq!(
            scene.bounds(),
            Some(Rect {
                x: -30.0,
                y: 20.0,
                w: 140.0,
                h: 180.0,
            })
        );
    }

    #[test]
    fn interaction_bounds_cover_heads_stems_and_messages() {
        let scene = BehaviorScene::Interaction {
            lifelines: vec![LifelineGeo {
                id: "a".into(),
                head: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 40.0,
                    h: 20.0,
                },
                stem_x: 20.0,
                stem_top: 20.0,
                stem_bottom: 300.0,
                destroyed: false,
                label: "a".into(),
                instance: None,
                bucket: AccentBucket::None,
            }],
            activations: Vec::new(),
            messages: vec![MessageGeo {
                id: "m0".into(),
                verb: MessageKind::SyncCall,
                from_x: 20.0,
                to_x: 220.0,
                y: 60.0,
                self_loop: None,
                label: None,
                label_rect: None,
            }],
            fragments: Vec::new(),
        };
        assert_eq!(
            scene.bounds(),
            Some(Rect {
                x: 0.0,
                y: 0.0,
                w: 220.0,
                h: 300.0,
            })
        );
    }
}
