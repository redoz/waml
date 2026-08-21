//! Use-case group projection: adapts declared member groups into the
//! `uml::use_case` classifier's input, applies the roles it returns and
//! reports the violations it finds.

use super::diagnostics::{declared_diagnostic, declared_diagnostic_range};
use super::syntax_util::trimmed_token_range;
use super::verdict::{
    inline_instance_validity, lower_member_group, InlineInstanceValidity, ValidInlineInstance,
};
use crate::uml::declared::DeclaredBundle;
use crate::{
    analysis::{AnalysisError, DomainAnalysisContext},
    diagnostic::Diagnostic,
};
use std::collections::{BTreeMap, BTreeSet};
use waml_syntax::{AstNode, TextRange};

fn use_case_member_kind(
    declared: &DeclaredBundle,
    concept_paths: &BTreeMap<String, String>,
    context: &DomainAnalysisContext<'_>,
    key: &str,
) -> crate::uml::use_case::UseCaseMemberKind {
    use crate::uml::use_case::UseCaseMemberKind;
    match context
        .okf
        .concept(key)
        .map(|concept| crate::model::ElementType::parse(&concept.ty))
    {
        Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::Actor)) => {
            UseCaseMemberKind::Actor
        }
        Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::UseCase)) => {
            UseCaseMemberKind::UseCase
        }
        Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::Note)) => {
            UseCaseMemberKind::Note
        }
        Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::Package))
            if package_contains_only_actors_and_notes(
                declared,
                concept_paths,
                context,
                key,
                &mut BTreeSet::new(),
            ) =>
        {
            UseCaseMemberKind::ActorPackage
        }
        _ => UseCaseMemberKind::Incompatible,
    }
}

fn package_contains_only_actors_and_notes(
    declared: &DeclaredBundle,
    concept_paths: &BTreeMap<String, String>,
    context: &DomainAnalysisContext<'_>,
    key: &str,
    visiting: &mut BTreeSet<String>,
) -> bool {
    if !visiting.insert(key.to_string()) {
        return false;
    }
    let Some(package) = declared.concept(key) else {
        visiting.remove(key);
        return false;
    };
    let path = concept_paths
        .get(key)
        .map(String::as_str)
        .unwrap_or_default();
    let mut has_actor = false;
    let mut compatible = true;
    for member in package.members.iter() {
        let crate::uml::DeclaredField::Valid { value, .. } = &member.target else {
            continue;
        };
        let target = crate::okf::resolve_href(path, value);
        match context
            .okf
            .concept(&target)
            .map(|concept| crate::model::ElementType::parse(&concept.ty))
        {
            Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::Actor)) => {
                has_actor = true;
            }
            Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::Note)) => {}
            Some(crate::model::ElementType::Uml(crate::model::UmlMetaclass::Package)) => {
                let nested = package_contains_only_actors_and_notes(
                    declared,
                    concept_paths,
                    context,
                    &target,
                    visiting,
                );
                has_actor |= nested;
                compatible &= nested;
            }
            Some(_) => compatible = false,
            None => {}
        }
    }
    visiting.remove(key);
    compatible && has_actor
}

fn use_case_group_input(
    declared: &DeclaredBundle,
    concept_paths: &BTreeMap<String, String>,
    context: &DomainAnalysisContext<'_>,
    group: &crate::uml::DeclaredMemberGroup,
    path: &str,
    claimed: &BTreeSet<&str>,
    group_path: Vec<usize>,
) -> Option<crate::uml::use_case::UseCaseGroupInput> {
    let depth = group_path.len().saturating_sub(1);
    let name = match &group.name {
        crate::uml::DeclaredField::Absent if depth == 0 => return None,
        crate::uml::DeclaredField::Absent => String::new(),
        crate::uml::DeclaredField::Valid { value, .. } => value.clone(),
        crate::uml::DeclaredField::Incomplete { .. }
        | crate::uml::DeclaredField::Invalid { .. } => return None,
    };
    let mut members = group
        .members
        .iter()
        .enumerate()
        .filter_map(|(index, member)| match &member.target {
            crate::uml::DeclaredField::Valid { value, .. } => {
                let target = crate::okf::resolve_href(path, value);
                claimed.contains(target.as_str()).then(|| {
                    let kind = use_case_member_kind(declared, concept_paths, context, &target);
                    crate::uml::use_case::UseCaseMemberInput {
                        name: target,
                        kind,
                        source: crate::uml::use_case::UseCaseMemberSource::Declared(index),
                    }
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    members.extend(
        group
            .inline_instances
            .iter()
            .enumerate()
            .filter_map(
                |(index, inline)| match inline_instance_validity(inline, path, claimed) {
                    InlineInstanceValidity::Valid(ValidInlineInstance { name, .. }) => {
                        Some(crate::uml::use_case::UseCaseMemberInput {
                            name: format!("{}#{name}", crate::okf::id_of(path)),
                            kind: crate::uml::use_case::UseCaseMemberKind::Incompatible,
                            source: crate::uml::use_case::UseCaseMemberSource::Inline(index),
                        })
                    }
                    InlineInstanceValidity::Invalid | InlineInstanceValidity::Unresolved => None,
                },
            ),
    );
    Some(crate::uml::use_case::UseCaseGroupInput {
        name,
        path: group_path.clone(),
        depth,
        members,
        children: group
            .children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                let mut child_path = group_path.clone();
                child_path.push(index);
                use_case_group_input(
                    declared,
                    concept_paths,
                    context,
                    child,
                    path,
                    claimed,
                    child_path,
                )
            })
            .collect(),
    })
}

fn apply_use_case_roles(
    group: &mut crate::model::DiagramGroup,
    input: &crate::uml::use_case::UseCaseGroupInput,
) {
    group.role = crate::uml::use_case::classify_group(input)
        .role
        .expect("validated use-case group must have a role");
    for (child, child_input) in group.children.iter_mut().zip(&input.children) {
        apply_use_case_roles(child, child_input);
    }
}

fn report_use_case_violation(
    context: &DomainAnalysisContext<'_>,
    concept: &crate::uml::DeclaredConcept,
    authored_path: &str,
    violation: &crate::uml::use_case::UseCaseViolation,
) -> Result<Diagnostic, AnalysisError> {
    use crate::uml::use_case::UseCaseViolation;
    let group_path = match violation {
        UseCaseViolation::InvalidGroup { path, .. }
        | UseCaseViolation::IncompatibleMember { path, .. }
        | UseCaseViolation::ActorInsideBoundary { path, .. }
        | UseCaseViolation::EmptyBand { path, .. } => path,
    };
    let declared_group = declared_group_at_path(concept, group_path).ok_or_else(|| {
        AnalysisError::CatalogInvariant {
            reason: "use-case violation points to no declared group".into(),
        }
    })?;
    let heading_range = declared_group
        .syntax
        .heading_token()
        .map(|token| token.range())
        .unwrap_or_else(|| declared_group.syntax.syntax().range());
    let (code, message, range) = match violation {
        UseCaseViolation::InvalidGroup {
            group: group_name,
            reason,
            ..
        } => (
            crate::diagnostic::DiagCode::InvalidUseCaseGroup,
            format!("use-case group '{group_name}' is invalid: {reason}"),
            heading_range,
        ),
        UseCaseViolation::IncompatibleMember {
            group: group_name,
            member,
            source,
            ..
        } => (
            crate::diagnostic::DiagCode::InvalidUseCaseGroup,
            format!("use-case group '{group_name}' has incompatible member '{member}'"),
            declared_member_target_range(declared_group, authored_path, member, *source)
                .unwrap_or(heading_range),
        ),
        UseCaseViolation::ActorInsideBoundary {
            group: group_name,
            member,
            source,
            ..
        } => (
            crate::diagnostic::DiagCode::ActorInsideSystemBoundary,
            format!("actor '{member}' is inside system boundary '{group_name}'"),
            declared_member_target_range(declared_group, authored_path, member, *source)
                .unwrap_or(heading_range),
        ),
        UseCaseViolation::EmptyBand {
            group: group_name, ..
        } => (
            crate::diagnostic::DiagCode::EmptyUseCaseBand,
            format!("use-case band '{group_name}' has no use case"),
            heading_range,
        ),
    };
    declared_diagnostic_range(context, authored_path, range, code, message, false)
}

fn declared_group_at_path<'a>(
    concept: &'a crate::uml::DeclaredConcept,
    path: &[usize],
) -> Option<&'a crate::uml::DeclaredMemberGroup> {
    let (first, rest) = path.split_first()?;
    let mut group = concept.member_groups.get(*first)?;
    for index in rest {
        group = group.children.get(*index)?;
    }
    Some(group)
}

fn declared_member_target_range(
    group: &crate::uml::DeclaredMemberGroup,
    authored_path: &str,
    target: &str,
    source: crate::uml::use_case::UseCaseMemberSource,
) -> Option<TextRange> {
    match source {
        crate::uml::use_case::UseCaseMemberSource::Declared(index) => {
            let member = group.members.get(index)?;
            let crate::uml::DeclaredField::Valid { value, .. } = &member.target else {
                return None;
            };
            (crate::okf::resolve_href(authored_path, value) == target)
                .then(|| member.syntax.target_token().map(|token| token.range()))
                .flatten()
        }
        crate::uml::use_case::UseCaseMemberSource::Inline(index) => {
            let inline = group.inline_instances.get(index)?;
            Some(
                inline
                    .syntax
                    .name_token()
                    .map(|token| trimmed_token_range(&token))
                    .unwrap_or_else(|| inline.syntax.syntax().range()),
            )
        }
    }
}

fn collect_group_members(
    group: &crate::uml::use_case::UseCaseGroupInput,
    members: &mut BTreeSet<String>,
) {
    members.extend(group.members.iter().map(|member| member.name.clone()));
    for child in &group.children {
        collect_group_members(child, members);
    }
}

pub(crate) fn project_use_case_groups(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    concept_paths: &BTreeMap<String, String>,
    concept: &crate::uml::DeclaredConcept,
    path: &str,
    claimed: &BTreeSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Option<Vec<crate::model::DiagramGroup>>, AnalysisError> {
    let mut valid = true;
    let mut projected = Vec::new();
    let mut boundary_counts = BTreeMap::<String, usize>::new();
    for (group_index, group) in concept.member_groups.iter().enumerate() {
        let Some(input) = use_case_group_input(
            declared,
            concept_paths,
            context,
            group,
            path,
            claimed,
            vec![group_index],
        ) else {
            continue;
        };
        let verdict = crate::uml::use_case::classify_group(&input);
        if verdict.is_system_boundary_candidate {
            let mut members = BTreeSet::new();
            collect_group_members(&input, &mut members);
            for member in members {
                if use_case_member_kind(declared, concept_paths, context, &member)
                    == crate::uml::use_case::UseCaseMemberKind::UseCase
                {
                    *boundary_counts.entry(member).or_default() += 1;
                }
            }
        }
        if !verdict.violations.is_empty() {
            valid = false;
            for violation in &verdict.violations {
                diagnostics.push(report_use_case_violation(
                    context, concept, path, violation,
                )?);
            }
            continue;
        }
        if let Some(mut group) = lower_member_group(group, path, claimed, &concept.concept_id) {
            apply_use_case_roles(&mut group, &input);
            projected.push(group);
        }
    }

    let mut use_cases = BTreeMap::<String, &crate::uml::DeclaredMember>::new();
    for member in concept.members.iter() {
        let crate::uml::DeclaredField::Valid { value, .. } = &member.target else {
            continue;
        };
        let target = crate::okf::resolve_href(path, value);
        if claimed.contains(target.as_str())
            && use_case_member_kind(declared, concept_paths, context, &target)
                == crate::uml::use_case::UseCaseMemberKind::UseCase
        {
            use_cases.entry(target).or_insert(member);
        }
    }
    for (use_case, member) in use_cases {
        match boundary_counts.get(&use_case).copied().unwrap_or_default() {
            0 => {
                valid = false;
                diagnostics.push(declared_diagnostic(
                    context,
                    path,
                    member.syntax.syntax(),
                    crate::diagnostic::DiagCode::UseCaseOutsideSystemBoundary,
                    format!("use case '{use_case}' is outside a system boundary"),
                    false,
                )?);
            }
            1 => {}
            _ => {
                valid = false;
                diagnostics.push(declared_diagnostic(
                    context,
                    path,
                    member.syntax.syntax(),
                    crate::diagnostic::DiagCode::UseCaseInMultipleSystemBoundaries,
                    format!("use case '{use_case}' is in more than one system boundary"),
                    false,
                )?);
            }
        }
    }
    Ok(valid.then_some(projected))
}
