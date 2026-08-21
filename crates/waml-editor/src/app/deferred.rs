//! What a navigation still owes once its target tab has drawn.
//!
//! Three things can survive a navigation into the next `Event::Draw`, because
//! each needs geometry that does not exist until the arriving view has laid
//! itself out: the `#fragment` a link asked to scroll to, the search hit a
//! results row asked to reveal, and the scroll/selection anchor a history
//! traversal is restoring. `App::handle_draw_restores` applies all three at the
//! same gate, in that order.
//!
//! # The rule they share
//!
//! **A deferred apply is the tail of one navigation gesture, and it is
//! abandoned if some other document is the one that draws.** [`claim`] is the
//! only way to read a slot, and it empties the slot *before* it asks whether
//! the drawn tab is the one that armed it — so "the target moved on" cannot be
//! written as a path that leaves the value armed for later. The reason is what
//! the alternative looks like to a user: a deferred apply that survives its
//! mismatch fires at an arbitrary future moment, attached to whatever gesture
//! happens to bring that document back. The user clicks a row in the tree and
//! the document jumps to a heading they last asked for minutes ago, possibly
//! with a stale "Section not found" in the status bar. A navigation that
//! silently completes long after the user stopped waiting for it is not a
//! completed navigation; it is a surprise.
//!
//! `pending_fragment` used to be the exception — it read its value by
//! reference and returned early without clearing, so it stayed armed across an
//! unrelated later visit. That is the behaviour this type deliberately drops;
//! see `docs/reviews/visual-signoff-ledger.md`.
//!
//! The mismatch is not the ordinary case: a transition activates its tab
//! synchronously, so the very next draw already has the right tab up. Reaching
//! `claim` with a mismatch means something genuinely superseded the gesture.
//!
//! # The generation
//!
//! Only the anchor restore carries one, and it is a second, narrower question
//! than the tab check: a restore whose tab *does* match can still be a stale
//! generation's, if a second rapid traversal scheduled its own while this one
//! was deferred. Applying it to the view is still right; refreshing history
//! from it is not. Scheduling bumps the generation and stamps the new restore
//! with it, so "bump then stamp" is one statement rather than two a caller has
//! to get in the right order.

use super::*;

/// The `#fragment` a link navigation still owes its target document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PendingFragment {
    pub(super) concept_id: String,
    pub(super) fragment: String,
}

/// A search-hit reveal awaiting its target document's tab (`ViewOutcome.reveal`,
/// spec §DocView::reveal).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingReveal {
    pub(super) concept_id: String,
    pub(super) target: crate::doc_view::RevealTarget,
}

/// The scroll/selection anchor a history traversal still owes, plus the
/// generation it was scheduled at.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingAnchorRestore {
    pub(super) document: crate::navigation::DocumentLocator,
    pub(super) anchor: ViewAnchor,
    /// Stamped from [`DeferredNavigation::generation`] when this restore was
    /// scheduled, so a later traversal that supersedes it can be detected.
    pub(super) generation: u64,
}

/// Take a deferred apply, keeping it only if the tab that drew is the one it
/// was armed for.
///
/// The slot is emptied either way. See the module docs for why that is the
/// shared rule rather than a per-slot choice.
fn claim<T>(slot: &mut Option<T>, drew_its_target: impl FnOnce(&T) -> bool) -> Option<T> {
    slot.take().filter(drew_its_target)
}

/// Everything a navigation still owes once its target tab has drawn.
#[derive(Default)]
pub(super) struct DeferredNavigation {
    fragment: Option<PendingFragment>,
    reveal: Option<PendingReveal>,
    anchor: Option<PendingAnchorRestore>,
    generation: u64,
}

impl DeferredNavigation {
    /// Arm (or, with `None`, disarm) the fragment a document navigation owes.
    ///
    /// Takes an `Option` because every document navigation replaces what the
    /// last one armed, including with nothing: arriving somewhere without a
    /// fragment is itself the statement that no scroll is owed.
    pub(super) fn arm_fragment(&mut self, fragment: Option<PendingFragment>) {
        self.fragment = fragment;
    }

    /// Arm the search-hit reveal a landing owes.
    pub(super) fn arm_reveal(&mut self, reveal: PendingReveal) {
        self.reveal = Some(reveal);
    }

    /// Schedule an anchor restore, superseding any still-deferred one.
    pub(super) fn schedule_anchor(
        &mut self,
        document: crate::navigation::DocumentLocator,
        anchor: ViewAnchor,
    ) {
        self.generation = self.generation.wrapping_add(1);
        self.anchor = Some(PendingAnchorRestore {
            document,
            anchor,
            generation: self.generation,
        });
    }

    /// Claim the fragment scroll, if the concept that drew is the one that
    /// asked for it.
    pub(super) fn claim_fragment(
        &mut self,
        drawn_concept: Option<&str>,
    ) -> Option<PendingFragment> {
        claim(&mut self.fragment, |pending| {
            drawn_concept == Some(pending.concept_id.as_str())
        })
    }

    /// Claim the search-hit reveal, if the concept that drew is the one the
    /// hit is in.
    pub(super) fn claim_reveal(&mut self, drawn_concept: Option<&str>) -> Option<PendingReveal> {
        claim(&mut self.reveal, |pending| {
            drawn_concept == Some(pending.concept_id.as_str())
        })
    }

    /// Claim the anchor restore, if the document that drew is the one being
    /// restored. Matches on the whole locator, not the concept: an anchor is
    /// a position within one *surface*.
    pub(super) fn claim_anchor(
        &mut self,
        drawn: Option<&crate::navigation::DocumentLocator>,
    ) -> Option<PendingAnchorRestore> {
        claim(&mut self.anchor, |pending| drawn == Some(&pending.document))
    }

    /// The document a still-deferred anchor restore is for, if any.
    ///
    /// A departing view whose restore has not applied yet has a stale captured
    /// anchor, and refreshing history with it would corrupt the entry that
    /// restore is about to write.
    pub(super) fn pending_anchor_document(&self) -> Option<&crate::navigation::DocumentLocator> {
        self.anchor.as_ref().map(|pending| &pending.document)
    }

    /// Whether `pending` is still the newest scheduled restore.
    ///
    /// A superseded restore's anchor is still correct for the view, but
    /// refreshing history with it would clobber the newer entry.
    pub(super) fn anchor_is_current(&self, pending: &PendingAnchorRestore) -> bool {
        pending.generation == self.generation
    }

    /// The current anchor generation. Test seam: a scenario asserts that the
    /// restore it observed carries the generation the schedule minted.
    #[cfg(test)]
    pub(super) fn anchor_generation(&self) -> u64 {
        self.generation
    }

    /// The deferred anchor restore without claiming it. Test seam: a scenario
    /// checks what a traversal scheduled before the deferred draw applies it.
    #[cfg(test)]
    pub(super) fn peek_anchor(&self) -> Option<&PendingAnchorRestore> {
        self.anchor.as_ref()
    }

    /// The armed fragment without claiming it. Test seam.
    #[cfg(test)]
    pub(super) fn peek_fragment(&self) -> Option<&PendingFragment> {
        self.fragment.as_ref()
    }

    /// The armed reveal without claiming it. Test seam.
    #[cfg(test)]
    pub(super) fn peek_reveal(&self) -> Option<&PendingReveal> {
        self.reveal.as_ref()
    }
}
