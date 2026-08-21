//! Lossless syntax primitives for Markdown and the WAML islands inside it.
//!
//! The core — green/red trees, text, incremental reparse — is domain-neutral
//! and parameterised over a `SyntaxLanguage`. The crate as a whole is not: it
//! exports `WamlLanguageIsland`, `WamlSectionKind` and
//! `OkfSyntaxDiagnosticCode`, because island recognition has to happen during
//! the one Markdown parse rather than in a second pass above it. That is a
//! deliberate trade, not an accident, and the headline used to claim
//! otherwise.

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
    scan_quoted_scalar, FrontmatterScalarKind, QuotedScalarScan,
};
pub use markdown::{
    has_leading_frontmatter_fence, leading_frontmatter_slice, parse_markdown, reparse_markdown,
    ConfirmedHeading, FencedCodeInfo, HtmlTagFilter, MarkdownEntity, MarkdownHeading,
    MarkdownImage, MarkdownLink, MarkdownLinkKind, MarkdownList, MarkdownListKind, MarkdownRawHtml,
    MarkdownReparseOutcome, MarkdownSemanticRole, MarkdownSourceRole, MarkdownStructureMap,
    MarkdownSyntaxQueries, MarkdownSyntaxSnapshot, MarkdownSyntaxSpan, MarkdownSyntaxUpdate,
    MarkdownTableCell, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode,
    SyntaxIdentity, TableAlignment, TaskListState, WamlLanguageIsland, WamlSectionKind,
    FRONTMATTER_MAX_NESTING_DEPTH,
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
