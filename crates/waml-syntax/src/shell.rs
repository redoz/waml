mod parser;
pub use crate::markdown::{OkfMarkdownLanguage, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode};
use crate::{MarkdownDialect, SourceText, SyntaxTree, TextRange, TextSize};
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
        write!(f, "shell parse error: {self:?}")
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
pub fn parse_okf_markdown(
    text: SourceText,
    dialect: MarkdownDialect,
) -> Result<ShellParse, ParseError> {
    parser::parse(text, dialect)
}
pub(crate) fn parse_okf_markdown_with_structure(
    text: SourceText,
    dialect: MarkdownDialect,
    structure: Arc<crate::MarkdownStructureMap>,
) -> Result<ShellParse, ParseError> {
    parser::parse_with_structure(text, dialect, structure)
}
pub(crate) fn frontmatter_range(
    text: &SourceText,
    structure: &crate::MarkdownStructureMap,
) -> Result<Option<TextRange>, ParseError> {
    parser::frontmatter_range(text, structure)
}
pub(crate) fn parse_window(
    text: &SourceText,
    structure: &crate::MarkdownStructureMap,
    window: ShellWindow,
) -> Result<ParsedShellWindow, ParseError> {
    parser::parse_window(text, structure, window)
}
