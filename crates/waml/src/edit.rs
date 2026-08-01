use reversible::{DeltaBatch, SourceDelta};
use std::sync::Arc;

use crate::source::{SourceBundle, SourceDocument};
use crate::{analysis::OkfAnalysis, host, uml};
use std::fmt;
use waml_syntax::{ChangeMap, DocumentRevision, SourceText, TextChange};

mod reversible;

pub type EditError = crate::ops::OpError;

impl fmt::Display for crate::ops::OpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "edit step {} ({}) failed: {}",
            self.index, self.op, self.reason
        )
    }
}

impl std::error::Error for crate::ops::OpError {}

#[derive(Clone, Copy)]
pub struct EditContext<'a> {
    pub source: &'a SourceBundle,
    pub okf_analysis: &'a OkfAnalysis,
    pub session_revision: u64,
    pub uml: &'a uml::Analysis,
}

pub(crate) mod sealed {
    pub trait Sealed {}
}

pub struct AppliedEdit {
    pub source: SourceBundle,
    pub inverse: PendingEdit,
}

pub trait EditBatch: sealed::Sealed {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError>;

    fn apply_reversible(&self, context: EditContext<'_>) -> Result<AppliedEdit, EditError> {
        let source = self.lower(context)?;
        let inverse = PendingEdit::from_delta(SourceDelta::between(&source, context.source));
        Ok(AppliedEdit { source, inverse })
    }
}

#[derive(Clone, Debug)]
pub struct ExactSourceEdit {
    pub document: crate::analysis::DocumentId,
    pub base_revision: DocumentRevision,
    pub changes: Arc<[TextChange]>,
    pub expected_text: SourceText,
}

impl sealed::Sealed for ExactSourceEdit {}

impl EditBatch for ExactSourceEdit {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        let document = context
            .okf_analysis
            .catalog
            .document(self.document)
            .ok_or_else(|| edit_error("source.document", "document is not in the catalog"))?;
        if document.revision() != self.base_revision {
            return Err(edit_error(
                "source.base_revision",
                "document revision does not match the edit base",
            ));
        }

        let change_map = ChangeMap::checked(document.text(), &self.changes)
            .map_err(|reason| edit_error("source.change_map", format!("{reason:?}")))?;
        let mut result = String::with_capacity(change_map.new_len().to_usize());
        let source = document.text().shared().as_str();
        let mut copied_through = 0;
        for change in self.changes.iter() {
            let start = change.old_range.start().to_usize();
            let end = change.old_range.end().to_usize();
            result.push_str(&source[copied_through..start]);
            result.push_str(&change.replacement);
            copied_through = end;
        }
        result.push_str(&source[copied_through..]);
        if result.as_bytes() != self.expected_text.shared().as_bytes() {
            return Err(edit_error(
                "source.expected_text",
                "changes do not produce the expected text",
            ));
        }

        let replacement = SourceDocument::from_shared(
            document.path().clone(),
            self.expected_text.shared().clone(),
        );
        host::replace_document(context.source, replacement)
            .map_err(|reason| edit_error("source.document", reason.to_string()))
    }
}

fn edit_error(op: &str, reason: impl Into<String>) -> EditError {
    EditError {
        index: 0,
        op: op.into(),
        selector: None,
        reason: reason.into(),
    }
}

#[derive(Clone)]
pub struct PendingEdit(Arc<dyn EditBatch>);

impl PendingEdit {
    pub fn new(batch: impl EditBatch + 'static) -> Self {
        Self(Arc::new(batch))
    }

    /// Compose reciprocal edits that have already been lowered against their
    /// original intermediate states.
    ///
    /// This is intentionally not a general operation-batch API: projection
    /// data in `EditContext` is not rebuilt between children. History uses it
    /// only for compact source deltas, applied in reciprocal order.
    pub fn sequence_reciprocals(edits: Vec<PendingEdit>) -> Self {
        Self::new(SequenceBatch(edits))
    }

    pub(crate) fn from_delta(delta: SourceDelta) -> Self {
        Self::new(DeltaBatch(delta))
    }
}

impl sealed::Sealed for PendingEdit {}

impl EditBatch for PendingEdit {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        self.0.lower(context)
    }

    fn apply_reversible(&self, context: EditContext<'_>) -> Result<AppliedEdit, EditError> {
        self.0.apply_reversible(context)
    }
}

struct SequenceBatch(Vec<PendingEdit>);

impl sealed::Sealed for SequenceBatch {}

impl EditBatch for SequenceBatch {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError> {
        let mut source = context.source.clone();
        for edit in &self.0 {
            source = edit.lower(EditContext {
                source: &source,
                okf_analysis: context.okf_analysis,
                session_revision: context.session_revision,
                uml: context.uml,
            })?;
        }
        Ok(source)
    }

    fn apply_reversible(&self, context: EditContext<'_>) -> Result<AppliedEdit, EditError> {
        let mut source = context.source.clone();
        let mut inverses = Vec::with_capacity(self.0.len());
        for edit in &self.0 {
            let applied = edit.apply_reversible(EditContext {
                source: &source,
                okf_analysis: context.okf_analysis,
                session_revision: context.session_revision,
                uml: context.uml,
            })?;
            source = applied.source;
            inverses.push(applied.inverse);
        }
        inverses.reverse();
        Ok(AppliedEdit {
            source,
            inverse: PendingEdit::sequence_reciprocals(inverses),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::reversible::SourceDelta;
    use super::{EditBatch, EditContext, PendingEdit};
    use crate::source::SourceBundle;

    fn apply(edit: &PendingEdit, source: &SourceBundle) -> super::AppliedEdit {
        let okf_analysis = Box::leak(Box::new(
            crate::analysis::analyze_okf(source, None, 0).unwrap(),
        ));
        let uml = Box::leak(Box::new(
            crate::uml::analyze(
                crate::analysis::DomainAnalysisContext {
                    source,
                    catalog: &okf_analysis.catalog,
                    markdown: &okf_analysis.markdown,
                    okf: &okf_analysis.bundle,
                    session_revision: 0,
                },
                None,
            )
            .unwrap(),
        ));
        edit.apply_reversible(EditContext {
            source,
            okf_analysis,
            session_revision: 0,
            uml,
        })
        .unwrap()
    }

    #[test]
    fn source_delta_round_trips_one_character_in_a_large_document() {
        let body = "x".repeat(5_000);
        let before = SourceBundle::try_from_pairs([
            ("customer.md", format!("# Customer\n{body}")),
            ("untouched.md", "# Untouched\n".to_string()),
        ])
        .unwrap();
        let after = SourceBundle::try_from_pairs([
            ("customer.md", format!("# Customerr\n{body}")),
            ("untouched.md", "# Untouched\n".to_string()),
        ])
        .unwrap();

        let delta = SourceDelta::between(&before, &after);
        assert_eq!(delta.stored_text_bytes(), 1);

        let (applied, inverse) = delta.apply(&before).unwrap();
        assert_eq!(applied, after);
        assert!(before.shares_text_with(&applied, "untouched.md"));

        let (restored, reciprocal) = inverse.apply(&applied).unwrap();
        assert_eq!(restored, before);

        let (redone, _) = reciprocal.apply(&restored).unwrap();
        assert_eq!(redone, after);
    }

    #[test]
    fn source_delta_round_trips_structural_document_changes() {
        let before =
            SourceBundle::try_from_pairs([("a.md", "# A\n"), ("folder/b.md", "# B\n")]).unwrap();
        let after = SourceBundle::try_from_pairs([
            ("renamed/a.md", "# A\n"),
            ("inserted.md", "# Inserted\n"),
            ("folder/b.md", "# B changed\n"),
        ])
        .unwrap();

        let delta = SourceDelta::between(&before, &after);
        let (applied, inverse) = delta.apply(&before).unwrap();
        assert_eq!(applied, after);

        let (restored, reciprocal) = inverse.apply(&applied).unwrap();
        assert_eq!(restored, before);

        let (redone, _) = reciprocal.apply(&restored).unwrap();
        assert_eq!(redone, after);
    }

    #[test]
    fn pending_edit_sequence_returns_reverse_order_reciprocals() {
        let before = SourceBundle::try_from_pairs([("customer.md", "# Customer\n")]).unwrap();
        let middle = SourceBundle::try_from_pairs([("customer.md", "# Customerr\n")]).unwrap();
        let after =
            SourceBundle::try_from_pairs([("renamed/customer.md", "# Customerr\n")]).unwrap();

        let edit = PendingEdit::sequence_reciprocals(vec![
            PendingEdit::from_delta(SourceDelta::between(&before, &middle)),
            PendingEdit::from_delta(SourceDelta::between(&middle, &after)),
        ]);

        let applied = apply(&edit, &before);
        assert_eq!(applied.source, after);

        let restored = apply(&applied.inverse, &applied.source);
        assert_eq!(restored.source, before);

        let redone = apply(&restored.inverse, &restored.source);
        assert_eq!(redone.source, after);
    }
}
