use crate::edit::{EditBatch, EditContext, EditError};
use crate::source::SourceBundle;

use super::DirectoryAddress;

#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    ConceptMove {
        id: String,
        to_directory: DirectoryAddress,
    },
    DirectoryRename {
        directory: DirectoryAddress,
        name: String,
    },
    DirectoryMove {
        directory: DirectoryAddress,
        to_parent: DirectoryAddress,
        name: Option<String>,
    },
    DirectoryDelete {
        directory: DirectoryAddress,
        cascade: bool,
    },
    IndexReorder {
        directory: DirectoryAddress,
        order: Vec<String>,
    },
    IndexSort {
        directory: DirectoryAddress,
    },
    IndexRetitle {
        directory: DirectoryAddress,
        title: String,
    },
    BundleImport {
        parent: DirectoryAddress,
        name: String,
        bundle: SourceBundle,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch(pub Vec<Op>);

impl crate::edit::sealed::Sealed for Batch {}

impl EditBatch for Batch {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        let mut cursor = super::lower::OkfLoweringCursor::new(context);
        for (index, op) in self.0.iter().enumerate() {
            cursor.apply(index, op)?;
        }
        Ok(cursor.finish())
    }
}

pub(crate) fn legacy_path(directory: &DirectoryAddress) -> String {
    directory.as_str().trim_start_matches('/').to_string()
}

pub(crate) fn lower_one(work: &mut SourceBundle, op: &Op) -> Result<(), EditError> {
    let mut state = super::lower::OkfLoweringState::from_source(work);
    lower_one_with_state(work, &mut state, op)
}

pub(crate) fn lower_one_with_state(
    work: &mut SourceBundle,
    state: &mut super::lower::OkfLoweringState,
    op: &Op,
) -> Result<(), EditError> {
    match op {
        Op::ConceptMove { id, to_directory } => {
            super::lower::op_pkg_move(work, id, &legacy_path(to_directory))
        }
        Op::DirectoryRename { directory, name } => {
            let from = legacy_path(directory);
            let to = match from.rsplit_once('/') {
                Some((parent, _)) => format!("{parent}/{name}"),
                None => name.clone(),
            };
            super::lower::op_pkg_rename(work, &from, &to)
        }
        Op::DirectoryMove {
            directory,
            to_parent,
            name,
        } => {
            let from = legacy_path(directory);
            let name = name
                .as_deref()
                .unwrap_or_else(|| from.rsplit('/').next().unwrap_or_default());
            let parent = legacy_path(to_parent);
            let to = if parent.is_empty() {
                name.to_string()
            } else {
                format!("{parent}/{name}")
            };
            super::lower::op_pkg_rename(work, &from, &to)
        }
        Op::DirectoryDelete { directory, cascade } => {
            super::lower::op_pkg_delete(work, &legacy_path(directory), *cascade)
        }
        Op::IndexReorder { directory, order } => {
            super::lower::op_pkg_reorder(work, state, &legacy_path(directory), order)
        }
        Op::IndexSort { directory } => {
            super::lower::op_pkg_sort(work, state, &legacy_path(directory))
        }
        Op::IndexRetitle { directory, title } => {
            super::lower::op_pkg_retitle(work, state, &legacy_path(directory), title)
        }
        Op::BundleImport {
            parent,
            name,
            bundle,
        } => super::lower::op_pkg_insert(work, &legacy_path(parent), name, &bundle.to_pairs()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::EditBatch;

    fn context<'a>(
        source: &'a SourceBundle,
        _okf: &'a crate::okf::Bundle,
        _uml: &'a crate::uml::Projection,
    ) -> EditContext<'a> {
        let okf_analysis = Box::leak(Box::new(
            crate::analysis::analyze_okf(source, None, 0).unwrap(),
        ));
        let uml = Box::leak(Box::new(
            crate::uml::analyze(
                crate::analysis::DomainAnalysisContext {
                    source,
                    catalog: &okf_analysis.catalog,
                    shell: &okf_analysis.shell,
                    structures: &okf_analysis.structures,
                    okf: &okf_analysis.bundle,
                    session_revision: 0,
                },
                None,
            )
            .unwrap(),
        ));
        EditContext {
            source,
            okf_analysis,
            session_revision: 0,
            uml,
        }
    }

    #[test]
    fn root_directory_mutations_are_rejected() {
        let source = SourceBundle::default();
        let okf = crate::okf::Bundle::parse(&source).unwrap();
        let uml = crate::uml::project(&okf);
        let root = DirectoryAddress::parse("/").unwrap();
        for op in [
            Op::DirectoryRename {
                directory: root.clone(),
                name: "x".into(),
            },
            Op::DirectoryMove {
                directory: root.clone(),
                to_parent: root.clone(),
                name: None,
            },
            Op::DirectoryDelete {
                directory: root.clone(),
                cascade: true,
            },
        ] {
            assert!(Batch(vec![op]).lower(context(&source, &okf, &uml)).is_err());
        }
    }

    #[test]
    fn rename_move_and_retitle_have_distinct_effects() {
        let source = SourceBundle::try_from_pairs([(
            "sales/order.md",
            "---\ntype: uml.Class\n---\n# Order\n",
        )])
        .unwrap();
        let okf = crate::okf::Bundle::parse(&source).unwrap();
        let uml = crate::uml::project(&okf);
        let sales = DirectoryAddress::parse("/sales").unwrap();

        let renamed = Batch(vec![Op::DirectoryRename {
            directory: sales.clone(),
            name: "commerce".into(),
        }])
        .lower(context(&source, &okf, &uml))
        .unwrap();
        assert_eq!(renamed.documents()[0].path().as_str(), "commerce/order.md");

        let moved = Batch(vec![Op::DirectoryMove {
            directory: sales.clone(),
            to_parent: DirectoryAddress::parse("/domains").unwrap(),
            name: None,
        }])
        .lower(context(&source, &okf, &uml))
        .unwrap();
        assert_eq!(
            moved.documents()[0].path().as_str(),
            "domains/sales/order.md"
        );

        let retitled = Batch(vec![Op::IndexRetitle {
            directory: sales,
            title: "Sales Domain".into(),
        }])
        .lower(context(&source, &okf, &uml))
        .unwrap();
        assert!(retitled
            .documents()
            .iter()
            .any(|doc| doc.path().as_str() == "sales/index.md"));
        assert_eq!(source.len(), 1);
    }

    #[test]
    fn collision_is_detected_before_source_changes() {
        let source = SourceBundle::try_from_pairs([
            ("sales/order.md", "# Order\n"),
            ("archive/order.md", "# Existing\n"),
        ])
        .unwrap();
        let okf = crate::okf::Bundle::parse(&source).unwrap();
        let uml = crate::uml::project(&okf);
        let result = Batch(vec![Op::ConceptMove {
            id: "sales/order".into(),
            to_directory: DirectoryAddress::parse("/archive").unwrap(),
        }])
        .lower(context(&source, &okf, &uml));
        assert!(result.is_err());
        assert_eq!(source.documents()[0].path().as_str(), "sales/order.md");
    }
}
