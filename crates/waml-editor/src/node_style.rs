//! Pure node-kind -> visual style mapping (U9 mock): buckets a model
//! element's `ElementType` into a small accent-color category plus an
//! optional stereotype guillemet label, both consumed by `canvas.rs`'s node
//! renderer. No makepad/GPU dependency here -- trivially unit-tested.
//!
//! Buckets are coarse on purpose (breadth over polish): several UML
//! metaclasses share a bucket rather than each getting a bespoke color.

use waml::model::{BehaviorKind, ElementType, UmlMetaclass};

/// A coarse accent-color category for a node's kind. `None` means "no accent
/// bar" -- the default look (plain `uml.Class`, `uml.Association`, and any
/// node whose type didn't resolve to something more specific).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentBucket {
    None,
    Interface,
    Enum,
    Note,
    Actor,
    UseCase,
    Package,
    Behavior,
    Unknown,
}

/// Which accent bucket a node's element type renders with.
pub fn accent_bucket(ty: &ElementType) -> AccentBucket {
    match ty {
        ElementType::Uml(UmlMetaclass::Interface) => AccentBucket::Interface,
        ElementType::Uml(UmlMetaclass::Enum) => AccentBucket::Enum,
        ElementType::Uml(UmlMetaclass::DataType) => AccentBucket::Enum,
        ElementType::Uml(UmlMetaclass::Note) => AccentBucket::Note,
        ElementType::Uml(UmlMetaclass::Actor) => AccentBucket::Actor,
        ElementType::Uml(UmlMetaclass::UseCase) => AccentBucket::UseCase,
        ElementType::Uml(UmlMetaclass::Package) => AccentBucket::Package,
        ElementType::Uml(UmlMetaclass::Class) | ElementType::Uml(UmlMetaclass::Association) => {
            AccentBucket::None
        }
        ElementType::Behavior(_) => AccentBucket::Behavior,
        ElementType::Diagram => AccentBucket::None,
        ElementType::Unknown(_) => AccentBucket::Unknown,
    }
}

/// The stereotype guillemet label drawn above a node's title, or `None` for
/// the default (plain `Class`) rendering, which needs no extra line.
pub fn stereotype_label(ty: &ElementType) -> Option<&'static str> {
    match ty {
        ElementType::Uml(UmlMetaclass::Interface) => Some("interface"),
        ElementType::Uml(UmlMetaclass::Enum) => Some("enumeration"),
        ElementType::Uml(UmlMetaclass::DataType) => Some("dataType"),
        ElementType::Uml(UmlMetaclass::Note) => Some("note"),
        ElementType::Uml(UmlMetaclass::Actor) => Some("actor"),
        ElementType::Uml(UmlMetaclass::UseCase) => Some("useCase"),
        ElementType::Uml(UmlMetaclass::Package) => Some("package"),
        ElementType::Uml(UmlMetaclass::Class) | ElementType::Uml(UmlMetaclass::Association) => None,
        ElementType::Behavior(BehaviorKind::Activity) => Some("activity"),
        ElementType::Behavior(BehaviorKind::StateMachine) => Some("stateMachine"),
        ElementType::Behavior(BehaviorKind::Sequence) => Some("sequence"),
        ElementType::Diagram => None,
        ElementType::Unknown(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_class_gets_no_accent_and_no_label() {
        let ty = ElementType::Uml(UmlMetaclass::Class);
        assert_eq!(accent_bucket(&ty), AccentBucket::None);
        assert_eq!(stereotype_label(&ty), None);
    }

    #[test]
    fn interface_gets_its_own_bucket_and_guillemet_label() {
        let ty = ElementType::Uml(UmlMetaclass::Interface);
        assert_eq!(accent_bucket(&ty), AccentBucket::Interface);
        assert_eq!(stereotype_label(&ty), Some("interface"));
    }

    #[test]
    fn enum_and_datatype_share_the_enum_bucket() {
        let e = ElementType::Uml(UmlMetaclass::Enum);
        let d = ElementType::Uml(UmlMetaclass::DataType);
        assert_eq!(accent_bucket(&e), AccentBucket::Enum);
        assert_eq!(accent_bucket(&d), AccentBucket::Enum);
        assert_eq!(stereotype_label(&e), Some("enumeration"));
        assert_eq!(stereotype_label(&d), Some("dataType"));
    }

    #[test]
    fn actor_and_usecase_are_distinct_buckets() {
        let actor = ElementType::Uml(UmlMetaclass::Actor);
        let usecase = ElementType::Uml(UmlMetaclass::UseCase);
        assert_eq!(accent_bucket(&actor), AccentBucket::Actor);
        assert_eq!(accent_bucket(&usecase), AccentBucket::UseCase);
        assert_ne!(accent_bucket(&actor), accent_bucket(&usecase));
    }

    #[test]
    fn note_and_package_get_their_own_buckets() {
        assert_eq!(
            accent_bucket(&ElementType::Uml(UmlMetaclass::Note)),
            AccentBucket::Note
        );
        assert_eq!(
            accent_bucket(&ElementType::Uml(UmlMetaclass::Package)),
            AccentBucket::Package
        );
    }

    #[test]
    fn all_behavior_kinds_share_the_behavior_bucket_but_have_distinct_labels() {
        let activity = ElementType::Behavior(BehaviorKind::Activity);
        let state = ElementType::Behavior(BehaviorKind::StateMachine);
        let seq = ElementType::Behavior(BehaviorKind::Sequence);
        for ty in [&activity, &state, &seq] {
            assert_eq!(accent_bucket(ty), AccentBucket::Behavior);
        }
        assert_eq!(stereotype_label(&activity), Some("activity"));
        assert_eq!(stereotype_label(&state), Some("stateMachine"));
        assert_eq!(stereotype_label(&seq), Some("sequence"));
    }

    #[test]
    fn diagram_and_unknown_types_degrade_gracefully() {
        assert_eq!(accent_bucket(&ElementType::Diagram), AccentBucket::None);
        assert_eq!(stereotype_label(&ElementType::Diagram), None);
        let unknown = ElementType::Unknown("x.Weird".to_string());
        assert_eq!(accent_bucket(&unknown), AccentBucket::Unknown);
        assert_eq!(stereotype_label(&unknown), None);
    }
}
