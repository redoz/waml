use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::layout::{Axis, Direction, Shape};
use crate::model::{Diagram, DiagramGroup, DiagramGroupRole};

use super::{resolve, BoxId, Constraint, Scene};

pub struct UseCaseLayoutDefaults {
    pub group_shapes: BTreeMap<BoxId, Shape>,
    pub constraints: Vec<Constraint>,
}

pub fn defaults(
    diagram: &Diagram,
    resolved: &Scene,
    relationships: &[(BoxId, BoxId)],
) -> UseCaseLayoutDefaults {
    let mut group_shapes = BTreeMap::new();
    let mut constraints = Vec::new();
    let mut next_group = 0u32;
    let mut top = Vec::new();
    for group in &diagram.groups {
        let id = collect_group_defaults(
            group,
            &mut next_group,
            resolved,
            relationships,
            &mut group_shapes,
            &mut constraints,
        );
        top.push((id, group.role));
    }

    let actors = top
        .iter()
        .find(|(_, role)| *role == DiagramGroupRole::ExternalActors)
        .map(|(id, _)| id.clone());
    let boundary = top
        .iter()
        .find(|(_, role)| *role == DiagramGroupRole::SystemBoundary)
        .map(|(id, _)| id.clone());
    if let (Some(a), Some(b)) = (actors, boundary) {
        push_default_place(resolved, &mut constraints, a, b, Direction::LeftOf);
    }

    UseCaseLayoutDefaults {
        group_shapes,
        constraints,
    }
}

pub fn resolve_use_case(
    diagram: &Diagram,
    relationships: &[(BoxId, BoxId)],
) -> (Scene, Vec<Diagnostic>) {
    let (mut scene, diagnostics) = resolve::resolve(diagram);
    let defaults = defaults(diagram, &scene, relationships);
    for bx in &mut scene.boxes {
        if let Some(shape) = defaults.group_shapes.get(&bx.id) {
            if bx.shape == Shape::Shrink {
                bx.shape = *shape;
            }
        }
        if bx.axis.is_none() {
            bx.axis = default_axis(diagram, &bx.id);
        }
    }
    scene.constraints.extend(defaults.constraints);
    (scene, diagnostics)
}

fn collect_group_defaults(
    group: &DiagramGroup,
    next_group: &mut u32,
    resolved: &Scene,
    relationships: &[(BoxId, BoxId)],
    shapes: &mut BTreeMap<BoxId, Shape>,
    constraints: &mut Vec<Constraint>,
) -> BoxId {
    let id = BoxId::Group(*next_group);
    *next_group += 1;
    match group.role {
        DiagramGroupRole::SystemBoundary => {
            shapes.insert(id.clone(), Shape::Frame);
        }
        DiagramGroupRole::Band => {
            shapes.insert(id.clone(), Shape::Box);
        }
        DiagramGroupRole::Generic | DiagramGroupRole::ExternalActors => {}
    }

    let ordered = stable_member_order(&group.members, relationships);
    for pair in ordered.windows(2) {
        let direction = if group.role == DiagramGroupRole::ExternalActors {
            Direction::Above
        } else {
            Direction::LeftOf
        };
        push_default_place(
            resolved,
            constraints,
            BoxId::Node(pair[0].clone()),
            BoxId::Node(pair[1].clone()),
            direction,
        );
    }

    let mut children = Vec::new();
    for child in &group.children {
        children.push(collect_group_defaults(
            child,
            next_group,
            resolved,
            relationships,
            shapes,
            constraints,
        ));
    }
    for pair in children.windows(2) {
        push_default_place(
            resolved,
            constraints,
            pair[0].clone(),
            pair[1].clone(),
            Direction::Above,
        );
    }
    id
}

fn stable_member_order(members: &[String], relationships: &[(BoxId, BoxId)]) -> Vec<String> {
    let mut indexed = members
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let degree = relationships
                .iter()
                .filter(|(a, b)| *a == BoxId::Node(key.clone()) || *b == BoxId::Node(key.clone()))
                .count();
            (key.clone(), degree, index)
        })
        .collect::<Vec<_>>();
    indexed.sort_by(|a, b| b.1.cmp(&a.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    indexed.into_iter().map(|(key, _, _)| key).collect()
}

fn push_default_place(
    resolved: &Scene,
    constraints: &mut Vec<Constraint>,
    a: BoxId,
    b: BoxId,
    dir: Direction,
) {
    let authored_decides = resolved
        .constraints
        .iter()
        .any(|constraint| match constraint {
            Constraint::Place { a: x, b: y, .. } | Constraint::Align { a: x, b: y, .. } => {
                (x == &a && y == &b) || (x == &b && y == &a)
            }
        });
    if !authored_decides {
        constraints.push(Constraint::Place { a, b, dir });
    }
}

fn default_axis(diagram: &Diagram, id: &BoxId) -> Option<Axis> {
    let BoxId::Group(wanted) = id else {
        return None;
    };
    let mut next = 0u32;
    find_group(diagram.groups.as_slice(), *wanted, &mut next).map(|role| match role {
        DiagramGroupRole::ExternalActors => Axis::Column,
        DiagramGroupRole::SystemBoundary | DiagramGroupRole::Band => Axis::Row,
        DiagramGroupRole::Generic => Axis::Row,
    })
}

fn find_group(groups: &[DiagramGroup], wanted: u32, next: &mut u32) -> Option<DiagramGroupRole> {
    for group in groups {
        let current = *next;
        *next += 1;
        if current == wanted {
            return Some(group.role);
        }
        if let Some(role) = find_group(&group.children, wanted, next) {
            return Some(role);
        }
    }
    None
}
