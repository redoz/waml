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
}
