//! The render seam: pick a diagram, solve it, and flatten to plain data.
//! Nothing below this module touches makepad; nothing here touches a GPU.

use waml::diagnostic::Diagnostic;
use waml::model::{Diagram, ElementType, Model, RelationshipKind};
use waml::solve::{solve_diagram, Rect, SolveConfig, SolvedGroup};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneNode {
    pub key: String,
    pub title: String,
    /// The node's model element type (`uml.Class`, `uml.Interface`, ...), used
    /// by `canvas.rs`'s renderer (via `node_style`) to pick an accent color
    /// and optional stereotype guillemet label (U9 mock).
    pub element_type: ElementType,
    /// User-declared stereotypes (e.g. `aggregateRoot`), rendered as the card's
    /// «guillemet» eyebrow above the title. Distinct from the metaclass-derived
    /// `node_style::stereotype_label` (which handles «interface» etc.); this is
    /// the node's own `stereotype:` front-matter list.
    pub stereotypes: Vec<String>,
    /// Attribute compartment rows (visibility marker + name + type token),
    /// projected via `inspector::build_view` so the canvas renderer and the
    /// inspector panel share one member projection. Empty for nodes with no
    /// attributes; only drawn by the focus card today.
    pub attributes: Vec<crate::inspector::AttrRow>,
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

// An empty scene (derived Default) is the sensible startup default (fed a real one via set_scene).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SceneEdge>,
}

/// Project classifier `key`'s attribute compartment rows via the shared
/// `inspector::build_view` seam, so the canvas card and the inspector panel
/// never re-derive UML member extraction. A non-classifier or missing key
/// yields no rows.
fn attribute_rows(model: &Model, key: &str) -> Vec<crate::inspector::AttrRow> {
    use crate::inspector::{build_view, Subject};
    build_view(model, &Subject::Classifier(key.to_string()))
        .map(|v| v.attributes)
        .unwrap_or_default()
}

/// The card's «stereotype» eyebrow label (raw, no guillemets): the node's own
/// declared stereotypes if any, else the metaclass-derived label. Shared by the
/// focus-card sizer (`build_focus_scene`) and its renderer (`draw_focus_card`)
/// so both measure and draw the same line.
pub fn focus_eyebrow(stereotypes: &[String], ty: &ElementType) -> Option<String> {
    if !stereotypes.is_empty() {
        Some(stereotypes.join(", "))
    } else {
        crate::node_style::stereotype_label(ty).map(str::to_string)
    }
}

/// Solve `diagram` against `model` and flatten the result into a `Scene`.
pub fn build_scene(model: &Model, diagram: &Diagram) -> (Scene, Vec<Diagnostic>) {
    use std::collections::BTreeMap;

    let sizes = crate::sizing::size_map(model, diagram);
    let (solved, diags) = solve_diagram(diagram, &sizes, &SolveConfig::default());

    let node_of: BTreeMap<&str, &waml::model::Node> =
        model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut nodes = Vec::with_capacity(solved.nodes.len());
    for (key, rect) in &solved.nodes {
        let flags = solved.flags.get(key).copied().unwrap_or_default();
        let model_node = node_of.get(key.as_str()).copied();
        let title = model_node
            .and_then(|n| n.concept.title.clone())
            .unwrap_or_else(|| key.clone());
        let element_type = model_node
            .map(|n| n.ty.clone())
            .unwrap_or_else(|| ElementType::Unknown(String::new()));
        let attributes = attribute_rows(model, key);
        let stereotypes = model_node
            .map(|n| n.stereotypes.clone())
            .unwrap_or_default();
        nodes.push(SceneNode {
            key: key.clone(),
            title,
            element_type,
            stereotypes,
            attributes,
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

/// Build a single-node `Scene` focused on classifier `key`, sized 1.5x its
/// natural box. Used by the classifier focus view (double/single-click a class
/// in the tree). An unknown key yields an empty scene.
pub fn build_focus_scene(model: &Model, key: &str) -> Scene {
    let Some(node) = model.nodes.iter().find(|n| n.key == key) else {
        return Scene {
            nodes: vec![],
            groups: vec![],
            edges: vec![],
        };
    };
    let title = node
        .concept
        .title
        .clone()
        .unwrap_or_else(|| node.key.clone());
    let attributes = attribute_rows(model, key);
    // The focus card is drawn at zoom 1.0 (world px == screen px). Build the
    // scene node, then size its rect to the exact hull the card box-tree hugs.
    let mut scene_node = SceneNode {
        key: key.to_string(),
        title,
        element_type: node.ty.clone(),
        stereotypes: node.stereotypes.clone(),
        attributes,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        },
        emphasized: true,
        collapsed: false,
    };
    let (w, h) = crate::card::card_size(&scene_node, &crate::card::mono_sheet());
    scene_node.rect = Rect {
        x: 0.0,
        y: 0.0,
        w,
        h,
    };
    Scene {
        nodes: vec![scene_node],
        groups: vec![],
        edges: vec![],
    }
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
        assert_eq!(
            titles,
            [
                ("customer", "Customer"),
                ("order", "Order"),
                ("payment-gateway", "PaymentGateway"),
            ]
        );
    }

    #[test]
    fn focus_scene_node_carries_attribute_rows() {
        let model = mini();
        let key = model
            .nodes
            .iter()
            .find(|n| n.concept.title.as_deref() == Some("Order"))
            .unwrap()
            .key
            .clone();
        let scene = build_focus_scene(&model, &key);
        let node = &scene.nodes[0];
        // Mirrors order.md's `## Attributes` block, in order.
        assert_eq!(node.attributes.len(), 2);
        assert_eq!(node.attributes[0].name, "id");
        assert_eq!(node.attributes[0].ty, "OrderId");
        assert_eq!(node.attributes[1].name, "total");
        assert_eq!(node.attributes[1].ty, "Decimal");
    }

    #[test]
    fn focus_scene_node_carries_declared_stereotypes() {
        let model = mini();
        let key = model
            .nodes
            .iter()
            .find(|n| n.concept.title.as_deref() == Some("Order"))
            .unwrap()
            .key
            .clone();
        let scene = build_focus_scene(&model, &key);
        // order.md declares `stereotype: [aggregateRoot]`.
        assert_eq!(
            scene.nodes[0].stereotypes,
            vec!["aggregateRoot".to_string()]
        );
    }

    #[test]
    fn build_scene_nodes_carry_attribute_rows() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        assert_eq!(order.attributes.len(), 2);
        assert_eq!(order.attributes[0].name, "id");
    }

    #[test]
    fn scene_nodes_carry_their_model_element_type() {
        let model = mini();
        let (scene, _) = build_scene(&model, &model.diagrams[0]);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let gateway = scene
            .nodes
            .iter()
            .find(|n| n.key == "payment-gateway")
            .unwrap();
        assert_eq!(
            order.element_type,
            ElementType::Uml(waml::model::UmlMetaclass::Class)
        );
        assert_eq!(
            gateway.element_type,
            ElementType::Uml(waml::model::UmlMetaclass::Interface)
        );
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
