//! Deprecated mixed-domain adapter retained for DTO, CLI, and LSP callers.

use crate::edit::{EditContext, EditError};
use crate::okf::lower::OkfLoweringState;
use crate::source::{BundlePath, SourceBundle};
use crate::uml::lower::UmlLoweringState;
use crate::{okf, uml};
use std::collections::BTreeMap;
use std::sync::Arc;
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, OkfMarkdownSyntaxKind, SourceText,
    SyntaxElement,
};

#[doc(hidden)]
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Okf(okf::Op),
    Uml(uml::Op),
}

#[doc(hidden)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    steps: Vec<Step>,
}

impl Batch {
    pub fn new(steps: Vec<Step>) -> Self {
        Self { steps }
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

fn directory(path: &str) -> Result<okf::DirectoryAddress, EditError> {
    let path = path.trim_matches('/');
    okf::DirectoryAddress::parse(if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    })
    .map_err(|error| EditError::at("directory", error.to_string()))
}

impl TryFrom<crate::ops::Op> for Step {
    type Error = EditError;

    fn try_from(op: crate::ops::Op) -> Result<Self, Self::Error> {
        use crate::ops::Op as Legacy;
        Ok(match op {
            Legacy::AttrAdd {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
            } => Step::Uml(uml::Op::AttributeAdd {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
            }),
            Legacy::AttrSet {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
                rename,
            } => Step::Uml(uml::Op::AttributeSet {
                node,
                name,
                ty_token,
                multiplicity,
                visibility,
                rename,
            }),
            Legacy::AttrRm { node, name } => Step::Uml(uml::Op::AttributeRemove { node, name }),
            Legacy::ValueAdd { node, literal } => Step::Uml(uml::Op::ValueAdd { node, literal }),
            Legacy::ValueRm { node, literal } => Step::Uml(uml::Op::ValueRemove { node, literal }),
            Legacy::RelAdd {
                source,
                kind,
                target,
                name,
                ends,
            } => Step::Uml(uml::Op::RelationshipAdd {
                source,
                kind,
                target,
                name,
                ends,
            }),
            Legacy::RelSet {
                selector,
                ends,
                name,
            } => Step::Uml(uml::Op::RelationshipSet {
                selector: uml::RelationshipSelector::try_from(selector).map_err(|_| {
                    EditError::at(
                        "rel.set",
                        "relationship operation requires a relationship selector",
                    )
                })?,
                ends,
                name,
            }),
            Legacy::RelRm { selector } => Step::Uml(uml::Op::RelationshipRemove {
                selector: uml::RelationshipSelector::try_from(selector).map_err(|_| {
                    EditError::at(
                        "rel.rm",
                        "relationship operation requires a relationship selector",
                    )
                })?,
            }),
            Legacy::NodeNew {
                slug,
                dir,
                ty,
                title,
                stereotype,
                description,
                abstract_,
            } => Step::Uml(uml::Op::ClassifierNew {
                slug,
                directory: directory(&dir)?,
                ty,
                title,
                stereotype,
                description,
                abstract_,
            }),
            Legacy::NodeSet {
                slug,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            } => Step::Uml(uml::Op::ClassifierSet {
                id: slug,
                title,
                description,
                stereotype,
                abstract_,
                ty,
            }),
            Legacy::NodeRm { slug, cascade } => {
                Step::Uml(uml::Op::ClassifierRemove { id: slug, cascade })
            }
            Legacy::NodeRename { from, to } => Step::Uml(uml::Op::ClassifierRename { from, to }),
            Legacy::PkgMove { slug, to_dir } => Step::Okf(okf::Op::ConceptMove {
                id: slug,
                to_directory: directory(&to_dir)?,
            }),
            Legacy::PkgRename { from, to } => {
                let from_parent = from.rsplit_once('/').map_or("", |(parent, _)| parent);
                let to_parent = to.rsplit_once('/').map_or("", |(parent, _)| parent);
                if from_parent != to_parent {
                    Step::Okf(okf::Op::DirectoryMove {
                        directory: directory(&from)?,
                        to_parent: directory(to_parent)?,
                        name: Some(to.rsplit('/').next().unwrap_or(&to).to_string()),
                    })
                } else {
                    Step::Okf(okf::Op::DirectoryRename {
                        directory: directory(&from)?,
                        name: to.rsplit('/').next().unwrap_or(&to).to_string(),
                    })
                }
            }
            Legacy::PkgDelete { path, cascade } => Step::Okf(okf::Op::DirectoryDelete {
                directory: directory(&path)?,
                cascade,
            }),
            Legacy::PkgReorder { path, order } => Step::Okf(okf::Op::IndexReorder {
                directory: directory(&path)?,
                order,
            }),
            Legacy::PkgSort { path } => Step::Okf(okf::Op::IndexSort {
                directory: directory(&path)?,
            }),
            Legacy::PkgRetitle { path, title } => Step::Okf(okf::Op::IndexRetitle {
                directory: directory(&path)?,
                title,
            }),
            Legacy::PkgInsert {
                parent_path,
                name,
                docs,
            } => Step::Okf(okf::Op::BundleImport {
                parent: directory(&parent_path)?,
                name,
                bundle: SourceBundle::try_from_pairs(docs)
                    .map_err(|error| EditError::at("pkg.insert", error.to_string()))?,
            }),
            Legacy::DiagramSet {
                key,
                title,
                description,
                clear_description,
                display,
            } => Step::Uml(uml::Op::DiagramSet {
                key,
                title,
                description,
                clear_description,
                display,
            }),
            Legacy::PlaceSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            } => Step::Uml(uml::Op::PlacementSet {
                diagram,
                subject_title,
                subject_slug,
                reference_title,
                reference_slug,
                directions,
            }),
            Legacy::PlaceRm {
                diagram,
                subject_slug,
                reference_slug,
            } => Step::Uml(uml::Op::PlacementRemove {
                diagram,
                subject_slug,
                reference_slug,
            }),
        })
    }
}

pub(crate) fn step_from_legacy(op: crate::ops::Op) -> Result<Step, EditError> {
    Step::try_from(op)
}

pub fn apply(source: &SourceBundle, batch: &Batch) -> Result<SourceBundle, EditError> {
    let okf = crate::analysis::analyze_okf(source, None, 0).map_err(EditError::from)?;
    let uml = uml::analyze(
        crate::analysis::DomainAnalysisContext {
            source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 0,
        },
        None,
    )
    .map_err(EditError::from)?;
    crate::edit::EditBatch::lower(
        batch,
        EditContext {
            source,
            okf_analysis: &okf,
            session_revision: 0,
            uml: &uml,
        },
    )
}

#[derive(Clone, Debug)]
enum CandidateInvalidation {
    TextChanged(BundlePath),
    Inserted {
        id: Option<String>,
        path: BundlePath,
    },
    Removed {
        id: Option<String>,
        path: BundlePath,
    },
    Renamed {
        id_from: Option<String>,
        id_to: Option<String>,
        from: BundlePath,
        to: BundlePath,
    },
}

#[derive(Clone, Copy)]
enum StepFamily {
    Okf,
    Uml,
}

struct MixedLoweringCursor<'a> {
    original: EditContext<'a>,
    candidate: SourceBundle,
    okf: OkfLoweringState,
    uml: UmlLoweringState,
}

impl<'a> MixedLoweringCursor<'a> {
    fn new(context: EditContext<'a>) -> Self {
        let candidate = context.source.clone();
        let okf = OkfLoweringState::from_context(&context);
        let uml = UmlLoweringState::from_context(&context);
        Self {
            original: context,
            candidate,
            okf,
            uml,
        }
    }

    fn apply(&mut self, index: usize, step: &Step) -> Result<(), EditError> {
        if index == 0 {
            self.validate_context()?;
        }
        let before = snapshot(&self.candidate);
        let family = match step {
            Step::Okf(op) => {
                okf::lower::apply_step(&mut self.candidate, &mut self.okf, index, op)?;
                StepFamily::Okf
            }
            Step::Uml(op) => {
                uml::lower::apply_step(&mut self.candidate, &mut self.uml, index, op)?;
                StepFamily::Uml
            }
        };
        let events = invalidations(&before, &self.candidate);
        for event in events {
            self.propagate_from(family, event).map_err(|mut error| {
                error.index = index;
                error
            })?;
        }
        Ok(())
    }

    fn finish(self) -> SourceBundle {
        self.candidate
    }

    fn propagate(&mut self, event: CandidateInvalidation) -> Result<(), EditError> {
        self.propagate_to_okf(&event)?;
        self.propagate_to_uml(&event)
    }

    fn propagate_from(
        &mut self,
        family: StepFamily,
        event: CandidateInvalidation,
    ) -> Result<(), EditError> {
        match &event {
            CandidateInvalidation::TextChanged(_) => self.propagate(event),
            _ => match family {
                StepFamily::Okf => self.propagate_to_uml(&event),
                StepFamily::Uml => self.propagate_to_okf(&event),
            },
        }
    }

    fn propagate_to_okf(&mut self, event: &CandidateInvalidation) -> Result<(), EditError> {
        match event {
            CandidateInvalidation::TextChanged(path) => self.okf.invalidate_text(path),
            CandidateInvalidation::Inserted { path, .. } => self.okf.inserted(path.clone())?,
            CandidateInvalidation::Removed { path, .. } => self.okf.removed(path),
            CandidateInvalidation::Renamed { from, to, .. } => {
                self.okf.renamed(from, to.clone())?
            }
        }
        Ok(())
    }

    fn propagate_to_uml(&mut self, event: &CandidateInvalidation) -> Result<(), EditError> {
        match event {
            CandidateInvalidation::TextChanged(path) => self.uml.invalidate_text(path),
            CandidateInvalidation::Inserted { id, path } => match id {
                Some(id) => self.uml.inserted_concept(id.clone(), path.clone())?,
                None => self.uml.invalidate_text(path),
            },
            CandidateInvalidation::Removed { id, path } => {
                if let Some(id) = id {
                    self.uml.removed_concept(id);
                }
                self.uml.invalidate_text(path);
            }
            CandidateInvalidation::Renamed {
                id_from,
                id_to,
                from,
                to,
            } => match (id_from, id_to) {
                (Some(from_id), Some(to_id)) => {
                    self.uml
                        .renamed_concept(from_id, to_id.clone(), to.clone())?;
                }
                (Some(from_id), None) => {
                    self.uml.removed_concept(from_id);
                    self.uml.invalidate_text(to);
                }
                (None, Some(to_id)) => {
                    self.uml.inserted_concept(to_id.clone(), to.clone())?;
                }
                (None, None) => {
                    self.uml.invalidate_text(from);
                    self.uml.invalidate_text(to);
                }
            },
        }
        Ok(())
    }

    fn validate_context(&self) -> Result<(), EditError> {
        let catalog = &self.original.okf_analysis.catalog;
        if catalog.session_revision() != self.original.session_revision
            || self.original.uml.session_revision() != self.original.session_revision
            || !Arc::ptr_eq(catalog, self.original.okf_analysis.markdown.catalog())
            || !Arc::ptr_eq(catalog, self.original.uml.syntax.catalog())
            || catalog.documents().len() != self.original.source.len()
        {
            return Err(EditError::at(
                "compat.context",
                "analysis/catalog revision does not match source",
            ));
        }
        Ok(())
    }
}

fn snapshot(source: &SourceBundle) -> BTreeMap<BundlePath, Arc<String>> {
    source
        .documents()
        .iter()
        .map(|document| (document.path().clone(), document.text_arc().clone()))
        .collect()
}

fn claimed_id(path: &BundlePath, text: &Arc<String>) -> Option<String> {
    let source = SourceText::from_shared(text.clone()).ok()?;
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source,
        MarkdownDialect::WAML_DEFAULT,
    )
    .ok()?;
    let frontmatter = snapshot
        .tree()
        .root()
        .children()
        .filter_map(SyntaxElement::into_node)
        .find(|node| node.kind() == OkfMarkdownSyntaxKind::Frontmatter)?;
    let parsed = crate::frontmatter::parse_closed_syntax(&frontmatter)?;
    let crate::frontmatter::FmValue::Str(ty) = parsed.get("type")? else {
        return None;
    };
    uml::recognizes_type(&crate::model::ElementType::parse(ty))
        .then(|| path.concept_id().map(str::to_owned))
        .flatten()
}

fn invalidations(
    before: &BTreeMap<BundlePath, Arc<String>>,
    candidate: &SourceBundle,
) -> Vec<CandidateInvalidation> {
    let after = snapshot(candidate);
    let mut removed: Vec<_> = before
        .iter()
        .filter(|(path, _)| !after.contains_key(*path))
        .map(|(path, text)| (path.clone(), text.clone()))
        .collect();
    let mut inserted: Vec<_> = after
        .iter()
        .filter(|(path, _)| !before.contains_key(*path))
        .map(|(path, text)| (path.clone(), text.clone()))
        .collect();
    let mut events = Vec::new();
    let mut removed_index = 0;
    while removed_index < removed.len() {
        if let Some(inserted_index) = inserted
            .iter()
            .position(|(_, inserted_text)| Arc::ptr_eq(&removed[removed_index].1, inserted_text))
        {
            let (from, _) = removed.remove(removed_index);
            let (to, to_text) = inserted.remove(inserted_index);
            let from_text = before.get(&from).expect("removed path was snapshotted");
            events.push(CandidateInvalidation::Renamed {
                id_from: claimed_id(&from, from_text),
                id_to: claimed_id(&to, &to_text),
                from,
                to,
            });
        } else {
            removed_index += 1;
        }
    }
    events.extend(
        removed
            .into_iter()
            .map(|(path, text)| CandidateInvalidation::Removed {
                id: claimed_id(&path, &text),
                path,
            }),
    );
    events.extend(
        inserted
            .into_iter()
            .map(|(path, text)| CandidateInvalidation::Inserted {
                id: claimed_id(&path, &text),
                path,
            }),
    );
    events.extend(
        after
            .iter()
            .filter(|(path, text)| {
                before
                    .get(*path)
                    .is_some_and(|previous| !Arc::ptr_eq(previous, text))
            })
            .map(|(path, _)| CandidateInvalidation::TextChanged(path.clone())),
    );
    events
}

impl crate::edit::sealed::Sealed for Batch {}

impl crate::edit::EditBatch for Batch {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        let mut cursor = MixedLoweringCursor::new(context);
        for (index, step) in self.steps().iter().enumerate() {
            cursor.apply(index, step)?;
        }
        Ok(cursor.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edit::EditBatch;

    #[test]
    fn malformed_legacy_directories_return_errors_instead_of_panicking() {
        let source = SourceBundle::default();
        let result = crate::ops::apply_source(
            &source,
            &[crate::ops::Op::PkgRetitle {
                path: "../escape".into(),
                title: "Nope".into(),
            }],
        );
        assert!(result.is_err());
    }

    #[test]
    fn malformed_legacy_import_bundle_returns_error() {
        let source = SourceBundle::default();
        let result = crate::ops::apply_source(
            &source,
            &[crate::ops::Op::PkgInsert {
                parent_path: String::new(),
                name: "x".into(),
                docs: vec![
                    ("a.md".into(), "# A".into()),
                    ("a.md".into(), "# Duplicate".into()),
                ],
            }],
        );
        assert!(result.is_err());
    }

    #[test]
    fn legacy_directory_rename_preserves_combined_move_and_rename() {
        let source = SourceBundle::try_from_pairs([(
            "domains/sales/order.md",
            "---\ntype: uml.Class\n---\n# Order\n",
        )])
        .unwrap();
        let changed = crate::ops::apply_source(
            &source,
            &[crate::ops::Op::PkgRename {
                from: "domains/sales".into(),
                to: "archive/commerce".into(),
            }],
        )
        .unwrap();
        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "archive/commerce/order.md"));
    }

    #[test]
    fn combined_rename_ignores_occupied_intermediate_destination() {
        let source = SourceBundle::try_from_pairs([
            (
                "domains/sales/order.md",
                "---\ntype: uml.Class\n---\n# Order\n",
            ),
            ("archive/sales/existing.md", "# Existing\n"),
        ])
        .unwrap();
        let changed = crate::ops::apply_source(
            &source,
            &[crate::ops::Op::PkgRename {
                from: "domains/sales".into(),
                to: "archive/commerce".into(),
            }],
        )
        .unwrap();
        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "archive/commerce/order.md"));
        assert!(changed
            .documents()
            .iter()
            .any(|document| document.path().as_str() == "archive/sales/existing.md"));
    }

    #[test]
    fn mixed_okf_uml_batch_round_trips_as_one_transaction() {
        let source = SourceBundle::try_from_pairs([(
            "sales/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )])
        .unwrap();
        let batch = Batch::new(vec![
            Step::Okf(okf::Op::ConceptMove {
                id: "sales/order".into(),
                to_directory: okf::DirectoryAddress::parse("/archive").unwrap(),
            }),
            Step::Uml(uml::Op::AttributeAdd {
                node: "archive/order".into(),
                name: "id".into(),
                ty_token: "OrderId".into(),
                multiplicity: None,
                visibility: None,
            }),
            Step::Okf(okf::Op::DirectoryRename {
                directory: okf::DirectoryAddress::parse("/archive").unwrap(),
                name: "commerce".into(),
            }),
        ]);
        let prepared = crate::analysis::prepare_candidate(source.clone(), None, 1).unwrap();

        let applied = batch
            .apply_reversible(EditContext {
                source: &source,
                okf_analysis: prepared.okf(),
                session_revision: prepared.revision(),
                uml: prepared.uml(),
            })
            .unwrap();
        assert_eq!(
            applied.source.documents()[0].path().as_str(),
            "commerce/order.md"
        );
        assert!(applied.source.documents()[0]
            .text()
            .contains("- id: OrderId"));

        let applied_prepared =
            crate::analysis::prepare_candidate(applied.source.clone(), None, 2).unwrap();
        let restored = applied
            .inverse
            .apply_reversible(EditContext {
                source: &applied.source,
                okf_analysis: applied_prepared.okf(),
                session_revision: applied_prepared.revision(),
                uml: applied_prepared.uml(),
            })
            .unwrap();
        assert_eq!(restored.source, source);

        let restored_prepared =
            crate::analysis::prepare_candidate(restored.source.clone(), None, 3).unwrap();
        let redone = restored
            .inverse
            .apply_reversible(EditContext {
                source: &restored.source,
                okf_analysis: restored_prepared.okf(),
                session_revision: restored_prepared.revision(),
                uml: restored_prepared.uml(),
            })
            .unwrap();
        assert_eq!(redone.source, applied.source);
    }

    #[test]
    fn late_mixed_batch_failure_publishes_no_source_or_inverse() {
        let source = SourceBundle::try_from_pairs([(
            "sales/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )])
        .unwrap();
        let batch = Batch::new(vec![
            Step::Okf(okf::Op::ConceptMove {
                id: "sales/order".into(),
                to_directory: okf::DirectoryAddress::parse("/archive").unwrap(),
            }),
            Step::Uml(uml::Op::AttributeRemove {
                node: "archive/order".into(),
                name: "missing".into(),
            }),
        ]);
        let prepared = crate::analysis::prepare_candidate(source.clone(), None, 1).unwrap();

        assert!(batch
            .apply_reversible(EditContext {
                source: &source,
                okf_analysis: prepared.okf(),
                session_revision: prepared.revision(),
                uml: prepared.uml(),
            })
            .is_err());
        assert_eq!(
            source.to_pairs(),
            vec![(
                "sales/order.md".into(),
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()
            )]
        );
    }
}
