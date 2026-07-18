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

/// One attribute row, pre-rendered to display strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrRow {
    pub name: String,
    pub ty: String,
    pub multiplicity: String,
    pub visibility: String, // "+"/"-"/"#"/"~" or ""
}

/// The flattened read model the panel renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectorView {
    pub title: String,
    pub kind_label: String,
    pub description: Option<String>,
    pub attributes: Vec<AttrRow>,
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
            visibility: a.visibility.map(|v| v.marker().to_string()).unwrap_or_default(),
        })
        .collect();

    Some(InspectorView {
        title: node.concept.title.clone().unwrap_or_else(|| node.key.clone()),
        kind_label: kind_label(&node.ty),
        description: node.concept.description.clone(),
        attributes,
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
        assert_eq!(effective_field(&view, FieldId::Title, Some(&over)), "Renamed Title");
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

        let a = effective_field(&view, FieldId::Title, overrides.get(&("a".to_string(), FieldId::Title)));
        let b = effective_field(&view, FieldId::Title, overrides.get(&("b".to_string(), FieldId::Title)));
        let c = effective_field(&view, FieldId::Title, overrides.get(&("c".to_string(), FieldId::Title)));

        assert_eq!(a, "A edited");
        assert_eq!(b, "B edited");
        assert_eq!(c, view.title, "an unedited subject falls back to the model");
    }
}
