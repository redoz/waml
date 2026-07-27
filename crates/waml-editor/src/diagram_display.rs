use waml::model::{CardinalityVisibility, DiagramDisplay};

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDiagramDisplay {
    pub show_attributes: bool,
    pub show_type: bool,
    pub show_attribute_visibility: bool,
    pub cardinality: CardinalityVisibility,
    pub max_attributes: Option<u32>,
    pub show_roles: bool,
    pub show_cardinality: bool,
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
    let cardinality = display
        .cardinality
        .or_else(|| {
            display
                .show_attribute_multiplicity
                .map(CardinalityVisibility::from_legacy_attribute_gate)
        })
        .unwrap_or_default();
    ResolvedDiagramDisplay {
        show_attributes: display.show_attributes.unwrap_or(true),
        show_type: display.show_type.unwrap_or(true),
        show_attribute_visibility: display.show_attribute_visibility.unwrap_or(true),
        cardinality,
        max_attributes: display.max_attributes,
        show_roles: display.show_roles.unwrap_or(true),
        show_cardinality: display.show_cardinality.unwrap_or(true),
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
    fn display_defaults_to_explicit_attribute_and_visible_relationship_cardinality() {
        let resolved = resolve_display(&DiagramDisplay::default());
        assert_eq!(resolved.cardinality, CardinalityVisibility::Explicit);
        assert!(resolved.show_cardinality);
    }

    #[test]
    fn attribute_and_relationship_cardinality_resolve_independently() {
        let partial = DiagramDisplay {
            cardinality: Some(CardinalityVisibility::Off),
            show_cardinality: Some(true),
            ..Default::default()
        };

        let resolved = resolve_display(&partial);

        assert_eq!(resolved.cardinality, CardinalityVisibility::Off);
        assert!(resolved.show_cardinality);
    }

    #[test]
    fn display_preserves_authored_values_with_cardinality_authority() {
        let partial = DiagramDisplay {
            show_attributes: Some(false),
            show_attribute_multiplicity: Some(false),
            cardinality: Some(CardinalityVisibility::All),
            ..Default::default()
        };
        let resolved = resolve_display(&partial);
        assert!(!resolved.show_attributes);
        assert_eq!(resolved.cardinality, CardinalityVisibility::All);
    }

    #[test]
    fn cardinality_is_authoritative_with_a_legacy_boolean_fallback() {
        let contradictory = DiagramDisplay {
            show_attribute_multiplicity: Some(true),
            cardinality: Some(CardinalityVisibility::Off),
            ..Default::default()
        };
        let resolved = resolve_display(&contradictory);
        assert_eq!(resolved.cardinality, CardinalityVisibility::Off);

        let legacy = DiagramDisplay {
            show_attribute_multiplicity: Some(false),
            cardinality: None,
            ..Default::default()
        };
        let resolved = resolve_display(&legacy);
        assert_eq!(resolved.cardinality, CardinalityVisibility::Off);
        assert!(
            resolved.show_cardinality,
            "the legacy attribute-only gate must not hide relationship cardinality"
        );
    }
}
