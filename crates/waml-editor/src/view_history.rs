//! Logical Back/Forward history for document views.
//!
//! Locations identify model documents and value-only anchors, not tabs or
//! widgets. Traversal skips currently unresolved locations without deleting
//! them, so a later Undo can make an old target reachable again. The timeline
//! is capped independently of model Undo/Redo and never mutates model state.

use std::collections::VecDeque;

use waml_markdown_editor::{
    input::ScrollState,
    selection::SelectionSet,
    syntax::{DocumentRevision, SourceText, TextSize},
};

pub const VIEW_HISTORY_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DocumentKind {
    Primary,
    Source,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DocumentLocator {
    pub concept_id: String,
    pub kind: DocumentKind,
}

impl DocumentLocator {
    pub fn new(concept_id: impl Into<String>, kind: DocumentKind) -> Self {
        Self {
            concept_id: concept_id.into(),
            kind,
        }
    }

    pub fn primary(concept_id: impl Into<String>) -> Self {
        Self::new(concept_id, DocumentKind::Primary)
    }

    pub fn source(concept_id: impl Into<String>) -> Self {
        Self::new(concept_id, DocumentKind::Source)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiagramCameraAnchor {
    pub pan_x: f64,
    pub pan_y: f64,
    pub zoom: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewAnchor {
    None,
    Markdown {
        fragment: Option<String>,
        revision: DocumentRevision,
        selection: SelectionSet,
        scroll: ScrollState,
    },
    Diagram {
        selected_key: Option<String>,
        camera: DiagramCameraAnchor,
    },
}

impl ViewAnchor {
    pub fn markdown_start(
        revision: DocumentRevision,
        fragment: Option<String>,
        scroll: ScrollState,
    ) -> Self {
        let empty =
            SourceText::new(String::new()).expect("an empty string is always valid source text");
        let selection = SelectionSet::caret_in_text(revision, &empty, TextSize::new(0))
            .expect("zero is a valid caret offset in empty source text");
        Self::Markdown {
            fragment,
            revision,
            selection,
            scroll,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewLocation {
    pub document: DocumentLocator,
    pub anchor: ViewAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryDirection {
    Back,
    Forward,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryTarget {
    index: usize,
    pub location: ViewLocation,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewHistory {
    entries: VecDeque<ViewLocation>,
    cursor: Option<usize>,
}

impl ViewHistory {
    pub fn reset(&mut self, initial: Option<ViewLocation>) {
        self.entries.clear();
        self.cursor = None;
        if let Some(initial) = initial {
            self.entries.push_back(initial);
            self.cursor = Some(0);
        }
    }

    pub fn record_transition(&mut self, departing: ViewLocation, arriving: ViewLocation) {
        let cursor = match self.cursor {
            Some(cursor) => cursor,
            None => {
                self.entries.clear();
                self.entries.push_back(departing.clone());
                self.cursor = Some(0);
                0
            }
        };
        self.entries[cursor] = departing;
        if self.entries.get(cursor) == Some(&arriving) {
            return;
        }
        self.entries.truncate(cursor + 1);
        self.entries.push_back(arriving);
        self.cursor = Some(cursor + 1);

        while self.entries.len() > VIEW_HISTORY_LIMIT {
            self.entries.pop_front();
            self.cursor = self.cursor.map(|cursor| cursor.saturating_sub(1));
        }
    }

    pub fn refresh_current(&mut self, current: ViewLocation) {
        if let Some(cursor) = self.cursor {
            self.entries[cursor] = current;
        } else {
            self.reset(Some(current));
        }
    }

    pub fn target(
        &self,
        direction: HistoryDirection,
        mut resolves: impl FnMut(&ViewLocation) -> bool,
    ) -> Option<HistoryTarget> {
        let cursor = self.cursor?;
        let index = match direction {
            HistoryDirection::Back => (0..cursor)
                .rev()
                .find(|&index| resolves(&self.entries[index])),
            HistoryDirection::Forward => {
                ((cursor + 1)..self.entries.len()).find(|&index| resolves(&self.entries[index]))
            }
        }?;
        Some(HistoryTarget {
            index,
            location: self.entries[index].clone(),
        })
    }

    pub fn commit_traversal(&mut self, target: HistoryTarget) {
        if target.index < self.entries.len() && self.entries[target.index] == target.location {
            self.cursor = Some(target.index);
        }
    }

    pub fn can_traverse(
        &self,
        direction: HistoryDirection,
        resolves: impl FnMut(&ViewLocation) -> bool,
    ) -> bool {
        self.target(direction, resolves).is_some()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current(&self) -> Option<&ViewLocation> {
        self.cursor.and_then(|cursor| self.entries.get(cursor))
    }

    /// Window into the raw entry list, primarily for tests asserting an
    /// intermediate (non-current) entry's anchor without a traversal. Cheap
    /// enough to leave available outside `cfg(test)`: the lib crate's own
    /// `cfg(test)` does not cover the `waml-editor` binary crate's tests,
    /// which depend on this crate as an ordinary dependency.
    pub fn entry_at(&self, index: usize) -> Option<&ViewLocation> {
        self.entries.get(index)
    }
}
