//! Lossless, domain-neutral syntax primitives.

mod annotation;
mod ast;
mod green;
mod incremental;
mod markdown;
mod red;
mod shell;
mod text;

pub use annotation::{annotate_occurrence, find_annotation, syntax_identity, SyntaxAnnotation};
pub use ast::{AstNode, AstSlots, SyntaxRewriter, SyntaxVisitor};
pub use green::{
    write_green_to, GreenElement, GreenError, GreenFactory, GreenNode, GreenNodeData, GreenText,
    GreenToken, GreenTokenData, GreenTrivia, TokenFlags, TriviaKind,
};
pub use incremental::{
    rebase_unchanged_green, transfer_mapped_annotations, ChangeMap, ChangeSegment,
    FullReparseReason, RebasedGreen, ReparseOutcome, TextChange,
};
pub use markdown::scalar::{
    classify_bare_scalar, decode_double_quoted_body, decode_quoted_scalar, normalize_line_endings,
    FrontmatterScalarKind,
};
pub use markdown::{
    leading_frontmatter_slice, parse_markdown, reparse_markdown, ConfirmedHeading, FencedCodeInfo,
    HtmlTagFilter, MarkdownEntity, MarkdownHeading, MarkdownImage, MarkdownLink, MarkdownLinkKind,
    MarkdownList, MarkdownListKind, MarkdownRawHtml, MarkdownReparseOutcome, MarkdownSemanticRole,
    MarkdownSourceRole, MarkdownStructureMap, MarkdownSyntaxQueries, MarkdownSyntaxSnapshot,
    MarkdownSyntaxSpan, MarkdownSyntaxUpdate, MarkdownTableCell, OkfMarkdownLanguage,
    OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode, SyntaxIdentity, TableAlignment, TaskListState,
    WamlLanguageIsland, WamlSectionKind, FRONTMATTER_MAX_NESTING_DEPTH,
};
pub use red::{
    RewriteError, SyntaxElement, SyntaxLocator, SyntaxNode, SyntaxPath, SyntaxSeverity,
    SyntaxToken, SyntaxTree, TreeDiagnostic, TreeInstanceId,
};
pub use shell::{ParseError, ShellParse};
pub use text::{
    DocumentRevision, LineColumn, LineIndex, MarkdownDialect, SourceText, TextError, TextRange,
    TextSize,
};

use std::{fmt::Debug, hash::Hash};

pub trait SyntaxLanguage: Send + Sync + 'static {
    type Kind: Copy + Eq + Hash + Debug + Send + Sync;
    type DiagnosticCode: Copy + Eq + Hash + Debug + Send + Sync;
}
