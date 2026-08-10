use crate::{
    canvas::{
        class::placement::PlacementSnapshot, primitives::world_rect_to_screen,
        viewport::ViewportSnapshot,
    },
    scene::Scene,
};
use makepad_widgets::*;

pub(in crate::canvas::class) struct ClassDrawResources<'a> {
    pub(in crate::canvas::class) bg: &'a mut DrawColor,
    pub(in crate::canvas::class) node: &'a mut DrawColor,
    pub(in crate::canvas::class) use_case_ellipse: &'a mut DrawColor,
    pub(in crate::canvas::class) actor_line: &'a mut DrawColor,
    pub(in crate::canvas::class) group: &'a mut DrawColor,
    pub(in crate::canvas::class) group_border: &'a mut DrawColor,
    pub(in crate::canvas::class) group_dashed: &'a mut DrawColor,
    pub(in crate::canvas::class) group_title_dim: &'a mut DrawColor,
    pub(in crate::canvas::class) edge: &'a mut DrawColor,
    pub(in crate::canvas::class) edge_dashed: &'a mut DrawColor,
    pub(in crate::canvas::class) elbow: &'a mut DrawColor,
    pub(in crate::canvas::class) marker: &'a mut DrawColor,
    pub(in crate::canvas::class) edge_label_bg: &'a mut DrawColor,
    pub(in crate::canvas::class) rule: &'a mut DrawColor,
    pub(in crate::canvas::class) veil: &'a mut DrawColor,
    pub(in crate::canvas::class) text: &'a mut DrawText,
    pub(in crate::canvas::class) edge_label: &'a mut DrawText,
    pub(in crate::canvas::class) mono_dim: &'a mut DrawText,
    pub(in crate::canvas::class) mono_bold: &'a mut DrawText,
    pub(in crate::canvas::class) mono_accent: &'a mut DrawText,
    pub(in crate::canvas::class) mono_amber: &'a mut DrawText,
}

pub(super) fn node_screen_rect(
    scene: &Scene,
    viewport: ViewportSnapshot,
    placement: &PlacementSnapshot,
    index: usize,
) -> Rect {
    let node = &scene.nodes[index];
    let rect = if placement.dragged_key.as_deref() == Some(node.key.as_str()) {
        placement.ghost.unwrap_or(node.rect)
    } else {
        node.rect
    };
    world_rect_to_screen(viewport, rect)
}
