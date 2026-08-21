//! The live bundle-wide search session (spec §Search session) and the cursor
//! index an F3/Shift+F3 step has stamped for the landing it is about to cause.
//!
//! The two moved together because they are one rule: **a step chooses the
//! index; the landing it triggers must use that index rather than re-deriving
//! one.** It cannot re-derive it — hits routinely share a
//! `(concept_id, RevealTarget)` pair (every Names/Model/Structure entry of one
//! concept carries the same `HitTarget::ModelElement`), so locating one by
//! target alone snaps the cursor back to the first of them.
//!
//! Held apart, that rule was two statements a caller had to get right in order,
//! and the stamp could outlive the session that minted it: `end_session_search`
//! cleared `session_search` and left `stepped_session_index` behind. Nothing
//! reads a stale stamp today, because the only producer stamps and the only
//! consumer takes inside one synchronous call — but only *reading the whole
//! chain* told you that, which is the same as not knowing.
//!
//! # Invariants
//!
//! * **Stamping is not a separate act.** [`SessionSearch::step`] stamps as part
//!   of advancing; there is no way to advance without stamping, and no way to
//!   stamp anything a step did not choose.
//! * **The stamp is consumed by the next landing, session or not.**
//!   [`SessionSearch::mark_landing`] takes it before it looks at whether a
//!   session is live, so a stamp cannot survive into a later landing.
//! * **Ending clears both.** A session and a stamp for it begin and end
//!   together.

use super::*;
use waml::search::Hit as SearchHit;

#[derive(Default)]
pub(super) struct SessionSearch {
    session: Option<SearchSession>,
    stepped: Option<usize>,
}

impl SessionSearch {
    /// Start a session, replacing any live one and its stamp.
    pub(super) fn begin(&mut self, session: SearchSession) {
        self.session = Some(session);
        self.stepped = None;
    }

    /// End the session. Esc does this; so does any caller that needs the
    /// bundle-wide traversal to stop claiming keys.
    pub(super) fn end(&mut self) {
        self.session = None;
        self.stepped = None;
    }

    /// Whether a bundle-wide session is live. F3/Shift+F3 and Esc route to it
    /// only while it is.
    pub(super) fn is_active(&self) -> bool {
        self.session.is_some()
    }

    /// The live session's query, hits and cursor, for the surfaces that read
    /// them: the results tab's id, and the per-document highlight set.
    pub(super) fn session(&self) -> Option<&SearchSession> {
        self.session.as_ref()
    }

    /// Where the session's cursor is now.
    pub(super) fn cursor(&self) -> Option<usize> {
        self.session.as_ref().and_then(|session| session.cursor)
    }

    /// Advance the cursor one hit, wrapping, and stamp the index it landed on
    /// for the landing this hit is about to cause. `None` with no live session
    /// or no hits.
    pub(super) fn step(&mut self, forward: bool) -> Option<SearchHit> {
        let session = self.session.as_mut()?;
        let hit = session.advance(forward).cloned()?;
        self.stepped = session.cursor;
        Some(hit)
    }

    /// Record where a landing put the session cursor, consuming any stamp a
    /// step left. Returns whether a session was live — a landing with none is
    /// not a session landing, and its caller has nothing to mirror.
    ///
    /// A stamped index wins. A landing from anywhere else (a results-tab row
    /// click, a palette commit) can only locate itself by target, matching how
    /// `navigation_for_hit` maps a hit to a reveal, since that is the only
    /// place doing the reverse mapping; several hits of one concept share a
    /// target, so it lands on the first of them.
    pub(super) fn mark_landing(
        &mut self,
        concept_id: &str,
        target: &crate::doc_view::RevealTarget,
    ) -> bool {
        let stepped = self.stepped.take();
        let Some(session) = self.session.as_mut() else {
            return false;
        };
        session.cursor = stepped
            .filter(|&index| index < session.hits.len())
            .or_else(|| {
                session.hits.iter().position(|hit| {
                    crate::search_results_view::navigation_for_hit(hit)
                        .1
                        .as_ref()
                        .is_some_and(|(cid, t)| cid == concept_id && t == target)
                })
            });
        true
    }
}
