//! Lossless, domain-neutral syntax primitives.

mod annotation;
mod ast;
mod green;
mod red;
mod text;

pub use annotation::{annotate_occurrence, find_annotation, SyntaxAnnotation};
pub use ast::{AstNode, AstSlots, SyntaxRewriter, SyntaxVisitor};
pub use green::{
    write_green_to, GreenElement, GreenError, GreenFactory, GreenNode, GreenNodeData, GreenText,
    GreenToken, GreenTokenData, GreenTrivia, TokenFlags, TriviaKind,
};
pub use red::{
    RewriteError, SyntaxElement, SyntaxLocator, SyntaxNode, SyntaxPath, SyntaxSeverity,
    SyntaxToken, SyntaxTree, TreeDiagnostic, TreeInstanceId,
};
pub use text::{
    LineColumn, LineIndex, MarkdownDialect, SourceText, TextError, TextRange, TextSize,
};

use std::{fmt::Debug, hash::Hash};

pub trait SyntaxLanguage: Send + Sync + 'static {
    type Kind: Copy + Eq + Hash + Debug + Send + Sync;
    type DiagnosticCode: Copy + Eq + Hash + Debug + Send + Sync;
}
