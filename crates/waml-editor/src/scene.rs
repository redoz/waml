//! The render seam: pick a diagram, solve it, and flatten to plain data.
//! Nothing below this module touches makepad; nothing here touches a GPU.

use waml::diagnostic::Diagnostic;
use waml::model::{Diagram, Model, RelationshipKind};
use waml::solve::{solve_diagram, Rect, SolveConfig, SolvedGroup};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub key: String,
    pub title: String,
    pub rect: Rect,
    pub emphasized: bool,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    pub source: Rect,
    pub target: Rect,
    pub kind: RelationshipKind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SceneEdge>,
}

/// Solve `diagram` against `model` and flatten the result into a `Scene`.
pub fn build_scene(model: &Model, diagram: &Diagram) -> (Scene, Vec<Diagnostic>) {
    use std::collections::BTreeMap;

    let sizes = crate::sizing::size_map(model, diagram);
    let (solved, diags) = solve_diagram(diagram, &sizes, &SolveConfig::default());

    let title_of: BTreeMap<&str, String> = model
        .nodes
        .iter()
        .map(|n| (n.key.as_str(), n.label.clone()))
        .collect();

    let mut nodes = Vec::with_capacity(solved.nodes.len());
    for (key, rect) in &solved.nodes {
        let flags = solved.flags.get(key).copied().unwrap_or_default();
        nodes.push(SceneNode {
            key: key.clone(),
            title: title_of
                .get(key.as_str())
                .cloned()
                .unwrap_or_else(|| key.clone()),
            rect: *rect,
            emphasized: flags.emphasized,
            collapsed: flags.collapsed,
        });
    }

    // Only edges whose endpoints both appear in the solved layout are drawable.
    let mut edges = Vec::new();
    for e in &model.edges {
        if let (Some(&source), Some(&target)) =
            (solved.nodes.get(&e.source), solved.nodes.get(&e.target))
        {
            edges.push(SceneEdge {
                source,
                target,
                kind: e.kind,
            });
        }
    }

    (
        Scene {
            nodes,
            groups: solved.groups.clone(),
            edges,
        },
        diags,
    )
}

/// Axis-aligned bounding box over all node and group rects, or `None` if empty.
pub fn bounding_box(scene: &Scene) -> Option<Rect> {
    let mut rects = scene
        .nodes
        .iter()
        .map(|n| n.rect)
        .chain(scene.groups.iter().map(|g| g.rect));
    let first = rects.next()?;
    let (mut min_x, mut min_y) = (first.x, first.y);
    let (mut max_x, mut max_y) = (first.x + first.w, first.y + first.h);
    for r in rects {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.w);
        max_y = max_y.max(r.y + r.h);
    }
    Some(Rect {
        x: min_x,
        y: min_y,
        w: max_x - min_x,
        h: max_y - min_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    #[test]
    fn scene_has_both_nodes_with_titles() {
        let model = mini();
        let (scene, diags) = build_scene(&model, &model.diagrams[0]);
        assert!(diags.is_empty(), "unexpected diagnostics: {diags:?}");

        let mut titles: Vec<(&str, &str)> = scene
            .nodes
            .iter()
            .map(|n| (n.key.as_str(), n.title.as_str()))
            .collect();
        titles.sort();
        assert_eq!(titles, [("customer", "Customer"), ("order", "Order")]);
    }

    #[test]
    fn scene_edge_endpoints_match_node_rects() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        assert_eq!(scene.edges.len(), 1);
        let edge = &scene.edges[0];
        assert_eq!(edge.kind, RelationshipKind::Associates);

        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // The associates edge runs order -> customer (see fixture order.md).
        assert_eq!(edge.source, order.rect);
        assert_eq!(edge.target, customer.rect);
    }

    #[test]
    fn layout_places_order_left_of_customer() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // "- [Order] left of [Customer]" => order's right edge is left of customer's left edge.
        assert!(order.rect.x + order.rect.w <= customer.rect.x);
    }

    #[test]
    fn bounding_box_covers_all_nodes() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        let bbox = bounding_box(&scene).unwrap();
        for node in &scene.nodes {
            assert!(node.rect.x >= bbox.x);
            assert!(node.rect.y >= bbox.y);
            assert!(node.rect.x + node.rect.w <= bbox.x + bbox.w + 1e-6);
            assert!(node.rect.y + node.rect.h <= bbox.y + bbox.h + 1e-6);
        }
        assert!(bbox.w > 0.0 && bbox.h > 0.0);
    }

    #[test]
    fn bounding_box_none_for_empty_scene() {
        let scene = Scene {
            nodes: vec![],
            groups: vec![],
            edges: vec![],
        };
        assert!(bounding_box(&scene).is_none());
    }
}
