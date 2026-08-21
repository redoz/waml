use std::{collections::BTreeMap, fmt, sync::Arc};

use waml::analysis::{
    prepare_candidate, prepare_candidate_with_markdown_recovery,
    prepare_candidate_with_markdown_updates, semantic_source_with_promoted_document,
    AffectedAnalysis, AnalysisError, DocumentId, OkfAnalysis, PreparedCandidate, PreviousAnalyses,
    PromotedMarkdownUpdate,
};
use waml::source::SourceBundle;

use crate::editor_session::{EditorSessionSnapshot, SessionChange};

#[derive(Clone)]
pub struct SemanticRevisionStep {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub promoted: PromotedMarkdownUpdate,
    pub changes: Arc<[waml_markdown_editor::syntax::TextChange]>,
}

#[derive(Clone)]
pub struct SemanticAnalysisRequest {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub previous: Arc<EditorSessionSnapshot>,
    pub steps: Arc<[SemanticRevisionStep]>,
}

#[derive(Clone)]
pub struct SemanticAnalysisCompletion {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub prepared: Arc<PreparedSemanticSnapshot>,
}

/// Why one document failed semantic analysis. Recorded per document in
/// [`PreparedSemanticSnapshot::diagnostics`] but not yet rendered anywhere --
/// the consumer is the editor's diagnostics surface, which has not landed. The
/// per-field allows below are what remains of a file-wide blanket allow.
#[derive(Clone)]
pub struct SemanticFailureDiagnostic {
    #[allow(dead_code)] // consumer: the editor diagnostics surface
    pub document: DocumentId,
    #[allow(dead_code)] // consumer: the editor diagnostics surface
    pub session_revision: u64,
    #[allow(dead_code)] // consumer: the editor diagnostics surface
    pub error: Arc<AnalysisError>,
}

pub struct PreparedSemanticSnapshot {
    pub semantic_source: Arc<SourceBundle>,
    pub okf_analysis: Arc<OkfAnalysis>,
    pub uml_analysis: Arc<waml::uml::Analysis>,
    pub revision: u64,
    pub affected: AffectedAnalysis,
    pub diagnostics: Arc<BTreeMap<DocumentId, SemanticFailureDiagnostic>>,
}

impl PreparedSemanticSnapshot {
    pub fn okf(&self) -> &OkfAnalysis {
        &self.okf_analysis
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionInvariantError {
    MissingMarkdownSnapshot { document: DocumentId },
    MarkdownIdentityMismatch { document: DocumentId },
    PreparedCandidateMismatch,
}

impl fmt::Display for CompletionInvariantError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "semantic completion invariant failed: {self:?}")
    }
}

pub enum CompletionInstall {
    Installed(SessionChange),
    IgnoredStale,
    RejectedInvariant(CompletionInvariantError),
}

pub fn run_semantic_request(
    request: SemanticAnalysisRequest,
) -> Result<SemanticAnalysisCompletion, AnalysisError> {
    run_semantic_request_with_preparer(request, |source, previous, revision, promoted| {
        prepare_candidate_with_markdown_updates(source, previous, revision, promoted)
    })
}

pub(crate) fn run_semantic_request_with_preparer<F>(
    request: SemanticAnalysisRequest,
    mut prepare: F,
) -> Result<SemanticAnalysisCompletion, AnalysisError>
where
    F: for<'a> FnMut(
        SourceBundle,
        PreviousAnalyses<'a>,
        u64,
        Arc<[PromotedMarkdownUpdate]>,
    ) -> Result<PreparedCandidate, AnalysisError>,
{
    let mut prepared: Option<PreparedCandidate> = None;
    let mut diagnostics = (*request.previous.semantic_diagnostics).clone();
    for step in request.steps.iter() {
        let semantic_source = match prepared.as_ref() {
            Some(previous) => previous.source().clone(),
            None => (*request.previous.semantic_source).clone(),
        };
        let attempt = {
            let previous = previous_analyses(prepared.as_ref(), &request.previous);
            let previous_revision = previous
                .okf
                .catalog
                .document(step.promoted.document)
                .map(|document| document.revision());
            let candidate_source = semantic_source_with_promoted_document(
                &semantic_source,
                &step.source,
                previous.okf,
                &step.promoted,
            )?;
            if previous_revision == Some(step.promoted.base_revision) {
                prepare(
                    candidate_source,
                    previous,
                    step.session_revision,
                    Arc::from([step.promoted.clone()]),
                )
            } else {
                prepare_candidate_with_markdown_recovery(
                    candidate_source,
                    previous,
                    step.session_revision,
                    step.promoted.clone(),
                )
            }
        };
        match attempt {
            Ok(next) => {
                diagnostics.remove(&step.promoted.document);
                prepared = Some(next);
            }
            Err(error) => {
                diagnostics.insert(
                    step.promoted.document,
                    SemanticFailureDiagnostic {
                        document: step.promoted.document,
                        session_revision: step.session_revision,
                        error: Arc::new(error),
                    },
                );
                let previous = previous_analyses(prepared.as_ref(), &request.previous);
                prepared = Some(prepare_candidate(
                    semantic_source,
                    Some(previous),
                    step.session_revision,
                )?);
            }
        }
    }
    let prepared = prepared.ok_or_else(|| AnalysisError::CatalogInvariant {
        reason: "semantic request has no revision steps".into(),
    })?;
    let affected = prepared.uml().affected().clone();
    let (semantic_source, okf_analysis, uml_analysis, revision) = prepared.into_parts();
    Ok(SemanticAnalysisCompletion {
        session_revision: request.session_revision,
        source: request.source,
        prepared: Arc::new(PreparedSemanticSnapshot {
            semantic_source: Arc::new(semantic_source),
            okf_analysis: Arc::new(okf_analysis),
            uml_analysis: Arc::new(uml_analysis),
            revision,
            affected,
            diagnostics: Arc::new(diagnostics),
        }),
    })
}

fn previous_analyses<'a>(
    prepared: Option<&'a PreparedCandidate>,
    baseline: &'a EditorSessionSnapshot,
) -> PreviousAnalyses<'a> {
    match prepared {
        Some(previous) => PreviousAnalyses {
            okf: previous.okf(),
            uml: previous.uml(),
        },
        None => PreviousAnalyses {
            okf: &baseline.okf_analysis,
            uml: &baseline.uml_analysis,
        },
    }
}
