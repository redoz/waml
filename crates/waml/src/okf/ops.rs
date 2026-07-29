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
    use crate::edit::{AppliedEdit, EditBatch};

    fn context(source: &SourceBundle) -> EditContext<'_> {
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

    fn apply_reversible(batch: &impl EditBatch, source: &SourceBundle) -> AppliedEdit {
        batch.apply_reversible(context(source)).unwrap()
    }

    fn assert_reversible(source: SourceBundle, batch: Batch, expected: SourceBundle) {
        let applied = apply_reversible(&batch, &source);
        assert_eq!(applied.source, expected);

        let restored = apply_reversible(&applied.inverse, &applied.source);
        assert_eq!(restored.source, source);

        let redone = apply_reversible(&restored.inverse, &restored.source);
        assert_eq!(redone.source, expected);
    }

    #[test]
    fn root_directory_mutations_are_rejected() {
        let source = SourceBundle::default();
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
            assert!(Batch(vec![op]).lower(context(&source)).is_err());
        }
    }

    #[test]
    fn rename_move_and_retitle_have_distinct_effects() {
        let source = SourceBundle::try_from_pairs([(
            "sales/order.md",
            "---\ntype: uml.Class\n---\n# Order\n",
        )])
        .unwrap();
        let sales = DirectoryAddress::parse("/sales").unwrap();

        let renamed = Batch(vec![Op::DirectoryRename {
            directory: sales.clone(),
            name: "commerce".into(),
        }])
        .lower(context(&source))
        .unwrap();
        assert_eq!(renamed.documents()[0].path().as_str(), "commerce/order.md");

        let moved = Batch(vec![Op::DirectoryMove {
            directory: sales.clone(),
            to_parent: DirectoryAddress::parse("/domains").unwrap(),
            name: None,
        }])
        .lower(context(&source))
        .unwrap();
        assert_eq!(
            moved.documents()[0].path().as_str(),
            "domains/sales/order.md"
        );

        let retitled = Batch(vec![Op::IndexRetitle {
            directory: sales,
            title: "Sales Domain".into(),
        }])
        .lower(context(&source))
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
        let result = Batch(vec![Op::ConceptMove {
            id: "sales/order".into(),
            to_directory: DirectoryAddress::parse("/archive").unwrap(),
        }])
        .lower(context(&source));
        assert!(result.is_err());
        assert_eq!(source.documents()[0].path().as_str(), "sales/order.md");
    }

    #[test]
    fn every_okf_operation_round_trips_exact_authored_source() {
        let doc = "---\ntitle: Order\n---\n# Order\n";
        let alpha = "---\ntitle: Alpha\n---\n# Alpha\n";
        let zulu = "---\ntitle: Zulu\n---\n# Zulu\n";
        let index = "# Sales\n\n* [Zulu](./zulu.md)\n* [Alpha](./alpha.md)\n";
        let sorted_index = "# Sales\n\n* [Alpha](./alpha.md)\n* [Zulu](./zulu.md)\n";

        assert_reversible(
            SourceBundle::try_from_pairs([("sales/order.md", doc)]).unwrap(),
            Batch(vec![Op::ConceptMove {
                id: "sales/order".into(),
                to_directory: DirectoryAddress::parse("/archive").unwrap(),
            }]),
            SourceBundle::try_from_pairs([("archive/order.md", doc)]).unwrap(),
        );

        assert_reversible(
            SourceBundle::try_from_pairs([
                ("sales/order.md", doc),
                ("sales/nested/customer.md", "# Customer\n"),
            ])
            .unwrap(),
            Batch(vec![Op::DirectoryRename {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                name: "commerce".into(),
            }]),
            SourceBundle::try_from_pairs([
                ("commerce/order.md", doc),
                ("commerce/nested/customer.md", "# Customer\n"),
            ])
            .unwrap(),
        );

        assert_reversible(
            SourceBundle::try_from_pairs([("sales/order.md", doc)]).unwrap(),
            Batch(vec![Op::DirectoryMove {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                to_parent: DirectoryAddress::parse("/domains").unwrap(),
                name: None,
            }]),
            SourceBundle::try_from_pairs([("domains/sales/order.md", doc)]).unwrap(),
        );

        assert_reversible(
            SourceBundle::try_from_pairs([
                ("keep.md", "# Keep\n"),
                ("sales/order.md", doc),
                ("sales/nested/customer.md", "# Customer\n"),
            ])
            .unwrap(),
            Batch(vec![Op::DirectoryDelete {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                cascade: true,
            }]),
            SourceBundle::try_from_pairs([("keep.md", "# Keep\n")]).unwrap(),
        );

        let indexed_source = || {
            SourceBundle::try_from_pairs([
                ("sales/index.md", index),
                ("sales/zulu.md", zulu),
                ("sales/alpha.md", alpha),
            ])
            .unwrap()
        };
        let sorted_source = || {
            SourceBundle::try_from_pairs([
                ("sales/index.md", sorted_index),
                ("sales/zulu.md", zulu),
                ("sales/alpha.md", alpha),
            ])
            .unwrap()
        };

        assert_reversible(
            indexed_source(),
            Batch(vec![Op::IndexReorder {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                order: vec!["sales/alpha".into(), "sales/zulu".into()],
            }]),
            sorted_source(),
        );

        assert_reversible(
            indexed_source(),
            Batch(vec![Op::IndexSort {
                directory: DirectoryAddress::parse("/sales").unwrap(),
            }]),
            sorted_source(),
        );

        assert_reversible(
            indexed_source(),
            Batch(vec![Op::IndexRetitle {
                directory: DirectoryAddress::parse("/sales").unwrap(),
                title: "Commerce".into(),
            }]),
            SourceBundle::try_from_pairs([
                (
                    "sales/index.md",
                    "# Commerce\n\n* [Zulu](./zulu.md)\n* [Alpha](./alpha.md)\n",
                ),
                ("sales/zulu.md", zulu),
                ("sales/alpha.md", alpha),
            ])
            .unwrap(),
        );

        let imported =
            SourceBundle::try_from_pairs([("template/index.md", "# Template\n")]).unwrap();
        assert_reversible(
            SourceBundle::try_from_pairs([("keep.md", "# Keep\n")]).unwrap(),
            Batch(vec![Op::BundleImport {
                parent: DirectoryAddress::parse("/domains").unwrap(),
                name: "sales".into(),
                bundle: imported,
            }]),
            SourceBundle::try_from_pairs([
                ("keep.md", "# Keep\n"),
                ("domains/sales/index.md", "# Template\n"),
            ])
            .unwrap(),
        );
    }

    #[test]
    fn multi_step_okf_batch_round_trips_as_one_transaction() {
        let doc = "---\ntitle: Order\n---\n# Order\n";
        assert_reversible(
            SourceBundle::try_from_pairs([("sales/order.md", doc)]).unwrap(),
            Batch(vec![
                Op::DirectoryRename {
                    directory: DirectoryAddress::parse("/sales").unwrap(),
                    name: "commerce".into(),
                },
                Op::ConceptMove {
                    id: "commerce/order".into(),
                    to_directory: DirectoryAddress::parse("/archive").unwrap(),
                },
            ]),
            SourceBundle::try_from_pairs([("archive/order.md", doc)]).unwrap(),
        );
    }

    #[test]
    fn late_okf_failure_does_not_publish_source_or_inverse() {
        let source = SourceBundle::try_from_pairs([("sales/order.md", "# Order\n")]).unwrap();
        let batch = Batch(vec![
            Op::ConceptMove {
                id: "sales/order".into(),
                to_directory: DirectoryAddress::parse("/archive").unwrap(),
            },
            Op::DirectoryRename {
                directory: DirectoryAddress::parse("/missing").unwrap(),
                name: "elsewhere".into(),
            },
        ]);

        assert!(batch.apply_reversible(context(&source)).is_err());
        assert_eq!(
            source.to_pairs(),
            vec![("sales/order.md".into(), "# Order\n".into())]
        );
    }
}
