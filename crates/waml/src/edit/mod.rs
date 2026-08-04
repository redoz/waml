//! The `edit` module is the crate's public edit surface: [`Step`]/[`Batch`]/
//! [`apply`] compose OKF and UML domain operations into one mixed
//! transaction, propagating structural changes between the two domains
//! through the [`Invalidation`]/[`InvalidationSink`] seam.

use reversible::{DeltaBatch, SourceDelta};
use std::sync::Arc;

use crate::source::{BundlePath, SourceBundle, SourceDocument};
use crate::{analysis::OkfAnalysis, host, uml};
use std::fmt;
use waml_syntax::{ChangeMap, DocumentRevision, FullReparseReason, SourceText, TextChange};

mod batch;
mod reversible;

pub use batch::{apply, Batch, Invalidation, InvalidationSink, Step};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError {
    pub index: usize,
    pub op: String,
    pub selector: Option<String>,
    pub reason: String,
}

impl EditError {
    pub(crate) fn at(op: &str, reason: impl Into<String>) -> EditError {
        EditError {
            index: 0,
            op: op.to_string(),
            selector: None,
            reason: reason.into(),
        }
    }

    pub(crate) fn with_sel(mut self, sel: String) -> EditError {
        self.selector = Some(sel);
        self
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "edit step {} ({}) failed: {}",
            self.index, self.op, self.reason
        )
    }
}

impl std::error::Error for EditError {}

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

#[derive(Debug)]
pub enum ExactSourceEditError {
    DocumentNotFound { path: BundlePath },
    BaseIdentityMismatch { path: BundlePath },
    InvalidChanges { reason: FullReparseReason },
    ResultTextMismatch { path: BundlePath },
    Transaction(EditError),
}

impl fmt::Display for ExactSourceEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "exact source edit failed: {self:?}")
    }
}

impl std::error::Error for ExactSourceEditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transaction(error) => Some(error),
            _ => None,
        }
    }
}

pub trait EditBatch: sealed::Sealed {
    fn lower(&self, context: EditContext<'_>) -> Result<SourceBundle, EditError>;

    fn apply_reversible(&self, context: EditContext<'_>) -> Result<AppliedEdit, EditError> {
        let source = self.lower(context)?;
        let inverse = PendingEdit::from_delta(SourceDelta::between(&source, context.source));
        Ok(AppliedEdit { source, inverse })
    }
}

#[derive(Clone)]
pub struct ExactSourceEdit {
    pub document: crate::analysis::DocumentId,
    pub base_revision: DocumentRevision,
    pub changes: Arc<[TextChange]>,
    pub expected_text: SourceText,
}

impl fmt::Debug for ExactSourceEdit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExactSourceEdit")
            .field("document", &self.document)
            .field("base_revision", &self.base_revision)
            .field("changes", &self.changes)
            .field("expected_text_len", &self.expected_text.len())
            .finish()
    }
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
        lower_exact_source_edit(
            context.source,
            document.path(),
            document.text(),
            &self.changes,
            self.expected_text.clone(),
        )
        .map_err(EditError::from)
    }
}

pub fn apply_exact_source_edit(
    source: &SourceBundle,
    path: &BundlePath,
    accepted_base: &SourceText,
    changes: &[TextChange],
    replacement: SourceText,
) -> Result<AppliedEdit, ExactSourceEditError> {
    let updated = lower_exact_source_edit(source, path, accepted_base, changes, replacement)?;
    let inverse = PendingEdit::from_delta(SourceDelta::between(&updated, source));
    Ok(AppliedEdit {
        source: updated,
        inverse,
    })
}

fn lower_exact_source_edit(
    source_bundle: &SourceBundle,
    path: &BundlePath,
    accepted_base: &SourceText,
    changes: &[TextChange],
    replacement_text: SourceText,
) -> Result<SourceBundle, ExactSourceEditError> {
    let source_document = source_bundle
        .document(path)
        .ok_or_else(|| ExactSourceEditError::DocumentNotFound { path: path.clone() })?;
    if !Arc::ptr_eq(source_document.text_shared(), accepted_base.shared()) {
        return Err(ExactSourceEditError::BaseIdentityMismatch { path: path.clone() });
    }
    let change_map = ChangeMap::checked(accepted_base, changes)
        .map_err(|reason| ExactSourceEditError::InvalidChanges { reason })?;
    let mut result = String::with_capacity(change_map.new_len().to_usize());
    let source = accepted_base.shared().as_str();
    let mut copied_through = 0;
    for change in changes {
        let start = change.old_range.start().to_usize();
        let end = change.old_range.end().to_usize();
        result.push_str(&source[copied_through..start]);
        result.push_str(&change.replacement);
        copied_through = end;
    }
    result.push_str(&source[copied_through..]);
    if result.as_bytes() != replacement_text.shared().as_bytes() {
        return Err(ExactSourceEditError::ResultTextMismatch { path: path.clone() });
    }
    let replacement = SourceDocument::from_shared(path.clone(), replacement_text.shared().clone());
    host::replace_document(source_bundle, replacement).map_err(|reason| {
        ExactSourceEditError::Transaction(edit_error("source.document", reason.to_string()))
    })
}

impl From<ExactSourceEditError> for EditError {
    fn from(error: ExactSourceEditError) -> Self {
        match error {
            other @ ExactSourceEditError::DocumentNotFound { .. } => {
                edit_error("source.document", other.to_string())
            }
            other @ ExactSourceEditError::BaseIdentityMismatch { .. } => {
                edit_error("source.base_identity", other.to_string())
            }
            other @ ExactSourceEditError::InvalidChanges { .. } => {
                edit_error("source.change_map", other.to_string())
            }
            other @ ExactSourceEditError::ResultTextMismatch { .. } => {
                edit_error("source.expected_text", other.to_string())
            }
            ExactSourceEditError::Transaction(error) => error,
        }
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
