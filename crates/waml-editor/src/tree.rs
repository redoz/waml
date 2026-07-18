//! The tree seam: flatten a `Model` into a `ProjectTree` for the panel.
//! Nothing here touches makepad; the `LiveId` bridge lives in `tree_panel.rs`.

use std::collections::HashMap;
use waml::model::{BehaviorKind, ElementType, Model, UmlMetaclass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeKind {
    Package,
    Class,
    Interface,
    Enum,
    DataType,
    Diagram,
    Behavior,
    Sequence,
    Note,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub key: String,
    pub title: String,
    pub kind: TreeKind,
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
/// `node_style::AccentBucket`'s granularity); Association/Actor/UseCase still
/// fold into the plain `Class` glyph, and `Sequence` behaviors get their own
/// glyph while other behavior kinds share the generic `Behavior` one.
fn kind_of(ty: &ElementType) -> TreeKind {
    match ty {
        ElementType::Uml(UmlMetaclass::Package) => TreeKind::Package,
        ElementType::Uml(UmlMetaclass::Note) => TreeKind::Note,
        ElementType::Uml(UmlMetaclass::Interface) => TreeKind::Interface,
        ElementType::Uml(UmlMetaclass::Enum) => TreeKind::Enum,
        ElementType::Uml(UmlMetaclass::DataType) => TreeKind::DataType,
        ElementType::Uml(
            UmlMetaclass::Class | UmlMetaclass::Association | UmlMetaclass::Actor | UmlMetaclass::UseCase,
        ) => TreeKind::Class,
        ElementType::Behavior(BehaviorKind::Sequence) => TreeKind::Sequence,
        ElementType::Behavior(_) => TreeKind::Behavior,
        ElementType::Diagram => TreeKind::Diagram,
        ElementType::Unknown(_) => TreeKind::Unknown,
    }
}

/// Flatten `model`'s package forest into a `ProjectTree`. Never empty: an
/// absent root package yields a flat diagram fallback.
pub fn build_tree(model: &Model) -> ProjectTree {
    // Unified key -> (title, kind) over all five collections.
    let mut meta: HashMap<String, (String, TreeKind)> = HashMap::new();
    for n in &model.nodes {
        let title = n.concept.title.clone().unwrap_or_else(|| n.key.clone());
        meta.insert(n.key.clone(), (title, kind_of(&n.ty)));
    }
    for d in &model.diagrams {
        meta.insert(d.key.clone(), (d.title.clone(), TreeKind::Diagram));
    }
    for p in &model.packages {
        let title = p.concept.title.clone().unwrap_or_else(|| p.key.clone());
        meta.insert(p.key.clone(), (title, TreeKind::Package));
    }
    for f in &model.flows {
        meta.insert(f.key.clone(), (f.title.clone(), TreeKind::Behavior));
    }
    for s in &model.interactions {
        meta.insert(s.key.clone(), (s.title.clone(), TreeKind::Behavior));
    }

    let root_title = if model.path.is_empty() {
        "bundle".to_string()
    } else {
        model.path.clone()
    };

    if let Some(root_pkg) = model.packages.iter().find(|p| p.key.is_empty()) {
        let root = TreeNode {
            key: String::new(),
            title: root_title,
            kind: TreeKind::Package,
            children: build_children(&root_pkg.members, model, &meta),
        };
        ProjectTree { roots: vec![root] }
    } else {
        let children = model
            .diagrams
            .iter()
            .map(|d| TreeNode {
                key: d.key.clone(),
                title: d.title.clone(),
                kind: TreeKind::Diagram,
                children: vec![],
            })
            .collect();
        ProjectTree {
            roots: vec![TreeNode {
                key: String::new(),
                title: root_title,
                kind: TreeKind::Package,
                children,
            }],
        }
    }
}

/// Resolve `members` in order through `meta`; recurse into sub-packages by
/// looking their own `members` up on `model.packages`. Unresolved keys are
/// dropped (`filter_map`), matching `reindex_bundle`'s behavior.
fn build_children(
    members: &[String],
    model: &Model,
    meta: &HashMap<String, (String, TreeKind)>,
) -> Vec<TreeNode> {
    members
        .iter()
        .filter_map(|k| {
            let (title, kind) = meta.get(k)?;
            let children = if *kind == TreeKind::Package {
                model
                    .packages
                    .iter()
                    .find(|p| &p.key == k)
                    .map(|p| build_children(&p.members, model, meta))
                    .unwrap_or_default()
            } else {
                vec![]
            };
            Some(TreeNode {
                key: k.clone(),
                title: title.clone(),
                kind: *kind,
                children,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load;
    use std::path::Path;
    use waml::model::{ElementType, Model, Node, UmlMetaclass};
    use waml::okf::Concept;

    fn mini() -> Model {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mini");
        load::load_model(&dir).unwrap()
    }

    // Depth-first flatten of (key, kind) pairs, for order-independent assertions.
    fn flatten(tree: &ProjectTree) -> Vec<(String, TreeKind)> {
        fn walk(nodes: &[TreeNode], out: &mut Vec<(String, TreeKind)>) {
            for n in nodes {
                out.push((n.key.clone(), n.kind));
                walk(&n.children, out);
            }
        }
        let mut out = Vec::new();
        walk(&tree.roots, &mut out);
        out
    }

    fn concept(title: &str) -> Concept {
        Concept {
            id: String::new(),
            ty: String::new(),
            title: Some(title.to_string()),
            description: None,
            resource: None,
            tags: vec![],
            timestamp: None,
            body: String::new(),
            links: vec![],
            citations: vec![],
            role: Default::default(),
            extra: Default::default(),
        }
    }

    fn node(key: &str, ty: ElementType, title: &str, members: Vec<&str>) -> Node {
        Node {
            concept: concept(title),
            key: key.to_string(),
            ty,
            stereotypes: vec![],
            abstract_: false,
            attributes: vec![],
            values: vec![],
            note_body: None,
            annotates: vec![],
            members: members.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn diagram(key: &str, title: &str) -> waml::model::Diagram {
        waml::model::Diagram {
            key: key.to_string(),
            title: title.to_string(),
            profile: "erd".to_string(),
            description: None,
            groups: vec![],
            layout: vec![],
            display: Default::default(),
        }
    }

    #[test]
    fn mini_fixture_has_single_labelled_root_with_the_diagram() {
        let model = mini();
        let tree = build_tree(&model);

        // One synthesized root package, titled from `model.path` ("Mini").
        assert_eq!(tree.roots.len(), 1);
        assert_eq!(tree.roots[0].key, "");
        assert_eq!(tree.roots[0].kind, TreeKind::Package);
        assert_eq!(tree.roots[0].title, "Mini");

        let flat = flatten(&tree);
        // The fixture's one diagram appears somewhere, as a Diagram leaf.
        let dkey = model.diagrams[0].key.clone();
        assert!(
            flat.iter()
                .any(|(k, kind)| *k == dkey && *kind == TreeKind::Diagram),
            "diagram {dkey:?} missing from {flat:?}"
        );
        // Every resolved row has a known kind (no dangling => Unknown leaks).
        assert!(flat.iter().all(|(_, kind)| *kind != TreeKind::Unknown));

        // `uml.Interface` nodes (PaymentGateway) resolve to the finer
        // `TreeKind::Interface`, not the coarse `Class` glyph.
        let payment_gateway = model
            .nodes
            .iter()
            .find(|n| n.concept.title.as_deref() == Some("PaymentGateway"))
            .expect("mini fixture has a PaymentGateway node");
        assert!(
            flat.iter()
                .any(|(k, kind)| *k == payment_gateway.key && *kind == TreeKind::Interface),
            "PaymentGateway ({:?}) missing TreeKind::Interface in {flat:?}",
            payment_gateway.key
        );
    }

    #[test]
    fn nested_packages_recurse_in_member_order() {
        let model = Model {
            path: "Root".to_string(),
            packages: vec![
                node(
                    "",
                    ElementType::Uml(UmlMetaclass::Package),
                    "Root",
                    vec!["sub"],
                ),
                node(
                    "sub",
                    ElementType::Uml(UmlMetaclass::Package),
                    "Sub Pkg",
                    vec!["cls"],
                ),
            ],
            nodes: vec![node(
                "cls",
                ElementType::Uml(UmlMetaclass::Class),
                "Cls",
                vec![],
            )],
            ..Default::default()
        };
        let tree = build_tree(&model);

        assert_eq!(tree.roots.len(), 1);
        let root = &tree.roots[0];
        assert_eq!(
            (root.key.as_str(), root.title.as_str(), root.kind),
            ("", "Root", TreeKind::Package)
        );

        assert_eq!(root.children.len(), 1);
        let sub = &root.children[0];
        assert_eq!(
            (sub.key.as_str(), sub.title.as_str(), sub.kind),
            ("sub", "Sub Pkg", TreeKind::Package)
        );

        assert_eq!(sub.children.len(), 1);
        let cls = &sub.children[0];
        assert_eq!(
            (cls.key.as_str(), cls.title.as_str(), cls.kind),
            ("cls", "Cls", TreeKind::Class)
        );
        assert!(cls.children.is_empty());
    }

    #[test]
    fn dangling_member_key_is_skipped_not_crashed() {
        let model = Model {
            path: "Root".to_string(),
            packages: vec![node(
                "",
                ElementType::Uml(UmlMetaclass::Package),
                "Root",
                vec!["ghost"], // resolves to nothing
            )],
            ..Default::default()
        };
        let tree = build_tree(&model);
        assert_eq!(tree.roots.len(), 1);
        assert!(tree.roots[0].children.is_empty());
    }

    #[test]
    fn empty_packages_falls_back_to_flat_diagram_list() {
        let model = Model {
            path: String::new(), // exercises the "bundle" fallback title
            diagrams: vec![diagram("d1", "D1"), diagram("d2", "D2")],
            ..Default::default()
        };
        let tree = build_tree(&model);

        assert_eq!(tree.roots.len(), 1);
        let root = &tree.roots[0];
        assert_eq!(
            (root.key.as_str(), root.title.as_str(), root.kind),
            ("", "bundle", TreeKind::Package)
        );
        assert_eq!(root.children.len(), 2);
        assert!(root
            .children
            .iter()
            .all(|c| c.kind == TreeKind::Diagram && c.children.is_empty()));
        assert_eq!(root.children[0].key, "d1");
        assert_eq!(root.children[1].key, "d2");
    }
}
