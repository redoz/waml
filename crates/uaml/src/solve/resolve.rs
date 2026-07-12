//! Layer A: resolve a `model::Diagram` into a syntax-free `Scene`.

use std::collections::{BTreeMap, BTreeSet};
use crate::diagnostic::Diagnostic;
use crate::model::{Diagram, DiagramGroup};
use crate::diagnostic::DiagCode;
use crate::slug::slugify;
use crate::syntax::{Edge, Flag, Hint, LayoutStatement, Margin, NameRef, Operand, OperandRef, Shape};
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

    fn box_mut(&mut self, id: &BoxId) -> Option<&mut Box> {
        self.boxes.iter_mut().find(|b| &b.id == id)
    }

    fn apply_treatment(&mut self, id: &BoxId, op: &Operand) {
        if let Some(bx) = self.box_mut(id) {
            if let Some(ax) = op.axis {
                bx.axis = Some(ax);
            }
            for h in &op.hints {
                match h {
                    Hint::Shape(s) => bx.shape = *s,
                    Hint::Margin(m) => bx.margin = *m,
                    Hint::Flag(Flag::Emphasized) => bx.flags.emphasized = true,
                    Hint::Flag(Flag::Collapsed) => bx.flags.collapsed = true,
                }
            }
        }
    }

    fn resolve_ref(&mut self, r: &OperandRef, file: &str, diags: &mut Vec<Diagnostic>) -> Option<BoxId> {
        match r {
            OperandRef::Name(NameRef::Link { slug, .. }) => {
                if self.node_keys.contains(slug) {
                    Some(BoxId::Node(slug.clone()))
                } else {
                    self.warn_unknown(slug, file, diags);
                    None
                }
            }
            OperandRef::Name(NameRef::Bare(name)) => {
                if let Some(id) = self.group_by_name.get(name) {
                    return Some(id.clone());
                }
                let slug = slugify(name, "");
                if self.node_keys.contains(&slug) {
                    Some(BoxId::Node(slug))
                } else {
                    self.warn_unknown(name, file, diags);
                    None
                }
            }
            OperandRef::InlineGroup { axis, items } => {
                let iid = self.next_inline;
                self.next_inline += 1;
                let id = BoxId::Inline(iid);
                let mut children = Vec::new();
                for it in items {
                    if let Some(cid) = self.resolve_operand(it, file, diags) {
                        children.push(cid);
                    }
                }
                self.boxes.push(Box {
                    id: id.clone(),
                    kind: BoxKind::Group,
                    children,
                    axis: Some(*axis),
                    shape: Shape::Shrink,
                    margin: Margin::Medium,
                    flags: FlagSet::default(),
                    title: None,
                    depth: 0,
                });
                Some(id)
            }
            OperandRef::Paren(inner) => self.resolve_operand(inner, file, diags),
        }
    }

    fn resolve_operand(&mut self, op: &Operand, file: &str, diags: &mut Vec<Diagnostic>) -> Option<BoxId> {
        let id = self.resolve_ref(&op.ref_, file, diags)?;
        self.apply_treatment(&id, op);
        Some(id)
    }

    fn warn_unknown(&self, name: &str, file: &str, diags: &mut Vec<Diagnostic>) {
        diags.push(Diagnostic::warn(
            DiagCode::UnresolvedLayoutRef,
            format!("layout references unknown operand `{name}`"),
            file,
            0,
        ));
    }

    fn add_statement(&mut self, stmt: &LayoutStatement, file: &str, diags: &mut Vec<Diagnostic>) {
        match stmt {
            LayoutStatement::Standalone(op) => {
                self.resolve_operand(op, file, diags);
            }
            LayoutStatement::Placement { operands, directions } => {
                let ids: Vec<Option<BoxId>> =
                    operands.iter().map(|o| self.resolve_operand(o, file, diags)).collect();
                for (i, dir) in directions.iter().enumerate() {
                    if let (Some(a), Some(b)) = (&ids[i], &ids[i + 1]) {
                        self.constraints.push(Constraint::Place { a: a.clone(), b: b.clone(), dir: *dir });
                    }
                }
            }
            LayoutStatement::Alignment { left, right } => {
                let a = self.resolve_operand(&left.operand, file, diags);
                let b = self.resolve_operand(&right.operand, file, diags);
                if let (Some(a), Some(b)) = (a, b) {
                    self.constraints.push(Constraint::Align {
                        a,
                        a_edge: left.edge.unwrap_or(Edge::Center),
                        b,
                        b_edge: right.edge.unwrap_or(Edge::Center),
                    });
                }
            }
        }
    }
}

pub fn resolve(diagram: &Diagram) -> (Scene, Vec<Diagnostic>) {
    let mut b = Builder::new();
    for g in &diagram.groups {
        b.add_group(g, 0);
    }
    let mut diags = vec![];
    for stmt in &diagram.layout {
        b.add_statement(stmt, &diagram.key, &mut diags);
    }
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

    #[test]
    fn resolves_refs_treatment_and_warns_unknown() {
        use crate::solve::Constraint;
        use crate::syntax::*;

        fn bare(name: &str) -> Operand {
            Operand { ref_: OperandRef::Name(NameRef::Bare(name.into())), axis: None, hints: vec![] }
        }
        let users_treated = Operand {
            ref_: OperandRef::Name(NameRef::Bare("Users".into())),
            axis: Some(Axis::Column),
            hints: vec![Hint::Shape(Shape::Frame)],
        };

        let d = diagram(
            vec![
                DiagramGroup { name: "Users".into(), members: vec!["customer".into(), "account".into()], children: vec![] },
                DiagramGroup { name: "Orders".into(), members: vec!["order".into()], children: vec![] },
            ],
            vec![
                LayoutStatement::Standalone(users_treated),
                LayoutStatement::Placement { operands: vec![bare("Users"), bare("Orders")], directions: vec![Direction::LeftOf] },
                LayoutStatement::Placement { operands: vec![bare("Nope"), bare("Orders")], directions: vec![Direction::LeftOf] },
            ],
        );
        let (scene, diags) = resolve(&d);

        let users = scene.boxes.iter().find(|b| b.title.as_deref() == Some("Users")).unwrap();
        assert_eq!(users.axis, Some(Axis::Column));
        assert_eq!(users.shape, Shape::Frame);

        assert_eq!(scene.constraints.len(), 1);
        assert_eq!(
            scene.constraints[0],
            Constraint::Place { a: BoxId::Group(0), b: BoxId::Group(1), dir: Direction::LeftOf }
        );

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, crate::diagnostic::DiagCode::UnresolvedLayoutRef);
    }

    #[test]
    fn resolves_bare_node_by_slug_and_inline_group() {
        use crate::syntax::*;
        let inline = Operand {
            ref_: OperandRef::InlineGroup {
                axis: Axis::Column,
                items: vec![Operand { ref_: OperandRef::Name(NameRef::Bare("Order Line".into())), axis: None, hints: vec![] }],
            },
            axis: None,
            hints: vec![Hint::Margin(Margin::Large)],
        };
        let d = diagram(
            vec![DiagramGroup { name: "".into(), members: vec!["order-line".into()], children: vec![] }],
            vec![LayoutStatement::Standalone(inline)],
        );
        let (scene, diags) = resolve(&d);
        assert!(diags.is_empty(), "`Order Line` slugifies to order-line and resolves");
        let ig = scene.boxes.iter().find(|b| b.id == BoxId::Inline(0)).unwrap();
        assert_eq!(ig.axis, Some(Axis::Column));
        assert_eq!(ig.margin, Margin::Large);
        assert_eq!(ig.children, vec![BoxId::Node("order-line".into())]);
    }
}
