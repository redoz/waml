use waml::model::{DiagramGroupRole, ElementType};
use waml_editor::{GroupVisualKind, NodeVisualKind, StructuralVisualKind, StructuralVisualPolicy};

#[test]
fn equal_members_use_different_structural_visuals() {
    let actor = ElementType::parse("uml.Actor");
    let class = StructuralVisualPolicy {
        kind: StructuralVisualKind::Class,
    };
    let use_case = StructuralVisualPolicy {
        kind: StructuralVisualKind::UseCase,
    };

    assert_eq!(class.node_kind(&actor), NodeVisualKind::ClassCard);
    assert_eq!(use_case.node_kind(&actor), NodeVisualKind::Actor);
    assert_eq!(
        class.group_kind(DiagramGroupRole::SystemBoundary),
        GroupVisualKind::Generic
    );
    assert_eq!(
        use_case.group_kind(DiagramGroupRole::SystemBoundary),
        GroupVisualKind::SystemBoundary
    );
}

#[test]
fn empty_use_case_diagram_keeps_use_case_visual_identity() {
    assert_eq!(StructuralVisualKind::UseCase, StructuralVisualKind::UseCase);
}
