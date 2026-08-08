use std::{collections::BTreeMap, error::Error, fmt, sync::Arc};

use waml::analysis::{
    prepare_candidate, prepare_candidate_with_markdown_updates, AnalysisError, DocumentId,
    OkfAnalysis, PreparedCandidate, PreviousAnalyses, PromotedMarkdownUpdate,
};
use waml::edit::{
    apply_exact_source_edit, EditBatch, EditContext, EditError, ExactSourceEditError, PendingEdit,
};
use waml::source::{BundlePath, SourceBundle};
use waml_markdown_editor::{
    edit::ProposedMarkdownEdit,
    syntax::{
        reparse_markdown, ChangeMap, DocumentRevision, FullReparseReason, MarkdownSyntaxSnapshot,
        MarkdownSyntaxUpdate, SourceText, TextChange, TextRange, TextSize,
    },
};

use crate::document::EditIntent;
use crate::editor_history::{EditorHistory, HistoryStateId};
use crate::markdown_analysis::{
    CompletionInstall, CompletionInvariantError, SemanticAnalysisCompletion,
    SemanticAnalysisRequest, SemanticFailureDiagnostic, SemanticRevisionStep,
};
use crate::view_history::ViewLocation;
#[cfg(test)]
use crate::view_history::{DocumentLocator, ViewAnchor};

pub struct EditorSession {
    current: Arc<EditorSessionSnapshot>,
    history: EditorHistory,
}

#[derive(Clone)]
pub struct EditorSessionSnapshot {
    pub revision: u64,
    pub source: Arc<SourceBundle>,
    pub persisted_source: Arc<SourceBundle>,
    pub markdown_snapshots: Arc<BTreeMap<DocumentId, Arc<MarkdownSyntaxSnapshot>>>,
    pub okf_analysis: Arc<OkfAnalysis>,
    pub uml_analysis: Arc<waml::uml::Analysis>,
    pub dirty_revision: Option<u64>,
    pub affected_documents: Arc<[DocumentId]>,
    pub affected_diagrams: Arc<[Arc<str>]>,
    pub semantic_diagnostics: Arc<BTreeMap<DocumentId, SemanticFailureDiagnostic>>,
    document_paths: Arc<BTreeMap<DocumentId, BundlePath>>,
    pub(crate) semantic_source: Arc<SourceBundle>,
    pending_semantic_steps: Arc<[SemanticRevisionStep]>,
}

#[derive(Clone)]
pub struct SaveTicket {
    pub snapshot: Arc<EditorSessionSnapshot>,
    pub revision: u64,
    pub history_state: HistoryStateId,
}

pub struct SaveCompletion {
    pub revision: u64,
    pub history_state: HistoryStateId,
    pub result: Result<(), String>,
}

pub enum ExternalReplacement {
    Installed(SessionChange),
    Conflict { dirty_revision: u64 },
    IgnoredStale,
}

pub(crate) fn exact_replacement_change(before: &str, after: &str) -> TextChange {
    let mut prefix = 0;
    for (left, right) in before.chars().zip(after.chars()) {
        if left != right {
            break;
        }
        prefix += left.len_utf8();
    }

    let mut suffix = 0;
    for (left, right) in before[prefix..]
        .chars()
        .rev()
        .zip(after[prefix..].chars().rev())
    {
        if left != right {
            break;
        }
        suffix += left.len_utf8();
    }
    let before_end = before.len() - suffix;
    let after_end = after.len() - suffix;
    TextChange {
        old_range: TextRange::new(
            TextSize::try_from_usize(prefix).expect("source text length was already validated"),
            TextSize::try_from_usize(before_end).expect("source text length was already validated"),
        )
        .expect("ordered UTF-8 replacement bounds form a valid range"),
        replacement: Arc::from(&after[prefix..after_end]),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRangeMapError {
    DocumentNotFound,
    SourceRevisionUnavailable,
    RangeChanged,
}

#[derive(Clone, Copy)]
pub struct EditorSnapshot<'a> {
    pub source: &'a SourceBundle,
    pub okf_analysis: &'a OkfAnalysis,
    pub uml_analysis: &'a waml::uml::Analysis,
    pub revision: u64,
}

impl EditorSessionSnapshot {
    pub fn borrowed(&self) -> EditorSnapshot<'_> {
        EditorSnapshot {
            source: &self.source,
            okf_analysis: &self.okf_analysis,
            uml_analysis: &self.uml_analysis,
            revision: self.revision,
        }
    }

    pub fn markdown_snapshot(&self, document: DocumentId) -> Option<&Arc<MarkdownSyntaxSnapshot>> {
        self.markdown_snapshots.get(&document)
    }

    /// Test-only: swap in a different `persisted_source`, so callers outside
    /// this module can build a snapshot that carries an unsaved new file
    /// (`persisted_source` predates it) without needing access to this
    /// struct's private fields.
    #[cfg(test)]
    pub(crate) fn with_persisted_source(&self, persisted_source: Arc<SourceBundle>) -> Self {
        Self {
            persisted_source,
            ..self.clone()
        }
    }

    /// Map a range from a known source revision to the current Markdown
    /// snapshot. A change that touches the range fails instead of selecting
    /// text that no longer has the same meaning.
    pub fn map_source_range_to_current(
        &self,
        document: DocumentId,
        revision: DocumentRevision,
        range: TextRange,
    ) -> Result<TextRange, SourceRangeMapError> {
        let target = self
            .markdown_snapshot(document)
            .ok_or(SourceRangeMapError::DocumentNotFound)?;
        if revision == target.revision() {
            target
                .text()
                .slice(range)
                .map_err(|_| SourceRangeMapError::RangeChanged)?;
            return Ok(range);
        }

        let baseline = self
            .okf_analysis
            .markdown_snapshot(document)
            .ok_or(SourceRangeMapError::DocumentNotFound)?;
        let mut current = if baseline.revision() == revision {
            baseline.clone()
        } else {
            self.pending_semantic_steps
                .iter()
                .find_map(|step| {
                    (step.promoted.document == document
                        && step.promoted.update.snapshot.revision() == revision)
                        .then(|| step.promoted.update.snapshot.clone())
                })
                .ok_or(SourceRangeMapError::SourceRevisionUnavailable)?
        };
        current
            .text()
            .slice(range)
            .map_err(|_| SourceRangeMapError::RangeChanged)?;
        let mut mapped = range;
        for step in self.pending_semantic_steps.iter() {
            if step.promoted.document != document
                || step.promoted.base_revision < current.revision()
            {
                continue;
            }
            if step.promoted.base_revision != current.revision() {
                return Err(SourceRangeMapError::SourceRevisionUnavailable);
            }
            let map = ChangeMap::checked(current.text(), &step.changes)
                .map_err(|_| SourceRangeMapError::SourceRevisionUnavailable)?;
            mapped = map
                .translate_unchanged(mapped)
                .ok_or(SourceRangeMapError::RangeChanged)?;
            current = step.promoted.update.snapshot.clone();
            if current.revision() == target.revision() {
                return Ok(mapped);
            }
        }
        Err(SourceRangeMapError::SourceRevisionUnavailable)
    }
}

#[derive(Clone)]
pub struct ProposedSourceEdit {
    pub document: DocumentId,
    pub base_revision: DocumentRevision,
    pub changes: Arc<[TextChange]>,
    pub syntax_update: MarkdownSyntaxUpdate,
}

impl fmt::Debug for ProposedSourceEdit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProposedSourceEdit")
            .field("document", &self.document)
            .field("base_revision", &self.base_revision)
            .field("changes", &self.changes)
            .field("syntax_revision", &self.syntax_update.snapshot.revision())
            .finish()
    }
}

impl ProposedSourceEdit {
    pub fn from_local(document: DocumentId, local: ProposedMarkdownEdit) -> Self {
        Self {
            document,
            base_revision: local.edit.base_revision,
            changes: Arc::from(local.edit.changes),
            syntax_update: local.syntax_update,
        }
    }
}

// These variant fields are read only through the derived Debug impl (used by
// `Display` below to render diagnostics); rustc's dead_code lint does not count a
// derive-generated field access as a read, so each field needs a narrow allow.
// They are not scaffolding for an unlanded consumer — Display is already landed.
#[derive(Debug)]
pub enum SourceEditError {
    DocumentNotFound {
        #[allow(dead_code)]
        document: DocumentId,
    },
    DocumentPathInvariant {
        #[allow(dead_code)]
        document: DocumentId,
    },
    StaleBaseRevision {
        #[allow(dead_code)]
        document: DocumentId,
        #[allow(dead_code)]
        base: DocumentRevision,
        #[allow(dead_code)]
        current: DocumentRevision,
    },
    RevisionOverflow {
        #[allow(dead_code)]
        document: DocumentId,
        #[allow(dead_code)]
        current: DocumentRevision,
    },
    InvalidChanges {
        #[allow(dead_code)]
        document: DocumentId,
        #[allow(dead_code)]
        reason: FullReparseReason,
    },
    SyntaxRevisionMismatch {
        #[allow(dead_code)]
        document: DocumentId,
        #[allow(dead_code)]
        expected: DocumentRevision,
        #[allow(dead_code)]
        actual: DocumentRevision,
    },
    ResultTextMismatch {
        #[allow(dead_code)]
        document: DocumentId,
    },
    BaseIdentityMismatch {
        #[allow(dead_code)]
        document: DocumentId,
    },
    Edit(EditError),
}

impl fmt::Display for SourceEditError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "source edit error: {self:?}")
    }
}

impl Error for SourceEditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Edit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EditError> for SourceEditError {
    fn from(error: EditError) -> Self {
        Self::Edit(error)
    }
}

impl Default for EditorSession {
    fn default() -> Self {
        let prepared = prepare_candidate(SourceBundle::default(), None, 0)
            .expect("the empty source bundle must produce valid analyses");
        let current = snapshot_from_prepared(prepared, None, Arc::from([]));
        Self {
            current,
            history: EditorHistory::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionChange {
    pub revision: u64,
    pub source_changed: bool,
    pub okf_changed: bool,
    pub uml_changed: bool,
    pub navigation_changed: bool,
    pub conflicts_changed: bool,
    pub affected_documents: Arc<[DocumentId]>,
    pub affected_diagrams: Arc<[Arc<str>]>,
}

pub struct EditRequest {
    pub intent: EditIntent,
    pub before_location: ViewLocation,
}

pub struct HistoryEffect {
    pub change: SessionChange,
    pub label: String,
    pub location: ViewLocation,
}

impl SessionChange {
    fn full(
        revision: u64,
        affected_documents: Arc<[DocumentId]>,
        affected_diagrams: Arc<[Arc<str>]>,
    ) -> SessionChange {
        SessionChange {
            revision,
            source_changed: true,
            okf_changed: true,
            uml_changed: true,
            navigation_changed: true,
            conflicts_changed: true,
            affected_documents,
            affected_diagrams,
        }
    }

    fn source_only(revision: u64, document: DocumentId) -> SessionChange {
        SessionChange {
            revision,
            source_changed: true,
            okf_changed: false,
            uml_changed: false,
            navigation_changed: false,
            conflicts_changed: false,
            affected_documents: Arc::from([document]),
            affected_diagrams: Arc::from([]),
        }
    }
}

impl EditorSession {
    pub fn snapshot(&self) -> Arc<EditorSessionSnapshot> {
        self.current.clone()
    }

    pub fn replace(&mut self, source: SourceBundle) -> Result<SessionChange, EditError> {
        let next_revision = self.current.revision.wrapping_add(1);
        let prepared = prepare_candidate(source, None, next_revision)?;
        let affected = affected_documents(prepared.source(), prepared.okf());
        self.current = snapshot_from_prepared(prepared, None, affected);
        self.history.reset();
        Ok(SessionChange::full(
            self.current.revision,
            self.current.affected_documents.clone(),
            self.current.affected_diagrams.clone(),
        ))
    }

    pub fn replace_external(
        &mut self,
        document: DocumentId,
        base_revision: DocumentRevision,
        text: String,
    ) -> Result<ExternalReplacement, AnalysisError> {
        self.replace_external_with_preparer(
            document,
            base_revision,
            text,
            prepare_candidate_with_markdown_updates,
        )
    }

    fn replace_external_with_preparer<F>(
        &mut self,
        document: DocumentId,
        base_revision: DocumentRevision,
        text: String,
        prepare: F,
    ) -> Result<ExternalReplacement, AnalysisError>
    where
        F: for<'a> FnOnce(
            SourceBundle,
            PreviousAnalyses<'a>,
            u64,
            Arc<[PromotedMarkdownUpdate]>,
        ) -> Result<PreparedCandidate, AnalysisError>,
    {
        let Some(current_document) = self.current.markdown_snapshot(document) else {
            return Ok(ExternalReplacement::IgnoredStale);
        };
        if current_document.revision() != base_revision {
            return Ok(ExternalReplacement::IgnoredStale);
        }
        if let Some(dirty_revision) = self.current.dirty_revision {
            return Ok(ExternalReplacement::Conflict { dirty_revision });
        }
        let Some(path) = self.current.document_paths.get(&document).cloned() else {
            return Ok(ExternalReplacement::IgnoredStale);
        };
        let bytes = text.len();
        let source_text =
            SourceText::new(text.clone()).map_err(|_| AnalysisError::SourceTooLarge {
                path: path.clone(),
                bytes,
            })?;
        let document_revision =
            base_revision
                .checked_next()
                .ok_or_else(|| AnalysisError::StructuralInvariant {
                    stage: waml::analysis::AnalysisStage::Shell,
                    reason: "external replacement document revision overflow".into(),
                })?;
        let changes: Arc<[TextChange]> = Arc::from([exact_replacement_change(
            current_document.text().shared().as_str(),
            &text,
        )]);
        let candidate = apply_exact_source_edit(
            &self.current.source,
            &path,
            current_document.text(),
            &changes,
            source_text.clone(),
        )
        .map_err(|error| AnalysisError::StructuralInvariant {
            stage: waml::analysis::AnalysisStage::Shell,
            reason: format!("external replacement source ingress failed: {error:?}").into(),
        })?;
        let update = reparse_markdown(current_document, document_revision, source_text, &changes)
            .map_err(|source| AnalysisError::Shell {
            path: path.clone(),
            source,
        })?;
        let next_revision = self.current.revision.wrapping_add(1);
        let prepared = prepare(
            candidate.source,
            PreviousAnalyses {
                okf: &self.current.okf_analysis,
                uml: &self.current.uml_analysis,
            },
            next_revision,
            Arc::from([PromotedMarkdownUpdate {
                document,
                base_revision,
                update,
            }]),
        )?;
        let affected = affected_documents(prepared.source(), prepared.okf());
        self.current = snapshot_from_prepared(prepared, None, affected);
        self.history.reset();
        Ok(ExternalReplacement::Installed(SessionChange::full(
            self.current.revision,
            self.current.affected_documents.clone(),
            self.current.affected_diagrams.clone(),
        )))
    }

    #[cfg(test)]
    pub fn apply<B: EditBatch + 'static>(&mut self, batch: B) -> Result<SessionChange, EditError> {
        self.apply_with_preparer(batch, prepare_candidate)
    }

    #[cfg(test)]
    fn apply_with_preparer<B, F>(
        &mut self,
        batch: B,
        prepare: F,
    ) -> Result<SessionChange, EditError>
    where
        B: EditBatch + 'static,
        F: for<'a> FnOnce(
            SourceBundle,
            Option<PreviousAnalyses<'a>>,
            u64,
        ) -> Result<PreparedCandidate, AnalysisError>,
    {
        let location = ViewLocation {
            document: DocumentLocator::concept(
                "__legacy_edit__",
                waml::view::surface::SurfaceId::markdown(),
            ),
            anchor: ViewAnchor::None,
        };
        self.apply_edit_with_preparer(
            EditRequest {
                intent: EditIntent {
                    edit: PendingEdit::new(batch),
                    label: "Edit".into(),
                    merge_key: None,
                    after_location: Some(location.clone()),
                },
                before_location: location,
            },
            prepare,
        )
    }

    fn apply_pending(
        &mut self,
        edit: &PendingEdit,
    ) -> Result<(SessionChange, PendingEdit), EditError> {
        self.apply_pending_with_preparer(edit, prepare_candidate)
    }

    fn apply_pending_with_preparer<F>(
        &mut self,
        edit: &PendingEdit,
        prepare: F,
    ) -> Result<(SessionChange, PendingEdit), EditError>
    where
        F: for<'a> FnOnce(
            SourceBundle,
            Option<PreviousAnalyses<'a>>,
            u64,
        ) -> Result<PreparedCandidate, AnalysisError>,
    {
        let applied = edit.apply_reversible(EditContext {
            source: &self.current.source,
            okf_analysis: &self.current.okf_analysis,
            session_revision: self.current.revision,
            uml: &self.current.uml_analysis,
        })?;
        let next_revision = self.current.revision.wrapping_add(1);
        let prepared = prepare(
            applied.source,
            Some(PreviousAnalyses {
                okf: &self.current.okf_analysis,
                uml: &self.current.uml_analysis,
            }),
            next_revision,
        )?;
        let affected = changed_documents(&self.current, prepared.source(), prepared.okf());
        self.current = snapshot_from_prepared(
            prepared,
            Some(self.current.persisted_source.clone()),
            affected,
        );
        let revision = self.current.revision;
        self.set_dirty_revision(Some(revision));
        Ok((
            SessionChange::full(
                revision,
                self.current.affected_documents.clone(),
                self.current.affected_diagrams.clone(),
            ),
            applied.inverse,
        ))
    }

    fn apply_edit_with_preparer<F>(
        &mut self,
        request: EditRequest,
        prepare: F,
    ) -> Result<SessionChange, EditError>
    where
        F: for<'a> FnOnce(
            SourceBundle,
            Option<PreviousAnalyses<'a>>,
            u64,
        ) -> Result<PreparedCandidate, AnalysisError>,
    {
        let EditRequest {
            intent:
                EditIntent {
                    edit,
                    label,
                    merge_key,
                    after_location,
                },
            before_location,
        } = request;
        let after_location = after_location.unwrap_or_else(|| before_location.clone());
        let (change, inverse) = self.apply_pending_with_preparer(&edit, prepare)?;
        self.history
            .record_edit(inverse, label, merge_key, before_location, after_location);
        self.sync_dirty_revision();
        Ok(change)
    }

    pub fn apply_edit(&mut self, request: EditRequest) -> Result<SessionChange, EditError> {
        self.apply_edit_with_preparer(request, prepare_candidate)
    }

    pub fn undo(&mut self) -> Result<Option<HistoryEffect>, EditError> {
        let Some(prepared) = self.history.prepare_undo() else {
            return Ok(None);
        };
        let label = prepared.label().to_owned();
        let location = prepared.target_location().clone();
        match self.apply_pending(prepared.edit()) {
            Ok((change, reciprocal)) => {
                let committed = self.history.commit_undo(prepared, reciprocal);
                debug_assert!(committed);
                self.sync_dirty_revision();
                Ok(Some(HistoryEffect {
                    change,
                    label,
                    location,
                }))
            }
            Err(error) => {
                self.history.abort_undo(prepared);
                Err(error)
            }
        }
    }

    pub fn redo(&mut self) -> Result<Option<HistoryEffect>, EditError> {
        let Some(prepared) = self.history.prepare_redo() else {
            return Ok(None);
        };
        let label = prepared.label().to_owned();
        let location = prepared.target_location().clone();
        match self.apply_pending(prepared.edit()) {
            Ok((change, reciprocal)) => {
                let committed = self.history.commit_redo(prepared, reciprocal);
                debug_assert!(committed);
                self.sync_dirty_revision();
                Ok(Some(HistoryEffect {
                    change,
                    label,
                    location,
                }))
            }
            Err(error) => {
                self.history.abort_redo(prepared);
                Err(error)
            }
        }
    }

    // Only used from #[cfg(test)] `bundle()` below; no non-test consumer has landed.
    #[cfg(test)]
    pub fn source(&self) -> &SourceBundle {
        &self.current.source
    }

    #[cfg(test)]
    fn bundle(&self) -> &SourceBundle {
        self.source()
    }

    // Only used from the #[cfg(test)] `mod tests` below; no non-test consumer has landed.
    #[cfg(test)]
    pub fn persisted_bundle(&self) -> &SourceBundle {
        &self.current.persisted_source
    }

    pub fn okf_analysis(&self) -> &OkfAnalysis {
        &self.current.okf_analysis
    }

    pub fn okf(&self) -> &waml::okf::Bundle {
        &self.current.okf_analysis.bundle
    }

    pub fn uml_analysis(&self) -> &waml::uml::Analysis {
        &self.current.uml_analysis
    }

    pub fn uml_projection(&self) -> &waml::uml::Projection {
        &self.current.uml_analysis.projection
    }

    #[cfg(test)]
    fn model(&self) -> &waml::uml::Projection {
        self.uml_projection()
    }

    pub fn revision(&self) -> u64 {
        self.current.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.current.dirty_revision.is_some()
    }

    pub fn save_ticket(&self) -> Option<SaveTicket> {
        self.current.dirty_revision.map(|_| SaveTicket {
            snapshot: self.current.clone(),
            revision: self.current.revision,
            history_state: self.history.current_state(),
        })
    }

    pub fn finish_save(&mut self, completion: SaveCompletion) -> bool {
        if completion.result.is_ok()
            && completion.revision == self.current.revision
            && completion.history_state == self.history.current_state()
        {
            self.history.mark_saved();
            let mut next = (*self.current).clone();
            next.persisted_source = self.current.source.clone();
            next.dirty_revision = None;
            self.current = Arc::new(next);
            true
        } else {
            false
        }
    }

    #[cfg(test)]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[cfg(test)]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    #[cfg(test)]
    pub fn history_state(&self) -> HistoryStateId {
        self.history.current_state()
    }

    pub fn break_edit_merge_group(&mut self) {
        self.history.break_merge_group();
    }

    pub fn promote_source_edit(
        &mut self,
        proposal: ProposedSourceEdit,
        before_location: ViewLocation,
    ) -> Result<(SessionChange, SemanticAnalysisRequest), SourceEditError> {
        let current_snapshot = self.current.markdown_snapshot(proposal.document).ok_or(
            SourceEditError::DocumentNotFound {
                document: proposal.document,
            },
        )?;
        let path = self
            .current
            .document_paths
            .get(&proposal.document)
            .ok_or(SourceEditError::DocumentPathInvariant {
                document: proposal.document,
            })?
            .clone();
        let current_revision = current_snapshot.revision();
        if proposal.base_revision != current_revision {
            return Err(SourceEditError::StaleBaseRevision {
                document: proposal.document,
                base: proposal.base_revision,
                current: current_revision,
            });
        }
        let expected_revision =
            current_revision
                .checked_next()
                .ok_or(SourceEditError::RevisionOverflow {
                    document: proposal.document,
                    current: current_revision,
                })?;
        let syntax_revision = proposal.syntax_update.snapshot.revision();
        if syntax_revision != expected_revision {
            return Err(SourceEditError::SyntaxRevisionMismatch {
                document: proposal.document,
                expected: expected_revision,
                actual: syntax_revision,
            });
        }
        let applied = apply_exact_source_edit(
            &self.current.source,
            &path,
            current_snapshot.text(),
            &proposal.changes,
            proposal.syntax_update.snapshot.text().clone(),
        )
        .map_err(|error| match error {
            ExactSourceEditError::DocumentNotFound { .. } => {
                SourceEditError::DocumentPathInvariant {
                    document: proposal.document,
                }
            }
            ExactSourceEditError::BaseIdentityMismatch { .. } => {
                SourceEditError::BaseIdentityMismatch {
                    document: proposal.document,
                }
            }
            ExactSourceEditError::InvalidChanges { reason } => SourceEditError::InvalidChanges {
                document: proposal.document,
                reason,
            },
            ExactSourceEditError::ResultTextMismatch { .. } => {
                SourceEditError::ResultTextMismatch {
                    document: proposal.document,
                }
            }
            ExactSourceEditError::Transaction(error) => SourceEditError::Edit(error),
        })?;
        let next_session_revision =
            self.current
                .revision
                .checked_add(1)
                .ok_or(SourceEditError::RevisionOverflow {
                    document: proposal.document,
                    current: current_revision,
                })?;
        let previous = self.current.clone();
        let source = Arc::new(applied.source);
        let changes = proposal.changes.clone();
        let promoted = PromotedMarkdownUpdate {
            document: proposal.document,
            base_revision: proposal.base_revision,
            update: proposal.syntax_update.clone(),
        };
        let mut pending_semantic_steps = self.current.pending_semantic_steps.to_vec();
        pending_semantic_steps.push(SemanticRevisionStep {
            session_revision: next_session_revision,
            source: source.clone(),
            promoted,
            changes,
        });
        let pending_semantic_steps: Arc<[SemanticRevisionStep]> = pending_semantic_steps.into();
        let mut markdown_snapshots = (*self.current.markdown_snapshots).clone();
        markdown_snapshots.insert(proposal.document, proposal.syntax_update.snapshot.clone());
        let next = Arc::new(EditorSessionSnapshot {
            revision: next_session_revision,
            source: source.clone(),
            persisted_source: self.current.persisted_source.clone(),
            markdown_snapshots: Arc::new(markdown_snapshots),
            okf_analysis: self.current.okf_analysis.clone(),
            uml_analysis: self.current.uml_analysis.clone(),
            dirty_revision: Some(next_session_revision),
            affected_documents: Arc::from([proposal.document]),
            affected_diagrams: Arc::from([]),
            semantic_diagnostics: self.current.semantic_diagnostics.clone(),
            document_paths: self.current.document_paths.clone(),
            semantic_source: self.current.semantic_source.clone(),
            pending_semantic_steps: pending_semantic_steps.clone(),
        });

        self.history.record_edit(
            applied.inverse,
            "Edit source",
            None,
            before_location.clone(),
            before_location,
        );
        self.current = next;
        let change = SessionChange::source_only(next_session_revision, proposal.document);
        Ok((
            change,
            SemanticAnalysisRequest {
                session_revision: next_session_revision,
                source,
                previous,
                steps: pending_semantic_steps,
            },
        ))
    }

    pub fn install_semantic_completion(
        &mut self,
        completion: SemanticAnalysisCompletion,
    ) -> CompletionInstall {
        if completion.session_revision != self.current.revision
            || !Arc::ptr_eq(&completion.source, &self.current.source)
        {
            return CompletionInstall::IgnoredStale;
        }
        for (document, current) in self.current.markdown_snapshots.iter() {
            let Some(prepared) = completion.prepared.okf().markdown_snapshot(*document) else {
                return CompletionInstall::RejectedInvariant(
                    CompletionInvariantError::MissingMarkdownSnapshot {
                        document: *document,
                    },
                );
            };
            if completion.prepared.diagnostics.contains_key(document) {
                continue;
            }
            if !Arc::ptr_eq(current, prepared) {
                return CompletionInstall::RejectedInvariant(
                    CompletionInvariantError::MarkdownIdentityMismatch {
                        document: *document,
                    },
                );
            }
        }
        let prepared = completion.prepared;
        let revision = prepared.revision;
        if revision != self.current.revision {
            return CompletionInstall::RejectedInvariant(
                CompletionInvariantError::PreparedCandidateMismatch,
            );
        }
        let change = SessionChange {
            revision,
            source_changed: false,
            okf_changed: true,
            uml_changed: true,
            navigation_changed: true,
            conflicts_changed: true,
            affected_documents: prepared.affected.documents.clone(),
            affected_diagrams: prepared.affected.diagrams.clone(),
        };
        self.current = Arc::new(EditorSessionSnapshot {
            revision,
            source: self.current.source.clone(),
            persisted_source: self.current.persisted_source.clone(),
            markdown_snapshots: self.current.markdown_snapshots.clone(),
            okf_analysis: prepared.okf_analysis.clone(),
            uml_analysis: prepared.uml_analysis.clone(),
            dirty_revision: self.current.dirty_revision,
            affected_documents: prepared.affected.documents.clone(),
            affected_diagrams: prepared.affected.diagrams.clone(),
            semantic_diagnostics: prepared.diagnostics.clone(),
            document_paths: self.current.document_paths.clone(),
            semantic_source: prepared.semantic_source.clone(),
            pending_semantic_steps: Arc::from([]),
        });
        CompletionInstall::Installed(change)
    }

    fn sync_dirty_revision(&mut self) {
        let dirty = (!self.history.is_saved()).then_some(self.current.revision);
        self.set_dirty_revision(dirty);
    }

    fn set_dirty_revision(&mut self, dirty_revision: Option<u64>) {
        if self.current.dirty_revision == dirty_revision {
            return;
        }
        let mut next = (*self.current).clone();
        next.dirty_revision = dirty_revision;
        self.current = Arc::new(next);
    }
}

fn markdown_snapshots(
    source: &SourceBundle,
    okf: &OkfAnalysis,
) -> BTreeMap<DocumentId, Arc<MarkdownSyntaxSnapshot>> {
    source
        .documents()
        .iter()
        .filter_map(|document| {
            let id = okf.catalog.id_for_path(document.path())?;
            Some((id, okf.markdown_snapshot(id)?.clone()))
        })
        .collect()
}

fn affected_documents(source: &SourceBundle, okf: &OkfAnalysis) -> Arc<[DocumentId]> {
    Arc::from(
        source
            .documents()
            .iter()
            .filter_map(|document| okf.catalog.id_for_path(document.path()))
            .collect::<Vec<_>>(),
    )
}

fn changed_documents(
    previous: &EditorSessionSnapshot,
    source: &SourceBundle,
    okf: &OkfAnalysis,
) -> Arc<[DocumentId]> {
    Arc::from(
        source
            .documents()
            .iter()
            .filter_map(|document| {
                let id = okf.catalog.id_for_path(document.path())?;
                let changed = previous
                    .source
                    .documents()
                    .iter()
                    .find(|previous| previous.path() == document.path())
                    .map_or(true, |previous| previous.text() != document.text());
                changed.then_some(id)
            })
            .collect::<Vec<_>>(),
    )
}

fn snapshot_from_prepared(
    prepared: PreparedCandidate,
    persisted_source: Option<Arc<SourceBundle>>,
    affected_documents: Arc<[DocumentId]>,
) -> Arc<EditorSessionSnapshot> {
    let markdown_snapshots = markdown_snapshots(prepared.source(), prepared.okf());
    let document_paths = prepared
        .source()
        .documents()
        .iter()
        .filter_map(|document| {
            prepared
                .okf()
                .catalog
                .id_for_path(document.path())
                .map(|id| (id, document.path().clone()))
        })
        .collect();
    let affected_diagrams = prepared.uml().affected().diagrams.clone();
    let (source, okf_analysis, uml_analysis, revision) = prepared.into_parts();
    let source = Arc::new(source);
    Arc::new(EditorSessionSnapshot {
        revision,
        persisted_source: persisted_source.unwrap_or_else(|| source.clone()),
        source: source.clone(),
        markdown_snapshots: Arc::new(markdown_snapshots),
        okf_analysis: Arc::new(okf_analysis),
        uml_analysis: Arc::new(uml_analysis),
        dirty_revision: None,
        affected_documents,
        affected_diagrams,
        semantic_diagnostics: Arc::new(BTreeMap::new()),
        document_paths: Arc::new(document_paths),
        semantic_source: source.clone(),
        pending_semantic_steps: Arc::from([]),
    })
}

#[cfg(test)]
mod tests;
