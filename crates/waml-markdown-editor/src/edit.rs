use std::{error::Error, fmt, sync::Arc};

use crate::{document::MarkdownDocumentSnapshot, ime::ImeError, selection::SelectionSet};
use waml_syntax::{
    DocumentRevision, FullReparseReason, MarkdownSyntaxUpdate, ParseError, TextChange, TextError,
    TextSize,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HistoryGroup(u64);

impl HistoryGroup {
    pub const fn isolated() -> Self {
        Self(0)
    }

    pub const fn named(id: u64) -> Self {
        Self(id.saturating_add(1))
    }

    pub const fn can_merge(self, next: Self) -> bool {
        self.0 != 0 && self.0 == next.0
    }
}

#[derive(Clone, Debug)]
pub struct MarkdownEdit {
    pub base_revision: DocumentRevision,
    pub changes: Vec<TextChange>,
    pub selection_after: SelectionSet,
    pub history_group: HistoryGroup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EditCommand {
    Insert(Arc<str>),
    DeleteBackward,
    DeleteForward,
    ReplaceSelections(Arc<str>),
    Paste(Arc<str>),
    Cut,
    Indent { spaces: usize },
    Outdent { spaces: usize },
}

#[derive(Clone, Debug)]
pub struct EditOutcome {
    pub proposal: Option<ProposedMarkdownEdit>,
    pub clipboard: Option<String>,
}

#[derive(Clone)]
pub struct ProposedMarkdownEdit {
    pub edit: MarkdownEdit,
    pub snapshot: Arc<MarkdownDocumentSnapshot>,
    pub syntax_update: MarkdownSyntaxUpdate,
}

impl fmt::Debug for ProposedMarkdownEdit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProposedMarkdownEdit")
            .field("edit", &self.edit)
            .field("snapshot_revision", &self.snapshot.revision())
            .field("affected_ranges", &self.syntax_update.affected_ranges)
            .finish()
    }
}

#[derive(Debug)]
pub enum MarkdownEditError {
    StaleRevision {
        base: DocumentRevision,
        current: DocumentRevision,
    },
    SelectionRevision {
        selection: DocumentRevision,
        expected: DocumentRevision,
    },
    InvalidBoundary {
        offset: TextSize,
    },
    InvalidChanges(FullReparseReason),
    Text(TextError),
    Parse(ParseError),
    RevisionOverflow {
        current: DocumentRevision,
    },
    Ime(ImeError),
}

impl fmt::Display for MarkdownEditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleRevision { base, current } => {
                write!(
                    f,
                    "stale edit revision {} (current {})",
                    base.get(),
                    current.get()
                )
            }
            Self::SelectionRevision {
                selection,
                expected,
            } => write!(
                f,
                "selection revision {} does not match expected {}",
                selection.get(),
                expected.get()
            ),
            Self::InvalidBoundary { offset } => {
                write!(f, "offset {} is not a UTF-8 boundary", offset.to_usize())
            }
            Self::InvalidChanges(reason) => write!(f, "invalid edit changes: {reason:?}"),
            Self::Text(error) => error.fmt(f),
            Self::Parse(error) => error.fmt(f),
            Self::RevisionOverflow { current } => {
                write!(f, "revision {} cannot advance", current.get())
            }
            Self::Ime(error) => write!(f, "IME error: {error:?}"),
        }
    }
}

impl Error for MarkdownEditError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Text(error) => Some(error),
            Self::Parse(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TextError> for MarkdownEditError {
    fn from(error: TextError) -> Self {
        Self::Text(error)
    }
}

impl From<ParseError> for MarkdownEditError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<ImeError> for MarkdownEditError {
    fn from(error: ImeError) -> Self {
        Self::Ime(error)
    }
}
