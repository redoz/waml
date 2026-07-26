use super::placement::PlacementInteraction;
use super::{InteractionEffects, ReleaseIntent, SurfaceIntent, SELECT_SLOP};
use crate::canvas::class::selection::SelectionState;
use crate::canvas::viewport::{ViewportController, ViewportSnapshot};
use crate::scene::{Scene, SceneNode};
use makepad_widgets::{DVec2, Rect};

#[cfg(test)]
pub(super) const EVENT_PRIORITY: [&str; 12] = [
    "camera_interval",
    "escape_cancel",
    "dwell_timeout",
    "preview_frame",
    "pinch",
    "secondary_down",
    "primary_down",
    "move",
    "primary_up",
    "other_up",
    "hover",
    "scroll",
];

pub(super) fn is_click(down: DVec2, up: DVec2) -> bool {
    (up - down).length() < SELECT_SLOP
}

pub(super) fn node_at(
    nodes: &[SceneNode],
    viewport: ViewportSnapshot,
    abs: DVec2,
) -> Option<usize> {
    nodes.iter().enumerate().rev().find_map(|(index, node)| {
        let (local_x, local_y) = viewport.camera.world_to_local(node.rect.x, node.rect.y);
        let screen = Rect {
            pos: viewport.view_rect.pos + makepad_widgets::dvec2(local_x, local_y),
            size: makepad_widgets::dvec2(
                node.rect.w * viewport.camera.zoom,
                node.rect.h * viewport.camera.zoom,
            ),
        };
        screen.contains(abs).then_some(index)
    })
}

pub(super) fn footer_screen_rect(node: &SceneNode, screen: Rect, zoom: f64) -> Option<Rect> {
    use crate::card::{self, Block};
    let placed = card::measure(&card::class_shape(node, &card::mono_sheet()));
    let footer = placed
        .blocks
        .iter()
        .find(|block| block.block == Block::Footer)?;
    Some(Rect {
        pos: makepad_widgets::dvec2(
            screen.pos.x + footer.x * zoom,
            screen.pos.y + footer.y * zoom,
        ),
        size: makepad_widgets::dvec2(footer.w * zoom, footer.h * zoom),
    })
}

pub(super) fn classify_release(
    down_abs: DVec2,
    up_abs: DVec2,
    nodes: &[SceneNode],
    viewport: ViewportSnapshot,
) -> ReleaseIntent {
    if !is_click(down_abs, up_abs) {
        return ReleaseIntent::NotClick;
    }
    let Some(index) = node_at(nodes, viewport, up_abs) else {
        return ReleaseIntent::Deselect;
    };
    let node = &nodes[index];
    let (local_x, local_y) = viewport.camera.world_to_local(node.rect.x, node.rect.y);
    let screen = Rect {
        pos: viewport.view_rect.pos + makepad_widgets::dvec2(local_x, local_y),
        size: makepad_widgets::dvec2(
            node.rect.w * viewport.camera.zoom,
            node.rect.h * viewport.camera.zoom,
        ),
    };
    if footer_screen_rect(node, screen, viewport.camera.zoom)
        .is_some_and(|footer| footer.contains(up_abs))
    {
        ReleaseIntent::ToggleExpand {
            key: node.key.clone(),
        }
    } else {
        ReleaseIntent::Select {
            key: node.key.clone(),
        }
    }
}

#[derive(Default)]
pub(super) struct ClassInteraction;

impl ClassInteraction {
    pub(super) fn secondary_down(
        &mut self,
        abs: DVec2,
        scene: &Scene,
        viewport: ViewportSnapshot,
    ) -> InteractionEffects {
        let intent = node_at(&scene.nodes, viewport, abs).map(|index| SurfaceIntent::NodeMenu {
            abs,
            key: scene.nodes[index].key.clone(),
        });
        InteractionEffects {
            consumed: intent.is_some(),
            intent,
            ..Default::default()
        }
    }

    pub(super) fn primary_down(
        &mut self,
        abs: DVec2,
        scene: &Scene,
        viewport: &mut ViewportController,
        placement: &mut PlacementInteraction,
    ) -> InteractionEffects {
        viewport.begin_pan(abs);
        let snapshot = viewport.snapshot();
        if let Some(index) = node_at(&scene.nodes, snapshot, abs) {
            let node = &scene.nodes[index];
            let (world_x, world_y) = snapshot.camera.local_to_world(
                abs.x - snapshot.view_rect.pos.x,
                abs.y - snapshot.view_rect.pos.y,
            );
            placement.begin_drag(
                &node.key,
                abs,
                (world_x - node.rect.x, world_y - node.rect.y),
            );
        }
        InteractionEffects {
            consumed: true,
            ..Default::default()
        }
    }

    pub(super) fn pointer_move(
        &mut self,
        abs: DVec2,
        scene: &mut Scene,
        viewport: &mut ViewportController,
        selection: &mut SelectionState,
        placement: &mut PlacementInteraction,
    ) -> InteractionEffects {
        let was_moved = placement.snapshot().drag_moved;
        let mut effects = placement.drag_to(abs, scene, viewport);
        if effects.consumed {
            let snapshot = placement.snapshot();
            if !was_moved && snapshot.drag_moved {
                if let Some(key) = snapshot.dragged_key {
                    if selection.select(&key, &scene.nodes) {
                        effects.intent = Some(SurfaceIntent::NodeSelect { key });
                    }
                }
            }
            return effects;
        }
        effects.redraw = viewport.pan_to(abs);
        effects
    }

    pub(super) fn pointer_up(
        &mut self,
        abs: DVec2,
        primary: bool,
        scene: &mut Scene,
        viewport: &mut ViewportController,
        selection: &mut SelectionState,
        placement: &mut PlacementInteraction,
    ) -> InteractionEffects {
        let release = if primary {
            viewport
                .pan_down_abs()
                .map(|down| classify_release(down, abs, &scene.nodes, viewport.snapshot()))
                .unwrap_or(ReleaseIntent::NotClick)
        } else {
            ReleaseIntent::NotClick
        };
        viewport.end_pan();
        let mut effects = placement.finish_pointer_up(scene, viewport);
        if primary {
            match release {
                ReleaseIntent::NotClick => {}
                ReleaseIntent::Select { key } => {
                    selection.select(&key, &scene.nodes);
                    effects.intent = Some(SurfaceIntent::NodeSelect { key });
                    effects.redraw = true;
                }
                ReleaseIntent::Deselect => {
                    selection.clear();
                    effects.intent = Some(SurfaceIntent::NodeDeselect);
                    effects.redraw = true;
                }
                ReleaseIntent::ToggleExpand { key } => {
                    effects.intent = Some(SurfaceIntent::ToggleExpand { key });
                    effects.redraw = true;
                }
            }
        }
        effects.consumed = true;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::viewport::ViewportController;
    use crate::scene::SceneNode;
    use makepad_widgets::{dvec2, Rect};

    fn test_node(key: &str, rect: waml::solve::Rect) -> SceneNode {
        use waml::model::{ElementType, UmlMetaclass};
        SceneNode {
            key: key.to_string(),
            title: key.to_string(),
            element_type: ElementType::Uml(UmlMetaclass::Class),
            stereotypes: Vec::new(),
            attributes: Vec::new(),
            operations: Vec::new(),
            header: crate::scene::HeaderStyle::Plain,
            ports: false,
            rect,
            emphasized: false,
            collapsed: false,
            expanded: false,
        }
    }

    fn test_viewport() -> ViewportController {
        let mut viewport = ViewportController::default();
        viewport.set_view_rect(Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(800.0, 600.0),
        });
        viewport
    }

    #[test]
    fn release_inside_click_slop_selects_the_topmost_node() {
        let rect = waml::solve::Rect {
            x: 80.0,
            y: 80.0,
            w: 80.0,
            h: 60.0,
        };
        let nodes = vec![test_node("back", rect), test_node("front", rect)];
        let hit = classify_release(
            dvec2(100.0, 100.0),
            dvec2(103.0, 100.0),
            &nodes,
            test_viewport().snapshot(),
        );
        assert_eq!(
            hit,
            ReleaseIntent::Select {
                key: "front".into()
            }
        );
    }

    #[test]
    fn footer_release_toggles_without_selecting() {
        let mut node = test_node(
            "order",
            waml::solve::Rect {
                x: 80.0,
                y: 80.0,
                w: 200.0,
                h: 200.0,
            },
        );
        node.attributes = (0..7)
            .map(|index| crate::inspector::AttrRow {
                name: format!("field{index}"),
                ty: "Int".into(),
                multiplicity: String::new(),
                visibility: "+".into(),
            })
            .collect();
        let screen = Rect {
            pos: dvec2(80.0, 80.0),
            size: dvec2(200.0, 200.0),
        };
        let footer = footer_screen_rect(&node, screen, 1.0).unwrap();
        let hit = classify_release(
            footer.pos + footer.size * 0.5,
            footer.pos + footer.size * 0.5,
            &[node],
            test_viewport().snapshot(),
        );
        assert_eq!(
            hit,
            ReleaseIntent::ToggleExpand {
                key: "order".into()
            }
        );
    }

    #[test]
    fn movement_at_the_slop_boundary_is_not_a_click() {
        assert!(!is_click(dvec2(0.0, 0.0), dvec2(SELECT_SLOP, 0.0)));
    }

    #[test]
    fn node_at_hits_the_topmost_node_under_the_point() {
        let nodes = vec![
            test_node(
                "a",
                waml::solve::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 60.0,
                },
            ),
            test_node(
                "b",
                waml::solve::Rect {
                    x: 200.0,
                    y: 0.0,
                    w: 100.0,
                    h: 60.0,
                },
            ),
        ];
        let viewport = test_viewport().snapshot();
        assert_eq!(node_at(&nodes, viewport, dvec2(50.0, 30.0)), Some(0));
        assert_eq!(node_at(&nodes, viewport, dvec2(250.0, 30.0)), Some(1));
        assert_eq!(node_at(&nodes, viewport, dvec2(150.0, 30.0)), None);
    }

    #[test]
    fn is_click_splits_on_the_slop_threshold() {
        let down = dvec2(100.0, 100.0);
        assert!(is_click(down, dvec2(102.0, 101.0)));
        assert!(is_click(down, dvec2(103.9, 100.0)));
        assert!(!is_click(down, dvec2(110.0, 100.0)));
        assert!(!is_click(down, dvec2(104.0, 100.0)));
    }

    #[test]
    fn a_sub_slop_click_selects_the_node_under_the_point() {
        let nodes = vec![
            test_node(
                "uml.A",
                waml::solve::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 100.0,
                    h: 60.0,
                },
            ),
            test_node(
                "uml.B",
                waml::solve::Rect {
                    x: 200.0,
                    y: 0.0,
                    w: 100.0,
                    h: 60.0,
                },
            ),
        ];
        let down = dvec2(250.0, 30.0);
        assert_eq!(
            classify_release(down, dvec2(251.0, 31.0), &nodes, test_viewport().snapshot(),),
            ReleaseIntent::Select {
                key: "uml.B".into()
            }
        );
        assert_eq!(
            classify_release(down, dvec2(280.0, 30.0), &nodes, test_viewport().snapshot(),),
            ReleaseIntent::NotClick
        );
    }

    fn many_attr_node(key: &str, n: usize) -> SceneNode {
        let mut node = test_node(
            key,
            waml::solve::Rect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 200.0,
            },
        );
        node.attributes = (0..n)
            .map(|index| crate::inspector::AttrRow {
                name: format!("field{index}"),
                ty: "Int".into(),
                multiplicity: String::new(),
                visibility: "+".into(),
            })
            .collect();
        node
    }

    #[test]
    fn footer_rect_present_for_an_over_cap_node_and_absent_otherwise() {
        let screen = Rect {
            pos: dvec2(0.0, 0.0),
            size: dvec2(200.0, 200.0),
        };
        assert!(footer_screen_rect(&many_attr_node("big", 7), screen, 1.0).is_some());
        assert!(footer_screen_rect(&many_attr_node("small", 2), screen, 1.0).is_none());
    }

    #[test]
    fn a_point_in_the_footer_band_is_inside_the_footer_rect() {
        let screen = Rect {
            pos: dvec2(10.0, 20.0),
            size: dvec2(200.0, 200.0),
        };
        let node = many_attr_node("big", 7);
        let footer = footer_screen_rect(&node, screen, 1.0).unwrap();
        let midpoint = footer.pos + footer.size * 0.5;
        assert!(footer.contains(midpoint));
        assert!(!footer.contains(dvec2(midpoint.x, screen.pos.y + 1.0)));
    }

    #[test]
    fn event_priority_matches_the_widget_contract() {
        assert_eq!(
            EVENT_PRIORITY,
            [
                "camera_interval",
                "escape_cancel",
                "dwell_timeout",
                "preview_frame",
                "pinch",
                "secondary_down",
                "primary_down",
                "move",
                "primary_up",
                "other_up",
                "hover",
                "scroll",
            ]
        );
    }
}
