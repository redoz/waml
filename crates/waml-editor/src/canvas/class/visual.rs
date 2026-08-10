use waml::model::{DiagramGroupRole, ElementType, UmlMetaclass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StructuralVisualKind {
    #[default]
    Class,
    UseCase,
}

impl From<waml::model::DiagramKind> for StructuralVisualKind {
    fn from(kind: waml::model::DiagramKind) -> Self {
        match kind {
            waml::model::DiagramKind::UseCase => Self::UseCase,
            _ => Self::Class,
        }
    }
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

impl StructuralVisualPolicy {
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
