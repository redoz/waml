//! The live search session (Task 9, spec §Search session): the query's hits
//! in results-tab order plus a cursor. `App` owns one; the results tab, the
//! find strip, and F3/Shift+F3 traversal (Tasks 12-14) all walk THIS list --
//! there is no second ordering.

use waml::search::{Hit as SearchHit, QueryScope};

/// `App` owns two of these: `find` (Task 13, document-scoped, driven by the
/// Ctrl+F strip) and, from Task 14, a bundle-wide `session_search` for F3
/// traversal across documents.
pub struct SearchSession {
    pub query: String,
    pub hits: Vec<SearchHit>,
    pub cursor: Option<usize>,
    pub scope: QueryScope,
}

impl SearchSession {
    pub fn new(query: String, hits: Vec<SearchHit>, scope: QueryScope) -> Self {
        SearchSession {
            query,
            hits,
            cursor: None,
            scope,
        }
    }

    /// Advance the cursor one step, wrapping at either end. `None` when
    /// there are no hits to walk (the cursor is cleared, never left
    /// dangling on a list that just emptied).
    pub fn advance(&mut self, forward: bool) -> Option<&SearchHit> {
        if self.hits.is_empty() {
            self.cursor = None;
            return None;
        }
        let len = self.hits.len();
        let next = match self.cursor {
            None if forward => 0,
            None => len - 1,
            Some(index) if forward => (index + 1) % len,
            Some(index) => (index + len - 1) % len,
        };
        self.cursor = Some(next);
        self.hits.get(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::search::{FieldGroup, HitTarget};

    fn hit(document: &str) -> SearchHit {
        SearchHit {
            document: document.to_string(),
            concept_id: None,
            group: FieldGroup::Names,
            target: HitTarget::ModelElement {
                key: document.to_string(),
            },
            score: 1.0,
        }
    }

    fn session(documents: &[&str]) -> SearchSession {
        SearchSession::new(
            "q".to_string(),
            documents.iter().map(|d| hit(d)).collect(),
            QueryScope::default(),
        )
    }

    #[test]
    fn advance_walks_forward_and_wraps() {
        let mut session = session(&["a", "b", "c"]);
        assert_eq!(session.advance(true).unwrap().document, "a");
        assert_eq!(session.advance(true).unwrap().document, "b");
        assert_eq!(session.advance(true).unwrap().document, "c");
        assert_eq!(session.advance(true).unwrap().document, "a");
    }

    #[test]
    fn advance_walks_backward_and_wraps() {
        let mut session = session(&["a", "b", "c"]);
        assert_eq!(session.advance(false).unwrap().document, "c");
        assert_eq!(session.advance(false).unwrap().document, "b");
        assert_eq!(session.advance(false).unwrap().document, "a");
        assert_eq!(session.advance(false).unwrap().document, "c");
    }

    #[test]
    fn advance_over_no_hits_returns_none_and_clears_cursor() {
        let mut session = session(&[]);
        assert!(session.advance(true).is_none());
        assert!(session.cursor.is_none());
        assert!(session.advance(false).is_none());
        assert!(session.cursor.is_none());
    }
}
