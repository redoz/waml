use std::sync::Arc;

use crate::{
    document::MarkdownDocumentSnapshot,
    edit::{
        EditCommand, EditOutcome, HistoryGroup, MarkdownEdit, MarkdownEditError,
        ProposedMarkdownEdit,
    },
    history::{History, HistoryEntry},
    ime::{ImeComposition, ImeError},
    input::ScrollState,
    layout::{LayoutError, LayoutSnapshot},
    selection::{
        translate_position_with_map, Affinity, Selection, SelectionError, SelectionSet,
        TextPosition,
    },
    unicode::{logical_lines, next_grapheme_boundary, previous_grapheme_boundary, word_range_at},
};
use waml_syntax::{
    reparse_markdown, ChangeMap, DocumentRevision, LineIndex, SourceText, TextChange, TextError,
    TextRange, TextSize,
};

pub struct MarkdownDocumentSession {
    snapshot: Arc<MarkdownDocumentSnapshot>,
    selections: SelectionSet,
    read_only: bool,
    history: History,
    ime: Option<ImeComposition>,
    preferred_x: Option<f64>,
    scroll: ScrollState,
}

impl MarkdownDocumentSession {
    pub fn new(snapshot: Arc<MarkdownDocumentSnapshot>) -> Self {
        let selections = SelectionSet::caret(snapshot.as_ref(), TextSize::new(0))
            .expect("a document always has a valid zero offset");
        Self {
            snapshot,
            selections,
            read_only: false,
            history: History::default(),
            ime: None,
            preferred_x: None,
            scroll: ScrollState::default(),
        }
    }

    pub fn with_selections(
        snapshot: Arc<MarkdownDocumentSnapshot>,
        selections: SelectionSet,
    ) -> Result<Self, MarkdownEditError> {
        if selections.revision() != snapshot.revision() {
            return Err(MarkdownEditError::SelectionRevision {
                selection: selections.revision(),
                expected: snapshot.revision(),
            });
        }
        selections
            .validate_for_text(snapshot.text())
            .map_err(map_selection_error)?;
        Ok(Self {
            snapshot,
            selections,
            read_only: false,
            history: History::default(),
            ime: None,
            preferred_x: None,
            scroll: ScrollState::default(),
        })
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
    pub fn set_read_only(&mut self, read_only: bool) {
        if read_only {
            self.cancel_ime();
        }
        self.read_only = read_only;
    }
    pub fn scroll(&self) -> ScrollState {
        self.scroll
    }
    pub(crate) fn set_scroll(&mut self, scroll: ScrollState) {
        self.scroll = scroll;
    }
    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }
    pub fn break_history_group(&mut self) {
        self.history.break_group();
    }

    pub fn ime(&self) -> Option<&ImeComposition> {
        self.ime.as_ref()
    }

    pub fn begin_ime(&mut self) -> Result<(), ImeError> {
        if self.read_only {
            return Err(ImeError::ReadOnly);
        }
        if self.ime.is_some() {
            return Err(ImeError::AlreadyActive);
        }
        self.ime = Some(ImeComposition::new(
            self.snapshot.clone(),
            self.selections.clone(),
        ));
        Ok(())
    }

    pub fn update_ime(
        &mut self,
        preedit: &str,
        utf16_selection: std::ops::Range<u32>,
    ) -> Result<(), ImeError> {
        let current = self.snapshot.revision();
        let composition = self.ime.as_mut().ok_or(ImeError::NotActive)?;
        if composition.base_revision() != current {
            return Err(ImeError::StaleRevision {
                base: composition.base_revision(),
                current,
            });
        }
        composition.update(preedit, utf16_selection)
    }

    pub fn commit_ime(
        &mut self,
        group: HistoryGroup,
    ) -> Result<Option<ProposedMarkdownEdit>, MarkdownEditError> {
        if self.read_only {
            return Err(ImeError::ReadOnly.into());
        }
        let current = self.snapshot.revision();
        let composition = self.ime.as_ref().ok_or(ImeError::NotActive)?;
        if composition.base_revision() != current {
            return Err(ImeError::StaleRevision {
                base: composition.base_revision(),
                current,
            }
            .into());
        }
        if composition.preedit().is_empty() {
            if let Some(composition) = self.ime.take() {
                self.restore_composition(composition);
            }
            return Ok(None);
        }

        let next = current
            .checked_next()
            .ok_or(MarkdownEditError::RevisionOverflow { current })?;
        let replacement: Arc<str> = Arc::from(composition.preedit());
        let change = TextChange {
            old_range: composition.replace_range(),
            replacement: replacement.clone(),
        };
        let after_text = apply_changes(self.snapshot.text(), std::slice::from_ref(&change))?;
        let caret = TextSize::try_from_usize(
            composition.replace_range().start().to_usize() + replacement.len(),
        )?;
        let selection_after =
            SelectionSet::caret_in_text(next, &after_text, caret).map_err(map_selection_error)?;
        let proposal = self.apply_with_history(MarkdownEdit {
            base_revision: current,
            changes: vec![change],
            selection_after,
            history_group: group,
        })?;
        self.ime = None;
        Ok(Some(proposal))
    }

    pub fn cancel_ime(&mut self) {
        if let Some(composition) = self.ime.take() {
            self.restore_composition(composition);
        }
    }

    fn restore_composition(&mut self, composition: ImeComposition) {
        self.snapshot = composition.committed_snapshot().clone();
        self.selections = composition.committed_selection().clone();
    }

    pub fn set_primary_offset(&mut self, offset: TextSize) -> Result<(), MarkdownEditError> {
        self.preferred_x = None;
        self.replace_primary_selection(Selection::caret(TextPosition::new(
            offset,
            Affinity::Before,
        )))
    }

    pub fn move_left(&mut self, extend: bool) -> Result<(), MarkdownEditError> {
        self.preferred_x = None;
        let selection = self.selections.primary();
        let offset = if !extend && !selection.is_empty() {
            selection.range().start().to_usize()
        } else {
            previous_grapheme_boundary(
                self.snapshot.text().shared().as_str(),
                selection.cursor.offset.to_usize(),
            )
        };
        self.move_primary_to(offset, extend)
    }

    pub fn move_right(&mut self, extend: bool) -> Result<(), MarkdownEditError> {
        self.preferred_x = None;
        let selection = self.selections.primary();
        let offset = if !extend && !selection.is_empty() {
            selection.range().end().to_usize()
        } else {
            next_grapheme_boundary(
                self.snapshot.text().shared().as_str(),
                selection.cursor.offset.to_usize(),
            )
        };
        self.move_primary_to(offset, extend)
    }

    pub fn select_word_at(&mut self, offset: TextSize) -> Result<Selection, MarkdownEditError> {
        self.preferred_x = None;
        let text = self.snapshot.text().shared().as_str();
        let offset = offset.to_usize();
        let (start, end) = word_range_at(text, offset).unwrap_or_else(|| {
            (
                previous_grapheme_boundary(text, offset),
                next_grapheme_boundary(text, offset),
            )
        });
        let selection = Selection::new(
            TextPosition::new(TextSize::try_from_usize(start).unwrap(), Affinity::Before),
            TextPosition::new(TextSize::try_from_usize(end).unwrap(), Affinity::After),
        );
        self.replace_primary_selection(selection)?;
        Ok(selection)
    }

    pub fn select_line_at(&mut self, offset: TextSize) -> Result<Selection, MarkdownEditError> {
        self.preferred_x = None;
        let text = self.snapshot.text().shared().as_str();
        let line = self
            .snapshot
            .line_index()
            .line_col(self.snapshot.text(), offset)?
            .line;
        let (start, content_end) = logical_lines(text)
            .get(line as usize)
            .copied()
            .ok_or(MarkdownEditError::InvalidBoundary { offset })?;
        let end = if content_end < text.len() {
            if text[content_end..].starts_with("\r\n") {
                content_end + 2
            } else {
                content_end + 1
            }
        } else {
            content_end
        };
        let selection = Selection::new(
            TextPosition::new(TextSize::try_from_usize(start).unwrap(), Affinity::Before),
            TextPosition::new(TextSize::try_from_usize(end).unwrap(), Affinity::After),
        );
        self.replace_primary_selection(selection)?;
        Ok(selection)
    }

    pub fn select_all(&mut self) -> Result<(), MarkdownEditError> {
        self.preferred_x = None;
        let end = TextSize::try_from_usize(self.snapshot.text().shared().as_str().len()).unwrap();
        self.selections = SelectionSet::from_selections(
            self.snapshot.as_ref(),
            vec![Selection::new(
                TextPosition::new(TextSize::new(0), Affinity::Before),
                TextPosition::new(end, Affinity::After),
            )],
            0,
        )
        .map_err(map_selection_error)?;
        Ok(())
    }

    pub fn move_vertical(
        &mut self,
        layout: &LayoutSnapshot,
        lines: i32,
        extend: bool,
    ) -> Result<(), LayoutError> {
        let document = self.snapshot.revision();
        if layout.revision() != document {
            return Err(LayoutError::RevisionMismatch {
                document,
                layout: layout.revision(),
            });
        }
        let primary = self.selections.primary();
        let (position, preferred_x) = layout
            .move_vertical(primary.cursor, self.preferred_x, lines)
            .ok_or(LayoutError::GeometryUnavailable)?;
        let selection = if extend {
            Selection::new(primary.anchor, position)
        } else {
            Selection::caret(position)
        };
        self.replace_primary_selection(selection)
            .map_err(|_| LayoutError::InvalidSelection)?;
        self.preferred_x = Some(preferred_x);
        Ok(())
    }

    fn move_primary_to(&mut self, offset: usize, extend: bool) -> Result<(), MarkdownEditError> {
        let position =
            TextPosition::new(TextSize::try_from_usize(offset).unwrap(), Affinity::Before);
        let primary = self.selections.primary();
        let selection = if extend {
            Selection::new(primary.anchor, position)
        } else {
            Selection::caret(position)
        };
        self.replace_primary_selection(selection)
    }

    pub(crate) fn replace_primary_selection(
        &mut self,
        selection: Selection,
    ) -> Result<(), MarkdownEditError> {
        let primary = self.selections.primary_index();
        let mut selections = self.selections.as_slice().to_vec();
        selections[primary] = selection;
        self.selections =
            SelectionSet::from_selections(self.snapshot.as_ref(), selections, primary)
                .map_err(map_selection_error)?;
        Ok(())
    }

    pub(crate) fn add_selection(&mut self, selection: Selection) -> Result<(), MarkdownEditError> {
        let mut selections = self.selections.as_slice().to_vec();
        selections.push(selection);
        self.selections = SelectionSet::from_selections(
            self.snapshot.as_ref(),
            selections,
            self.selections.as_slice().len(),
        )
        .map_err(map_selection_error)?;
        Ok(())
    }

    pub fn execute(
        &mut self,
        command: EditCommand,
        group: HistoryGroup,
    ) -> Result<EditOutcome, MarkdownEditError> {
        self.cancel_ime();
        let skipped_primary = self.closing_delimiter_target(&command);
        let clipboard = if matches!(command, EditCommand::Cut) {
            Some(
                self.selections
                    .as_slice()
                    .iter()
                    .map(|s| self.snapshot.text().slice(s.range()).map(|x| x.to_owned()))
                    .collect::<Result<Vec<_>, _>>()?
                    .join(""),
            )
        } else {
            None
        };
        let (changes, positions) = self.lower(command)?;
        if changes.is_empty() {
            return Ok(EditOutcome {
                proposal: None,
                clipboard,
            });
        }
        let next =
            self.snapshot
                .revision()
                .checked_next()
                .ok_or(MarkdownEditError::RevisionOverflow {
                    current: self.snapshot.revision(),
                })?;
        let after_text = apply_changes(self.snapshot.text(), &changes)?;
        let primary = if let Some(selection) = skipped_primary {
            let map = ChangeMap::checked(self.snapshot.text(), &changes)
                .map_err(MarkdownEditError::InvalidChanges)?;
            let selection = map_selection(selection, &map).expect("valid selection boundaries map");
            positions
                .iter()
                .position(|candidate| *candidate == selection)
                .expect("lowering retains each skipped closer")
        } else {
            self.selections
                .primary_index()
                .min(self.selections.as_slice().len() - 1)
        };
        let selection_after = SelectionSet::from_source(next, &after_text, positions, primary)
            .map_err(map_selection_error)?;
        let edit = MarkdownEdit {
            base_revision: self.snapshot.revision(),
            changes,
            selection_after,
            history_group: group,
        };
        let proposal = self.apply_with_history(edit)?;
        Ok(EditOutcome {
            proposal: Some(proposal),
            clipboard,
        })
    }

    pub fn undo(&mut self) -> Result<Option<ProposedMarkdownEdit>, MarkdownEditError> {
        self.cancel_ime();
        let Some(group) = self.history.undo.pop() else {
            return Ok(None);
        };
        if group.is_empty() {
            return self.undo();
        }
        let selection = group.first().unwrap().before_selection.clone();
        let changes = compose_group_changes(
            self.snapshot.text(),
            group
                .iter()
                .rev()
                .map(|entry| entry.inverse_changes.as_slice()),
        )?;
        let proposal = self.apply_restoring(changes, selection)?;
        self.history.redo.push(group);
        Ok(Some(proposal))
    }

    pub fn redo(&mut self) -> Result<Option<ProposedMarkdownEdit>, MarkdownEditError> {
        self.cancel_ime();
        let Some(group) = self.history.redo.pop() else {
            return Ok(None);
        };
        let selection = group.last().unwrap().after_selection.clone();
        let changes = compose_group_changes(
            self.snapshot.text(),
            group.iter().map(|entry| entry.forward_changes.as_slice()),
        )?;
        let proposal = self.apply_restoring(changes, selection)?;
        self.history.undo.push(group);
        Ok(Some(proposal))
    }

    pub fn apply_edit(
        &mut self,
        edit: MarkdownEdit,
    ) -> Result<ProposedMarkdownEdit, MarkdownEditError> {
        self.cancel_ime();
        self.apply_edit_without_history(edit)
    }

    fn apply_with_history(
        &mut self,
        edit: MarkdownEdit,
    ) -> Result<ProposedMarkdownEdit, MarkdownEditError> {
        let before = self.snapshot.clone();
        let before_selection = self.selections.clone();
        let inverse_changes = inverse_changes(before.text(), &edit.changes)?;
        let proposal = self.apply_edit_without_history(edit)?;
        self.history.push(HistoryEntry {
            before,
            before_selection,
            after: self.snapshot.clone(),
            after_selection: self.selections.clone(),
            forward_changes: proposal.edit.changes.clone(),
            inverse_changes,
            group: proposal.edit.history_group,
        });
        Ok(proposal)
    }

    fn apply_restoring(
        &mut self,
        changes: Vec<TextChange>,
        old_selection: SelectionSet,
    ) -> Result<ProposedMarkdownEdit, MarkdownEditError> {
        let next =
            self.snapshot
                .revision()
                .checked_next()
                .ok_or(MarkdownEditError::RevisionOverflow {
                    current: self.snapshot.revision(),
                })?;
        let text = apply_changes(self.snapshot.text(), &changes)?;
        let positions = old_selection
            .as_slice()
            .iter()
            .map(|s| Selection::new(s.anchor, s.cursor))
            .collect();
        let selection_after =
            SelectionSet::from_source(next, &text, positions, old_selection.primary_index())
                .map_err(map_selection_error)?;
        self.apply_edit_without_history(MarkdownEdit {
            base_revision: self.snapshot.revision(),
            changes,
            selection_after,
            history_group: HistoryGroup::isolated(),
        })
    }

    fn lower(
        &self,
        command: EditCommand,
    ) -> Result<(Vec<TextChange>, Vec<Selection>), MarkdownEditError> {
        let source = self.snapshot.text().shared().as_str();
        let selections = self.selections.as_slice();
        let mut changes = Vec::new();
        let mut carets = Vec::new();
        let mut skipped_closers = Vec::new();
        let is_indent = matches!(
            &command,
            EditCommand::Indent { .. } | EditCommand::Outdent { .. }
        );
        let delimiter = match &command {
            EditCommand::Insert(text) => delimiter_pair(text),
            _ => None,
        };
        match command {
            EditCommand::Insert(text) => {
                let pair = delimiter_pair(text.as_ref());
                for selection in selections {
                    let range = selection.range();
                    if let Some(typed) = text.chars().next().filter(|_| text.len() == 1) {
                        if selection.is_empty()
                            && is_closing_delimiter(typed)
                            && source[range.start().to_usize()..].starts_with(typed)
                        {
                            skipped_closers.push(Selection::caret(TextPosition::new(
                                TextSize::try_from_usize(
                                    range.start().to_usize() + typed.len_utf8(),
                                )
                                .unwrap(),
                                Affinity::After,
                            )));
                            continue;
                        }
                    }
                    let replacement = if let Some((open, close)) = pair {
                        if !selection.is_empty() {
                            Arc::from(format!(
                                "{open}{}{}",
                                &source[range.start().to_usize()..range.end().to_usize()],
                                close
                            ))
                        } else if can_insert_pair(source, range.start().to_usize()) {
                            Arc::from(format!("{open}{close}"))
                        } else {
                            text.clone()
                        }
                    } else {
                        text.clone()
                    };
                    changes.push(TextChange {
                        old_range: range,
                        replacement,
                    });
                    let offset = TextSize::try_from_usize(
                        range.start().to_usize() + changes.last().unwrap().replacement.len(),
                    )
                    .unwrap();
                    carets.push(Selection::caret(TextPosition::new(offset, Affinity::After)));
                }
            }
            EditCommand::ReplaceSelections(text) | EditCommand::Paste(text) => {
                let replacement = text;
                for selection in selections {
                    let range = selection.range();
                    changes.push(TextChange {
                        old_range: range,
                        replacement: replacement.clone(),
                    });
                    let offset =
                        TextSize::try_from_usize(range.start().to_usize() + replacement.len())
                            .unwrap();
                    carets.push(Selection::caret(TextPosition::new(offset, Affinity::After)));
                }
            }
            EditCommand::Cut => {
                for selection in selections {
                    let range = selection.range();
                    changes.push(TextChange {
                        old_range: range,
                        replacement: Arc::from(""),
                    });
                    carets.push(Selection::caret(TextPosition::new(
                        range.start(),
                        Affinity::Before,
                    )));
                }
            }
            EditCommand::DeleteBackward | EditCommand::DeleteForward => {
                for selection in selections {
                    let range = if selection.is_empty() {
                        char_range(
                            source,
                            selection.cursor.offset.to_usize(),
                            matches!(command, EditCommand::DeleteBackward),
                        )
                    } else {
                        Some(selection.range())
                    };
                    if let Some(range) = range {
                        changes.push(TextChange {
                            old_range: range,
                            replacement: Arc::from(""),
                        });
                        carets.push(Selection::caret(TextPosition::new(
                            range.start(),
                            Affinity::Before,
                        )));
                    }
                }
            }
            EditCommand::Indent { spaces } | EditCommand::Outdent { spaces } => {
                let starts = selected_line_starts(
                    self.snapshot.line_index(),
                    self.snapshot.text(),
                    selections,
                )?;
                for start in starts {
                    if matches!(command, EditCommand::Indent { .. }) {
                        changes.push(TextChange {
                            old_range: empty_range(start),
                            replacement: Arc::from(" ".repeat(spaces)),
                        });
                    } else {
                        let remove = source[start..]
                            .bytes()
                            .take(spaces)
                            .take_while(|b| *b == b' ')
                            .count();
                        if remove != 0 {
                            changes.push(TextChange {
                                old_range: range(start, start + remove),
                                replacement: Arc::from(""),
                            });
                        }
                    }
                }
                let map = ChangeMap::checked(self.snapshot.text(), &changes)
                    .map_err(MarkdownEditError::InvalidChanges)?;
                carets = selections
                    .iter()
                    .map(|s| map_selection(*s, &map))
                    .collect::<Option<Vec<_>>>()
                    .unwrap_or_else(|| selections.to_vec());
            }
        }
        changes.sort_unstable_by_key(|c| c.old_range.start());
        if !is_indent {
            let map = ChangeMap::checked(self.snapshot.text(), &changes)
                .map_err(MarkdownEditError::InvalidChanges)?;
            let mut delta: isize = 0;
            carets = changes
                .iter()
                .map(|change| {
                    let start = (change.old_range.start().to_usize() as isize + delta) as usize;
                    let (offset, affinity) = if delimiter.is_some()
                        && change.old_range.start() == change.old_range.end()
                        && change.replacement.len() == 2
                    {
                        (
                            TextSize::try_from_usize(start + 1).unwrap(),
                            Affinity::After,
                        )
                    } else if change.replacement.is_empty() {
                        (TextSize::try_from_usize(start).unwrap(), Affinity::Before)
                    } else {
                        (
                            TextSize::try_from_usize(start + change.replacement.len()).unwrap(),
                            Affinity::After,
                        )
                    };
                    delta += change.replacement.len() as isize
                        - (change.old_range.end().to_usize() - change.old_range.start().to_usize())
                            as isize;
                    Selection::caret(TextPosition::new(offset, affinity))
                })
                .collect();
            carets.extend(
                skipped_closers
                    .into_iter()
                    .filter_map(|selection| map_selection(selection, &map)),
            );
        }
        Ok((changes, carets))
    }

    fn closing_delimiter_target(&self, command: &EditCommand) -> Option<Selection> {
        let EditCommand::Insert(text) = command else {
            return None;
        };
        let typed = text.chars().next().filter(|_| text.len() == 1)?;
        if !is_closing_delimiter(typed) || !self.selections.primary().is_empty() {
            return None;
        }
        let offset = self.selections.primary().cursor.offset.to_usize();
        let current = self.snapshot.text().shared().as_str()[offset..]
            .chars()
            .next()?;
        (current == typed).then(|| {
            Selection::caret(TextPosition::new(
                TextSize::try_from_usize(offset + typed.len_utf8()).unwrap(),
                Affinity::After,
            ))
        })
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
        let new_text = apply_changes(old_text, &edit.changes)?;
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
        self.preferred_x = None;
        Ok(ProposedMarkdownEdit {
            edit,
            snapshot,
            syntax_update,
        })
    }
}

fn range(start: usize, end: usize) -> TextRange {
    TextRange::new(
        TextSize::try_from_usize(start).unwrap(),
        TextSize::try_from_usize(end).unwrap(),
    )
    .unwrap()
}
fn empty_range(offset: usize) -> TextRange {
    range(offset, offset)
}
fn char_range(text: &str, offset: usize, back: bool) -> Option<TextRange> {
    if back {
        text[..offset]
            .char_indices()
            .last()
            .map(|(i, _)| range(i, offset))
    } else {
        text[offset..]
            .chars()
            .next()
            .map(|c| range(offset, offset + c.len_utf8()))
    }
}
fn selected_line_starts(
    lines: &LineIndex,
    source: &SourceText,
    selections: &[Selection],
) -> Result<Vec<usize>, MarkdownEditError> {
    let text = source.shared().as_str();
    let mut offsets = vec![0];
    offsets.extend(
        text.char_indices()
            .map(|(offset, _)| offset)
            .filter(|offset| *offset != 0),
    );
    offsets.push(text.len());
    let mut starts = Vec::new();
    for selection in selections {
        let range = selection.range();
        let first = lines.line_col(source, range.start())?.line;
        let end = range.end().to_usize();
        let last_offset = if end > range.start().to_usize() {
            text[..end]
                .char_indices()
                .last()
                .map(|(offset, _)| offset)
                .unwrap_or(0)
        } else {
            end
        };
        let last = lines
            .line_col(source, TextSize::try_from_usize(last_offset).unwrap())?
            .line;
        let mut previous = None;
        for offset in &offsets {
            let line = lines
                .line_col(source, TextSize::try_from_usize(*offset).unwrap())?
                .line;
            if (first..=last).contains(&line) && previous != Some(line) {
                starts.push(*offset);
            }
            previous = Some(line);
        }
    }
    starts.sort_unstable();
    starts.dedup();
    Ok(starts)
}
fn apply_changes(
    old: &SourceText,
    changes: &[TextChange],
) -> Result<SourceText, MarkdownEditError> {
    let mut value = old.shared().as_str().to_owned();
    for c in changes.iter().rev() {
        value.replace_range(
            c.old_range.start().to_usize()..c.old_range.end().to_usize(),
            &c.replacement,
        );
    }
    Ok(SourceText::new(value)?)
}
fn inverse_changes(
    old: &SourceText,
    changes: &[TextChange],
) -> Result<Vec<TextChange>, MarkdownEditError> {
    ChangeMap::checked(old, changes).map_err(MarkdownEditError::InvalidChanges)?;
    let mut delta: isize = 0;
    changes
        .iter()
        .map(|change| {
            let start = (change.old_range.start().to_usize() as isize + delta) as usize;
            let end = start + change.replacement.len();
            delta += change.replacement.len() as isize
                - (change.old_range.end().to_usize() - change.old_range.start().to_usize())
                    as isize;
            Ok(TextChange {
                old_range: range(start, end),
                replacement: Arc::from(old.slice(change.old_range)?.to_owned()),
            })
        })
        .collect::<Result<_, TextError>>()
        .map_err(map_text_error)
}
fn compose_group_changes<'a>(
    current: &SourceText,
    entries: impl Iterator<Item = &'a [TextChange]>,
) -> Result<Vec<TextChange>, MarkdownEditError> {
    let text = current.shared().as_str();
    let mut pieces = vec![Piece::original(range(0, text.len()), text.to_owned())];
    for entry in entries {
        let mut changes: Vec<_> = entry.iter().collect();
        changes.sort_unstable_by_key(|change| std::cmp::Reverse(change.old_range.start()));
        for change in changes {
            apply_piece_change(&mut pieces, change);
        }
    }
    Ok(pieces_to_changes(pieces, text.len()))
}

#[derive(Clone)]
struct Piece {
    original: Option<TextRange>,
    text: String,
}
impl Piece {
    fn original(range: TextRange, text: String) -> Self {
        Self {
            original: Some(range),
            text,
        }
    }
    fn generated(text: String) -> Self {
        Self {
            original: None,
            text,
        }
    }
    fn fragment(&self, start: usize, end: usize) -> Option<Self> {
        (start != end).then(|| Self {
            original: self
                .original
                .map(|old| range(old.start().to_usize() + start, old.start().to_usize() + end)),
            text: self.text[start..end].to_owned(),
        })
    }
}
fn apply_piece_change(pieces: &mut Vec<Piece>, change: &TextChange) {
    let start = change.old_range.start().to_usize();
    let end = change.old_range.end().to_usize();
    let mut output = Vec::new();
    let mut cursor = 0;
    let mut inserted = false;
    for piece in pieces.drain(..) {
        let piece_end = cursor + piece.text.len();
        if piece_end <= start {
            output.push(piece);
        } else if cursor >= end {
            if !inserted {
                output.push(Piece::generated(change.replacement.to_string()));
                inserted = true;
            }
            output.push(piece);
        } else {
            if start > cursor {
                output.push(piece.fragment(0, start - cursor).unwrap());
            }
            if !inserted {
                output.push(Piece::generated(change.replacement.to_string()));
                inserted = true;
            }
            if end < piece_end {
                output.push(piece.fragment(end - cursor, piece.text.len()).unwrap());
            }
        }
        cursor = piece_end;
    }
    if !inserted {
        output.push(Piece::generated(change.replacement.to_string()));
    }
    *pieces = output;
}
fn pieces_to_changes(pieces: Vec<Piece>, original_len: usize) -> Vec<TextChange> {
    let mut changes = Vec::new();
    let mut cursor = 0;
    let mut pending: Option<(usize, usize, String)> = None;
    let mut flush = |pending: &mut Option<(usize, usize, String)>,
                     changes: &mut Vec<TextChange>| {
        if let Some((start, end, replacement)) = pending.take() {
            changes.push(TextChange {
                old_range: range(start, end),
                replacement: Arc::from(replacement),
            });
        }
    };
    for piece in pieces {
        if let Some(old) = piece.original {
            if cursor < old.start().to_usize() {
                let entry = pending.get_or_insert((cursor, cursor, String::new()));
                entry.1 = old.start().to_usize();
            }
            flush(&mut pending, &mut changes);
            cursor = old.end().to_usize();
        } else {
            let entry = pending.get_or_insert((cursor, cursor, String::new()));
            entry.2.push_str(&piece.text);
        }
    }
    if cursor < original_len {
        let entry = pending.get_or_insert((cursor, cursor, String::new()));
        entry.1 = original_len;
    }
    flush(&mut pending, &mut changes);
    changes
}
fn map_selection(selection: Selection, map: &ChangeMap) -> Option<Selection> {
    Some(Selection::new(
        translate_position_with_map(map, selection.anchor)?,
        translate_position_with_map(map, selection.cursor)?,
    ))
}
fn delimiter_pair(text: &str) -> Option<(char, char)> {
    if text.len() != 1 {
        return None;
    }
    match text.chars().next()? {
        '(' => Some(('(', ')')),
        '[' => Some(('[', ']')),
        '{' => Some(('{', '}')),
        '"' => Some(('"', '"')),
        '`' => Some(('`', '`')),
        _ => None,
    }
}
fn is_closing_delimiter(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}' | '"' | '`')
}
fn can_insert_pair(source: &str, offset: usize) -> bool {
    source[offset..]
        .chars()
        .next()
        .map_or(true, |ch| ch.is_whitespace() || is_closing_delimiter(ch))
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
        SelectionError::Changes(reason) => MarkdownEditError::InvalidChanges(reason),
        SelectionError::EmptySet | SelectionError::PrimaryOutOfBounds { .. } => {
            unreachable!("a constructed selection set remains structurally valid")
        }
    }
}
