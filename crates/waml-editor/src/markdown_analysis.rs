#![allow(dead_code)] // Task 2 API is mounted by the editor integration in Task 4.

use std::{fmt, sync::Arc};

use waml::analysis::{
    prepare_candidate_with_markdown_updates, AnalysisError, DocumentId, PreparedCandidate,
    PreviousAnalyses, PromotedMarkdownUpdate,
};
use waml::source::SourceBundle;

use crate::editor_session::{EditorSessionSnapshot, SessionChange};

#[derive(Clone)]
pub struct SemanticAnalysisRequest {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub previous: Arc<EditorSessionSnapshot>,
    pub promoted: Arc<[PromotedMarkdownUpdate]>,
}

#[derive(Clone)]
pub struct SemanticAnalysisCompletion {
    pub session_revision: u64,
    pub source: Arc<SourceBundle>,
    pub prepared: Arc<PreparedCandidate>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionInvariantError {
    MissingMarkdownSnapshot { document: DocumentId },
    MarkdownIdentityMismatch { document: DocumentId },
    SharedPreparedCandidate,
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
    prepare: F,
) -> Result<SemanticAnalysisCompletion, AnalysisError>
where
    F: for<'a> FnOnce(
        SourceBundle,
        PreviousAnalyses<'a>,
        u64,
        Arc<[PromotedMarkdownUpdate]>,
    ) -> Result<PreparedCandidate, AnalysisError>,
{
    let prepared = prepare(
        (*request.source).clone(),
        PreviousAnalyses {
            okf: &request.previous.okf_analysis,
            uml: &request.previous.uml_analysis,
        },
        request.session_revision,
        request.promoted,
    )?;
    Ok(SemanticAnalysisCompletion {
        session_revision: request.session_revision,
        source: request.source,
        prepared: Arc::new(prepared),
    })
}
