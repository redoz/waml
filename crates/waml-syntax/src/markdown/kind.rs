use std::{
    num::NonZeroU64,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use crate::{ParseError, SyntaxAnnotation, SyntaxLanguage};

static NEXT_MARKDOWN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SyntaxIdentity(NonZeroU64);

impl SyntaxIdentity {
    pub fn get(self) -> u64 {
        self.0.get()
    }

    #[doc(hidden)]
    pub fn from_raw_for_test(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("a test identity is non-zero"))
    }

    pub(crate) fn fresh() -> Result<Self, ParseError> {
        // `fetch_update` was renamed `try_update` in 1.95; the old name stays
        // until the workspace MSRV (1.80) catches up.
        #[allow(deprecated)]
        let value = NEXT_MARKDOWN_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| ParseError::StructuralInvariant {
                reason: "Markdown syntax identity space exhausted".into(),
            })?;
        Ok(Self(
            NonZeroU64::new(value).expect("identity starts at one"),
        ))
    }

    pub(crate) fn annotation(self) -> SyntaxAnnotation {
        SyntaxAnnotation::new(
            NonZeroU64::MAX,
            "waml.markdown.identity",
            Some(Arc::<str>::from(self.get().to_string())),
        )
    }

    pub(crate) fn metadata_annotation(
        self,
        kind: &'static str,
        data: &'static str,
    ) -> SyntaxAnnotation {
        SyntaxAnnotation::new(self.0, kind, Some(Arc::from(data)))
    }

    pub(crate) fn from_annotation_data(data: &str) -> Option<Self> {
        data.parse::<u64>().ok().and_then(NonZeroU64::new).map(Self)
    }
}

#[derive(Debug)]
pub struct OkfMarkdownLanguage;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OkfMarkdownSyntaxKind {
    Root,
    Frontmatter,
    FrontmatterEntry,
    FrontmatterMapping,
    FrontmatterSequence,
    FrontmatterSequenceItem,
    BlockQuote,
    List,
    ListItem,
    Paragraph,
    AtxHeading,
    SetextHeading,
    ThematicBreak,
    IndentedCodeBlock,
    FencedCodeBlock,
    HtmlBlock,
    LinkReferenceDefinition,
    Table,
    TableHead,
    TableBody,
    TableRow,
    TableCell,
    Text,
    Escape,
    Entity,
    CodeSpan,
    Emphasis,
    StrongEmphasis,
    Strikethrough,
    Link,
    Image,
    Autolink,
    RawHtml,
    SoftLineBreak,
    HardLineBreak,
    WamlSection,
    SkippedTokensSyntax,
    BomToken,
    FrontmatterFenceToken,
    FrontmatterKeyToken,
    ColonToken,
    FrontmatterValueToken,
    FrontmatterDashToken,
    FrontmatterQuotedValueToken,
    FrontmatterCommentToken,
    FrontmatterBlockScalarHeaderToken,
    BlockQuoteMarkerToken,
    ListMarkerToken,
    TaskListMarkerToken,
    HeadingMarkerToken,
    SetextUnderlineToken,
    ThematicBreakToken,
    IndentToken,
    CodeFenceToken,
    InfoStringToken,
    CodeTextToken,
    HtmlToken,
    LinkLabelOpenToken,
    LinkLabelCloseToken,
    LinkDestinationOpenToken,
    LinkDestinationToken,
    LinkDestinationCloseToken,
    LinkTitleToken,
    TablePipeToken,
    TableAlignmentColonToken,
    TextToken,
    BackslashToken,
    EntityToken,
    CodeDelimiterToken,
    EmphasisDelimiterToken,
    StrikethroughDelimiterToken,
    ImageBangToken,
    AutolinkOpenToken,
    AutolinkCloseToken,
    WhitespaceToken,
    NewlineToken,
    EndOfFileToken,
    BadToken,
    #[doc(hidden)]
    FrontmatterOpenFence,
    #[doc(hidden)]
    FrontmatterKey,
    #[doc(hidden)]
    FrontmatterValue,
    #[doc(hidden)]
    FrontmatterCloseFence,
    #[doc(hidden)]
    Heading,
    #[doc(hidden)]
    HeadingText,
    #[doc(hidden)]
    MarkdownRegion,
    #[doc(hidden)]
    RawTextToken,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OkfSyntaxDiagnosticCode {
    FrontmatterNotClean,
    MissingFrontmatterFence,
    MalformedFrontmatterEntry,
    DuplicateFrontmatterKey,
    TabInFrontmatterIndent,
    InvalidFrontmatterIndent,
    UnterminatedQuotedScalar,
    InvalidUtf8Boundary,
    ParserStalled,
    MalformedBlock,
    MalformedInline,
    UnclosedFence,
    UnclosedLink,
    MalformedTable,
    FilteredHtmlTag,
    NestingDepthExceeded,
}

impl SyntaxLanguage for OkfMarkdownLanguage {
    type Kind = OkfMarkdownSyntaxKind;
    type DiagnosticCode = OkfSyntaxDiagnosticCode;
}
