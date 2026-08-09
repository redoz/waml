//! `uml` middleware (Task 4): a pure decorator that stamps the `"box"` icon
//! on folder rows whose bundle-level `profile` is `uml-domain`, and leaves
//! every other row untouched.
//!
//! It mints nothing of its own: `resolve` always declines (`Unresolved`),
//! `occludes` keeps the default `false`, `apply`/`surface` forward to `next`
//! unconditionally, and no `RowId.owner` is ever rewritten -- rewriting it
//! would break persisted ids and misdirect `apply`.

use crate::okf;

use super::projection::{
    Next, Projection, ProjectionCtx, ProjectionError, RowOp, Unresolved, Unsupported,
};
use super::row::{IconId, Row, RowPath, RowTarget};
use super::surface::SurfaceId;

fn is_package(ctx: &ProjectionCtx<'_>, row: &Row) -> bool {
    match &row.target {
        RowTarget::Folder(addr) => {
            ctx.bundle
                .index(addr)
                .and_then(|index| index.profile.as_deref())
                == Some("uml-domain")
        }
        RowTarget::Concept(_) | RowTarget::Virtual => false,
    }
}

pub(crate) struct UmlView;

impl Projection for UmlView {
    fn project(
        &self,
        ctx: &ProjectionCtx<'_>,
        next: Next<'_>,
    ) -> Result<Vec<Row>, ProjectionError> {
        let mut rows = next.project(ctx)?;
        for row in &mut rows {
            if is_package(ctx, row) {
                row.icon = Some(IconId::new("box"));
            }
        }
        Ok(rows)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prepare_candidate;
    use crate::frontmatter::Frontmatter;
    use crate::source::SourceBundle;
    use crate::view::chain::{Chain, ChainLimits, MiddlewareRegistry};
    use crate::view::decl::{ViewDecl, ViewEntry};
    use crate::view::row::ViewId;

    /// A directory with one `uml-domain`-profiled child (`pkg`) and one
    /// plain child (`plain`).
    fn fixture() -> (okf::Bundle, okf::DirectoryAddress) {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Pkg](pkg/)\n* [Plain](plain/)\n"),
            ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
            ("plain/index.md", "# Plain\n"),
        ])
        .unwrap();
        let prepared = prepare_candidate(source, None, 1).unwrap();
        let (_, okf, _uml, _) = prepared.into_parts();
        (okf.bundle, okf::DirectoryAddress::parse("/").unwrap())
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

    fn projected_rows(bundle: &okf::Bundle, root: &okf::DirectoryAddress) -> Vec<Row> {
        let directory = bundle.directory(root.as_str()).unwrap().clone();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, bundle, &params, &descend);

        let mut registry = MiddlewareRegistry::new();
        registry.register("uml", || Box::new(UmlView));
        let index = bundle.index(root.as_str()).unwrap();
        let decl = ViewDecl {
            entries: vec![ViewEntry {
                raw: "uml".to_string(),
                line: 1,
            }],
        };
        let (chain, diagnostics) = Chain::build(
            &decl,
            &registry,
            index,
            &crate::view::mask::ProjectionMask::default(),
        );
        assert!(diagnostics.is_empty(), "uml chain builds cleanly");
        chain.run(&projection_ctx, ChainLimits::default()).rows
    }

    #[test]
    fn occludes_is_false_for_every_projected_path() {
        let (bundle, root) = fixture();
        let directory = bundle.directory(root.as_str()).unwrap().clone();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);
        let rows = projected_rows(&bundle, &root);
        for row in &rows {
            assert!(!UmlView.occludes(&projection_ctx, &row.id.path));
        }
    }

    #[test]
    fn resolve_always_declines() {
        let (bundle, root) = fixture();
        let directory = bundle.directory(root.as_str()).unwrap().clone();
        let params = Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);
        let path = RowPath::parse("pkg").unwrap();
        assert!(matches!(
            UmlView.resolve(&projection_ctx, &path),
            Err(Unresolved)
        ));
    }

    #[test]
    fn row_id_owner_is_untouched() {
        let (bundle, root) = fixture();
        let rows = projected_rows(&bundle, &root);
        for row in &rows {
            assert_eq!(
                row.id.owner,
                ViewId::new(super::super::root::ROOT_VIEW_OWNER)
            );
        }
    }
}
