//! The inspector seam: project a `Model` + a subject into a flat `InspectorView`
//! for the panel. Nothing here touches makepad; the widget lives in
//! `inspector_panel.rs`. Mirrors the `tree.rs` (pure) / `tree_panel.rs` (widget)
//! split.

use waml::model::{ElementType, Model};

/// What the inspector is currently pointed at. `None` renders the empty state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Subject {
    #[default]
    None,
    Classifier(String),
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
    Node,
    Edge,
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

/// One association row, pre-rendered to display strings. Derived from
/// `Model::edges` where `key` is either endpoint -- read-only breadth (U6),
/// not an editable field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssocRow {
    pub kind: String,            // RelationshipKind::as_str(), e.g. "associates"
    pub direction: &'static str, // "->" (key is source) or "<-" (key is target)
    pub other_label: String,     // the other endpoint's title, falling back to its key
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
    for nk in node_keys {
        rows.push(ElementRow {
            key: nk.clone(),
            label: title_of(nk),
            kind: ElementKind::Node,
        });
        // Edges anchored at this node's source end, nested right after it.
        for edge in &model.edges {
            if &edge.source == nk && present.contains(edge.target.as_str()) {
                rows.push(ElementRow {
                    key: format!("{}->{}", edge.source, edge.target),
                    label: format!("{} -> {}", title_of(&edge.source), title_of(&edge.target)),
                    kind: ElementKind::Edge,
                });
            }
        }
    }
    rows
}

/// The picker index for `subject`: 0 (placeholder) for `None` or a key with no
/// matching `Node` row, else the `Node` row whose key matches.
pub fn subject_to_index(rows: &[ElementRow], subject: &Subject) -> usize {
    let Subject::Classifier(key) = subject else {
        return 0;
    };
    rows.iter()
        .position(|r| r.kind == ElementKind::Node && &r.key == key)
        .unwrap_or(0)
}

/// Project `subject` against `model`. Returns `None` for `Subject::None` and for
/// a classifier key that resolves to nothing (both render the empty state).
pub fn build_view(model: &Model, subject: &Subject) -> Option<InspectorView> {
    let Subject::Classifier(key) = subject else {
        return None;
    };
    let node = model.nodes.iter().find(|n| &n.key == key)?;

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

    let node_label = |k: &str| -> String {
        model
            .nodes
            .iter()
            .find(|n| n.key == k)
            .and_then(|n| n.concept.title.clone())
            .unwrap_or_else(|| k.to_string())
    };
    let mut associations = Vec::new();
    for edge in &model.edges {
        if &edge.source == key {
            associations.push(AssocRow {
                kind: edge.kind.as_str().to_string(),
                direction: "->",
                other_label: node_label(&edge.target),
            });
        } else if &edge.target == key {
            associations.push(AssocRow {
                kind: edge.kind.as_str().to_string(),
                direction: "<-",
                other_label: node_label(&edge.source),
            });
        }
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
        associations,
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
        assert_eq!(assoc.direction, "->");
        assert_eq!(assoc.other_label, "Customer");
    }

    #[test]
    fn classifier_projects_incoming_association() {
        let model = mini();
        let key = key_for(&model, "Customer");
        let view = build_view(&model, &Subject::Classifier(key)).unwrap();
        assert_eq!(view.associations.len(), 1);
        let assoc = &view.associations[0];
        assert_eq!(assoc.kind, "associates");
        assert_eq!(assoc.direction, "<-");
        assert_eq!(assoc.other_label, "Order");
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
        let node_rows: Vec<_> = rows.iter().filter(|r| r.kind == ElementKind::Node).collect();
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
}
