use waml::{adornment::Marker, model::RelationshipKind};
use waml_editor::{EdgeLineStyle, StructuralVisualKind, StructuralVisualPolicy};

#[test]
fn all_four_use_case_relationships_have_exact_uml_notation() {
    let policy = StructuralVisualPolicy {
        kind: StructuralVisualKind::UseCase,
    };
    let notation = |kind| policy.edge_notation(kind, Some(true), Some(true));

    let association = notation(RelationshipKind::Associates);
    assert_eq!(association.line, EdgeLineStyle::Solid);
    assert_eq!(
        (association.from_marker, association.to_marker),
        (Marker::None, Marker::None)
    );
    assert_eq!(association.middle_label, None);

    for (kind, label) in [
        (RelationshipKind::Includes, "«include»"),
        (RelationshipKind::Extends, "«extend»"),
    ] {
        let dependency = notation(kind);
        assert_eq!(dependency.line, EdgeLineStyle::Dashed);
        assert_eq!(
            (dependency.from_marker, dependency.to_marker),
            (Marker::None, Marker::OpenArrow)
        );
        assert_eq!(dependency.middle_label, Some(label));
    }

    let specialization = notation(RelationshipKind::Specializes);
    assert_eq!(specialization.line, EdgeLineStyle::Solid);
    assert_eq!(specialization.to_marker, Marker::HollowTriangle);
}

#[test]
fn navigable_association_stays_bare_only_in_use_case_views() {
    let use_case = StructuralVisualPolicy {
        kind: StructuralVisualKind::UseCase,
    };
    let class = StructuralVisualPolicy {
        kind: StructuralVisualKind::Class,
    };
    assert_eq!(
        use_case
            .edge_notation(RelationshipKind::Associates, None, Some(true))
            .to_marker,
        Marker::None
    );
    assert_eq!(
        class
            .edge_notation(RelationshipKind::Associates, None, Some(true))
            .to_marker,
        Marker::OpenArrow
    );
}
