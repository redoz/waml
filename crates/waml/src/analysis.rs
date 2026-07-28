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
    pub fn catalog(&self) -> &Arc<DocumentCatalog> {
        &self.catalog
    }
    pub fn document(&self, id: DocumentId) -> Option<&Arc<SyntaxSnapshot<L>>> {
        self.documents.get(&id)
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
        first: &'static str,
        second: &'static str,
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

trait PreparationHooks {
    fn before(&mut self, stage: AnalysisStage) -> Result<(), AnalysisError>;
}
struct NoopPreparationHooks;
impl PreparationHooks for NoopPreparationHooks {
    fn before(&mut self, _: AnalysisStage) -> Result<(), AnalysisError> {
        Ok(())
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
}
