//! The tree seam: flatten a `Model` into a `ProjectTree` for the panel.
//! Nothing here touches makepad; the `LiveId` bridge lives in `tree_panel.rs`.

use crate::document::{DocumentPresentation, NavCategory};
use crate::icons::Icon;
use waml::model::{BehaviorKind, ElementType, UmlMetaclass};

pub type TreeKind = NavCategory;

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub key: String,
    pub title: String,
    pub kind: TreeKind,
    pub presentation: DocumentPresentation,
    pub is_directory: bool,
    pub openable: bool,
    pub concept_id: Option<String>,
    pub can_edit_classifier: bool,
    pub can_delete_classifier: bool,
    /// Directory rows only: whether this folder's declared `view:` chain
    /// failed to build (an unknown middleware name, bad params) and fell
    /// back to the root view. Drives the tree's degraded-chain marker so a
    /// folder inside a collapsed subtree is not silently wrong. Always
    /// `false` for a non-directory row.
    pub view_degraded: bool,
    pub children: Vec<TreeNode>,
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
) -> ProjectTree {
    fn directory_node(
        okf: &waml::analysis::OkfAnalysis,
        uml_analysis: &waml::uml::Analysis,
        address: &waml::okf::DirectoryAddress,
        root_fallback: &str,
    ) -> Option<TreeNode> {
        let bundle = &okf.bundle;
        let directory = bundle
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
        let concept_node = |concept_id: &str| {
            let concept = bundle.concept(concept_id)?;
            let descriptor = crate::documents::describe(okf, uml_analysis, concept_id)?;
            let presentation = descriptor.presentation;
            Some(TreeNode {
                key: concept_id.to_owned(),
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
                can_edit_classifier: descriptor.capabilities.can_edit_classifier,
                can_delete_classifier: descriptor.capabilities.can_delete_classifier,
                view_degraded: false,
                children: Vec::new(),
            })
        };
        let mut children = Vec::new();
        let mut seen = std::collections::HashSet::new();
        if let Some(index) = index {
            for member in &index.members {
                if let Some(child) = directory
                    .child_directories
                    .iter()
                    .find(|child| child.as_str() == member)
                {
                    if let Some(row) = directory_node(okf, uml_analysis, child, root_fallback) {
                        seen.insert(member.clone());
                        children.push(row);
                    }
                } else if directory.concepts.iter().any(|concept| concept == member) {
                    if let Some(row) = concept_node(member) {
                        seen.insert(member.clone());
                        children.push(row);
                    }
                }
            }
        }
        for child in &directory.child_directories {
            if seen.insert(child.as_str().to_owned()) {
                if let Some(row) = directory_node(okf, uml_analysis, child, root_fallback) {
                    children.push(row);
                }
            }
        }
        for concept_id in &directory.concepts {
            if seen.insert(concept_id.clone()) {
                if let Some(row) = concept_node(concept_id) {
                    children.push(row);
                }
            }
        }
        // Build-level diagnostics only (unknown middleware name, bad params):
        // `Chain::build` catches these without running anything. Must be the
        // SAME registry `FolderView::build` uses, or the tree marks a folder
        // degraded that opens fine, or misses one that does not.
        let registry = crate::folder_view::core_registry();
        let (_, diagnostics) = bundle.resolved_view(address.as_str(), &registry);
        Some(TreeNode {
            key: address.as_str().to_string(),
            title,
            kind: NavCategory::Directory,
            presentation,
            is_directory: true,
            openable: false,
            concept_id: None,
            can_edit_classifier: false,
            can_delete_classifier: false,
            view_degraded: !diagnostics.is_empty(),
            children,
        })
    }

    let root = waml::okf::DirectoryAddress::parse("/").expect("root address is valid");
    ProjectTree {
        roots: directory_node(okf, uml_analysis, &root, root_fallback)
            .into_iter()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

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
        let tree = build_tree(&bundle, &projection, "Fallback");
        let root = &tree.roots[0];
        assert_eq!((root.key.as_str(), root.title.as_str()), ("/", "Root"));
        let sales = &root.children[0];
        assert_eq!(
            (sales.key.as_str(), sales.title.as_str()),
            ("/sales", "Sales")
        );
        assert_eq!(
            sales
                .children
                .iter()
                .map(|row| row.key.as_str())
                .collect::<Vec<_>>(),
            ["sales/order", "/sales/archive", "sales/runbook"]
        );
    }

    #[test]
    fn providers_decorate_claimed_and_generic_rows_with_capabilities() {
        let (bundle, projection) = mixed();
        let tree = build_tree(&bundle, &projection, "Fallback");
        let rows = &tree.roots[0].children[0].children;
        let order = rows.iter().find(|row| row.key == "sales/order").unwrap();
        assert_eq!(order.kind, NavCategory::Class);
        assert!(order.openable && order.can_edit_classifier);
        assert_eq!(order.concept_id.as_deref(), Some("sales/order"));
        let runbook = rows.iter().find(|row| row.key == "sales/runbook").unwrap();
        assert_eq!(runbook.kind, NavCategory::OkfDocument);
        assert!(runbook.openable);
        assert!(!runbook.can_edit_classifier && !runbook.can_delete_classifier);
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
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback");
        let sales = &tree.roots[0].children[0];
        assert_eq!(sales.key, "/sales");
        assert!(sales.view_degraded);
        assert!(!tree.roots[0].view_degraded);
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
        let tree = build_tree(prepared.okf(), prepared.uml(), "Fallback");
        let domain = &tree.roots[0].children[0];

        assert_eq!(domain.kind, NavCategory::Directory);
        assert!(!domain.is_directory);
        assert!(domain.openable);
        assert!(crate::documents::open(prepared.okf(), prepared.uml(), "domain").is_some());
    }
}
