//! The mixed OKF/UML edit batch: `Step`, `Batch`, `apply`, and the invalidation
//! seam that keeps the OKF and UML lowering states in sync as steps apply.

use super::{EditCode, EditContext, EditError};
use crate::okf::lower::OkfLoweringState;
use crate::source::{BundlePath, SourceBundle};
use crate::uml::lower::UmlLoweringState;
use crate::{okf, uml};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use waml_syntax::{
    has_leading_frontmatter_fence, leading_frontmatter_slice, parse_markdown, DocumentRevision,
    MarkdownDialect, OkfMarkdownSyntaxKind, SourceText, SyntaxElement,
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
            return Err(EditError::new(
                EditCode::StaleContext,
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

/// Classification only needs the frontmatter `type` and the concept id, never
/// the full tree — so this parses just the leading frontmatter fence slice
/// (a cheap line scan away, see [`leading_frontmatter_slice`]) instead of the
/// whole document. A directory move of N documents now costs N small parses
/// instead of N full-document parses.
///
/// The slice scanner is deliberately stricter than the parser's frontmatter
/// classifier, and answers `None` for every shape the two could resolve
/// differently (odd fence whitespace, a fence inside a block scalar, an
/// unclosed-but-recovered block). Those fall back to the full parse, so the
/// resolved id is exactly what a full parse yields — the fast path is an
/// optimization, never a behavior change.
fn claimed_id(path: &BundlePath, text: &Arc<String>) -> Option<String> {
    let ty = claimed_type(text)?;
    uml::recognizes_type(&crate::model::ElementType::parse(&ty))
        .then(|| path.concept_id().map(str::to_owned))
        .flatten()
}

/// The frontmatter `type` of `text`, read from the leading frontmatter slice
/// when that slice is provably equivalent and from the whole document
/// otherwise.
fn claimed_type(text: &Arc<String>) -> Option<String> {
    if !has_leading_frontmatter_fence(text.as_str()) {
        return None;
    }
    let shared = match leading_frontmatter_slice(text.as_str()) {
        Some(slice) => Arc::new(slice.to_owned()),
        None => text.clone(),
    };
    frontmatter_type(shared)
}

fn frontmatter_type(shared: Arc<String>) -> Option<String> {
    let source = SourceText::from_shared(shared).ok()?;
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
    Some(ty.clone())
}

fn invalidations(
    before: &BTreeMap<BundlePath, Arc<String>>,
    candidate: &SourceBundle,
) -> Vec<Invalidation> {
    let after = snapshot(candidate);
    let removed: Vec<_> = before
        .iter()
        .filter(|(path, _)| !after.contains_key(*path))
        .map(|(path, text)| (path.clone(), text.clone()))
        .collect();
    let inserted: Vec<_> = after
        .iter()
        .filter(|(path, _)| !before.contains_key(*path))
        .map(|(path, text)| (path.clone(), text.clone()))
        .collect();

    // Rename matching is Arc-pointer identity, not content: index `inserted`
    // by pointer once so each `removed` entry finds its match in O(1) instead
    // of an O(removed * inserted) linear `position` scan (a directory move of
    // N documents would otherwise cost N^2). A queue per pointer preserves
    // "first match by pointer identity, in insertion order" when several
    // inserted entries happen to share the same text pointer.
    let mut inserted_by_ptr: HashMap<*const String, VecDeque<usize>> = HashMap::new();
    for (index, (_, text)) in inserted.iter().enumerate() {
        inserted_by_ptr
            .entry(Arc::as_ptr(text))
            .or_default()
            .push_back(index);
    }

    let mut events = Vec::new();
    let mut matched_inserted: HashSet<usize> = HashSet::new();
    let mut unmatched_removed = Vec::new();
    for (from, from_text) in removed {
        let matched_index = inserted_by_ptr
            .get_mut(&Arc::as_ptr(&from_text))
            .and_then(VecDeque::pop_front);
        if let Some(inserted_index) = matched_index {
            matched_inserted.insert(inserted_index);
            let (to, to_text) = inserted[inserted_index].clone();
            events.push(Invalidation::Renamed {
                id_from: claimed_id(&from, &from_text),
                id_to: claimed_id(&to, &to_text),
                from,
                to,
            });
        } else {
            unmatched_removed.push((from, from_text));
        }
    }
    events.extend(
        unmatched_removed
            .into_iter()
            .map(|(path, text)| Invalidation::Removed {
                id: claimed_id(&path, &text),
                path,
            }),
    );
    events.extend(
        inserted
            .into_iter()
            .enumerate()
            .filter(|(index, _)| !matched_inserted.contains(index))
            .map(|(_, (path, text))| Invalidation::Inserted {
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
    fn malformed_legacy_directories_return_errors_instead_of_panicking() {
        // Mirrors what the legacy bridge's `directory()` helper surfaced for a
        // path-escaping directory: `okf::DirectoryAddress::parse` itself rejects it.
        assert!(okf::DirectoryAddress::parse("/../escape").is_err());
    }

    #[test]
    fn malformed_legacy_import_bundle_returns_error() {
        // Mirrors the legacy bridge's `pkg.insert` mapping: a bundle with a
        // duplicate path is rejected by `SourceBundle::try_from_pairs`.
        let result = SourceBundle::try_from_pairs([
            ("a.md".to_string(), "# A".to_string()),
            ("a.md".to_string(), "# Duplicate".to_string()),
        ]);
        assert!(result.is_err());
    }

    /// A move op must carry the document's text `Arc` across, not rebuild an
    /// equal `String`.
    ///
    /// `invalidations` matches renames by pointer identity, so an op that
    /// reconstructs the text produces a `Removed` plus an `Inserted` that are
    /// individually correct and jointly wrong: the document's identity is
    /// broken, its concept id is retired rather than moved, and every consumer
    /// downstream sees a delete-and-create where the user did a rename. It was
    /// documented and nothing checked it.
    ///
    /// Asserted through the op layer rather than `rename_document`, because
    /// that is where a future op could get it wrong.
    #[test]
    fn every_move_op_preserves_the_text_arc() {
        use crate::edit::{apply, Batch, Step};

        let pairs = [
            (
                "sales/order.md".to_string(),
                "---
type: uml.Class
title: Order
---
# Order
"
                .to_string(),
            ),
            (
                "sales/index.md".to_string(),
                "---
type: okf.Index
title: Sales
---
# Sales
"
                .to_string(),
            ),
        ];

        for (name, step) in [
            (
                "concept.move",
                Step::Okf(crate::okf::Op::ConceptMove {
                    id: "sales/order".into(),
                    to_directory: crate::okf::DirectoryAddress::parse("/archive").unwrap(),
                }),
            ),
            (
                "directory.rename",
                Step::Okf(crate::okf::Op::DirectoryRename {
                    directory: crate::okf::DirectoryAddress::parse("/sales").unwrap(),
                    name: "archive".into(),
                }),
            ),
        ] {
            let source = SourceBundle::try_from_pairs(pairs.iter().cloned()).unwrap();
            let before: BTreeMap<BundlePath, Arc<String>> = source
                .documents()
                .iter()
                .map(|document| (document.path().clone(), document.text_shared().clone()))
                .collect();

            let after = apply(&source, &Batch::new(vec![step])).expect("op applies");
            let events = invalidations(&before, &after);

            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, Invalidation::Renamed { .. })),
                "{name} must report a rename, got {events:?} --                  the op rebuilt the text instead of carrying its Arc across"
            );
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, Invalidation::Removed { .. })),
                "{name} must not retire a document it only moved, got {events:?}"
            );
        }
    }

    #[test]
    fn invalidations_classifies_same_arc_rename_with_resolved_ids() {
        // `invalidations` matches a rename by Arc pointer identity: the same
        // text, present at a removed path and an inserted path. Exercise the
        // HashMap-indexed matching directly and confirm `claimed_id` (now
        // reading only the frontmatter slice) still resolves the concept id
        // on both sides of the rename.
        let mut candidate = SourceBundle::try_from_pairs([(
            "sales/order.md",
            "---\ntype: uml.Class\n---\n# Order\n",
        )])
        .unwrap();
        let before: BTreeMap<BundlePath, Arc<String>> = candidate
            .documents()
            .iter()
            .map(|document| (document.path().clone(), document.text_shared().clone()))
            .collect();

        candidate.rename_document(0, "archive/order.md").unwrap();

        let events = invalidations(&before, &candidate);
        assert_eq!(events.len(), 1);
        match &events[0] {
            Invalidation::Renamed {
                id_from,
                id_to,
                from,
                to,
            } => {
                assert_eq!(from.as_str(), "sales/order.md");
                assert_eq!(to.as_str(), "archive/order.md");
                assert_eq!(id_from.as_deref(), Some("sales/order"));
                assert_eq!(id_to.as_deref(), Some("archive/order"));
            }
            other => panic!("expected a Renamed invalidation, got {other:?}"),
        }
    }

    /// The slice fast path must never change what the full parse claims. Each
    /// case is a frontmatter shape whose boundaries the slice scanner and the
    /// parser's classifier resolve by different rules - the scanner bails out
    /// and `claimed_type` falls back, so both sides must agree exactly.
    #[test]
    fn claimed_type_agrees_with_the_full_parse_for_every_frontmatter_shape() {
        for (label, text) in [
            ("canonical", "---\ntype: uml.Class\n---\n# Order\n"),
            (
                "trailing spaces on both fences",
                "---   \ntype: uml.Class\n---  \n# Order\n",
            ),
            (
                "indented close fence",
                "---\ntype: uml.Class\n  ---\n# Order\n",
            ),
            ("dot close fence", "---\ntype: uml.Class\n...\n# Order\n"),
            (
                "byte order mark",
                "\u{feff}---\ntype: uml.Class\n---\n# Order\n",
            ),
            ("unclosed but recovered", "---\ntype: uml.Class\n"),
            (
                "fence hidden in a block scalar",
                "---\nnote: |\n  ---\ntype: uml.Class\n---\n# Order\n",
            ),
            ("no frontmatter at all", "# Order\n"),
        ] {
            let shared = Arc::new(text.to_owned());
            assert_eq!(
                claimed_type(&shared),
                frontmatter_type(shared.clone()),
                "{label}"
            );
        }
    }

    /// Parity alone would be satisfied by two `None`s: pin that the shapes the
    /// parser does accept still reach a resolved id through `claimed_id`.
    #[test]
    fn claimed_id_resolves_the_shapes_the_parser_accepts() {
        for text in [
            "---\ntype: uml.Class\n---\n# Order\n",
            "---   \ntype: uml.Class\n---  \n# Order\n",
            "---\ntype: uml.Class\n...\n# Order\n",
            "\u{feff}---\ntype: uml.Class\n---\n# Order\n",
        ] {
            let bundle = SourceBundle::try_from_pairs([("sales/order.md", text)]).unwrap();
            let document = &bundle.documents()[0];
            assert_eq!(
                claimed_id(document.path(), document.text_shared()).as_deref(),
                Some("sales/order"),
                "{text:?}"
            );
        }
    }

    #[test]
    fn claimed_id_is_none_without_a_leading_frontmatter_fence() {
        let bundle = SourceBundle::try_from_pairs([("sales/order.md", "# Order\n")]).unwrap();
        let document = &bundle.documents()[0];
        assert_eq!(claimed_id(document.path(), document.text_shared()), None);
    }

    #[test]
    fn legacy_directory_rename_preserves_combined_move_and_rename() {
        let source = SourceBundle::try_from_pairs([(
            "domains/sales/order.md",
            "---\ntype: uml.Class\n---\n# Order\n",
        )])
        .unwrap();
        let batch = Batch::new(vec![Step::Okf(okf::Op::DirectoryMove {
            directory: okf::DirectoryAddress::parse("/domains/sales").unwrap(),
            to_parent: okf::DirectoryAddress::parse("/archive").unwrap(),
            name: Some("commerce".into()),
        })]);
        let changed = crate::edit::apply(&source, &batch).unwrap();
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
        let batch = Batch::new(vec![Step::Okf(okf::Op::DirectoryMove {
            directory: okf::DirectoryAddress::parse("/domains/sales").unwrap(),
            to_parent: okf::DirectoryAddress::parse("/archive").unwrap(),
            name: Some("commerce".into()),
        })]);
        let changed = crate::edit::apply(&source, &batch).unwrap();
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
