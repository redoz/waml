//! The inspector seam: project a `Model` + a subject into a flat `InspectorView`
//! for the panel. Nothing here touches makepad; the widget lives in
//! `inspector_panel.rs`. Mirrors the `tree.rs` (pure) / `tree_panel.rs` (widget)
//! split.

use waml::model::{DiagramGroup, ElementType, Model, RelationshipKind};

/// What the inspector is currently pointed at. `None` renders the empty state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Subject {
    #[default]
    None,
    Classifier(String),
    /// Group name (diagram-scoped; resolved by name, first match wins).
    Group(String),
    /// Synthetic `"src->tgt"` id (the Edge picker row's key).
    Edge(String),
}

/// An editable inspector field. Overrides are keyed `(subject_key, FieldId)`.
/// UX mock scope A/B: title + description; attribute-row editing is a
/// fast-follow (see `AttrField`, used once attribute rows gain the same
/// inline-edit affordance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldId {
    Title,
    Description,
}

/// One row in the inspector's element-picker dropdown. The picker lists a
/// diagram's whole contents; only `Node` rows actually inspect (Diagram and
/// Edge rows are listed for completeness but selecting them is a no-op for now,
/// pending `Subject::Diagram` / `Subject::Edge` views).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRow {
    /// For `Node`, the classifier key (the `set_subject` target). For `Diagram`,
    /// the diagram key. For `Edge`, a synthetic `"src->tgt"` id (unused while
    /// edge rows are no-ops). Empty for `Placeholder`.
    pub key: String,
    pub label: String,
    pub kind: ElementKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    /// Index-0 sentinel shown when nothing is selected.
    Placeholder,
    Diagram,
    Group,
    Node,
    Edge,
}

/// A navigable reference to one diagram element: enough for the panel to
/// repoint (`key` + `kind`) and to label a card (`label`). Backs both member
/// and association cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRef {
    pub key: String,
    pub kind: ElementKind,
    pub label: String,
}

/// The label of the index-0 sentinel row (shown when nothing is selected).
pub const PICKER_PLACEHOLDER: &str = "Select an element…";

/// One attribute row, pre-rendered to display strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrRow {
    pub name: String,
    pub ty: String,
    pub multiplicity: String,
    pub visibility: String, // "+"/"-"/"#"/"~" or ""
}

/// One operation row, pre-rendered to display strings: `<vis> <name>(<params>) :
/// <ret>`. Mirrors `AttrRow` for the operations compartment. The model has no
/// operations concept today, so `build_view` never emits these; the node design
/// editor populates them directly on `SceneNode`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpRow {
    pub name: String,
    /// `Some(sig)` renders `(sig)` glued to the name (empty `sig` -> `()`);
    /// `None` hides the parameter list entirely (Params column off).
    pub params: Option<String>,
    /// Return-type token; empty omits the ` : ret` tail (Return column off).
    pub ret: String,
    pub visibility: String, // "+"/"-"/"#"/"~" or ""
}

/// Orientation of a relationship from the *subject node's* point of view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssocDir {
    Out, // subject is the edge's source        -> glyph "\u{2192}"
    In,  // subject is the edge's target        -> glyph "\u{2190}"
    Bi,  // both ends navigable / bidirectional -> glyph "\u{2194}"
}

/// One association row, pre-rendered to display strings. Derived from
/// `Model::edges` where `key` is either endpoint -- read-only breadth (U6),
/// not an editable field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocRow {
    pub kind: String,             // RelationshipKind::as_str(), e.g. "associates"
    pub dir: AssocDir,            // orientation from the subject's point of view
    pub other_label: String,      // the far endpoint's title, falling back to its key
    pub role: String,             // far end's role, "" when unset
    pub multiplicity: String,     // far end's multiplicity, "" when unset or trivial "1"
    pub target_key: String,       // the far endpoint's element key (the navigate target)
    pub target_kind: ElementKind, // the far endpoint's kind (Node for associations)
}

/// The flattened read model the panel renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorView {
    pub title: String,
    pub kind_label: String,
    pub abstract_flag: bool,
    pub stereotypes: Vec<String>,
    pub description: Option<String>,
    pub attributes: Vec<AttrRow>,
    /// Group member references; empty for every non-group subject.
    pub members: Vec<ElementRef>,
    pub associations: Vec<AssocRow>,
}

/// Human label for a classifier's element type: `uml.Class` -> `Class`.
fn kind_label(ty: &ElementType) -> String {
    let s = ty.as_str();
    s.strip_prefix("uml.").unwrap_or(&s).to_string()
}

/// Resolve a field's effective value: the override if present, else the
/// model's value. Pure — the widget calls this keyed per `(subject_key,
/// field)`; unit-tested here without any `Cx`.
pub fn effective_field(view: &InspectorView, field: FieldId, over: Option<&String>) -> String {
    if let Some(v) = over {
        return v.clone();
    }
    match field {
        FieldId::Title => view.title.clone(),
        FieldId::Description => view.description.clone().unwrap_or_default(),
    }
}

/// Depth-first (parent, then children) flatten of a group tree into flat picker
/// rows. The implicit top-level group (`name == ""`) is skipped; every named
/// group emits one row keyed/labelled by its name, no indent.
fn push_group_rows(groups: &[DiagramGroup], rows: &mut Vec<ElementRow>) {
    for g in groups {
        if !g.name.is_empty() {
            rows.push(ElementRow {
                key: g.name.clone(),
                label: g.name.clone(),
                kind: ElementKind::Group,
            });
        }
        push_group_rows(&g.children, rows);
    }
}

/// The synthetic picker key for the edge at `edges[idx]`: `"src->tgt"` for the
/// first edge of that ordered pair, `"src->tgt#N"` (N = its 0-based occurrence
/// among same-pair edges) for each later parallel edge. Keeping the first bare
/// preserves the common single-edge case and its readable key. `build_edge_view`
/// reverses it.
fn edge_key(edges: &[waml::model::Edge], idx: usize) -> String {
    let edge = &edges[idx];
    let occ = edges[..idx]
        .iter()
        .filter(|e| e.source == edge.source && e.target == edge.target)
        .count();
    if occ == 0 {
        format!("{}->{}", edge.source, edge.target)
    } else {
        format!("{}->{}#{}", edge.source, edge.target, occ)
    }
}

/// Build the ordered picker rows for a diagram whose drawable node set is
/// `node_keys` (in display order). Row 0 is always the placeholder sentinel;
/// then the diagram title; then each node followed immediately by the edges it
/// is the *source* of (source end), giving a shallow two-level hierarchy. Only
/// edges whose target is also in `node_keys` are listed (an edge to a node
/// outside this diagram isn't drawn, so it isn't part of the diagram either).
///
/// Pure — no `Cx`, unit-tested here. `App` supplies `node_keys` from the built
/// `Scene`; titles are resolved from `model`.
pub fn diagram_elements(
    model: &Model,
    diagram_key: &str,
    diagram_title: &str,
    node_keys: &[String],
) -> Vec<ElementRow> {
    let present: std::collections::HashSet<&str> = node_keys.iter().map(String::as_str).collect();
    let title_of = |k: &str| -> String {
        model
            .nodes
            .iter()
            .find(|n| n.key == k)
            .and_then(|n| n.concept.title.clone())
            .unwrap_or_else(|| k.to_string())
    };

    let mut rows = Vec::with_capacity(node_keys.len() + 2);
    rows.push(ElementRow {
        key: String::new(),
        label: PICKER_PLACEHOLDER.to_string(),
        kind: ElementKind::Placeholder,
    });
    rows.push(ElementRow {
        key: diagram_key.to_string(),
        label: diagram_title.to_string(),
        kind: ElementKind::Diagram,
    });
    // Group rows, flat and depth-first, after the diagram and before the nodes.
    if let Some(diagram) = model.diagrams.iter().find(|d| d.key == diagram_key) {
        push_group_rows(&diagram.groups, &mut rows);
    }
    for nk in node_keys {
        rows.push(ElementRow {
            key: nk.clone(),
            label: title_of(nk),
            kind: ElementKind::Node,
        });
        // Edges anchored at this node's source end, nested right after it. A
        // diagram can hold parallel edges between the same pair (association +
        // dependency etc.), so the synthetic key carries an occurrence ordinal
        // (`src->tgt#N`, N omitted for the first) — keyed on `src->tgt` alone,
        // every parallel row would collapse onto the first edge. `build_edge_view`
        // resolves the ordinal back to the matching edge.
        for (ei, edge) in model.edges.iter().enumerate() {
            if &edge.source == nk && present.contains(edge.target.as_str()) {
                rows.push(ElementRow {
                    key: edge_key(&model.edges, ei),
                    label: format!("{} -> {}", title_of(&edge.source), title_of(&edge.target)),
                    kind: ElementKind::Edge,
                });
            }
        }
    }
    rows
}

/// The picker index for `subject`: 0 (placeholder) for `None` or a key with no
/// matching row of the right kind, else the row whose kind+key matches.
pub fn subject_to_index(rows: &[ElementRow], subject: &Subject) -> usize {
    let (kind, key) = match subject {
        Subject::Classifier(k) => (ElementKind::Node, k),
        Subject::Group(k) => (ElementKind::Group, k),
        Subject::Edge(k) => (ElementKind::Edge, k),
        Subject::None => return 0,
    };
    rows.iter()
        .position(|r| r.kind == kind && &r.key == key)
        .unwrap_or(0)
}

/// Build the `Subject` a navigable reference points at. Node/Group/Edge map to
/// their inspectable subjects; Diagram/Placeholder are not inspectable (`None`).
pub fn subject_from(key: &str, kind: ElementKind) -> Option<Subject> {
    match kind {
        ElementKind::Node => Some(Subject::Classifier(key.to_string())),
        ElementKind::Group => Some(Subject::Group(key.to_string())),
        ElementKind::Edge => Some(Subject::Edge(key.to_string())),
        ElementKind::Diagram | ElementKind::Placeholder => None,
    }
}

/// A node's display title, falling back to its key.
fn node_title(model: &Model, key: &str) -> String {
    model
        .nodes
        .iter()
        .find(|n| n.key == key)
        .and_then(|n| n.concept.title.clone())
        .unwrap_or_else(|| key.to_string())
}

/// Project `subject` against `model`. Returns `None` for `Subject::None` and for
/// any key that resolves to nothing (all render the empty state).
pub fn build_view(model: &Model, subject: &Subject) -> Option<InspectorView> {
    match subject {
        Subject::None => None,
        Subject::Classifier(key) => build_classifier_view(model, key),
        Subject::Group(name) => build_group_view(model, name),
        Subject::Edge(id) => build_edge_view(model, id),
    }
}

fn build_classifier_view(model: &Model, key: &str) -> Option<InspectorView> {
    let node = model.nodes.iter().find(|n| n.key == key)?;

    let attributes = node
        .attributes
        .iter()
        .map(|a| AttrRow {
            name: a.name.clone(),
            ty: a.ty.name.clone(),
            multiplicity: a.multiplicity.as_str().to_string(),
            visibility: a
                .visibility
                .map(|v| v.marker().to_string())
                .unwrap_or_default(),
        })
        .collect();

    let mut associations = Vec::new();
    for edge in &model.edges {
        // uml.Note anchor, not a real relationship (mirrors the web skip).
        if edge.kind == RelationshipKind::Annotates {
            continue;
        }
        let outgoing = edge.source == key;
        let incoming = edge.target == key;
        if !outgoing && !incoming {
            continue;
        }
        let dir = if edge.bidirectional
            || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))
        {
            AssocDir::Bi
        } else if outgoing {
            AssocDir::Out
        } else {
            AssocDir::In
        };
        // Role + multiplicity read from the FAR end.
        let far_end = if outgoing {
            &edge.to_end
        } else {
            &edge.from_end
        };
        let far_key = if outgoing { &edge.target } else { &edge.source };
        let role = far_end.role.clone().unwrap_or_default();
        // Hide a bare "1" like the attribute rows do.
        let multiplicity = match &far_end.multiplicity {
            Some(m) if m.as_str() != "1" => m.as_str().to_string(),
            _ => String::new(),
        };
        associations.push(AssocRow {
            kind: edge.kind.as_str().to_string(),
            dir,
            other_label: node_title(model, far_key),
            role,
            multiplicity,
            target_key: far_key.clone(),
            target_kind: ElementKind::Node,
        });
    }

    Some(InspectorView {
        title: node
            .concept
            .title
            .clone()
            .unwrap_or_else(|| node.key.clone()),
        kind_label: kind_label(&node.ty),
        abstract_flag: node.abstract_,
        stereotypes: node.stereotypes.clone(),
        description: node.concept.description.clone(),
        attributes,
        members: Vec::new(),
        associations,
    })
}

fn build_group_view(model: &Model, name: &str) -> Option<InspectorView> {
    fn find<'a>(groups: &'a [DiagramGroup], name: &str) -> Option<&'a DiagramGroup> {
        for g in groups {
            if g.name == name {
                return Some(g);
            }
            if let Some(found) = find(&g.children, name) {
                return Some(found);
            }
        }
        None
    }
    // First match wins across every diagram's group tree (see Global Constraints).
    let group = model.diagrams.iter().find_map(|d| find(&d.groups, name))?;
    let members = group
        .members
        .iter()
        .map(|k| ElementRef {
            key: k.clone(),
            kind: ElementKind::Node,
            label: node_title(model, k),
        })
        .collect();
    Some(InspectorView {
        title: name.to_string(),
        kind_label: "Group".to_string(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members,
        associations: Vec::new(),
    })
}

fn build_edge_view(model: &Model, id: &str) -> Option<InspectorView> {
    // Split an optional `#N` occurrence ordinal off the tail (see `edge_key`);
    // its absence means the first (occurrence 0) edge of the pair.
    let (pair, occ) = match id.rsplit_once('#') {
        Some((pair, n)) => (pair, n.parse::<usize>().ok()?),
        None => (id, 0),
    };
    let (src, tgt) = pair.split_once("->")?;
    let edge = model
        .edges
        .iter()
        .filter(|e| e.source == src && e.target == tgt)
        .nth(occ)?;
    Some(InspectorView {
        title: format!(
            "{} \u{2192} {}",
            node_title(model, src),
            node_title(model, tgt)
        ),
        kind_label: edge.kind.as_str().to_string(),
        abstract_flag: false,
        stereotypes: Vec::new(),
        description: None,
        attributes: Vec::new(),
        members: Vec::new(),
        associations: Vec::new(),
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

    fn key_for(model: &Model, title: &str) -> String {
        model
            .nodes
            .iter()
            .find(|n| n.concept.title.as_deref() == Some(title))
            .unwrap_or_else(|| panic!("no node titled {title}"))
            .key
            .clone()
    }

    #[test]
    fn classifier_projects_title_kind_and_attributes() {
        let model = mini();
        // The mini fixture's first classifier, whatever its key.
        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key.clone())).unwrap();

        assert!(!view.title.is_empty());
        assert!(!view.kind_label.is_empty());
        assert!(!view.kind_label.starts_with("uml."));
        // Attribute rows mirror the node's attributes, in order.
        let node = model.nodes.iter().find(|n| n.key == key).unwrap();
        assert_eq!(view.attributes.len(), node.attributes.len());
        for (row, attr) in view.attributes.iter().zip(&node.attributes) {
            assert_eq!(row.name, attr.name);
            assert_eq!(row.ty, attr.ty.name);
        }
    }

    #[test]
    fn classifier_projects_abstract_flag_and_stereotypes() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(view.abstract_flag);
        assert_eq!(view.stereotypes, vec!["aggregateRoot".to_string()]);
    }

    #[test]
    fn classifier_without_abstract_or_stereotype_defaults_empty() {
        let model = mini();
        let key = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(!view.abstract_flag);
        assert!(view.stereotypes.is_empty());
    }

    #[test]
    fn classifier_projects_outgoing_association() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.dir, AssocDir::Out);
        assert_eq!(assoc.other_label, "Customer");
        // Far end (to_end = "1 customer"): role kept, trivial "1" multiplicity hidden.
        assert_eq!(assoc.role, "customer");
        assert_eq!(assoc.multiplicity, "");
    }

    #[test]
    fn classifier_projects_incoming_association() {
        let model = mini();
        let key = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.dir, AssocDir::In);
        assert_eq!(assoc.other_label, "Order");
        // Far end (from_end = "1 order").
        assert_eq!(assoc.role, "order");
        assert_eq!(assoc.multiplicity, "");
    }

    #[test]
    fn classifier_projects_bidirectional_association() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: true,
        });
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        let bi = view
            .associations
            .iter()
            .find(|r| r.dir == AssocDir::Bi)
            .expect("a bidirectional row projected");
        assert_eq!(bi.other_label, "PaymentGateway");
        assert_eq!(bi.kind, "associates");
    }

    #[test]
    fn classifier_projects_far_end_role_and_multiplicity() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        use waml::multiplicity::Multiplicity;
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Aggregates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd {
                multiplicity: Multiplicity::parse("0..1"),
                role: Some("buyer".to_string()),
                navigable: None,
            },
            bidirectional: false,
        });
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        let agg = view
            .associations
            .iter()
            .find(|r| r.kind == "aggregates")
            .expect("the aggregates row projected");
        assert_eq!(agg.dir, AssocDir::Out);
        assert_eq!(agg.role, "buyer");
        assert_eq!(agg.multiplicity, "0..1");
    }

    #[test]
    fn annotates_edges_are_skipped() {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        let before = build_view(&model, &Subject::Classifier(order.clone()))
            .unwrap()
            .associations
            .len();
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway,
            kind: RelationshipKind::Annotates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        let after = build_view(&model, &Subject::Classifier(order))
            .unwrap()
            .associations
            .len();
        assert_eq!(before, after, "an annotates edge must not project a row");
    }

    #[test]
    fn none_subject_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::None).is_none());
    }

    #[test]
    fn missing_key_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Classifier("does-not-exist".into())).is_none());
    }

    #[test]
    fn effective_field_falls_back_to_model_when_no_override() {
        let model = mini();
        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(effective_field(&view, FieldId::Title, None), view.title);
    }

    #[test]
    fn effective_field_prefers_override_over_model() {
        let model = mini();
        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        let over = "Renamed Title".to_string();
        assert_eq!(
            effective_field(&view, FieldId::Title, Some(&over)),
            "Renamed Title"
        );
        // The source view (and thus the model it was built from) is untouched.
        assert_ne!(view.title, "Renamed Title");
    }

    #[test]
    fn overrides_are_keyed_per_subject() {
        use std::collections::HashMap;

        let model = mini();
        let mut overrides: HashMap<(String, FieldId), String> = HashMap::new();
        overrides.insert(("a".into(), FieldId::Title), "A edited".into());
        overrides.insert(("b".into(), FieldId::Title), "B edited".into());

        let key = model.nodes[0].key.clone();
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();

        let a = effective_field(
            &view,
            FieldId::Title,
            overrides.get(&("a".to_string(), FieldId::Title)),
        );
        let b = effective_field(
            &view,
            FieldId::Title,
            overrides.get(&("b".to_string(), FieldId::Title)),
        );
        let c = effective_field(
            &view,
            FieldId::Title,
            overrides.get(&("c".to_string(), FieldId::Title)),
        );

        assert_eq!(a, "A edited");
        assert_eq!(b, "B edited");
        assert_eq!(c, view.title, "an unedited subject falls back to the model");
    }

    /// `mini()` with one named group (`Sales` = Order + Customer) pushed onto the
    /// `orders-diagram` diagram, alongside the parser-produced implicit (`""`)
    /// group. The on-disk fixture is untouched, so scene/layout tests are
    /// unaffected. Used by the group/edge tests below.
    fn mini_with_group() -> Model {
        let mut model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let diagram = model
            .diagrams
            .iter_mut()
            .find(|d| d.key == "orders-diagram")
            .expect("mini has the orders-diagram");
        diagram.groups.push(waml::model::DiagramGroup {
            name: "Sales".to_string(),
            members: vec![order, customer],
            children: Vec::new(),
        });
        model
    }

    #[test]
    fn mini_with_group_shapes_the_diagram() {
        let model = mini_with_group();
        let diagram = model
            .diagrams
            .iter()
            .find(|d| d.key == "orders-diagram")
            .expect("mini has the orders-diagram");
        // The named "Sales" group holds Order + Customer.
        let sales = diagram
            .groups
            .iter()
            .find(|g| g.name == "Sales")
            .expect("Sales group present");
        assert_eq!(sales.members.len(), 2, "Sales holds Order + Customer");
        // The parser's implicit ("") group is still present (holds the flat members).
        assert!(
            diagram.groups.iter().any(|g| g.name.is_empty()),
            "implicit unnamed group present"
        );
        // The on-disk fixture is untouched: still exactly three classifiers.
        assert_eq!(model.nodes.len(), 3);
    }

    fn node_keys(model: &Model) -> Vec<String> {
        model.nodes.iter().map(|n| n.key.clone()).collect()
    }

    #[test]
    fn picker_rows_lead_with_placeholder_then_diagram() {
        let model = mini();
        let rows = diagram_elements(&model, "d1", "Orders", &node_keys(&model));
        assert_eq!(rows[0].kind, ElementKind::Placeholder);
        assert_eq!(rows[0].label, PICKER_PLACEHOLDER);
        assert_eq!(rows[1].kind, ElementKind::Diagram);
        assert_eq!(rows[1].key, "d1");
        assert_eq!(rows[1].label, "Orders");
    }

    #[test]
    fn picker_rows_list_every_node() {
        let model = mini();
        let keys = node_keys(&model);
        let rows = diagram_elements(&model, "d1", "Orders", &keys);
        let node_rows: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == ElementKind::Node)
            .collect();
        assert_eq!(node_rows.len(), keys.len());
    }

    #[test]
    fn picker_nests_edge_after_its_source_node() {
        let model = mini();
        let keys = node_keys(&model);
        let order = key_for(&model, "Order");
        let rows = diagram_elements(&model, "d1", "Orders", &keys);

        let order_idx = rows
            .iter()
            .position(|r| r.kind == ElementKind::Node && r.key == order)
            .expect("Order node row present");
        // The Order->Customer edge is listed immediately after the Order node.
        let edge = &rows[order_idx + 1];
        assert_eq!(edge.kind, ElementKind::Edge);
        assert_eq!(edge.label, "Order -> Customer");
    }

    #[test]
    fn subject_to_index_resolves_node_row() {
        let model = mini();
        let keys = node_keys(&model);
        let customer = key_for(&model, "Customer");
        let rows = diagram_elements(&model, "d1", "Orders", &keys);

        let idx = subject_to_index(&rows, &Subject::Classifier(customer.clone()));
        assert_eq!(rows[idx].kind, ElementKind::Node);
        assert_eq!(rows[idx].key, customer);
    }

    #[test]
    fn subject_to_index_none_and_unknown_fall_back_to_placeholder() {
        let model = mini();
        let rows = diagram_elements(&model, "d1", "Orders", &node_keys(&model));
        assert_eq!(subject_to_index(&rows, &Subject::None), 0);
        assert_eq!(
            subject_to_index(&rows, &Subject::Classifier("nope".into())),
            0
        );
    }

    #[test]
    fn picker_lists_named_groups_after_diagram_before_nodes() {
        let model = mini_with_group();
        // Pass the REAL diagram key so groups resolve off the model.
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));

        // Row 0 = placeholder, row 1 = diagram, row 2 = first (only) named group.
        assert_eq!(rows[1].kind, ElementKind::Diagram);
        assert_eq!(rows[2].kind, ElementKind::Group);
        assert_eq!(rows[2].key, "Sales");
        assert_eq!(rows[2].label, "Sales");

        // Groups precede nodes.
        let first_group = rows
            .iter()
            .position(|r| r.kind == ElementKind::Group)
            .expect("a group row");
        let first_node = rows
            .iter()
            .position(|r| r.kind == ElementKind::Node)
            .expect("a node row");
        assert!(first_group < first_node, "group rows come before node rows");

        // Exactly one named group; the implicit "" group is skipped.
        let group_rows: Vec<_> = rows
            .iter()
            .filter(|r| r.kind == ElementKind::Group)
            .collect();
        assert_eq!(group_rows.len(), 1);
        assert!(
            group_rows.iter().all(|r| !r.key.is_empty()),
            "the implicit unnamed group must be skipped"
        );
    }

    #[test]
    fn group_projects_name_kind_and_members() {
        let model = mini_with_group();
        let view = build_view(&model, &Subject::Group("Sales".into())).unwrap();
        assert_eq!(view.title, "Sales");
        assert_eq!(view.kind_label, "Group");
        // Members are ElementRefs: node kind, key = member key, label = node title.
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        assert_eq!(view.members.len(), 2);
        assert_eq!(view.members[0].key, order);
        assert_eq!(view.members[0].kind, ElementKind::Node);
        assert_eq!(view.members[0].label, "Order");
        assert_eq!(view.members[1].key, customer);
        assert_eq!(view.members[1].kind, ElementKind::Node);
        assert_eq!(view.members[1].label, "Customer");
        assert!(view.attributes.is_empty());
        assert!(view.associations.is_empty());
        assert!(view.description.is_none());
    }

    #[test]
    fn association_target_resolves_to_far_endpoint() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(order)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        // Outgoing Order->Customer: far endpoint is Customer.
        assert_eq!(assoc.target_key, customer);
        assert_eq!(assoc.target_kind, ElementKind::Node);
        assert_eq!(assoc.other_label, "Customer");
    }

    #[test]
    fn incoming_association_target_is_the_source_node() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(customer)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        // Incoming (Customer is the target): far endpoint is the source, Order.
        assert_eq!(assoc.target_key, order);
        assert_eq!(assoc.target_kind, ElementKind::Node);
    }

    #[test]
    fn unknown_group_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Group("Nope".into())).is_none());
    }

    #[test]
    fn edge_projects_endpoint_titles_and_kind() {
        let model = mini();
        let order = key_for(&model, "Order");
        let customer = key_for(&model, "Customer");
        let id = format!("{order}->{customer}");
        let view = build_view(&model, &Subject::Edge(id)).unwrap();
        // Title carries both endpoint titles.
        assert!(
            view.title.contains("Order"),
            "title has source: {}",
            view.title
        );
        assert!(
            view.title.contains("Customer"),
            "title has target: {}",
            view.title
        );
        // Kind is the relationship kind string.
        assert_eq!(view.kind_label, "associates");
        assert!(view.members.is_empty());
    }

    #[test]
    fn unknown_edge_yields_empty_state() {
        let model = mini();
        assert!(build_view(&model, &Subject::Edge("a->b".into())).is_none());
    }

    #[test]
    fn classifier_has_empty_members() {
        let model = mini();
        let key = key_for(&model, "Order");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert!(view.members.is_empty());
    }

    #[test]
    fn subject_to_index_resolves_group_row() {
        let model = mini_with_group();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        let idx = subject_to_index(&rows, &Subject::Group("Sales".into()));
        assert_eq!(rows[idx].kind, ElementKind::Group);
        assert_eq!(rows[idx].key, "Sales");
    }

    #[test]
    fn subject_to_index_resolves_edge_row() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        let edge_key = format!(
            "{}->{}",
            key_for(&model, "Order"),
            key_for(&model, "Customer")
        );
        let idx = subject_to_index(&rows, &Subject::Edge(edge_key.clone()));
        assert_eq!(rows[idx].kind, ElementKind::Edge);
        assert_eq!(rows[idx].key, edge_key);
    }

    #[test]
    fn subject_to_index_unknown_group_and_edge_fall_back_to_placeholder() {
        let model = mini();
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));
        assert_eq!(subject_to_index(&rows, &Subject::Group("Nope".into())), 0);
        assert_eq!(subject_to_index(&rows, &Subject::Edge("x->y".into())), 0);
    }

    #[test]
    fn subject_from_maps_each_kind() {
        assert_eq!(
            subject_from("k", ElementKind::Node),
            Some(Subject::Classifier("k".into()))
        );
        assert_eq!(
            subject_from("g", ElementKind::Group),
            Some(Subject::Group("g".into()))
        );
        assert_eq!(
            subject_from("a->b", ElementKind::Edge),
            Some(Subject::Edge("a->b".into()))
        );
        assert_eq!(subject_from("d", ElementKind::Diagram), None);
        assert_eq!(subject_from("", ElementKind::Placeholder), None);
    }

    /// `mini()` with two *parallel* Order->PaymentGateway edges of different
    /// relationship kinds. The synthetic edge key must disambiguate them so each
    /// picker row resolves to (and projects) its own edge — not the first match.
    fn mini_with_parallel_edges() -> Model {
        use waml::model::{Edge, RelEnd, RelationshipKind};
        let mut model = mini();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        model.edges.push(Edge {
            source: order.clone(),
            target: gateway.clone(),
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        model.edges.push(Edge {
            source: order,
            target: gateway,
            kind: RelationshipKind::Depends,
            name: None,
            from_end: RelEnd::default(),
            to_end: RelEnd::default(),
            bidirectional: false,
        });
        model
    }

    #[test]
    fn parallel_edges_get_distinct_keys_and_project_each_kind() {
        let model = mini_with_parallel_edges();
        let order = key_for(&model, "Order");
        let gateway = key_for(&model, "PaymentGateway");
        let rows = diagram_elements(&model, "orders-diagram", "Orders", &node_keys(&model));

        let pair_prefix = format!("{order}->{gateway}");
        let parallel: Vec<&ElementRow> = rows
            .iter()
            .filter(|r| r.kind == ElementKind::Edge && r.key.starts_with(&pair_prefix))
            .collect();
        assert_eq!(parallel.len(), 2, "two parallel Order->PaymentGateway rows");

        // Distinct picker keys — else both rows collapse onto the first edge.
        assert_ne!(
            parallel[0].key, parallel[1].key,
            "parallel edges must have distinct picker keys"
        );

        // Each key resolves back to its own row and projects its own edge kind.
        let mut kinds = Vec::new();
        for r in &parallel {
            let idx = subject_to_index(&rows, &Subject::Edge(r.key.clone()));
            assert_eq!(rows[idx].key, r.key, "each key resolves to its own row");
            let view = build_view(&model, &Subject::Edge(r.key.clone())).unwrap();
            kinds.push(view.kind_label);
        }
        assert!(
            kinds.contains(&"associates".to_string()),
            "one parallel edge projects `associates`, got {kinds:?}"
        );
        assert!(
            kinds.contains(&"depends".to_string()),
            "one parallel edge projects `depends`, got {kinds:?}"
        );
    }
}
