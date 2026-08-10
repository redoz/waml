use waml::{analysis::prepare_candidate, model::CardinalityVisibility, source::SourceBundle};

#[test]
fn diagram_display_frontmatter_projects_all_supported_fields() {
    let source = SourceBundle::try_from_pairs([(
        "domain.md",
        "---\n\
         type: uml.ClassDiagram\n\
         title: Domain\n\
         profile: uml-domain\n\
         description: Notes\n\
         showAttributes: false\n\
         showType: false\n\
         attributeDetail: name-type\n\
         showAttributeVisibility: false\n\
         showAttributeMultiplicity: true\n\
         cardinality: explicit\n\
         maxAttributes: 6\n\
         showRoles: false\n\
         showCardinality: true\n\
         showLabels: true\n\
         showStereotype: false\n\
         stereotypeFilter: [entity, valueObject]\n\
         stereotypeColors: [\"entity:#ffedd5\"]\n\
         ---\n\
         # Domain\n",
    )])
    .unwrap();

    let projection = prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone();
    let diagram = &projection.diagrams[0];

    assert_eq!(diagram.description.as_deref(), Some("Notes"));
    assert_eq!(diagram.display.show_attributes, Some(false));
    assert_eq!(diagram.display.show_type, Some(false));
    assert_eq!(diagram.display.show_attribute_visibility, Some(false));
    assert_eq!(diagram.display.show_attribute_multiplicity, Some(true));
    assert_eq!(
        diagram.display.cardinality,
        Some(CardinalityVisibility::Explicit)
    );
    assert_eq!(diagram.display.max_attributes, Some(6));
    assert_eq!(diagram.display.show_roles, Some(false));
    assert_eq!(diagram.display.show_cardinality, Some(true));
    assert_eq!(diagram.display.show_labels, Some(true));
    assert_eq!(diagram.display.show_stereotype, Some(false));
    assert_eq!(
        diagram.display.stereotype_filter,
        Some(vec!["entity".into(), "valueObject".into()])
    );
    assert_eq!(diagram.display.stereotype_colors, ["entity:#ffedd5"]);
}

#[test]
fn diagram_display_uses_legacy_attribute_fields_when_newer_fields_are_absent() {
    let source = SourceBundle::try_from_pairs([(
        "domain.md",
        "---\n\
         type: uml.ClassDiagram\n\
         title: Domain\n\
         profile: uml-domain\n\
         attributeDetail: name-type\n\
         showAttributeMultiplicity: false\n\
         ---\n\
         # Domain\n",
    )])
    .unwrap();

    let projection = prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone();
    let display = &projection.diagrams[0].display;

    assert_eq!(display.show_type, Some(true));
    assert_eq!(display.cardinality, Some(CardinalityVisibility::Off));
    assert_eq!(display.show_attribute_multiplicity, Some(false));
    assert_eq!(display.show_cardinality, None);
}

#[test]
fn diagram_display_preserves_absent_empty_and_zero_frontmatter_states() {
    let source = SourceBundle::try_from_pairs([
        (
            "absent.md",
            "---\n\
             type: uml.ClassDiagram\n\
             title: Absent\n\
             profile: uml-domain\n\
             ---\n\
             # Absent\n",
        ),
        (
            "empty.md",
            "---\n\
             type: uml.ClassDiagram\n\
             title: Empty\n\
             profile: uml-domain\n\
             stereotypeFilter: []\n\
             maxAttributes: 0\n\
             ---\n\
             # Empty\n",
        ),
    ])
    .unwrap();

    let projection = prepare_candidate(source, None, 0)
        .unwrap()
        .uml()
        .projection
        .clone();
    let absent = projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "absent")
        .unwrap();
    let empty = projection
        .diagrams
        .iter()
        .find(|diagram| diagram.key == "empty")
        .unwrap();

    assert_eq!(absent.display.stereotype_filter, None);
    assert_eq!(absent.display.max_attributes, None);
    assert_eq!(empty.display.stereotype_filter, Some(vec![]));
    assert_eq!(empty.display.max_attributes, None);
}
