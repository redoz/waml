//! The tree seam: flatten a `Model` into a `ProjectTree` for the panel.
//! Nothing here touches makepad; the `LiveId` bridge lives in `tree_panel.rs`.

use crate::document::{DocumentPresentation, NavCategory};
use crate::icons::Icon;
use waml::model::ElementType;

pub type TreeKind = NavCategory;

#[cfg(test)]
pub mod test_support {
    use super::*;

    /// A minimal leaf `TreeNode` whose `key_string` is `key`. Callers mutate
    /// the fields they care about.
    pub fn node(key: &str) -> TreeNode {
        TreeNode {
            key: waml::view::row::RowId {
                owner: waml::view::row::ViewId::new("test"),
                path: waml::view::row::RowPath::parse(key).unwrap(),
            },
            address: None,
            title: key.to_string(),
            kind: TreeKind::OkfDocument,
            presentation: DocumentPresentation {
                icon: crate::icons::Icon::Package,
                accent: None,
                category: NavCategory::OkfDocument,
            },
            is_directory: false,
            openable: false,
            concept_id: None,
            caps: waml::view::row::RowCaps::default(),
            child_caps: waml::view::row::ChildCaps::default(),
            view_degraded: false,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// The projected row's identity, NOT a file address. Stable across a
    /// re-projection, so selection and expansion survive the chain re-run a
    /// mask change triggers; a file address is not, because a middleware may
    /// relabel or mint rows with no file behind them.
    pub key: waml::view::row::RowId,
    /// Directory rows only: the real OKF address this row expands into.
    /// `None` for concept and virtual rows.
    pub address: Option<String>,
    pub title: String,
    pub kind: TreeKind,
    pub presentation: DocumentPresentation,
    pub is_directory: bool,
    pub openable: bool,
    pub concept_id: Option<String>,
    /// What the row's OWNING chain stage declares it will accept for this row
    /// (rename, delete, move out) and for the rows beneath it (reorder,
    /// insert, accept a move in).
    ///
    /// Advisory, for affordances only -- `Chain::apply` remains the authority.
    /// A middleware may under-declare and still accept an op; the invariant
    /// that matters is the converse, that a declared capability must not
    /// yield Unsupported.
    pub caps: waml::view::row::RowCaps,
    pub child_caps: waml::view::row::ChildCaps,
    /// Directory rows only: whether the projection that produced this row's
    /// children diagnosed anything -- the SAME diagnostics the folder tab
    /// shows when this folder is opened, from the SAME run. Drives the tree's
    /// degraded-chain marker so a folder inside a collapsed subtree is not
    /// silently wrong.
    ///
    /// Mask-dependent by construction: a fully-masked mask never builds the
    /// declared chain, so it never diagnoses one, and the marker is absent
    /// exactly where the folder tab would also show nothing. Always `false` for a non-directory
    /// row.
    pub view_degraded: bool,
    pub children: Vec<TreeNode>,
}

/// The bundle root's own `RowPath`. The root is not a projected row -- nothing
/// emits it -- so it mints its own id, and that id must not be one any chain
/// could also mint: a literal `"root"` segment is exactly what a top-level
/// `root.md` concept or a `/root` directory produces, and two rows sharing a
/// `key_string` share one `LiveId` and therefore one selection, fold, and
/// chevron state in the panel.
///
/// `\u{1}` is the one segment no real row carries: it is `key_string`'s own
/// separator, and no `ViewId` or `RowPath` segment in the bundle contains it
/// (see `key_string`).
pub const ROOT_ROW_SEGMENT: &str = "\u{1}";

/// The flat string the tree panel keys its `LiveId` maps and cached chevron
/// rects on. `\u{1}` separates the two halves: neither a `ViewId` nor a
/// `RowPath` segment produced by a projection can contain it, so distinct
/// `RowId`s never collide.
pub fn key_string(key: &waml::view::row::RowId) -> String {
    format!("{}\u{1}{}", key.owner, key.path)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectTree {
    pub roots: Vec<TreeNode>,
}

/// Map a resolved element type to the panel's kind, used to pick both the
/// row glyph (`tree_panel::draw_row_icon`) and click routing. Written without
/// a `_ =>` catch-all on the `Uml` arm so a new metaclass forces a decision
/// here. Interface/Enum/DataType get their own glyph-bearing variant (matching
/// `node_style::AccentBucket`'s granularity); Association/Actor/UseCase/
/// InstanceSpecification still fold into the plain `Class` glyph, and
/// `Sequence` behaviors get their own glyph while other behavior kinds share
/// the generic `Behavior` one.
pub fn kind_of(ty: &ElementType) -> TreeKind {
    TreeKind::from(waml::view::kind::kind_of(ty))
}

pub fn build_tree(
    okf: &waml::analysis::OkfAnalysis,
    uml_analysis: &waml::uml::Analysis,
    root_fallback: &str,
    mask: &waml::view::mask::ProjectionMask,
    limits: waml::view::chain::ChainLimits,
) -> ProjectTree {
    // One registry for the whole recursion: `FolderView::build` resolves
    // against the same name table, and minting it per directory put a fresh
    // allocation on a path that runs for every directory on every refresh.
    build_tree_with_registry(
        okf,
        uml_analysis,
        root_fallback,
        mask,
        limits,
        &crate::folder_projection::core_registry(),
    )
}

/// `build_tree` against a caller-supplied registry. The registry decides which
/// middleware names resolve, so a test can install a hostile stage; production
/// always passes `folder_projection::core_registry`.
fn build_tree_with_registry(
    okf: &waml::analysis::OkfAnalysis,
    uml_analysis: &waml::uml::Analysis,
    root_fallback: &str,
    mask: &waml::view::mask::ProjectionMask,
    limits: waml::view::chain::ChainLimits,
    registry: &waml::view::chain::MiddlewareRegistry,
) -> ProjectTree {
    /// The glyph a directory node draws when no projected row supplied one:
    /// the tree's own ROOT (nothing lists it, so no row carries its icon) and
    /// a repeat occurrence's shallow children. Resolved through `resolve_icon`
    /// against the SAME table every projected row resolves through, from the
    /// very `IconId` `RootView` stamps on the folder rows it mints -- so the
    /// root row cannot draw one glyph while its own directory children draw
    /// another. Any diagnostic is the caller's to fold into `view_degraded`.
    fn default_directory_icon(
        address: &waml::okf::DirectoryAddress,
        table: &[(&str, Icon)],
    ) -> (Icon, Option<waml::diagnostic::Diagnostic>) {
        crate::extension_editor::resolve_icon(
            Some(&waml::view::row::IconId::new(waml::view::FOLDER_ROW_ICON)),
            &waml::view::row::RowTarget::Folder(address.as_str().to_string()),
            table,
            address.as_str(),
            0,
        )
    }

    /// A directory node with no chain run and no children, used for a
    /// directory the build has already descended into once. The caller
    /// overwrites `key`, `title`, and the caps from the row that produced it,
    /// so the only thing this decides is "folder, nothing beneath it here".
    fn shallow_directory_node(
        address: &waml::okf::DirectoryAddress,
        table: &[(&str, Icon)],
    ) -> Option<TreeNode> {
        Some(TreeNode {
            key: waml::view::row::RowId {
                owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
                path: waml::view::row::RowPath::parse(ROOT_ROW_SEGMENT)
                    .expect("literal non-empty single segment parses"),
            },
            address: Some(address.as_str().to_string()),
            title: String::new(),
            kind: NavCategory::Directory,
            presentation: DocumentPresentation {
                icon: default_directory_icon(address, table).0,
                accent: crate::accent::tree_kind_color(NavCategory::Directory),
                category: NavCategory::Directory,
            },
            is_directory: true,
            openable: false,
            concept_id: None,
            caps: waml::view::row::RowCaps::default(),
            child_caps: waml::view::row::ChildCaps::default(),
            view_degraded: false,
            children: Vec::new(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn directory_node(
        okf: &waml::analysis::OkfAnalysis,
        uml_analysis: &waml::uml::Analysis,
        address: &waml::okf::DirectoryAddress,
        root_fallback: &str,
        mask: &waml::view::mask::ProjectionMask,
        limits: waml::view::chain::ChainLimits,
        registry: &waml::view::chain::MiddlewareRegistry,
        // The `IconId` -> `Icon` table every Folder row's icon resolves
        // against -- the same table the folder view resolves against (see
        // `folder_projection::icon_table`), passed down rather than
        // re-minted per directory for the same reason `registry` is.
        table: &[(&str, Icon)],
        // Every directory address this build has already expanded, anywhere in
        // the tree -- NOT just the current ancestor path.
        expanded: &mut std::collections::HashSet<String>,
    ) -> Option<TreeNode> {
        let bundle = &okf.bundle;
        // Confirms the directory still exists; its member lists are no
        // longer read here -- children come from the chain's rows instead.
        bundle
            .directories()
            .iter()
            .find(|directory| &directory.address == address)?;
        let index = bundle.index(address.as_str());
        let title = index
            .and_then(|index| index.title.clone())
            .unwrap_or_else(|| {
                if address.as_str() == "/" {
                    root_fallback.to_string()
                } else {
                    address
                        .as_str()
                        .rsplit('/')
                        .next()
                        .unwrap_or(root_fallback)
                        .to_string()
                }
            });
        // Only the tree's ROOT keeps this presentation: every other directory
        // node is overwritten below with the icon the row that produced it
        // resolved to.
        let (default_icon, default_icon_diagnostic) = default_directory_icon(address, table);
        let presentation = DocumentPresentation {
            icon: default_icon,
            accent: crate::accent::tree_kind_color(NavCategory::Directory),
            category: NavCategory::Directory,
        };
        let concept_node = |concept_id: &str,
                            key: waml::view::row::RowId,
                            caps: waml::view::row::RowCaps,
                            child_caps: waml::view::row::ChildCaps| {
            let concept = bundle.concept(concept_id)?;
            let descriptor = crate::documents::describe(okf, uml_analysis, concept_id)?;
            let presentation = descriptor.presentation;
            Some(TreeNode {
                key,
                address: None,
                title: concept.title.clone().unwrap_or_else(|| {
                    concept_id
                        .rsplit('/')
                        .next()
                        .unwrap_or(concept_id)
                        .to_string()
                }),
                kind: presentation.category,
                presentation,
                is_directory: false,
                openable: true,
                concept_id: Some(concept_id.to_owned()),
                caps,
                child_caps,
                view_degraded: false,
                children: Vec::new(),
            })
        };
        // The child addresses this recursion descends on come from the chain,
        // not from `bundle.child_directories`, so they are neither acyclic nor
        // even a tree: a middleware may emit a `Folder` row addressing an
        // ancestor (infinite recursion, stack overflow, dead editor) or one
        // addressing a directory another branch already listed (a DAG, whose
        // full expansion is exponential in the bundle's depth). `ChainLimits`
        // bounds one directory's run and nothing bounds this descent, so the
        // guard is global, and it bounds the DESCENT, not the listing: a
        // directory is descended into at most ONCE per build, so the total
        // cost stays proportional to the bundle. A repeat occurrence still
        // runs its own chain and lists its own rows -- the tree must agree
        // with the folder view about what a directory contains, and a node
        // that draws empty but opens full is exactly the disagreement this
        // module exists to prevent. What a repeat does NOT do is recurse:
        // its directory children are listed as childless nodes, which
        // terminates a cycle after one honest level instead of never.
        let repeat = !expanded.insert(address.as_str().to_string());
        let mut children = Vec::new();
        // Children ARE the chain's rows for this directory, in the chain's
        // order, carrying the chain's labels -- not the OKF member list. The
        // tree and the folder surface therefore cannot disagree about what a
        // directory contains. `project_rows` returning None means the
        // directory left the bundle underneath us; an empty child list is the
        // honest answer, not a panic.
        let projected =
            crate::folder_projection::project_rows(okf, address.as_str(), mask, limits, registry);
        // The marker reads the diagnostics of the run that produced the
        // children above -- the folder tab runs the same chain under the same
        // mask and shows the same list, so a second `resolved_view` here would
        // only be a chance to disagree (and a second full chain run per
        // directory, recursively, on every model refresh).
        // An icon this build could not resolve (an `UnknownIcon` warning, from
        // this node's own fallback glyph or from any child row's stamped one)
        // degrades this directory too -- the tree has no diagnostics strip of
        // its own, so the marker is the only way the warning reaches a reader
        // here; opening the folder tab shows the text.
        let mut view_degraded = default_icon_diagnostic.is_some()
            || projected
                .as_ref()
                .is_some_and(|(_, _, diagnostics)| !diagnostics.is_empty());
        for row in projected.iter().flat_map(|(_, rows, _)| rows.iter()) {
            match &row.target {
                waml::view::row::RowTarget::Folder(child_address) => {
                    let Ok(child) = waml::okf::DirectoryAddress::parse(child_address) else {
                        continue;
                    };
                    // A repeat occurrence lists but never descends (see the
                    // guard above), so its directory children are built
                    // shallow: same identity, same label, no chain run and
                    // no recursion.
                    let child_node = if repeat {
                        shallow_directory_node(&child, table)
                    } else {
                        directory_node(
                            okf,
                            uml_analysis,
                            &child,
                            root_fallback,
                            mask,
                            limits,
                            registry,
                            table,
                            expanded,
                        )
                    };
                    if let Some(mut node) = child_node {
                        // The chain owns the label, the identity, and the
                        // declared capabilities; a middleware may relabel a
                        // folder row, and the tree must show and key on what
                        // it said.
                        node.title = row.label.clone();
                        node.key = row.id.clone();
                        node.caps = row.caps;
                        node.child_caps = row.child_caps;
                        // The row's own `IconId` (or the target's default,
                        // resolved against the SAME table the folder view
                        // resolves against) -- a directory row in the tree
                        // must draw the same glyph the folder tab for that
                        // same directory would, and `resolve_icon` is the
                        // one place both surfaces resolve through.
                        let (icon, diagnostic) = crate::extension_editor::resolve_icon(
                            row.icon.as_ref(),
                            &row.target,
                            table,
                            address.as_str(),
                            0,
                        );
                        node.presentation.icon = icon;
                        view_degraded |= diagnostic.is_some();
                        children.push(node);
                    }
                }
                waml::view::row::RowTarget::Concept(concept_id) => {
                    if let Some(mut node) =
                        concept_node(concept_id, row.id.clone(), row.caps, row.child_caps)
                    {
                        node.title = row.label.clone();
                        // A stage-stamped `IconId` wins over the document
                        // kind's default glyph, resolved against the SAME
                        // table the folder view resolves against -- a stamp
                        // the folder tab honours must not be dropped here.
                        // An UNSTAMPED row keeps the descriptor's per-kind
                        // glyph (class, note, diagram, ...), which is what
                        // every shipped listing produces today.
                        //
                        // A stamp EQUAL to the root view's baseline is not a
                        // decision, it is the baseline: `RootView` stamps
                        // every concept row from `concept.ty` alone, while
                        // the descriptor derives its category from the UML
                        // PROJECTION (claims, diagrams, packages, ...) and
                        // falls back to `concept.ty` last. Honouring the
                        // baseline would silently repaint every row where the
                        // two disagree -- an unclaimed `type: uml.Class` doc
                        // draws the descriptor's `FileText` today -- and it
                        // would repaint the ICON ONLY, leaving
                        // `presentation.category` (accent bucket, classifier
                        // menus) on the old value, so the row's glyph and its
                        // colour would contradict each other. Only a stamp a
                        // LATER stage changed is a decision, and only that is
                        // honoured here.
                        let baseline = bundle.concept(concept_id).map(|concept| {
                            waml::view::kind::kind_of(&waml::model::ElementType::parse(&concept.ty))
                                .as_icon_name()
                        });
                        let stamped_by_middleware = match (row.icon.as_ref(), baseline) {
                            (Some(icon), Some(baseline)) => icon.as_str() != baseline,
                            (Some(_), None) => true,
                            (None, _) => false,
                        };
                        if stamped_by_middleware {
                            let (icon, diagnostic) = crate::extension_editor::resolve_icon(
                                row.icon.as_ref(),
                                &row.target,
                                table,
                                address.as_str(),
                                0,
                            );
                            node.presentation.icon = icon;
                            view_degraded |= diagnostic.is_some();
                        }
                        children.push(node);
                    }
                }
                // No file behind it, so nothing to open by concept id or
                // address. It still gets a row: dropping it would make the
                // tree disagree with the folder view about what is there.
                waml::view::row::RowTarget::Virtual => {
                    children.push(TreeNode {
                        key: row.id.clone(),
                        address: None,
                        title: row.label.clone(),
                        kind: NavCategory::OkfDocument,
                        presentation: DocumentPresentation {
                            icon: Icon::FileText,
                            accent: None,
                            category: NavCategory::OkfDocument,
                        },
                        is_directory: false,
                        openable: false,
                        concept_id: None,
                        caps: row.caps,
                        child_caps: row.child_caps,
                        view_degraded: false,
                        children: Vec::new(),
                    });
                }
            }
        }
        Some(TreeNode {
            // The bundle root mints its own id (see `ROOT_ROW_SEGMENT`); a
            // directory child immediately overwrites this with the row that
            // produced it (see the `Folder` arm above).
            key: waml::view::row::RowId {
                owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
                path: waml::view::row::RowPath::parse(ROOT_ROW_SEGMENT)
                    .expect("literal non-empty single segment parses"),
            },
            address: Some(address.as_str().to_string()),
            title,
            kind: NavCategory::Directory,
            presentation,
            is_directory: true,
            openable: false,
            concept_id: None,
            caps: waml::view::row::RowCaps::default(),
            child_caps: waml::view::row::ChildCaps::default(),
            view_degraded,
            children,
        })
    }

    let root = waml::okf::DirectoryAddress::parse("/").expect("root address is valid");
    let table = crate::folder_projection::icon_table();
    ProjectTree {
        roots: directory_node(
            okf,
            uml_analysis,
            &root,
            root_fallback,
            mask,
            limits,
            registry,
            &table,
            &mut std::collections::HashSet::new(),
        )
        .into_iter()
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;
    use waml::view::mask::ProjectionMask;

    fn every_maskable_name() -> ProjectionMask {
        let registry = crate::folder_projection::core_registry();
        ProjectionMask::from_names(
            crate::folder_projection::maskable_names(&registry)
                .into_iter()
                .flat_map(|(_owner, names)| names)
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
        )
    }

    fn hidden() -> waml::analysis::PreparedCandidate {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nview: hide\nhide: [\"**\"]\n---\n# Sales\n\n* [Order](./order.md)\n",
            ),
            ("sales/order.md", "# Order\n"),
        ])
        .unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    /// The folder-view spec's own checklist item that did not hold: an opaque
    /// folder showed no rows in its folder view and still listed every hidden
    /// child in the tree.
    #[test]
    fn an_opaque_folder_has_no_tree_children_projected_and_all_of_them_raw() {
        let prepared = hidden();
        let limits = waml::view::chain::ChainLimits::default();

        let projected = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            limits,
        );
        let sales = &projected.roots[0].children[0];
        assert!(
            sales.children.is_empty(),
            "hide: [\"**\"] leaves nothing for the tree to list",
        );

        let raw = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &every_maskable_name(),
            limits,
        );
        let sales = &raw.roots[0].children[0];
        assert_eq!(
            sales
                .children
                .iter()
                .map(|row| row.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Order"],
            "raw bypasses the chain, so the row is reachable again",
        );
    }

    /// The tree and the folder surface must never disagree about what a
    /// directory contains -- they read the same projection now, so this is a
    /// regression fence, not an aspiration.
    #[test]
    fn tree_children_equal_the_folder_views_rows_row_for_row_in_both_modes() {
        let prepared = hidden();
        let limits = waml::view::chain::ChainLimits::default();
        for mask in [ProjectionMask::default(), every_maskable_name()] {
            let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", &mask, limits);
            let sales = &tree.roots[0].children[0];
            let (_, rows, _) = crate::folder_projection::project_rows(
                prepared.okf(),
                "/sales",
                &mask,
                limits,
                &crate::folder_projection::core_registry(),
            )
            .unwrap();
            assert_eq!(
                sales
                    .children
                    .iter()
                    .map(|node| node.title.as_str())
                    .collect::<Vec<_>>(),
                rows.iter()
                    .map(|row| row.label.as_str())
                    .collect::<Vec<_>>(),
                "{mask:?}: tree children must be the chain's rows, in the chain's order",
            );
        }
    }

    fn mixed() -> (waml::analysis::OkfAnalysis, waml::uml::Analysis) {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "# Sales\n\n* [Order](./order.md)\n* [Archive](archive/)\n* [Runbook](./runbook.md)\n",
            ),
            ("sales/archive/index.md", "# Archive\n"),
            ("sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
            ("sales/runbook.md", "---\ntype: Runbook\ntitle: Runbook\n---\n# Runbook\n"),
            ("sales/log.md", "# Log\n"),
        ]).unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let (_, okf, uml, _) = prepared.into_parts();
        (okf, uml)
    }

    #[test]
    fn navigator_uses_okf_directories_and_authored_index_order() {
        let (bundle, projection) = mixed();
        let tree = build_tree(
            &bundle,
            &projection,
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let root = &tree.roots[0];
        assert_eq!(
            (root.address.as_deref(), root.title.as_str()),
            (Some("/"), "Root")
        );
        let sales = &root.children[0];
        assert_eq!(
            (sales.address.as_deref(), sales.title.as_str()),
            (Some("/sales"), "Sales")
        );
        assert_eq!(
            sales
                .children
                .iter()
                .map(|row| key_string(&row.key))
                .collect::<Vec<_>>(),
            [
                key_string(&waml::view::row::RowId {
                    owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
                    path: waml::view::row::RowPath::parse("sales/order").unwrap(),
                }),
                key_string(&waml::view::row::RowId {
                    owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
                    path: waml::view::row::RowPath::parse("sales/archive").unwrap(),
                }),
                key_string(&waml::view::row::RowId {
                    owner: waml::view::row::ViewId::new(waml::view::ROOT_VIEW_OWNER),
                    path: waml::view::row::RowPath::parse("sales/runbook").unwrap(),
                }),
            ]
        );
    }

    #[test]
    fn providers_decorate_claimed_and_generic_rows_with_capabilities() {
        let (bundle, projection) = mixed();
        let tree = build_tree(
            &bundle,
            &projection,
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let rows = &tree.roots[0].children[0].children;
        let order = rows
            .iter()
            .find(|row| row.concept_id.as_deref() == Some("sales/order"))
            .unwrap();
        assert_eq!(order.kind, NavCategory::Class);
        assert!(order.openable && order.caps.rename && order.caps.delete);
        assert_eq!(order.concept_id.as_deref(), Some("sales/order"));
        let runbook = rows
            .iter()
            .find(|row| row.concept_id.as_deref() == Some("sales/runbook"))
            .unwrap();
        assert_eq!(runbook.kind, NavCategory::OkfDocument);
        assert!(runbook.openable);
        // A concept row's rename/delete are declared by its owning chain
        // stage, not derived from the document's classifier type -- the
        // guess this field replaced.
        assert!(runbook.caps.rename && runbook.caps.delete);
    }

    /// `mixed()` returns the analyses split apart; `project_rows` needs the
    /// `PreparedCandidate` itself.
    fn mixed_prepared() -> waml::analysis::PreparedCandidate {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "# Sales\n\n* [Order](./order.md)\n* [Archive](archive/)\n* [Runbook](./runbook.md)\n",
            ),
            ("sales/archive/index.md", "# Archive\n"),
            ("sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
            ("sales/runbook.md", "---\ntype: Runbook\ntitle: Runbook\n---\n# Runbook\n"),
            ("sales/log.md", "# Log\n"),
        ])
        .unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    /// Capabilities are advisory and `apply` remains the authority, but the
    /// invariant the chain spec states is that a DECLARED capability must not
    /// yield Unsupported. Carrying the row's own declaration is what lets the
    /// tree gate an affordance on something apply will honour, instead of on
    /// a guess made from the document type.
    #[test]
    fn tree_rows_carry_the_projected_rows_declared_capabilities() {
        let prepared = mixed_prepared();
        let limits = waml::view::chain::ChainLimits::default();
        let mask = ProjectionMask::default();
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", &mask, limits);
        let sales = &tree.roots[0].children[0];
        let (_, rows, _) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            &mask,
            limits,
            &crate::folder_projection::core_registry(),
        )
        .unwrap();

        assert_eq!(sales.children.len(), rows.len());
        for (node, row) in sales.children.iter().zip(rows.iter()) {
            assert_eq!(node.caps, row.caps);
            assert_eq!(node.child_caps, row.child_caps);
        }
    }

    /// A middleware that emits a `Folder` row addressing the bundle root, from
    /// whatever directory it runs in. Nothing in the core extension does this
    /// -- both shipped emitters constrain themselves to `child_directories` --
    /// but nothing in the chain contract forbids it either, and the tree
    /// descends on the address the row carries.
    struct CycleBack;

    impl waml::view::projection::Projection for CycleBack {
        fn project(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::ProjectionError> {
            Ok(vec![waml::view::row::Row::new(
                waml::view::row::RowId {
                    owner: waml::view::row::ViewId::new("cycle-back"),
                    path: waml::view::row::RowPath::parse("up").unwrap(),
                },
                "Up".to_string(),
                waml::view::row::RowTarget::Folder("/".to_string()),
                None,
            )
            .expect("a Folder target never requires a surface override")])
        }

        fn resolve(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::Unresolved> {
            Err(waml::view::projection::Unresolved)
        }

        fn apply(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
            _op: waml::view::projection::RowOp,
            _next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::okf::Op>, waml::view::projection::Unsupported> {
            Err(waml::view::projection::Unsupported)
        }

        fn surface(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            next: waml::view::projection::Next<'_>,
        ) -> waml::view::surface::SurfaceId {
            next.surface(ctx)
        }
    }

    struct CycleExt;

    impl waml::extension::CoreExtension for CycleExt {
        fn name(&self) -> &str {
            "cycle-test"
        }

        fn middleware(&self) -> Vec<(&'static str, waml::extension::MiddlewareFactory)> {
            vec![(
                "cycle-back",
                std::sync::Arc::new(|| {
                    Box::new(CycleBack) as Box<dyn waml::view::projection::Projection>
                }),
            )]
        }

        fn profiles(&self) -> Vec<waml::profile::ProfileDef> {
            Vec::new()
        }
    }

    /// The recursion descends on addresses the CHAIN emitted, which -- unlike
    /// `bundle.child_directories` -- are not acyclic by construction. Without
    /// a guard this recurses until the stack is gone and the editor dies; the
    /// test therefore aborts the process outright when the guard is missing.
    #[test]
    fn a_middleware_that_points_a_folder_row_at_an_ancestor_does_not_recurse_forever() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "---\nview: cycle-back\n---\n# Sales\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let registry = waml::view::chain::MiddlewareRegistry::from_extensions(&[
            &waml::extension::CoreExt,
            &CycleExt,
        ])
        .unwrap();

        let tree = build_tree_with_registry(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
            &registry,
        );
        let sales = &tree.roots[0].children[0];
        let up = &sales.children[0];
        assert_eq!(
            up.title, "Up",
            "the row stays listed -- the tree must not disagree with the folder view",
        );
        // The row that closed the cycle addresses the root, and the root is
        // not empty: it lists Sales. Drawing it childless would be the
        // disagreement this module exists to prevent -- opening that row
        // shows a listing the tree claimed was not there. So it lists, and
        // the level below it is where the cycle stops.
        assert_eq!(
            up.children
                .iter()
                .map(|node| node.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Sales"],
            "the closing row lists what its directory actually contains",
        );
        assert!(
            up.children.iter().all(|node| node.children.is_empty()),
            "and nothing below it descends -- one honest level, then stop",
        );
    }

    /// A middleware that lists the SAME directory from two different branches:
    /// a DAG, not a cycle, so an ancestor-path-only guard never fires on it and
    /// the shared subtree is rebuilt once per path that reaches it -- which is
    /// exponential in the bundle's depth, not linear in its size.
    struct Fanout;

    fn folder_row(path: &str, label: &str, target: &str) -> waml::view::row::Row {
        waml::view::row::Row::new(
            waml::view::row::RowId {
                owner: waml::view::row::ViewId::new("fanout"),
                path: waml::view::row::RowPath::parse(path).unwrap(),
            },
            label.to_string(),
            waml::view::row::RowTarget::Folder(target.to_string()),
            None,
        )
        .expect("a Folder target never requires a surface override")
    }

    impl waml::view::projection::Projection for Fanout {
        fn project(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            _next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::ProjectionError> {
            Ok(match ctx.dir.address.as_str() {
                "/" => vec![
                    folder_row("left", "Left", "/left"),
                    folder_row("right", "Right", "/right"),
                ],
                "/left" | "/right" => vec![folder_row("shared", "Shared", "/shared")],
                "/shared" => vec![folder_row("deep", "Deep", "/shared/deep")],
                _ => Vec::new(),
            })
        }

        fn resolve(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::Unresolved> {
            Err(waml::view::projection::Unresolved)
        }

        fn apply(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
            _op: waml::view::projection::RowOp,
            _next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::okf::Op>, waml::view::projection::Unsupported> {
            Err(waml::view::projection::Unsupported)
        }

        fn surface(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            next: waml::view::projection::Next<'_>,
        ) -> waml::view::surface::SurfaceId {
            next.surface(ctx)
        }
    }

    struct FanoutExt;

    impl waml::extension::CoreExtension for FanoutExt {
        fn name(&self) -> &str {
            "fanout-test"
        }

        fn middleware(&self) -> Vec<(&'static str, waml::extension::MiddlewareFactory)> {
            vec![(
                "fanout",
                std::sync::Arc::new(|| {
                    Box::new(Fanout) as Box<dyn waml::view::projection::Projection>
                }),
            )]
        }

        fn profiles(&self) -> Vec<waml::profile::ProfileDef> {
            Vec::new()
        }
    }

    fn count_titled(node: &TreeNode, title: &str) -> usize {
        usize::from(node.title == title)
            + node
                .children
                .iter()
                .map(|child| count_titled(child, title))
                .sum::<usize>()
    }

    /// `ChainLimits` bounds ONE directory's run; nothing bounds the descent
    /// `build_tree` performs across directories. A chain that lists the same
    /// non-ancestor directory from several branches therefore re-expands it
    /// once per branch, and a chain of such levels multiplies. A directory is
    /// expanded at most once per build; the repeat row stays listed, without
    /// children.
    #[test]
    fn a_directory_listed_from_two_branches_is_expanded_only_once() {
        let index = "---\nview: fanout\n---\n# Dir\n";
        let source = SourceBundle::try_from_pairs([
            ("index.md", index),
            ("left/index.md", index),
            ("right/index.md", index),
            ("shared/index.md", index),
            ("shared/deep/index.md", index),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let registry = waml::view::chain::MiddlewareRegistry::from_extensions(&[
            &waml::extension::CoreExt,
            &FanoutExt,
        ])
        .unwrap();

        let tree = build_tree_with_registry(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
            &registry,
        );
        let root = &tree.roots[0];
        assert_eq!(
            root.children
                .iter()
                .map(|node| node.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Left", "Right"],
            "both branches stay listed -- the tree must not disagree with the folder view",
        );
        assert_eq!(
            count_titled(root, "Shared"),
            2,
            "the repeat row is listed by both branches",
        );
        // Both occurrences of the shared directory list their own rows, so
        // neither draws as an empty folder that opens full. What the guard
        // bounds is the DESCENT: the second occurrence's children are
        // childless, so the shared subtree is walked once however many
        // branches reach it.
        assert_eq!(
            count_titled(root, "Deep"),
            2,
            "each occurrence lists its own rows -- the tree agrees with the folder view at both",
        );
        let shared: Vec<&TreeNode> = root
            .children
            .iter()
            .flat_map(|branch| branch.children.iter())
            .filter(|node| node.title == "Shared")
            .collect();
        assert_eq!(shared.len(), 2, "one Shared row under each branch");
        for node in &shared {
            assert_eq!(
                node.children
                    .iter()
                    .map(|child| child.title.as_str())
                    .collect::<Vec<_>>(),
                vec!["Deep"],
                "every occurrence lists the same rows the folder view would",
            );
        }
        // What the guard bounds is the level BELOW: the repeat occurrence's
        // children are built shallow and never recurse. `Fanout` bottoms out
        // at `/shared/deep`, so the stop is asserted by the ancestor test
        // above, where the closing row's child would otherwise keep going.
    }

    #[test]
    fn kind_of_keeps_uml_presentation_granularity() {
        assert_eq!(
            kind_of(&ElementType::parse("uml.Interface")),
            NavCategory::Interface
        );
        assert_eq!(
            kind_of(&ElementType::parse("uml.Sequence")),
            NavCategory::Sequence
        );
        assert_eq!(
            kind_of(&ElementType::parse("vendor.Custom")),
            NavCategory::OkfDocument
        );
    }

    #[test]
    fn a_degraded_chain_outcome_flags_the_tree_row() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nview: no-such-middleware\n---\n# Sales\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let sales = &tree.roots[0].children[0];
        assert_eq!(sales.address.as_deref(), Some("/sales"));
        assert!(sales.view_degraded);
        assert!(!tree.roots[0].view_degraded);
    }

    /// Masking a declared stage by name never builds it, so the folder tab
    /// opened under that mask shows no diagnostics at all -- even for a
    /// stage name the registry does not recognize. The tree must not
    /// contradict it by marking the same folder degraded.
    #[test]
    fn masking_the_declared_stage_does_not_mark_a_folder_degraded_the_folder_tab_reports_clean() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nview: no-such-middleware\n---\n# Sales\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let limits = waml::view::chain::ChainLimits::default();
        // Masking targets the exact declared (unknown) name -- an unknown name
        // is never offered by `maskable_names`, so `every_maskable_name()`
        // would not bypass it; the point here is that masking the DECLARED
        // stage bypasses it regardless of whether the registry knows it.
        let mask = ProjectionMask::from_names(["no-such-middleware"]);
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", &mask, limits);
        let sales = &tree.roots[0].children[0];
        let (_, _, diagnostics) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            &mask,
            limits,
            &crate::folder_projection::core_registry(),
        )
        .unwrap();
        assert!(
            diagnostics.is_empty(),
            "masking the declared stage never builds it, so it never diagnoses"
        );
        assert!(
            !sales.view_degraded,
            "the tree marked a folder degraded that the folder tab reports clean",
        );
    }

    /// The root mints its own `RowId`; a real row that happened to mint the
    /// same one would share the root's `LiveId`, and with it the root's
    /// selection, fold, and chevron state.
    #[test]
    fn the_bundle_roots_key_cannot_collide_with_a_projected_row() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root\n\n* [Root doc](root.md)\n* [Root dir](root/)\n",
            ),
            ("root.md", "# Root doc\n"),
            ("root/index.md", "# Root dir\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let root = &tree.roots[0];
        let root_key = key_string(&root.key);
        for child in &root.children {
            assert_ne!(
                key_string(&child.key),
                root_key,
                "{} collides with the bundle root's key",
                child.title,
            );
        }
    }

    /// In Raw the chain is [index], so the root view owns every row and
    /// RootView::resolve answers every path -- Raw is today's listing.
    #[test]
    fn raw_rows_are_owned_by_the_root_view() {
        let prepared = hidden();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &every_maskable_name(),
            waml::view::chain::ChainLimits::default(),
        );
        let sales = &tree.roots[0].children[0];
        assert!(sales
            .children
            .iter()
            .all(|node| node.key.owner.as_str() == waml::view::ROOT_VIEW_OWNER),);
    }

    /// A RowId minted by a middleware while Projected does not resolve in
    /// Raw, because its owner is not in the raw chain. Expansion or selection
    /// sitting on such a row falls back to the nearest resolvable prefix --
    /// at worst the folder. That is the existing Unresolved rule, not a new
    /// one, and it must NOT panic or vanish.
    #[test]
    fn a_row_id_whose_owner_is_absent_from_the_raw_chain_falls_back_to_a_prefix() {
        let prepared = hidden();
        let limits = waml::view::chain::ChainLimits::default();
        let (chain, _, _) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            &every_maskable_name(),
            limits,
            &crate::folder_projection::core_registry(),
        )
        .unwrap();
        let dir = prepared.okf().bundle.directory("/sales").unwrap().clone();
        let params = prepared
            .okf()
            .bundle
            .index("/sales")
            .map(|index| index.extra.clone())
            .unwrap_or_default();
        let descend = |_: &waml::okf::Directory| waml::view::chain::Chain::default();
        let ctx = waml::view::projection::ProjectionCtx {
            dir: &dir,
            bundle: &prepared.okf().bundle,
            params: &params,
            descend: &descend,
        };
        let stranger = waml::view::row::RowId {
            owner: waml::view::row::ViewId::new("group-by-tag"),
            path: waml::view::row::RowPath::parse("synthesized/leaf").unwrap(),
        };
        let rows = chain.resolve(&ctx, &stranger);
        assert!(
            rows.is_ok(),
            "an unresolvable RowId falls back to the folder's own listing",
        );
    }

    /// The panel keys its maps on a flat string; the encoding must not let two
    /// distinct RowIds collide.
    #[test]
    fn key_string_is_injective_across_owner_and_path() {
        let a = waml::view::row::RowId {
            owner: waml::view::row::ViewId::new("a/b"),
            path: waml::view::row::RowPath::parse("c").unwrap(),
        };
        let b = waml::view::row::RowId {
            owner: waml::view::row::ViewId::new("a"),
            path: waml::view::row::RowPath::parse("b/c").unwrap(),
        };
        assert_ne!(key_string(&a), key_string(&b));
    }

    #[test]
    fn claimed_uml_package_concept_is_an_openable_leaf_not_a_structural_directory() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Domain](domain.md)\n"),
            (
                "domain.md",
                "---\ntype: uml.Package\ntitle: Domain\n---\n# Domain\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let domain = &tree.roots[0].children[0];

        assert_eq!(domain.kind, NavCategory::Directory);
        assert!(!domain.is_directory);
        assert!(domain.openable);
        assert!(crate::documents::open(prepared.okf(), prepared.uml(), "domain").is_some());
    }

    /// Task 11: a tree row and the folder row for the same directory resolve
    /// to the same `Icon` -- both surfaces resolve the SAME `IconId` against
    /// the SAME table (`folder_projection::icon_table`), so a `uml-domain`
    /// package draws the box glyph and a plain folder draws the book glyph in
    /// the tree exactly as it does in the folder tab for that directory.
    #[test]
    fn tree_row_icon_matches_the_folder_row_icon_for_the_same_directory() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "---\nview: uml\n---\n# Root\n\n* [Pkg](pkg/)\n* [Docs](docs/)\n",
            ),
            ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
            ("docs/index.md", "# Docs\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let limits = waml::view::chain::ChainLimits::default();
        let mask = ProjectionMask::default();

        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", &mask, limits);
        let tree_icons: std::collections::HashMap<String, Icon> = tree.roots[0]
            .children
            .iter()
            .map(|node| (node.title.clone(), node.presentation.icon))
            .collect();

        let folder = crate::folder_view::FolderView::build(prepared.okf(), "/", limits, &mask)
            .expect("root is in the bundle");
        let folder_icons: std::collections::HashMap<String, Icon> = folder
            .row_views()
            .iter()
            .map(|row| (row.label.clone(), row.icon))
            .collect();

        assert_eq!(tree_icons["Pkg"], Icon::Box, "a uml-domain package");
        assert_eq!(tree_icons["Docs"], Icon::Book, "a plain folder");
        assert_eq!(tree_icons["Pkg"], folder_icons["Pkg"]);
        assert_eq!(tree_icons["Docs"], folder_icons["Docs"]);
    }

    /// The `uml` stage stamps the package glyph while projecting a LISTING, so
    /// the stage has to be in the PARENT directory's resolved chain. A folder
    /// that declares `profile: uml-domain` under a parent that resolves no
    /// `uml` stage is still a package -- `resolved_profile` says so -- but
    /// nothing is running that would stamp its row, so it draws the plain
    /// folder glyph. Declaring the profile on the child is necessary and NOT
    /// sufficient; this is the failure a bundle author hits first.
    #[test]
    fn a_declared_package_under_a_chainless_parent_draws_the_plain_folder_glyph() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Pkg](pkg/)\n"),
            ("pkg/index.md", "---\nprofile: uml-domain\n---\n# Pkg\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );

        assert_eq!(
            prepared.okf().bundle.resolved_profile("/pkg"),
            Some("uml-domain"),
            "the declaration itself resolves",
        );
        assert_eq!(
            tree.roots[0].children[0].presentation.icon,
            Icon::Book,
            "but the root listing ran no uml stage, so nothing stamped the row",
        );
    }

    /// The same thing again, but against a fixture on disk rather than a
    /// hand-built bundle: `tests/fixtures/packages` is the one shipped bundle
    /// with real nested directories, one declaring `profile: uml-domain` and
    /// one declaring nothing, so the package glyph has a fixture a human can
    /// open in the editor and see.
    #[test]
    fn the_packages_fixture_draws_a_box_for_its_declared_package() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/packages");
        let source = crate::load::read_bundle(&dir).expect("the packages fixture loads");
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let icons: std::collections::HashMap<String, Icon> = tree.roots[0]
            .children
            .iter()
            .map(|node| (node.title.clone(), node.presentation.icon))
            .collect();

        assert_eq!(icons["Billing"], Icon::Box, "a declared uml-domain package");
        assert_eq!(
            icons["Notes"],
            Icon::Book,
            "a plain folder declaring nothing"
        );
    }

    /// The ROOT node is the one directory in the tree that no listing
    /// produced, so nothing hands it a row icon. It must still draw the glyph
    /// a directory row draws -- a root drawing the folder glyph while every
    /// directory beneath it draws the book glyph is exactly the cross-surface
    /// disagreement the icon table exists to prevent (visual check V5).
    #[test]
    fn the_root_node_draws_the_same_glyph_as_its_directory_children() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "# Root

* [Docs](docs/)
",
            ),
            (
                "docs/index.md",
                "# Docs
",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let tree = build_tree(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
        );
        let root = &tree.roots[0];
        assert_eq!(root.presentation.icon, Icon::Book);
        assert_eq!(
            root.presentation.icon, root.children[0].presentation.icon,
            "the root and its directory children draw one glyph, not two",
        );
    }

    /// A stage that stamps the `box` glyph on every row, concept rows
    /// included -- the shipped `uml` stage only stamps folders, but nothing
    /// in the row model restricts a stamp to one target kind.
    struct StampBox;

    impl waml::view::projection::Projection for StampBox {
        fn project(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::ProjectionError> {
            let mut rows = next.project(ctx)?;
            for row in &mut rows {
                row.icon = Some(waml::view::row::IconId::new("box"));
            }
            Ok(rows)
        }

        fn resolve(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::Unresolved> {
            Err(waml::view::projection::Unresolved)
        }

        fn apply(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
            _op: waml::view::projection::RowOp,
            _next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::okf::Op>, waml::view::projection::Unsupported> {
            Err(waml::view::projection::Unsupported)
        }

        fn surface(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            next: waml::view::projection::Next<'_>,
        ) -> waml::view::surface::SurfaceId {
            next.surface(ctx)
        }
    }

    struct StampBoxExt;

    impl waml::extension::CoreExtension for StampBoxExt {
        fn name(&self) -> &str {
            "stamp-box-test"
        }

        fn middleware(&self) -> Vec<(&'static str, waml::extension::MiddlewareFactory)> {
            vec![(
                "stamp-box",
                std::sync::Arc::new(|| {
                    Box::new(StampBox) as Box<dyn waml::view::projection::Projection>
                }),
            )]
        }

        fn profiles(&self) -> Vec<waml::profile::ProfileDef> {
            Vec::new()
        }
    }

    /// Task 11 applies to CONCEPT rows too: a stamped icon the folder tab
    /// honours must not be dropped by the tree, or the two surfaces disagree
    /// about the same row -- exactly what the icon table exists to prevent.
    #[test]
    fn a_stamped_concept_icon_is_honoured_in_the_tree_and_matches_the_folder_row() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nview: stamp-box\n---\n# Sales\n\n* [Doc](./doc.md)\n",
            ),
            ("sales/doc.md", "---\ntitle: Doc\n---\n# Doc\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let registry = waml::view::chain::MiddlewareRegistry::from_extensions(&[
            &waml::extension::CoreExt,
            &StampBoxExt,
        ])
        .unwrap();
        let limits = waml::view::chain::ChainLimits::default();
        let mask = ProjectionMask::default();

        let tree = build_tree_with_registry(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &mask,
            limits,
            &registry,
        );
        let sales = &tree.roots[0].children[0];
        let doc = sales
            .children
            .iter()
            .find(|node| node.title == "Doc")
            .expect("the stamped concept row lists");

        let (_, rows, _) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            &mask,
            limits,
            &registry,
        )
        .expect("the directory is in the bundle");
        let (folder_rows, _) =
            crate::folder_view::row_views(&rows, &crate::folder_projection::icon_table(), "/sales");
        let folder_icon = folder_rows
            .iter()
            .find(|row| row.label == "Doc")
            .expect("the same row lists in the folder view")
            .icon;

        assert_eq!(doc.presentation.icon, Icon::Box, "the stamp is honoured");
        assert_eq!(doc.presentation.icon, folder_icon);
    }

    /// The other half of the same rule: `RootView` stamps EVERY concept row
    /// from `concept.ty`, so a bare stamp is not a decision and must not
    /// repaint a row whose descriptor disagrees. A `type: uml.Class` document
    /// that the UML analysis does not claim is an OKF document as far as the
    /// descriptor is concerned -- it draws `FileText`, and its accent bucket
    /// and classifier menus agree with that. Honouring the baseline stamp
    /// would move the GLYPH to the class icon and leave the category behind,
    /// so the row would contradict itself.
    #[test]
    fn the_root_views_baseline_stamp_does_not_repaint_an_unclaimed_document() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n\n* [Doc](./doc.md)\n"),
            (
                "sales/doc.md",
                "---\ntitle: Doc\ntype: uml.Class\n---\n# Doc\n",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let registry =
            waml::view::chain::MiddlewareRegistry::from_extensions(&[&waml::extension::CoreExt])
                .unwrap();
        let tree = build_tree_with_registry(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
            &registry,
        );
        let doc = tree.roots[0].children[0]
            .children
            .iter()
            .find(|node| node.title == "Doc")
            .expect("the document lists");

        let descriptor = crate::documents::describe(
            prepared.okf(),
            prepared.uml(),
            doc.concept_id
                .as_deref()
                .expect("a concept row carries its id"),
        )
        .expect("the document has a descriptor");
        assert_eq!(
            doc.presentation.icon, descriptor.presentation.icon,
            "the descriptor still owns the glyph"
        );
        assert_eq!(
            doc.presentation.category, descriptor.presentation.category,
            "and the glyph and the category still come from the same place"
        );
    }

    /// A stage that stamps an icon name nothing resolves. The core extension
    /// ships no such stage -- the gate (`script_gate.rs`) asserts every
    /// mintable name has a registered `Icon` -- but a third-party stage can,
    /// and the tree must not degrade to the default glyph silently.
    struct UnknownIcon;

    impl waml::view::projection::Projection for UnknownIcon {
        fn project(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::ProjectionError> {
            let mut rows = next.project(ctx)?;
            for row in &mut rows {
                row.icon = Some(waml::view::row::IconId::new("no-such-icon"));
            }
            Ok(rows)
        }

        fn resolve(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
        ) -> Result<Vec<waml::view::row::Row>, waml::view::projection::Unresolved> {
            Err(waml::view::projection::Unresolved)
        }

        fn apply(
            &self,
            _ctx: &waml::view::projection::ProjectionCtx<'_>,
            _path: &waml::view::row::RowPath,
            _op: waml::view::projection::RowOp,
            _next: waml::view::projection::Next<'_>,
        ) -> Result<Vec<waml::okf::Op>, waml::view::projection::Unsupported> {
            Err(waml::view::projection::Unsupported)
        }

        fn surface(
            &self,
            ctx: &waml::view::projection::ProjectionCtx<'_>,
            next: waml::view::projection::Next<'_>,
        ) -> waml::view::surface::SurfaceId {
            next.surface(ctx)
        }
    }

    struct UnknownIconExt;

    impl waml::extension::CoreExtension for UnknownIconExt {
        fn name(&self) -> &str {
            "unknown-icon-test"
        }

        fn middleware(&self) -> Vec<(&'static str, waml::extension::MiddlewareFactory)> {
            vec![(
                "unknown-icon",
                std::sync::Arc::new(|| {
                    Box::new(UnknownIcon) as Box<dyn waml::view::projection::Projection>
                }),
            )]
        }

        fn profiles(&self) -> Vec<waml::profile::ProfileDef> {
            Vec::new()
        }
    }

    /// The tree has no diagnostics strip, so its degraded marker is the only
    /// way an `UnknownIcon` warning can reach a reader here. Without it the
    /// row simply draws the default glyph and nothing anywhere says why.
    #[test]
    fn an_unknown_icon_name_marks_the_directory_degraded() {
        let source = SourceBundle::try_from_pairs([
            (
                "index.md",
                "---
view: unknown-icon
---
# Root

* [Docs](docs/)
",
            ),
            (
                "docs/index.md",
                "# Docs
",
            ),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let registry = waml::view::chain::MiddlewareRegistry::from_extensions(&[
            &waml::extension::CoreExt,
            &UnknownIconExt,
        ])
        .unwrap();

        let tree = build_tree_with_registry(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
            &registry,
        );
        let root = &tree.roots[0];
        assert_eq!(
            root.children[0].presentation.icon,
            Icon::Folder,
            "an unresolvable name degrades to the target's default glyph",
        );
        assert!(
            root.view_degraded,
            "an unresolvable icon name must be visible as a degraded listing",
        );
    }

    /// The same warning must reach a reader when the unresolvable stamp landed
    /// on a CONCEPT row -- dropping `row.icon` there would swallow both the
    /// glyph and the warning.
    #[test]
    fn an_unknown_icon_name_on_a_concept_row_marks_the_directory_degraded() {
        let source = SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nview: unknown-icon\n---\n# Sales\n\n* [Doc](./doc.md)\n",
            ),
            ("sales/doc.md", "---\ntitle: Doc\n---\n# Doc\n"),
        ])
        .unwrap();
        let prepared = waml::analysis::prepare_candidate(source, None, 1).unwrap();
        let registry = waml::view::chain::MiddlewareRegistry::from_extensions(&[
            &waml::extension::CoreExt,
            &UnknownIconExt,
        ])
        .unwrap();

        let tree = build_tree_with_registry(
            prepared.okf(),
            prepared.uml(),
            "Fallback",
            &ProjectionMask::default(),
            waml::view::chain::ChainLimits::default(),
            &registry,
        );
        let sales = &tree.roots[0].children[0];
        assert!(
            sales.children.iter().any(|node| node.title == "Doc"),
            "the concept row still lists",
        );
        assert!(
            sales.view_degraded,
            "an unresolvable icon name on a concept row must degrade the listing",
        );
    }
}
