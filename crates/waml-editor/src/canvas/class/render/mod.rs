mod edges;
mod groups;
mod nodes;
mod overlays;
mod primitives;
mod relations;

use super::{placement::PlacementSnapshot, selection::SelectionSnapshot};
use crate::{canvas::viewport::ViewportSnapshot, scene::Scene};
use makepad_widgets::Cx2d;

pub(super) use primitives::ClassDrawResources;

pub(super) struct RenderSnapshot<'a> {
    pub(super) scene: &'a Scene,
    pub(super) viewport: ViewportSnapshot,
    pub(super) selection: SelectionSnapshot,
    pub(super) placement: PlacementSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderPass {
    Background,
    Groups,
    Edges,
    Nodes,
    Relations,
    ConflictFocus,
    Placement,
}

pub(super) const PASS_ORDER: [RenderPass; 7] = [
    RenderPass::Background,
    RenderPass::Groups,
    RenderPass::Edges,
    RenderPass::Nodes,
    RenderPass::Relations,
    RenderPass::ConflictFocus,
    RenderPass::Placement,
];

pub(super) fn draw(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
) {
    for pass in PASS_ORDER {
        match pass {
            RenderPass::Background => draws.bg.draw_abs(cx, snapshot.viewport.view_rect),
            RenderPass::Groups => groups::draw_groups(cx, snapshot, draws),
            RenderPass::Edges => edges::draw_edges(cx, snapshot, draws),
            RenderPass::Nodes => nodes::draw_nodes(cx, snapshot, draws),
            RenderPass::Relations => relations::draw_relations(cx, snapshot, draws),
            RenderPass::ConflictFocus => overlays::draw_conflict_focus(cx, snapshot, draws),
            RenderPass::Placement => overlays::draw_placement(cx, snapshot, draws),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_render_order_is_behaviorally_stable() {
        assert_eq!(
            PASS_ORDER,
            [
                RenderPass::Background,
                RenderPass::Groups,
                RenderPass::Edges,
                RenderPass::Nodes,
                RenderPass::Relations,
                RenderPass::ConflictFocus,
                RenderPass::Placement,
            ],
        );
    }
}
