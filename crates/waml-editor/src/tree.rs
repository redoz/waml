//! The tree seam: flatten a `Model` into a `ProjectTree` for the panel.
//! Nothing here touches makepad; the `LiveId` bridge lives in `tree_panel.rs`.

use crate::document::{DocumentPresentation, NavCategory};
use crate::icons::Icon;
use waml::model::{BehaviorKind, ElementType, UmlMetaclass};

pub type TreeKind = NavCategory;

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// The projected row's identity, NOT a file address. Stable across a
    /// re-projection, so selection and expansion survive the chain re-run a
    /// mode flip triggers; a file address is not, because a middleware may
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
    /// Mode-dependent by construction: `Raw` never builds the declared chain,
    /// so it never diagnoses one, and the marker is absent exactly where the
    /// folder tab would also show nothing. Always `false` for a non-directory
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
    match ty {
        ElementType::Uml(UmlMetaclass::Package) => TreeKind::Directory,
        ElementType::Uml(UmlMetaclass::Note) => TreeKind::Note,
        ElementType::Uml(UmlMetaclass::Interface) => TreeKind::Interface,
        ElementType::Uml(UmlMetaclass::Enum) => TreeKind::Enum,
        ElementType::Uml(UmlMetaclass::DataType) => TreeKind::DataType,
        ElementType::Uml(
            UmlMetaclass::Class
            | UmlMetaclass::Association
            | UmlMetaclass::Actor
            | UmlMetaclass::UseCase
            | UmlMetaclass::InstanceSpecification,
        ) => TreeKind::Class,
        ElementType::Behavior(BehaviorKind::Sequence) => TreeKind::Sequence,
        ElementType::Behavior(_) => TreeKind::Behavior,
        ElementType::Diagram => TreeKind::Diagram,
        ElementType::Unknown(_) => TreeKind::OkfDocument,
    }
}

pub fn build_tree(
    okf: &waml::analysis::OkfAnalysis,
    uml_analysis: &waml::uml::Analysis,
    root_fallback: &str,
    mode: crate::folder_projection::ViewMode,
    limits: waml::view::chain::ChainLimits,
) -> ProjectTree {
    fn directory_node(
        okf: &waml::analysis::OkfAnalysis,
        uml_analysis: &waml::uml::Analysis,
        address: &waml::okf::DirectoryAddress,
        root_fallback: &str,
        mode: crate::folder_projection::ViewMode,
        limits: waml::view::chain::ChainLimits,
        registry: &waml::view::chain::MiddlewareRegistry,
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
        let presentation = DocumentPresentation {
            icon: Icon::Folder,
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
        let mut children = Vec::new();
        // Children ARE the chain's rows for this directory, in the chain's
        // order, carrying the chain's labels -- not the OKF member list. The
        // tree and the folder surface therefore cannot disagree about what a
        // directory contains. `project_rows` returning None means the
        // directory left the bundle underneath us; an empty child list is the
        // honest answer, not a panic.
        let projected =
            crate::folder_projection::project_rows(okf, address.as_str(), mode, limits, registry);
        // The marker reads the diagnostics of the run that produced the
        // children above -- the folder tab runs the same chain in the same
        // mode and shows the same list, so a second `resolved_view` here would
        // only be a chance to disagree (and a second full chain run per
        // directory, recursively, on every model refresh).
        let view_degraded = projected
            .as_ref()
            .is_some_and(|(_, _, diagnostics)| !diagnostics.is_empty());
        for row in projected.iter().flat_map(|(_, rows, _)| rows.iter()) {
            match &row.target {
                waml::view::row::RowTarget::Folder(child_address) => {
                    let Ok(child) = waml::okf::DirectoryAddress::parse(child_address) else {
                        continue;
                    };
                    if let Some(mut node) = directory_node(
                        okf,
                        uml_analysis,
                        &child,
                        root_fallback,
                        mode,
                        limits,
                        registry,
                    ) {
                        // The chain owns the label, the identity, and the
                        // declared capabilities; a middleware may relabel a
                        // folder row, and the tree must show and key on what
                        // it said.
                        node.title = row.label.clone();
                        node.key = row.id.clone();
                        node.caps = row.caps;
                        node.child_caps = row.child_caps;
                        children.push(node);
                    }
                }
                waml::view::row::RowTarget::Concept(concept_id) => {
                    if let Some(mut node) =
                        concept_node(concept_id, row.id.clone(), row.caps, row.child_caps)
                    {
                        node.title = row.label.clone();
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
    // One registry for the whole recursion: `FolderView::build` resolves
    // against the same name table, and minting it per directory put a fresh
    // allocation on a path that runs for every directory on every refresh.
    let registry = crate::folder_projection::core_registry();
    ProjectTree {
        roots: directory_node(
            okf,
            uml_analysis,
            &root,
            root_fallback,
            mode,
            limits,
            &registry,
        )
        .into_iter()
        .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

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
            crate::folder_projection::ViewMode::Projected,
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
            crate::folder_projection::ViewMode::Raw,
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
        for mode in [
            crate::folder_projection::ViewMode::Projected,
            crate::folder_projection::ViewMode::Raw,
        ] {
            let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", mode, limits);
            let sales = &tree.roots[0].children[0];
            let (_, rows, _) = crate::folder_projection::project_rows(
                prepared.okf(),
                "/sales",
                mode,
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
                "{mode:?}: tree children must be the chain's rows, in the chain's order",
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
            crate::folder_projection::ViewMode::Projected,
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
            crate::folder_projection::ViewMode::Projected,
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
        let mode = crate::folder_projection::ViewMode::Projected;
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", mode, limits);
        let sales = &tree.roots[0].children[0];
        let (_, rows, _) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            mode,
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
            crate::folder_projection::ViewMode::Projected,
            waml::view::chain::ChainLimits::default(),
        );
        let sales = &tree.roots[0].children[0];
        assert_eq!(sales.address.as_deref(), Some("/sales"));
        assert!(sales.view_degraded);
        assert!(!tree.roots[0].view_degraded);
    }

    /// Raw never builds the declared chain, so the folder tab opened in Raw
    /// shows no diagnostics at all. The tree must not contradict it by marking
    /// the same folder degraded.
    #[test]
    fn raw_does_not_mark_a_folder_degraded_the_folder_tab_reports_clean() {
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
        let mode = crate::folder_projection::ViewMode::Raw;
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback", mode, limits);
        let sales = &tree.roots[0].children[0];
        let (_, _, diagnostics) = crate::folder_projection::project_rows(
            prepared.okf(),
            "/sales",
            mode,
            limits,
            &crate::folder_projection::core_registry(),
        )
        .unwrap();
        assert!(diagnostics.is_empty(), "raw builds no declared chain");
        assert!(
            !sales.view_degraded,
            "the tree marked a folder degraded that the raw folder tab reports clean",
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
            crate::folder_projection::ViewMode::Projected,
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
            crate::folder_projection::ViewMode::Raw,
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
            crate::folder_projection::ViewMode::Raw,
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
            crate::folder_projection::ViewMode::Projected,
            waml::view::chain::ChainLimits::default(),
        );
        let domain = &tree.roots[0].children[0];

        assert_eq!(domain.kind, NavCategory::Directory);
        assert!(!domain.is_directory);
        assert!(domain.openable);
        assert!(crate::documents::open(prepared.okf(), prepared.uml(), "domain").is_some());
    }
}
