//! `Projection`: the trait every folder-view middleware stage implements,
//! plus the machinery (`ProjectionCtx`, `Next`, `RowOp`) a stage uses to
//! read its context and delegate to the rest of the chain.

use std::fmt;

use crate::frontmatter::Frontmatter;
use crate::okf;

use super::chain::Chain;
use super::row::{Row, RowId, RowPath};
use super::surface::SurfaceId;

pub struct ProjectionCtx<'a> {
    pub dir: &'a okf::Directory,
    pub bundle: &'a okf::Bundle,
    /// This middleware's frontmatter params (the folder's index frontmatter).
    pub params: &'a Frontmatter,
    /// Default descent: resolve the child's own declared chain.
    pub descend: &'a dyn Fn(&okf::Directory) -> Chain,
}

/// Ops a surface can address to a row. v1 set mirrors what the root view can
/// lower to OKF ops (Phase G); middleware forward or refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowOp {
    Rename {
        title: String,
    },
    Delete,
    Reorder {
        before: Option<RowPath>,
    },
    InsertConcept {
        after: Option<RowPath>,
        title: String,
    },
    MoveIn {
        from: RowId,
    },
    MoveOut,
}

/// Op-level refusal, not a chain failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported;

/// Path no longer resolves; NOT a chain failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unresolved;

/// Stage failure — triggers whole-chain fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionError {
    pub message: String,
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ProjectionError {}

/// The continuation. Calling it runs the rest of the chain. Built and handed
/// out only by the chain runner (Task B5); the type here is the shape a
/// [`Projection`] stage sees.
pub struct Next<'a> {
    pub(super) remaining: &'a [Box<dyn Projection>],
}

impl<'a> Next<'a> {
    /// Reaching the end of `remaining` is not "no more rows" -- it is the
    /// declared chain running out, at which point the terminal stage (the
    /// root view, [`super::root::RootView`]) takes over. This is how "a
    /// folder with no `view:` declaration" and "a chain's tail" end up being
    /// the same one row of code, per the design's terminal-stage model.
    pub fn project(self, ctx: &ProjectionCtx<'_>) -> Result<Vec<Row>, ProjectionError> {
        match self.remaining.split_first() {
            Some((stage, rest)) => stage.project(ctx, Next { remaining: rest }),
            None => super::root::RootView.project(ctx, Next { remaining: &[] }),
        }
    }

    pub fn apply(
        self,
        ctx: &ProjectionCtx<'_>,
        path: &RowPath,
        op: RowOp,
    ) -> Result<Vec<okf::Op>, Unsupported> {
        match self.remaining.split_first() {
            Some((stage, rest)) => stage.apply(ctx, path, op, Next { remaining: rest }),
            None => Err(Unsupported),
        }
    }

    pub fn surface(self, ctx: &ProjectionCtx<'_>) -> SurfaceId {
        match self.remaining.split_first() {
            Some((stage, rest)) => stage.surface(ctx, Next { remaining: rest }),
            None => super::root::RootView.surface(ctx, Next { remaining: &[] }),
        }
    }
}

pub trait Projection {
    fn project(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>)
        -> Result<Vec<Row>, ProjectionError>;

    /// Given a path this middleware minted, return the rows along it, labels
    /// included — from the directory alone, on a later run. Breadcrumbs, deep
    /// links, and session restore are the same call.
    fn resolve(&self, ctx: &ProjectionCtx<'_>, path: &RowPath) -> Result<Vec<Row>, Unresolved>;

    fn apply(
        &self,
        ctx: &ProjectionCtx<'_>,
        path: &RowPath,
        op: RowOp,
        next: Next<'_>,
    ) -> Result<Vec<okf::Op>, Unsupported>;

    /// Which surface renders this container's own tab. Decline by returning
    /// `next.surface(ctx)`.
    fn surface(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>) -> SurfaceId;
}

/// Identity middleware — forwards everything to the rest of the chain.
/// Used as the filter-shaped test double by the chain runner tests
/// (`pass_through_chain_equals_root_only_chain`, `view::chain::tests`).
#[cfg(test)]
pub(crate) struct PassThrough;

#[cfg(test)]
impl Projection for PassThrough {
    fn project(
        &self,
        ctx: &ProjectionCtx<'_>,
        next: Next<'_>,
    ) -> Result<Vec<Row>, ProjectionError> {
        next.project(ctx)
    }

    fn resolve(&self, _ctx: &ProjectionCtx<'_>, _path: &RowPath) -> Result<Vec<Row>, Unresolved> {
        Err(Unresolved)
    }

    fn apply(
        &self,
        ctx: &ProjectionCtx<'_>,
        path: &RowPath,
        op: RowOp,
        next: Next<'_>,
    ) -> Result<Vec<okf::Op>, Unsupported> {
        next.apply(ctx, path, op)
    }

    fn surface(&self, ctx: &ProjectionCtx<'_>, next: Next<'_>) -> SurfaceId {
        next.surface(ctx)
    }
}
