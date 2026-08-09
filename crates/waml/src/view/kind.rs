//! `RowKind`: the ten presentational kinds a row can carry, lifted from
//! `waml-editor`'s `NavCategory`/`kind_of` so headless code can stamp an icon
//! without depending on the editor crate.

use crate::model::{BehaviorKind, ElementType, UmlMetaclass};

/// The ten kinds a row's element type maps to. Mirrors
/// `waml_editor::document::NavCategory` variant for variant — the editor
/// gains a `From<RowKind>` in Task 7 rather than duplicating the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Directory,
    OkfDocument,
    Class,
    Interface,
    Enum,
    DataType,
    Diagram,
    Behavior,
    Sequence,
    Note,
}

impl RowKind {
    /// Every `RowKind` variant, in declaration order. The one place that
    /// enumerates the ten kinds outside a `match` -- callers that need "all
    /// of them" (an icon table, an exhaustiveness test) iterate this instead
    /// of hand-listing the variants a second time.
    pub const ALL: [RowKind; 10] = [
        RowKind::Directory,
        RowKind::OkfDocument,
        RowKind::Class,
        RowKind::Interface,
        RowKind::Enum,
        RowKind::DataType,
        RowKind::Diagram,
        RowKind::Behavior,
        RowKind::Sequence,
        RowKind::Note,
    ];

    /// The shipped `IconId` name for this kind, kebab-case.
    pub fn as_icon_name(&self) -> &'static str {
        match self {
            RowKind::Directory => "directory",
            RowKind::OkfDocument => "okf-document",
            RowKind::Class => "class",
            RowKind::Interface => "interface",
            RowKind::Enum => "enum",
            RowKind::DataType => "data-type",
            RowKind::Diagram => "diagram",
            RowKind::Behavior => "behavior",
            RowKind::Sequence => "sequence",
            RowKind::Note => "note",
        }
    }
}

/// Map a resolved element type to its `RowKind`. Lifted **verbatim** from
/// `waml-editor::tree::kind_of` — same arms, same order — so the two agree by
/// construction, checked by `kind_of_agrees_with_the_editors_arms` below.
pub fn kind_of(ty: &ElementType) -> RowKind {
    match ty {
        ElementType::Uml(UmlMetaclass::Package) => RowKind::Directory,
        ElementType::Uml(UmlMetaclass::Note) => RowKind::Note,
        ElementType::Uml(UmlMetaclass::Interface) => RowKind::Interface,
        ElementType::Uml(UmlMetaclass::Enum) => RowKind::Enum,
        ElementType::Uml(UmlMetaclass::DataType) => RowKind::DataType,
        ElementType::Uml(
            UmlMetaclass::Class
            | UmlMetaclass::Association
            | UmlMetaclass::Actor
            | UmlMetaclass::UseCase
            | UmlMetaclass::InstanceSpecification,
        ) => RowKind::Class,
        ElementType::Behavior(BehaviorKind::Sequence) => RowKind::Sequence,
        ElementType::Behavior(_) => RowKind::Behavior,
        ElementType::Diagram(_) => RowKind::Diagram,
        ElementType::Unknown(_) => RowKind::OkfDocument,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The moved-code regression check named in the spec's Risks: `kind_of`
    /// agrees with `waml-editor::tree`'s arms variant for variant. Encoded
    /// here as the same inputs `tree.rs`'s own tests exercise, since this
    /// crate cannot depend on `waml-editor`.
    #[test]
    fn kind_of_agrees_with_the_editors_arms() {
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Package)),
            RowKind::Directory
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Note)),
            RowKind::Note
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Interface)),
            RowKind::Interface
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Enum)),
            RowKind::Enum
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::DataType)),
            RowKind::DataType
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Class)),
            RowKind::Class
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Association)),
            RowKind::Class
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::Actor)),
            RowKind::Class
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::UseCase)),
            RowKind::Class
        );
        assert_eq!(
            kind_of(&ElementType::Uml(UmlMetaclass::InstanceSpecification)),
            RowKind::Class
        );
        assert_eq!(
            kind_of(&ElementType::Behavior(BehaviorKind::Sequence)),
            RowKind::Sequence
        );
        assert_eq!(
            kind_of(&ElementType::Diagram(crate::model::DiagramKind::Class)),
            RowKind::Diagram
        );
    }

    #[test]
    fn as_icon_name_is_distinct_and_kebab_case_across_all_ten() {
        let all = [
            RowKind::Directory,
            RowKind::OkfDocument,
            RowKind::Class,
            RowKind::Interface,
            RowKind::Enum,
            RowKind::DataType,
            RowKind::Diagram,
            RowKind::Behavior,
            RowKind::Sequence,
            RowKind::Note,
        ];
        let names: Vec<&str> = all.iter().map(RowKind::as_icon_name).collect();
        for name in &names {
            assert!(
                name.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "not kebab-case: {name}"
            );
        }
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "icon names must be distinct");
    }
}
