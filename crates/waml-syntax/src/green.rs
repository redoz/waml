use std::{fmt, marker::PhantomData, sync::Arc};

use crate::{
    annotation::SyntaxAnnotation, SourceText, SyntaxLanguage, TextError, TextRange, TextSize,
};

pub type GreenNode<L> = Arc<GreenNodeData<L>>;
pub type GreenToken<L> = Arc<GreenTokenData<L>>;

#[derive(Debug)]
pub enum GreenElement<L: SyntaxLanguage> {
    Node(GreenNode<L>),
    Token(GreenToken<L>),
}
impl<L: SyntaxLanguage> Clone for GreenElement<L> {
    fn clone(&self) -> Self {
        match self {
            Self::Node(node) => Self::Node(node.clone()),
            Self::Token(token) => Self::Token(token.clone()),
        }
    }
}

#[derive(Clone, Debug)]
pub enum GreenText {
    Static(&'static str),
    SourceSlice {
        source: SourceText,
        range: TextRange,
    },
    Owned(Arc<str>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriviaKind {
    Whitespace,
    Newline,
    Comment,
}

#[derive(Clone, Debug)]
pub struct GreenTrivia {
    pub kind: TriviaKind,
    pub text: GreenText,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TokenFlags(u8);

impl TokenFlags {
    const MISSING: u8 = 1 << 0;
    const BAD: u8 = 1 << 1;

    pub fn is_missing(self) -> bool {
        self.0 & Self::MISSING != 0
    }

    pub fn is_bad(self) -> bool {
        self.0 & Self::BAD != 0
    }
}

#[derive(Debug)]
pub struct GreenNodeData<L: SyntaxLanguage> {
    kind: L::Kind,
    children: Arc<[GreenElement<L>]>,
    annotations: Arc<[SyntaxAnnotation]>,
    width: TextSize,
}
#[derive(Debug)]
pub struct GreenTokenData<L: SyntaxLanguage> {
    kind: L::Kind,
    leading: Arc<[GreenTrivia]>,
    text: GreenText,
    trailing: Arc<[GreenTrivia]>,
    annotations: Arc<[L::DiagnosticCode]>,
    syntax_annotations: Arc<[SyntaxAnnotation]>,
    flags: TokenFlags,
    width: TextSize,
}
pub struct GreenFactory<L: SyntaxLanguage>(PhantomData<L>);

#[derive(Debug)]
pub enum GreenError {
    Text(TextError),
    EmptyBadToken,
}
impl From<TextError> for GreenError {
    fn from(value: TextError) -> Self {
        Self::Text(value)
    }
}
impl fmt::Display for GreenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(error) => error.fmt(f),
            Self::EmptyBadToken => f.write_str("bad token spelling must not be empty"),
        }
    }
}
impl std::error::Error for GreenError {}

impl GreenText {
    pub fn write_to_string(&self) -> String {
        let mut output = String::new();
        self.write_to(&mut output)
            .expect("String writes cannot fail");
        output
    }
    fn validate(&self) -> Result<(), TextError> {
        if let Self::SourceSlice { source, range } = self {
            source.slice(*range)?;
        }
        Ok(())
    }

    fn width(&self) -> Result<TextSize, TextError> {
        match self {
            Self::Static(text) => TextSize::try_from_usize(text.len()),
            Self::SourceSlice { range, .. } => Ok(range.len()),
            Self::Owned(text) => TextSize::try_from_usize(text.len()),
        }
    }
    fn is_source_independent(&self) -> bool {
        !matches!(self, Self::SourceSlice { .. })
    }
    fn write_to<W: fmt::Write>(&self, writer: &mut W) -> fmt::Result {
        match self {
            Self::Static(text) => writer.write_str(text),
            Self::SourceSlice { source, range } => {
                writer.write_str(source.slice(*range).map_err(|_| fmt::Error)?)
            }
            Self::Owned(text) => writer.write_str(text),
        }
    }
}
impl GreenTrivia {
    fn validate(&self) -> Result<(), TextError> {
        self.text.validate()
    }

    fn width(&self) -> Result<TextSize, TextError> {
        self.text.width()
    }
    fn is_source_independent(&self) -> bool {
        self.text.is_source_independent()
    }
}
impl<L: SyntaxLanguage> GreenElement<L> {
    fn width(&self) -> TextSize {
        match self {
            Self::Node(node) => node.width,
            Self::Token(token) => token.width,
        }
    }
    fn is_source_independent(&self) -> bool {
        match self {
            Self::Node(node) => node.is_source_independent(),
            Self::Token(token) => token.is_source_independent(),
        }
    }
    fn write_to<W: fmt::Write>(&self, writer: &mut W) -> fmt::Result {
        match self {
            Self::Node(node) => node.write_to(writer),
            Self::Token(token) => token.write_to(writer),
        }
    }
}
impl<L: SyntaxLanguage> GreenNodeData<L> {
    pub fn kind(&self) -> L::Kind {
        self.kind
    }
    pub fn children(&self) -> &[GreenElement<L>] {
        &self.children
    }
    pub fn width(&self) -> TextSize {
        self.width
    }
    pub fn annotations(&self) -> &[SyntaxAnnotation] {
        &self.annotations
    }
    pub fn is_source_independent(&self) -> bool {
        self.children
            .iter()
            .all(GreenElement::is_source_independent)
    }
    fn write_to<W: fmt::Write>(&self, writer: &mut W) -> fmt::Result {
        for child in self.children.iter() {
            child.write_to(writer)?;
        }
        Ok(())
    }
}
impl<L: SyntaxLanguage> GreenTokenData<L> {
    pub fn kind(&self) -> L::Kind {
        self.kind
    }
    pub fn width(&self) -> TextSize {
        self.width
    }
    pub fn text(&self) -> &GreenText {
        &self.text
    }
    pub fn annotations(&self) -> &[L::DiagnosticCode] {
        &self.annotations
    }
    pub fn syntax_annotations(&self) -> &[SyntaxAnnotation] {
        &self.syntax_annotations
    }
    pub fn flags(&self) -> TokenFlags {
        self.flags
    }
    pub fn leading_trivia(&self) -> &[GreenTrivia] {
        &self.leading
    }
    pub fn trailing_trivia(&self) -> &[GreenTrivia] {
        &self.trailing
    }
    pub fn is_source_independent(&self) -> bool {
        self.text.is_source_independent()
            && self.leading.iter().all(GreenTrivia::is_source_independent)
            && self.trailing.iter().all(GreenTrivia::is_source_independent)
    }
    fn write_to<W: fmt::Write>(&self, writer: &mut W) -> fmt::Result {
        for trivia in self.leading.iter() {
            trivia.text.write_to(writer)?;
        }
        self.text.write_to(writer)?;
        for trivia in self.trailing.iter() {
            trivia.text.write_to(writer)?;
        }
        Ok(())
    }
}

impl<L: SyntaxLanguage> GreenFactory<L> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
    pub fn trivia(&self, kind: TriviaKind, text: GreenText) -> Result<GreenTrivia, GreenError> {
        text.validate()?;
        Ok(GreenTrivia { kind, text })
    }
    pub fn token<I, J>(
        &self,
        kind: L::Kind,
        text: GreenText,
        leading: I,
        trailing: J,
    ) -> Result<GreenToken<L>, GreenError>
    where
        I: IntoIterator<Item = GreenTrivia>,
        J: IntoIterator<Item = GreenTrivia>,
    {
        self.make_token(
            kind,
            text,
            leading.into_iter().collect(),
            trailing.into_iter().collect(),
            Arc::from([]),
            TokenFlags(0),
        )
    }
    pub fn missing_token(&self, kind: L::Kind) -> GreenToken<L> {
        self.make_token(
            kind,
            GreenText::Static(""),
            Vec::new(),
            Vec::new(),
            Arc::from([]),
            TokenFlags(TokenFlags::MISSING),
        )
        .expect("empty missing token has zero width")
    }
    pub fn missing_token_with_leading<I>(
        &self,
        kind: L::Kind,
        leading: I,
    ) -> Result<GreenToken<L>, GreenError>
    where
        I: IntoIterator<Item = GreenTrivia>,
    {
        self.make_token(
            kind,
            GreenText::Static(""),
            leading.into_iter().collect(),
            Vec::new(),
            Arc::from([]),
            TokenFlags(TokenFlags::MISSING),
        )
    }
    pub fn bad_token(
        &self,
        kind: L::Kind,
        text: GreenText,
        code: L::DiagnosticCode,
    ) -> Result<GreenToken<L>, GreenError> {
        text.validate()?;
        if text.width()?.to_usize() == 0 {
            return Err(GreenError::EmptyBadToken);
        }
        self.make_token(
            kind,
            text,
            Vec::new(),
            Vec::new(),
            Arc::from([code]),
            TokenFlags(TokenFlags::BAD),
        )
    }
    pub fn bad_token_with_leading<I>(
        &self,
        kind: L::Kind,
        text: GreenText,
        leading: I,
        code: L::DiagnosticCode,
    ) -> Result<GreenToken<L>, GreenError>
    where
        I: IntoIterator<Item = GreenTrivia>,
    {
        text.validate()?;
        if text.width()?.to_usize() == 0 {
            return Err(GreenError::EmptyBadToken);
        }
        self.make_token(
            kind,
            text,
            leading.into_iter().collect(),
            Vec::new(),
            Arc::from([code]),
            TokenFlags(TokenFlags::BAD),
        )
    }
    pub fn node<I>(&self, kind: L::Kind, children: I) -> Result<GreenNode<L>, GreenError>
    where
        I: IntoIterator<Item = GreenElement<L>>,
    {
        let children: Arc<[GreenElement<L>]> = children.into_iter().collect();
        let width = children
            .iter()
            .try_fold(TextSize::try_from_usize(0)?, |sum, child| {
                sum.checked_add(child.width())
            })?;
        Ok(Arc::new(GreenNodeData {
            kind,
            children,
            annotations: Arc::from([]),
            width,
        }))
    }
    pub(crate) fn node_with_annotations<I>(
        &self,
        kind: L::Kind,
        children: I,
        annotations: Arc<[SyntaxAnnotation]>,
    ) -> Result<GreenNode<L>, GreenError>
    where
        I: IntoIterator<Item = GreenElement<L>>,
    {
        let node = self.node(kind, children)?;
        Ok(Arc::new(GreenNodeData {
            kind: node.kind,
            children: node.children.clone(),
            annotations,
            width: node.width,
        }))
    }
    fn make_token(
        &self,
        kind: L::Kind,
        text: GreenText,
        leading: Vec<GreenTrivia>,
        trailing: Vec<GreenTrivia>,
        annotations: Arc<[L::DiagnosticCode]>,
        flags: TokenFlags,
    ) -> Result<GreenToken<L>, GreenError> {
        text.validate()?;
        for trivia in leading.iter().chain(trailing.iter()) {
            trivia.validate()?;
        }
        let width = leading
            .iter()
            .chain(std::iter::once(&GreenTrivia {
                kind: TriviaKind::Whitespace,
                text: text.clone(),
            }))
            .chain(trailing.iter())
            .try_fold(TextSize::try_from_usize(0)?, |sum, piece| {
                sum.checked_add(piece.width()?)
            })?;
        Ok(Arc::new(GreenTokenData {
            kind,
            leading: leading.into(),
            text,
            trailing: trailing.into(),
            annotations,
            syntax_annotations: Arc::from([]),
            flags,
            width,
        }))
    }
    pub(crate) fn token_with_syntax_annotations(
        &self,
        token: &GreenToken<L>,
        annotations: Arc<[SyntaxAnnotation]>,
    ) -> GreenToken<L> {
        Arc::new(GreenTokenData {
            kind: token.kind,
            leading: token.leading.clone(),
            text: token.text.clone(),
            trailing: token.trailing.clone(),
            annotations: token.annotations.clone(),
            syntax_annotations: annotations,
            flags: token.flags,
            width: token.width,
        })
    }
}
impl<L: SyntaxLanguage> Default for GreenFactory<L> {
    fn default() -> Self {
        Self::new()
    }
}
pub fn write_green_to<L: SyntaxLanguage, W: fmt::Write>(
    node: &GreenNode<L>,
    writer: &mut W,
) -> fmt::Result {
    node.write_to(writer)
}
