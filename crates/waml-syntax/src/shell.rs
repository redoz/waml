pub use crate::markdown::{OkfMarkdownLanguage, OkfSyntaxDiagnosticCode};
use crate::{SourceText, SyntaxTree, TextRange, TextSize};
use std::{fmt, sync::Arc};
#[derive(Debug)]
pub enum ParseError {
    SourceTooLarge {
        bytes: usize,
    },
    InvalidRange {
        range: TextRange,
    },
    WidthOverflow,
    StructuralInvariant {
        reason: Arc<str>,
    },
    ParserStalled {
        offset: TextSize,
    },
    NonMonotonicRevision {
        previous: crate::DocumentRevision,
        requested: crate::DocumentRevision,
    },
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceTooLarge { bytes } => {
                write!(f, "source is too large to parse: {bytes} bytes")
            }
            Self::InvalidRange { range } => write!(
                f,
                "invalid text range: {}..{}",
                range.start().to_usize(),
                range.end().to_usize()
            ),
            Self::WidthOverflow => write!(f, "text width overflow"),
            Self::StructuralInvariant { reason } => {
                write!(f, "structural invariant violated: {reason}")
            }
            Self::ParserStalled { offset } => {
                write!(f, "parser stalled at byte offset {}", offset.to_usize())
            }
            Self::NonMonotonicRevision {
                previous,
                requested,
            } => write!(
                f,
                "document revision must increase: previous {}, requested {}",
                previous.get(),
                requested.get()
            ),
        }
    }
}
impl std::error::Error for ParseError {}
pub struct ShellParse {
    pub tree: Arc<SyntaxTree<OkfMarkdownLanguage>>,
    pub structure: Arc<crate::MarkdownStructureMap>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellWindowKind {
    Frontmatter,
    Heading,
    MarkdownRegion,
    Tail,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ShellWindow {
    pub kind: ShellWindowKind,
    pub range: TextRange,
}
pub(crate) struct ParsedShellWindow {
    pub elements: Arc<[crate::GreenElement<OkfMarkdownLanguage>]>,
    pub diagnostics: Arc<[crate::TreeDiagnostic<OkfSyntaxDiagnosticCode>]>,
}
pub(crate) fn frontmatter_range(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
) -> Result<Option<TextRange>, ParseError> {
    crate::markdown::parser::frontmatter_range(text, structure)
}
pub(crate) fn frontmatter_close_fence_line(
    source: &str,
    open_end: usize,
    boundary: usize,
) -> Option<crate::markdown::parser::Line> {
    crate::markdown::parser::frontmatter_close_fence_line(source, open_end, boundary)
}
pub(crate) fn parse_window(
    text: &SourceText,
    structure: &crate::markdown::ShellStructure,
    window: ShellWindow,
) -> Result<ParsedShellWindow, ParseError> {
    crate::markdown::parser::parse_window(text, structure, window)
}
