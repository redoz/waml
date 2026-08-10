use waml::{
    adornment::{end_marker, End, Marker},
    model::{DiagramGroupRole, ElementType, RelationshipKind, UmlMetaclass},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StructuralVisualKind {
    #[default]
    Class,
    UseCase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeVisualKind {
    ClassCard,
    Actor,
    UseCase,
    Note,
    Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupVisualKind {
    Generic,
    ActorRail,
    SystemBoundary,
    Band,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructuralVisualPolicy {
    pub kind: StructuralVisualKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeLineStyle {
    Solid,
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeNotation {
    pub line: EdgeLineStyle,
    pub from_marker: Marker,
    pub to_marker: Marker,
    pub middle_label: Option<&'static str>,
}

impl StructuralVisualPolicy {
    pub fn edge_notation(
        &self,
        kind: RelationshipKind,
        from_navigable: Option<bool>,
        to_navigable: Option<bool>,
    ) -> EdgeNotation {
        if self.kind == StructuralVisualKind::UseCase {
            return match kind {
                RelationshipKind::Associates => EdgeNotation {
                    line: EdgeLineStyle::Solid,
                    from_marker: Marker::None,
                    to_marker: Marker::None,
                    middle_label: None,
                },
                RelationshipKind::Includes => EdgeNotation {
                    line: EdgeLineStyle::Dashed,
                    from_marker: Marker::None,
                    to_marker: Marker::OpenArrow,
                    middle_label: Some("«include»"),
                },
                RelationshipKind::Extends => EdgeNotation {
                    line: EdgeLineStyle::Dashed,
                    from_marker: Marker::None,
                    to_marker: Marker::OpenArrow,
                    middle_label: Some("«extend»"),
                },
                RelationshipKind::Specializes => EdgeNotation {
                    line: EdgeLineStyle::Solid,
                    from_marker: Marker::None,
                    to_marker: Marker::HollowTriangle,
                    middle_label: None,
                },
                _ => generic_edge_notation(kind, from_navigable, to_navigable),
            };
        }
        generic_edge_notation(kind, from_navigable, to_navigable)
    }

    pub fn node_kind(&self, ty: &ElementType) -> NodeVisualKind {
        if self.kind == StructuralVisualKind::UseCase {
            match ty {
                ElementType::Uml(UmlMetaclass::Actor) => NodeVisualKind::Actor,
                ElementType::Uml(UmlMetaclass::UseCase) => NodeVisualKind::UseCase,
                ElementType::Uml(UmlMetaclass::Note) => NodeVisualKind::Note,
                ElementType::Uml(UmlMetaclass::Package) => NodeVisualKind::Package,
                _ => NodeVisualKind::ClassCard,
            }
        } else {
            NodeVisualKind::ClassCard
        }
    }

    pub fn group_kind(&self, role: DiagramGroupRole) -> GroupVisualKind {
        if self.kind == StructuralVisualKind::UseCase {
            match role {
                DiagramGroupRole::Generic => GroupVisualKind::Generic,
                DiagramGroupRole::ExternalActors => GroupVisualKind::ActorRail,
                DiagramGroupRole::SystemBoundary => GroupVisualKind::SystemBoundary,
                DiagramGroupRole::Band => GroupVisualKind::Band,
            }
        } else {
            GroupVisualKind::Generic
        }
    }
}

fn generic_edge_notation(
    kind: RelationshipKind,
    from: Option<bool>,
    to: Option<bool>,
) -> EdgeNotation {
    EdgeNotation {
        line: EdgeLineStyle::Solid,
        from_marker: end_marker(kind, End::From, from),
        to_marker: end_marker(kind, End::To, to),
        middle_label: None,
    }
}

#[cfg(test)]
mod edge_tests {
    use super::*;

    #[test]
    fn use_case_relationship_notation_is_exact() {
        let policy = StructuralVisualPolicy {
            kind: StructuralVisualKind::UseCase,
        };
        let notation = |kind| policy.edge_notation(kind, Some(true), Some(true));
        assert_eq!(
            notation(RelationshipKind::Associates),
            EdgeNotation {
                line: EdgeLineStyle::Solid,
                from_marker: Marker::None,
                to_marker: Marker::None,
                middle_label: None
            }
        );
        assert_eq!(
            notation(RelationshipKind::Includes),
            EdgeNotation {
                line: EdgeLineStyle::Dashed,
                from_marker: Marker::None,
                to_marker: Marker::OpenArrow,
                middle_label: Some("«include»")
            }
        );
        assert_eq!(
            notation(RelationshipKind::Extends).middle_label,
            Some("«extend»")
        );
        assert_eq!(
            notation(RelationshipKind::Specializes).to_marker,
            Marker::HollowTriangle
        );
    }

    #[test]
    fn class_association_navigability_is_unchanged() {
        let policy = StructuralVisualPolicy {
            kind: StructuralVisualKind::Class,
        };
        assert_eq!(
            policy
                .edge_notation(RelationshipKind::Associates, None, Some(true))
                .to_marker,
            Marker::OpenArrow
        );
    }
}
