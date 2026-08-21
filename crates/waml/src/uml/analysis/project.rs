//! Projection: the declared bundle becomes a `Projection` of the domain
//! model. Admission consumes the verdicts in `verdict`; it never derives a
//! rule of its own.

use super::behavior::lower_flow_behavior;
use super::diagnostics::declared_diagnostic_range;
use super::link::{normalize_slot_value, parse_link_in_text, resolve_slug};
use super::use_case_groups::project_use_case_groups;
use super::verdict::{
    inline_instance_validity, lower_member_group, relationship_end_verdict, EndVerdict,
    InlineInstanceValidity, ValidInlineInstance,
};
use crate::uml::{
    declared::{DeclaredAttribute, DeclaredBundle},
    syntax,
};
use crate::{
    analysis::{AnalysisError, DomainAnalysisContext},
    diagnostic::Diagnostic,
};
use std::collections::{BTreeMap, BTreeSet};
use waml_syntax::AstNode;

pub(crate) fn declared_projection(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    concept_paths: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<crate::uml::Projection, AnalysisError> {
    let claimed: BTreeSet<_> = declared.concepts().map(|c| c.concept_id.as_str()).collect();
    let mut model = crate::model::Model::default();
    for concept in declared.concepts() {
        let okf = context.okf.concept(&concept.concept_id).ok_or_else(|| {
            AnalysisError::CatalogInvariant {
                reason: format!(
                    "declared concept is not a claimed OKF concept: {}",
                    concept.concept_id
                )
                .into(),
            }
        })?;
        let path = concept_paths
            .get(concept.concept_id.as_str())
            .cloned()
            .unwrap_or_default();
        let attributes = concept
            .attributes
            .iter()
            .filter_map(DeclaredAttribute::validated)
            .collect();
        let values = concept
            .values
            .iter()
            .filter_map(|v| match &v.value {
                crate::uml::DeclaredField::Valid { value, .. } => Some(value.clone()),
                _ => None,
            })
            .collect();
        let slots = concept
            .slots
            .iter()
            .filter_map(|s| match (&s.name, &s.value) {
                (
                    crate::uml::DeclaredField::Valid { value: name, .. },
                    crate::uml::DeclaredField::Valid { value, .. },
                ) => {
                    let (value, ref_) = match s.syntax.value_kind() {
                        syntax::SlotValueKind::Quoted => (
                            value
                                .strip_prefix('"')
                                .and_then(|value| value.strip_suffix('"'))
                                .unwrap_or(value)
                                .to_owned(),
                            None,
                        ),
                        syntax::SlotValueKind::Link => {
                            let range = s.syntax.syntax().range();
                            let authored = crate::source::BundlePath::parse(path.clone())
                                .ok()
                                .and_then(|bundle_path| context.source.document(&bundle_path))
                                .map(|document| {
                                    &document.text()
                                        [range.start().to_usize()..range.end().to_usize()]
                                })
                                .unwrap_or(value);
                            let link = parse_link_in_text(authored);
                            (
                                link.as_ref()
                                    .map(|link| link.title.clone())
                                    .unwrap_or_else(|| value.clone()),
                                link.and_then(|link| resolve_slug(&path, &link.slug, &claimed)),
                            )
                        }
                        syntax::SlotValueKind::Bare => (value.clone(), None),
                        syntax::SlotValueKind::Missing | syntax::SlotValueKind::Invalid => {
                            (value.clone(), None)
                        }
                    };
                    Some(crate::model::Slot {
                        name: name.clone(),
                        value,
                        ref_,
                    })
                }
                _ => None,
            })
            .collect();
        let ty = crate::model::ElementType::parse(&okf.ty);
        if matches!(
            ty,
            crate::model::ElementType::Diagram(
                crate::model::DiagramKind::Activity | crate::model::DiagramKind::StateMachine
            )
        ) {
            lower_flow_behavior(
                context,
                concept,
                okf,
                &path,
                &claimed,
                &mut model,
                diagnostics,
            );
        } else if ty == crate::model::ElementType::Diagram(crate::model::DiagramKind::Sequence) {
            crate::uml::sequence::lower(
                context,
                declared,
                concept,
                okf,
                &path,
                &claimed,
                concept_paths,
                &mut model,
                diagnostics,
            );
        } else if let crate::model::ElementType::Diagram(
            kind @ (crate::model::DiagramKind::Class | crate::model::DiagramKind::UseCase),
        ) = ty
        {
            for member in concept.members.iter() {
                let crate::uml::DeclaredField::Valid { value: href, .. } = &member.target else {
                    continue;
                };
                let target = crate::okf::resolve_href(&path, href);
                if claimed.contains(target.as_str()) {
                    continue;
                }
                let range = member
                    .syntax
                    .target_token()
                    .ok_or_else(|| AnalysisError::CatalogInvariant {
                        reason: "valid member has no target token".into(),
                    })?
                    .range();
                diagnostics.push(declared_diagnostic_range(
                    context,
                    &path,
                    range,
                    crate::diagnostic::DiagCode::UnresolvedTarget,
                    format!("unresolved UML member '{href}'"),
                    true,
                )?);
            }
            let groups = match kind {
                crate::model::DiagramKind::Class => concept
                    .member_groups
                    .iter()
                    .filter_map(|group| {
                        lower_member_group(group, &path, &claimed, &concept.concept_id)
                    })
                    .collect(),
                crate::model::DiagramKind::UseCase => {
                    let Some(groups) = project_use_case_groups(
                        context,
                        declared,
                        concept_paths,
                        concept,
                        &path,
                        &claimed,
                        diagnostics,
                    )?
                    else {
                        continue;
                    };
                    groups
                }
                crate::model::DiagramKind::Activity
                | crate::model::DiagramKind::StateMachine
                | crate::model::DiagramKind::Sequence => unreachable!(
                    "structural diagram branch only accepts class and use-case diagrams"
                ),
            };
            model.diagrams.push(crate::model::Diagram {
                key: concept.concept_id.clone(),
                title: okf
                    .title
                    .clone()
                    .unwrap_or_else(|| concept.concept_id.clone()),
                kind,
                profile: okf.extra.get_str("profile").unwrap_or_default().to_string(),
                description: okf.description.clone(),
                groups,
                layout: concept
                    .layout
                    .iter()
                    .filter_map(|field| match field {
                        crate::uml::DeclaredField::Valid {
                            value:
                                crate::uml::DeclaredLayoutStatement::Placement {
                                    operands,
                                    directions,
                                },
                            ..
                        } => Some(crate::layout::LayoutStatement::Placement {
                            operands: operands
                                .iter()
                                .filter_map(|field| match field {
                                    crate::uml::DeclaredField::Valid { value, .. } => {
                                        Some(value.clone())
                                    }
                                    _ => None,
                                })
                                .collect(),
                            directions: directions
                                .iter()
                                .filter_map(|field| match field {
                                    crate::uml::DeclaredField::Valid { value, .. } => Some(*value),
                                    _ => None,
                                })
                                .collect(),
                        }),
                        crate::uml::DeclaredField::Valid {
                            value: crate::uml::DeclaredLayoutStatement::Alignment { left, right },
                            ..
                        } => Some(crate::layout::LayoutStatement::Alignment {
                            left: match left {
                                crate::uml::DeclaredField::Valid { value, .. } => value.clone(),
                                _ => return None,
                            },
                            right: match right {
                                crate::uml::DeclaredField::Valid { value, .. } => value.clone(),
                                _ => return None,
                            },
                        }),
                        crate::uml::DeclaredField::Valid {
                            value:
                                crate::uml::DeclaredLayoutStatement::Standalone(
                                    crate::uml::DeclaredField::Valid { value, .. },
                                ),
                            ..
                        } => Some(crate::layout::LayoutStatement::Standalone(value.clone())),
                        _ => None,
                    })
                    .collect(),
                display: {
                    let frontmatter = &okf.extra;
                    let max_attributes = match frontmatter.get("maxAttributes") {
                        Some(crate::frontmatter::FmValue::Num(value)) if *value > 0.0 => {
                            Some(*value as u32)
                        }
                        _ => None,
                    };
                    let legacy_attribute_gate = frontmatter.get_bool("showAttributeMultiplicity");
                    let cardinality = frontmatter
                        .get_str("cardinality")
                        .and_then(|value| match value {
                            "off" => Some(crate::model::CardinalityVisibility::Off),
                            "explicit" => Some(crate::model::CardinalityVisibility::Explicit),
                            "all" => Some(crate::model::CardinalityVisibility::All),
                            _ => None,
                        })
                        .or_else(|| {
                            legacy_attribute_gate.map(
                                crate::model::CardinalityVisibility::from_legacy_attribute_gate,
                            )
                        });
                    crate::model::DiagramDisplay {
                        show_attributes: frontmatter.get_bool("showAttributes"),
                        show_type: frontmatter.get_bool("showType").or_else(|| {
                            frontmatter
                                .get_str("attributeDetail")
                                .map(|value| value == "name-type")
                        }),
                        show_attribute_visibility: frontmatter.get_bool("showAttributeVisibility"),
                        show_attribute_multiplicity: cardinality
                            .map(crate::model::CardinalityVisibility::legacy_attribute_gate),
                        cardinality,
                        max_attributes,
                        show_roles: frontmatter.get_bool("showRoles"),
                        show_cardinality: frontmatter.get_bool("showCardinality"),
                        show_labels: frontmatter.get_bool("showLabels"),
                        show_stereotype: frontmatter.get_bool("showStereotype"),
                        stereotype_filter: frontmatter
                            .get("stereotypeFilter")
                            .map(|_| frontmatter.get_string_list("stereotypeFilter")),
                        stereotype_colors: frontmatter.get_string_list("stereotypeColors"),
                    }
                },
            });
        } else {
            model.nodes.push(crate::model::Node {
                concept: okf.clone(),
                key: concept.concept_id.clone(),
                ty,
                stereotypes: okf.extra.get_string_list("stereotype"),
                abstract_: okf.extra.get_bool("abstract") == Some(true),
                attributes,
                values,
                note_body: None,
                annotates: vec![],
                members: vec![],
                slots,
            });
        }
        for relationship in concept.relationships.iter() {
            let (
                crate::uml::DeclaredField::Valid { value: kind, .. },
                crate::uml::DeclaredField::Valid { value: href, .. },
            ) = (&relationship.kind, &relationship.target)
            else {
                continue;
            };
            let ends_valid =
                relationship_end_verdict(*kind, &relationship.from_end, &relationship.to_end)
                    == EndVerdict::Ok;
            if !ends_valid
                || matches!(
                    relationship.name,
                    crate::uml::DeclaredField::Incomplete { .. }
                        | crate::uml::DeclaredField::Invalid { .. }
                )
            {
                continue;
            }
            let target = crate::okf::resolve_href(&path, href);
            if !claimed.contains(target.as_str()) {
                if matches!(
                    kind,
                    crate::model::RelationshipKind::InstanceOf
                        | crate::model::RelationshipKind::Links
                ) {
                    continue;
                }
                let range = relationship
                    .syntax
                    .target_token()
                    .ok_or_else(|| AnalysisError::CatalogInvariant {
                        reason: "valid relationship has no target token".into(),
                    })?
                    .range();
                diagnostics.push(declared_diagnostic_range(
                    context,
                    &path,
                    range,
                    crate::diagnostic::DiagCode::UnresolvedTarget,
                    format!("unresolved UML target '{href}'"),
                    false,
                )?);
                continue;
            }
            let from_end = match &relationship.from_end {
                crate::uml::DeclaredField::Valid { value, .. } => value.clone(),
                _ => crate::model::RelEnd::default(),
            };
            let to_end = match &relationship.to_end {
                crate::uml::DeclaredField::Valid { value, .. } => value.clone(),
                _ => crate::model::RelEnd::default(),
            };
            let name = match &relationship.name {
                crate::uml::DeclaredField::Valid { value, .. } => Some(value.clone()),
                _ => None,
            };
            model.edges.push(crate::model::Edge {
                source: concept.concept_id.clone(),
                target,
                kind: *kind,
                name,
                from_end,
                to_end,
                bidirectional: false,
            });
        }
        for inline in concept.inline_instances.iter() {
            let validity = inline_instance_validity(inline, &path, &claimed);
            if matches!(validity, InlineInstanceValidity::Unresolved) {
                continue;
            }
            let InlineInstanceValidity::Valid(ValidInlineInstance { name, target }) = validity
            else {
                continue;
            };
            let key = format!("{}#{}", concept.concept_id, name);
            let mut instance_concept = okf.clone();
            instance_concept.title = Some(name.to_string());
            let slots = inline
                .slots
                .iter()
                .filter_map(|slot| match (&slot.name, &slot.value) {
                    (
                        crate::uml::DeclaredField::Valid { value: name, .. },
                        crate::uml::DeclaredField::Valid { value, .. },
                    ) => Some(crate::model::Slot {
                        name: name.clone(),
                        value: normalize_slot_value(value),
                        ref_: None,
                    }),
                    _ => None,
                })
                .collect();
            model.nodes.push(crate::model::Node {
                concept: instance_concept,
                key: key.clone(),
                ty: crate::model::ElementType::Uml(
                    crate::model::UmlMetaclass::InstanceSpecification,
                ),
                stereotypes: vec![],
                abstract_: false,
                attributes: vec![],
                values: vec![],
                note_body: None,
                annotates: vec![],
                members: vec![],
                slots,
            });
            model.edges.push(crate::model::Edge {
                source: key,
                target,
                kind: crate::model::RelationshipKind::InstanceOf,
                name: None,
                from_end: Default::default(),
                to_end: Default::default(),
                bidirectional: false,
            });
        }
    }
    Ok(model)
}
