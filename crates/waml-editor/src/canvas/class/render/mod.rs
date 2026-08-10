mod edges;
mod groups;
mod labels;
mod metrics;
mod nodes;
mod overlays;
mod primitives;
mod relations;
pub mod use_case_groups;
pub mod use_case_nodes;

use super::{placement::PlacementSnapshot, selection::SelectionSnapshot};
use crate::{canvas::viewport::ViewportSnapshot, scene::Scene};
use makepad_widgets::Cx2d;

pub(super) use metrics::LineworkMetrics;
pub(super) use primitives::ClassDrawResources;

/// Cached per-node card measurements. `card::measure` runs a full taffy tree
/// build + layout, so re-measuring every node every frame dominated draw cost
/// (P-2). Entries derive from node *content* only -- zoom and position are
/// applied at draw time -- so the owning surface clears the cache whenever it
/// installs a new scene (`reconcile_scene`), the only path that changes node
/// content. Placement previews only move rects and need no invalidation.
#[derive(Default)]
pub(super) struct CardMeasureCache {
    map: std::collections::HashMap<String, crate::card::Placed>,
}

impl CardMeasureCache {
    pub(super) fn clear(&mut self) {
        self.map.clear();
    }

    /// The measured card for `node`, computed once per key per scene.
    pub(super) fn placed(&mut self, node: &crate::scene::SceneNode) -> &crate::card::Placed {
        use crate::card;
        self.map
            .entry(node.key.clone())
            .or_insert_with(|| card::measure(&card::class_shape(node, &card::mono_sheet())))
    }
}

pub(super) struct RenderSnapshot<'a> {
    pub(super) scene: &'a Scene,
    pub(super) linework: LineworkMetrics,
    pub(super) viewport: ViewportSnapshot,
    pub(super) selection: SelectionSnapshot,
    pub(super) placement: PlacementSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RenderPass {
    Background,
    Groups,
    Edges,
    EdgeLabels,
    Nodes,
    Relations,
    ConflictFocus,
    Placement,
}

pub(super) const PASS_ORDER: [RenderPass; 8] = [
    RenderPass::Background,
    RenderPass::Groups,
    RenderPass::Edges,
    RenderPass::EdgeLabels,
    RenderPass::Nodes,
    RenderPass::Relations,
    RenderPass::ConflictFocus,
    RenderPass::Placement,
];

pub(super) fn draw(
    cx: &mut Cx2d,
    snapshot: &RenderSnapshot<'_>,
    draws: &mut ClassDrawResources<'_>,
    cards: &mut CardMeasureCache,
) {
    for pass in PASS_ORDER {
        match pass {
            RenderPass::Background => draws.bg.draw_abs(cx, snapshot.viewport.view_rect),
            RenderPass::Groups => groups::draw_groups(cx, snapshot, draws),
            RenderPass::Edges => edges::draw_edges(cx, snapshot, draws),
            RenderPass::EdgeLabels => labels::draw_edge_labels(cx, snapshot, draws),
            RenderPass::Nodes => nodes::draw_nodes(cx, snapshot, draws, cards),
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
                RenderPass::EdgeLabels,
                RenderPass::Nodes,
                RenderPass::Relations,
                RenderPass::ConflictFocus,
                RenderPass::Placement,
            ],
        );
    }
}
