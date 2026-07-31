//! Lossless, domain-neutral syntax primitives.

mod annotation;
mod ast;
mod green;
mod incremental;
mod markdown;
mod red;
mod shell;
mod text;

pub use annotation::{annotate_occurrence, find_annotation, SyntaxAnnotation};
pub use ast::{AstNode, AstSlots, SyntaxRewriter, SyntaxVisitor};
pub use green::{
    write_green_to, GreenElement, GreenError, GreenFactory, GreenNode, GreenNodeData, GreenText,
    GreenToken, GreenTokenData, GreenTrivia, TokenFlags, TriviaKind,
};
pub use incremental::{
    rebase_unchanged_green, reparse_okf_markdown, reparse_okf_markdown_with_structure,
    transfer_mapped_annotations, ChangeMap, ChangeSegment, FullReparseReason, RebasedGreen,
    ReparseOutcome, TextChange,
};
pub use markdown::{
    parse_markdown, reparse_markdown, ConfirmedHeading, MarkdownLink, MarkdownLinkKind,
    MarkdownReparseOutcome, MarkdownStructureMap, MarkdownSyntaxQueries, MarkdownSyntaxSnapshot,
    MarkdownSyntaxUpdate, OkfMarkdownLanguage, OkfMarkdownSyntaxKind, OkfSyntaxDiagnosticCode,
    SyntaxIdentity,
};
pub use red::{
    RewriteError, SyntaxElement, SyntaxLocator, SyntaxNode, SyntaxPath, SyntaxSeverity,
    SyntaxToken, SyntaxTree, TreeDiagnostic, TreeInstanceId,
};
pub use shell::{parse_okf_markdown, ParseError, ShellParse};
pub use text::{
    DocumentRevision, LineColumn, LineIndex, MarkdownDialect, SourceText, TextError, TextRange,
    TextSize,
};

use std::{fmt::Debug, hash::Hash};

pub trait SyntaxLanguage: Send + Sync + 'static {
    type Kind: Copy + Eq + Hash + Debug + Send + Sync;
    type DiagnosticCode: Copy + Eq + Hash + Debug + Send + Sync;
}
