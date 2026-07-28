use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use waml_syntax::{
    parse_okf_markdown, LineIndex, MarkdownDialect, MarkdownStructureMap, OkfMarkdownLanguage,
    ParseError, SourceText, SyntaxLanguage, SyntaxTree,
};

use crate::{
    okf,
    source::{BundlePath, SourceBundle},
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentId(u64);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DocumentRevision(u64);

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
    pub(crate) fn documents(&self) -> &BTreeMap<DocumentId, Arc<SyntaxSnapshot<L>>> {
        &self.documents
    }
}

pub struct OkfAnalysis {
    pub catalog: Arc<DocumentCatalog>,
    pub shell: SyntaxSet<OkfMarkdownLanguage>,
    pub structures: Arc<BTreeMap<DocumentId, Arc<MarkdownStructureMap>>>,
    pub bundle: okf::Bundle,
}

pub struct DomainAnalysisContext<'a> {
    pub source: &'a SourceBundle,
    pub catalog: &'a Arc<DocumentCatalog>,
    pub shell: &'a SyntaxSet<OkfMarkdownLanguage>,
    pub structures: &'a Arc<BTreeMap<DocumentId, Arc<MarkdownStructureMap>>>,
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

fn prepare_candidate_inner(
    candidate_source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    candidate_revision: u64,
    hooks: &mut impl PreparationHooks,
) -> Result<PreparedCandidate, AnalysisError> {
    let okf = analyze_okf_inner(
        &candidate_source,
        previous.as_ref().map(|analyses| analyses.okf),
        candidate_revision,
        hooks,
    )?;
    hooks.before(AnalysisStage::Specialization("uml"))?;
    let uml = crate::uml::analyze(
        DomainAnalysisContext {
            source: &candidate_source,
            catalog: &okf.catalog,
            shell: &okf.shell,
            structures: &okf.structures,
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
    }

    impl PreparationProbe {
        pub fn succeed() -> Self {
            Self {
                fail_at: None,
                calls: Vec::new(),
            }
        }

        pub fn fail_at(stage: AnalysisStage) -> Self {
            Self {
                fail_at: Some(stage),
                calls: Vec::new(),
            }
        }

        pub fn phase_names(&self) -> Vec<&'static str> {
            self.calls.iter().map(phase_name).collect()
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
    }

    pub fn prepare_candidate_with_probe(
        candidate_source: SourceBundle,
        previous: Option<PreviousAnalyses<'_>>,
        candidate_revision: u64,
        probe: &mut PreparationProbe,
    ) -> Result<PreparedCandidate, AnalysisError> {
        prepare_candidate_inner(candidate_source, previous, candidate_revision, probe)
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
        &mut NoopPreparationHooks,
    )
}

fn analyze_okf_inner(
    source: &SourceBundle,
    previous: Option<&OkfAnalysis>,
    session_revision: u64,
    hooks: &mut impl PreparationHooks,
) -> Result<OkfAnalysis, AnalysisError> {
    let previous_catalog = previous.map(|analysis| &analysis.catalog);
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
            Some(prior) => Arc::new(version(
                prior.id(),
                DocumentRevision(prior.revision().0 + 1),
                path.clone(),
                source_document.text_arc().clone(),
            )?),
            None => {
                let id = DocumentId(next_id);
                next_id += 1;
                Arc::new(version(
                    id,
                    DocumentRevision(1),
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
    hooks.before(AnalysisStage::Shell)?;
    let mut shell_documents = BTreeMap::new();
    let mut structures = BTreeMap::new();
    for document in candidate.documents.values() {
        if let Some(previous_snapshot) =
            previous.and_then(|analysis| analysis.shell.document(document.id()))
        {
            if Arc::ptr_eq(previous_snapshot.document(), document) {
                shell_documents.insert(document.id(), previous_snapshot.clone());
                if let Some(structure) =
                    previous.and_then(|analysis| analysis.structures.get(&document.id()))
                {
                    structures.insert(document.id(), structure.clone());
                    continue;
                }
            }
        }
        let parsed =
            parse_okf_markdown(document.text().clone(), MarkdownDialect::CommonMarkCurrent)
                .map_err(|source| shell_error(document.path().clone(), source))?;
        structures.insert(document.id(), parsed.structure);
        shell_documents.insert(
            document.id(),
            Arc::new(SyntaxSnapshot {
                document: document.clone(),
                syntax: parsed.tree,
            }),
        );
    }
    hooks.before(AnalysisStage::Okf)?;
    let shell = SyntaxSet {
        catalog: candidate.clone(),
        documents: Arc::new(shell_documents),
    };
    let structures = Arc::new(structures);
    let bundle = okf::shell::derive(&candidate, &shell, &structures)?;
    Ok(OkfAnalysis {
        catalog: candidate.clone(),
        shell,
        structures,
        bundle,
    })
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
                analyze_okf_inner(&candidate_source, Some(&committed), 2, &mut hooks),
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
                DocumentRevision(2)
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
    fn shell_projection_rejects_tree_source_mismatch() {
        let source = SourceBundle::try_from_pairs([("one.md", "# one")]).unwrap();
        let mut analysis = analyze_okf(&source, None, 1).unwrap();
        let id = analysis
            .catalog
            .id_for_path(source.documents()[0].path())
            .unwrap();
        let other_text = SourceText::from_shared(Arc::new("# other".to_owned())).unwrap();
        let other = parse_okf_markdown(other_text, MarkdownDialect::CommonMarkCurrent).unwrap();
        let mut documents = (*analysis.shell.documents).clone();
        documents.insert(
            id,
            Arc::new(SyntaxSnapshot {
                document: analysis.catalog.document(id).unwrap().clone(),
                syntax: other.tree,
            }),
        );
        analysis.shell.documents = Arc::new(documents);

        assert!(matches!(
            okf::shell::derive(&analysis.catalog, &analysis.shell, &analysis.structures),
            Err(AnalysisError::StructuralInvariant {
                stage: AnalysisStage::Shell,
                ..
            })
        ));
    }

    #[test]
    fn shell_projection_rejects_missing_structure_map() {
        let source = SourceBundle::try_from_pairs([("one.md", "# one")]).unwrap();
        let mut analysis = analyze_okf(&source, None, 1).unwrap();
        analysis.structures = Arc::new(BTreeMap::new());

        assert!(matches!(
            okf::shell::derive(&analysis.catalog, &analysis.shell, &analysis.structures),
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
