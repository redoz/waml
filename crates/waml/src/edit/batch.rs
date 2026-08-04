//! The mixed OKF/UML edit batch: `Step`, `Batch`, `apply`, and the invalidation
//! seam that keeps the OKF and UML lowering states in sync as steps apply.

use super::{EditContext, EditError};
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

/// One primitive edit, either an OKF structural operation or a UML domain
/// operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Okf(okf::Op),
    Uml(uml::Op),
}

/// An ordered sequence of [`Step`]s applied as one transaction.
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

/// Applies `batch` to `source`, producing the resulting bundle.
///
/// This is the public entry point for `waml::edit::{Step, Batch}` — the
/// mixed OKF/UML edit surface.
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

/// A candidate-bundle change surfaced between steps, routed to the domain
/// lowering states that were not the origin of the step.
#[derive(Clone, Debug)]
pub enum Invalidation {
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

/// A domain lowering state that can absorb [`Invalidation`] events raised by
/// steps applied in another domain.
pub trait InvalidationSink {
    fn absorb(&mut self, event: &Invalidation) -> Result<(), EditError>;
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
            self.route(family, &event).map_err(|mut error| {
                error.index = index;
                error
            })?;
        }
        Ok(())
    }

    fn finish(self) -> SourceBundle {
        self.candidate
    }

    /// Routes one invalidation event to the domain sinks that did not
    /// originate the step: `TextChanged` always reaches both sinks; every
    /// other event reaches only the sink whose family differs from the
    /// originating step's family.
    fn route(&mut self, origin: StepFamily, event: &Invalidation) -> Result<(), EditError> {
        let text_changed = matches!(event, Invalidation::TextChanged(_));
        let sinks: [(StepFamily, &mut dyn InvalidationSink); 2] = [
            (StepFamily::Okf, &mut self.okf),
            (StepFamily::Uml, &mut self.uml),
        ];
        for (family, sink) in sinks {
            let different_family = !matches!(
                (family, origin),
                (StepFamily::Okf, StepFamily::Okf) | (StepFamily::Uml, StepFamily::Uml)
            );
            if text_changed || different_family {
                sink.absorb(event)?;
            }
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
                "edit.context",
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
) -> Vec<Invalidation> {
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
            events.push(Invalidation::Renamed {
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
            .map(|(path, text)| Invalidation::Removed {
                id: claimed_id(&path, &text),
                path,
            }),
    );
    events.extend(
        inserted
            .into_iter()
            .map(|(path, text)| Invalidation::Inserted {
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
            .map(|(path, _)| Invalidation::TextChanged(path.clone())),
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
