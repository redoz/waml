//! Immutable presentation model.
//!
//! A `PresentationPlan` partitions every source byte into exactly one text run.
//! Block decorations and embedded blocks describe the same source without
//! owning any of its bytes, so they may overlap their owner's range freely.

pub mod assets;
pub mod compile;
pub mod draw;
pub mod highlight;
pub mod layout;
pub mod style;

pub use assets::{
    ApprovedImageSource, AssetEventOutcome, AssetRequestId, EmbeddedAssetFrame, EmbeddedAssets,
    EmbeddedState, ImageAssetEvent, ImageAssetRequest, ImageMediaType, MarkdownAssetHost,
};
pub use compile::{compile_presentation, render_plan_golden};
pub use draw::{
    build_draw_commands, DecorationRole, DrawCommand, InstalledPresentation, PresentationFrame,
    PresentedDiagnostic, PresentedDiagnosticSeverity, ResolvedTextStyle,
};
pub use highlight::{
    CodeHighlightError, CodeHighlightHost, CodeHighlightRequest, CodeHighlightResult,
    CodeHighlightSpan, CodeTokenRole, HighlightOutcome, HighlighterRegistry,
};
pub use layout::{build_layout_document, EmbeddedMeasurements};
pub use style::PresentationStyles;

use std::{collections::BTreeSet, fmt, sync::Arc};

use waml_syntax::{
    DocumentRevision, SyntaxIdentity, TableAlignment, TextError, TextRange, TextSize,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PresentationItemId {
    pub owner: SyntaxIdentity,
    pub role: PresentationRole,
    pub fragment_ordinal: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationRole {
    Text(TextRole),
    Block(BlockDecorationRole),
    Embedded(EmbeddedBlockRole),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TextRole {
    Body,
    SyntaxMarker,
    Heading(u8),
    Emphasis,
    Strong,
    StrongEmphasis,
    Strikethrough,
    LinkLabel,
    LinkDestination,
    ListMarker,
    TaskMarker,
    QuoteMarker,
    InlineCode,
    CodeFence,
    CodeInfo,
    CodeContent,
    /// A highlighted token inside fenced-code content.
    CodeToken(CodeTokenRole),
    TableDelimiter,
    RawHtml,
    Frontmatter,
    Recovery,
    Whitespace,
    LineBreak,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BlockDecorationRole {
    QuoteRule,
    InlineCodeFill,
    FencedCodeSurface,
    TableGrid,
    TableHeaderFill,
    TaskCheckbox,
    ThematicRule,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EmbeddedBlockRole {
    Image,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockDecorationKind {
    QuoteRule,
    InlineCodeFill,
    FencedCodeSurface,
    TableGrid { columns: u32 },
    TableHeaderFill,
    TaskCheckbox { checked: bool },
    ThematicRule,
}

impl BlockDecorationKind {
    pub fn role(self) -> BlockDecorationRole {
        match self {
            Self::QuoteRule => BlockDecorationRole::QuoteRule,
            Self::InlineCodeFill => BlockDecorationRole::InlineCodeFill,
            Self::FencedCodeSurface => BlockDecorationRole::FencedCodeSurface,
            Self::TableGrid { .. } => BlockDecorationRole::TableGrid,
            Self::TableHeaderFill => BlockDecorationRole::TableHeaderFill,
            Self::TaskCheckbox { .. } => BlockDecorationRole::TaskCheckbox,
            Self::ThematicRule => BlockDecorationRole::ThematicRule,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmbeddedBlockKind {
    Image {
        destination: Arc<str>,
        alt: Arc<str>,
        title: Option<Arc<str>>,
    },
}

impl EmbeddedBlockKind {
    pub fn role(&self) -> EmbeddedBlockRole {
        match self {
            Self::Image { .. } => EmbeddedBlockRole::Image,
        }
    }
}

/// Style roles. Task 3 resolves them into concrete metrics and colors; the
/// model only names them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FontRole {
    Body,
    Heading,
    Monospace,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FontSizeRole {
    Body,
    Heading(u8),
    Code,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FontWeightRole {
    Regular,
    Medium,
    SemiBold,
    Bold,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ColorRole {
    Text,
    Muted,
    Marker,
    ActiveMarker,
    Link,
    Code,
    CodeSurface,
    Quote,
    Recovery,
    TableRule,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextStyle {
    pub font: FontRole,
    pub size: FontSizeRole,
    pub weight: FontWeightRole,
    pub italic: bool,
    pub color: ColorRole,
    pub active_color: ColorRole,
    pub background: Option<ColorRole>,
    pub underline: bool,
    pub strikethrough: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationItem {
    TextRun {
        id: PresentationItemId,
        range: TextRange,
        role: TextRole,
        style: TextStyle,
    },
    BlockDecoration {
        id: PresentationItemId,
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: BlockDecorationKind,
    },
    EmbeddedBlock {
        id: PresentationItemId,
        owner: SyntaxIdentity,
        source_range: TextRange,
        kind: EmbeddedBlockKind,
    },
}

impl PresentationItem {
    pub fn id(&self) -> PresentationItemId {
        match self {
            Self::TextRun { id, .. }
            | Self::BlockDecoration { id, .. }
            | Self::EmbeddedBlock { id, .. } => *id,
        }
    }

    pub fn owner(&self) -> SyntaxIdentity {
        match self {
            Self::TextRun { id, .. } => id.owner,
            Self::BlockDecoration { owner, .. } | Self::EmbeddedBlock { owner, .. } => *owner,
        }
    }

    pub fn source_range(&self) -> TextRange {
        match self {
            Self::TextRun { range, .. } => *range,
            Self::BlockDecoration { source_range, .. }
            | Self::EmbeddedBlock { source_range, .. } => *source_range,
        }
    }
}

/// A parsed link the application can navigate to. Drawing never resolves it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedLink {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub destination: Arc<str>,
    pub title: Option<Arc<str>>,
}

/// A parsed block, taken from the syntax tree rather than from the text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationBlock {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    /// Index of the parent block in `PresentationPlan::blocks`.
    pub parent: Option<usize>,
    pub kind: PresentationBlockKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationBlockKind {
    Paragraph,
    Heading(u8),
    ListItem {
        marker_range: TextRange,
    },
    Quote,
    Code,
    Table {
        columns: u32,
    },
    TableRow,
    TableCell {
        column: u32,
        alignment: TableAlignment,
    },
    Image,
}

/// A non-fatal problem kept next to the block it affects. Presentation stays
/// editable; nothing is hidden or rolled back.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationDiagnostic {
    pub owner: SyntaxIdentity,
    pub source_range: TextRange,
    pub message: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct PresentationPlan {
    pub revision: DocumentRevision,
    pub source_len: TextSize,
    pub items: Arc<[PresentationItem]>,
    pub links: Arc<[PresentedLink]>,
    /// Parsed block structure, outermost first, in source order.
    pub blocks: Arc<[PresentationBlock]>,
    /// Non-fatal per-block problems, such as a failed code highlighter.
    pub diagnostics: Arc<[PresentationDiagnostic]>,
}

impl PresentationPlan {
    /// Checks that the text runs cover `0..source_len` exactly once, in order,
    /// and that every item identity is unique. Reports the first bad boundary.
    pub fn validate_source_partition(&self) -> Result<(), PresentationError> {
        let mut expected = TextSize::try_from_usize(0)?;
        let mut ids = BTreeSet::new();
        for item in self.items.iter() {
            if !ids.insert(item.id()) {
                return Err(PresentationError::DuplicateId(item.id()));
            }
            let PresentationItem::TextRun { range, role, .. } = item else {
                continue;
            };
            if let TextRole::Heading(level) = role {
                if !(1..=6).contains(level) {
                    return Err(PresentationError::UnsupportedHeadingLevel(*level));
                }
            }
            if range.end() > self.source_len {
                return Err(PresentationError::OutOfBounds {
                    range: *range,
                    len: self.source_len,
                });
            }
            if range.start() < expected {
                return Err(PresentationError::Overlap {
                    previous_end: expected,
                    next: *range,
                });
            }
            if range.start() > expected {
                return Err(PresentationError::Gap {
                    expected,
                    actual: range.start(),
                });
            }
            expected = range.end();
        }
        if expected != self.source_len {
            return Err(PresentationError::Gap {
                expected,
                actual: self.source_len,
            });
        }
        Ok(())
    }

    /// Owners whose source range touches `caret`, in first-appearance order.
    /// A caret on a boundary belongs to both neighbours.
    pub fn active_owners(&self, caret: TextSize) -> Arc<[SyntaxIdentity]> {
        let mut owners = Vec::new();
        for item in self.items.iter() {
            let range = item.source_range();
            if range.start() > caret || caret > range.end() {
                continue;
            }
            let owner = item.owner();
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        owners.into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationError {
    Gap {
        expected: TextSize,
        actual: TextSize,
    },
    Overlap {
        previous_end: TextSize,
        next: TextRange,
    },
    OutOfBounds {
        range: TextRange,
        len: TextSize,
    },
    DuplicateId(PresentationItemId),
    UnsupportedHeadingLevel(u8),
    RevisionMismatch {
        expected: DocumentRevision,
        actual: DocumentRevision,
        component: &'static str,
    },
    Text(TextError),
}

impl From<TextError> for PresentationError {
    fn from(error: TextError) -> Self {
        Self::Text(error)
    }
}

impl fmt::Display for PresentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gap { expected, actual } => write!(
                f,
                "presentation gap: expected a run at {} but the next boundary is {}",
                expected.to_usize(),
                actual.to_usize()
            ),
            Self::Overlap { previous_end, next } => write!(
                f,
                "presentation overlap: {}..{} starts before {}",
                next.start().to_usize(),
                next.end().to_usize(),
                previous_end.to_usize()
            ),
            Self::OutOfBounds { range, len } => write!(
                f,
                "presentation range {}..{} exceeds source length {}",
                range.start().to_usize(),
                range.end().to_usize(),
                len.to_usize()
            ),
            Self::DuplicateId(id) => write!(
                f,
                "duplicate presentation item identity for owner {}",
                id.owner.get()
            ),
            Self::UnsupportedHeadingLevel(level) => {
                write!(f, "unsupported heading level {level}")
            }
            Self::RevisionMismatch {
                expected,
                actual,
                component,
            } => write!(
                f,
                "{component} carries revision {} but the plan is revision {}",
                actual.get(),
                expected.get()
            ),
            Self::Text(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PresentationError {}
