//! Registered, snapshot-based code highlighting.
//!
//! Highlighting is an application-owned host seam. This crate normalizes the
//! parsed language key, validates the host's result against the request, and
//! keeps every failure local to one fenced block.

use std::{collections::BTreeMap, fmt, sync::Arc};

use waml_syntax::{DocumentRevision, SourceText, SyntaxIdentity, TextRange, TextSize};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHighlightRequest {
    pub revision: DocumentRevision,
    pub owner: SyntaxIdentity,
    pub language: Arc<str>,
    pub content_range: TextRange,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CodeTokenRole {
    Keyword,
    Type,
    Property,
    String,
    Number,
    Comment,
    Punctuation,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHighlightSpan {
    pub range: TextRange,
    pub role: CodeTokenRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeHighlightResult {
    pub revision: DocumentRevision,
    pub owner: SyntaxIdentity,
    pub spans: Arc<[CodeHighlightSpan]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeHighlightError {
    OutOfBounds {
        content_range: TextRange,
        span: TextRange,
    },
    Overlap {
        previous_end: TextSize,
        next: TextRange,
    },
    InvalidUtf8Boundary(TextSize),
    StaleRevision {
        expected: DocumentRevision,
        actual: DocumentRevision,
    },
    WrongOwner {
        expected: SyntaxIdentity,
        actual: SyntaxIdentity,
    },
    Host(Arc<str>),
}

impl fmt::Display for CodeHighlightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfBounds {
                content_range,
                span,
            } => write!(
                f,
                "highlight span {}..{} leaves content range {}..{}",
                span.start().to_usize(),
                span.end().to_usize(),
                content_range.start().to_usize(),
                content_range.end().to_usize()
            ),
            Self::Overlap { previous_end, next } => write!(
                f,
                "highlight span {}..{} overlaps {}",
                next.start().to_usize(),
                next.end().to_usize(),
                previous_end.to_usize()
            ),
            Self::InvalidUtf8Boundary(offset) => {
                write!(f, "offset {} is not a UTF-8 boundary", offset.to_usize())
            }
            Self::StaleRevision { expected, actual } => write!(
                f,
                "highlight result is revision {} but the request is revision {}",
                actual.get(),
                expected.get()
            ),
            Self::WrongOwner { expected, actual } => write!(
                f,
                "highlight result owner {} does not match requested owner {}",
                actual.get(),
                expected.get()
            ),
            Self::Host(reason) => write!(f, "highlight host failed: {reason}"),
        }
    }
}

impl std::error::Error for CodeHighlightError {}

pub trait CodeHighlightHost: Send + Sync {
    fn highlight(
        &self,
        request: &CodeHighlightRequest,
    ) -> Result<CodeHighlightResult, CodeHighlightError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HighlightOutcome {
    Classified(Arc<[CodeHighlightSpan]>),
    Unclassified,
    Failed(CodeHighlightError),
}

/// Language-keyed highlight hosts. Keys are trimmed ASCII-lowercase language
/// identifiers taken from the parsed fence info.
#[derive(Default, Clone)]
pub struct HighlighterRegistry {
    hosts: BTreeMap<Arc<str>, Arc<dyn CodeHighlightHost>>,
}

impl fmt::Debug for HighlighterRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HighlighterRegistry")
            .field("languages", &self.hosts.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl HighlighterRegistry {
    pub fn register(
        &mut self,
        language: impl Into<Arc<str>>,
        host: Arc<dyn CodeHighlightHost>,
    ) -> Option<Arc<dyn CodeHighlightHost>> {
        self.hosts.insert(normalize(&language.into()), host)
    }

    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty()
    }

    /// Runs the registered host for the request's language and validates its
    /// result. An unknown language is `Unclassified`; every other problem is
    /// `Failed` and stays local to this block.
    pub fn highlight(
        &self,
        request: &CodeHighlightRequest,
        source: &SourceText,
    ) -> HighlightOutcome {
        let Some(host) = self.hosts.get(&normalize(&request.language)) else {
            return HighlightOutcome::Unclassified;
        };
        let normalized = CodeHighlightRequest {
            language: normalize(&request.language),
            ..request.clone()
        };
        match host.highlight(&normalized) {
            Ok(result) => match validate(&normalized, &result, source) {
                Ok(()) => HighlightOutcome::Classified(result.spans),
                Err(error) => HighlightOutcome::Failed(error),
            },
            Err(error) => HighlightOutcome::Failed(error),
        }
    }
}

fn normalize(language: &str) -> Arc<str> {
    Arc::from(language.trim().to_ascii_lowercase())
}

fn validate(
    request: &CodeHighlightRequest,
    result: &CodeHighlightResult,
    source: &SourceText,
) -> Result<(), CodeHighlightError> {
    if result.revision != request.revision {
        return Err(CodeHighlightError::StaleRevision {
            expected: request.revision,
            actual: result.revision,
        });
    }
    if result.owner != request.owner {
        return Err(CodeHighlightError::WrongOwner {
            expected: request.owner,
            actual: result.owner,
        });
    }
    let mut previous_end = request.content_range.start();
    for span in result.spans.iter() {
        if span.range.start() < request.content_range.start()
            || span.range.end() > request.content_range.end()
        {
            return Err(CodeHighlightError::OutOfBounds {
                content_range: request.content_range,
                span: span.range,
            });
        }
        if span.range.start() < previous_end {
            return Err(CodeHighlightError::Overlap {
                previous_end,
                next: span.range,
            });
        }
        for offset in [span.range.start(), span.range.end()] {
            if source.slice(range_to(offset, offset)?).is_err() {
                return Err(CodeHighlightError::InvalidUtf8Boundary(offset));
            }
        }
        previous_end = span.range.end();
    }
    Ok(())
}

fn range_to(start: TextSize, end: TextSize) -> Result<TextRange, CodeHighlightError> {
    TextRange::new(start, end).map_err(|_| CodeHighlightError::InvalidUtf8Boundary(start))
}
