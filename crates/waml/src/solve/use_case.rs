use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::layout::{Axis, Direction, Hint, LayoutStatement, NameRef, Operand, OperandRef, Shape};
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
    let actor_rank = actor_ranks(&diagram.groups);
    let authored_shapes = authored_shape_groups(diagram);
    for group in &diagram.groups {
        let id = collect_group_defaults(
            group,
            &mut next_group,
            resolved,
            relationships,
            &actor_rank,
            &authored_shapes,
            &mut group_shapes,
            &mut constraints,
        );
        top.push((id, group.role));
    }

    let actors = top
        .iter()
        .filter(|(_, role)| *role == DiagramGroupRole::ExternalActors)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let boundaries = top
        .iter()
        .filter(|(_, role)| *role == DiagramGroupRole::SystemBoundary)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    for actor in actors {
        for boundary in &boundaries {
            push_default_place(
                resolved,
                &mut constraints,
                actor.clone(),
                boundary.clone(),
                Direction::LeftOf,
            );
        }
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

#[allow(clippy::too_many_arguments)]
fn collect_group_defaults(
    group: &DiagramGroup,
    next_group: &mut u32,
    resolved: &Scene,
    relationships: &[(BoxId, BoxId)],
    actor_rank: &BTreeMap<String, usize>,
    authored_shapes: &BTreeSet<String>,
    shapes: &mut BTreeMap<BoxId, Shape>,
    constraints: &mut Vec<Constraint>,
) -> BoxId {
    let id = BoxId::Group(*next_group);
    *next_group += 1;
    match group.role {
        DiagramGroupRole::SystemBoundary => {
            if !authored_shapes.contains(&group.name) {
                shapes.insert(id.clone(), Shape::Frame);
            }
        }
        DiagramGroupRole::Band => {
            if !authored_shapes.contains(&group.name) {
                shapes.insert(id.clone(), Shape::Box);
            }
        }
        DiagramGroupRole::Generic | DiagramGroupRole::ExternalActors => {}
    }

    let ordered = if group.role == DiagramGroupRole::Band {
        stable_band_order(&group.members, relationships, actor_rank)
    } else {
        group.members.clone()
    };
    add_member_constraints(group.role, &ordered, resolved, constraints);

    let mut children = Vec::new();
    for child in &group.children {
        children.push(collect_group_defaults(
            child,
            next_group,
            resolved,
            relationships,
            actor_rank,
            authored_shapes,
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

fn stable_band_order(
    members: &[String],
    relationships: &[(BoxId, BoxId)],
    actor_rank: &BTreeMap<String, usize>,
) -> Vec<String> {
    let mut indexed = members
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let adjacent_actor = relationships
                .iter()
                .filter_map(|(a, b)| match (a, b) {
                    (BoxId::Node(left), BoxId::Node(right)) if left == key => actor_rank.get(right),
                    (BoxId::Node(left), BoxId::Node(right)) if right == key => actor_rank.get(left),
                    _ => None,
                })
                .copied()
                .min()
                .unwrap_or(usize::MAX);
            (key.clone(), adjacent_actor, index)
        })
        .collect::<Vec<_>>();
    indexed.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)).then(a.0.cmp(&b.0)));
    indexed.into_iter().map(|(key, _, _)| key).collect()
}

fn add_member_constraints(
    role: DiagramGroupRole,
    ordered: &[String],
    resolved: &Scene,
    constraints: &mut Vec<Constraint>,
) {
    if role == DiagramGroupRole::ExternalActors {
        for pair in ordered.windows(2) {
            push_default_place(
                resolved,
                constraints,
                BoxId::Node(pair[0].clone()),
                BoxId::Node(pair[1].clone()),
                Direction::Above,
            );
        }
        return;
    }
    let columns = if ordered.len() <= 4 {
        ordered.len().max(1)
    } else {
        (ordered.len() as f64).sqrt().ceil() as usize
    };
    for row in ordered.chunks(columns) {
        for pair in row.windows(2) {
            push_default_place(
                resolved,
                constraints,
                BoxId::Node(pair[0].clone()),
                BoxId::Node(pair[1].clone()),
                Direction::LeftOf,
            );
        }
    }
    for index in columns..ordered.len() {
        push_default_place(
            resolved,
            constraints,
            BoxId::Node(ordered[index - columns].clone()),
            BoxId::Node(ordered[index].clone()),
            Direction::Above,
        );
    }
}

fn actor_ranks(groups: &[DiagramGroup]) -> BTreeMap<String, usize> {
    groups
        .iter()
        .filter(|group| group.role == DiagramGroupRole::ExternalActors)
        .flat_map(|group| group.members.iter())
        .enumerate()
        .map(|(index, key)| (key.clone(), index))
        .collect()
}

fn authored_shape_groups(diagram: &Diagram) -> BTreeSet<String> {
    let mut groups = BTreeSet::new();
    for statement in &diagram.layout {
        match statement {
            LayoutStatement::Standalone(operand) => collect_shape_operand(operand, &mut groups),
            LayoutStatement::Placement { operands, .. } => {
                for operand in operands {
                    collect_shape_operand(operand, &mut groups);
                }
            }
            LayoutStatement::Alignment { left, right } => {
                collect_shape_operand(&left.operand, &mut groups);
                collect_shape_operand(&right.operand, &mut groups);
            }
        }
    }
    groups
}

fn collect_shape_operand(operand: &Operand, groups: &mut BTreeSet<String>) {
    if operand
        .hints
        .iter()
        .any(|hint| matches!(hint, Hint::Shape(_)))
    {
        if let OperandRef::Name(NameRef::Bare(name)) = &operand.ref_ {
            groups.insert(name.clone());
        }
    }
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
    let group = find_group(diagram.groups.as_slice(), *wanted, &mut next)?;
    match group.role {
        DiagramGroupRole::ExternalActors => Some(Axis::Column),
        DiagramGroupRole::SystemBoundary if !group.children.is_empty() => Some(Axis::Column),
        DiagramGroupRole::SystemBoundary | DiagramGroupRole::Band if group.members.len() > 4 => {
            None
        }
        DiagramGroupRole::SystemBoundary | DiagramGroupRole::Band | DiagramGroupRole::Generic => {
            Some(Axis::Row)
        }
    }
}

fn find_group<'a>(
    groups: &'a [DiagramGroup],
    wanted: u32,
    next: &mut u32,
) -> Option<&'a DiagramGroup> {
    for group in groups {
        let current = *next;
        *next += 1;
        if current == wanted {
            return Some(group);
        }
        if let Some(role) = find_group(&group.children, wanted, next) {
            return Some(role);
        }
    }
    None
}
