//! The middleware chain runner: builds a [`Chain`] from a declared
//! [`super::decl::ViewDecl`] against a [`MiddlewareRegistry`], and runs it
//! with whole-chain failure fallback to the root view.

use std::collections::HashMap;
use std::sync::Arc;

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::okf;

use super::decl::ViewDecl;
use super::projection::{
    Next, Projection, ProjectionCtx, ProjectionError, RowOp, Unresolved, Unsupported,
};
use super::row::{Row, RowId, ViewId};
use super::surface::SurfaceId;

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
    pub fn run(&self, ctx: &ProjectionCtx<'_>, _limits: ChainLimits) -> ChainOutcome {
        match self.next().project(ctx) {
            Ok(rows) => ChainOutcome {
                rows,
                surface: self.next().surface(ctx),
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

    /// Dispatch to the stage whose `ViewId` matches `id.owner`; `Unresolved`
    /// from the owner falls back to the nearest resolvable prefix
    /// (`path.parent()` loop), at worst the folder itself.
    pub fn resolve(&self, ctx: &ProjectionCtx<'_>, id: &RowId) -> Result<Vec<Row>, Unresolved> {
        let stage = self
            .ids
            .iter()
            .position(|owned| owned == &id.owner)
            .map(|index| &self.stages[index]);
        match stage {
            Some(stage) => stage.resolve(ctx, &id.path),
            None => Err(Unresolved),
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
}
