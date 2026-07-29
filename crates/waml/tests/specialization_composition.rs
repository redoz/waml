use std::{collections::BTreeMap, ops::Deref, sync::Arc};

use waml::{
    analysis::{
        prepare_candidate, validate_disjoint_claims, AnalysisError, ClaimSet, DocumentCatalog,
        DocumentId, DocumentVersion, DomainAnalysisContext, PreparedCandidate, PreviousAnalyses,
    },
    source::{BundlePath, SourceBundle},
};
use waml_syntax::{
    GreenElement, GreenFactory, GreenText, MarkdownDialect, SyntaxLanguage, SyntaxTree,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum FutureSyntaxKind {
    Document,
    RawText,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum FutureDiagnosticCode {}

struct FutureLanguage;

impl SyntaxLanguage for FutureLanguage {
    type Kind = FutureSyntaxKind;
    type DiagnosticCode = FutureDiagnosticCode;
}

#[derive(Debug, Eq, PartialEq)]
struct FutureDeclared {
    concept_id: String,
    kind: String,
}

struct FutureAnalysis {
    claims: ClaimSet,
    catalog: Arc<DocumentCatalog>,
    syntax: BTreeMap<DocumentId, (Arc<DocumentVersion>, Arc<SyntaxTree<FutureLanguage>>)>,
    declared: Arc<[FutureDeclared]>,
}

fn analyze_future(context: DomainAnalysisContext<'_>) -> FutureAnalysis {
    let factory = GreenFactory::<FutureLanguage>::new();
    let claimed: Vec<_> = context
        .okf
        .concepts()
        .iter()
        .filter(|concept| {
            concept.ty == "future.Widget" || concept.title.as_deref() == Some("Future Collision")
        })
        .collect();
    let claims = ClaimSet::from_concept_ids(claimed.iter().map(|concept| concept.id.to_owned()));
    let mut syntax = BTreeMap::new();
    let mut declared = Vec::new();

    for concept in claimed {
        let path = BundlePath::parse(format!("{}.md", concept.id)).unwrap();
        let id = context.catalog.id_for_path(&path).unwrap();
        let document = context.catalog.document(id).unwrap().clone();
        let raw = factory
            .token(
                FutureSyntaxKind::RawText,
                GreenText::Owned(Arc::from(document.text().shared().as_str())),
                [],
                [],
            )
            .unwrap();
        let root = factory
            .node(FutureSyntaxKind::Document, [GreenElement::Token(raw)])
            .unwrap();
        let tree = Arc::new(SyntaxTree::new(
            root,
            Arc::from([]),
            MarkdownDialect::CommonMarkCurrent,
        ));
        syntax.insert(id, (document, tree));
        declared.push(FutureDeclared {
            concept_id: concept.id.clone(),
            kind: concept.ty.clone(),
        });
    }

    FutureAnalysis {
        claims,
        catalog: context.catalog.clone(),
        syntax,
        declared: declared.into(),
    }
}

struct StaticTestCandidate {
    core: PreparedCandidate,
    future: FutureAnalysis,
}

impl StaticTestCandidate {
    fn previous(&self) -> PreviousAnalyses<'_> {
        PreviousAnalyses {
            okf: self.core.okf(),
            uml: self.core.uml(),
        }
    }
}

impl Deref for StaticTestCandidate {
    type Target = PreparedCandidate;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

fn prepare_static_test_candidate(
    candidate_source: SourceBundle,
    previous: Option<PreviousAnalyses<'_>>,
    candidate_revision: u64,
) -> Result<StaticTestCandidate, AnalysisError> {
    let core = prepare_candidate(candidate_source, previous, candidate_revision)?;
    let future = analyze_future(context(core.source(), core.okf()));
    validate_disjoint_claims([("uml", &core.uml().claims), ("future", &future.claims)])?;
    Ok(StaticTestCandidate { core, future })
}

struct StaticTestOwner {
    current: StaticTestCandidate,
    dirty: bool,
    install_count: u64,
}

impl StaticTestOwner {
    fn new(source: SourceBundle, revision: u64) -> Self {
        Self {
            current: prepare_static_test_candidate(source, None, revision).unwrap(),
            dirty: false,
            install_count: 0,
        }
    }

    fn try_install(
        &mut self,
        candidate_source: SourceBundle,
        candidate_revision: u64,
    ) -> Result<(), AnalysisError> {
        let candidate = prepare_static_test_candidate(
            candidate_source,
            Some(self.current.previous()),
            candidate_revision,
        )?;
        self.current = candidate;
        self.dirty = true;
        self.install_count += 1;
        Ok(())
    }
}

fn mixed_source() -> SourceBundle {
    SourceBundle::try_from_pairs([
        (
            "class.md",
            "---\ntype: uml.Class\n---\n# Class\n\n## Attributes\n- id: String\n",
        ),
        (
            "widget.md",
            "---\ntype: future.Widget\n---\n# Widget\n\nFuture body.\n",
        ),
        ("arbitrary.md", "---\ntype: Runbook\n---\n# Arbitrary\n"),
        ("missing.md", "---\ntitle: Missing\n---\n# Missing\n"),
        (
            "unknown-uml.md",
            "---\ntype: uml.NotYetAType\n---\n# Unknown UML\n",
        ),
        ("index.md", "# Root\n\n* [Class](class.md)\n"),
        ("log.md", "# Log\n\n- entry\n"),
        ("nested/index.md", "# Nested\n\n* [Widget](../widget.md)\n"),
    ])
    .unwrap()
}

fn context<'a>(
    source: &'a SourceBundle,
    okf: &'a waml::analysis::OkfAnalysis,
) -> DomainAnalysisContext<'a> {
    DomainAnalysisContext {
        source,
        catalog: &okf.catalog,
        shell: &okf.shell,
        structures: &okf.structures,
        okf: &okf.bundle,
        session_revision: okf.catalog.session_revision(),
    }
}

#[test]
fn future_sibling_composes_without_expanding_okf_or_uml_kinds() {
    let source = mixed_source();
    let prepared = prepare_static_test_candidate(source, None, 7).unwrap();
    let future = &prepared.future;

    assert_eq!(prepared.revision(), 7);
    assert_eq!(prepared.uml().claims.iter().collect::<Vec<_>>(), ["class"]);
    assert_eq!(future.claims.iter().collect::<Vec<_>>(), ["widget"]);
    assert_eq!(
        future.declared.as_ref(),
        [FutureDeclared {
            concept_id: "widget".into(),
            kind: "future.Widget".into(),
        }]
    );
    assert!(Arc::ptr_eq(&future.catalog, &prepared.okf().catalog));

    let widget_id = prepared
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("widget.md").unwrap())
        .unwrap();
    let (future_document, future_tree) = future.syntax.get(&widget_id).unwrap();
    assert!(Arc::ptr_eq(
        future_document,
        prepared.okf().catalog.document(widget_id).unwrap()
    ));
    assert_eq!(
        future_tree.write_to_string(),
        prepared
            .source()
            .document(&BundlePath::parse("widget.md").unwrap())
            .unwrap()
            .text()
    );

    for generic in ["arbitrary", "missing", "unknown-uml"] {
        assert!(!prepared.uml().claims.contains(generic));
        assert!(!future.claims.contains(generic));
        assert!(prepared.okf().bundle.concept(generic).is_some());
        assert!(prepared.uml().declared.concept(generic).is_none());
    }
    assert_eq!(prepared.okf().bundle.indexes().len(), 2);
    assert_eq!(prepared.okf().bundle.logs().len(), 1);
    assert!(!prepared.okf().bundle.directories().is_empty());
    assert!(prepared.uml().projection.packages.is_empty());
}

#[test]
fn ambiguous_static_candidate_is_rejected_before_owner_install_and_counter_commit() {
    let committed_source =
        SourceBundle::try_from_pairs([("base.md", "---\ntype: uml.Class\n---\n# Base\n")]).unwrap();
    let mut owner = StaticTestOwner::new(committed_source, 4);
    let committed_catalog = owner.current.core.okf().catalog.clone();
    let committed_future_catalog = owner.current.future.catalog.clone();
    let base_id = owner
        .current
        .core
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("base.md").unwrap())
        .unwrap();
    let committed_source_allocation = owner
        .current
        .core
        .okf()
        .catalog
        .document(base_id)
        .unwrap()
        .text()
        .shared()
        .clone();

    let collision = SourceBundle::try_from_pairs([
        ("base.md", "---\ntype: uml.Class\n---\n# Base\n"),
        (
            "shared.md",
            "---\ntype: uml.Class\ntitle: Future Collision\n---\n# Shared\n",
        ),
    ])
    .unwrap();
    let error = owner.try_install(collision, 5).unwrap_err();
    assert!(matches!(
        error,
        waml::analysis::AnalysisError::AmbiguousClaim {
            concept_id,
            first,
            second,
        } if concept_id == "shared" && first == "future" && second == "uml"
    ));
    assert_eq!(owner.current.core.revision(), 4);
    assert!(Arc::ptr_eq(
        owner
            .current
            .core
            .okf()
            .catalog
            .document(base_id)
            .unwrap()
            .text()
            .shared(),
        &committed_source_allocation
    ));
    assert!(Arc::ptr_eq(
        &owner.current.core.okf().catalog,
        &committed_catalog
    ));
    assert!(Arc::ptr_eq(
        &owner.current.future.catalog,
        &committed_future_catalog
    ));
    assert_eq!(
        owner
            .current
            .core
            .okf()
            .catalog
            .id_for_path(&BundlePath::parse("base.md").unwrap()),
        Some(base_id)
    );
    assert!(owner
        .current
        .core
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("shared.md").unwrap())
        .is_none());
    assert!(!owner.dirty);
    assert_eq!(owner.install_count, 0);

    let disjoint = SourceBundle::try_from_pairs([
        ("base.md", "---\ntype: uml.Class\n---\n# Base\n"),
        ("shared.md", "---\ntype: future.Widget\n---\n# Shared\n"),
    ])
    .unwrap();
    let control =
        prepare_static_test_candidate(disjoint.clone(), Some(owner.current.previous()), 5).unwrap();
    let expected_shared_id = control
        .core
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("shared.md").unwrap())
        .unwrap();

    owner.try_install(disjoint, 5).unwrap();
    assert_eq!(owner.current.core.revision(), 5);
    assert!(owner.dirty);
    assert_eq!(owner.install_count, 1);
    assert_eq!(
        owner
            .current
            .core
            .okf()
            .catalog
            .id_for_path(&BundlePath::parse("shared.md").unwrap()),
        Some(expected_shared_id)
    );
    assert_eq!(
        owner.current.future.claims.iter().collect::<Vec<_>>(),
        ["shared"]
    );
}

#[test]
fn malformed_claimed_syntax_is_prepared_with_diagnostics() {
    let source = SourceBundle::try_from_pairs([(
        "broken.md",
        "---\ntype: uml.Class\n---\n# Broken\n\n## Attributes\n- broken String [1..x]\n",
    )])
    .unwrap();

    let prepared = prepare_candidate(source, None, 3).unwrap();
    assert_eq!(prepared.revision(), 3);
    assert!(!prepared.uml().diagnostics.is_empty());
}
