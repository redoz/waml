use std::sync::Arc;

use crate::{document::MarkdownDocumentSnapshot, edit::HistoryGroup, selection::SelectionSet};
use waml_syntax::TextChange;

#[derive(Clone, Debug)]
pub(crate) struct HistoryEntry {
    pub before: Arc<MarkdownDocumentSnapshot>,
    pub before_selection: SelectionSet,
    pub after: Arc<MarkdownDocumentSnapshot>,
    pub after_selection: SelectionSet,
    pub forward_changes: Vec<TextChange>,
    pub inverse_changes: Vec<TextChange>,
    pub group: HistoryGroup,
}

#[derive(Default)]
pub(crate) struct History {
    pub undo: Vec<Vec<HistoryEntry>>,
    pub redo: Vec<Vec<HistoryEntry>>,
    break_group: bool,
}

impl History {
    pub fn break_group(&mut self) {
        self.break_group = true;
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        if !self.break_group {
            if let Some(group) = self.undo.last_mut() {
                if group
                    .last()
                    .is_some_and(|last| last.group.can_merge(entry.group))
                {
                    group.push(entry);
                    self.redo.clear();
                    return;
                }
            }
        }
        self.undo.push(vec![entry]);
        self.break_group = false;
        self.redo.clear();
    }
}
