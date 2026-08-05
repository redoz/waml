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

/// A middleware name registered by more than one extension. One flat name
/// table across all extensions -- a collision is a build-time programming
/// error, not something a document author can trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateMiddlewareName(pub String);

impl std::fmt::Display for DuplicateMiddlewareName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "duplicate middleware name `{}`", self.0)
    }
}

impl std::error::Error for DuplicateMiddlewareName {}

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

    /// Build a registry from every `CoreExtension`'s declared middleware.
    /// One flat name table: the same name declared by two extensions is a
    /// build error rather than a silent last-write-wins.
    pub fn from_extensions(
        extensions: &[&dyn crate::extension::CoreExtension],
    ) -> Result<MiddlewareRegistry, DuplicateMiddlewareName> {
        let mut registry = MiddlewareRegistry::new();
        for extension in extensions {
            for (name, factory) in extension.middleware() {
                if registry.factories.contains_key(name) {
                    return Err(DuplicateMiddlewareName(name.to_string()));
                }
                registry.factories.insert(name.to_string(), factory);
            }
        }
        Ok(registry)
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

/// The text after the first `:` in a `view:` entry, if any -- `member`'s
/// href, verbatim. `None` for a bare name (`"markdown"`) or a name with an
/// empty rest (`"member:"`).
fn entry_rest(raw: &str) -> Option<&str> {
    raw.split_once(':').map(|(_name, rest)| rest)
}

/// Task E3: a resolution the chain attaches to its own surface rather than
/// projecting rows through -- "the chain resolves surfaces too", not a
/// middleware stage. Recognized by [`Chain::build`], applied by [`Chain::run`]
/// after the ordinary stage walk computes its row-projection surface; the row
/// projection itself is unaffected either way.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Resolution {
    /// `view: markdown` -- the folder's own tab renders the generic markdown
    /// surface over `index.md`, rows unchanged.
    Markdown,
    /// `view: member:<href>` -- the folder's own tab resolves to the named
    /// member's target, at that member's own resolved surface.
    Member(String),
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
    /// Task E3's `markdown`/`member:<href>` surface resolution, if the
    /// declared chain named one. `None` for every chain built before Task E3
    /// and for a chain with neither entry -- ordinary stage-walk surface
    /// resolution applies unchanged.
    resolution: Option<Resolution>,
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
        let mut resolution: Option<Resolution> = None;
        let mut diagnostics = Vec::new();
        for (entry, (name, view_id)) in decl.entries.iter().zip(names.iter().zip(disambiguated)) {
            match *name {
                // Not middleware -- a resolution outcome attached to the
                // chain (spec: "the chain resolves surfaces too"). Row
                // projection beneath is unchanged: skip straight to the
                // next entry rather than looking this name up as a stage.
                "markdown" => {
                    resolution = Some(Resolution::Markdown);
                    continue;
                }
                "member" => {
                    match entry_rest(&entry.raw) {
                        Some(href) if !href.is_empty() => {
                            resolution = Some(Resolution::Member(href.to_string()));
                        }
                        _ => diagnostics.push(Diagnostic::new(
                            DiagCode::InvalidViewParams,
                            "`member:` requires a non-empty href".to_string(),
                            file.clone(),
                            entry.line,
                        )),
                    }
                    continue;
                }
                // `hide`'s params live in the folder's index frontmatter
                // (`ctx.params` at project time, `index.extra` here at
                // declaration time -- the same data, see
                // `Chain::resolve_member_surface`'s own `index.extra`
                // read). Checked at declaration time, not deferred to
                // `project`, so a missing/malformed `hide:` degrades the
                // whole chain up front with a diagnostic that names the
                // `view:` entry, exactly like an unknown middleware name.
                "hide" => {
                    if let Err(message) = super::hide::parse_hide_globs(&index.extra) {
                        let diagnostic =
                            super::hide::invalid_params_diagnostic(message, &file, entry.line);
                        return (Chain::root_only(registry), vec![diagnostic]);
                    }
                }
                _ => {}
            }
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
                resolution,
            },
            diagnostics,
        )
    }

    /// The zero-stage chain -- a folder with no `view:` declaration (or one
    /// that failed to build). `registry` is accepted for symmetry with
    /// [`Chain::build`] and future terminal-stage lookups; the empty chain
    /// itself does not consult it.
    pub fn root_only(_registry: &MiddlewareRegistry) -> Chain {
        Chain::default()
    }

    /// The raw OKF layer (spec: "The raw OKF layer"): pins the chain to the
    /// identity listing, bypassing every declared `view:` stage regardless of
    /// what the folder declares. Row-for-row identical to [`Chain::root_only`]
    /// today -- both are the zero-stage chain -- but named and constructed
    /// separately because the two mean different things: `root_only` is what
    /// a folder gets when it declares nothing, `raw` is what a caller asks
    /// for explicitly to see past a declared chain. Hidden rows are reachable
    /// only through this route, never leaked into the declared listing --
    /// this is presentational, not a permission boundary, and performs no
    /// access check.
    pub fn raw() -> Chain {
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
            Ok(rows) => {
                let stage_surface = self.next().surface(&guarded_ctx);
                let (surface, diagnostics) = match &self.resolution {
                    None => (stage_surface, Vec::new()),
                    Some(Resolution::Markdown) => (SurfaceId("markdown".to_string()), Vec::new()),
                    Some(Resolution::Member(href)) => {
                        match Self::resolve_member_surface(&guarded_ctx, href, &rows, limits) {
                            Some(surface) => (surface, Vec::new()),
                            None => {
                                let diagnostic = Diagnostic::new(
                                    DiagCode::InvalidViewParams,
                                    format!(
                                        "`member:{href}` does not resolve to a member of this folder"
                                    ),
                                    ctx.dir.address.as_str().to_string(),
                                    0,
                                );
                                (stage_surface, vec![diagnostic])
                            }
                        }
                    }
                };
                ChainOutcome {
                    rows,
                    surface,
                    diagnostics,
                }
            }
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

    /// `member:<href>` (Task E3): resolve `href` against `ctx.dir` to the
    /// member it names, then that member's own resolved surface -- a
    /// concept's type default, or a nested folder's own declared chain.
    ///
    /// For a folder target, prefers the SAME `Chain` this run's own row
    /// projection already resolved for that row (`rows`, via its `expand`)
    /// over calling `ctx.descend` again: `RootView::folder_row` already
    /// called `descend` once per listed folder row to stash it there, and
    /// the depth/cycle guard counts every `descend` call regardless of
    /// whether the caller runs the result -- a second call for a folder this
    /// same pass already touched would trip the guard as a false-positive
    /// "cycle", not a real one. Only a target that is not one of this
    /// folder's own listed rows (e.g. filtered out by `hide`) falls back to
    /// calling `ctx.descend` directly, still guarded against a genuine
    /// `member:` cycle between two folders the way any `descend`-based cycle
    /// is. `None` means the href does not name a real member of this folder
    /// -- the caller degrades to the row-projection surface with a
    /// diagnostic.
    fn resolve_member_surface(
        ctx: &ProjectionCtx<'_>,
        href: &str,
        rows: &[Row],
        limits: ChainLimits,
    ) -> Option<SurfaceId> {
        let target = Self::resolve_member_target(ctx, href)?;
        match &target {
            RowTarget::Concept(_) => Some(super::surface::default_surface(&target, ctx.bundle)),
            RowTarget::Folder(address) => {
                if let Some(row) = rows.iter().find(|row| row.target == target) {
                    let chain = row.expand.clone().unwrap_or_default();
                    let child_dir = ctx.bundle.directory(address.as_str())?.clone();
                    let extra = ctx
                        .bundle
                        .index(address.as_str())
                        .map(|index| index.extra.clone())
                        .unwrap_or_default();
                    let child_ctx = ProjectionCtx {
                        dir: &child_dir,
                        bundle: ctx.bundle,
                        params: &extra,
                        descend: ctx.descend,
                    };
                    return Some(chain.run(&child_ctx, limits).surface);
                }
                let child_dir = ctx.bundle.directory(address.as_str())?.clone();
                let child_chain = (ctx.descend)(&child_dir);
                let extra = ctx
                    .bundle
                    .index(address.as_str())
                    .map(|index| index.extra.clone())
                    .unwrap_or_default();
                let child_ctx = ProjectionCtx {
                    dir: &child_dir,
                    bundle: ctx.bundle,
                    params: &extra,
                    descend: ctx.descend,
                };
                Some(child_chain.run(&child_ctx, limits).surface)
            }
            RowTarget::Virtual => None,
        }
    }

    /// `href` (`./orders`, `sales/`) resolved against `ctx.dir`'s own
    /// `index.md` to a real concept or child directory of this folder --
    /// [`okf::resolve_href`]'s bundle-relative-id resolution, then matched
    /// against `ctx.dir`'s own member lists the same way
    /// [`super::root::RootView::row_for_member`] does. Never a `Virtual`
    /// target: nothing in this folder resolves to one.
    fn resolve_member_target(ctx: &ProjectionCtx<'_>, href: &str) -> Option<RowTarget> {
        let dir_path = ctx.dir.address.as_str().trim_start_matches('/');
        let referring = if dir_path.is_empty() {
            "index.md".to_string()
        } else {
            format!("{dir_path}/index.md")
        };
        let resolved = okf::resolve_href(&referring, href);
        if ctx.dir.concepts.iter().any(|concept| concept == &resolved) {
            return Some(RowTarget::Concept(resolved));
        }
        let dir_address = format!("/{resolved}");
        if ctx
            .dir
            .child_directories
            .iter()
            .any(|child| child.as_str() == dir_address)
        {
            return Some(RowTarget::Folder(dir_address));
        }
        None
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

    /// Dispatch mirrors `resolve`: a declared stage's `ViewId` routes to
    /// that stage (with the rest of the chain as its `next`); the root
    /// view's reserved owner -- unregistered in `self.ids`, since it owns
    /// no `stages` slot -- routes to [`super::root::RootView`] directly,
    /// terminal `next`. Any other unmatched owner (a stale `RowId` from a
    /// chain that has since changed shape) is `Unsupported`, not a panic.
    pub fn apply(
        &self,
        ctx: &ProjectionCtx<'_>,
        id: &RowId,
        op: RowOp,
    ) -> Result<Vec<okf::Op>, Unsupported> {
        let index = self.ids.iter().position(|owned| owned == &id.owner);
        match index {
            Some(index) => self.stages[index].apply(ctx, &id.path, op, self.next_from(index + 1)),
            None if id.owner.as_str() == super::root::ROOT_VIEW_OWNER => {
                super::root::RootView.apply(ctx, &id.path, op, Next { remaining: &[] })
            }
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
    fn chain_raw_equals_the_identity_listing() {
        let registry = registry_with_doubles();
        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();

        let raw = Chain::raw().run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );
        let root_only = Chain::root_only(&registry).run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );

        assert_eq!(raw.rows.len(), root_only.rows.len());
        assert_eq!(raw.surface, root_only.surface);
        assert!(raw.diagnostics.is_empty());
    }

    #[test]
    fn chain_raw_ignores_a_declared_chain() {
        let registry = registry_with_doubles();
        let idx = index();
        // A folder declaring `adding` gets an extra row ahead of the
        // identity listing when its declared chain runs.
        let (declared, build_diags) = Chain::build(&decl(&["adding"]), &registry, &idx);
        assert!(build_diags.is_empty());

        let directory = dir();
        let bundle = okf::Bundle::default();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();

        let via_declared = declared.run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );
        assert_eq!(
            via_declared.rows.len(),
            1,
            "the declared chain's `adding` stage contributes its own row"
        );

        let via_raw = Chain::raw().run(
            &ctx(&directory, &bundle, &params, &descend),
            ChainLimits::default(),
        );
        assert!(
            via_raw.rows.is_empty(),
            "raw bypasses the declared chain entirely -- the `adding` stage never runs"
        );
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
        // Uses "pass-through", not "hide" -- since Task F1, "hide" is a
        // reserved name `Chain::build` declaration-checks for a real
        // `hide:` param before ever reaching a registry lookup, so it no
        // longer works as a generic disambiguation-only test double.
        let registry = registry_with_doubles();
        let idx = index();
        let (chain, build_diags) =
            Chain::build(&decl(&["pass-through", "pass-through"]), &registry, &idx);
        assert!(build_diags.is_empty());
        assert_eq!(
            chain.ids.iter().map(ViewId::as_str).collect::<Vec<_>>(),
            vec!["pass-through", "pass-through#2"]
        );

        let (chain_again, _) =
            Chain::build(&decl(&["pass-through", "pass-through"]), &registry, &idx);
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
            ..Chain::default()
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
                ..Chain::default()
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
            ..Chain::default()
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

    // Task E3: `markdown` and `member:<href>` as chain surface resolutions.
    mod surface_resolutions {
        use super::*;
        use crate::okf::Bundle;
        use crate::source::SourceBundle;

        fn descend_for<'a>(bundle: &'a okf::Bundle) -> impl Fn(&okf::Directory) -> Chain + 'a {
            move |dir: &okf::Directory| {
                let registry = MiddlewareRegistry::new();
                bundle.resolved_view(dir.address.as_str(), &registry).0
            }
        }

        fn run_root(
            bundle: &okf::Bundle,
            registry: &MiddlewareRegistry,
        ) -> (Vec<Row>, SurfaceId, Vec<Diagnostic>) {
            let (chain, mut build_diags) = bundle.resolved_view("/", registry);
            let directory = bundle.directory("/").unwrap().clone();
            let params = bundle
                .index("/")
                .map(|i| i.extra.clone())
                .unwrap_or_default();
            let descend = descend_for(bundle);
            let projection_ctx = ctx(&directory, bundle, &params, &descend);
            let outcome = chain.run(&projection_ctx, ChainLimits::default());
            build_diags.extend(outcome.diagnostics);
            (outcome.rows, outcome.surface, build_diags)
        }

        #[test]
        fn view_markdown_resolves_the_folder_target_to_the_markdown_surface() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: markdown\n---\n# Root\n\n* [Orders](orders.md)\n",
                    ),
                    ("orders.md", "# Orders\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::new();

            let identity_bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    ("index.md", "# Root\n\n* [Orders](orders.md)\n"),
                    ("orders.md", "# Orders\n"),
                ])
                .unwrap(),
            )
            .unwrap();

            let (rows, surface, diagnostics) = run_root(&bundle, &registry);
            let (identity_rows, identity_surface, identity_diags) =
                run_root(&identity_bundle, &registry);

            assert!(diagnostics.is_empty());
            assert!(identity_diags.is_empty());
            assert_eq!(surface.as_str(), "markdown");
            assert_ne!(identity_surface.as_str(), "markdown");
            assert_eq!(
                rows.len(),
                identity_rows.len(),
                "rows unchanged vs identity chain"
            );
        }

        #[test]
        fn view_member_resolves_to_the_members_target_and_surface() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: member:./orders\n---\n# Root\n\n* [Orders](orders.md)\n* [Sales](sales/)\n",
                    ),
                    (
                        "orders.md",
                        "---\ntype: uml.Class\n---\n# Orders\n",
                    ),
                    ("sales/index.md", "# Sales\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::new();

            let (_, surface, diagnostics) = run_root(&bundle, &registry);
            assert!(diagnostics.is_empty());
            assert_eq!(
                surface.as_str(),
                "canvas",
                "orders is a uml.Class -- canvas default"
            );

            let folder_bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: member:./sales\n---\n# Root\n\n* [Sales](sales/)\n",
                    ),
                    ("sales/index.md", "# Sales\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let (_, folder_surface, folder_diags) = run_root(&folder_bundle, &registry);
            assert!(folder_diags.is_empty());
            assert_eq!(
                folder_surface.as_str(),
                "folder",
                "targeting a folder member yields that member's own resolved chain surface"
            );
        }

        #[test]
        fn member_with_a_missing_href_degrades_with_a_spanned_diagnostic() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: member\n---\n# Root\n\n* [Orders](orders.md)\n",
                    ),
                    ("orders.md", "# Orders\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::new();

            let (rows, surface, diagnostics) = run_root(&bundle, &registry);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, DiagCode::InvalidViewParams);
            assert_eq!(surface.as_str(), "folder", "degrades to default resolution");
            assert_eq!(rows.len(), 1);
        }

        #[test]
        fn a_member_href_that_does_not_resolve_degrades_with_a_diagnostic() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: member:./nonexistent\n---\n# Root\n\n* [Orders](orders.md)\n",
                    ),
                    ("orders.md", "# Orders\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::new();

            let (_, surface, diagnostics) = run_root(&bundle, &registry);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, DiagCode::InvalidViewParams);
            assert_eq!(surface.as_str(), "folder", "degrades to default resolution");
        }

        #[test]
        fn no_auto_detection_a_lone_diagram_does_not_change_the_folder_listing_surface() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    ("index.md", "# Root\n\n* [Order](order.md)\n"),
                    ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::new();

            let (_, surface, diagnostics) = run_root(&bundle, &registry);
            assert!(diagnostics.is_empty());
            assert_eq!(surface.as_str(), "folder");
        }
    }

    // Task F1: `hide` -- the first non-identity chain, end to end.
    mod hide_middleware {
        use super::*;
        use crate::extension::CoreExt;
        use crate::okf::Bundle;
        use crate::source::SourceBundle;

        fn descend_for<'a>(bundle: &'a okf::Bundle) -> impl Fn(&okf::Directory) -> Chain + 'a {
            move |dir: &okf::Directory| {
                let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();
                bundle.resolved_view(dir.address.as_str(), &registry).0
            }
        }

        fn run_root(
            bundle: &okf::Bundle,
            registry: &MiddlewareRegistry,
        ) -> (Vec<Row>, SurfaceId, Vec<Diagnostic>) {
            let (chain, mut build_diags) = bundle.resolved_view("/", registry);
            let directory = bundle.directory("/").unwrap().clone();
            let params = bundle
                .index("/")
                .map(|i| i.extra.clone())
                .unwrap_or_default();
            let descend = descend_for(bundle);
            let projection_ctx = ctx(&directory, bundle, &params, &descend);
            let outcome = chain.run(&projection_ctx, ChainLimits::default());
            build_diags.extend(outcome.diagnostics);
            (outcome.rows, outcome.surface, build_diags)
        }

        fn hidden_fixture() -> Bundle {
            Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n\
                         * [Orders](orders.md)\n* [References](references/)\n",
                    ),
                    ("orders.md", "# Orders\n"),
                    ("references/index.md", "# References\n"),
                    ("references/appendix.md", "# Appendix\n"),
                ])
                .unwrap(),
            )
            .unwrap()
        }

        fn identity_fixture() -> Bundle {
            Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "# Root\n\n* [Orders](orders.md)\n* [References](references/)\n",
                    ),
                    ("orders.md", "# Orders\n"),
                    ("references/index.md", "# References\n"),
                    ("references/appendix.md", "# Appendix\n"),
                ])
                .unwrap(),
            )
            .unwrap()
        }

        #[test]
        fn hide_drops_exactly_the_matching_rows_and_nothing_else() {
            let bundle = hidden_fixture();
            let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();
            let (rows, _surface, diagnostics) = run_root(&bundle, &registry);
            assert!(diagnostics.is_empty());

            let identity = identity_fixture();
            let (identity_rows, _, identity_diags) = run_root(&identity, &registry);
            assert!(identity_diags.is_empty());

            let ids: Vec<&str> = rows.iter().map(|r| r.id.path.as_str()).collect();
            assert_eq!(ids, vec!["orders"]);

            let identity_ids: Vec<&str> =
                identity_rows.iter().map(|r| r.id.path.as_str()).collect();
            assert_eq!(identity_ids, vec!["orders", "references"]);
        }

        #[test]
        fn hide_declines_surface_resolution() {
            let bundle = hidden_fixture();
            let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();
            let (_, surface, diagnostics) = run_root(&bundle, &registry);
            assert!(diagnostics.is_empty());

            let identity = identity_fixture();
            let (_, identity_surface, identity_diags) = run_root(&identity, &registry);
            assert!(identity_diags.is_empty());

            assert_eq!(
                surface, identity_surface,
                "hide declines -- next.surface(ctx)"
            );
        }

        #[test]
        fn hide_with_no_hide_param_is_a_declaration_failure() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([
                    (
                        "index.md",
                        "---\nview: hide\n---\n# Root\n\n* [Orders](orders.md)\n",
                    ),
                    ("orders.md", "# Orders\n"),
                ])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();
            let (rows, _surface, diagnostics) = run_root(&bundle, &registry);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, DiagCode::InvalidViewParams);
            assert_eq!(
                rows.len(),
                1,
                "declaration failure falls all the way back to the root view"
            );
        }

        #[test]
        fn hide_with_a_malformed_hide_param_is_a_declaration_failure() {
            let bundle = Bundle::parse(
                &SourceBundle::try_from_pairs([(
                    "index.md",
                    "---\nview: hide\nhide: not-a-list\n---\n# Root\n\n* [Orders](orders.md)\n",
                )])
                .unwrap(),
            )
            .unwrap();
            let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();
            let (_, _surface, diagnostics) = run_root(&bundle, &registry);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, DiagCode::InvalidViewParams);
        }

        #[test]
        fn hide_forwards_every_op_unchanged() {
            let params = Frontmatter {
                entries: vec![(
                    "hide".to_string(),
                    crate::frontmatter::FmValue::List(vec![crate::frontmatter::FmValue::Str(
                        "nothing/**".to_string(),
                    )]),
                )],
            };
            let directory = dir();
            let bundle = okf::Bundle::default();
            let descend = |_: &okf::Directory| Chain::default();
            let inner_ctx = ctx(&directory, &bundle, &params, &descend);

            let stage = super::super::super::hide::Hide;
            let ops = [
                RowOp::Rename {
                    title: "New title".to_string(),
                },
                RowOp::Delete,
                RowOp::Reorder { before: None },
                RowOp::InsertConcept {
                    after: None,
                    title: "New".to_string(),
                },
                RowOp::MoveOut,
            ];
            for op in ops {
                let path = RowPath::parse("orders").unwrap();
                // No inner stage is registered -- forwarding to `next` on an
                // empty remaining chain always yields `Unsupported`. What
                // this asserts is that `Hide::apply` itself adds no
                // transformation: the result matches calling `next.apply`
                // directly with the same op, for every `RowOp` variant.
                let via_hide = stage.apply(&inner_ctx, &path, op.clone(), Next { remaining: &[] });
                let direct = Next { remaining: &[] }.apply(&inner_ctx, &path, op);
                assert_eq!(via_hide, direct);
            }
        }

        #[test]
        fn a_hidden_path_does_not_resolve_through_the_chain() {
            let bundle = hidden_fixture();
            let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();
            let (chain, build_diags) = bundle.resolved_view("/", &registry);
            assert!(build_diags.is_empty());

            let directory = bundle.directory("/").unwrap().clone();
            let params = bundle.index("/").unwrap().extra.clone();
            let descend = descend_for(&bundle);
            let projection_ctx = ctx(&directory, &bundle, &params, &descend);

            // `hide`'s own `resolve` never forwards a hidden path -- it
            // always answers `Unresolved`. The runner's parent-prefix loop
            // then falls back to the folder's own listing (still under the
            // same, hide-filtered chain): the hidden row itself is never
            // resolved back, only the containing folder.
            let hidden_id = RowId {
                owner: ViewId::new("hide"),
                path: RowPath::parse("references").unwrap(),
            };
            let resolved = chain
                .resolve(&projection_ctx, &hidden_id)
                .expect("falls back to the folder's own (hide-filtered) listing");
            assert!(
                resolved
                    .iter()
                    .all(|row| row.target != RowTarget::Folder("/references".to_string())),
                "the hidden row itself must never come back out of resolve"
            );
        }

        #[test]
        fn the_raw_okf_layer_resolves_a_hidden_path() {
            let bundle = hidden_fixture();
            let raw = Chain::raw();

            let directory = bundle.directory("/").unwrap().clone();
            let params = bundle.index("/").unwrap().extra.clone();
            let descend = descend_for(&bundle);
            let projection_ctx = ctx(&directory, &bundle, &params, &descend);

            let hidden_id = RowId {
                owner: ViewId::new(super::super::super::root::ROOT_VIEW_OWNER),
                path: RowPath::parse("references").unwrap(),
            };
            let resolved = raw
                .resolve(&projection_ctx, &hidden_id)
                .expect("the raw OKF layer bypasses the declared hide chain");
            assert_eq!(resolved.len(), 1);
            assert_eq!(
                resolved[0].target,
                RowTarget::Folder("/references".to_string())
            );
        }

        /// B8's mint/resolve invariant (`assert_mint_resolve_roundtrip`),
        /// scoped to *surviving* rows: every path `hide`'s own declared
        /// chain mints must still resolve through that same declared chain
        /// on a later, freshly-parsed run.
        #[test]
        fn every_surviving_row_resolves_through_the_declared_chain_on_a_fresh_run() {
            let pairs = [
                (
                    "index.md",
                    "---\nview: hide\nhide: [\"references/**\"]\n---\n# Root\n\n\
                     * [Orders](orders.md)\n* [References](references/)\n",
                ),
                ("orders.md", "# Orders\n"),
                ("references/index.md", "# References\n"),
                ("references/appendix.md", "# Appendix\n"),
            ];
            let minted_from = Bundle::parse(&SourceBundle::try_from_pairs(pairs).unwrap()).unwrap();
            let fresh = Bundle::parse(&SourceBundle::try_from_pairs(pairs).unwrap()).unwrap();
            let registry = MiddlewareRegistry::from_extensions(&[&CoreExt]).unwrap();

            let (rows, _, diagnostics) = run_root(&minted_from, &registry);
            assert!(diagnostics.is_empty());
            assert_eq!(
                rows.len(),
                1,
                "references/** is hidden -- only orders survives"
            );

            let (chain, _) = fresh.resolved_view("/", &registry);
            let directory = fresh.directory("/").unwrap().clone();
            let params = fresh.index("/").unwrap().extra.clone();
            let descend = descend_for(&fresh);
            let fresh_ctx = ctx(&directory, &fresh, &params, &descend);

            for row in &rows {
                let resolved = chain
                    .resolve(&fresh_ctx, &row.id)
                    .unwrap_or_else(|_| panic!("surviving row `{}` must resolve", row.id.path));
                assert_eq!(resolved[0].target, row.target);
            }
        }
    }
}
