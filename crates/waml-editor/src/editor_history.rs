use std::ops::Range;

use waml::edit::PendingEdit;

use crate::view_history::{DocumentLocator, ViewLocation};

pub const ATOMIC_TAIL: usize = 64;
pub const HISTORY_LIMIT: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HistoryStateId(u64);

impl HistoryStateId {
    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditMergeKind {
    Insert,
    Delete,
    Continuous,
    Structural,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditMergeKey {
    pub document: DocumentLocator,
    pub control: String,
    pub kind: EditMergeKind,
    pub span: Option<Range<usize>>,
}

pub struct EditHistoryStep {
    edit: PendingEdit,
    label: String,
    merge_key: Option<EditMergeKey>,
    source_state: HistoryStateId,
    target_state: HistoryStateId,
    target_location: ViewLocation,
    reciprocal_location: ViewLocation,
    merge_epoch: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedSide {
    Undo,
    Redo,
}

pub struct PreparedHistoryEdit {
    edit: PendingEdit,
    label: String,
    target_location: ViewLocation,
    source_state: HistoryStateId,
    target_state: HistoryStateId,
    side: PreparedSide,
}

impl PreparedHistoryEdit {
    pub fn edit(&self) -> &PendingEdit {
        &self.edit
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn target_location(&self) -> &ViewLocation {
        &self.target_location
    }

    pub fn source_state(&self) -> HistoryStateId {
        self.source_state
    }

    pub fn target_state(&self) -> HistoryStateId {
        self.target_state
    }
}

pub struct EditorHistory {
    undo: Vec<EditHistoryStep>,
    redo: Vec<EditHistoryStep>,
    current_state: HistoryStateId,
    saved_state: HistoryStateId,
    next_state: u64,
    merge_epoch: u64,
}

impl Default for EditorHistory {
    fn default() -> Self {
        let initial = HistoryStateId(0);
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            current_state: initial,
            saved_state: initial,
            next_state: 1,
            merge_epoch: 0,
        }
    }
}

impl EditorHistory {
    pub fn reset(&mut self) {
        let fresh = self.allocate_state();
        self.undo.clear();
        self.redo.clear();
        self.current_state = fresh;
        self.saved_state = fresh;
        self.break_merge_group();
    }

    pub fn record_edit(
        &mut self,
        inverse: PendingEdit,
        label: impl Into<String>,
        merge_key: Option<EditMergeKey>,
        before_location: ViewLocation,
        after_location: ViewLocation,
    ) -> HistoryStateId {
        let source_state = self.allocate_state();
        let step = EditHistoryStep {
            edit: inverse,
            label: label.into(),
            merge_key,
            source_state,
            target_state: self.current_state,
            target_location: before_location,
            reciprocal_location: after_location,
            merge_epoch: self.merge_epoch,
        };
        self.undo.push(step);
        self.redo.clear();
        self.current_state = source_state;
        self.coalesce_old_undo();
        self.enforce_limit();
        source_state
    }

    pub fn prepare_undo(&self) -> Option<PreparedHistoryEdit> {
        Self::prepare(self.undo.last()?, PreparedSide::Undo)
    }

    pub fn prepare_redo(&self) -> Option<PreparedHistoryEdit> {
        Self::prepare(self.redo.last()?, PreparedSide::Redo)
    }

    pub fn commit_undo(&mut self, prepared: PreparedHistoryEdit, reciprocal: PendingEdit) -> bool {
        if prepared.side != PreparedSide::Undo
            || !Self::matches_prepared(self.undo.last(), &prepared)
        {
            return false;
        }
        let original = self.undo.pop().expect("checked undo top");
        self.current_state = original.target_state;
        self.redo.push(Self::reciprocal_step(original, reciprocal));
        true
    }

    pub fn commit_redo(&mut self, prepared: PreparedHistoryEdit, reciprocal: PendingEdit) -> bool {
        if prepared.side != PreparedSide::Redo
            || !Self::matches_prepared(self.redo.last(), &prepared)
        {
            return false;
        }
        let original = self.redo.pop().expect("checked redo top");
        self.current_state = original.target_state;
        self.undo.push(Self::reciprocal_step(original, reciprocal));
        true
    }

    pub fn abort_undo(&self, prepared: PreparedHistoryEdit) {
        debug_assert_eq!(prepared.side, PreparedSide::Undo);
    }

    pub fn abort_redo(&self, prepared: PreparedHistoryEdit) {
        debug_assert_eq!(prepared.side, PreparedSide::Redo);
    }

    pub fn can_undo(&self) -> bool {
        self.undo.last().is_some_and(|step| {
            step.source_state == self.current_state && step.target_state != self.current_state
        })
    }

    pub fn can_redo(&self) -> bool {
        self.redo.last().is_some_and(|step| {
            step.source_state == self.current_state && step.target_state != self.current_state
        })
    }

    pub fn current_state(&self) -> HistoryStateId {
        self.current_state
    }

    pub fn saved_state(&self) -> HistoryStateId {
        self.saved_state
    }

    pub fn is_saved(&self) -> bool {
        self.current_state == self.saved_state
    }

    pub fn mark_saved(&mut self) {
        self.saved_state = self.current_state;
        self.break_merge_group();
    }

    pub fn break_merge_group(&mut self) {
        self.merge_epoch = self.merge_epoch.wrapping_add(1);
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    fn allocate_state(&mut self) -> HistoryStateId {
        let state = HistoryStateId(self.next_state);
        self.next_state = self.next_state.wrapping_add(1);
        state
    }

    fn prepare(step: &EditHistoryStep, side: PreparedSide) -> Option<PreparedHistoryEdit> {
        Some(PreparedHistoryEdit {
            edit: step.edit.clone(),
            label: step.label.clone(),
            target_location: step.target_location.clone(),
            source_state: step.source_state,
            target_state: step.target_state,
            side,
        })
    }

    fn matches_prepared(step: Option<&EditHistoryStep>, prepared: &PreparedHistoryEdit) -> bool {
        step.is_some_and(|step| {
            step.source_state == prepared.source_state
                && step.target_state == prepared.target_state
                && step.label == prepared.label
        })
    }

    fn reciprocal_step(original: EditHistoryStep, edit: PendingEdit) -> EditHistoryStep {
        EditHistoryStep {
            edit,
            label: original.label,
            merge_key: original.merge_key,
            source_state: original.target_state,
            target_state: original.source_state,
            target_location: original.reciprocal_location,
            reciprocal_location: original.target_location,
            merge_epoch: original.merge_epoch,
        }
    }

    fn coalesce_old_undo(&mut self) {
        loop {
            let older_len = self.undo.len().saturating_sub(ATOMIC_TAIL);
            let Some(index) =
                (0..older_len.saturating_sub(1)).find(|&index| self.can_merge(index, index + 1))
            else {
                break;
            };
            let right = self.undo.remove(index + 1);
            let left = self.undo.remove(index);
            let merge_key = merged_key(
                left.merge_key.as_ref().expect("checked merge key"),
                right.merge_key.as_ref().expect("checked merge key"),
            )
            .expect("checked compatible spans");
            self.undo.insert(
                index,
                EditHistoryStep {
                    edit: PendingEdit::sequence(vec![right.edit, left.edit]),
                    label: right.label,
                    merge_key: Some(merge_key),
                    source_state: right.source_state,
                    target_state: left.target_state,
                    target_location: left.target_location,
                    reciprocal_location: right.reciprocal_location,
                    merge_epoch: left.merge_epoch,
                },
            );
        }
    }

    fn can_merge(&self, left: usize, right: usize) -> bool {
        let left = &self.undo[left];
        let right = &self.undo[right];
        if left.merge_epoch != right.merge_epoch
            || left.source_state != right.target_state
            || touches(left, self.saved_state)
            || touches(right, self.saved_state)
        {
            return false;
        }
        let (Some(left_key), Some(right_key)) = (&left.merge_key, &right.merge_key) else {
            return false;
        };
        merged_key(left_key, right_key).is_some()
    }

    fn enforce_limit(&mut self) {
        while self.undo.len() + self.redo.len() > HISTORY_LIMIT {
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else if !self.redo.is_empty() {
                self.redo.remove(0);
            }
        }
    }
}

fn touches(step: &EditHistoryStep, state: HistoryStateId) -> bool {
    step.source_state == state || step.target_state == state
}

fn merged_key(left: &EditMergeKey, right: &EditMergeKey) -> Option<EditMergeKey> {
    if left.document != right.document
        || left.control != right.control
        || left.kind != right.kind
        || left.kind == EditMergeKind::Structural
    {
        return None;
    }
    let span = match (&left.span, &right.span, &left.kind) {
        (None, None, EditMergeKind::Continuous) => None,
        (Some(left), Some(right), _) if left.end == right.start || right.end == left.start => {
            Some(left.start.min(right.start)..left.end.max(right.end))
        }
        _ => return None,
    };
    Some(EditMergeKey {
        document: left.document.clone(),
        control: left.control.clone(),
        kind: left.kind.clone(),
        span,
    })
}
