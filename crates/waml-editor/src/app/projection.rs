//! How this session projects the bundle: the scope the tree is browsing, the
//! stages that are masked off, the descent cap the chain runner is bounded by,
//! and the memoised tree those three produce.
//!
//! The four moved together because they are one dependency graph. `mask` and
//! `limits` are inputs to *every* projection in the editor -- the tree seam and
//! every open folder tab read the same two, which is the whole reason a folder
//! view and the tree can never disagree about what a directory contains -- and
//! `scope` selects which part of the resulting tree the panel shows. The cache
//! is a pure function of them plus the session revision.
//!
//! No makepad types here, so everything in this module is exercisable without a
//! `Cx`.
//!
//! # Invariants
//!
//! * **The key covers every input the build reads.** [`Projection::ensure_tree`]
//!   is the only place a tree is built and the only place the key is minted, so
//!   the two cannot drift apart. Held apart, they did: the key stored
//!   `limits.max_depth` rather than `limits`, which was correct only for as long
//!   as `ChainLimits` had exactly one field. It stores the whole value now.
//! * **Scope is applied to the cached tree, not baked into it.** `scope` is
//!   deliberately NOT part of the key: `nav::view_of` slices the full tree, so a
//!   scope change costs a slice, not a rebuild of every directory's chain.
//! * **The mask is memory-only.** Raw is a deliberate act, not a preference, so
//!   nothing here is persisted and every launch starts with an empty mask and
//!   the author's declared `view:` as what a reader gets.
//!
//! # A note on `scope`
//!
//! Nothing in the shipping UI sets a scope other than the root: the only
//! non-default writes are in `app/tests/navigation.rs`, which set it by hand to
//! assert that a navigation does not clobber it. The affordance in
//! `nav::view_of` is real and tested; the control that would drive it is not
//! built. Kept rather than deleted because the tree panel's scope title reads
//! through the same path on every open.

use crate::nav::NavState;
use crate::tree::ProjectTree;
use waml::view::chain::ChainLimits;
use waml::view::mask::ProjectionMask;

/// Everything a tree build depends on, so a `refresh_nav` that changes none of
/// it cannot pay for one. The session revision covers the bundle itself: both
/// an edit and `EditorSession::replace` bump it.
type TreeKey = (u64, ProjectionMask, ChainLimits);

#[derive(Default)]
pub(super) struct Projection {
    scope: NavState,
    mask: ProjectionMask,
    limits: ChainLimits,
    tree: Option<(TreeKey, ProjectTree)>,
}

impl Projection {
    /// The stages masked off for this session. Read by the tree seam and by
    /// every folder-view open, which is why it is one value and not a copy per
    /// surface.
    pub(super) fn mask(&self) -> &ProjectionMask {
        &self.mask
    }

    /// Install a new mask, reporting whether it actually moved.
    ///
    /// The caller has real work to do on a change (re-run the open folder tabs,
    /// redraw) and none on a no-op, so the comparison lives here rather than
    /// being restated at the call site.
    pub(super) fn set_mask(&mut self, mask: ProjectionMask) -> bool {
        if self.mask == mask {
            return false;
        }
        self.mask = mask;
        true
    }

    /// The view-chain descent cap in force. User/workspace scope ONLY -- a
    /// bundle cannot reach this, by construction: nothing builds `ChainLimits`
    /// from bundle frontmatter. Defaults to 20 when the project declares
    /// nothing.
    pub(super) fn limits(&self) -> ChainLimits {
        self.limits
    }

    /// Seed the descent cap from the project that just opened.
    pub(super) fn set_limits(&mut self, limits: ChainLimits) {
        self.limits = limits;
    }

    /// Return to the whole-model browse state. A replacement bundle's
    /// directories are not the old one's, so a retained scope could name one
    /// that no longer exists.
    pub(super) fn reset_scope(&mut self) {
        self.scope = NavState::default();
    }

    /// The full tree for the current mask and cap, together with the scope it
    /// is to be viewed through.
    ///
    /// Handed out as a pair because both callers need both, and the build that
    /// produces the tree borrows the same value the scope lives on.
    pub(super) fn tree_with_scope(
        &mut self,
        revision: u64,
        okf: &waml::analysis::OkfAnalysis,
        uml: &waml::uml::Analysis,
    ) -> (&ProjectTree, &NavState) {
        self.ensure_tree(revision, okf, uml);
        let tree = &self
            .tree
            .as_ref()
            .expect("ensure_tree populates the cache")
            .1;
        (tree, &self.scope)
    }

    /// Rebuild the tree if anything it reads has moved.
    ///
    /// A build runs the folder-view chain for every directory in the bundle,
    /// recursively, and `refresh_nav` fires on every row click and every
    /// navigation change -- where nothing about the projection moved. Keep the
    /// key and the build in this one function: a new input to `build_tree` that
    /// is not in `key` is a stale tree, which looks like a working view with
    /// content missing.
    fn ensure_tree(
        &mut self,
        revision: u64,
        okf: &waml::analysis::OkfAnalysis,
        uml: &waml::uml::Analysis,
    ) {
        let key: TreeKey = (revision, self.mask.clone(), self.limits);
        if self.tree.as_ref().map(|(cached, _)| cached) == Some(&key) {
            return;
        }
        let tree = crate::tree::build_tree(okf, uml, "Untitled", &self.mask, self.limits);
        self.tree = Some((key, tree));
    }

    /// The part of the bundle the tree panel is browsing. Test seam: the one
    /// production reader takes it from [`Projection::tree_with_scope`], which
    /// is the only place it is ever wanted alongside a tree.
    #[cfg(test)]
    pub(super) fn scope(&self) -> &NavState {
        &self.scope
    }

    /// Overwrite the browse scope. Test seam: no shipping control sets a
    /// non-root scope, but scenarios set one to assert that a navigation
    /// leaves it alone.
    #[cfg(test)]
    pub(super) fn set_scope(&mut self, scope: NavState) {
        self.scope = scope;
    }
}
