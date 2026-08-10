//! The candidate list an author is choosing from, as a value.
//!
//! This crate cannot see the analysis that produces candidates -- it depends on
//! `waml-syntax` and nothing above it -- so a candidate arrives here already
//! reduced to what the editor needs: what to show, what to author, and which
//! bytes to replace. `waml-editor` does the conversion from
//! `waml::uml::Completion`.
//!
//! Keeping the session a plain value, with no widget and no `Cx`, is what lets
//! the selection and accept behaviour be tested without a window.

use std::sync::Arc;

use waml_syntax::{DocumentRevision, TextRange};

/// One offer. `replace` is document-absolute, as every offset in this crate is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionCandidate {
    /// What the list shows.
    pub label: Arc<str>,
    /// What replaces `replace` when the author accepts.
    pub insert: Arc<str>,
    /// A secondary line, when there is something worth saying.
    pub detail: Option<Arc<str>>,
    /// The bytes the candidate replaces. Empty when nothing was authored yet.
    pub replace: TextRange,
}

/// An open completion list. Constructed only when there is something to choose
/// from, so "a session exists" and "the popup is showing" are the same fact and
/// cannot disagree.
#[derive(Clone, Debug)]
pub struct CompletionSession {
    /// The document revision every `replace` range was computed against. The
    /// widget refuses to accept against any other revision: a candidate whose
    /// byte range belongs to an older text is the one way this feature could
    /// corrupt a document, so the check is carried in the value itself.
    revision: DocumentRevision,
    candidates: Vec<CompletionCandidate>,
    selected: usize,
}

impl CompletionSession {
    /// `None` when nothing is offered. An empty popup is never a state: the
    /// caller drops the session rather than showing a list with no rows.
    pub fn open(revision: DocumentRevision, candidates: Vec<CompletionCandidate>) -> Option<Self> {
        if candidates.is_empty() {
            return None;
        }
        Some(Self {
            revision,
            candidates,
            selected: 0,
        })
    }

    pub fn revision(&self) -> DocumentRevision {
        self.revision
    }

    pub fn candidates(&self) -> &[CompletionCandidate] {
        &self.candidates
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn selected(&self) -> &CompletionCandidate {
        // `open` rejects an empty list and `move_selection` wraps, so the index
        // is always in range.
        &self.candidates[self.selected]
    }

    /// Move by `delta`, wrapping at both ends. Wrapping rather than clamping
    /// because a list the author is arrowing through should not silently
    /// dead-end at the last row.
    pub fn move_selection(&mut self, delta: i32) {
        let len = self.candidates.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len);
        self.selected = next as usize;
    }

    /// Select by row, for a pointer click. Out-of-range is ignored rather than
    /// clamped: a click that misses every row should change nothing.
    pub fn select(&mut self, index: usize) {
        if index < self.candidates.len() {
            self.selected = index;
        }
    }

    /// The edit to apply: replace these bytes with this text. The caller sets
    /// the selection to `range` and issues `EditCommand::ReplaceSelections`,
    /// which is why no new edit primitive is needed for completion.
    pub fn accept(&self) -> (TextRange, Arc<str>) {
        let candidate = self.selected();
        (candidate.replace, candidate.insert.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml_syntax::TextSize;

    const REVISION: DocumentRevision = DocumentRevision::new(7);

    fn at(start: usize, end: usize) -> TextRange {
        TextRange::new(
            TextSize::try_from_usize(start).unwrap(),
            TextSize::try_from_usize(end).unwrap(),
        )
        .unwrap()
    }

    fn candidate(label: &str, insert: &str) -> CompletionCandidate {
        CompletionCandidate {
            label: Arc::from(label),
            insert: Arc::from(insert),
            detail: None,
            replace: at(4, 7),
        }
    }

    #[test]
    fn an_empty_offer_list_opens_no_session() {
        assert!(CompletionSession::open(REVISION, Vec::new()).is_none());
    }

    #[test]
    fn a_session_remembers_the_revision_its_ranges_belong_to() {
        let session = CompletionSession::open(REVISION, vec![candidate("calls", "calls")]).unwrap();
        assert_eq!(session.revision(), REVISION);
    }

    #[test]
    fn the_first_candidate_is_selected_to_begin_with() {
        let session = CompletionSession::open(
            REVISION,
            vec![candidate("calls", "calls"), candidate("returns", "returns")],
        )
        .unwrap();
        assert_eq!(session.selected_index(), 0);
        assert_eq!(session.selected().label.as_ref(), "calls");
    }

    #[test]
    fn arrowing_past_either_end_wraps_rather_than_dead_ending() {
        let mut session = CompletionSession::open(
            REVISION,
            vec![
                candidate("a", "a"),
                candidate("b", "b"),
                candidate("c", "c"),
            ],
        )
        .unwrap();
        session.move_selection(1);
        assert_eq!(session.selected().label.as_ref(), "b");
        session.move_selection(-1);
        assert_eq!(session.selected().label.as_ref(), "a");
        // Backwards off the front lands on the last row.
        session.move_selection(-1);
        assert_eq!(session.selected().label.as_ref(), "c");
        // Forwards off the end lands on the first.
        session.move_selection(1);
        assert_eq!(session.selected().label.as_ref(), "a");
    }

    #[test]
    fn a_click_that_misses_every_row_changes_nothing() {
        let mut session =
            CompletionSession::open(REVISION, vec![candidate("a", "a"), candidate("b", "b")])
                .unwrap();
        session.select(1);
        assert_eq!(session.selected_index(), 1);
        session.select(9);
        assert_eq!(session.selected_index(), 1);
    }

    #[test]
    fn accepting_yields_the_selected_candidates_range_and_text() {
        let mut session = CompletionSession::open(
            REVISION,
            vec![candidate("calls", "calls"), candidate("returns", "returns")],
        )
        .unwrap();
        session.move_selection(1);
        let (range, text) = session.accept();
        assert_eq!(range, at(4, 7));
        assert_eq!(text.as_ref(), "returns");
    }
}
