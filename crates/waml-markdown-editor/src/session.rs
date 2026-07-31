use std::sync::Arc;

use crate::{
    document::MarkdownDocumentSnapshot,
    edit::{MarkdownEdit, MarkdownEditError, ProposedMarkdownEdit},
    selection::{SelectionError, SelectionSet},
};
use waml_syntax::{reparse_markdown, ChangeMap, DocumentRevision, SourceText, TextError, TextSize};

pub struct MarkdownDocumentSession {
    snapshot: Arc<MarkdownDocumentSnapshot>,
    selections: SelectionSet,
    read_only: bool,
}

impl MarkdownDocumentSession {
    pub fn new(snapshot: Arc<MarkdownDocumentSnapshot>) -> Self {
        let selections = SelectionSet::caret(snapshot.as_ref(), TextSize::new(0))
            .expect("a document always has a valid zero offset");
        Self {
            snapshot,
            selections,
            read_only: false,
        }
    }

    pub fn snapshot(&self) -> &Arc<MarkdownDocumentSnapshot> {
        &self.snapshot
    }

    pub fn selections(&self) -> &SelectionSet {
        &self.selections
    }

    pub fn local_revision(&self) -> DocumentRevision {
        self.snapshot.revision()
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn apply_edit(
        &mut self,
        edit: MarkdownEdit,
    ) -> Result<ProposedMarkdownEdit, MarkdownEditError> {
        self.apply_edit_without_history(edit)
    }

    fn apply_edit_without_history(
        &mut self,
        edit: MarkdownEdit,
    ) -> Result<ProposedMarkdownEdit, MarkdownEditError> {
        let current = self.snapshot.revision();
        if edit.base_revision != current {
            return Err(MarkdownEditError::StaleRevision {
                base: edit.base_revision,
                current,
            });
        }

        let next_revision = current
            .checked_next()
            .ok_or(MarkdownEditError::RevisionOverflow { current })?;
        if edit.selection_after.revision() != next_revision {
            return Err(MarkdownEditError::SelectionRevision {
                selection: edit.selection_after.revision(),
                expected: next_revision,
            });
        }

        let old_text = self.snapshot.text();
        for change in &edit.changes {
            old_text.slice(change.old_range).map_err(map_text_error)?;
        }
        ChangeMap::checked(old_text, &edit.changes).map_err(MarkdownEditError::InvalidChanges)?;

        let mut new_string = old_text.shared().as_str().to_owned();
        for change in edit.changes.iter().rev() {
            new_string.replace_range(
                change.old_range.start().to_usize()..change.old_range.end().to_usize(),
                &change.replacement,
            );
        }
        let new_text = SourceText::new(new_string)?;
        edit.selection_after
            .validate_for_text(&new_text)
            .map_err(map_selection_error)?;

        let syntax_update = reparse_markdown(
            self.snapshot.syntax().as_ref(),
            next_revision,
            new_text,
            &edit.changes,
        )?;
        let snapshot = Arc::new(MarkdownDocumentSnapshot::new(
            syntax_update.snapshot.clone(),
        ));

        self.snapshot = snapshot.clone();
        self.selections = edit.selection_after.clone();
        Ok(ProposedMarkdownEdit {
            edit,
            snapshot,
            syntax_update,
        })
    }
}

fn map_text_error(error: TextError) -> MarkdownEditError {
    match error {
        TextError::NonUtf8Boundary { offset } => MarkdownEditError::InvalidBoundary { offset },
        error => MarkdownEditError::Text(error),
    }
}

fn map_selection_error(error: SelectionError) -> MarkdownEditError {
    match error {
        SelectionError::InvalidBoundary { offset } => MarkdownEditError::InvalidBoundary { offset },
        SelectionError::Text(error) => MarkdownEditError::Text(error),
        SelectionError::EmptySet | SelectionError::PrimaryOutOfBounds { .. } => {
            unreachable!("a constructed selection set remains structurally valid")
        }
    }
}
