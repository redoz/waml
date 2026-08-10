use crate::model::DiagramGroupRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseCaseMemberKind {
    Actor,
    UseCase,
    Note,
    ActorPackage,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UseCaseMemberSource {
    Declared(usize),
    Inline(usize),
}

pub(crate) struct UseCaseMemberInput {
    pub name: String,
    pub kind: UseCaseMemberKind,
    pub source: UseCaseMemberSource,
}

pub(crate) struct UseCaseGroupInput {
    pub name: String,
    pub path: Vec<usize>,
    pub depth: usize,
    pub members: Vec<UseCaseMemberInput>,
    pub children: Vec<UseCaseGroupInput>,
}

pub(crate) struct UseCaseGroupVerdict {
    pub role: Option<DiagramGroupRole>,
    pub is_system_boundary_candidate: bool,
    pub violations: Vec<UseCaseViolation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UseCaseViolation {
    InvalidGroup {
        group: String,
        path: Vec<usize>,
        reason: &'static str,
    },
    IncompatibleMember {
        group: String,
        path: Vec<usize>,
        member: String,
        source: UseCaseMemberSource,
    },
    ActorInsideBoundary {
        group: String,
        path: Vec<usize>,
        member: String,
        source: UseCaseMemberSource,
    },
    EmptyBand {
        group: String,
        path: Vec<usize>,
    },
}

pub(crate) fn classify_group(input: &UseCaseGroupInput) -> UseCaseGroupVerdict {
    if input.depth == 0 {
        classify_top_level(input)
    } else {
        classify_band(input)
    }
}

fn classify_top_level(input: &UseCaseGroupInput) -> UseCaseGroupVerdict {
    let has_actor = input.members.iter().any(|member| {
        matches!(
            member.kind,
            UseCaseMemberKind::Actor | UseCaseMemberKind::ActorPackage
        )
    });
    let is_system_boundary_candidate = group_contains_use_case(input);

    if !is_system_boundary_candidate && has_actor {
        let mut violations = Vec::new();
        for member in &input.members {
            if !matches!(
                member.kind,
                UseCaseMemberKind::Actor
                    | UseCaseMemberKind::Note
                    | UseCaseMemberKind::ActorPackage
            ) {
                violations.push(UseCaseViolation::IncompatibleMember {
                    group: input.name.clone(),
                    path: input.path.clone(),
                    member: member.name.clone(),
                    source: member.source,
                });
            }
        }
        for child in &input.children {
            violations.push(UseCaseViolation::InvalidGroup {
                group: child.name.clone(),
                path: child.path.clone(),
                reason: "an actor group cannot have a child group",
            });
        }
        return UseCaseGroupVerdict {
            role: violations
                .is_empty()
                .then_some(DiagramGroupRole::ExternalActors),
            is_system_boundary_candidate: false,
            violations,
        };
    }

    if !is_system_boundary_candidate {
        return UseCaseGroupVerdict {
            role: None,
            is_system_boundary_candidate: false,
            violations: vec![UseCaseViolation::InvalidGroup {
                group: input.name.clone(),
                path: input.path.clone(),
                reason: "a top-level group must contain an actor or a use case",
            }],
        };
    }

    let mut violations = Vec::new();
    for member in &input.members {
        match member.kind {
            UseCaseMemberKind::Actor | UseCaseMemberKind::ActorPackage => {
                violations.push(UseCaseViolation::ActorInsideBoundary {
                    group: input.name.clone(),
                    path: input.path.clone(),
                    member: member.name.clone(),
                    source: member.source,
                });
            }
            UseCaseMemberKind::UseCase | UseCaseMemberKind::Note => {}
            UseCaseMemberKind::Incompatible => {
                violations.push(UseCaseViolation::IncompatibleMember {
                    group: input.name.clone(),
                    path: input.path.clone(),
                    member: member.name.clone(),
                    source: member.source,
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
        is_system_boundary_candidate: true,
        violations,
    }
}

fn classify_band(input: &UseCaseGroupInput) -> UseCaseGroupVerdict {
    let mut violations = Vec::new();
    let has_use_case = input
        .members
        .iter()
        .any(|member| member.kind == UseCaseMemberKind::UseCase);
    if !has_use_case {
        violations.push(UseCaseViolation::EmptyBand {
            group: input.name.clone(),
            path: input.path.clone(),
        });
    }
    for member in &input.members {
        match member.kind {
            UseCaseMemberKind::Actor | UseCaseMemberKind::ActorPackage => {
                violations.push(UseCaseViolation::ActorInsideBoundary {
                    group: input.name.clone(),
                    path: input.path.clone(),
                    member: member.name.clone(),
                    source: member.source,
                });
            }
            UseCaseMemberKind::UseCase | UseCaseMemberKind::Note => {}
            UseCaseMemberKind::Incompatible => {
                violations.push(UseCaseViolation::IncompatibleMember {
                    group: input.name.clone(),
                    path: input.path.clone(),
                    member: member.name.clone(),
                    source: member.source,
                });
            }
        }
    }
    for child in &input.children {
        violations.push(UseCaseViolation::InvalidGroup {
            group: child.name.clone(),
            path: child.path.clone(),
            reason: "a use-case band cannot have a child group",
        });
    }
    UseCaseGroupVerdict {
        role: violations.is_empty().then_some(DiagramGroupRole::Band),
        is_system_boundary_candidate: false,
        violations,
    }
}

fn group_contains_use_case(input: &UseCaseGroupInput) -> bool {
    input
        .members
        .iter()
        .any(|member| member.kind == UseCaseMemberKind::UseCase)
        || input.children.iter().any(group_contains_use_case)
}
