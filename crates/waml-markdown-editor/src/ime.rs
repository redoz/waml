use crate::selection::SelectionSet;
use std::{ops::Range, sync::Arc};
use waml_syntax::{DocumentRevision, TextRange};

#[derive(Clone, Debug)]
pub struct ImeComposition {
    base_revision: DocumentRevision,
    replace_range: TextRange,
    committed_snapshot: Arc<crate::document::MarkdownDocumentSnapshot>,
    committed_selection: SelectionSet,
    preedit: String,
    utf16_selection: Range<u32>,
}

impl ImeComposition {
    pub(crate) fn new(
        committed_snapshot: Arc<crate::document::MarkdownDocumentSnapshot>,
        committed_selection: SelectionSet,
    ) -> Self {
        Self {
            base_revision: committed_snapshot.revision(),
            replace_range: committed_selection.primary().range(),
            committed_snapshot,
            committed_selection,
            preedit: String::new(),
            utf16_selection: 0..0,
        }
    }

    pub fn base_revision(&self) -> DocumentRevision {
        self.base_revision
    }

    pub fn replace_range(&self) -> TextRange {
        self.replace_range
    }

    pub fn committed_snapshot(&self) -> &Arc<crate::document::MarkdownDocumentSnapshot> {
        &self.committed_snapshot
    }

    pub fn committed_selection(&self) -> &SelectionSet {
        &self.committed_selection
    }

    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    pub fn utf16_selection(&self) -> Range<u32> {
        self.utf16_selection.clone()
    }

    pub(crate) fn update(
        &mut self,
        preedit: &str,
        utf16_selection: Range<u32>,
    ) -> Result<(), ImeError> {
        let units = preedit.encode_utf16().count() as u32;
        if utf16_selection.start > utf16_selection.end
            || utf16_selection.end > units
            || !is_utf16_boundary(preedit, utf16_selection.start)
            || !is_utf16_boundary(preedit, utf16_selection.end)
        {
            return Err(ImeError::InvalidUtf16Selection {
                start: utf16_selection.start,
                end: utf16_selection.end,
                preedit_units: units,
            });
        }
        self.preedit.clear();
        self.preedit.push_str(preedit);
        self.utf16_selection = utf16_selection;
        Ok(())
    }
}

fn is_utf16_boundary(text: &str, target: u32) -> bool {
    let mut units = 0_u32;
    if target == 0 {
        return true;
    }
    for character in text.chars() {
        units += character.len_utf16() as u32;
        if units == target {
            return true;
        }
        if units > target {
            return false;
        }
    }
    units == target
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImeError {
    AlreadyActive,
    NotActive,
    StaleRevision {
        base: DocumentRevision,
        current: DocumentRevision,
    },
    InvalidUtf16Selection {
        start: u32,
        end: u32,
        preedit_units: u32,
    },
    ReadOnly,
}
