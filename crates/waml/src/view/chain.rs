//! The middleware chain runner: builds a [`Chain`] from a declared
//! [`super::decl::ViewDecl`] against a [`MiddlewareRegistry`], and runs it
//! with whole-chain failure fallback to the root view.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::okf;

use super::decl::ViewDecl;
use super::projection::{
    Next, Projection, ProjectionCtx, ProjectionError, RowOp, Unresolved, Unsupported,
};
use super::row::{Row, RowId, RowPath, RowTarget, ViewId};
use super::surface::SurfaceId;

/// Which bound the runner's descent guard tripped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trip {
    DepthExceeded,
    Cycle,
}

/// Runner-owned descent bookkeeping. Lives behind an `Rc<RefCell<_>>` shared
/// by every `Chain` handed out along one descent path (via the wrapped
/// `descend` closure installed by [`Chain::run`]), so a stage cannot reset it
/// by constructing its own `ProjectionCtx` or calling `run` again — the
/// state, and the `max_depth` it was seeded with, travel with the chain, not
/// with whatever `ChainLimits` a later `run` call happens to be passed.
#[derive(Debug)]
struct DescentState {
    depth: usize,
    visited: HashSet<String>,
    max_depth: usize,
}

/// The reserved owner of a depth-cap/cycle diagnostic row. Never produced by
/// registered middleware -- reserved by the runner.
const DEPTH_GUARD_OWNER: &str = "view-depth-guard";

/// A name -> stage-factory map. Populated by the host (Task E1's
/// `CoreExtension`, later others); the chain looks names up the same way
/// regardless of who populated it.
#[derive(Default)]
pub struct MiddlewareRegistry {
    factories: HashMap<String, Arc<dyn Fn() -> Box<dyn Projection> + Send + Sync>>,
}

impl MiddlewareRegistry {
    pub fn new() -> MiddlewareRegistry {
        MiddlewareRegistry::default()
    }

    /// Register a middleware under `name`. A later registration for the same
    /// name replaces the earlier one.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn() -> Box<dyn Projection> + Send + Sync + 'static,
    ) {
        self.factories.insert(name.into(), Arc::new(factory));
    }

    fn build(&self, name: &str) -> Option<Box<dyn Projection>> {
        self.factories.get(name).map(|factory| factory())
    }
}

/// Split a `view:` entry's raw text into its middleware name and the rest
/// (params), on the first `:`. `"hide-refs"` -> `("hide-refs", None)`;
/// `"member:./orders"` -> `("member", Some("./orders"))`.
fn entry_name(raw: &str) -> &str {
    raw.split_once(':').map_or(raw, |(name, _rest)| name)
}

/// A resolved sequence of [`Projection`] stages for one folder, built from a
/// declared [`ViewDecl`]. Cheap to clone: both arrays are `Arc`-backed.
#[derive(Clone, Default)]
pub struct Chain {
    ids: Arc<[ViewId]>,
    stages: Arc<[Box<dyn Projection>]>,
    /// Set only on a chain returned by a tripped descent guard: `run` then
    /// short-circuits to a single diagnostic row instead of running stages.
    tripped: Option<Arc<(Trip, String)>>,
    /// Shared descent state, installed on a chain handed back by a wrapped
    /// `descend` closure so a recursive `run` continues counting instead of
    /// starting over at depth zero. `None` means "top-level" -- `run` seeds
    /// a fresh state from the `ChainLimits` it is given.
    descent: Option<Rc<RefCell<DescentState>>>,
}

/// Outcome of running a [`Chain`].
pub struct ChainOutcome {
    pub rows: Vec<Row>,
    /// The folder's own surface, chain-resolved.
    pub surface: SurfaceId,
    /// Non-empty when the declared chain failed and the root view rendered
    /// instead (whole-chain granularity), or when bounds tripped.
    pub diagnostics: Vec<Diagnostic>,
}

impl Chain {
    /// Build from a `ViewDecl` against a middleware registry. An unknown
    /// name is a declaration-level failure: returns the root-view-only
    /// chain plus a diagnostic spanned on the name in `view:`.
    pub fn build(
        decl: &ViewDecl,
        registry: &MiddlewareRegistry,
        index: &okf::Index,
    ) -> (Chain, Vec<Diagnostic>) {
        let file = format!("{}/index.md", index.directory.as_str());
        let names: Vec<&str> = decl.entries.iter().map(|e| entry_name(&e.raw)).collect();
        let disambiguated = ViewId::disambiguate(names.iter().copied());

        let mut ids = Vec::with_capacity(decl.entries.len());
        let mut stages: Vec<Box<dyn Projection>> = Vec::with_capacity(decl.entries.len());
        for (entry, (name, view_id)) in decl.entries.iter().zip(names.iter().zip(disambiguated)) {
            match registry.build(name) {
                Some(stage) => {
                    ids.push(view_id);
                    stages.push(stage);
                }
                None => {
                    let diagnostic = Diagnostic::new(
                        DiagCode::UnknownViewMiddleware,
                        format!("unknown view middleware `{name}`"),
                        file.clone(),
                        entry.line,
                    );
                    return (Chain::root_only(registry), vec![diagnostic]);
                }
            }
        }

        (
            Chain {
                ids: ids.into(),
                stages: stages.into(),
                tripped: None,
                descent: None,
            },
            Vec::new(),
        )
    }

    /// The zero-stage chain -- a folder with no `view:` declaration (or one
    /// that failed to build). `registry` is accepted for symmetry with
    /// [`Chain::build`] and future terminal-stage lookups; the empty chain
    /// itself does not consult it.
    pub fn root_only(_registry: &MiddlewareRegistry) -> Chain {
        Chain::default()
    }

    fn next(&self) -> Next<'_> {
        Next {
            remaining: &self.stages,
        }
    }

    /// Run. A stage returning `Err` discards ALL stage output and re-runs
    /// the root view alone (the same object -- the fallback path IS the
    /// default path), attaching a document-level diagnostic.
    ///
    /// Enforces the descent depth cap and the visited-directory cycle guard
    /// around every `ctx.descend` call a stage makes while projecting: see
    /// [`DescentState`]. A chain returned by a tripped guard (`self.tripped`)
    /// short-circuits here to a single diagnostic row -- it never reaches a
    /// stage's `project`.
    pub fn run(&self, ctx: &ProjectionCtx<'_>, limits: ChainLimits) -> ChainOutcome {
        if let Some(trip) = &self.tripped {
            return Chain::tripped_outcome(trip, ctx);
        }

        let state = self.descent.clone().unwrap_or_else(|| {
            Rc::new(RefCell::new(DescentState {
                depth: 0,
                visited: HashSet::from([ctx.dir.address.as_str().to_string()]),
                max_depth: limits.max_depth,
            }))
        });
        let wrapped_descend = guard_descend(ctx.descend, Rc::clone(&state));
        let guarded_ctx = ProjectionCtx {
            dir: ctx.dir,
            bundle: ctx.bundle,
            params: ctx.params,
            descend: &wrapped_descend,
        };

        match self.next().project(&guarded_ctx) {
            Ok(rows) => ChainOutcome {
                rows,
                surface: self.next().surface(&guarded_ctx),
                diagnostics: Vec::new(),
            },
            Err(ProjectionError { message }) => {
                let fallback = Chain::default();
                let rows = fallback.next().project(ctx).unwrap_or_default();
                let surface = fallback.next().surface(ctx);
                let diagnostic = Diagnostic::new(
                    DiagCode::ViewStageFailed,
                    format!("view chain failed, showing the root view instead: {message}"),
                    ctx.dir.address.as_str().to_string(),
                    0,
                );
                ChainOutcome {
                    rows,
                    surface,
                    diagnostics: vec![diagnostic],
                }
            }
        }
    }

    /// The chain returned by a tripped descent guard: no stages, just the
    /// trip marker. `run` on this never calls a stage.
    fn tripped_chain(trip: Trip, address: String) -> Chain {
        Chain {
            tripped: Some(Arc::new((trip, address))),
            ..Chain::default()
        }
    }

    /// One diagnostic row plus a document-level diagnostic, naming the
    /// folder the guard tripped on. Never a silent truncation.
    fn tripped_outcome(trip: &(Trip, String), ctx: &ProjectionCtx<'_>) -> ChainOutcome {
        let (kind, address) = trip;
        let (code, label, message) = match kind {
            Trip::DepthExceeded => (
                DiagCode::ViewDepthExceeded,
                format!("(view depth limit reached at {address})"),
                format!("view chain descent exceeded the depth cap at `{address}`"),
            ),
            Trip::Cycle => (
                DiagCode::ViewCycle,
                format!("(view chain cycle detected at {address})"),
                format!("view chain descent revisited `{address}`, stopping to avoid a cycle"),
            ),
        };
        let row = Row::new(
            RowId {
                owner: ViewId::new(DEPTH_GUARD_OWNER),
                path: RowPath::parse("limit").expect("literal row path is valid"),
            },
            label,
            RowTarget::Virtual,
            Some(SurfaceId("default".to_string())),
        )
        .expect("a Virtual row with an explicit surface always constructs");
        let diagnostic = Diagnostic::new(code, message, ctx.dir.address.as_str().to_string(), 0);
        ChainOutcome {
            rows: vec![row],
            surface: SurfaceId("default".to_string()),
            diagnostics: vec![diagnostic],
        }
    }

    /// Dispatch to the stage whose `ViewId` matches `id.owner` (the root
    /// view's reserved owner included -- it owns no `stages` slot, see
    /// [`super::projection::Next`]'s terminal fallback). `Unresolved` from
    /// the owner falls back to the nearest resolvable prefix (`path.parent()`
    /// loop, re-tried against the SAME owner), at worst the folder's own
    /// listing -- `Unresolved` is not a chain failure and never produces a
    /// diagnostic.
    pub fn resolve(&self, ctx: &ProjectionCtx<'_>, id: &RowId) -> Result<Vec<Row>, Unresolved> {
        let try_owner = |path: &RowPath| -> Result<Vec<Row>, Unresolved> {
            match self.ids.iter().position(|owned| owned == &id.owner) {
                Some(index) => self.stages[index].resolve(ctx, path),
                None if id.owner.as_str() == super::root::ROOT_VIEW_OWNER => {
                    super::root::RootView.resolve(ctx, path)
                }
                None => Err(Unresolved),
            }
        };

        if let Ok(rows) = try_owner(&id.path) {
            return Ok(rows);
        }
        let mut candidate = id.path.parent();
        while let Some(path) = candidate {
            if let Ok(rows) = try_owner(&path) {
                return Ok(rows);
            }
            candidate = path.parent();
        }

        // Worst case: no prefix of the path resolves at all -- fall back to
        // the folder's own listing rather than surfacing a chain failure.
        match self.next().project(ctx) {
            Ok(rows) => Ok(rows),
            Err(_) => Err(Unresolved),
        }
    }

    pub fn apply(
        &self,
        ctx: &ProjectionCtx<'_>,
        id: &RowId,
        op: RowOp,
    ) -> Result<Vec<okf::Op>, Unsupported> {
        let index = self.ids.iter().position(|owned| owned == &id.owner);
        match index {
            Some(index) => self.stages[index].apply(ctx, &id.path, op, self.next_from(index + 1)),
            None => Err(Unsupported),
        }
    }

    fn next_from(&self, index: usize) -> Next<'_> {
        Next {
            remaining: &self.stages[index.min(self.stages.len())..],
        }
    }
}

/// Wraps a host-supplied `descend` closure with the depth cap and
/// visited-directory cycle guard. Called every time a stage descends into a
/// child folder's chain while a `run` is in flight. On a trip, returns
/// [`Chain::tripped_chain`] instead of consulting the host closure at all --
/// the offending directory's real chain is never even built. Otherwise
/// advances `state` and hands the child chain the SAME shared state, so
/// further descent below it keeps counting from here rather than resetting.
fn guard_descend<'a>(
    original: &'a dyn Fn(&okf::Directory) -> Chain,
    state: Rc<RefCell<DescentState>>,
) -> impl Fn(&okf::Directory) -> Chain + 'a {
    move |dir: &okf::Directory| {
        let address = dir.address.as_str().to_string();
        let trip = {
            let state = state.borrow();
            if state.visited.contains(&address) {
                Some(Trip::Cycle)
            } else if state.depth + 1 > state.max_depth {
                Some(Trip::DepthExceeded)
            } else {
                None
            }
        };
        if let Some(trip) = trip {
            return Chain::tripped_chain(trip, address);
        }
        {
            let mut state = state.borrow_mut();
            state.depth += 1;
            state.visited.insert(address);
        }
        let mut chain = original(dir);
        chain.descent = Some(Rc::clone(&state));
        chain
    }
}

/// Runner bounds. Constructed by the HOST (editor from `.waml/settings.json`,
/// tests directly, LSP from its own config) and passed in. There is no
/// constructor that reads a bundle: bundle-supplied `max_view_depth` is
/// unreachable by construction, not by filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChainLimits {
    /// Maximum descent depth the runner will walk before giving up.
    pub max_depth: usize,
}

impl Default for ChainLimits {
    fn default() -> Self {
        ChainLimits { max_depth: 20 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontmatter::Frontmatter;
    use crate::view::decl::ViewEntry;
    use crate::view::projection::{PassThrough, Unresolved};
    use crate::view::row::{RowPath, RowTarget};

    fn dir() -> okf::Directory {
        okf::Directory {
            address: okf::DirectoryAddress::parse("/sales").unwrap(),
            parent: Some(okf::DirectoryAddress::parse("/").unwrap()),
            child_directories: Vec::new(),
            concepts: Vec::new(),
        }
    }

    fn index() -> okf::Index {
        okf::Index {
            directory: okf::DirectoryAddress::parse("/sales").unwrap(),
            title: None,
            description: None,
            members: Vec::new(),
            body: None,
            authored: true,
            profile: None,
            view: None,
            extra: Frontmatter::default(),
        }
    }

    fn ctx<'a>(
        dir: &'a okf::Directory,
        bundle: &'a okf::Bundle,
        params: &'a Frontmatter,
        descend: &'a dyn Fn(&okf::Directory) -> Chain,
    ) -> ProjectionCtx<'a> {
        ProjectionCtx {
            dir,
            bundle,
            params,
            descend,
        }
    }

    fn decl(names: &[&str]) -> ViewDecl {
        ViewDecl {
            entries: names
                .iter()
                .enumerate()
                .map(|(i, name)| ViewEntry {
                    raw: name.to_string(),
                    line: i + 1,
                })
                .collect(),
        }
    }

    /// Adds one row of its own ahead of whatever the rest of the chain
    /// produces. If the rest of the chain fails, its own row is discarded
    /// too -- the `?` on `next.project` propagates the failure before this
    /// stage gets to combine anything.
    struct AddingDouble;
    impl Projection for AddingDouble {
        fn project(
            &self,
            ctx: &ProjectionCtx<'_>,
            next: Next<'_>,
        ) -> Result<Vec<Row>, ProjectionError> {
            let own = Row::new(
                RowId {
                    owner: ViewId::new("adding"),
                    path: RowPath::parse("added").unwrap(),
                },
                "Added".to_string(),
                RowTarget::Concept("added.waml".to_string()),
                None,
            )
            .unwrap();
            let mut rows = vec![own];
            rows.extend(next.project(ctx)?);
            Ok(rows)
        }
        fn resolve(
            &self,
            _ctx: &ProjectionCtx<'_>,
            _path: &RowPath,
        ) -> Result<Vec<Row>, Unresolved> {
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

    struct FailingDouble;
    impl Projection for FailingDouble {
        fn project(
            &self,
            _ctx: &ProjectionCtx<'_>,
            _next: Next<'_>,
        ) -> Result<Vec<Row>, ProjectionError> {
            Err(ProjectionError {
                message: "boom".to_string(),
            })
        }
        fn resolve(
            &self,
            _ctx: &ProjectionCtx<'_>,
            _path: &RowPath,
        ) -> Result<Vec<Row>, Unresolved> {
            Err(Unresolved)
        }
        fn apply(
            &self,
            _ctx: &ProjectionCtx<'_>,
            _path: &RowPath,
            _op: RowOp,
            _next: Next<'_>,
        ) -> Result<Vec<okf::Op>, Unsupported> {
            Err(Unsupported)
        }
        fn surface(&self, _ctx: &ProjectionCtx<'_>, _next: Next<'_>) -> SurfaceId {
            SurfaceId("default".to_string())
        }
    }

    fn registry_with_doubles() -> MiddlewareRegistry {
        let mut registry = MiddlewareRegistry::new();
        registry.register("adding", || Box::new(AddingDouble));
        registry.register("failing", || Box::new(FailingDouble));
        registry.register("pass-through", || Box::new(PassThrough));
        registry.register("hide", || Box::new(PassThrough));
        registry
    }

    #[test]
    fn an_unknown_middleware_name_yields_root_chain_plus_spanned_diagnostic() {
        let registry = registry_with_doubles();
        let idx = index();
        let (chain, diagnostics) = Chain::build(&decl(&["nonexistent"]), &registry, &idx);

        assert!(chain.ids.is_empty(), "unknown name yields the root chain");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagCode::UnknownViewMiddleware);
        assert_eq!(diagnostics[0].file, "/sales/index.md");
        assert_eq!(diagnostics[0].line, 1);
    }

    #[test]
    fn a_failing_stage_discards_earlier_stages_output_and_yields_the_root_view() {
        let registry = registry_with_doubles();
        let idx = index();
        let (chain, build_diags) = Chain::build(&decl(&["adding", "failing"]), &registry, &idx);
        assert!(build_diags.is_empty());

        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let outcome = chain.run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );

        assert!(
            outcome.rows.is_empty(),
            "the plain root listing has no rows in this fixture -- no decoration survives"
        );
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].code, DiagCode::ViewStageFailed);
    }

    #[test]
    fn pass_through_chain_equals_root_only_chain() {
        let registry = registry_with_doubles();
        let idx = index();
        let (chain, build_diags) = Chain::build(&decl(&["pass-through"]), &registry, &idx);
        assert!(build_diags.is_empty());

        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let via_pass_through = chain.run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );
        let root_only = Chain::root_only(&registry).run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );

        assert_eq!(via_pass_through.rows.len(), root_only.rows.len());
        assert_eq!(via_pass_through.surface, root_only.surface);
    }

    #[test]
    fn row_id_is_stable_across_reprojection_with_unchanged_inputs() {
        let registry = registry_with_doubles();
        let idx = index();
        let (chain, _) = Chain::build(&decl(&["adding"]), &registry, &idx);

        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let first = chain.run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );
        let second = chain.run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );

        let ids_of = |outcome: &ChainOutcome| -> Vec<RowId> {
            outcome.rows.iter().map(|row| row.id.clone()).collect()
        };
        assert_eq!(ids_of(&first), ids_of(&second));
    }

    #[test]
    fn repeated_names_in_one_chain_get_stable_disambiguated_view_ids() {
        let registry = registry_with_doubles();
        let idx = index();
        let (chain, build_diags) = Chain::build(&decl(&["hide", "hide"]), &registry, &idx);
        assert!(build_diags.is_empty());
        assert_eq!(
            chain.ids.iter().map(ViewId::as_str).collect::<Vec<_>>(),
            vec!["hide", "hide#2"]
        );

        let (chain_again, _) = Chain::build(&decl(&["hide", "hide"]), &registry, &idx);
        assert_eq!(chain.ids, chain_again.ids);
    }

    #[test]
    fn chain_limits_default_is_twenty() {
        assert_eq!(ChainLimits::default().max_depth, 20);
    }

    #[test]
    fn bundle_frontmatter_max_view_depth_never_reaches_the_runner() {
        // ChainLimits has no constructor that reads a bundle or an Index's
        // frontmatter -- the only way to get one is `default()` or building
        // the struct literal directly. A bundle whose root and folder
        // indexes both declare `max_view_depth: 3` therefore cannot affect
        // this value: nothing ever reads that key on this path. This test
        // documents the invariant at the type level; the descent-depth
        // assertion against a live bundle is added once B6's runner exists.
        let limits = ChainLimits::default();
        assert_eq!(limits.max_depth, 20);
    }

    // --- Task B6: depth cap and cycle guard ------------------------------

    fn dir_at(address: &str) -> okf::Directory {
        okf::Directory {
            address: okf::DirectoryAddress::parse(address).unwrap(),
            parent: okf::DirectoryAddress::parse(address).unwrap().parent(),
            child_directories: Vec::new(),
            concepts: Vec::new(),
        }
    }

    /// A synthesized folder whose declared chain descends into itself: each
    /// `project` emits one row for a synthesized child (a distinct address
    /// per level, since a synthesized descent need not track a real one)
    /// with `expand` set to that child's chain -- true lazy descent, per
    /// [`Row::expand`]'s contract. Nothing here forces it; the CALLER (real
    /// usage: the editor, one click at a time; here: the test, standing in
    /// for repeated clicks) decides whether and when to `run` it.
    struct SelfDescendingDouble;
    impl Projection for SelfDescendingDouble {
        fn project(
            &self,
            ctx: &ProjectionCtx<'_>,
            _next: Next<'_>,
        ) -> Result<Vec<Row>, ProjectionError> {
            let child_dir = dir_at(&format!("{}/child", ctx.dir.address.as_str()));
            let expand = (ctx.descend)(&child_dir);
            let mut row = Row::new(
                RowId {
                    owner: ViewId::new("self-descend"),
                    path: RowPath::parse("child").expect("literal row path is valid"),
                },
                "Child".to_string(),
                RowTarget::Virtual,
                Some(SurfaceId("default".to_string())),
            )
            .expect("a Virtual row with an explicit surface always constructs");
            row.expand = Some(expand);
            Ok(vec![row])
        }
        fn resolve(
            &self,
            _ctx: &ProjectionCtx<'_>,
            _path: &RowPath,
        ) -> Result<Vec<Row>, Unresolved> {
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

    fn self_descending_chain() -> Chain {
        Chain {
            ids: vec![ViewId::new("self-descend")].into(),
            stages: vec![Box::new(SelfDescendingDouble) as Box<dyn Projection>].into(),
            tripped: None,
            descent: None,
        }
    }

    /// Walks a lazily self-descending chain by repeatedly `run`ning the sole
    /// row's `expand` chain, the way a UI would descend one click at a time,
    /// stopping when a row has no `expand` left (the guard tripped, or the
    /// stage stopped emitting one). Bounded so a broken guard fails the test
    /// instead of hanging it.
    fn walk_to_the_end(chain: Chain, root: okf::Directory, limits: ChainLimits) -> ChainOutcome {
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| self_descending_chain();

        let mut current_chain = chain;
        let mut current_dir = root;
        let mut outcome;
        let mut steps = 0;
        loop {
            steps += 1;
            assert!(
                steps <= 10,
                "runaway descent -- the guard failed to stop it"
            );
            let ctx = ctx(&current_dir, &bundle, &params, &descend);
            outcome = current_chain.run(&ctx, limits);
            let mut rows = std::mem::take(&mut outcome.rows);
            let row = rows.pop().expect("self-descend always emits one row");
            match row.expand {
                Some(next_chain) => {
                    current_dir = dir_at(&format!("{}/child", current_dir.address.as_str()));
                    current_chain = next_chain;
                }
                None => {
                    outcome.rows = vec![row];
                    return outcome;
                }
            }
        }
    }

    #[test]
    fn depth_cap_trips_at_the_configured_value() {
        let chain = self_descending_chain();
        let root = dir_at("/root");

        let outcome = walk_to_the_end(chain, root, ChainLimits { max_depth: 3 });

        assert_eq!(outcome.rows.len(), 1, "the descent terminates at the cap");
        assert_eq!(outcome.rows[0].id.owner, ViewId::new(DEPTH_GUARD_OWNER));
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].code, DiagCode::ViewDepthExceeded);
    }

    #[test]
    fn cycle_guard_trips_on_first_revisit() {
        // A's chain descends into B, B's chain descends back into A -- a
        // real two-directory cycle, not a synthesized one.
        let dir_a = dir_at("/a");
        let dir_b = dir_at("/b");

        struct CrossDescendingDouble {
            partner: okf::Directory,
        }
        impl Projection for CrossDescendingDouble {
            fn project(
                &self,
                ctx: &ProjectionCtx<'_>,
                _next: Next<'_>,
            ) -> Result<Vec<Row>, ProjectionError> {
                let expand = (ctx.descend)(&self.partner);
                let mut row = Row::new(
                    RowId {
                        owner: ViewId::new("cross-descend"),
                        path: RowPath::parse("partner").expect("literal row path is valid"),
                    },
                    "Partner".to_string(),
                    RowTarget::Virtual,
                    Some(SurfaceId("default".to_string())),
                )
                .expect("a Virtual row with an explicit surface always constructs");
                row.expand = Some(expand);
                Ok(vec![row])
            }
            fn resolve(
                &self,
                _ctx: &ProjectionCtx<'_>,
                _path: &RowPath,
            ) -> Result<Vec<Row>, Unresolved> {
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

        fn cross_chain(partner: okf::Directory) -> Chain {
            Chain {
                ids: vec![ViewId::new("cross-descend")].into(),
                stages: vec![Box::new(CrossDescendingDouble { partner }) as Box<dyn Projection>]
                    .into(),
                tripped: None,
                descent: None,
            }
        }

        let chain_a = cross_chain(dir_b.clone());
        let chain_b = cross_chain(dir_a.clone());
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |dir: &okf::Directory| {
            if dir.address == dir_b.address {
                chain_b.clone()
            } else {
                chain_a.clone()
            }
        };

        // Step 1: A, whose row expands into B.
        let ctx_a = ctx(&dir_a, &bundle, &params, &descend);
        let step1 = chain_a.run(&ctx_a, ChainLimits::default());
        assert!(step1.diagnostics.is_empty(), "no trip yet, depth 1 « cap");
        let row = step1.rows.into_iter().next().unwrap();
        let chain_at_b = row.expand.expect("A's row expands into B");

        // Step 2: B. Its row wants to expand back into A -- the wrapped
        // `descend` trips right here (first revisit of A) and hands B's
        // stage an already-tripped chain instead; no diagnostic yet, the
        // trip is still lazy on that row's `expand`.
        let ctx_b = ctx(&dir_b, &bundle, &params, &descend);
        let step2 = chain_at_b.run(&ctx_b, ChainLimits::default());
        assert!(
            step2.diagnostics.is_empty(),
            "the trip is lazy, not forced yet"
        );
        let row = step2.rows.into_iter().next().unwrap();
        let tripped_chain = row.expand.expect("B's row still carries an expand chain");

        // Step 3: running that tripped chain (the next click, back on A) is
        // where the guard actually surfaces -- at the first revisit, well
        // short of the default depth cap.
        let ctx_a_again = ctx(&dir_a, &bundle, &params, &descend);
        let step3 = tripped_chain.run(&ctx_a_again, ChainLimits::default());

        assert_eq!(step3.rows.len(), 1);
        assert_eq!(step3.rows[0].id.owner, ViewId::new(DEPTH_GUARD_OWNER));
        assert_eq!(step3.diagnostics.len(), 1);
        assert_eq!(step3.diagnostics[0].code, DiagCode::ViewCycle);
    }

    #[test]
    fn bundle_max_view_depth_does_not_change_the_trip_point() {
        // A bundle can declare `max_view_depth: 50` in its index frontmatter
        // (Task B4), but `ChainLimits` has no constructor that reads it --
        // the runner is built with whatever the HOST decided, here 3, and
        // the bundle's own claim is never consulted on this path.
        let mut idx = index();
        idx.extra.entries.push((
            "max_view_depth".to_string(),
            crate::frontmatter::FmValue::Num(50.0),
        ));
        assert!(idx.extra.get("max_view_depth").is_some());

        let chain = self_descending_chain();
        let root = dir_at("/root");

        let outcome = walk_to_the_end(chain, root, ChainLimits { max_depth: 3 });

        assert_eq!(outcome.rows.len(), 1);
        assert_eq!(outcome.diagnostics[0].code, DiagCode::ViewDepthExceeded);
    }

    // --- Task B8: resolve nearest-prefix fallback -------------------------

    /// Resolves only the exact path `"a"`; anything longer is `Unresolved`.
    /// Stands in for a folder row whose child no longer exists at the
    /// exact minted path, letting the fallback walk shorten it.
    struct PrefixDouble;
    impl Projection for PrefixDouble {
        fn project(
            &self,
            _ctx: &ProjectionCtx<'_>,
            _next: Next<'_>,
        ) -> Result<Vec<Row>, ProjectionError> {
            Ok(Vec::new())
        }
        fn resolve(
            &self,
            _ctx: &ProjectionCtx<'_>,
            path: &RowPath,
        ) -> Result<Vec<Row>, Unresolved> {
            if path.as_str() == "a" {
                Ok(vec![Row::new(
                    RowId {
                        owner: ViewId::new("prefix"),
                        path: RowPath::parse("a").unwrap(),
                    },
                    "A".to_string(),
                    RowTarget::Concept("a.waml".to_string()),
                    None,
                )
                .unwrap()])
            } else {
                Err(Unresolved)
            }
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

    fn prefix_chain() -> Chain {
        Chain {
            ids: vec![ViewId::new("prefix")].into(),
            stages: vec![Box::new(PrefixDouble) as Box<dyn Projection>].into(),
            tripped: None,
            descent: None,
        }
    }

    #[test]
    fn an_unresolvable_path_falls_back_to_its_nearest_resolvable_prefix() {
        let chain = prefix_chain();
        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let id = RowId {
            owner: ViewId::new("prefix"),
            path: RowPath::parse("a/b/c").unwrap(),
        };
        let rows = chain
            .resolve(&projection_ctx, &id)
            .expect("the parent prefix `a` resolves even though `a/b/c` doesn't");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "A");
    }

    #[test]
    fn a_deleted_everything_path_falls_back_to_the_folder_itself() {
        let chain = prefix_chain();
        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let id = RowId {
            owner: ViewId::new("prefix"),
            path: RowPath::parse("z/y/x").unwrap(),
        };
        let rows = chain
            .resolve(&projection_ctx, &id)
            .expect("no prefix resolves -- falls back to the folder's own listing, not an error");
        assert!(
            rows.is_empty(),
            "the folder's own listing in this fixture has no rows -- \
             empty is a valid worst-case fallback, not a failure"
        );
    }

    #[test]
    fn the_trip_is_a_diagnostic_row_not_a_missing_row() {
        let chain = self_descending_chain();
        let root = dir_at("/root");

        let outcome = walk_to_the_end(chain, root, ChainLimits { max_depth: 3 });

        let last = outcome.rows.last().expect("never a silently empty list");
        assert_eq!(last.target, RowTarget::Virtual);
        assert_eq!(last.id.owner, ViewId::new(DEPTH_GUARD_OWNER));
    }
}
