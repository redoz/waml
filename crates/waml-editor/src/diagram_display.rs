use waml::model::{CardinalityVisibility, DiagramDisplay};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDiagramDisplay {
    pub show_attributes: bool,
    pub show_type: bool,
    pub show_attribute_visibility: bool,
    /// Preserved for complete display updates. Native cards do not currently
    /// render attribute multiplicity as a separately configurable detail.
    pub show_attribute_multiplicity: bool,
    pub max_attributes: Option<u32>,
    pub show_roles: bool,
    pub cardinality: CardinalityVisibility,
    pub show_labels: bool,
    pub show_stereotype: bool,
    pub stereotype_filter: Option<Vec<String>>,
    pub stereotype_colors: Vec<String>,
}

impl Default for ResolvedDiagramDisplay {
    fn default() -> Self {
        resolve_display(&DiagramDisplay::default())
    }
}

pub fn resolve_display(display: &DiagramDisplay) -> ResolvedDiagramDisplay {
    ResolvedDiagramDisplay {
        show_attributes: display.show_attributes.unwrap_or(true),
        show_type: display.show_type.unwrap_or(true),
        show_attribute_visibility: display.show_attribute_visibility.unwrap_or(true),
        show_attribute_multiplicity: display.show_attribute_multiplicity.unwrap_or(true),
        max_attributes: display.max_attributes,
        show_roles: display.show_roles.unwrap_or(true),
        cardinality: display
            .cardinality
            .unwrap_or(CardinalityVisibility::Explicit),
        show_labels: display.show_labels.unwrap_or(true),
        show_stereotype: display.show_stereotype.unwrap_or(true),
        stereotype_filter: display.stereotype_filter.clone(),
        stereotype_colors: display.stereotype_colors.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_display;
    use waml::model::{CardinalityVisibility, DiagramDisplay};

    #[test]
    fn display_defaults_to_explicit_cardinality() {
        assert_eq!(
            resolve_display(&DiagramDisplay::default()).cardinality,
            CardinalityVisibility::Explicit
        );
    }

    #[test]
    fn display_preserves_authored_values() {
        let partial = DiagramDisplay {
            show_attributes: Some(false),
            show_attribute_multiplicity: Some(false),
            cardinality: Some(CardinalityVisibility::All),
            ..Default::default()
        };
        let resolved = resolve_display(&partial);
        assert!(!resolved.show_attributes);
        assert!(!resolved.show_attribute_multiplicity);
        assert_eq!(resolved.cardinality, CardinalityVisibility::All);
    }
}
