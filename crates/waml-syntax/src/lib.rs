//! Lossless, domain-neutral syntax primitives.

mod green;
mod text;

pub use green::{
    write_green_to, GreenElement, GreenError, GreenFactory, GreenNode, GreenNodeData, GreenText,
    GreenToken, GreenTokenData, GreenTrivia, TokenFlags, TriviaKind,
};
pub use text::{
    LineColumn, LineIndex, MarkdownDialect, SourceText, TextError, TextRange, TextSize,
};

use std::{fmt::Debug, hash::Hash};

pub trait SyntaxLanguage: Send + Sync + 'static {
    type Kind: Copy + Eq + Hash + Debug + Send + Sync;
    type DiagnosticCode: Copy + Eq + Hash + Debug + Send + Sync;
}
