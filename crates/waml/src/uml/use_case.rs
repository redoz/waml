use crate::model::DiagramGroupRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseCaseMemberKind {
    Actor,
    UseCase,
    Note,
    ActorPackage,
    Incompatible,
}

pub(crate) struct UseCaseGroupInput {
    pub name: String,
    pub depth: usize,
    pub members: Vec<(String, UseCaseMemberKind)>,
    pub children: Vec<UseCaseGroupInput>,
}

pub(crate) struct UseCaseGroupVerdict {
    pub role: Option<DiagramGroupRole>,
    pub violations: Vec<UseCaseViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UseCaseViolation {
    InvalidGroup { group: String, reason: &'static str },
    ActorInsideBoundary { group: String, member: String },
    EmptyBand { group: String },
}

pub(crate) fn classify_group(input: &UseCaseGroupInput) -> UseCaseGroupVerdict {
    if input.depth == 0 {
        classify_top_level(input)
    } else {
        classify_band(input)
    }
}

fn classify_top_level(input: &UseCaseGroupInput) -> UseCaseGroupVerdict {
    let has_actor = input.members.iter().any(|(_, kind)| {
        matches!(
            kind,
            UseCaseMemberKind::Actor | UseCaseMemberKind::ActorPackage
        )
    });
    let has_direct_use_case = input
        .members
        .iter()
        .any(|(_, kind)| *kind == UseCaseMemberKind::UseCase);

    if has_actor && !has_direct_use_case {
        let mut violations = Vec::new();
        if input.members.iter().any(|(_, kind)| {
            !matches!(
                kind,
                UseCaseMemberKind::Actor
                    | UseCaseMemberKind::Note
                    | UseCaseMemberKind::ActorPackage
            )
        }) {
            violations.push(UseCaseViolation::InvalidGroup {
                group: input.name.clone(),
                reason: "an actor group has an incompatible member",
            });
        }
        if !input.children.is_empty() {
            violations.push(UseCaseViolation::InvalidGroup {
                group: input.name.clone(),
                reason: "an actor group cannot have a child group",
            });
        }
        return UseCaseGroupVerdict {
            role: violations
                .is_empty()
                .then_some(DiagramGroupRole::ExternalActors),
            violations,
        };
    }

    let has_nested_use_case = input.children.iter().any(group_contains_use_case);
    if !has_direct_use_case && !has_nested_use_case {
        return UseCaseGroupVerdict {
            role: None,
            violations: vec![UseCaseViolation::InvalidGroup {
                group: input.name.clone(),
                reason: "a top-level group must contain an actor or a use case",
            }],
        };
    }

    let mut violations = Vec::new();
    for (member, kind) in &input.members {
        match kind {
            UseCaseMemberKind::Actor | UseCaseMemberKind::ActorPackage => {
                violations.push(UseCaseViolation::ActorInsideBoundary {
                    group: input.name.clone(),
                    member: member.clone(),
                });
            }
            UseCaseMemberKind::UseCase | UseCaseMemberKind::Note => {}
            UseCaseMemberKind::Incompatible => {
                violations.push(UseCaseViolation::InvalidGroup {
                    group: input.name.clone(),
                    reason: "a system boundary has an incompatible member",
                });
            }
        }
    }
    for child in &input.children {
        violations.extend(classify_group(child).violations);
    }
    UseCaseGroupVerdict {
        role: violations
            .is_empty()
            .then_some(DiagramGroupRole::SystemBoundary),
        violations,
    }
}

fn classify_band(input: &UseCaseGroupInput) -> UseCaseGroupVerdict {
    let mut violations = Vec::new();
    let has_use_case = input
        .members
        .iter()
        .any(|(_, kind)| *kind == UseCaseMemberKind::UseCase);
    if !has_use_case {
        violations.push(UseCaseViolation::EmptyBand {
            group: input.name.clone(),
        });
    }
    for (member, kind) in &input.members {
        match kind {
            UseCaseMemberKind::Actor | UseCaseMemberKind::ActorPackage => {
                violations.push(UseCaseViolation::ActorInsideBoundary {
                    group: input.name.clone(),
                    member: member.clone(),
                });
            }
            UseCaseMemberKind::UseCase | UseCaseMemberKind::Note => {}
            UseCaseMemberKind::Incompatible => {
                violations.push(UseCaseViolation::InvalidGroup {
                    group: input.name.clone(),
                    reason: "a use-case band has an incompatible member",
                });
            }
        }
    }
    if !input.children.is_empty() {
        violations.push(UseCaseViolation::InvalidGroup {
            group: input.name.clone(),
            reason: "a use-case band cannot have a child group",
        });
    }
    UseCaseGroupVerdict {
        role: violations.is_empty().then_some(DiagramGroupRole::Band),
        violations,
    }
}

fn group_contains_use_case(input: &UseCaseGroupInput) -> bool {
    input
        .members
        .iter()
        .any(|(_, kind)| *kind == UseCaseMemberKind::UseCase)
        || input.children.iter().any(group_contains_use_case)
}
