//! The render seam: pick a diagram, solve it, and flatten to plain data.
//! Nothing below this module touches makepad; nothing here touches a GPU.

use waml::diagnostic::Diagnostic;
use waml::layout::{Margin, Shape};
use waml::model::{
    CardinalityVisibility, Diagram, DiagramGroup, ElementType, Model, RelEnd, RelationshipKind,
};
use waml::multiplicity::Multiplicity;
use waml::solve::{
    constrain, resolve, route, stress, BoxId, BoxKind, Constraint, DroppedPlacement, FlagSet, Rect,
    Size, SizeMap, SolveConfig, Solved, SolvedGroup, SolvedRouting,
};

use crate::diagram_display::ResolvedDiagramDisplay;

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
    pub geometry: crate::MeasuredNodeGeometry,
    /// User-declared stereotypes (e.g. `aggregateRoot`), rendered as the card's
    /// «guillemet» eyebrow above the title. Distinct from the metaclass-derived
    /// `node_style::stereotype_label` (which handles «interface» etc.); this is
    /// the node's own `stereotype:` front-matter list.
    pub stereotypes: Vec<String>,
    /// Explicit policy gate for both authored and metaclass-derived stereotype
    /// eyebrows. An empty `stereotypes` list alone cannot represent "hidden",
    /// because interfaces and other metaclasses have a fallback eyebrow.
    pub stereotype_visible: bool,
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
    /// capped at `card::MAX_BODY_ROWS` with a `+ N more` footer (false). Set from
    /// `App`'s expanded key-set in `build_scene`; never derived from the model.
    /// Defaults `false` (collapsed) everywhere the model projects a node.
    pub expanded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    pub source: Rect,
    pub target: Rect,
    pub kind: RelationshipKind,
    /// Optional relationship label, carried through so the canvas can apply
    /// the diagram's `show_labels` policy without reaching back into the model.
    pub name: Option<waml::model::AssocName>,
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
    pub dir: waml::layout::Direction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SceneGroup {
    pub role: waml::model::DiagramGroupRole,
    pub bounds: Rect,
    pub heading_bounds: Rect,
    pub title: Option<String>,
    pub depth: u8,
}

/// A placement the solver could not honor, projected from `DroppedPlacement`
/// into slug-level relations for the editor's conflict error list.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneConflict {
    pub dropped: SceneRelation,
    pub conflicts_with: Vec<SceneRelation>,
}

// An empty scene (derived Default) is the sensible startup default (fed a real one via set_scene).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scene {
    pub visual_kind: crate::canvas::StructuralVisualKind,
    pub display: ResolvedDiagramDisplay,
    pub nodes: Vec<SceneNode>,
    pub groups: Vec<SolvedGroup>,
    pub use_case_groups: Vec<SceneGroup>,
    pub edges: Vec<SceneEdge>,
    pub relations: Vec<SceneRelation>,
    pub conflicts: Vec<SceneConflict>,
    /// World-space label placements the solver found for `edges`, keyed by
    /// `PlacedLabel::edge` (an index into `edges`) and `slot`. The renderer
    /// draws from these rects rather than deriving its own geometry.
    pub labels: Vec<waml::solve::label::PlacedLabel>,
}

/// Render an attribute's cardinality under the diagram display policy.
pub fn attribute_cardinality_text(
    authored: Option<&Multiplicity>,
    mode: CardinalityVisibility,
) -> Option<String> {
    match (mode, authored) {
        (CardinalityVisibility::Off, _) => None,
        (CardinalityVisibility::Explicit, None) => None,
        (CardinalityVisibility::Explicit, Some(m)) | (CardinalityVisibility::All, Some(m)) => {
            Some(format!("{{{}}}", m.as_str()))
        }
        (CardinalityVisibility::All, None) => Some("{1}".into()),
    }
}

/// Project classifier `key`'s attribute compartment rows via the shared
/// `inspector::build_view` seam, so the canvas card and the inspector panel
/// never re-derive UML member extraction. A non-classifier or missing key
/// yields no rows.
fn attribute_rows(
    model: &Model,
    key: &str,
    display: &ResolvedDiagramDisplay,
) -> Vec<crate::inspector::AttrRow> {
    use crate::inspector::{build_view, Subject};
    if !display.show_attributes {
        return Vec::new();
    }
    let Some(node) = model.nodes.iter().find(|node| node.key == key) else {
        return Vec::new();
    };
    let Some(view) = build_view(model, &Subject::Classifier(key.to_string())) else {
        return Vec::new();
    };
    let max = display.max_attributes.map(|max| max as usize);
    view.attributes
        .into_iter()
        .zip(&node.attributes)
        .take(max.unwrap_or(usize::MAX))
        .map(|(row, attribute)| crate::inspector::AttrRow {
            name: row.name,
            ty: if display.show_type {
                row.ty
            } else {
                String::new()
            },
            multiplicity: attribute_cardinality_text(
                attribute.multiplicity.as_ref(),
                display.cardinality,
            )
            .unwrap_or_default(),
            visibility: if display.show_attribute_visibility {
                row.visibility
            } else {
                String::new()
            },
        })
        .collect()
}

fn displayed_stereotypes(stereotypes: &[String], display: &ResolvedDiagramDisplay) -> Vec<String> {
    if !display.show_stereotype {
        return Vec::new();
    }
    match &display.stereotype_filter {
        Some(allowed) => stereotypes
            .iter()
            .filter(|stereotype| allowed.contains(stereotype))
            .cloned()
            .collect(),
        None => stereotypes.to_vec(),
    }
}

/// The card's «stereotype» eyebrow label (raw, no guillemets): the node's own
/// declared stereotypes if any, else the metaclass-derived label. Shared by the
/// focus-card sizer (`build_focus_scene`) and its renderer (`draw_focus_card`)
/// so both measure and draw the same line.
pub fn focus_eyebrow(
    stereotypes: &[String],
    ty: &ElementType,
    stereotype_visible: bool,
) -> Option<String> {
    if !stereotype_visible {
        None
    } else if !stereotypes.is_empty() {
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
    project_scene_node_with_display(model, node, &ResolvedDiagramDisplay::default())
}

/// Project model `node` into a `SceneNode` using its diagram's resolved display
/// policy. Enabled columns are the only text that reaches card measurement.
pub fn project_scene_node_with_display(
    model: &Model,
    node: &waml::model::Node,
    display: &ResolvedDiagramDisplay,
) -> SceneNode {
    SceneNode {
        key: node.key.clone(),
        title: node
            .concept
            .title
            .clone()
            .unwrap_or_else(|| node.key.clone()),
        element_type: node.ty.clone(),
        geometry: Default::default(),
        stereotypes: displayed_stereotypes(&node.stereotypes, display),
        stereotype_visible: display.show_stereotype,
        attributes: attribute_rows(model, &node.key, display),
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
fn operand_slug(op: &waml::layout::Operand, diagram: &Diagram) -> Option<String> {
    use waml::layout::{NameRef, OperandRef};
    match &op.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(waml::okf::resolve_href(
            &format!("{}.md", diagram.key),
            slug,
        )),
        OperandRef::Name(NameRef::Bare(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Project the diagram's `## Layout` into 2-operand single-direction relation
/// triples (subject_slug, reference_slug, dir). Mirrors `ops::placement_matches`'
/// shape: only 2-operand, 1-direction placements qualify.
fn project_relations(diagram: &Diagram) -> Vec<SceneRelation> {
    use waml::layout::LayoutStatement;
    let mut out = Vec::new();
    for stmt in &diagram.layout {
        if let LayoutStatement::Placement {
            operands,
            directions,
        } = stmt
        {
            if operands.len() == 2 && directions.len() == 1 {
                if let (Some(subject), Some(reference)) = (
                    operand_slug(&operands[0], diagram),
                    operand_slug(&operands[1], diagram),
                ) {
                    out.push(SceneRelation {
                        subject,
                        reference,
                        dir: directions[0],
                    });
                }
            }
        }
    }
    out
}

/// Project a single solver `Constraint::Place` into a slug-level `SceneRelation`.
/// Non-`Place` or non-`Node` operands (group/inline endpoints) yield `None`, so
/// only 2-node placements — the ones the conflict list can name — survive.
fn relation_of_constraint(c: &waml::solve::Constraint) -> Option<SceneRelation> {
    use waml::solve::Constraint;
    if let Constraint::Place {
        a: BoxId::Node(subject),
        b: BoxId::Node(reference),
        dir,
    } = c
    {
        Some(SceneRelation {
            subject: subject.clone(),
            reference: reference.clone(),
            dir: *dir,
        })
    } else {
        None
    }
}

/// Project the solver's dropped-placement report into `SceneConflict`s. A dropped
/// placement whose subject/reference don't both resolve to node slugs is skipped
/// (it can't be named in the DSL error list).
fn project_conflicts(dropped: &[waml::solve::DroppedPlacement]) -> Vec<SceneConflict> {
    dropped
        .iter()
        .filter_map(|d| {
            let dropped = relation_of_constraint(&d.relation)?;
            let conflicts_with = d
                .conflicts_with
                .iter()
                .filter_map(relation_of_constraint)
                .collect();
            Some(SceneConflict {
                dropped,
                conflicts_with,
            })
        })
        .collect()
}

/// DSL keyword for a placement direction (matches the `## Layout` surface form).
pub fn dir_keyword(d: waml::layout::Direction) -> &'static str {
    use waml::layout::Direction::*;
    match d {
        LeftOf => "left of",
        RightOf => "right of",
        Above => "above",
        Below => "below",
        AboveLeft => "above left of",
        AboveRight => "above right of",
        BelowLeft => "below left of",
        BelowRight => "below right of",
    }
}

/// Render one relation as its `A <dir> B` DSL form.
pub fn relation_statement(r: &SceneRelation) -> String {
    format!("{} {} {}", r.subject, dir_keyword(r.dir), r.reference)
}

/// Human-readable error-list text for a dropped constraint: the dropped
/// statement, the statements it contradicts, and a one-line "these contradict"
/// note (spec §4). Superseded as the badge's error-list rendering by
/// `popup::conflict_list::rows_of`'s per-row `relation_statement`s (grouped,
/// deletable list); kept (not deleted) as a plain one-line summary form for a
/// future non-interactive use (tooltip/log), per "add-only, prune later".
#[allow(dead_code)]
pub fn conflict_statement(c: &SceneConflict) -> String {
    let mut lines = vec![relation_statement(&c.dropped)];
    for w in &c.conflicts_with {
        lines.push(relation_statement(w));
    }
    format!("{}  —  these contradict", lines.join("; "))
}

/// Every node key involved in a conflict (dropped + all contradicting relations),
/// for the fade-the-rest focus (spec §4). Not deduped — callers dedup as needed.
/// Canvas now fades by an explicit key set (`set_conflict_focus_keys`, keyed
/// off a single focused RELATION's two nodes, not a whole conflict group) --
/// kept for a future group-scoped ("All", spec §7) fade that DOES want every
/// participant, per "add-only, prune later".
#[allow(dead_code)]
pub fn conflict_participants(c: &SceneConflict) -> Vec<String> {
    let mut out = vec![c.dropped.subject.clone(), c.dropped.reference.clone()];
    for w in &c.conflicts_with {
        out.push(w.subject.clone());
        out.push(w.reference.clone());
    }
    out
}

/// Per-kept-group bookkeeping for the entanglement warning: what the hull is
/// labelled, how deep it sits, and which kept group encloses it. `parent` is
/// the nearest *kept* ancestor — a dropped one contributed no resolved
/// members, so it can share none and skipping it changes no answer.
struct GroupMeta {
    title: Option<String>,
    parent: Option<usize>,
}

impl GroupMeta {
    /// Name for a diagnostic. Unnamed groups exist (an untitled `###` block that
    /// carries children), so fall back to a positional label rather than an
    /// empty pair of backticks.
    fn label(&self, i: usize) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| format!("<unnamed group {i}>"))
    }
}

/// Flatten a `DiagramGroup` forest into member-index sets (resolved against
/// `index`, a sizes-derived key->index map, so an unresolved/unsized member
/// key never contributes a phantom hull) plus a parallel `GroupMeta` list, in
/// the same pre-order the input tree is walked.
///
/// Purely diagnostic: used ONLY to compute the entanglement warning below.
/// Real cohesion/hulls/routing come from `constrain::Compiled` (the box
/// forest `resolve::resolve` builds), which enforces single ownership across
/// the WHOLE scene — a node declared under two `### ` sections is legal WAML
/// (the entanglement warning exists precisely to flag it) but can only be
/// parented ONCE in a layout tree, so `resolve::add_group` drops the second
/// claim. Re-deriving straight from the raw, unresolved `DiagramGroup` forest
/// here is what lets a doubly-declared member still show up in BOTH sets.
fn flatten_groups(
    groups: &[DiagramGroup],
    index: &std::collections::BTreeMap<&str, usize>,
    parent: Option<usize>,
    specs: &mut Vec<std::collections::BTreeSet<usize>>,
    meta: &mut Vec<GroupMeta>,
) {
    fn collect_members(
        g: &DiagramGroup,
        index: &std::collections::BTreeMap<&str, usize>,
        acc: &mut std::collections::BTreeSet<usize>,
    ) {
        for m in &g.members {
            if let Some(&i) = index.get(m.as_str()) {
                acc.insert(i);
            }
        }
        for c in &g.children {
            collect_members(c, index, acc);
        }
    }

    for g in groups {
        // The parser wraps a diagram's flat `## Members` list in a trivial
        // (unnamed, childless) member group. Skip it; a *named* group (or an
        // unnamed one that carries real `###` children) is real.
        if g.name.is_empty() && g.children.is_empty() {
            continue;
        }
        let mut members = std::collections::BTreeSet::new();
        collect_members(g, index, &mut members);
        let mut child_parent = parent;
        if !members.is_empty() {
            specs.push(members);
            let title = if g.name.is_empty() {
                None
            } else {
                Some(g.name.clone())
            };
            child_parent = Some(meta.len());
            meta.push(GroupMeta { title, parent });
        }
        flatten_groups(&g.children, index, child_parent, specs, meta);
    }
}

/// Group pairs whose hulls will ship tangled: they share at least one resolved
/// member and neither encloses the other.
///
/// This predicate MUST stay the mirror of `stress::separate_hulls`'s own
/// `is_entangled`, which refuses to translate *any* pair with a shared member —
/// moving one set drags the shared node out of the other, so the passes would
/// just fight until the cap. Genuine nesting is the one case that shares members
/// and is still fine (an inner hull is *supposed* to sit inside its outer), so
/// it alone is excluded here.
///
/// Runs on the same resolved indices `flatten_groups` produced, not on member
/// names: a shared key that resolves to nothing has no geometry, so it cannot
/// tangle anything and must not raise a warning.
fn entangled_group_pairs(
    sets: &[std::collections::BTreeSet<usize>],
    meta: &[GroupMeta],
) -> Vec<(String, String)> {
    let encloses = |anc: usize, mut node: usize| -> bool {
        while let Some(p) = meta[node].parent {
            if p == anc {
                return true;
            }
            node = p;
        }
        false
    };

    let mut pairs = Vec::new();
    for i in 0..sets.len() {
        for j in (i + 1)..sets.len() {
            if sets[i].is_disjoint(&sets[j]) || encloses(i, j) || encloses(j, i) {
                continue;
            }
            pairs.push((meta[i].label(i), meta[j].label(j)));
        }
    }
    pairs
}

/// Route `route_edges` over solved stress geometry, containment-aware.
///
/// The rect map the router works from carries the node rects **and** each
/// group's hull under `BoxId::Group(i)`, so a hull is an obstacle exactly like
/// a solved group frame is on the routing side; the box forest gives
/// the router the membership it must never infer from rect overlap. Returns the
/// routes plus both router inputs, so the caller can hand them to
/// `place_labels_with_reroute` and reroute under the same containment rules.
///
/// Split out of `stress_layout` so containment-aware routing is testable
/// against hand-placed geometry (the stress solve's own placement is not
/// controllable enough to stage a crossing).
struct SceneRoutePolicy<'a> {
    hard_obstacles: &'a [Rect],
    cost: &'a route::RouteCost,
}

#[allow(clippy::type_complexity)]
#[inline(never)]
fn route_with_groups(
    keys: &[String],
    rects: &[Rect],
    hulls: &[Rect],
    group_specs: &[stress::GroupSpec],
    group_depths: &[u8],
    route_edges: &[(BoxId, BoxId)],
    policy: &SceneRoutePolicy,
) -> (
    Vec<waml::solve::Route>,
    std::collections::BTreeMap<BoxId, Rect>,
    Vec<waml::solve::Box>,
) {
    let mut rect_map: std::collections::BTreeMap<BoxId, Rect> = keys
        .iter()
        .cloned()
        .map(BoxId::Node)
        .zip(rects.iter().copied())
        .collect();
    for (i, hull) in hulls.iter().enumerate() {
        rect_map.insert(BoxId::Group(i as u32), *hull);
    }
    for (i, obstacle) in policy.hard_obstacles.iter().enumerate() {
        rect_map.insert(BoxId::Inline(i as u32), *obstacle);
    }

    // One `Box` per group (flat: `GroupSpec::members` already includes every
    // descendant member, so `build_membership`'s leaf-child walk needs no
    // nested Group children to recurse through). This is the only input the
    // router derives group membership from — never rect overlap.
    let boxes: Vec<waml::solve::Box> = group_specs
        .iter()
        .enumerate()
        .map(|(i, spec)| waml::solve::Box {
            id: BoxId::Group(i as u32),
            kind: BoxKind::Group,
            children: spec
                .members
                .iter()
                .map(|&m| BoxId::Node(keys[m].clone()))
                .collect(),
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title: None,
            depth: group_depths.get(i).copied().unwrap_or(0),
        })
        .collect();

    let keyed = route_edges
        .iter()
        .map(|(source, target)| (source.clone(), target.clone(), None, None))
        .collect::<Vec<_>>();
    let routes = route::route_keyed_with(
        &boxes,
        &rect_map,
        &keyed,
        &SolveConfig::default(),
        policy.cost,
    );
    (routes, rect_map, boxes)
}

const USE_CASE_CROSSING_PENALTY: f64 = 2048.0;

fn diagram_route_cost(kind: waml::model::DiagramKind) -> route::RouteCost {
    if kind == waml::model::DiagramKind::UseCase {
        route::RouteCost {
            crossing: USE_CASE_CROSSING_PENALTY,
            ..route::RouteCost::default()
        }
    } else {
        route::RouteCost::default()
    }
}

/// Order-independent key for an unordered box pair (local twin of
/// `waml::solve::geometry::pair`, which is `pub(super)` and not visible
/// outside the `waml` crate).
fn pair(a: &BoxId, b: &BoxId) -> (BoxId, BoxId) {
    if a <= b {
        (a.clone(), b.clone())
    } else {
        (b.clone(), a.clone())
    }
}

/// Unordered node<->node edge-connected pairs, so `constrain::compile`'s gap
/// policy can floor a `Place` gap between associated boxes. Group-as-endpoint
/// edges are ignored.
fn connected_pairs(edges: &[(BoxId, BoxId)]) -> std::collections::BTreeSet<(BoxId, BoxId)> {
    let mut connected = std::collections::BTreeSet::new();
    for (a, b) in edges {
        if matches!(a, BoxId::Node(_)) && matches!(b, BoxId::Node(_)) {
            connected.insert(pair(a, b));
        }
    }
    connected
}

/// The unified placement path: authored `## Layout` statements compile
/// (`constrain::compile`) into hard separation/alignment constraints
/// projected inside the stress solve (`stress::layout_constrained`), instead
/// of switching the whole diagram onto the edge-blind rigid-offset strip
/// packer the moment any `place` statement is authored. Every diagram --
/// hinted or not, grouped or not -- goes through this one function.
///
/// Node set is `compiled.keys` (scene order) plus any sized-but-unauthored
/// key appended after (still renders, matching the old "every sized member
/// solves" contract). `compiled.seps`' boundary-var indices assume a base
/// offset of `compiled.keys.len()`; those are shifted to `ids.len()` so they
/// land on `layout_constrained`'s own extra-var space, which starts after
/// EVERY id (including the appended ones).
#[allow(clippy::type_complexity)]
#[inline(never)]
fn stress_layout(
    diagram: &Diagram,
    compiled: &constrain::Compiled,
    sizes: &SizeMap,
    model_edges: &[&waml::model::Edge],
) -> (
    Solved,
    SolvedRouting,
    Vec<(String, String)>,
    Vec<DroppedPlacement>,
    Vec<Diagnostic>,
) {
    use std::collections::{BTreeMap, BTreeSet};
    use waml::solve::vpsc::Sep;

    let base_cfg = SolveConfig::default();
    let n = compiled.keys.len();
    let mut keys = compiled.keys.clone();
    let mut dims: Vec<Size> = (0..n)
        .map(|i| {
            if compiled.flags[i].collapsed {
                base_cfg.chip
            } else {
                sizes.get(&keys[i]).copied().unwrap_or(base_cfg.chip)
            }
        })
        .collect();

    // Every sized key the scene never mentioned still renders.
    let known: BTreeSet<&str> = keys.iter().map(|k| k.as_str()).collect();
    let mut extra_keys: Vec<String> = sizes
        .keys()
        .filter(|k| !known.contains(k.as_str()))
        .cloned()
        .collect();
    extra_keys.sort(); // deterministic: no authored order exists for these
    let extra = extra_keys.len();
    for k in &extra_keys {
        dims.push(sizes[k]);
    }
    keys.extend(extra_keys);

    let ids: Vec<BoxId> = keys.iter().cloned().map(BoxId::Node).collect();
    let index: BTreeMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();

    let mut seen = BTreeSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for e in model_edges {
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

    let shift = |i: usize| if i < n { i } else { i + extra };
    let shift_sep = |s: &Sep| Sep {
        left: shift(s.left),
        right: shift(s.right),
        ..*s
    };
    let seps = stress::SepSpecs {
        x: compiled.seps.x.iter().map(shift_sep).collect(),
        y: compiled.seps.y.iter().map(shift_sep).collect(),
        extra_vars: compiled.seps.extra_vars,
    };

    let cohesion_groups: Vec<stress::GroupSpec> = compiled
        .group_specs
        .iter()
        .cloned()
        .chain(compiled.inline_specs.iter().cloned())
        .collect();

    let cfg = stress::StressConfig::default();
    let (rects, all_hulls, (dx, dy)) =
        stress::layout_constrained(&ids, &dims, &pairs, &cohesion_groups, &seps, &cfg);
    // `layout_constrained`'s hull list is one-per-`cohesion_groups`-entry;
    // inline groups (appended after the real ones) never get a hull.
    let hulls: Vec<Rect> = all_hulls
        .into_iter()
        .take(compiled.group_specs.len())
        .collect();

    let route_edges: Vec<(BoxId, BoxId)> = model_edges
        .iter()
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();
    let group_depths: Vec<u8> = compiled.group_meta.iter().map(|(_, d, _)| *d).collect();
    let groups: Vec<SolvedGroup> = hulls
        .iter()
        .copied()
        .zip(compiled.group_meta.iter())
        .map(|(rect, (title, depth, shape))| SolvedGroup {
            rect,
            shape: *shape,
            title: title.clone(),
            depth: *depth,
        })
        .collect();
    let hard_obstacles = if diagram.kind == waml::model::DiagramKind::UseCase {
        project_use_case_scene_groups(diagram, &groups)
            .into_iter()
            .filter(|group| group.role != waml::model::DiagramGroupRole::ExternalActors)
            .map(|group| group.heading_bounds)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let route_cost = diagram_route_cost(diagram.kind);
    let (routes, rect_map, boxes) = route_with_groups(
        &keys,
        &rects,
        &hulls,
        &compiled.group_specs,
        &group_depths,
        &route_edges,
        &SceneRoutePolicy {
            hard_obstacles: &hard_obstacles,
            cost: &route_cost,
        },
    );

    let mut flags: BTreeMap<String, FlagSet> = BTreeMap::new();
    for (key, flag) in keys.iter().take(n).zip(compiled.flags.iter()) {
        if flag.emphasized || flag.collapsed {
            flags.insert(key.clone(), *flag);
        }
    }

    let solved = Solved {
        nodes: keys.iter().cloned().zip(rects).collect(),
        groups,
        flags,
        routes,
        labels: Vec::new(),
        label_reroutes: 0,
        label_leaders: 0,
    };
    let routing = SolvedRouting {
        boxes,
        rects: rect_map,
        edges: route_edges.into_iter().map(|(s, t)| (s, t, None)).collect(),
    };

    // Entanglement is diagnostic-only, re-derived straight from the raw,
    // unresolved `diagram.groups` forest against a sizes-keyed index (see
    // `flatten_groups`' doc): the box forest `constrain::Compiled` came from
    // enforces single ownership, so a node declared under two `### `
    // sections — legal WAML, and exactly what this warning exists to flag —
    // only keeps its FIRST group membership there.
    let legacy_index: BTreeMap<&str, usize> = sizes
        .keys()
        .enumerate()
        .map(|(i, k)| (k.as_str(), i))
        .collect();
    let mut legacy_specs: Vec<BTreeSet<usize>> = Vec::new();
    let mut legacy_meta: Vec<GroupMeta> = Vec::new();
    flatten_groups(
        &diagram.groups,
        &legacy_index,
        None,
        &mut legacy_specs,
        &mut legacy_meta,
    );
    let entangled = entangled_group_pairs(&legacy_specs, &legacy_meta);

    // Solver-dropped seps -> DroppedPlacement, deduped by relation (a
    // diagonal dropped on both axes must report once). `conflicts_with`: the
    // OTHER live seps on the same axis pinning the exact same variable pair
    // (a direct A-vs-B reversal, e.g. authoring both `a left of b` and
    // `b left of a`) — narrower than full rigid-component reachability
    // (which `vpsc::project`'s dropped-index-only return doesn't expose),
    // but it covers the direct-contradiction case the conflict list exists
    // for.
    let same_pair_conflicts = |seps: &[Sep],
                               provenance: &[Option<Constraint>],
                               dropped_idx: usize,
                               own: &Constraint|
     -> Vec<Constraint> {
        let d = seps[dropped_idx];
        let mut out = Vec::new();
        for (j, s) in seps.iter().enumerate() {
            if j == dropped_idx {
                continue;
            }
            let same_pair = (s.left == d.left && s.right == d.right)
                || (s.left == d.right && s.right == d.left);
            if !same_pair {
                continue;
            }
            if let Some(Some(c)) = provenance.get(j) {
                if c != own && !out.contains(c) {
                    out.push(c.clone());
                }
            }
        }
        out
    };

    let mut relations: Vec<(Constraint, Vec<Constraint>)> = Vec::new();
    for &i in &dx {
        if let Some(Some(c)) = compiled.provenance_x.get(i) {
            if !relations.iter().any(|(r, _)| r == c) {
                let conflicts = same_pair_conflicts(&seps.x, &compiled.provenance_x, i, c);
                relations.push((c.clone(), conflicts));
            }
        }
    }
    for &i in &dy {
        if let Some(Some(c)) = compiled.provenance_y.get(i) {
            if !relations.iter().any(|(r, _)| r == c) {
                let conflicts = same_pair_conflicts(&seps.y, &compiled.provenance_y, i, c);
                relations.push((c.clone(), conflicts));
            }
        }
    }
    let solver_dropped_count = relations.len();
    let mut dropped: Vec<DroppedPlacement> = relations
        .into_iter()
        .map(|(relation, conflicts_with)| DroppedPlacement {
            relation,
            conflicts_with,
        })
        .collect();
    dropped.extend(compiled.dropped.iter().cloned());

    let mut diags: Vec<Diagnostic> = entangled
        .iter()
        .map(|(a, b)| {
            Diagnostic::warn(
                waml::diagnostic::DiagCode::EntangledGroups,
                format!(
                    "groups `{a}` and `{b}` share members without nesting; their clusters cannot be separated and will overlap"
                ),
                diagram.key.clone(),
                0,
            )
        })
        .collect();
    // One diagnostic per dropped constraint. The solver-dropped prefix is a
    // genuine contradiction. The compile-time suffix
    // (`compiled.dropped`: unknown/sizeless operand, alignment with no
    // shared axis, group-center align) never reaches the solver but must
    // surface too — the retired geometry path warned for each of these
    // ("alignment edges share no axis", ...), and swallowing them leaves an
    // authored statement silently without effect.
    diags.extend(dropped.iter().enumerate().map(|(i, d)| {
        let msg = if i < solver_dropped_count {
            "conflicting layout constraint dropped"
        } else {
            match &d.relation {
                Constraint::Align { .. } => "alignment constraint could not be applied; dropped",
                Constraint::Place { .. } => "placement constraint could not be applied; dropped",
            }
        };
        Diagnostic::warn(
            waml::diagnostic::DiagCode::LayoutConflict,
            msg,
            diagram.key.clone(),
            0,
        )
    }));

    (solved, routing, entangled, dropped, diags)
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
#[inline(never)]
pub fn build_scene(
    model: &Model,
    diagram: &Diagram,
    display: ResolvedDiagramDisplay,
    expanded: &std::collections::HashSet<String>,
) -> (Scene, Vec<Diagnostic>) {
    use std::collections::BTreeMap;

    // Key -> model node, built once up front: the size loop below and the
    // scene-node projection loop further down both look nodes up per key
    // (P-5: the size loop used to scan `model.nodes` per key, O(n^2)).
    let node_of: BTreeMap<&str, &waml::model::Node> =
        model.nodes.iter().map(|n| (n.key.as_str(), n)).collect();

    let visual_kind = match diagram.kind {
        waml::model::DiagramKind::UseCase => crate::StructuralVisualKind::UseCase,
        _ => crate::StructuralVisualKind::Class,
    };
    let policy = crate::StructuralVisualPolicy { kind: visual_kind };
    let text = crate::MonoTextMeasurer;
    let measured: BTreeMap<&str, crate::MeasuredNodeGeometry> = node_of
        .iter()
        .map(|(key, node)| (*key, crate::measure_node(policy, node, &text)))
        .collect();

    let mut sizes = crate::sizing::size_map(model, diagram, expanded);
    for (key, size) in &mut sizes {
        if let Some(node) = node_of.get(key.as_str()).copied() {
            let mut projected = project_scene_node_with_display(model, node, &display);
            projected.expanded = expanded.contains(key);
            if let Some(bounds) = measured
                .get(key.as_str())
                .and_then(|geometry| geometry.bounds())
            {
                *size = Size {
                    w: bounds.w,
                    h: bounds.h,
                };
            } else {
                let (w, h) = crate::card::card_size(&projected, &crate::card::mono_sheet());
                *size = Size { w, h };
            }
        }
    }
    let model_edges = drawable_edges(model);
    let edges: Vec<(BoxId, BoxId)> = model_edges
        .iter()
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();
    // Label text is known before any geometry is, so the connected-gap floor can
    // be sized to hold each pair's terminal labels. These requests index into
    // `model_edges`, which is the same list (same order) as `edges`.
    let sizing_requests =
        crate::edge_labels::model_label_requests_with_policy(&model_edges, &display, policy);

    // Every diagram -- hinted or not, grouped or not -- goes through the one
    // unified placement path: authored `## Layout` statements compile into
    // hard separation/alignment constraints projected inside the stress
    // solve, instead of switching the whole diagram onto the edge-blind
    // rigid-offset strip packer the moment any `place` statement is authored.
    let relationship_pairs = edges.clone();
    let (scene, resolve_diags) = if diagram.kind == waml::model::DiagramKind::UseCase {
        waml::solve::use_case::resolve_use_case(diagram, &relationship_pairs)
    } else {
        resolve::resolve(diagram)
    };
    let connected = connected_pairs(&edges);
    let label_widths = waml::solve::connected_label_widths(&edges, &sizing_requests);
    let compiled = constrain::compile(
        &scene,
        &sizes,
        &label_widths,
        &connected,
        &SolveConfig::default(),
    );
    let (mut solved, routing, _entangled, dropped, mut diags) =
        stress_layout(diagram, &compiled, &sizes, &model_edges);
    diags.extend(resolve_diags);

    let mut nodes = Vec::with_capacity(solved.nodes.len());
    for (key, rect) in &solved.nodes {
        let flags = solved.flags.get(key).copied().unwrap_or_default();
        let mut node = match node_of.get(key.as_str()).copied() {
            Some(model_node) => project_scene_node_with_display(model, model_node, &display),
            // Keys with no resolving model node (synthetic/unknown) fall back to
            // a title-only node: key as title, Unknown type, no members.
            None => SceneNode {
                key: key.clone(),
                title: key.clone(),
                element_type: ElementType::Unknown(String::new()),
                geometry: Default::default(),
                stereotypes: Vec::new(),
                stereotype_visible: display.show_stereotype,
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
        if let Some(geometry) = measured.get(key.as_str()) {
            node.geometry = geometry.translated(rect.x, rect.y);
        }
        node.rect = node.geometry.bounds().unwrap_or(*rect);
        node.emphasized = flags.emphasized;
        node.collapsed = flags.collapsed;
        node.expanded = expanded.contains(key);
        nodes.push(node);
    }

    let use_case_groups = project_use_case_scene_groups(diagram, &solved.groups);
    let ports = nodes
        .iter()
        .map(|node| (node.key.clone(), port_geometry(node)))
        .collect::<BTreeMap<_, _>>();

    // Walk the same `drawable_edges` list route::route was fed, so the ordered
    // route stream and this consumption stay locked together by construction.
    // Only edges whose endpoints both appear in the solved layout are drawable;
    // match each to its Route by consuming solved.routes IN ORDER (key-only
    // lookup is ambiguous for parallel edges). On a key mismatch (e.g. an edge
    // route::route presence-filtered out, desyncing the stream) fall back to a
    // straight center-to-center polyline WITHOUT advancing the cursor, so later
    // edges stay aligned.
    let mut edges: Vec<SceneEdge> = Vec::new();
    let mut edge_endpoint_keys: Vec<(String, String)> = Vec::new();
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
                name: e.name.clone(),
                from_end: e.from_end.clone(),
                to_end: e.to_end.clone(),
                points,
            });
            edge_endpoint_keys.push((e.source.clone(), e.target.clone()));
        }
    }

    // Requests index into `edges` (the scene's drawable edges), and `edges` is
    // NOT `solved.routes`: presence-filtering and the straight-polyline fallback
    // above let the two lists desync. Hand placement the very polylines the
    // requests were built from, so a label can never be placed against another
    // edge's route.
    //
    // Rerouting is part of placement: an edge whose label fits nowhere asks the
    // router for a path that leaves room for it. `place_labels_with_reroute`
    // disables the reroute when this list and the router's own route list have
    // desynced, degrading to a plain placement pass.
    let requests = crate::edge_labels::label_requests_with_policy(&edges, &display, policy);
    let mut routes: Vec<Vec<(f64, f64)>> = edges.iter().map(|e| e.points.clone()).collect();
    let unresolved = waml::solve::place_labels_with_reroute_cost(
        &mut solved,
        &routing.context(&SolveConfig::default()),
        &mut routes,
        &requests,
        &waml::solve::label::LabelConfig::default(),
        diagram_route_cost(diagram.kind),
    );
    // A reroute moves polylines, and the scene draws from `edges`, not from
    // `solved.routes`.
    for (edge, points) in edges.iter_mut().zip(routes) {
        edge.points = points;
    }
    for (edge, (source_key, target_key)) in edges.iter_mut().zip(&edge_endpoint_keys) {
        if edge.points.len() < 2 {
            continue;
        }
        if let (Some(source_port), Some(target_port)) =
            (ports.get(source_key), ports.get(target_key))
        {
            waml::solve::route::clip_route_endpoints(&mut edge.points, source_port, target_port);
        }
    }
    debug_assert!(
        unresolved.is_empty(),
        "edge labels with no position at all: {unresolved:?}"
    );

    let relations = project_relations(diagram);
    let conflicts = project_conflicts(&dropped);
    (
        Scene {
            visual_kind,
            display,
            nodes,
            groups: solved.groups,
            use_case_groups,
            edges,
            relations,
            conflicts,
            labels: solved.labels,
        },
        diags,
    )
}

fn port_geometry(node: &SceneNode) -> waml::solve::route::PortGeometry {
    match &node.geometry {
        crate::MeasuredNodeGeometry::UseCase(geometry) => {
            waml::solve::route::PortGeometry::Ellipse(geometry.bounds)
        }
        crate::MeasuredNodeGeometry::Actor(geometry) => {
            let segments = std::iter::once(geometry.body)
                .chain(geometry.arms)
                .chain(geometry.legs)
                .map(|segment| {
                    (
                        (segment.from.x, segment.from.y),
                        (segment.to.x, segment.to.y),
                    )
                })
                .collect();
            waml::solve::route::PortGeometry::Actor {
                bounds: geometry.bounds,
                head_center: (geometry.head_center.x, geometry.head_center.y),
                head_radius: geometry.head_radius,
                stroke_radius: 2.0,
                segments,
            }
        }
        crate::MeasuredNodeGeometry::ClassCard
        | crate::MeasuredNodeGeometry::Note
        | crate::MeasuredNodeGeometry::Package => {
            waml::solve::route::PortGeometry::Rectangle(node.rect)
        }
    }
}

fn project_use_case_scene_groups(diagram: &Diagram, solved: &[SolvedGroup]) -> Vec<SceneGroup> {
    fn semantics(groups: &[DiagramGroup], out: &mut Vec<(waml::model::DiagramGroupRole, String)>) {
        for group in groups {
            semantics(&group.children, out);
            // resolve::Builder pushes group boxes after their children, and
            // constrain::compile preserves that postorder in group_meta.
            out.push((group.role, group.name.clone()));
        }
    }
    let mut ordered_semantics = Vec::new();
    semantics(&diagram.groups, &mut ordered_semantics);
    let max_depth = solved.iter().map(|group| group.depth).max().unwrap_or(0);
    solved
        .iter()
        .zip(ordered_semantics)
        .map(|(group, (role, title))| {
            let reserve = if matches!(
                role,
                waml::model::DiagramGroupRole::SystemBoundary | waml::model::DiagramGroupRole::Band
            ) {
                (max_depth - group.depth + 1) as f64 * waml::solve::label::GROUP_TITLE_BAND
            } else {
                0.0
            };
            let heading_y = group.rect.y - reserve;
            SceneGroup {
                role,
                bounds: Rect {
                    y: heading_y,
                    h: group.rect.h + reserve,
                    ..group.rect
                },
                heading_bounds: Rect {
                    x: group.rect.x,
                    y: heading_y,
                    // The overlay only needs to protect the heading run, not
                    // erase the full-width frame and every route beneath it.
                    w: ((title.chars().count() as f64 * 8.0) + 12.0).min(group.rect.w),
                    h: waml::solve::label::GROUP_TITLE_BAND,
                },
                title: Some(title),
                depth: group.depth,
            }
        })
        .collect()
}

/// Build a single-node `Scene` focused on classifier `key`, sized 1.5x its
/// natural box. Used by the classifier focus view (double/single-click a class
/// in the tree). An unknown key yields an empty scene.
pub fn build_focus_scene(model: &Model, key: &str) -> Scene {
    let Some(node) = model.nodes.iter().find(|n| n.key == key) else {
        return Scene {
            visual_kind: Default::default(),
            display: ResolvedDiagramDisplay::default(),
            nodes: vec![],
            groups: vec![],
            use_case_groups: vec![],
            edges: vec![],
            relations: Vec::new(),
            conflicts: Vec::new(),
            labels: Vec::new(),
        };
    };
    let title = node
        .concept
        .title
        .clone()
        .unwrap_or_else(|| node.key.clone());
    let display = ResolvedDiagramDisplay::default();
    let attributes = attribute_rows(model, key, &display);
    // The focus card is drawn at zoom 1.0 (world px == screen px). Build the
    // scene node, then size its rect to the exact hull the card box-tree hugs.
    let mut scene_node = SceneNode {
        key: key.to_string(),
        title,
        element_type: node.ty.clone(),
        geometry: Default::default(),
        stereotypes: displayed_stereotypes(&node.stereotypes, &display),
        stereotype_visible: display.show_stereotype,
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
        visual_kind: Default::default(),
        display: ResolvedDiagramDisplay::default(),
        nodes: vec![scene_node],
        groups: vec![],
        use_case_groups: vec![],
        edges: vec![],
        relations: Vec::new(),
        conflicts: Vec::new(),
        labels: Vec::new(),
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

/// True iff a placement matches the given UNORDERED `{subject, reference}` pair
/// as a 2-operand single-direction relation, in EITHER operand order (mirrors
/// `ops::placement_matches`). Pair-symmetric so a reversed-pair re-drag replaces
/// the existing relation rather than stacking a conflicting one.
fn placement_is_pair(
    stmt: &waml::layout::LayoutStatement,
    diagram: &Diagram,
    subject: &str,
    reference: &str,
) -> bool {
    use waml::layout::LayoutStatement;
    if let LayoutStatement::Placement {
        operands,
        directions,
    } = stmt
    {
        let (a, b) = (
            operand_slug(&operands[0], diagram),
            operand_slug(&operands[1], diagram),
        );
        operands.len() == 2
            && directions.len() == 1
            && ((a.as_deref() == Some(subject) && b.as_deref() == Some(reference))
                || (a.as_deref() == Some(reference) && b.as_deref() == Some(subject)))
    } else {
        false
    }
}

/// Speculatively author `subject <dir> reference` into a scratch clone of the
/// diagram (one-relation-per-pair replace: drop any existing placement for this
/// ordered pair, then push the hypothetical one), re-solve, and report whether
/// the solver emits a `LayoutConflict`. The solver is the ground truth — it
/// catches transitive / cycle contradictions a hand-rolled rule would miss.
///
/// Production callers want the candidate layout too and go through
/// `placement_preview`; this verdict-only wrapper is the tests' entry point.
#[cfg(test)]
pub fn placement_would_conflict(
    model: &Model,
    diagram: &Diagram,
    subject_slug: &str,
    reference_slug: &str,
    dir: waml::layout::Direction,
    expanded: &std::collections::HashSet<String>,
) -> bool {
    placement_preview(model, diagram, subject_slug, reference_slug, dir, expanded).0
}

fn placement_candidate(
    model: &Model,
    diagram: &Diagram,
    subject_slug: &str,
    reference_slug: &str,
    dir: waml::layout::Direction,
) -> Diagram {
    use waml::layout::{LayoutStatement, NameRef, Operand, OperandRef};

    let referring_path = format!("{}.md", diagram.key);
    let link = |slug: &str| -> Operand {
        let target_path = format!("{slug}.md");
        Operand {
            ref_: OperandRef::Name(NameRef::Link {
                title: title_for(model, slug),
                slug: waml::okf::relative_href(&referring_path, &target_path),
            }),
            axis: None,
            hints: Vec::new(),
        }
    };

    let mut scratch = diagram.clone();
    scratch
        .layout
        .retain(|statement| !placement_is_pair(statement, diagram, subject_slug, reference_slug));
    scratch.layout.push(LayoutStatement::Placement {
        operands: vec![link(subject_slug), link(reference_slug)],
        directions: vec![dir],
    });
    scratch
}

/// The same speculative solve as `placement_would_conflict`, but keeping the
/// candidate layout it produced (node key -> world rect) instead of throwing it
/// away. The drag-time hover preview animates the canvas into this layout, so
/// the two callers share one solve: the compass reddens a zone from `.0` and the
/// preview tweens toward `.1`.
pub fn placement_preview(
    model: &Model,
    diagram: &Diagram,
    subject_slug: &str,
    reference_slug: &str,
    dir: waml::layout::Direction,
    expanded: &std::collections::HashSet<String>,
) -> (bool, std::collections::BTreeMap<String, Rect>) {
    use waml::diagnostic::DiagCode;
    let scratch = placement_candidate(model, diagram, subject_slug, reference_slug, dir);

    // Solve the candidate through the SAME unified path `build_scene` ships
    // (resolve -> constrain::compile -> stress_layout), so the preview animates
    // toward the layout a drop would actually produce and the compass reddens
    // exactly the placements that path would drop. Label-width gap floors are
    // skipped (no `display` here) -- a fidelity nit that can only pad gaps, it
    // cannot change the conflict verdict.
    let sizes = crate::sizing::size_map(model, &scratch, expanded);
    let model_edges = drawable_edges(model);
    let edges: Vec<(BoxId, BoxId)> = model_edges
        .iter()
        .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
        .collect();
    let (scene, _resolve_diags) = resolve::resolve(&scratch);
    let connected = connected_pairs(&edges);
    let label_widths = waml::solve::connected_label_widths(&edges, &[]);
    let compiled = constrain::compile(
        &scene,
        &sizes,
        &label_widths,
        &connected,
        &SolveConfig::default(),
    );
    let (solved, _routing, _entangled, _dropped, diags) =
        stress_layout(&scratch, &compiled, &sizes, &model_edges);
    let conflict = diags.iter().any(|d| d.code == DiagCode::LayoutConflict);
    (conflict, solved.nodes)
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

    fn real_use_case_scene(title: &str) -> Scene {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/waml");
        let model = crate::load::load_model(&root).unwrap();
        let diagram = model
            .diagrams
            .iter()
            .find(|diagram| diagram.title == title)
            .unwrap();
        build_scene(
            &model,
            diagram,
            test_display(),
            &std::collections::HashSet::new(),
        )
        .0
    }

    fn editor_workflow_scene() -> Scene {
        real_use_case_scene("Editor Workflows")
    }

    fn proper_segment_crossing(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
        fn orient(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
            (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
        }
        let (o1, o2, o3, o4) = (
            orient(a, b, c),
            orient(a, b, d),
            orient(c, d, a),
            orient(c, d, b),
        );
        o1 * o2 < -1e-6 && o3 * o4 < -1e-6
    }

    fn route_crossings(scene: &Scene) -> usize {
        let mut count = 0;
        for i in 0..scene.edges.len() {
            for j in (i + 1)..scene.edges.len() {
                let left_edge = &scene.edges[i];
                let right_edge = &scene.edges[j];
                if [left_edge.source, left_edge.target].iter().any(|endpoint| {
                    *endpoint == right_edge.source || *endpoint == right_edge.target
                }) {
                    continue;
                }
                for left in scene.edges[i].points.windows(2) {
                    for right in scene.edges[j].points.windows(2) {
                        count += usize::from(proper_segment_crossing(
                            left[0], left[1], right[0], right[1],
                        ));
                    }
                }
            }
        }
        count
    }

    #[test]
    fn real_editor_workflow_has_at_most_four_route_crossings() {
        let scene = editor_workflow_scene();
        let crossings = route_crossings(&scene);
        assert!(crossings <= 4, "Editor Workflows has {crossings} crossings");
    }

    #[test]
    fn real_editor_workflow_routes_avoid_every_heading_strip() {
        fn cuts(rect: Rect, a: (f64, f64), b: (f64, f64)) -> bool {
            let min_x = a.0.min(b.0);
            let max_x = a.0.max(b.0);
            let min_y = a.1.min(b.1);
            let max_y = a.1.max(b.1);
            min_x < rect.x + rect.w && max_x > rect.x && min_y < rect.y + rect.h && max_y > rect.y
        }
        let scene = editor_workflow_scene();
        for edge in &scene.edges {
            for segment in edge.points.windows(2) {
                for group in &scene.use_case_groups {
                    if group.role == waml::model::DiagramGroupRole::ExternalActors {
                        continue;
                    }
                    assert!(
                        !cuts(group.heading_bounds, segment[0], segment[1]),
                        "edge {:?}->{:?} crosses heading {:?}",
                        edge.source,
                        edge.target,
                        group.title
                    );
                }
            }
        }
    }

    #[test]
    fn real_editor_workflow_routes_are_orthogonal() {
        let scene = editor_workflow_scene();
        for edge in &scene.edges {
            for segment in edge.points.windows(2) {
                assert!(
                    (segment[0].0 - segment[1].0).abs() < 1e-6
                        || (segment[0].1 - segment[1].1).abs() < 1e-6,
                    "diagonal route segment: {:?}",
                    segment
                );
            }
        }
    }

    #[test]
    fn every_real_use_case_document_has_bounded_orthogonal_routes() {
        for title in [
            "Editor Workflows",
            "Browser and Publishing Workflows",
            "Tooling Workflows",
        ] {
            let scene = real_use_case_scene(title);
            for edge in &scene.edges {
                assert!(
                    edge.points.len() <= 32,
                    "{title} route has {} points: {:?}",
                    edge.points.len(),
                    edge.points
                );
                for segment in edge.points.windows(2) {
                    assert!(
                        (segment[0].0 - segment[1].0).abs() < 1e-6
                            || (segment[0].1 - segment[1].1).abs() < 1e-6,
                        "{title} diagonal route segment: {segment:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn actor_fanout_terminal_segments_are_orthogonal_and_distinct() {
        let scene = editor_workflow_scene();
        for actor in ["author", "reader"] {
            let actor_rect = scene
                .nodes
                .iter()
                .find(|node| node.key.ends_with(actor))
                .map(|node| node.rect)
                .unwrap();
            let mut ports = Vec::new();
            for edge in &scene.edges {
                let endpoint = if edge_endpoint_key(&scene, edge, true)
                    .is_some_and(|key| key.ends_with(actor))
                {
                    Some((edge.points[0], edge.points[1]))
                } else if edge_endpoint_key(&scene, edge, false)
                    .is_some_and(|key| key.ends_with(actor))
                {
                    let last = edge.points.len() - 1;
                    Some((edge.points[last], edge.points[last - 1]))
                } else {
                    None
                };
                if let Some((port, neighbour)) = endpoint {
                    assert!(
                        (port.0 - neighbour.0).abs() < 0.0001
                            || (port.1 - neighbour.1).abs() < 0.0001,
                        "{actor} route leaves diagonally: {port:?} -> {neighbour:?}"
                    );
                    assert!(
                        neighbour.0 >= actor_rect.x + actor_rect.w / 2.0,
                        "{actor} route leaves away from the system boundary: {port:?} -> {neighbour:?}"
                    );
                    ports.push(port);
                }
            }
            ports.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
            ports.dedup();
            assert!(ports.len() >= 4, "{actor} fanout collapsed to {ports:?}");
        }
    }

    fn edge_endpoint_key<'a>(scene: &'a Scene, edge: &SceneEdge, source: bool) -> Option<&'a str> {
        let rect = if source { edge.source } else { edge.target };
        scene
            .nodes
            .iter()
            .find(|node| node.rect == rect)
            .map(|node| node.key.as_str())
    }

    #[test]
    fn real_editor_workflow_geometry_is_stable_across_repeated_solves() {
        let first = editor_workflow_scene();
        for _ in 0..5 {
            let next = editor_workflow_scene();
            assert_eq!(next.nodes, first.nodes);
            assert_eq!(next.edges, first.edges);
            assert_eq!(next.labels, first.labels);
        }
    }

    #[test]
    fn real_editor_workflow_scene_preserves_nested_band_roles_and_titles() {
        let overlaps = |a: Rect, b: Rect| {
            a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y
        };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/waml");
        let model = crate::load::load_model(&root).unwrap();
        let diagram = model
            .diagrams
            .iter()
            .find(|diagram| diagram.title == "Editor Workflows")
            .unwrap();
        let (scene, _) = build_scene(
            &model,
            diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        let groups: Vec<_> = scene
            .use_case_groups
            .iter()
            .map(|group| (group.role, group.title.as_deref()))
            .collect();
        assert_eq!(
            groups,
            vec![
                (
                    waml::model::DiagramGroupRole::ExternalActors,
                    Some("External actors")
                ),
                (
                    waml::model::DiagramGroupRole::Band,
                    Some("Create and change")
                ),
                (
                    waml::model::DiagramGroupRole::Band,
                    Some("Find and understand")
                ),
                (
                    waml::model::DiagramGroupRole::SystemBoundary,
                    Some("WAML editor boundary")
                ),
            ]
        );
        let boundary = scene.use_case_groups.last().unwrap();
        let first_band = &scene.use_case_groups[1];
        assert_eq!(boundary.heading_bounds.y, boundary.bounds.y);
        assert_eq!(first_band.heading_bounds.y, first_band.bounds.y);
        assert_ne!(boundary.heading_bounds.y, first_band.heading_bounds.y);
        for group in &scene.use_case_groups[1..] {
            for node in &scene.nodes {
                assert!(!overlaps(group.heading_bounds, node.rect));
            }
        }
    }
    use crate::load;
    use std::path::Path;

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    fn groups() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/groups");
        load::load_model(&dir).unwrap()
    }

    fn sixkind() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sixkind");
        load::load_model(&dir).unwrap()
    }

    fn test_display() -> ResolvedDiagramDisplay {
        ResolvedDiagramDisplay::default()
    }

    /// The whole label-placement feature reaches the editor only through
    /// `build_scene`, and `place_labels_with_reroute` silently degrades to a
    /// plain pass when the scene's edge list and the router's route list
    /// desync. Unit tests in `waml` cover the placer on synthetic scenes; this
    /// covers the wiring on real fixtures, with every text toggle on so the
    /// terminal role/multiplicity labels are actually requested.
    #[test]
    fn placed_labels_clear_the_cards_on_real_fixtures() {
        use waml::model::CardinalityVisibility;

        // `groups` is deliberately absent: that fixture declares no
        // relationships, so it has no edges to label.
        for (name, model) in [("mini", mini()), ("sixkind", sixkind())] {
            let mut display = test_display();
            display.show_cardinality = true;
            display.cardinality = CardinalityVisibility::All;
            display.show_roles = true;
            display.show_labels = true;
            let (scene, _) = build_scene(
                &model,
                &model.diagrams[0],
                display,
                &std::collections::HashSet::new(),
            );
            assert!(
                !scene.labels.is_empty(),
                "{name}: no edge labels were placed at all — the wiring is dead"
            );
            for label in &scene.labels {
                for node in &scene.nodes {
                    let clear = label.rect.x + label.rect.w <= node.rect.x
                        || node.rect.x + node.rect.w <= label.rect.x
                        || label.rect.y + label.rect.h <= node.rect.y
                        || node.rect.y + node.rect.h <= label.rect.y;
                    assert!(
                        clear,
                        "{name}: label {:?} at {:?} sits on card {} at {:?}",
                        label.text, label.rect, node.key, node.rect
                    );
                }
            }
        }
    }

    #[test]
    fn attribute_cardinality_respects_all_three_modes() {
        use waml::model::CardinalityVisibility::{All, Explicit, Off};
        use waml::multiplicity::Multiplicity;

        let implicit = None;
        let explicit_one = Multiplicity::parse("1");
        let explicit_many = Multiplicity::parse("0..*");

        assert_eq!(attribute_cardinality_text(implicit.as_ref(), Off), None);
        assert_eq!(attribute_cardinality_text(explicit_one.as_ref(), Off), None);
        assert_eq!(
            attribute_cardinality_text(implicit.as_ref(), Explicit),
            None
        );
        assert_eq!(
            attribute_cardinality_text(explicit_one.as_ref(), Explicit),
            Some("{1}".into())
        );
        assert_eq!(
            attribute_cardinality_text(explicit_many.as_ref(), Explicit),
            Some("{0..*}".into())
        );
        assert_eq!(
            attribute_cardinality_text(implicit.as_ref(), All),
            Some("{1}".into())
        );
    }

    #[test]
    fn display_hides_attributes_before_card_measurement() {
        let model = mini();
        let (full, _) = build_scene(
            &model,
            &model.diagrams[0],
            test_display(),
            &std::collections::HashSet::new(),
        );
        let mut display = test_display();
        display.show_attributes = false;
        let (hidden, _) = build_scene(
            &model,
            &model.diagrams[0],
            display,
            &std::collections::HashSet::new(),
        );
        let full_order = full.nodes.iter().find(|node| node.key == "order").unwrap();
        let hidden_order = hidden
            .nodes
            .iter()
            .find(|node| node.key == "order")
            .unwrap();

        assert!(hidden_order.attributes.is_empty());
        assert!(
            crate::card::card_size(hidden_order, &crate::card::mono_sheet()).1
                < crate::card::card_size(full_order, &crate::card::mono_sheet()).1
        );
        assert!(hidden_order.rect.h < full_order.rect.h);
    }

    #[test]
    fn display_projects_attribute_columns_and_cap_before_measurement() {
        let model = mini();
        let mut display = test_display();
        display.show_type = false;
        display.show_attribute_visibility = false;
        display.cardinality = CardinalityVisibility::Off;
        display.max_attributes = Some(1);
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            display,
            &std::collections::HashSet::new(),
        );
        let order = scene.nodes.iter().find(|node| node.key == "order").unwrap();

        assert_eq!(order.attributes.len(), 1);
        assert_eq!(order.attributes[0].name, "id");
        assert!(order.attributes[0].ty.is_empty());
        assert!(order.attributes[0].visibility.is_empty());
        assert!(order.attributes[0].multiplicity.is_empty());
    }

    #[test]
    fn display_filters_stereotypes_before_card_eyebrow() {
        let model = mini();
        let mut display = test_display();
        display.stereotype_filter = Some(Vec::new());
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            display,
            &std::collections::HashSet::new(),
        );
        let order = scene.nodes.iter().find(|node| node.key == "order").unwrap();
        let text: Vec<_> =
            crate::card::measure(&crate::card::class_shape(order, &crate::card::mono_sheet()))
                .texts
                .into_iter()
                .map(|text| text.text)
                .collect();

        assert!(order.stereotypes.is_empty());
        assert!(!text.contains(&"«AGGREGATEROOT»".to_string()));
    }

    #[test]
    fn display_hides_interface_metaclass_eyebrow() {
        let model = sixkind();
        let mut display = test_display();
        display.show_stereotype = false;
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            display,
            &std::collections::HashSet::new(),
        );
        let interface = scene
            .nodes
            .iter()
            .find(|node| node.key == "drivable")
            .unwrap();
        let text: Vec<_> = crate::card::measure(&crate::card::class_shape(
            interface,
            &crate::card::mono_sheet(),
        ))
        .texts
        .into_iter()
        .map(|text| text.text)
        .collect();

        assert!(!text.contains(&"«INTERFACE»".to_string()));
    }

    /// The `groups` fixture is what the canvas's group-render gating is judged
    /// against: one `frame` group (draws chrome) and one default-shape group
    /// (layout-only, invisible unless the hidden-borders x-ray is on). Pin both
    /// the shapes and the clean solve here, or the gating's whole premise can
    /// rot silently.
    #[test]
    fn groups_fixture_solves_a_frame_and_a_default_group() {
        use waml::diagnostic::DiagCode;
        use waml::layout::Shape;
        let model = groups();
        let (scene, diags) = build_scene(
            &model,
            &model.diagrams[0],
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert!(
            !diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
            "groups fixture must load conflict-free: {diags:?}"
        );
        assert!(
            diags.is_empty(),
            "groups fixture must load clean: {diags:?}"
        );
        assert!(
            scene.conflicts.is_empty(),
            "groups fixture must drop no placement: {:?}",
            scene.conflicts
        );
        let by_title = |t: &str| {
            scene
                .groups
                .iter()
                .find(|g| g.title.as_deref() == Some(t))
                .unwrap_or_else(|| panic!("no `{t}` group in {:?}", scene.groups))
        };
        assert_eq!(scene.groups.len(), 2, "{:?}", scene.groups);
        assert_eq!(
            by_title("Users").shape,
            Shape::Frame,
            "`with frame` must resolve to the chrome-drawing shape"
        );
        assert_eq!(
            by_title("Billing").shape,
            Shape::Shrink,
            "a shapeless group must resolve to the layout-only default"
        );
    }

    #[test]
    fn scene_projects_existing_placement_relations() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            test_display(),
            &std::collections::HashSet::new(),
        );
        use waml::layout::Direction;
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
    fn clean_diagram_has_no_conflicts() {
        // mini's default layout is satisfiable, so the solver drops nothing and the
        // scene carries an empty conflict report.
        let model = mini();
        let (scene, diags) = build_scene(
            &model,
            &model.diagrams[0],
            test_display(),
            &std::collections::HashSet::new(),
        );
        use waml::diagnostic::DiagCode;
        assert!(
            !diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
            "mini must be conflict-free: {diags:?}"
        );
        assert!(
            scene.conflicts.is_empty(),
            "clean diagram must report no conflicts: {:?}",
            scene.conflicts
        );
    }

    #[test]
    fn compile_dropped_alignment_surfaces_a_layout_conflict_diag() {
        use waml::diagnostic::DiagCode;
        use waml::layout::{Anchored, Edge, LayoutStatement, NameRef, Operand, OperandRef};
        // `align Order.top, Customer.left` shares no axis: constrain::compile
        // records it as a compile-time drop, so it never reaches the solver.
        // The replaced geometry path warned ("alignment edges share no axis");
        // the unified path must not swallow it silently.
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        let anchored = |slug: &str, edge| Anchored {
            edge: Some(edge),
            operand: Operand {
                ref_: OperandRef::Name(NameRef::Link {
                    title: title_for(&model, slug),
                    slug: slug.to_string(),
                }),
                axis: None,
                hints: Vec::new(),
            },
        };
        diagram.layout.push(LayoutStatement::Alignment {
            left: anchored("order", Edge::Top),
            right: anchored("customer", Edge::Left),
        });

        let (_, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert!(
            diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
            "a compile-dropped alignment must surface a LayoutConflict diagnostic: {diags:?}"
        );
    }

    #[test]
    fn contradiction_surfaces_in_scene_conflicts() {
        use waml::layout::{Direction, LayoutStatement, NameRef, Operand, OperandRef};
        // mini authors `Order left of Customer`. Add the reversed pair
        // `Customer left of Order`: a different ordered pair, so both coexist and the
        // solver cannot satisfy them. The dropped placement + its contradiction set
        // surface in scene.conflicts (NO canvas red, NO leave-one-out).
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        let link = |slug: &str| Operand {
            ref_: OperandRef::Name(NameRef::Link {
                title: title_for(&model, slug),
                slug: slug.to_string(),
            }),
            axis: None,
            hints: Vec::new(),
        };
        diagram.layout.push(LayoutStatement::Placement {
            operands: vec![link("customer"), link("order")],
            directions: vec![Direction::LeftOf],
        });

        let (scene, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        use waml::diagnostic::DiagCode;
        assert!(
            diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
            "must be genuinely contradictory: {diags:?}"
        );
        assert!(
            !scene.conflicts.is_empty(),
            "contradiction must surface in scene.conflicts"
        );
        // Every reported conflict names a real projected relation and a non-empty
        // contradiction set; the independent `payment-gateway below order` never appears.
        for c in &scene.conflicts {
            assert!(
                !c.conflicts_with.is_empty(),
                "a dropped relation must list what it conflicts with"
            );
            let touches = |r: &SceneRelation| {
                (r.subject == "order" && r.reference == "customer")
                    || (r.subject == "customer" && r.reference == "order")
            };
            assert!(
                touches(&c.dropped),
                "dropped relation should be one of the reversed pair: {:?}",
                c.dropped
            );
            assert!(
                c.conflicts_with.iter().any(touches),
                "conflict set should include the opposing placement"
            );
            assert!(
                !(c.dropped.subject == "payment-gateway"),
                "the independent placement must not be reported as dropped"
            );
        }
    }

    #[test]
    fn oracle_flags_a_contradictory_placement() {
        use waml::layout::{Direction, LayoutStatement, NameRef, Operand, OperandRef};
        // A genuine TRANSITIVE contradiction the override rule can't dissolve.
        // Seed a horizontal chain: mini already has `order left of customer`; add
        // `customer left of payment-gateway`. Dragging `payment-gateway left of
        // order` closes an unsatisfiable cycle (order < customer <
        // payment-gateway < order). The dragged pair has no prior relation to
        // replace, so the cycle stands and the solver conflicts.
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        let link = |slug: &str| Operand {
            ref_: OperandRef::Name(NameRef::Link {
                title: title_for(&model, slug),
                slug: slug.to_string(),
            }),
            axis: None,
            hints: Vec::new(),
        };
        diagram.layout.push(LayoutStatement::Placement {
            operands: vec![link("customer"), link("payment-gateway")],
            directions: vec![Direction::LeftOf],
        });
        assert!(
            placement_would_conflict(
                &model,
                &diagram,
                "payment-gateway",
                "order",
                Direction::LeftOf,
                &std::collections::HashSet::new(),
            ),
            "a transitive cycle the override can't dissolve must be predicted conflicting"
        );
    }

    #[test]
    fn oracle_accepts_a_reversed_pair_via_override() {
        // mini has `order left of customer`. Dragging the REVERSED pair `customer
        // left of order` REPLACES it (one relation per UNORDERED pair), so the
        // scratch holds only the new relation -- satisfiable, no conflict. The
        // drag oracle must mirror the write-back's override, not predict a
        // phantom conflict against the line it is about to overwrite.
        let model = mini();
        let diagram = &model.diagrams[0];
        assert!(
            !placement_would_conflict(
                &model,
                diagram,
                "customer",
                "order",
                waml::layout::Direction::LeftOf,
                &std::collections::HashSet::new(),
            ),
            "a reversed-pair re-drag overrides, so it must NOT be predicted conflicting"
        );
    }

    #[test]
    fn nested_candidate_authors_relative_hrefs_and_preserves_unrelated_spelling() {
        use waml::layout::{Direction, LayoutStatement, NameRef, Operand, OperandRef};

        let link = |title: &str, slug: &str| Operand {
            ref_: OperandRef::Name(NameRef::Link {
                title: title.into(),
                slug: slug.into(),
            }),
            axis: None,
            hints: Vec::new(),
        };
        let diagram = Diagram {
            key: "views/orders".into(),
            title: "Orders".into(),
            kind: waml::model::DiagramKind::Class,
            profile: "uml-domain".into(),
            description: None,
            groups: vec![],
            layout: vec![
                LayoutStatement::Standalone(link("Audit", "../shared/audit.md?mode=compact#card")),
                LayoutStatement::Placement {
                    operands: vec![
                        link("Order", "../domain/order.md"),
                        link("Customer", "../shared/customer.md"),
                    ],
                    directions: vec![Direction::LeftOf],
                },
            ],
            display: Default::default(),
        };

        let candidate = placement_candidate(
            &Model::default(),
            &diagram,
            "domain/order",
            "shared/customer",
            Direction::AboveLeft,
        );

        assert_eq!(candidate.layout.len(), 2);
        assert!(matches!(
            &candidate.layout[0],
            LayoutStatement::Standalone(Operand {
                ref_: OperandRef::Name(NameRef::Link { slug, .. }),
                ..
            }) if slug == "../shared/audit.md?mode=compact#card"
        ));
        let LayoutStatement::Placement {
            operands,
            directions,
        } = &candidate.layout[1]
        else {
            panic!("replacement placement");
        };
        assert_eq!(directions, &[Direction::AboveLeft]);
        assert!(matches!(
            &operands[0].ref_,
            OperandRef::Name(NameRef::Link { title, slug })
                if title == "domain/order" && slug == "../domain/order.md"
        ));
        assert!(matches!(
            &operands[1].ref_,
            OperandRef::Name(NameRef::Link { title, slug })
                if title == "shared/customer" && slug == "../shared/customer.md"
        ));
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
                waml::layout::Direction::AboveLeft,
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
            test_display(),
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
            test_display(),
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
            test_display(),
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
            test_display(),
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
            test_display(),
            &std::collections::HashSet::new(),
        );
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        // "- [Order] left of [Customer]" => order's right edge is left of customer's left edge.
        assert!(order.rect.x + order.rect.w <= customer.rect.x);
    }

    #[test]
    fn a_diagram_with_hints_no_longer_strip_packs_unrelated_nodes() {
        use waml::layout::{Direction, LayoutStatement, NameRef, Operand, OperandRef};
        // The screenshot regression that motivated the unification: one hint
        // between two nodes that a connected pair has nothing to do with. The
        // old dispatch flipped the WHOLE diagram onto the rigid-offset strip
        // packer: `car` and `driver` (edge-connected, unhinted) landed in
        // separate rigid components and were packed a full strip apart. The
        // unified path keeps the connected pair at stress distance while the
        // hint itself still holds.
        let model = sixkind();
        let mut diagram = model.diagrams[0].clone();
        let link = |slug: &str| Operand {
            ref_: OperandRef::Name(NameRef::Link {
                title: title_for(&model, slug),
                slug: slug.to_string(),
            }),
            axis: None,
            hints: Vec::new(),
        };
        diagram.layout = vec![LayoutStatement::Placement {
            operands: vec![link("wheel"), link("engine")],
            directions: vec![Direction::LeftOf],
        }];
        let (scene, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        use waml::diagnostic::DiagCode;
        assert!(
            !diags.iter().any(|d| d.code == DiagCode::LayoutConflict),
            "a single hint is trivially satisfiable: {diags:?}"
        );
        let rect_of = |k: &str| scene.nodes.iter().find(|n| n.key == k).unwrap().rect;
        let (wheel, engine) = (rect_of("wheel"), rect_of("engine"));
        assert!(
            wheel.x + wheel.w <= engine.x,
            "the authored hint itself must hold: wheel {wheel:?} engine {engine:?}"
        );
        let center = |r: Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let (car, driver) = (rect_of("car"), rect_of("driver"));
        let (cx, cy) = center(car);
        let (dx, dy) = center(driver);
        let dist = (cx - dx).hypot(cy - dy);
        // Stress targets one hop at edge_len inflated by the pair's half
        // extents; twice that is comfortably "adjacent", while the old strip
        // packer put four whole strips between them.
        let half_extent = |r: Rect| (r.w + r.h) / 4.0;
        let bound = 2.0
            * (waml::solve::stress::StressConfig::default().edge_len
                + half_extent(car)
                + half_extent(driver));
        assert!(
            dist < bound,
            "connected pair flung apart: |car-driver| = {dist}, bound {bound}"
        );
    }

    #[test]
    fn collapsed_flag_survives_the_unified_path() {
        use waml::layout::{Direction, Flag, Hint, LayoutStatement, NameRef, Operand, OperandRef};
        // stress_layout's flags/chip-size wiring at the editor level: a
        // collapsed node on a hint-carrying diagram must come back with
        // collapsed=true AND solve at chip size. This was silently dead on
        // the old stress path (flags: BTreeMap::new()).
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        let link = |slug: &str, hints: Vec<Hint>| Operand {
            ref_: OperandRef::Name(NameRef::Link {
                title: title_for(&model, slug),
                slug: slug.to_string(),
            }),
            axis: None,
            hints,
        };
        diagram.layout = vec![LayoutStatement::Placement {
            operands: vec![
                link("order", vec![Hint::Flag(Flag::Collapsed)]),
                link("customer", vec![]),
            ],
            directions: vec![Direction::LeftOf],
        }];
        let (scene, _) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        assert!(order.collapsed, "collapsed flag must survive the solve");
        assert!(!customer.collapsed);
        let chip = SolveConfig::default().chip;
        assert_eq!(
            (order.rect.w, order.rect.h),
            (chip.w, chip.h),
            "collapsed node must solve at chip size"
        );
        assert!(
            customer.rect.w > chip.w,
            "an uncollapsed card keeps its measured size"
        );
    }

    #[test]
    fn bounding_box_covers_all_nodes() {
        let model = mini();
        let (scene, _) = build_scene(
            &model,
            &model.diagrams[0],
            test_display(),
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
            use_case_groups: Vec::new(),
            visual_kind: Default::default(),
            display: test_display(),
            nodes: vec![],
            groups: vec![],
            edges: vec![],
            relations: vec![],
            conflicts: vec![],
            labels: vec![],
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
        let (scene, _) = build_scene(&model, &model.diagrams[0], test_display(), &expanded);
        let order = scene.nodes.iter().find(|n| n.key == "order").unwrap();
        let customer = scene.nodes.iter().find(|n| n.key == "customer").unwrap();
        assert!(order.expanded, "order was in the expanded set");
        assert!(!customer.expanded, "customer was not");
    }

    #[test]
    fn stress_layout_populates_routes() {
        let model = mini();
        let diagram = &model.diagrams[0];
        let sizes = crate::sizing::size_map(&model, diagram, &std::collections::HashSet::new());
        let model_edges = drawable_edges(&model);
        let edges: Vec<(BoxId, BoxId)> = model_edges
            .iter()
            .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
            .collect();
        let (scene, _) = resolve::resolve(diagram);
        let connected = connected_pairs(&edges);
        let compiled = constrain::compile(
            &scene,
            &sizes,
            &std::collections::BTreeMap::new(),
            &connected,
            &SolveConfig::default(),
        );
        let (solved, ..) = stress_layout(diagram, &compiled, &sizes, &model_edges);
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
    fn stress_routing_avoids_a_foreign_group_hull() {
        // Task 5's containment-aware routing: each group's hull is handed to
        // the router as an obstacle (plus a `Box` per group for membership), so
        // an edge between two nodes that belong to NO group is deflected around
        // a cluster instead of cutting straight through it.
        //
        // Geometry is hand-placed: `left` and `right` straddle a two-member
        // cluster whose hull sits squarely on the straight line between them.
        let keys: Vec<String> = ["left", "right", "c1", "c2"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let rects = vec![
            Rect {
                x: 0.0,
                y: 200.0,
                w: 80.0,
                h: 50.0,
            },
            Rect {
                x: 700.0,
                y: 200.0,
                w: 80.0,
                h: 50.0,
            },
            Rect {
                x: 300.0,
                y: 0.0,
                w: 120.0,
                h: 80.0,
            },
            Rect {
                x: 300.0,
                y: 400.0,
                w: 120.0,
                h: 80.0,
            },
        ];
        // The members leave a wide vertical gap the router would happily thread
        // straight through; only the hull closes it.
        let hull = Rect {
            x: 284.0,
            y: -16.0,
            w: 152.0,
            h: 512.0,
        };
        let specs = vec![stress::GroupSpec {
            members: vec![2, 3],
            depth: 0,
        }];
        let route_edges = vec![(
            BoxId::Node("left".to_string()),
            BoxId::Node("right".to_string()),
        )];

        let (routes, rect_map, boxes) = route_with_groups(
            &keys,
            &rects,
            std::slice::from_ref(&hull),
            &specs,
            &[0],
            &route_edges,
            &SceneRoutePolicy {
                hard_obstacles: &[],
                cost: &route::RouteCost::default(),
            },
        );

        assert!(
            rect_map.contains_key(&BoxId::Group(0)),
            "the hull must reach the router as an obstacle"
        );
        assert_eq!(boxes.len(), 1, "one Box per group for membership");
        let route = routes.first().expect("left -> right must be routed");

        // No segment may cross the cluster hull's interior.
        fn crosses(a: (f64, f64), b: (f64, f64), r: Rect) -> bool {
            let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
            let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
            x1 > r.x + 1e-6 && r.x + r.w > x0 + 1e-6 && y1 > r.y + 1e-6 && r.y + r.h > y0 + 1e-6
        }
        for pair in route.points.windows(2) {
            assert!(
                !crosses(pair[0], pair[1], hull),
                "segment {:?}->{:?} cuts through the cluster hull {hull:?} (route {:?})",
                pair[0],
                pair[1],
                route.points
            );
        }
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
            test_display(),
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
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert_eq!(
            scene.edges.len(),
            1,
            "self-loop must not produce a drawable scene edge"
        );
    }

    #[test]
    fn unified_layout_scene_edges_carry_points() {
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();

        let (scene, _) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert_eq!(scene.edges.len(), 1, "mini has one drawable edge");
        assert!(
            !scene.edges[0].points.is_empty(),
            "stress-default edges must carry a routed polyline"
        );
    }

    #[test]
    fn unified_layout_ignores_the_implicit_unnamed_member_group() {
        // A diagram with a flat `## Members` list still parses into ONE
        // unnamed, childless `DiagramGroup` holding every member. It is not an
        // authored cluster: an ungrouped diagram must keep the plain stress
        // layout — no whole-diagram cohesion, no whole-canvas hull.
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        assert!(
            diagram.groups.iter().any(|g| g.name.is_empty()),
            "fixture must carry the implicit unnamed group, or this test is vacuous"
        );

        let (scene, _) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert!(
            scene.groups.is_empty(),
            "ungrouped diagram grew a phantom hull: {:?}",
            scene.groups
        );

        // ...and it contributes no COHESION either. `stress_layout` builds
        // its cohesion-group list directly off `compiled.group_specs` (+
        // `inline_specs`) -- the same field the hull-emptiness assert above
        // already pins -- so there is no separate code path left for the
        // trivial wrapper to sneak a soft cohesion force through even though
        // it draws no hull. (A rect-equality comparison against a truly
        // groupless diagram isn't meaningful here: `resolve::add_group` is
        // the ONLY thing that ever creates a leaf box, so clearing
        // `diagram.groups` entirely also removes every leaf box and changes
        // which order the nodes solve in -- a confound, not evidence.)
        let (scene_boxes, _) = resolve::resolve(&diagram);
        let compiled = constrain::compile(
            &scene_boxes,
            &crate::sizing::size_map(&model, &diagram, &std::collections::HashSet::new()),
            &std::collections::BTreeMap::new(),
            &std::collections::BTreeSet::new(),
            &SolveConfig::default(),
        );
        assert!(
            compiled.group_specs.is_empty() && compiled.inline_specs.is_empty(),
            "the implicit wrapper must not compile to a cohesion group: {:?} / {:?}",
            compiled.group_specs,
            compiled.inline_specs
        );
    }

    #[test]
    fn unified_layout_emits_group_hull_from_diagram_groups() {
        let model = mini();
        // A group with no `with frame` layout statement still emits a hull
        // for containment-aware routing / the show-hidden overlay (it just
        // renders as `GroupDraw::Skip` normally).
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        diagram.groups = vec![DiagramGroup {
            name: "Ordering".into(),
            role: waml::model::DiagramGroupRole::Generic,
            members: vec!["order".into(), "customer".into()],
            children: Vec::new(),
        }];

        let (scene, _) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert_eq!(scene.groups.len(), 1);
        let hull = scene.groups[0].rect;
        assert_eq!(scene.groups[0].title.as_deref(), Some("Ordering"));
        assert_eq!(scene.groups[0].depth, 0);
        assert_eq!(scene.groups[0].shape, Shape::Shrink);
        for key in ["order", "customer"] {
            let node = scene.nodes.iter().find(|n| n.key == key).unwrap();
            let r = node.rect;
            assert!(
                r.x >= hull.x - 1e-6
                    && r.y >= hull.y - 1e-6
                    && r.x + r.w <= hull.x + hull.w + 1e-6
                    && r.y + r.h <= hull.y + hull.h + 1e-6,
                "{key} rect not inside the emitted hull"
            );
        }
    }

    #[test]
    fn unified_layout_warns_about_entangled_groups() {
        // The same element under two `###` headings: WAML permits it, but the
        // separation pass has to skip such a pair (translating one drags the
        // shared member out of the other), so their hulls stay tangled. That
        // must surface as a warning, not silently ship overlapping clusters.
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        diagram.groups = vec![
            DiagramGroup {
                name: "Ordering".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["order".into(), "customer".into()],
                children: Vec::new(),
            },
            DiagramGroup {
                name: "Payments".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["customer".into(), "payment-gateway".into()],
                children: Vec::new(),
            },
        ];

        let (_scene, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        let warning = diags
            .iter()
            .find(|d| d.code == waml::diagnostic::DiagCode::EntangledGroups)
            .expect("entangled sibling groups must be reported");
        assert_eq!(warning.severity, waml::diagnostic::Severity::Warning);
        assert!(
            warning.message.contains("Ordering") && warning.message.contains("Payments"),
            "message must name both groups: {}",
            warning.message
        );

        // A nested pair (one group inside the other) is expected containment,
        // not entanglement: no warning.
        let mut nested = diagram.clone();
        nested.groups = vec![DiagramGroup {
            name: "Ordering".into(),
            role: waml::model::DiagramGroupRole::Generic,
            members: vec!["order".into(), "customer".into()],
            children: vec![DiagramGroup {
                name: "Inner".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["customer".into()],
                children: Vec::new(),
            }],
        }];
        let (_scene, diags) = build_scene(
            &model,
            &nested,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert!(
            !diags
                .iter()
                .any(|d| d.code == waml::diagnostic::DiagCode::EntangledGroups),
            "nesting must not be reported as entanglement: {diags:?}"
        );
    }

    #[test]
    fn a_subset_sibling_pair_is_entangled_not_nested() {
        // `Small`'s members are a strict SUBSET of `Big`'s, but the two are
        // SIBLINGS — neither is declared inside the other. The separation pass
        // skips any pair sharing a member, so these hulls ship tangled; only
        // real structural nesting is exempt. Treating "one set contains the
        // other" as nesting is what let this case ship silently.
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        diagram.groups = vec![
            DiagramGroup {
                name: "Big".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["order".into(), "customer".into()],
                children: Vec::new(),
            },
            DiagramGroup {
                name: "Small".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["customer".into()],
                children: Vec::new(),
            },
        ];

        let (_scene, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        let warning = diags
            .iter()
            .find(|d| d.code == waml::diagnostic::DiagCode::EntangledGroups)
            .expect("a subset sibling pair is still entangled");
        assert!(
            warning.message.contains("Big") && warning.message.contains("Small"),
            "message must name both groups: {}",
            warning.message
        );
    }

    #[test]
    fn an_unresolved_shared_member_is_not_entanglement() {
        // The only key the two groups share resolves to no sized node, so it has
        // no geometry and cannot tangle anything. The warning must run on
        // resolved indices, not on member names, or this is a false positive.
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        diagram.groups = vec![
            DiagramGroup {
                name: "Left".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["order".into(), "ghost".into()],
                children: Vec::new(),
            },
            DiagramGroup {
                name: "Right".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["customer".into(), "ghost".into()],
                children: Vec::new(),
            },
        ];

        let (_scene, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert!(
            !diags
                .iter()
                .any(|d| d.code == waml::diagnostic::DiagCode::EntangledGroups),
            "an unresolved shared key must not raise entanglement: {diags:?}"
        );
    }

    #[test]
    fn unified_layout_skips_groups_with_no_resolved_members() {
        // Every member key misses the size map, so the group has no geometry:
        // it must not emit a zero-size hull (a phantom route obstacle at the
        // origin plus a degenerate dashed overlay).
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        diagram.groups = vec![
            DiagramGroup {
                name: "Ghost".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["not-a-node".into()],
                children: Vec::new(),
            },
            DiagramGroup {
                name: "Ordering".into(),
                role: waml::model::DiagramGroupRole::Generic,
                members: vec!["order".into(), "customer".into()],
                children: Vec::new(),
            },
        ];

        let (scene, _) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert_eq!(
            scene.groups.len(),
            1,
            "the memberless group must be dropped"
        );
        assert_eq!(scene.groups[0].title.as_deref(), Some("Ordering"));
        assert!(
            !scene.nodes.iter().any(|n| n.key == "not-a-node"),
            "an unresolvable member must not render a phantom chip-sized node"
        );
    }

    /// The plan's Task 5 regression pin the unification owed: a golden dump
    /// of the unified no-hint editor output, so any future churn to the
    /// hintless solve (node set, solve order, stress tuning, card sizing) is
    /// caught instead of silently re-laying-out every hintless diagram. The
    /// coordinates are the CURRENT unified output — the unification itself
    /// already changed the hintless solve order from sorted-by-key (the old
    /// `sizes.keys()` BTreeMap order) to scene/declaration order
    /// (`compiled.keys`), which was accepted; this pins the result. If a
    /// deliberate layout change trips this, re-pin the dump.
    #[test]
    fn hintless_unified_layout_geometry_is_pinned() {
        let model = mini();
        let mut diagram = model.diagrams[0].clone();
        diagram.layout = Vec::new();
        let (scene, diags) = build_scene(
            &model,
            &diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert!(
            diags.is_empty(),
            "hintless mini must solve clean: {diags:?}"
        );
        let mut dump = String::new();
        for n in &scene.nodes {
            dump.push_str(&format!(
                "{} {:.1} {:.1} {:.1} {:.1}\n",
                n.key, n.rect.x, n.rect.y, n.rect.w, n.rect.h
            ));
        }
        let expected = "customer 0.0 9.7 190.8 98.4\n\
                        order 298.9 0.0 190.8 117.7\n\
                        payment-gateway 0.0 157.7 199.6 92.7\n";
        assert_eq!(
            dump, expected,
            "hintless unified output drifted; if the change is deliberate, re-pin"
        );
    }

    #[test]
    fn groups_linked_fixture_clusters_and_never_overlaps() {
        // Groups + relationships + no `## Layout` section: the one combination
        // previously uncovered anywhere in the tree (see plan Task 6). Members
        // must land inside their own hull and sibling hulls must never overlap.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/groups-linked");
        let model = load::load_model(&dir).unwrap();
        let diagram = &model.diagrams[0];
        assert!(
            diagram.layout.is_empty(),
            "fixture must have no ## Layout section"
        );

        let (scene, _) = build_scene(
            &model,
            diagram,
            test_display(),
            &std::collections::HashSet::new(),
        );
        assert_eq!(
            scene.groups.len(),
            2,
            "expected the Users and Billing hulls"
        );

        fn rect_inside(r: Rect, hull: Rect) -> bool {
            r.x >= hull.x - 1e-6
                && r.y >= hull.y - 1e-6
                && r.x + r.w <= hull.x + hull.w + 1e-6
                && r.y + r.h <= hull.y + hull.h + 1e-6
        }
        fn rects_overlap(a: Rect, b: Rect) -> bool {
            a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
        }

        let users = ["customer", "account"];
        let billing = ["order", "invoice"];
        for (hull, members) in [
            (scene.groups[0].rect, users.as_slice()),
            (scene.groups[1].rect, billing.as_slice()),
        ] {
            for key in members {
                let node = scene.nodes.iter().find(|n| n.key == *key).unwrap();
                assert!(
                    rect_inside(node.rect, hull),
                    "{key} rect not inside its group hull"
                );
            }
        }
        assert!(
            !rects_overlap(scene.groups[0].rect, scene.groups[1].rect),
            "sibling hulls overlap: {:?} vs {:?}",
            scene.groups[0].rect,
            scene.groups[1].rect
        );
    }

    #[test]
    fn conflict_statement_reads_as_dsl() {
        use waml::layout::Direction;
        let c = SceneConflict {
            dropped: SceneRelation {
                subject: "order".into(),
                reference: "customer".into(),
                dir: Direction::LeftOf,
            },
            conflicts_with: vec![SceneRelation {
                subject: "customer".into(),
                reference: "order".into(),
                dir: Direction::LeftOf,
            }],
        };
        let s = conflict_statement(&c);
        assert!(
            s.contains("order left of customer"),
            "dropped statement missing: {s}"
        );
        assert!(
            s.contains("customer left of order"),
            "conflict statement missing: {s}"
        );
        assert!(
            s.to_lowercase().contains("contradict"),
            "missing the 'contradict' note: {s}"
        );
    }

    #[test]
    fn conflict_participants_lists_every_involved_node() {
        use waml::layout::Direction;
        let c = SceneConflict {
            dropped: SceneRelation {
                subject: "order".into(),
                reference: "customer".into(),
                dir: Direction::LeftOf,
            },
            conflicts_with: vec![SceneRelation {
                subject: "customer".into(),
                reference: "order".into(),
                dir: Direction::LeftOf,
            }],
        };
        let mut p = conflict_participants(&c);
        p.sort();
        p.dedup();
        assert_eq!(p, vec!["customer".to_string(), "order".to_string()]);
    }

    /// Task 1 baseline: report node/edge count and both crossing counters,
    /// per fixture, for the stress-default layout. `#[ignore]`d -- this is a
    /// measurement harness run by hand (`cargo test -p waml-editor --
    /// --ignored crossing_baseline_report --nocapture`), not a pass/fail
    /// gate; the numbers it prints belong in the landing commit message, not
    /// in an assertion (there is no "correct" crossing count to pin here).
    ///
    /// Runs each fixture at `group_weight` 4 and 30 (`d56da727` raised the
    /// shipped default from 4 to 30) so the report also answers whether that
    /// raise made crossings worse.
    ///
    /// Task 3 re-measurement, `crossing_passes` 0 -> 8 (route_crossings /
    /// segment_crossings). NOTE: the sweep first recorded in `458478cc` read
    /// "zero delta everywhere" -- but it was taken while an *absolute* hull
    /// guard rejected every candidate and made the pass a silent no-op.
    /// `d8ec2767` fixed that guard (absolute -> relative) and did not re-record.
    /// These are the numbers on the fixed pass:
    ///
    ///   mini / groups / groups-linked / sixkind : no change at either weight
    ///     (these fixtures have 0 segment_crossings to begin with, so the
    ///     hill-climb's objective is already at its floor and it never moves).
    ///   docs/waml domain-model @ group_weight=4 : 8/2 -> 8/1
    ///   docs/waml domain-model @ group_weight=30: 12/4 -> 8/1  <-- the shipped
    ///     default; the pass cancels d56da727's whole cohesion-raise regression
    ///     (route_crossings back to the group_weight=4 level) and beats it on
    ///     the proxy.
    ///
    /// So the pass is NOT a null result: at the shipped default it removes a
    /// third of the route-level crossings on the only corpus view that has
    /// non-trivial ones. Re-run this harness and update these numbers whenever
    /// the pass or the layout defaults change.
    #[test]
    #[ignore = "measurement harness, run by hand and record the numbers"]
    fn crossing_baseline_report() {
        use waml::solve::crossing::{route_crossings, segment_crossings};
        use waml::solve::stress::StressConfig;

        fn report_for(name: &str, model: &Model, group_weight: f64, crossing_passes: u32) {
            for diagram in &model.diagrams {
                // Cleared for continuity with the numbers already recorded in
                // this test's doc comment (taken before the unification, when
                // any authored `## Layout` block routed the whole diagram
                // through the OTHER, edge-blind path). The unified path
                // handles both the same way now, but re-recording the corpus
                // with hints intact is a separate re-measurement.
                let mut diagram = diagram.clone();
                diagram.layout = Vec::new();

                let sizes =
                    crate::sizing::size_map(model, &diagram, &std::collections::HashSet::new());
                if sizes.is_empty() {
                    continue;
                }
                let model_edges = drawable_edges(model);
                let edges: Vec<(BoxId, BoxId)> = model_edges
                    .iter()
                    .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
                    .collect();

                let cfg = StressConfig {
                    group_weight,
                    crossing_passes,
                    ..StressConfig::default()
                };
                let (scene, _) = resolve::resolve(&diagram);
                let connected = connected_pairs(&edges);
                let compiled = constrain::compile(
                    &scene,
                    &sizes,
                    &std::collections::BTreeMap::new(),
                    &connected,
                    &SolveConfig::default(),
                );
                let keys = compiled.keys.clone();
                let index: std::collections::BTreeMap<&str, usize> = keys
                    .iter()
                    .enumerate()
                    .map(|(i, k)| (k.as_str(), i))
                    .collect();
                let ids: Vec<BoxId> = keys.iter().cloned().map(BoxId::Node).collect();
                let dims: Vec<Size> = keys
                    .iter()
                    .map(|k| sizes.get(k).copied().unwrap_or(SolveConfig::default().chip))
                    .collect();

                let mut seen = std::collections::BTreeSet::new();
                let mut pairs: Vec<(usize, usize)> = Vec::new();
                for e in &model_edges {
                    let (Some(&a), Some(&b)) =
                        (index.get(e.source.as_str()), index.get(e.target.as_str()))
                    else {
                        continue;
                    };
                    if a != b && seen.insert((a.min(b), a.max(b))) {
                        pairs.push((a, b));
                    }
                }

                let cohesion_groups: Vec<stress::GroupSpec> = compiled
                    .group_specs
                    .iter()
                    .cloned()
                    .chain(compiled.inline_specs.iter().cloned())
                    .collect();
                let (rects, all_hulls, _) = stress::layout_constrained(
                    &ids,
                    &dims,
                    &pairs,
                    &cohesion_groups,
                    &compiled.seps,
                    &cfg,
                );
                let hulls: Vec<Rect> = all_hulls
                    .into_iter()
                    .take(compiled.group_specs.len())
                    .collect();

                let route_edges: Vec<(BoxId, BoxId)> = model_edges
                    .iter()
                    .map(|e| (BoxId::Node(e.source.clone()), BoxId::Node(e.target.clone())))
                    .collect();
                let depths: Vec<u8> = compiled.group_meta.iter().map(|(_, d, _)| *d).collect();
                let (routes, _rect_map, _boxes) = route_with_groups(
                    &keys,
                    &rects,
                    &hulls,
                    &compiled.group_specs,
                    &depths,
                    &route_edges,
                    &SceneRoutePolicy {
                        hard_obstacles: &[],
                        cost: &route::RouteCost::default(),
                    },
                );

                let centers: Vec<(f64, f64)> = rects
                    .iter()
                    .map(|r| (r.x + r.w / 2.0, r.y + r.h / 2.0))
                    .collect();
                let rc = route_crossings(&routes);
                let sc = segment_crossings(&centers, &pairs);
                println!(
                    "{name} / {} (group_weight={group_weight}, crossing_passes={crossing_passes}): nodes={} edges={} route_crossings={rc} segment_crossings={sc}",
                    diagram.key,
                    keys.len(),
                    pairs.len(),
                );
            }
        }

        for name in ["mini", "groups", "groups-linked", "sixkind"] {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name);
            let model = load::load_model(&dir).expect("fixture loads");
            for group_weight in [4.0, 30.0] {
                for crossing_passes in [0, 8] {
                    report_for(name, &model, group_weight, crossing_passes);
                }
            }
        }

        // The domain-model architecture view, loaded from its own docs/waml
        // corpus (a whole directory, not a standalone fixture).
        let docs_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/waml")
            .canonicalize()
            .expect("docs/waml exists");
        let model = load::load_model(&docs_dir).expect("docs/waml loads");
        let domain_model: Vec<Diagram> = model
            .diagrams
            .iter()
            .filter(|d| d.key.ends_with("domain-model"))
            .cloned()
            .collect();
        assert!(
            !domain_model.is_empty(),
            "docs/waml must define a domain-model diagram"
        );
        let mut narrowed = model.clone();
        narrowed.diagrams = domain_model;
        for group_weight in [4.0, 30.0] {
            for crossing_passes in [0, 8] {
                report_for("docs/waml", &narrowed, group_weight, crossing_passes);
            }
        }
    }
}
