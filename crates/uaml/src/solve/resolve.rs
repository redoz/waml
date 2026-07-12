//! Layer A: resolve a `model::Diagram` into a syntax-free `Scene`.

use std::collections::{BTreeMap, BTreeSet};
use crate::diagnostic::Diagnostic;
use crate::model::{Diagram, DiagramGroup};
use crate::syntax::{Margin, Shape};
use super::{Box, BoxId, BoxKind, Constraint, FlagSet, Scene};

struct Builder {
    boxes: Vec<Box>,
    constraints: Vec<Constraint>,
    group_by_name: BTreeMap<String, BoxId>,
    node_keys: BTreeSet<String>,
    next_group: u32,
    next_inline: u32,
}

impl Builder {
    fn new() -> Builder {
        Builder {
            boxes: vec![],
            constraints: vec![],
            group_by_name: BTreeMap::new(),
            node_keys: BTreeSet::new(),
            next_group: 0,
            next_inline: 0,
        }
    }

    fn add_group(&mut self, g: &DiagramGroup, depth: u8) -> BoxId {
        let gid = self.next_group;
        self.next_group += 1;
        let id = BoxId::Group(gid);
        if !g.name.is_empty() {
            self.group_by_name.insert(g.name.clone(), id.clone());
        }
        let mut children = Vec::new();
        for key in &g.members {
            self.node_keys.insert(key.clone());
            let leaf = BoxId::Node(key.clone());
            if !self.boxes.iter().any(|b| b.id == leaf) {
                self.boxes.push(Box {
                    id: leaf.clone(),
                    kind: BoxKind::Leaf,
                    children: vec![],
                    axis: None,
                    shape: Shape::Shrink,
                    margin: Margin::Medium,
                    flags: FlagSet::default(),
                    title: None,
                    depth,
                });
            }
            children.push(leaf);
        }
        for child in &g.children {
            let cid = self.add_group(child, depth + 1);
            children.push(cid);
        }
        let title = if g.name.is_empty() { None } else { Some(g.name.clone()) };
        self.boxes.push(Box {
            id: id.clone(),
            kind: BoxKind::Group,
            children,
            axis: None,
            shape: Shape::Shrink,
            margin: Margin::Medium,
            flags: FlagSet::default(),
            title,
            depth,
        });
        id
    }
}

pub fn resolve(diagram: &Diagram) -> (Scene, Vec<Diagnostic>) {
    let mut b = Builder::new();
    for g in &diagram.groups {
        b.add_group(g, 0);
    }
    // Layout walk (constraints + treatment + warnings) arrives in Task 3.
    let diags = vec![];
    (Scene { boxes: b.boxes, constraints: b.constraints }, diags)
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use crate::model::{Diagram, DiagramGroup};
    use crate::solve::{BoxId, BoxKind};

    fn diagram(groups: Vec<DiagramGroup>, layout: Vec<crate::syntax::LayoutStatement>) -> Diagram {
        Diagram { key: "orders".into(), title: "Orders".into(), profile: "uml-domain".into(), groups, layout }
    }

    #[test]
    fn builds_box_forest_from_groups() {
        let d = diagram(
            vec![DiagramGroup {
                name: "Users".into(),
                members: vec!["customer".into(), "account".into()],
                children: vec![DiagramGroup { name: "VIP".into(), members: vec!["platinum".into()], children: vec![] }],
            }],
            vec![],
        );
        let (scene, diags) = resolve(&d);
        assert!(diags.is_empty());

        let users = scene.boxes.iter().find(|b| b.title.as_deref() == Some("Users")).unwrap();
        assert_eq!(users.kind, BoxKind::Group);
        assert_eq!(users.depth, 0);
        assert_eq!(
            users.children,
            vec![BoxId::Node("customer".into()), BoxId::Node("account".into()), BoxId::Group(1)]
        );

        let vip = scene.boxes.iter().find(|b| b.title.as_deref() == Some("VIP")).unwrap();
        assert_eq!(vip.depth, 1);
        assert!(scene.boxes.iter().any(|b| b.id == BoxId::Node("platinum".into()) && b.kind == BoxKind::Leaf));
    }
}
