//! The root view: the plain OKF directory listing, in authored `index.md`
//! member order with unlisted members appended. This is the terminal stage
//! every chain ends at -- see [`super::projection::Next`]'s terminal
//! fallback, which reaches [`RootView`] whenever a chain runs out of
//! declared stages. A folder with no `view:` declaration therefore has a
//! one-element chain containing only this stage: today's behavior,
//! expressed in the new model rather than special-cased beside it.

use std::collections::HashSet;

use crate::okf;

use super::projection::{
    Next, Projection, ProjectionCtx, ProjectionError, RowOp, Unresolved, Unsupported,
};
use super::row::{Row, RowId, RowPath, RowTarget, ViewId};
use super::surface::SurfaceId;

/// This stage's reserved `ViewId` name. Never produced by a declared
/// `view:` entry -- see [`super::chain::DEPTH_GUARD_OWNER`] for the sibling
/// reserved name.
pub(crate) const ROOT_VIEW_OWNER: &str = "index";

fn folder_surface() -> SurfaceId {
    SurfaceId("folder".to_string())
}

/// The plain OKF listing. Never fails and never emits a `Virtual` row --
/// every row has a real file or real child directory behind it.
pub(crate) struct RootView;

impl RootView {
    fn row_for_member(ctx: &ProjectionCtx<'_>, member: &str) -> Option<Row> {
        if let Some(child) = ctx
            .dir
            .child_directories
            .iter()
            .find(|child| child.as_str() == member)
        {
            return Some(Self::folder_row(ctx, child));
        }
        if ctx.dir.concepts.iter().any(|concept| concept == member) {
            return Self::concept_row(ctx, member);
        }
        None
    }

    fn concept_row(ctx: &ProjectionCtx<'_>, concept_id: &str) -> Option<Row> {
        let concept = ctx.bundle.concept(concept_id)?;
        let label = concept.title.clone().unwrap_or_else(|| {
            concept_id
                .rsplit('/')
                .next()
                .filter(|last| !last.is_empty())
                .unwrap_or(concept_id)
                .to_string()
        });
        let mut row = Row::new(
            RowId {
                owner: ViewId::new(ROOT_VIEW_OWNER),
                path: RowPath::parse(concept_id)
                    .expect("a bundle concept id is a valid, non-empty RowPath"),
            },
            label,
            RowTarget::Concept(concept_id.to_string()),
            None,
        )
        .expect("a Concept target never requires a surface override");
        row.blurb = concept.description.clone();
        Some(row)
    }

    fn folder_row(ctx: &ProjectionCtx<'_>, address: &okf::DirectoryAddress) -> Row {
        let child_index = ctx.bundle.index(address.as_str());
        let label = child_index
            .and_then(|index| index.title.clone())
            .unwrap_or_else(|| {
                address
                    .as_str()
                    .rsplit('/')
                    .next()
                    .filter(|last| !last.is_empty())
                    .unwrap_or(address.as_str())
                    .to_string()
            });
        let mut row = Row::new(
            RowId {
                owner: ViewId::new(ROOT_VIEW_OWNER),
                path: RowPath::parse(address.as_str().trim_start_matches('/'))
                    .expect("a bundle directory address is a valid, non-empty RowPath"),
            },
            label,
            RowTarget::Folder(address.as_str().to_string()),
            None,
        )
        .expect("a Folder target never requires a surface override");
        // Honor the child: the default descent policy is to let the caller
        // (a click, a test) run the child's own declared chain lazily.
        if let Some(child_dir) = ctx.bundle.directory(address.as_str()) {
            row.expand = Some((ctx.descend)(child_dir));
        }
        row
    }
}

impl Projection for RootView {
    fn project(
        &self,
        ctx: &ProjectionCtx<'_>,
        _next: Next<'_>,
    ) -> Result<Vec<Row>, ProjectionError> {
        let mut rows = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        if let Some(index) = ctx.bundle.index(ctx.dir.address.as_str()) {
            for member in &index.members {
                if let Some(row) = Self::row_for_member(ctx, member) {
                    seen.insert(member.clone());
                    rows.push(row);
                }
            }
        }
        for child in &ctx.dir.child_directories {
            if seen.insert(child.as_str().to_string()) {
                rows.push(Self::folder_row(ctx, child));
            }
        }
        for concept_id in &ctx.dir.concepts {
            if seen.insert(concept_id.clone()) {
                if let Some(row) = Self::concept_row(ctx, concept_id) {
                    rows.push(row);
                }
            }
        }
        Ok(rows)
    }

    /// Stub until Task B8 wires the mint/resolve invariant.
    fn resolve(&self, _ctx: &ProjectionCtx<'_>, _path: &RowPath) -> Result<Vec<Row>, Unresolved> {
        Err(Unresolved)
    }

    /// Stub until Task G2 lowers `RowOp`s to OKF ops.
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
        folder_surface()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::prepare_candidate;
    use crate::source::SourceBundle;
    use crate::view::chain::{Chain, ChainLimits, MiddlewareRegistry};

    /// A fixture whose authored member order deliberately differs from
    /// filename order, plus one unlisted file (`zzz-unlisted.md`, authored
    /// nowhere in the index body).
    fn fixture() -> (okf::Bundle, okf::DirectoryAddress) {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\n* [Zebra](./zebra.md)\n* [Archive](archive/)\n* [Apple](./apple.md)\n",
            ),
            (
                "apple.md",
                "---\ntype: uml.Class\ntitle: Apple\ndescription: A fruit\n---\n# Apple\n",
            ),
            (
                "zebra.md",
                "---\ntype: uml.Class\ntitle: Zebra\n---\n# Zebra\n",
            ),
            ("archive/index.md", "---\ntitle: Archive\n---\n# Archive\n"),
            (
                "zzz-unlisted.md",
                "---\ntype: uml.Class\ntitle: Unlisted\n---\n# Unlisted\n",
            ),
        ])
        .unwrap();
        let prepared = prepare_candidate(source, None, 1).unwrap();
        let (_, okf, _uml, _) = prepared.into_parts();
        (okf.bundle, okf::DirectoryAddress::parse("/").unwrap())
    }

    fn ctx<'a>(
        dir: &'a okf::Directory,
        bundle: &'a okf::Bundle,
        params: &'a crate::frontmatter::Frontmatter,
        descend: &'a dyn Fn(&okf::Directory) -> Chain,
    ) -> ProjectionCtx<'a> {
        ProjectionCtx {
            dir,
            bundle,
            params,
            descend,
        }
    }

    #[test]
    fn identity_chain_reproduces_the_plain_okf_listing_row_for_row() {
        let (bundle, root_address) = fixture();
        let directory = bundle.directory(root_address.as_str()).unwrap().clone();
        let params = crate::frontmatter::Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let registry = MiddlewareRegistry::new();
        let chain = Chain::root_only(&registry);
        let outcome = chain.run(&projection_ctx, ChainLimits::default());

        let index = bundle.index(root_address.as_str()).unwrap();
        let mut expected_order: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for member in &index.members {
            let is_child = directory
                .child_directories
                .iter()
                .any(|child| child.as_str() == member);
            let is_concept = directory.concepts.iter().any(|concept| concept == member);
            if (is_child || is_concept) && seen.insert(member.clone()) {
                expected_order.push(member.trim_start_matches('/').to_string());
            }
        }
        for child in &directory.child_directories {
            if seen.insert(child.as_str().to_string()) {
                expected_order.push(child.as_str().trim_start_matches('/').to_string());
            }
        }
        for concept_id in &directory.concepts {
            if seen.insert(concept_id.clone()) {
                expected_order.push(concept_id.clone());
            }
        }

        let actual: Vec<String> = outcome
            .rows
            .iter()
            .map(|row| row.id.path.as_str().to_string())
            .collect();
        assert_eq!(actual, expected_order);
        // The authored order deliberately differs from filename order --
        // proves the assertion above is exercising real reordering, not a
        // coincidence of iteration order.
        assert_eq!(expected_order[0], "zebra");
    }

    #[test]
    fn a_folder_with_no_view_declaration_gets_the_root_only_chain() {
        let (bundle, root_address) = fixture();
        let directory = bundle.directory(root_address.as_str()).unwrap().clone();
        let params = crate::frontmatter::Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let outcome = Chain::default().run(&projection_ctx, ChainLimits::default());
        assert_eq!(outcome.rows.len(), 4, "3 members + 1 unlisted file");
    }

    #[test]
    fn every_root_view_row_has_a_real_target() {
        let (bundle, root_address) = fixture();
        let directory = bundle.directory(root_address.as_str()).unwrap().clone();
        let params = crate::frontmatter::Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let outcome = Chain::default().run(&projection_ctx, ChainLimits::default());
        assert!(!outcome.rows.is_empty());
        for row in &outcome.rows {
            assert!(
                !matches!(row.target, RowTarget::Virtual),
                "root view row `{}` must own a real target",
                row.id.path
            );
        }
    }

    #[test]
    fn child_folder_rows_expand_with_the_childs_own_chain() {
        let (bundle, root_address) = fixture();
        let directory = bundle.directory(root_address.as_str()).unwrap().clone();
        let params = crate::frontmatter::Frontmatter::default();

        // The child (`archive`) has its own chain -- descend hands back that
        // object (honor-the-child), never the parent's own chain.
        let descend = |dir: &okf::Directory| {
            assert_eq!(dir.address.as_str(), "/archive");
            Chain::default()
        };
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let outcome = Chain::default().run(&projection_ctx, ChainLimits::default());
        let archive_row = outcome
            .rows
            .iter()
            .find(|row| row.id.path.as_str() == "archive")
            .expect("archive is a declared member");
        assert_eq!(
            archive_row.target,
            RowTarget::Folder("/archive".to_string())
        );
        assert!(
            archive_row.expand.is_some(),
            "a folder row's expand chain is the child's own -- honor the child"
        );
    }

    #[test]
    fn root_view_surface_is_the_folder_listing() {
        let (bundle, root_address) = fixture();
        let directory = bundle.directory(root_address.as_str()).unwrap().clone();
        let params = crate::frontmatter::Frontmatter::default();
        let descend = |_: &okf::Directory| Chain::default();
        let projection_ctx = ctx(&directory, &bundle, &params, &descend);

        let outcome = Chain::default().run(&projection_ctx, ChainLimits::default());
        assert_eq!(outcome.surface, folder_surface());
    }
}
