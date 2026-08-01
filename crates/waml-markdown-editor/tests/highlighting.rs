use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

use waml_markdown_editor::presentation::{
    compile_presentation, CodeHighlightError, CodeHighlightHost, CodeHighlightRequest,
    CodeHighlightResult, CodeHighlightSpan, CodeTokenRole, HighlightOutcome, HighlighterRegistry,
    PresentationItem, PresentationStyles, TextRole,
};
use waml_syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText, SyntaxIdentity, TextRange,
    TextSize,
};

const SOURCE: &str = "```waml\ntype: uml.class\n```\n";

fn t(value: usize) -> TextSize {
    TextSize::try_from_usize(value).unwrap()
}

fn range(bounds: Range<usize>) -> TextRange {
    TextRange::new(t(bounds.start), t(bounds.end)).unwrap()
}

fn span(bounds: Range<usize>, role: CodeTokenRole) -> CodeHighlightSpan {
    CodeHighlightSpan {
        range: range(bounds),
        role,
    }
}

fn source() -> SourceText {
    SourceText::new(SOURCE.to_owned()).unwrap()
}

fn owner(value: u64) -> SyntaxIdentity {
    SyntaxIdentity::from_raw_for_test(value)
}

fn request(language: &str, bounds: Range<usize>) -> CodeHighlightRequest {
    CodeHighlightRequest {
        revision: DocumentRevision::INITIAL,
        owner: owner(3),
        language: Arc::from(language),
        content_range: range(bounds),
    }
}

#[derive(Default)]
struct RecordingHost {
    result: Mutex<Option<Result<CodeHighlightResult, CodeHighlightError>>>,
    seen: Mutex<Vec<CodeHighlightRequest>>,
}

impl RecordingHost {
    fn returning(result: Result<CodeHighlightResult, CodeHighlightError>) -> Arc<Self> {
        Arc::new(Self {
            result: Mutex::new(Some(result)),
            seen: Mutex::new(Vec::new()),
        })
    }

    fn only_request(&self) -> CodeHighlightRequest {
        let seen = self.seen.lock().unwrap();
        assert_eq!(seen.len(), 1, "the host is called exactly once");
        seen[0].clone()
    }
}

impl CodeHighlightHost for RecordingHost {
    fn highlight(
        &self,
        request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError> {
        self.seen.lock().unwrap().push(request.clone());
        self.result
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(Err(CodeHighlightError::Host(Arc::from("no result"))))
    }
}

fn registry_with(language: &str, host: Arc<RecordingHost>) -> HighlighterRegistry {
    let mut registry = HighlighterRegistry::default();
    registry.register(language, host);
    registry
}

fn result(spans: impl IntoIterator<Item = CodeHighlightSpan>) -> CodeHighlightResult {
    CodeHighlightResult {
        revision: DocumentRevision::INITIAL,
        owner: owner(3),
        spans: spans.into_iter().collect::<Vec<_>>().into(),
    }
}

#[test]
fn a_registered_host_receives_the_normalized_language_and_exact_content_range() {
    let host = RecordingHost::returning(Ok(result([span(9..13, CodeTokenRole::Keyword)])));
    let outcome =
        registry_with("waml", host.clone()).highlight(&request("WAML ", 9..24), &source());
    assert_eq!(
        outcome,
        HighlightOutcome::Classified(vec![span(9..13, CodeTokenRole::Keyword)].into())
    );
    let seen = host.only_request();
    assert_eq!(seen.language.as_ref(), "waml");
    assert_eq!(seen.content_range, range(9..24));
    assert_eq!(seen.revision, DocumentRevision::INITIAL);
}

#[test]
fn an_unknown_language_is_unclassified_and_never_calls_a_host() {
    assert_eq!(
        HighlighterRegistry::default().highlight(&request("rust", 9..24), &source()),
        HighlightOutcome::Unclassified
    );
}

#[test]
fn host_failures_and_invalid_results_stay_local_failures() {
    let failing = RecordingHost::returning(Err(CodeHighlightError::Host(Arc::from("boom"))));
    assert!(matches!(
        registry_with("waml", failing).highlight(&request("waml", 9..24), &source()),
        HighlightOutcome::Failed(CodeHighlightError::Host(_))
    ));

    let out_of_range = RecordingHost::returning(Ok(result([span(0..30, CodeTokenRole::Keyword)])));
    assert!(matches!(
        registry_with("waml", out_of_range).highlight(&request("waml", 9..24), &source()),
        HighlightOutcome::Failed(CodeHighlightError::OutOfBounds { .. })
    ));

    let overlapping = RecordingHost::returning(Ok(result([
        span(9..14, CodeTokenRole::Keyword),
        span(12..16, CodeTokenRole::Type),
    ])));
    assert!(matches!(
        registry_with("waml", overlapping).highlight(&request("waml", 9..24), &source()),
        HighlightOutcome::Failed(CodeHighlightError::Overlap { .. })
    ));

    let stale = RecordingHost::returning(Ok(CodeHighlightResult {
        revision: DocumentRevision::new(9),
        owner: owner(3),
        spans: Arc::from([]),
    }));
    assert!(matches!(
        registry_with("waml", stale).highlight(&request("waml", 9..24), &source()),
        HighlightOutcome::Failed(CodeHighlightError::StaleRevision { .. })
    ));

    let wrong_owner = RecordingHost::returning(Ok(CodeHighlightResult {
        revision: DocumentRevision::INITIAL,
        owner: owner(4),
        spans: Arc::from([]),
    }));
    assert!(matches!(
        registry_with("waml", wrong_owner).highlight(&request("waml", 9..24), &source()),
        HighlightOutcome::Failed(CodeHighlightError::WrongOwner { .. })
    ));
}

/// Highlights the four bytes of `type` inside the fenced block.
struct KeywordHost;

impl CodeHighlightHost for KeywordHost {
    fn highlight(
        &self,
        request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError> {
        Ok(CodeHighlightResult {
            revision: request.revision,
            owner: request.owner,
            spans: vec![CodeHighlightSpan {
                range: TextRange::new(
                    request.content_range.start(),
                    t(request.content_range.start().to_usize() + 4),
                )
                .unwrap(),
                role: CodeTokenRole::Keyword,
            }]
            .into(),
        })
    }
}

struct FailingHost;

impl CodeHighlightHost for FailingHost {
    fn highlight(
        &self,
        _request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError> {
        Err(CodeHighlightError::Host(Arc::from("host is unavailable")))
    }
}

fn compile_with(registry: HighlighterRegistry) -> (SourceText, Vec<(TextRange, TextRole)>, usize) {
    let snapshot = parse_markdown(
        DocumentRevision::INITIAL,
        source(),
        MarkdownDialect::WAML_DEFAULT,
    )
    .unwrap();
    let plan = compile_presentation(&snapshot, &PresentationStyles::balanced(), &registry).unwrap();
    assert_eq!(plan.validate_source_partition(), Ok(()));
    let runs = plan
        .items
        .iter()
        .filter_map(|item| match item {
            PresentationItem::TextRun { range, role, .. } => Some((*range, *role)),
            _ => None,
        })
        .collect();
    (source(), runs, plan.diagnostics.len())
}

#[test]
fn highlight_roles_override_only_code_content_and_keep_the_partition() {
    let mut registry = HighlighterRegistry::default();
    registry.register("waml", Arc::new(KeywordHost));
    let (text, runs, diagnostics) = compile_with(registry);
    assert_eq!(diagnostics, 0);

    let token = runs
        .iter()
        .find(|(_, role)| matches!(role, TextRole::CodeToken(_)))
        .expect("the keyword token is present");
    assert_eq!(text.slice(token.0).unwrap(), "type");
    assert_eq!(token.1, TextRole::CodeToken(CodeTokenRole::Keyword));

    // The gap after the token stays unclassified code content, and the fence
    // and info string keep their own roles.
    assert!(runs
        .iter()
        .any(|(range, role)| *role == TextRole::CodeContent
            && text.slice(*range).unwrap().starts_with(':')));
    assert!(runs.iter().any(|(_, role)| *role == TextRole::CodeFence));
    assert!(runs.iter().any(|(_, role)| *role == TextRole::CodeInfo));
}

#[test]
fn a_failing_host_keeps_unclassified_code_and_one_local_diagnostic() {
    let mut registry = HighlighterRegistry::default();
    registry.register("waml", Arc::new(FailingHost));
    let (_, runs, diagnostics) = compile_with(registry);
    assert_eq!(diagnostics, 1);
    assert!(!runs
        .iter()
        .any(|(_, role)| matches!(role, TextRole::CodeToken(_))));
    assert!(runs.iter().any(|(_, role)| *role == TextRole::CodeContent));
}
