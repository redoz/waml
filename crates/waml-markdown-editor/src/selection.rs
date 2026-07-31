use crate::document::MarkdownDocumentSnapshot;
use waml_syntax::{DocumentRevision, SourceText, TextError, TextRange, TextSize};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Affinity {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextPosition {
    pub offset: TextSize,
    pub affinity: Affinity,
}

impl TextPosition {
    pub fn new(offset: TextSize, affinity: Affinity) -> Self {
        Self { offset, affinity }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Selection {
    pub anchor: TextPosition,
    pub cursor: TextPosition,
}

impl Selection {
    pub fn new(anchor: TextPosition, cursor: TextPosition) -> Self {
        Self { anchor, cursor }
    }

    pub fn caret(position: TextPosition) -> Self {
        Self::new(position, position)
    }

    pub fn is_empty(self) -> bool {
        self.anchor.offset == self.cursor.offset
    }

    pub fn range(self) -> TextRange {
        TextRange::new(
            self.anchor.offset.min(self.cursor.offset),
            self.anchor.offset.max(self.cursor.offset),
        )
        .expect("ordered selection endpoints")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectionSet {
    revision: DocumentRevision,
    selections: Vec<Selection>,
    primary: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionError {
    EmptySet,
    PrimaryOutOfBounds { primary: usize, len: usize },
    InvalidBoundary { offset: TextSize },
    Text(TextError),
}

impl SelectionSet {
    pub fn caret(
        snapshot: &MarkdownDocumentSnapshot,
        offset: TextSize,
    ) -> Result<Self, SelectionError> {
        Self::caret_in_text(snapshot.revision(), snapshot.text(), offset)
    }

    pub fn single(
        snapshot: &MarkdownDocumentSnapshot,
        selection: Selection,
    ) -> Result<Self, SelectionError> {
        Self::from_selections(snapshot, vec![selection], 0)
    }

    pub fn from_selections(
        snapshot: &MarkdownDocumentSnapshot,
        selections: Vec<Selection>,
        primary: usize,
    ) -> Result<Self, SelectionError> {
        Self::from_source(snapshot.revision(), snapshot.text(), selections, primary)
    }

    pub fn caret_in_text(
        revision: DocumentRevision,
        text: &SourceText,
        offset: TextSize,
    ) -> Result<Self, SelectionError> {
        Self::from_source(
            revision,
            text,
            vec![Selection::caret(TextPosition::new(
                offset,
                Affinity::Before,
            ))],
            0,
        )
    }

    pub fn from_source(
        revision: DocumentRevision,
        text: &SourceText,
        selections: Vec<Selection>,
        primary: usize,
    ) -> Result<Self, SelectionError> {
        if selections.is_empty() {
            return Err(SelectionError::EmptySet);
        }
        if primary >= selections.len() {
            return Err(SelectionError::PrimaryOutOfBounds {
                primary,
                len: selections.len(),
            });
        }

        for selection in &selections {
            Self::validate_position(text, selection.anchor)?;
            Self::validate_position(text, selection.cursor)?;
        }

        let primary_cursor = selections[primary].cursor.offset;
        let mut selections = selections;
        selections.sort_unstable_by_key(|selection| selection.range().start());

        let mut normalized = Vec::with_capacity(selections.len());
        for selection in selections {
            if let Some(previous) = normalized.last_mut() {
                if Self::should_merge(*previous, selection) {
                    *previous = Self::merge(*previous, selection);
                    continue;
                }
            }
            normalized.push(selection);
        }

        let primary = normalized
            .iter()
            .position(|selection| Self::contains_cursor(*selection, primary_cursor))
            .expect("normalized selection retains the primary cursor");

        Ok(Self {
            revision,
            selections: normalized,
            primary,
        })
    }

    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }

    pub fn as_slice(&self) -> &[Selection] {
        &self.selections
    }

    pub fn primary(&self) -> Selection {
        self.selections[self.primary]
    }

    pub fn primary_index(&self) -> usize {
        self.primary
    }

    fn validate_position(text: &SourceText, position: TextPosition) -> Result<(), SelectionError> {
        let range =
            TextRange::new(position.offset, position.offset).expect("zero-width range is ordered");
        match text.slice(range) {
            Ok(_) => Ok(()),
            Err(TextError::NonUtf8Boundary { offset }) => {
                Err(SelectionError::InvalidBoundary { offset })
            }
            Err(error) => Err(SelectionError::Text(error)),
        }
    }

    fn should_merge(left: Selection, right: Selection) -> bool {
        if left.is_empty() || right.is_empty() {
            return left.is_empty()
                && right.is_empty()
                && left.anchor.offset == right.anchor.offset;
        }

        right.range().start() < left.range().end()
    }

    fn merge(left: Selection, right: Selection) -> Selection {
        let range = left.range().cover(right.range());
        Selection::new(
            TextPosition::new(range.start(), Affinity::Before),
            TextPosition::new(range.end(), Affinity::After),
        )
    }

    fn contains_cursor(selection: Selection, cursor: TextSize) -> bool {
        let range = selection.range();
        selection.is_empty() && cursor == range.start()
            || !selection.is_empty() && range.start() <= cursor && cursor <= range.end()
    }
}
