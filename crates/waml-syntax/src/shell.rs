mod parser;
use crate::{MarkdownDialect, SourceText, SyntaxLanguage, SyntaxTree, TextRange, TextSize};
use std::{fmt, sync::Arc};
#[derive(Debug)]
pub struct OkfMarkdownLanguage;
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OkfMarkdownSyntaxKind {
    Root,
    Frontmatter,
    FrontmatterOpenFence,
    FrontmatterEntry,
    FrontmatterKey,
    ColonToken,
    FrontmatterValue,
    FrontmatterCloseFence,
    Heading,
    HeadingMarkerToken,
    HeadingText,
    MarkdownRegion,
    RawTextToken,
    NewlineToken,
    EndOfFileToken,
    BadToken,
    SkippedTokensSyntax,
}
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OkfSyntaxDiagnosticCode {
    FrontmatterNotClean,
    MissingFrontmatterFence,
    MalformedFrontmatterEntry,
    InvalidUtf8Boundary,
    ParserStalled,
}
impl SyntaxLanguage for OkfMarkdownLanguage {
    type Kind = OkfMarkdownSyntaxKind;
    type DiagnosticCode = OkfSyntaxDiagnosticCode;
}
#[derive(Debug)]
pub enum ParseError {
    SourceTooLarge { bytes: usize },
    InvalidRange { range: TextRange },
    WidthOverflow,
    StructuralInvariant { reason: Arc<str> },
    ParserStalled { offset: TextSize },
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
pub fn parse_okf_markdown(
    text: SourceText,
    dialect: MarkdownDialect,
) -> Result<ShellParse, ParseError> {
    parser::parse(text, dialect)
}
