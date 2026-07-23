//! The render seam: pick a diagram, solve it, and flatten to plain data.
//! Nothing below this module touches makepad; nothing here touches a GPU.

use waml::diagnostic::Diagnostic;
use waml::model::{Diagram, ElementType, Model, RelEnd, RelationshipKind};
use waml::solve::{
    route, solve_diagram, stress, BoxId, Rect, Size, SizeMap, SolveConfig, Solved, SolvedGroup,
};

/// How a node's header (eyebrow + title) is treated. Additive: `Plain` is the
/// historical look (no wash) and is what every projected node uses, so real
/// canvas nodes render unchanged. Only the node design editor sets `Hidden`/
/// `Fill`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeaderStyle {
    /// No header block at all.
    Hidden,
    /// Header with no background treatment (today's look).
    #[default]
    Plain,
    /// Header band washed with the accent color.
    Fill,
}

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
    /// Operation compartment rows (`<vis> <name>(<params>) : <ret>`). The model
    /// has no operations concept, so projection leaves this empty; only the node
    /// design editor populates it. Additive: empty renders no operations block.
    pub operations: Vec<crate::inspector::OpRow>,
    /// Header treatment. Defaults to `Plain` (today's look) everywhere the model
    /// projects a node; the design editor overrides it.
    pub header: HeaderStyle,
    /// Whether to draw port nubs straddling the card border. Off for projected
    /// nodes; the design editor toggles it.
    pub ports: bool,
    pub rect: Rect,
    pub emphasized: bool,
    pub collapsed: bool,
    /// Ephemeral view-state: whether the card shows all members (true) or is
    /// capped at `card::MAX_BODY_ROWS` with a `▾ N more` footer (false). Set from
    /// `App`'s expanded key-set in `build_scene`; never derived from the model.
    /// Defaults `false` (collapsed) everywhere the model projects a node.
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    pub source: Rect,
    pub target: Rect,
    pub kind: RelationshipKind,
    /// Relationship ends (multiplicity, role, navigability), carried verbatim
    /// from `model::Edge`. The router is geometry-only; end adornments
    /// (arrowheads, crowsfoot, multiplicity labels) are chosen downstream from
    /// these + a notation policy, so they must ride along the drawable edge.
    pub from_end: RelEnd,
    pub to_end: RelEnd,
    /// Routed orthogonal polyline in world coordinates; the renderer strokes it
    /// segment-by-segment. Always non-empty (router emits ≥2 points; a defensive
    /// straight [source-center, target-center] fallback is used on route
    /// mismatch).
    pub points: Vec<(f64, f64)>,
}

/// A placement relation projected from the diagram's `## Layout` for drag-time
/// overlay + conflict prediction: a 2-operand single-direction placement, its
/// operands resolved to `SceneNode.key` slugs. Multi-operand / alignment
/// statements are not projected (the drag overlay + one-relation-per-pair
/// oracle only reason about 2-node placements).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneRelation {
    pub subject: String,
    pub reference: String,
    pub dir: waml::syntax::Direction,
}

// An empty scene (derived Default) is the sensible startup default (fed a real one via set_scene).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub edges: Vec<SceneEdge>,
    pub relations: Vec<SceneRelation>,
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

/// Project model `node` into a `SceneNode` with a zeroed rect. The rect is
/// filled later — from the solver in `build_scene`, or measured to the card
/// hull in `sizing`. One place derives title / element_type / stereotypes /
/// attributes so measurement and drawing never diverge. `emphasized` and
/// `collapsed` default to `false`; callers set them from solved flags.
pub fn project_scene_node(model: &Model, node: &waml::model::Node) -> SceneNode {
    SceneNode {
        key: node.key.clone(),
        title: node
            .concept
            .title
            .clone()
            .unwrap_or_else(|| node.key.clone()),
        element_type: node.ty.clone(),
        stereotypes: node.stereotypes.clone(),
        attributes: attribute_rows(model, &node.key),
        operations: Vec::new(),
        header: HeaderStyle::Plain,
        ports: false,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        },
        emphasized: false,
        collapsed: false,
        expanded: false,
    }
}

/// A diagram with no authored layout statements and only trivial (unnamed,
/// childless) member groups gets the semi-smart stress-majorization default
/// instead of the constraint solver's edge-blind left-to-right strip. Authored
/// named/nested groups still route to `solve_diagram` — structure wins.
fn use_stress_default(diagram: &Diagram) -> bool {
    diagram.layout.is_empty()
}

/// The model's drawable edges, in `model.edges` order, with self-loops dropped
/// (`source != target`, Node endpoints only). This is the single load-bearing
/// definition tying the router's ordered `Solved.routes` stream to the scene:
/// both layout paths feed `route::route` the pairs derived from here, so it
/// emits one `Route` per surviving edge IN THIS ORDER, and `build_scene`
/// consumes that stream by walking this same list. Every site that touches the
/// drawable-edge order MUST route through this helper or the route-to-edge match
/// silently desyncs and degrades every subsequent edge to the straight fallback.
fn drawable_edges(model: &Model) -> Vec<&waml::model::Edge> {
    model
        .edges
        .iter()
        .filter(|e| e.source != e.target)
        .collect()
}

/// The slug a placement operand refers to (`[Title](./slug.md)` or a bare
/// name). `None` for inline-group / paren operands, which the relation
/// projection skips.
fn operand_slug(op: &waml::syntax::Operand) -> Option<&str> {
    use waml::syntax::{NameRef, OperandRef};
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(slug.as_str()),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// Project the diagram's `## Layout` into 2-operand single-direction relation
/// triples (subject_slug, reference_slug, dir). Mirrors `ops::placement_matches`'
/// shape: only 2-operand, 1-direction placements qualify.
fn project_relations(diagram: &Diagram) -> Vec<SceneRelation> {
    use waml::syntax::LayoutStatement;
    let mut out = Vec::new();
    for stmt in &diagram.layout {
        if let LayoutStatement::Placement {
            operands,
            directions,
        } = stmt
        {
            if operands.len() == 2 && directions.len() == 1 {
                if let (Some(subject), Some(reference)) =
                    (operand_slug(&operands[0]), operand_slug(&operands[1]))
                {
                    out.push(SceneRelation {
                        subject: subject.to_string(),
                        reference: reference.to_string(),
                        dir: directions[0],
                    });
                }
            }
        }
    }
    out
}

/// Native-only stress/grid default layout. Kept at this call seam (not inside
/// `solve_diagram`) so the wasm/web path stays unchanged — web keeps dagre.
/// Node set is every sized member; undirected `model.edges` among them drive the
/// stress solve, and an edgeless set falls back to `grid_pack`.
fn stress_default(model: &Model, sizes: &SizeMap) -> Solved {
    use std::collections::{BTreeMap, BTreeSet};

    let keys: Vec<String> = sizes.keys().cloned().collect();
    let ids: Vec<BoxId> = keys.iter().cloned().map(BoxId::Node).collect();
    let dims: Vec<Size> = keys.iter().map(|k| sizes[k]).collect();
    let index: BTreeMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    // Undirected edge index pairs among members; drop self-loops and duplicates.
    let mut seen = BTreeSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for e in &model.edges {
        let (Some(&a), Some(&b)) = (index.get(e.source.as_str()), index.get(e.target.as_str()))
        else {
            continue;
        };
        if a == b {
            continue;
        }
        if seen.insert((a.min(b), a.max(b))) {
            pairs.push((a, b));
        }
    }

    let cfg = stress::StressConfig::default();
    let rects = if pairs.is_empty() {
        stress::grid_pack(&ids, &dims, &cfg)
    } else {
        stress::layout(&ids, &dims, &pairs, &cfg)
    };

    // Rects keyed by BoxId for the router (obstacles derive from these rects).
    let rect_map: BTreeMap<BoxId, Rect> = ids.iter().cloned().zip(rects.iter().copied()).collect();

    // Directed (BoxId, BoxId) edge list from the shared `drawable_edges` filter,
    // so routes come out in the exact order build_scene consumes them.
    // route::route presence-filters internally.
    let route_edges: Vec<(BoxId, BoxId)> = drawable_edges(model)
        .into_iter()
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();

    // Empty boxes slice: the stress layout is group-less, so build_membership(&[])
    // yields no groups and routing degrades to pure leaf-obstacle avoidance.
    let routes = route::route(&[], &rect_map, &route_edges, &SolveConfig::default());

    Solved {
        nodes: keys.into_iter().zip(rects).collect(),
        groups: Vec::new(),
        flags: BTreeMap::new(),
        routes,
    }
}

/// Straight-line fallback route between two node centers, emitted as an
/// axis-aligned L (horizontal leg to the target's column, then vertical leg).
/// Used when the ordered route stream desyncs from the drawable-edge list.
///
/// The canvas strokes every segment with a single top-left->bottom-right
/// diagonal pen (`draw_edge_down`, flip = 0), which only renders axis-aligned
/// segments correctly; a raw diagonal `[source-center, target-center]` on a
/// negative-slope pair would stroke the AABB's *other* diagonal and render
/// mirrored. Keeping the fallback axis-aligned preserves the single-pen
/// invariant. The degenerate elbow (when the centers share a column or row) is
/// dropped so a straight vertical/horizontal fallback stays two points.
fn fallback_route(source: Rect, target: Rect) -> Vec<(f64, f64)> {
    let sc = (source.x + source.w / 2.0, source.y + source.h / 2.0);
    let tc = (target.x + target.w / 2.0, target.y + target.h / 2.0);
    let elbow = (tc.0, sc.1);
    // Drop the elbow when it coincides with an endpoint (centers aligned on an
    // axis), avoiding a zero-length leading/trailing segment.
    let dup_source = (elbow.0 - sc.0).abs() < f64::EPSILON;
    let dup_target = (elbow.1 - tc.1).abs() < f64::EPSILON;
    if dup_source || dup_target {
        vec![sc, tc]
    } else {
        vec![sc, elbow, tc]
    }
}

/// Solve `diagram` against `model` and flatten the result into a `Scene`.
pub fn build_scene(
    model: &Model,
    diagram: &Diagram,
    expanded: &std::collections::HashSet<String>,
) -> (Scene, Vec<Diagnostic>) {
    use std::collections::BTreeMap;

    let sizes = crate::sizing::size_map(model, diagram, expanded);
    let edges: Vec<(BoxId, BoxId)> = drawable_edges(model)
        .into_iter()
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();
    let (solved, diags) = if use_stress_default(diagram) {
        (stress_default(model, &sizes), Vec::new())
    } else {
        solve_diagram(diagram, &edges, &sizes, &SolveConfig::default())
    };

    let node_of: BTreeMap<&str, &waml::model::Node> =
        model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let mut nodes = Vec::with_capacity(solved.nodes.len());
    for (key, rect) in &solved.nodes {
        let flags = solved.flags.get(key).copied().unwrap_or_default();
        let mut node = match node_of.get(key.as_str()).copied() {
            Some(model_node) => project_scene_node(model, model_node),
            // Keys with no resolving model node (synthetic/unknown) fall back to
            // a title-only node: key as title, Unknown type, no members.
            None => SceneNode {
                key: key.clone(),
                title: key.clone(),
                element_type: ElementType::Unknown(String::new()),
                stereotypes: Vec::new(),
                attributes: Vec::new(),
                operations: Vec::new(),
                header: HeaderStyle::Plain,
                ports: false,
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                },
                emphasized: false,
                collapsed: false,
                expanded: false,
            },
        };
        node.rect = *rect;
        node.emphasized = flags.emphasized;
        node.collapsed = flags.collapsed;
        node.expanded = expanded.contains(key);
        nodes.push(node);
    }

    // Walk the same `drawable_edges` list route::route was fed, so the ordered
    // route stream and this consumption stay locked together by construction.
    // Only edges whose endpoints both appear in the solved layout are drawable;
    // match each to its Route by consuming solved.routes IN ORDER (key-only
    // lookup is ambiguous for parallel edges). On a key mismatch (e.g. an edge
    // route::route presence-filtered out, desyncing the stream) fall back to a
    // straight center-to-center polyline WITHOUT advancing the cursor, so later
    // edges stay aligned.
    let mut edges = Vec::new();
    let mut route_cursor = 0usize;
    for e in drawable_edges(model) {
        if let (Some(&source), Some(&target)) =
            (solved.nodes.get(&e.source), solved.nodes.get(&e.target))
        {
            let points = match solved.routes.get(route_cursor) {
                Some(r) if r.source == e.source && r.target == e.target => {
                    route_cursor += 1;
                    r.points.clone()
                }
                _ => fallback_route(source, target),
            };
            edges.push(SceneEdge {
                source,
                target,
                kind: e.kind,
                from_end: e.from_end.clone(),
                to_end: e.to_end.clone(),
                points,
            });
        }
    }

    (
        Scene {
            nodes,
            groups: solved.groups.clone(),
            edges,
            relations: project_relations(diagram),
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
            relations: Vec::new(),
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
        operations: Vec::new(),
        header: HeaderStyle::Plain,
        ports: false,
        rect: Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        },
        emphasized: true,
        collapsed: false,
        expanded: false,
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
        relations: Vec::new(),
    }
}

/// The classifier title for a slug (for a `[Title](./slug.md)` operand), or the
/// slug itself when unknown.
fn title_for(model: &Model, slug: &str) -> String {
    model
        .nodes
        .iter()
        .find(|n| n.key == slug)
        .and_then(|n| n.concept.title.clone())
        .unwrap_or_else(|| slug.to_string())
}

/// True iff a placement matches the given ordered `(subject, reference)` pair
/// as a 2-operand single-direction relation (mirrors `ops::placement_matches`).
fn placement_is_pair(stmt: &waml::syntax::LayoutStatement, subject: &str, reference: &str) -> bool {
    use waml::syntax::LayoutStatement;
    if let LayoutStatement::Placement {
        operands,
        directions,
    } = stmt
    {
        operands.len() == 2
            && directions.len() == 1
            && operand_slug(&operands[0]) == Some(subject)
            && operand_slug(&operands[1]) == Some(reference)
    } else {
        false
    }
}

/// Speculatively author `subject <dir> reference` into a scratch clone of the
/// diagram (one-relation-per-pair replace: drop any existing placement for this
/// ordered pair, then push the hypothetical one), re-solve, and report whether
/// the solver emits a `LayoutConflict`. The solver is the ground truth — it
/// catches transitive / cycle contradictions a hand-rolled rule would miss.
pub fn placement_would_conflict(
    model: &Model,
    diagram: &Diagram,
    subject_slug: &str,
    reference_slug: &str,
    dir: waml::syntax::Direction,
    expanded: &std::collections::HashSet<String>,
) -> bool {
    use waml::diagnostic::DiagCode;
    use waml::syntax::{LayoutStatement, NameRef, Operand, OperandRef};

    let link = |slug: &str| Operand {
        ref_: OperandRef::Name(NameRef::Link {
            title: title_for(model, slug),
            slug: slug.to_string(),
        }),
        axis: None,
        hints: Vec::new(),
    };

    let mut scratch = diagram.clone();
    scratch
        .layout
        .retain(|s| !placement_is_pair(s, subject_slug, reference_slug));
    scratch.layout.push(LayoutStatement::Placement {
        operands: vec![link(subject_slug), link(reference_slug)],
        directions: vec![dir],
    });

    let (_scene, diags) = build_scene(model, &scratch, expanded);
    diags.iter().any(|d| d.code == DiagCode::LayoutConflict)
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
    fn scene_projects_existing_placement_relations() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        use waml::syntax::Direction;
        // orders-diagram.md's ## Layout: `Order left of Customer` +
        // `PaymentGateway below Order`.
        let has = |subj: &str, refr: &str, dir: Direction| {
            scene
                .relations
                .iter()
                .any(|r| r.subject == subj && r.reference == refr && r.dir == dir)
        };
        assert!(
            has("order", "customer", Direction::LeftOf),
            "missing order left-of customer: {:?}",
            scene.relations
        );
        assert!(
            has("payment-gateway", "order", Direction::Below),
            "missing payment-gateway below order: {:?}",
            scene.relations
        );
    }

    #[test]
    fn oracle_flags_a_contradictory_placement() {
        // mini has `Order left of Customer`. Authoring the REVERSED ordered pair
        // `Customer left of Order` is a different pair (so the existing relation is
        // NOT replaced) — both coexist, the solver cannot satisfy them, and emits a
        // LayoutConflict.
        let model = mini();
        let diagram = &model.diagrams[0];
        assert!(
            placement_would_conflict(
                &model,
                diagram,
                "customer",
                "order",
                waml::syntax::Direction::LeftOf,
                &std::collections::HashSet::new(),
            ),
            "reversed cardinal on an existing pair must be predicted conflicting"
        );
    }

    #[test]
    fn oracle_accepts_a_clean_diagonal_placement() {
        // One-relation-per-pair replace: authoring a diagonal for the ORDER ->
        // CUSTOMER pair drops mini's existing `order left of customer` (same
        // ordered pair) before re-solving, so the diagonal is the only relation
        // on that pair -- and `payment-gateway below order` never touches
        // customer, so nothing else contradicts it. Satisfiable, no
        // LayoutConflict.
        let model = mini();
        let diagram = &model.diagrams[0];
        assert!(
            !placement_would_conflict(
                &model,
                diagram,
                "order",
                "customer",
                waml::syntax::Direction::AboveLeft,
                &std::collections::HashSet::new(),
            ),
            "a non-contradictory diagonal must NOT be predicted conflicting"
        );
    }

    #[test]
    fn scene_has_both_nodes_with_titles() {
        let model = mini();
        let (scene, diags) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
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
    fn project_scene_node_carries_concept_and_members() {
        let model = mini();
        let node = model.nodes.iter().find(|n| n.key == "order").unwrap();
        let projected = project_scene_node(&model, node);

        assert_eq!(projected.title, "Order");
        assert_eq!(
            projected.element_type,
            ElementType::Uml(waml::model::UmlMetaclass::Class)
        );
        // order.md declares `stereotype: [aggregateRoot]`.
        assert_eq!(projected.stereotypes, vec!["aggregateRoot".to_string()]);
        // Mirrors order.md's `## Attributes` block, in order.
        assert_eq!(projected.attributes.len(), 2);
        assert_eq!(projected.attributes[0].name, "id");
        assert_eq!(projected.attributes[0].ty, "OrderId");
        assert_eq!(projected.attributes[1].name, "total");
        assert_eq!(projected.attributes[1].ty, "Decimal");
    }

    #[test]
    fn build_scene_nodes_carry_attribute_rows() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        assert_eq!(order.attributes.len(), 2);
        assert_eq!(order.attributes[0].name, "id");
    }

    #[test]
    fn scene_nodes_carry_their_model_element_type() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
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
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        assert_eq!(scene.edges.len(), 1);
        let edge = &scene.edges[0];
        assert_eq!(edge.kind, RelationshipKind::Associates);
        assert!(!edge.points.is_empty(), "routed edge must carry a polyline");

        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // The associates edge runs order -> customer (see fixture order.md).
        assert_eq!(edge.source, order.rect);
        assert_eq!(edge.target, customer.rect);
    }

    #[test]
    fn layout_places_order_left_of_customer() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // "- [Order] left of [Customer]" => order's right edge is left of customer's left edge.
        assert!(order.rect.x + order.rect.w <= customer.rect.x);
    }

    #[test]
    fn bounding_box_covers_all_nodes() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
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
            relations: vec![],
        };
        assert!(bounding_box(&scene).is_none());
    }

    #[test]
    fn projected_node_defaults_to_not_expanded() {
        let model = mini();
        let node = model.nodes.iter().find(|n| n.key == "order").unwrap();
        let projected = project_scene_node(&model, node);
        assert!(!projected.expanded);
    }

    #[test]
    fn build_scene_mirrors_the_expanded_flag_onto_its_node() {
        let model = mini();
        let mut expanded = std::collections::HashSet::new();
        expanded.insert("order".to_string());
        let (scene, _) = build_scene(&model, &model.diagrams[0], &expanded);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        assert!(order.expanded, "order was in the expanded set");
        assert!(!customer.expanded, "customer was not");
    }

    #[test]
    fn stress_default_populates_routes() {
        let model = mini();
        // stress_default is layout-agnostic (it reads model + sizes, not the
        // diagram's layout block), so any sized diagram exercises it.
        let sizes = crate::sizing::size_map(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        let solved = stress_default(&model, &sizes);
        // mini declares one associates edge order -> customer.
        assert_eq!(solved.routes.len(), 1);
        assert!(!solved.routes[0].points.is_empty());
        let r = &solved.routes[0];
        assert!(
            (r.source == "order" && r.target == "customer")
                || (r.source == "customer" && r.target == "order"),
            "unexpected route endpoints: {} -> {}",
            r.source,
            r.target
        );
    }

    #[test]
    fn routed_edge_points_anchor_near_node_borders() {
        // A point is "at" a rect when it lies within `tol` of the rect's bounds;
        // router endpoints attach to box-perimeter ports, so both ends land on
        // (or within a route-margin of) their node.
        fn near_rect(p: (f64, f64), r: Rect, tol: f64) -> bool {
            p.0 >= r.x - tol && p.0 <= r.x + r.w + tol && p.1 >= r.y - tol && p.1 <= r.y + r.h + tol
        }

        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        let edge = &scene.edges[0];
        assert!(edge.points.len() >= 2, "polyline needs both endpoints");

        // edge.source is order's rect, edge.target is customer's rect.
        let first = *edge.points.first().unwrap();
        let last = *edge.points.last().unwrap();
        assert!(
            near_rect(first, edge.source, 12.0),
            "first point {first:?} not anchored to source {:?}",
            edge.source
        );
        assert!(
            near_rect(last, edge.target, 12.0),
            "last point {last:?} not anchored to target {:?}",
            edge.target
        );
    }

    #[test]
    fn fallback_route_stays_axis_aligned() {
        // Every consecutive segment must be axis-aligned: the canvas strokes the
        // fallback with a single top-left->bottom-right diagonal pen, which would
        // render a raw diagonal on a negative-slope pair mirrored.
        fn assert_axis_aligned(points: &[(f64, f64)]) {
            assert!(points.len() >= 2, "fallback needs both endpoints");
            for w in points.windows(2) {
                let dx = (w[1].0 - w[0].0).abs();
                let dy = (w[1].1 - w[0].1).abs();
                assert!(
                    dx < f64::EPSILON || dy < f64::EPSILON,
                    "segment {:?}->{:?} is diagonal (dx={dx}, dy={dy})",
                    w[0],
                    w[1]
                );
            }
        }
        let rect = |x, y| Rect {
            x,
            y,
            w: 100.0,
            h: 100.0,
        };

        // Negative slope (target up-and-right of source): the regression case.
        let up_right = fallback_route(rect(0.0, 400.0), rect(400.0, 0.0));
        assert_axis_aligned(&up_right);
        // Endpoints preserved (centers), elbow inserted for the diagonal pair.
        assert_eq!(up_right.first().copied(), Some((50.0, 450.0)));
        assert_eq!(up_right.last().copied(), Some((450.0, 50.0)));
        assert_eq!(up_right.len(), 3);

        // Positive slope, plus axis-aligned pairs collapse to two points.
        assert_axis_aligned(&fallback_route(rect(0.0, 0.0), rect(400.0, 400.0)));
        assert_eq!(fallback_route(rect(0.0, 0.0), rect(0.0, 400.0)).len(), 2);
        assert_eq!(fallback_route(rect(0.0, 0.0), rect(400.0, 0.0)).len(), 2);
    }

    #[test]
    fn drawable_edges_drops_self_loops_from_the_scene() {
        // A self-loop (source == target) is not drawable: `drawable_edges` filters
        // it out, so it never reaches the router's route stream nor the scene's
        // consumption loop. Both must agree, or the ordered route-to-edge match
        // desyncs. mini has exactly one real edge (order -> customer); injecting a
        // self-loop must leave scene.edges unchanged.
        let mut model = mini();
        let mut self_loop = model.edges[0].clone();
        self_loop.target = self_loop.source.clone();
        model.edges.push(self_loop);

        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            &std::collections::HashSet::new(),
        );
        assert_eq!(
            scene.edges.len(),
            1,
            "self-loop must not produce a drawable scene edge"
        );
    }

    #[test]
    fn stress_default_scene_edges_carry_points() {
        let model = mini();
        // Clearing `layout` routes build_scene through stress_default (see
        // use_stress_default: layout.is_empty()).
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        assert!(super::use_stress_default(&diagram), "expected stress path");

        let (scene, _) = build_scene(&model, &diagram, &std::collections::HashSet::new());
        assert_eq!(scene.edges.len(), 1, "mini has one drawable edge");
        assert!(
            !scene.edges[0].points.is_empty(),
            "stress-default edges must carry a routed polyline"
        );
    }
}
