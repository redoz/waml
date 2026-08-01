use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

pub use waml_syntax::DocumentRevision;
use waml_syntax::{
    parse_markdown, reparse_markdown, LineIndex, MarkdownDialect, MarkdownSyntaxSnapshot,
    MarkdownSyntaxUpdate, ParseError, SourceText, SyntaxLanguage, SyntaxTree, TextChange,
    TextRange, TextSize,
};

use crate::{
    okf,
    source::{BundlePath, SourceBundle, SourceDocument},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentId(u64);

#[derive(Debug)]
pub struct DocumentVersion {
    id: DocumentId,
    revision: DocumentRevision,
    path: BundlePath,
    text: SourceText,
    line_index: Arc<LineIndex>,
}

impl DocumentVersion {
    pub fn id(&self) -> DocumentId {
        self.id
    }
    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }
    pub fn path(&self) -> &BundlePath {
        &self.path
    }
    pub fn text(&self) -> &SourceText {
        &self.text
    }
    pub fn line_index(&self) -> &Arc<LineIndex> {
        &self.line_index
    }
}

#[derive(Debug)]
pub struct DocumentCatalog {
    session_revision: u64,
    documents: Arc<BTreeMap<DocumentId, Arc<DocumentVersion>>>,
    paths: Arc<BTreeMap<BundlePath, DocumentId>>,
    next_document_id: u64,
}

impl DocumentCatalog {
    pub fn session_revision(&self) -> u64 {
        self.session_revision
    }
    pub fn document(&self, id: DocumentId) -> Option<&Arc<DocumentVersion>> {
        self.documents.get(&id)
    }
    pub fn id_for_path(&self, path: &BundlePath) -> Option<DocumentId> {
        self.paths.get(path).copied()
    }
    pub fn path_for_id(&self, id: DocumentId) -> Option<&BundlePath> {
        self.document(id).map(|document| document.path())
    }
    pub(crate) fn documents(&self) -> &BTreeMap<DocumentId, Arc<DocumentVersion>> {
        &self.documents
    }
}

pub struct SyntaxSnapshot<L: SyntaxLanguage> {
    document: Arc<DocumentVersion>,
    syntax: Arc<SyntaxTree<L>>,
}
impl<L: SyntaxLanguage> SyntaxSnapshot<L> {
    pub(crate) fn new(document: Arc<DocumentVersion>, syntax: Arc<SyntaxTree<L>>) -> Self {
        Self { document, syntax }
    }
    pub fn document(&self) -> &Arc<DocumentVersion> {
        &self.document
    }
    pub fn syntax(&self) -> &Arc<SyntaxTree<L>> {
        &self.syntax
    }
}

pub struct SyntaxSet<L: SyntaxLanguage> {
    catalog: Arc<DocumentCatalog>,
    documents: Arc<BTreeMap<DocumentId, Arc<SyntaxSnapshot<L>>>>,
}
impl<L: SyntaxLanguage> SyntaxSet<L> {
    pub(crate) fn from_snapshots(
        catalog: Arc<DocumentCatalog>,
        documents: BTreeMap<DocumentId, Arc<SyntaxSnapshot<L>>>,
    ) -> Self {
        Self {
            catalog,
            documents: Arc::new(documents),
        }
    }
    pub fn catalog(&self) -> &Arc<DocumentCatalog> {
        &self.catalog
    }
    pub fn document(&self, id: DocumentId) -> Option<&Arc<SyntaxSnapshot<L>>> {
        self.documents.get(&id)
    }
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
    pub(crate) fn documents(&self) -> &BTreeMap<DocumentId, Arc<SyntaxSnapshot<L>>> {
        &self.documents
    }
}

pub struct OkfAnalysis {
    pub catalog: Arc<DocumentCatalog>,
    pub markdown: MarkdownSyntaxSet,
    pub bundle: okf::Bundle,
}

impl OkfAnalysis {
    pub fn markdown_snapshot(&self, document: DocumentId) -> Option<&Arc<MarkdownSyntaxSnapshot>> {
        self.markdown.document(document)
    }
}

#[derive(Clone)]
pub struct PromotedMarkdownUpdate {
    pub document: DocumentId,
    pub base_revision: DocumentRevision,
    pub update: MarkdownSyntaxUpdate,
}

#[derive(Clone)]
pub struct MarkdownSyntaxSet {
    catalog: Arc<DocumentCatalog>,
    documents: Arc<BTreeMap<DocumentId, Arc<MarkdownSyntaxSnapshot>>>,
}

impl MarkdownSyntaxSet {
    pub fn document(&self, id: DocumentId) -> Option<&Arc<MarkdownSyntaxSnapshot>> {
        self.documents.get(&id)
    }

    pub fn catalog(&self) -> &Arc<DocumentCatalog> {
        &self.catalog
    }

    pub(crate) fn documents(&self) -> &BTreeMap<DocumentId, Arc<MarkdownSyntaxSnapshot>> {
        &self.documents
    }
}

pub struct DomainAnalysisContext<'a> {
    pub source: &'a SourceBundle,
    pub catalog: &'a Arc<DocumentCatalog>,
    pub markdown: &'a MarkdownSyntaxSet,
    pub okf: &'a okf::Bundle,
    pub session_revision: u64,
}

#[derive(Default)]
pub struct ClaimSet {
    concept_ids: BTreeSet<String>,
}
impl ClaimSet {
    pub fn from_concept_ids(ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            concept_ids: ids.into_iter().collect(),
        }
    }
    pub fn contains(&self, id: &str) -> bool {
        self.concept_ids.contains(id)
    }
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.concept_ids.iter().map(String::as_str)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum AnalysisStage {
    Shell,
    Okf,
    Specialization(&'static str),
    Claims,
}
#[derive(Debug)]
pub enum AnalysisError {
    SourceTooLarge {
        path: BundlePath,
        bytes: usize,
    },
    Shell {
        path: BundlePath,
        source: ParseError,
    },
    Okf(okf::BundleError),
    CatalogInvariant {
        reason: Arc<str>,
    },
    InvalidPromotedMarkdownUpdate {
        document: DocumentId,
        reason: InvalidPromotedMarkdownUpdateReason,
    },
    Specialization {
        name: &'static str,
        reason: Arc<str>,
    },
    AmbiguousClaim {
        concept_id: String,
        first: String,
        second: String,
    },
    StructuralInvariant {
        stage: AnalysisStage,
        reason: Arc<str>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidPromotedMarkdownUpdateReason {
    MissingPreviousDocument,
    MissingCandidateDocument,
    SourcePathMismatch,
    StaleBaseRevision {
        expected: DocumentRevision,
        actual: DocumentRevision,
    },
    NonSuccessorRevision {
        expected: DocumentRevision,
        actual: DocumentRevision,
    },
    DuplicateDocument,
    ResultTextMismatch,
    InvalidAffectedRange {
        range: TextRange,
    },
    RecoveryRevisionNotNewer {
        previous: DocumentRevision,
        actual: DocumentRevision,
    },
}
impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "analysis error: {self:?}")
    }
}
impl std::error::Error for AnalysisError {}

impl From<AnalysisError> for crate::edit::EditError {
    fn from(error: AnalysisError) -> Self {
        crate::edit::EditError {
            index: 0,
            op: "analysis.prepare".into(),
            selector: None,
            reason: error.to_string(),
        }
    }
}

pub fn validate_disjoint_claims<'a>(
    claims: impl IntoIterator<Item = (&'a str, &'a ClaimSet)>,
) -> Result<(), AnalysisError> {
    let mut claimants: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (analyzer, claims) in claims {
        for concept_id in claims.iter() {
            claimants.entry(concept_id).or_default().insert(analyzer);
        }
    }
    for (concept_id, analyzers) in claimants {
        let mut analyzers = analyzers.into_iter();
        let Some(first) = analyzers.next() else {
            continue;
        };
        if let Some(second) = analyzers.next() {
            return Err(AnalysisError::AmbiguousClaim {
                concept_id: concept_id.to_owned(),
                first: first.to_owned(),
                second: second.to_owned(),
            });
        }
    }
    Ok(())
}

trait PreparationHooks {
    fn before(&mut self, stage: AnalysisStage) -> Result<(), AnalysisError>;

    fn markdown_parsed(&mut self, _document: DocumentId) {}

    fn markdown_reparsed(&mut self, _document: DocumentId) {}

    fn markdown_promoted(&mut self, _document: DocumentId) {}
}
struct NoopPreparationHooks;
impl PreparationHooks for NoopPreparationHooks {
    fn before(&mut self, _: AnalysisStage) -> Result<(), AnalysisError> {
        Ok(())
    }
}

/// Borrowed reuse inputs only; ownership and revision clocks stay with the host.
///
/// ```compile_fail
/// use waml::analysis::PreviousAnalyses;
/// fn advance_revision(previous: &mut PreviousAnalyses<'_>) {
///     previous.revision += 1;
/// }
/// ```
pub struct PreviousAnalyses<'a> {
    pub okf: &'a OkfAnalysis,
    pub uml: &'a crate::uml::Analysis,
}

/// Owned, immutable state produced by the static product composition root.
///
/// ```compile_fail
/// use waml::analysis::PreparedCandidate;
/// fn forge_revision(candidate: &mut PreparedCandidate) {
///     candidate.revision = 99;
/// }
/// ```
pub struct PreparedCandidate {
    source: SourceBundle,
    okf: OkfAnalysis,
    uml: crate::uml::Analysis,
    revision: u64,
}

impl PreparedCandidate {
    pub fn source(&self) -> &SourceBundle {
        &self.source
    }

    pub fn okf(&self) -> &OkfAnalysis {
        &self.okf
    }

    pub fn uml(&self) -> &crate::uml::Analysis {
        &self.uml
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn referrers(&self, target: &str) -> Vec<String> {
        self.uml.referrers(target)
    }

    pub fn into_parts(self) -> (SourceBundle, OkfAnalysis, crate::uml::Analysis, u64) {
        (self.source, self.okf, self.uml, self.revision)
    }
}

pub fn prepare_candidate(
    candidate_source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    candidate_revision: u64,
) -> Result<PreparedCandidate, AnalysisError> {
    prepare_candidate_inner(
        candidate_source,
        previous,
        candidate_revision,
        &mut NoopPreparationHooks,
    )
}

pub fn prepare_candidate_with_markdown_updates(
    candidate_source: SourceBundle,
    previous: PreviousAnalyses<'_>,
    candidate_revision: u64,
    promoted: Arc<[PromotedMarkdownUpdate]>,
) -> Result<PreparedCandidate, AnalysisError> {
    prepare_candidate_inner_with_markdown_updates(
        candidate_source,
        Some(previous),
        candidate_revision,
        &promoted,
        None,
        &mut NoopPreparationHooks,
    )
}

pub fn prepare_candidate_with_markdown_recovery(
    candidate_source: SourceBundle,
    previous: PreviousAnalyses<'_>,
    candidate_revision: u64,
    recovered: PromotedMarkdownUpdate,
) -> Result<PreparedCandidate, AnalysisError> {
    prepare_candidate_inner_with_markdown_updates(
        candidate_source,
        Some(previous),
        candidate_revision,
        &[],
        Some(&recovered),
        &mut NoopPreparationHooks,
    )
}

pub fn semantic_source_with_promoted_document(
    semantic_source: &SourceBundle,
    accepted_source: &SourceBundle,
    previous: &OkfAnalysis,
    promoted: &PromotedMarkdownUpdate,
) -> Result<SourceBundle, AnalysisError> {
    let previous_document = previous.catalog.document(promoted.document).ok_or(
        AnalysisError::InvalidPromotedMarkdownUpdate {
            document: promoted.document,
            reason: InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
        },
    )?;
    let accepted_document = accepted_source.document(previous_document.path()).ok_or(
        AnalysisError::InvalidPromotedMarkdownUpdate {
            document: promoted.document,
            reason: InvalidPromotedMarkdownUpdateReason::MissingCandidateDocument,
        },
    )?;
    if !Arc::ptr_eq(
        accepted_document.text_arc(),
        promoted.update.snapshot.text().shared(),
    ) {
        return Err(AnalysisError::InvalidPromotedMarkdownUpdate {
            document: promoted.document,
            reason: InvalidPromotedMarkdownUpdateReason::ResultTextMismatch,
        });
    }
    let mut candidate = semantic_source.clone();
    let target = candidate.document_mut(previous_document.path()).ok_or(
        AnalysisError::InvalidPromotedMarkdownUpdate {
            document: promoted.document,
            reason: InvalidPromotedMarkdownUpdateReason::MissingCandidateDocument,
        },
    )?;
    *target = SourceDocument::from_shared(
        previous_document.path().clone(),
        accepted_document.text_arc().clone(),
    );
    Ok(candidate)
}

fn prepare_candidate_inner(
    candidate_source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    candidate_revision: u64,
    hooks: &mut impl PreparationHooks,
) -> Result<PreparedCandidate, AnalysisError> {
    prepare_candidate_inner_with_markdown_updates(
        candidate_source,
        previous,
        candidate_revision,
        &[],
        None,
        hooks,
    )
}

fn prepare_candidate_inner_with_markdown_updates(
    candidate_source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    candidate_revision: u64,
    promoted: &[PromotedMarkdownUpdate],
    recovered: Option<&PromotedMarkdownUpdate>,
    hooks: &mut impl PreparationHooks,
) -> Result<PreparedCandidate, AnalysisError> {
    let okf = analyze_okf_inner(
        &candidate_source,
        previous.as_ref().map(|analyses| analyses.okf),
        candidate_revision,
        promoted,
        recovered,
        hooks,
    )?;
    hooks.before(AnalysisStage::Specialization("uml"))?;
    let uml = crate::uml::analyze(
        DomainAnalysisContext {
            source: &candidate_source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: candidate_revision,
        },
        previous.as_ref().map(|analyses| analyses.uml),
    )?;
    hooks.before(AnalysisStage::Claims)?;
    validate_disjoint_claims([("uml", &uml.claims)])?;
    Ok(PreparedCandidate {
        source: candidate_source,
        okf,
        uml,
        revision: candidate_revision,
    })
}

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    use super::*;

    pub struct PreparationProbe {
        fail_at: Option<AnalysisStage>,
        calls: Vec<AnalysisStage>,
        markdown_parse_calls: BTreeMap<DocumentId, usize>,
        markdown_reparse_calls: BTreeMap<DocumentId, usize>,
        markdown_promotions: BTreeMap<DocumentId, usize>,
    }

    impl PreparationProbe {
        pub fn succeed() -> Self {
            Self {
                fail_at: None,
                calls: Vec::new(),
                markdown_parse_calls: BTreeMap::new(),
                markdown_reparse_calls: BTreeMap::new(),
                markdown_promotions: BTreeMap::new(),
            }
        }

        pub fn fail_at(stage: AnalysisStage) -> Self {
            Self {
                fail_at: Some(stage),
                calls: Vec::new(),
                markdown_parse_calls: BTreeMap::new(),
                markdown_reparse_calls: BTreeMap::new(),
                markdown_promotions: BTreeMap::new(),
            }
        }

        pub fn phase_names(&self) -> Vec<&'static str> {
            self.calls.iter().map(phase_name).collect()
        }

        pub fn markdown_parse_calls(&self, document: DocumentId) -> usize {
            self.markdown_parse_calls
                .get(&document)
                .copied()
                .unwrap_or(0)
        }

        pub fn markdown_reparse_calls(&self, document: DocumentId) -> usize {
            self.markdown_reparse_calls
                .get(&document)
                .copied()
                .unwrap_or(0)
        }

        pub fn markdown_promotions(&self, document: DocumentId) -> usize {
            self.markdown_promotions
                .get(&document)
                .copied()
                .unwrap_or(0)
        }
    }

    impl PreparationHooks for PreparationProbe {
        fn before(&mut self, stage: AnalysisStage) -> Result<(), AnalysisError> {
            self.calls.push(stage);
            if self
                .fail_at
                .is_some_and(|fail_at| same_stage(fail_at, stage))
            {
                return Err(AnalysisError::StructuralInvariant {
                    stage,
                    reason: "test probe injected failure".into(),
                });
            }
            Ok(())
        }

        fn markdown_parsed(&mut self, document: DocumentId) {
            *self.markdown_parse_calls.entry(document).or_default() += 1;
        }

        fn markdown_reparsed(&mut self, document: DocumentId) {
            *self.markdown_reparse_calls.entry(document).or_default() += 1;
        }

        fn markdown_promoted(&mut self, document: DocumentId) {
            *self.markdown_promotions.entry(document).or_default() += 1;
        }
    }

    pub fn prepare_candidate_with_probe(
        candidate_source: SourceBundle,
        previous: Option<PreviousAnalyses<'_>>,
        candidate_revision: u64,
        probe: &mut PreparationProbe,
    ) -> Result<PreparedCandidate, AnalysisError> {
        prepare_candidate_inner(candidate_source, previous, candidate_revision, probe)
    }

    pub fn prepare_candidate_with_promoted_probe(
        candidate_source: SourceBundle,
        previous: PreviousAnalyses<'_>,
        candidate_revision: u64,
        promoted: Arc<[PromotedMarkdownUpdate]>,
        probe: &mut PreparationProbe,
    ) -> Result<PreparedCandidate, AnalysisError> {
        prepare_candidate_inner_with_markdown_updates(
            candidate_source,
            Some(previous),
            candidate_revision,
            &promoted,
            None,
            probe,
        )
    }

    fn phase_name(stage: &AnalysisStage) -> &'static str {
        match stage {
            AnalysisStage::Shell => "shell",
            AnalysisStage::Okf => "okf",
            AnalysisStage::Specialization("uml") => "uml",
            AnalysisStage::Specialization(_) => "other-specialization",
            AnalysisStage::Claims => "claims",
        }
    }

    fn same_stage(left: AnalysisStage, right: AnalysisStage) -> bool {
        matches!(
            (left, right),
            (AnalysisStage::Shell, AnalysisStage::Shell)
                | (AnalysisStage::Okf, AnalysisStage::Okf)
                | (
                    AnalysisStage::Specialization("uml"),
                    AnalysisStage::Specialization("uml")
                )
                | (AnalysisStage::Claims, AnalysisStage::Claims)
        )
    }
}

pub fn analyze_okf(
    source: &SourceBundle,
    previous: Option<&OkfAnalysis>,
    session_revision: u64,
) -> Result<OkfAnalysis, AnalysisError> {
    analyze_okf_inner(
        source,
        previous,
        session_revision,
        &[],
        None,
        &mut NoopPreparationHooks,
    )
}

fn analyze_okf_inner(
    source: &SourceBundle,
    previous: Option<&OkfAnalysis>,
    session_revision: u64,
    promoted: &[PromotedMarkdownUpdate],
    recovered: Option<&PromotedMarkdownUpdate>,
    hooks: &mut impl PreparationHooks,
) -> Result<OkfAnalysis, AnalysisError> {
    let previous_catalog = previous.map(|analysis| &analysis.catalog);
    let recovered = recovered
        .map(|recovered| {
            let previous_analysis = previous.ok_or_else(|| {
                invalid_promoted_update(
                    recovered.document,
                    InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
                )
            })?;
            let previous_document = previous_analysis
                .catalog
                .document(recovered.document)
                .ok_or_else(|| {
                    invalid_promoted_update(
                        recovered.document,
                        InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
                    )
                })?;
            let previous_snapshot = previous_analysis
                .markdown_snapshot(recovered.document)
                .ok_or_else(|| {
                    invalid_promoted_update(
                        recovered.document,
                        InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
                    )
                })?;
            let source_document = source.document(previous_document.path()).ok_or_else(|| {
                invalid_promoted_update(
                    recovered.document,
                    InvalidPromotedMarkdownUpdateReason::MissingCandidateDocument,
                )
            })?;
            if !Arc::ptr_eq(
                source_document.text_arc(),
                recovered.update.snapshot.text().shared(),
            ) {
                return Err(invalid_promoted_update(
                    recovered.document,
                    InvalidPromotedMarkdownUpdateReason::ResultTextMismatch,
                ));
            }
            let actual = recovered.update.snapshot.revision();
            if actual <= previous_snapshot.revision() {
                return Err(invalid_promoted_update(
                    recovered.document,
                    InvalidPromotedMarkdownUpdateReason::RecoveryRevisionNotNewer {
                        previous: previous_snapshot.revision(),
                        actual,
                    },
                ));
            }
            Ok((
                recovered.document,
                actual,
                recovered.update.snapshot.clone(),
            ))
        })
        .transpose()?;
    let mut documents = BTreeMap::new();
    let mut paths = BTreeMap::new();
    let mut next_id = previous_catalog.map_or(0, |catalog| catalog.next_document_id);
    for source_document in source.documents() {
        let path = source_document.path().clone();
        let prior = previous_catalog
            .and_then(|catalog| catalog.id_for_path(&path))
            .and_then(|id| previous_catalog.and_then(|catalog| catalog.document(id)));
        let version = match prior {
            Some(prior) if Arc::ptr_eq(prior.text().shared(), source_document.text_arc()) => {
                prior.clone()
            }
            Some(prior) => {
                let revision = recovered
                    .as_ref()
                    .filter(|(document, _, _)| *document == prior.id())
                    .map(|(_, revision, _)| *revision)
                    .map(Ok)
                    .unwrap_or_else(|| {
                        prior.revision().checked_next().ok_or_else(|| {
                            AnalysisError::CatalogInvariant {
                                reason: "document revision overflow".into(),
                            }
                        })
                    })?;
                Arc::new(version(
                    prior.id(),
                    revision,
                    path.clone(),
                    source_document.text_arc().clone(),
                )?)
            }
            None => {
                let id = DocumentId(next_id);
                next_id += 1;
                Arc::new(version(
                    id,
                    DocumentRevision::new(1),
                    path.clone(),
                    source_document.text_arc().clone(),
                )?)
            }
        };
        paths.insert(path, version.id());
        documents.insert(version.id(), version);
    }
    let candidate = Arc::new(DocumentCatalog {
        session_revision,
        documents: Arc::new(documents),
        paths: Arc::new(paths),
        next_document_id: next_id,
    });
    let markdown_updates = validate_promoted_markdown_updates(previous, &candidate, promoted)?;
    hooks.before(AnalysisStage::Shell)?;
    let mut markdown_documents = BTreeMap::new();
    for document in candidate.documents.values() {
        let snapshot = match markdown_updates.get(&document.id()) {
            Some(update) => {
                hooks.markdown_promoted(document.id());
                update.snapshot.clone()
            }
            None => match previous.and_then(|analysis| analysis.markdown.document(document.id())) {
                Some(previous_snapshot) if previous_snapshot.revision() == document.revision() => {
                    previous_snapshot.clone()
                }
                Some(previous_snapshot) => {
                    let snapshot = reparse_markdown(
                        previous_snapshot,
                        document.revision(),
                        document.text().clone(),
                        &single_text_change(previous_snapshot.text(), document.text()),
                    )
                    .map_err(|source| shell_error(document.path().clone(), source))?
                    .snapshot;
                    hooks.markdown_reparsed(document.id());
                    snapshot
                }
                None => {
                    let snapshot = parse_markdown(
                        document.revision(),
                        document.text().clone(),
                        MarkdownDialect::WAML_DEFAULT,
                    )
                    .map_err(|source| shell_error(document.path().clone(), source))?;
                    hooks.markdown_parsed(document.id());
                    snapshot
                }
            },
        };
        let snapshot = recovered
            .as_ref()
            .filter(|(recovered, _, _)| *recovered == document.id())
            .map(|(_, _, snapshot)| snapshot.clone())
            .unwrap_or(snapshot);
        markdown_documents.insert(document.id(), snapshot);
    }
    hooks.before(AnalysisStage::Okf)?;
    let markdown = MarkdownSyntaxSet {
        catalog: candidate.clone(),
        documents: Arc::new(markdown_documents),
    };
    let bundle = okf::shell::derive(&candidate, &markdown)?;
    Ok(OkfAnalysis {
        catalog: candidate.clone(),
        markdown,
        bundle,
    })
}

fn validate_promoted_markdown_updates(
    previous: Option<&OkfAnalysis>,
    candidate: &DocumentCatalog,
    promoted: &[PromotedMarkdownUpdate],
) -> Result<BTreeMap<DocumentId, MarkdownSyntaxUpdate>, AnalysisError> {
    let mut validated = BTreeMap::new();
    for promoted in promoted {
        let document = promoted.document;
        if validated.contains_key(&document) {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::DuplicateDocument,
            ));
        }
        let previous_document = previous
            .and_then(|analysis| analysis.catalog.document(document))
            .ok_or_else(|| {
                invalid_promoted_update(
                    document,
                    InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
                )
            })?;
        let previous_snapshot = previous
            .and_then(|analysis| analysis.markdown_snapshot(document))
            .ok_or_else(|| {
                invalid_promoted_update(
                    document,
                    InvalidPromotedMarkdownUpdateReason::MissingPreviousDocument,
                )
            })?;
        let candidate_document = candidate.document(document).ok_or_else(|| {
            invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::MissingCandidateDocument,
            )
        })?;
        // IDs are retained only by path. This is a defensive check for an
        // internally inconsistent catalog; public callers cannot forge it.
        if previous_document.path() != candidate_document.path() {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::SourcePathMismatch,
            ));
        }
        let expected_base = previous_snapshot.revision();
        if promoted.base_revision != expected_base {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::StaleBaseRevision {
                    expected: expected_base,
                    actual: promoted.base_revision,
                },
            ));
        }
        let expected_revision = promoted
            .base_revision
            .checked_next()
            .unwrap_or(promoted.base_revision);
        let actual_revision = promoted.update.snapshot.revision();
        if actual_revision != expected_revision {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::NonSuccessorRevision {
                    expected: expected_revision,
                    actual: actual_revision,
                },
            ));
        }
        let candidate_revision = candidate_document.revision();
        if candidate_revision != expected_revision {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::NonSuccessorRevision {
                    expected: expected_revision,
                    actual: candidate_revision,
                },
            ));
        }
        if !Arc::ptr_eq(
            promoted.update.snapshot.text().shared(),
            candidate_document.text().shared(),
        ) {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::ResultTextMismatch,
            ));
        }
        if let Some(range) = promoted
            .update
            .affected_ranges
            .iter()
            .copied()
            .find(|range| range.end() > promoted.update.snapshot.text().len())
        {
            return Err(invalid_promoted_update(
                document,
                InvalidPromotedMarkdownUpdateReason::InvalidAffectedRange { range },
            ));
        }
        validated.insert(document, promoted.update.clone());
    }
    Ok(validated)
}

fn invalid_promoted_update(
    document: DocumentId,
    reason: InvalidPromotedMarkdownUpdateReason,
) -> AnalysisError {
    AnalysisError::InvalidPromotedMarkdownUpdate { document, reason }
}

pub(crate) fn single_text_change(old: &SourceText, new: &SourceText) -> Vec<TextChange> {
    if Arc::ptr_eq(old.shared(), new.shared()) || old.shared().as_str() == new.shared().as_str() {
        return Vec::new();
    }

    let old_source = old.shared();
    let new_source = new.shared();
    let mut prefix = 0;
    for ((old_at, old_char), (new_at, new_char)) in
        old_source.char_indices().zip(new_source.char_indices())
    {
        if old_char != new_char {
            break;
        }
        debug_assert_eq!(old_at, new_at);
        prefix = old_at + old_char.len_utf8();
    }
    while !old_source.is_char_boundary(prefix) || !new_source.is_char_boundary(prefix) {
        prefix -= 1;
    }

    let mut old_end = old_source.len();
    let mut new_end = new_source.len();
    while old_end > prefix && new_end > prefix {
        let old_char = old_source
            .get(..old_end)
            .and_then(|source| source.chars().next_back());
        let new_char = new_source
            .get(..new_end)
            .and_then(|source| source.chars().next_back());
        if old_char != new_char {
            break;
        }
        let Some(character) = old_char else {
            break;
        };
        old_end -= character.len_utf8();
        new_end -= character.len_utf8();
    }

    let old_range = TextRange::new(
        TextSize::try_from_usize(prefix).expect("SourceText widths fit TextSize"),
        TextSize::try_from_usize(old_end).expect("SourceText widths fit TextSize"),
    )
    .expect("common prefix cannot exceed old text");
    let replacement = Arc::from(
        new_source
            .get(prefix..new_end)
            .expect("common prefix and suffix are UTF-8 boundaries"),
    );
    vec![TextChange {
        old_range,
        replacement,
    }]
}

fn version(
    id: DocumentId,
    revision: DocumentRevision,
    path: BundlePath,
    source: Arc<String>,
) -> Result<DocumentVersion, AnalysisError> {
    let bytes = source.len();
    let text = SourceText::from_shared(source).map_err(|_| AnalysisError::SourceTooLarge {
        path: path.clone(),
        bytes,
    })?;
    Ok(DocumentVersion {
        id,
        revision,
        path,
        line_index: Arc::new(LineIndex::new(&text)),
        text,
    })
}
fn shell_error(path: BundlePath, source: ParseError) -> AnalysisError {
    match source {
        ParseError::SourceTooLarge { bytes } => AnalysisError::SourceTooLarge { path, bytes },
        ParseError::StructuralInvariant { reason } => AnalysisError::StructuralInvariant {
            stage: AnalysisStage::Shell,
            reason: format!("{}: {reason}", path.as_str()).into(),
        },
        source => AnalysisError::Shell { path, source },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::source_text_weak;

    #[derive(Clone, Copy)]
    enum FailAt {
        Shell,
        Okf,
    }

    struct RecordingHooks {
        fail_at: FailAt,
        calls: Vec<AnalysisStage>,
    }

    impl PreparationHooks for RecordingHooks {
        fn before(&mut self, stage: AnalysisStage) -> Result<(), AnalysisError> {
            self.calls.push(stage);
            let fail = matches!(
                (self.fail_at, stage),
                (FailAt::Shell, AnalysisStage::Shell) | (FailAt::Okf, AnalysisStage::Okf)
            );
            if fail {
                return Err(AnalysisError::StructuralInvariant {
                    stage,
                    reason: "injected".into(),
                });
            }
            Ok(())
        }
    }

    #[test]
    fn candidate_failure_is_non_mutating() {
        let committed_source = SourceBundle::try_from_pairs([("one.md", "# one")]).unwrap();
        let committed = analyze_okf(&committed_source, None, 1).unwrap();
        let committed_catalog = committed.catalog.clone();
        let committed_bundle = committed.bundle.clone();
        let candidate_source =
            SourceBundle::try_from_pairs([("one.md", "# changed"), ("two.md", "# two")]).unwrap();

        for (fail_at, expected_calls) in [
            (FailAt::Shell, vec!["Shell"]),
            (FailAt::Okf, vec!["Shell", "Okf"]),
        ] {
            let mut hooks = RecordingHooks {
                fail_at,
                calls: Vec::new(),
            };
            assert!(matches!(
                analyze_okf_inner(
                    &candidate_source,
                    Some(&committed),
                    2,
                    &[],
                    None,
                    &mut hooks,
                ),
                Err(AnalysisError::StructuralInvariant { .. })
            ));
            let calls: Vec<_> = hooks
                .calls
                .iter()
                .map(|stage| match stage {
                    AnalysisStage::Shell => "Shell",
                    AnalysisStage::Okf => "Okf",
                    _ => "other",
                })
                .collect();
            assert_eq!(calls, expected_calls);
            assert!(Arc::ptr_eq(&committed.catalog, &committed_catalog));
            assert_eq!(committed.bundle, committed_bundle);

            let retried = analyze_okf(&candidate_source, Some(&committed), 2).unwrap();
            assert_eq!(
                retried.catalog.document(DocumentId(0)).unwrap().revision(),
                DocumentRevision::new(2)
            );
            assert_eq!(
                retried
                    .catalog
                    .id_for_path(&BundlePath::parse("two.md").unwrap()),
                Some(DocumentId(1))
            );
        }
    }

    #[test]
    fn parser_structural_failures_are_shell_invariants() {
        let error = shell_error(
            BundlePath::parse("broken.md").unwrap(),
            ParseError::StructuralInvariant {
                reason: "injected parser mismatch".into(),
            },
        );
        assert!(matches!(
            error,
            AnalysisError::StructuralInvariant {
                stage: AnalysisStage::Shell,
                ..
            }
        ));
    }

    #[test]
    fn repeated_candidates_retain_only_baseline_current_and_one_untouched_allocation() {
        let touched_path = BundlePath::parse("touched.md").unwrap();
        let untouched_path = BundlePath::parse("untouched.md").unwrap();
        let mut baseline = SourceBundle::try_from_pairs([
            ("touched.md", "# A\nbody\n"),
            ("untouched.md", "# Untouched\nbody\n"),
        ])
        .unwrap();
        let mut touched_weaks = vec![source_text_weak(baseline.document(&touched_path).unwrap())];
        let untouched_weak = source_text_weak(baseline.document(&untouched_path).unwrap());
        let mut current = prepare_candidate(baseline.clone(), None, 1).unwrap();

        for edit in 0..1_000 {
            let mut candidate_source = current.source().clone();
            candidate_source
                .document_mut(&touched_path)
                .unwrap()
                .text_mut()
                .replace_range(2..3, if edit % 2 == 0 { "B" } else { "A" });
            let next = prepare_candidate(
                candidate_source,
                Some(PreviousAnalyses {
                    okf: current.okf(),
                    uml: current.uml(),
                }),
                edit + 2,
            )
            .unwrap();
            touched_weaks.push(source_text_weak(
                next.source().document(&touched_path).unwrap(),
            ));
            current = next;
        }

        assert_eq!(
            touched_weaks
                .iter()
                .filter(|weak| weak.upgrade().is_some())
                .count(),
            2
        );
        assert!(current.source().shares_text_with(&baseline, "untouched.md"));
        assert!(untouched_weak.upgrade().is_some());

        baseline.clone_from(current.source());

        assert_eq!(
            touched_weaks
                .iter()
                .filter(|weak| weak.upgrade().is_some())
                .count(),
            1
        );
        assert!(current.source().shares_text_with(&baseline, "touched.md"));
        assert!(current.source().shares_text_with(&baseline, "untouched.md"));
    }

    #[test]
    fn single_text_change_is_minimal_and_utf8_boundary_safe() {
        let shared = Arc::new("prefix café suffix".to_owned());
        let same_arc = SourceText::from_shared(shared.clone()).unwrap();
        assert!(single_text_change(&same_arc, &same_arc).is_empty());
        let same_bytes = SourceText::from_shared(Arc::new((*shared).clone())).unwrap();
        assert!(single_text_change(&same_arc, &same_bytes).is_empty());

        let changed = SourceText::from_shared(Arc::new("prefix кафé suffix".to_owned())).unwrap();
        let changes = single_text_change(&same_arc, &changed);
        assert_eq!(changes.len(), 1);
        let change = &changes[0];
        assert!(same_arc
            .shared()
            .is_char_boundary(change.old_range.start().to_usize()));
        assert!(same_arc
            .shared()
            .is_char_boundary(change.old_range.end().to_usize()));
        assert_eq!(same_arc.slice(change.old_range).unwrap(), "caf");
        assert_eq!(&*change.replacement, "каф");
    }

    #[test]
    fn shell_projection_rejects_tree_source_mismatch() {
        let source = SourceBundle::try_from_pairs([("one.md", "# one")]).unwrap();
        let mut analysis = analyze_okf(&source, None, 1).unwrap();
        let id = analysis
            .catalog
            .id_for_path(source.documents()[0].path())
            .unwrap();
        let other_text = SourceText::from_shared(Arc::new("# other".to_owned())).unwrap();
        let other = parse_markdown(
            DocumentRevision::new(1),
            other_text,
            MarkdownDialect::WAML_DEFAULT,
        )
        .unwrap();
        let mut documents = (*analysis.markdown.documents).clone();
        documents.insert(id, other);
        analysis.markdown.documents = Arc::new(documents);

        assert!(matches!(
            okf::shell::derive(&analysis.catalog, &analysis.markdown),
            Err(AnalysisError::StructuralInvariant {
                stage: AnalysisStage::Shell,
                ..
            })
        ));
    }

    struct PhaseHooks {
        fail_at: Option<AnalysisStage>,
        calls: Vec<AnalysisStage>,
    }

    impl PreparationHooks for PhaseHooks {
        fn before(&mut self, stage: AnalysisStage) -> Result<(), AnalysisError> {
            self.calls.push(stage);
            let should_fail = matches!(
                (self.fail_at, stage),
                (Some(AnalysisStage::Shell), AnalysisStage::Shell)
                    | (Some(AnalysisStage::Okf), AnalysisStage::Okf)
                    | (
                        Some(AnalysisStage::Specialization("uml")),
                        AnalysisStage::Specialization("uml")
                    )
                    | (Some(AnalysisStage::Claims), AnalysisStage::Claims)
            );
            if should_fail {
                return Err(AnalysisError::StructuralInvariant {
                    stage,
                    reason: "injected".into(),
                });
            }
            Ok(())
        }
    }

    fn phase_names(stages: &[AnalysisStage]) -> Vec<&'static str> {
        stages
            .iter()
            .map(|stage| match stage {
                AnalysisStage::Shell => "shell",
                AnalysisStage::Okf => "okf",
                AnalysisStage::Specialization("uml") => "uml",
                AnalysisStage::Specialization(_) => "other-specialization",
                AnalysisStage::Claims => "claims",
            })
            .collect()
    }

    #[test]
    fn prepare_candidate_runs_static_phases_once() {
        let source =
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap();
        let mut hooks = PhaseHooks {
            fail_at: None,
            calls: Vec::new(),
        };

        let prepared = prepare_candidate_inner(source, None, 11, &mut hooks).unwrap();

        assert_eq!(phase_names(&hooks.calls), ["shell", "okf", "uml", "claims"]);
        assert_eq!(prepared.revision(), 11);
        assert_eq!(prepared.okf().catalog.session_revision(), 11);
        assert_eq!(prepared.uml().session_revision(), 11);
        assert_eq!(prepared.uml().claims.iter().collect::<Vec<_>>(), ["order"]);
        let (source, okf, uml, revision) = prepared.into_parts();
        assert_eq!(source.documents().len(), 1);
        assert_eq!(okf.catalog.session_revision(), 11);
        assert_eq!(uml.session_revision(), 11);
        assert_eq!(revision, 11);
    }

    #[test]
    fn prepare_candidate_failure_is_non_mutating() {
        let committed = prepare_candidate(
            SourceBundle::try_from_pairs([("order.md", "---\ntype: uml.Class\n---\n# Order\n")])
                .unwrap(),
            None,
            4,
        )
        .unwrap();
        let committed_source = committed.source().documents()[0].text_arc().clone();
        let committed_catalog = committed.okf().catalog.clone();
        let committed_uml_catalog = committed.uml().syntax.catalog().clone();
        let candidate_source = SourceBundle::try_from_pairs([
            ("order.md", "---\ntype: uml.Class\n---\n# Changed Order\n"),
            ("notes.md", "---\ntype: Notes\n---\n# Notes\n"),
        ])
        .unwrap();

        for fail_at in [
            AnalysisStage::Shell,
            AnalysisStage::Okf,
            AnalysisStage::Specialization("uml"),
            AnalysisStage::Claims,
        ] {
            let mut hooks = PhaseHooks {
                fail_at: Some(fail_at),
                calls: Vec::new(),
            };
            let result = prepare_candidate_inner(
                candidate_source.clone(),
                Some(PreviousAnalyses {
                    okf: committed.okf(),
                    uml: committed.uml(),
                }),
                5,
                &mut hooks,
            );
            assert!(matches!(
                result,
                Err(AnalysisError::StructuralInvariant { stage, .. })
                    if phase_names(&[stage]) == phase_names(&[fail_at])
            ));
            assert!(Arc::ptr_eq(
                committed.source().documents()[0].text_arc(),
                &committed_source
            ));
            assert!(Arc::ptr_eq(&committed.okf().catalog, &committed_catalog));
            assert!(Arc::ptr_eq(
                committed.uml().syntax.catalog(),
                &committed_uml_catalog
            ));
            assert_eq!(committed.revision(), 4);
        }
    }

    #[test]
    fn disjoint_claim_validation_sorts_concepts_and_analyzers() {
        let uml = ClaimSet::from_concept_ids(["z".into(), "shared".into()]);
        let future = ClaimSet::from_concept_ids(["a".into(), "shared".into()]);
        assert!(matches!(
            validate_disjoint_claims([("uml", &uml), ("future", &future)]),
            Err(AnalysisError::AmbiguousClaim {
                concept_id,
                first,
                second,
            }) if concept_id == "shared" && first == "future" && second == "uml"
        ));
    }
}
