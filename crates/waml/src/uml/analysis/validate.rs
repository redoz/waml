//! Validation: declared facts in, `Diagnostic`s out. Nothing here mutates
//! the model; a rule shared with admission is read from `verdict`.

use super::diagnostics::{declared_diagnostic, declared_diagnostic_range};
use super::syntax_util::trimmed_token_range;
use super::verdict::{relationship_end_verdict, EndVerdict};
use crate::uml::{
    declared::DeclaredBundle,
    syntax::{self, UmlLanguage},
};
use crate::{
    analysis::{AnalysisError, DomainAnalysisContext},
    diagnostic::Diagnostic,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode, TextRange};

pub(crate) fn validate_shared_context(
    context: &DomainAnalysisContext<'_>,
) -> Result<(), AnalysisError> {
    if context.session_revision != context.catalog.session_revision()
        || !Arc::ptr_eq(context.catalog, context.markdown.catalog())
    {
        return Err(AnalysisError::Specialization {
            name: "uml",
            reason: "UML analysis context does not share the shell catalog revision".into(),
        });
    }
    Ok(())
}

fn obsolete_diagram_message(ty: &str) -> Option<&'static str> {
    match ty {
        "uml.Activity" => Some(
            "obsolete diagram type 'uml.Activity'; use 'uml.ActivityDiagram' or run 'waml upgrade'",
        ),
        "uml.StateMachine" => Some(
            "obsolete diagram type 'uml.StateMachine'; use 'uml.StateMachineDiagram' or run 'waml upgrade'",
        ),
        "uml.Sequence" => Some(
            "obsolete diagram type 'uml.Sequence'; use 'uml.SequenceDiagram' or run 'waml upgrade'",
        ),
        "Diagram" => Some(
            "obsolete diagram type 'Diagram'; run 'waml upgrade' to select 'uml.ClassDiagram' or 'uml.UseCaseDiagram'",
        ),
        _ => None,
    }
}

/// The source range of the `type:` scalar, read off the document's *existing*
/// parse rather than a fresh one.
///
/// The string-taking `inspect_frontmatter_string_scalar` parses the whole
/// document to find one token. This runs once per untyped or wrongly-typed
/// document on every keystroke anywhere in the bundle, and a real bundle is
/// full of untyped documents (index pages, notes, contracts). Reparsing them
/// all was the largest single item in a per-edit reanalysis; the snapshot the
/// shell already produced answers the same question for free.
fn type_scalar_range(
    snapshot: &waml_syntax::MarkdownSyntaxSnapshot,
    ty: &str,
) -> Option<TextRange> {
    let Ok(crate::frontmatter::FrontmatterStringScalar::String { value, range }) =
        crate::frontmatter::inspect_parsed_frontmatter_string_scalar(snapshot, "type")
    else {
        return None;
    };
    (value == ty).then_some(range)
}

pub(crate) fn validate_document_types(
    context: &DomainAnalysisContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), AnalysisError> {
    for (id, document) in context.catalog.documents() {
        let concept_id = crate::okf::id_of(document.path().as_str());
        let Some(concept) = context.okf.concept(&concept_id) else {
            continue;
        };
        let (code, message, warning) = if let Some(message) = obsolete_diagram_message(&concept.ty)
        {
            (
                crate::diagnostic::DiagCode::ObsoleteDiagramType,
                message.to_string(),
                false,
            )
        } else if matches!(
            crate::model::ElementType::parse(&concept.ty),
            crate::model::ElementType::Unknown(_)
        ) {
            (
                crate::diagnostic::DiagCode::UnknownType,
                format!("unknown UML type '{}'", concept.ty),
                true,
            )
        } else {
            continue;
        };
        let Some(snapshot) = context.markdown.document(*id) else {
            continue;
        };
        let Some(range) = type_scalar_range(snapshot, &concept.ty) else {
            continue;
        };
        let line = document
            .line_index()
            .line_col(document.text(), range.start())
            .map_err(|_| AnalysisError::CatalogInvariant {
                reason: format!("type scalar range is not in document: {}", document.path()).into(),
            })?;
        let diagnostic = if warning {
            Diagnostic::warn(
                code,
                message,
                document.path().as_str(),
                line.line as usize + 1,
            )
        } else {
            Diagnostic::new(
                code,
                message,
                document.path().as_str(),
                line.line as usize + 1,
            )
        };
        diagnostics.push(
            diagnostic
                .with_span((
                    line.byte_column as usize,
                    line.byte_column as usize + range.len().to_usize(),
                ))
                .with_provenance(*id, document.revision(), range),
        );
    }
    Ok(())
}

pub(crate) fn validate_declared_semantics(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    concept_paths: &BTreeMap<String, String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), AnalysisError> {
    for concept in declared.concepts() {
        let Some(source_okf) = context.okf.concept(&concept.concept_id) else {
            continue;
        };
        let source_ty = crate::model::ElementType::parse(&source_okf.ty);
        let path = concept_paths
            .get(concept.concept_id.as_str())
            .map(String::as_str)
            .unwrap_or_default();
        for relationship in concept.relationships.iter() {
            let (
                crate::uml::DeclaredField::Valid { value: kind, .. },
                crate::uml::DeclaredField::Valid { value: href, .. },
            ) = (&relationship.kind, &relationship.target)
            else {
                continue;
            };
            let target = crate::okf::resolve_href(path, href);
            let target_concept = declared.concept(&target);
            if source_ty
                == crate::model::ElementType::Uml(crate::model::UmlMetaclass::InstanceSpecification)
                && *kind == crate::model::RelationshipKind::Links
                && target_concept.is_none()
            {
                if let Some(token) = relationship.syntax.target_token() {
                    diagnostics.push(declared_diagnostic_range(
                        context,
                        path,
                        token.range(),
                        crate::diagnostic::DiagCode::UnresolvedTarget,
                        format!("unresolved UML target '{href}'"),
                        true,
                    )?);
                }
            }
            let ends_absent = matches!(
                (&relationship.from_end, &relationship.to_end),
                (
                    crate::uml::DeclaredField::Absent,
                    crate::uml::DeclaredField::Absent
                )
            );
            let target_ty = target_concept
                .and_then(|_| context.okf.concept(&target))
                .map(|target| crate::model::ElementType::parse(&target.ty));
            if *kind == crate::model::RelationshipKind::Associates
                && ends_absent
                && source_ty.is_classifier()
                && target_ty.as_ref().is_some_and(|ty| ty.is_classifier())
                && !is_communication_party(&source_ty)
                && !target_ty.as_ref().is_some_and(is_communication_party)
            {
                // Same contract as the `end_message` report below: a
                // relationship missing its target token still reports,
                // against its own range, rather than going silent.
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    relationship.syntax.report_range(),
                    crate::diagnostic::DiagCode::MalformedRelationship,
                    "'associates' between classifiers requires ': <near> to <far>' multiplicity ends (ends are optional only on an actor↔use-case communication link)".into(),
                    false,
                )?);
            }
            let end_verdict =
                relationship_end_verdict(*kind, &relationship.from_end, &relationship.to_end);
            let end_message = match end_verdict {
                EndVerdict::Ok => None,
                EndVerdict::EndsRequired => Some(format!(
                    "'{}' requires ': <near> to <far>' multiplicity ends",
                    kind.as_str()
                )),
                EndVerdict::OneEnded => Some(format!(
                    "'{}' relationship has only one multiplicity end; both a near and a far end are required",
                    kind.as_str()
                )),
                EndVerdict::EndsUnparsable => Some(format!(
                    "'{}' has a multiplicity end that could not be parsed",
                    kind.as_str()
                )),
                EndVerdict::EndsForbidden => Some(format!(
                    "'{}' does not take multiplicity ends",
                    kind.as_str()
                )),
            };
            if let Some(message) = end_message {
                // A malformed-end relationship may also be missing its target
                // token; `declared_projection` drops it either way, so the
                // report falls back to the relationship's own range rather
                // than going silent.
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    relationship.syntax.report_range(),
                    crate::diagnostic::DiagCode::MalformedRelationship,
                    message,
                    false,
                )?);
            }
        }
    }

    for concept in declared.concepts() {
        let Some(okf) = context.okf.concept(&concept.concept_id) else {
            continue;
        };
        if crate::model::ElementType::parse(&okf.ty)
            != crate::model::ElementType::Uml(crate::model::UmlMetaclass::InstanceSpecification)
        {
            continue;
        }
        let path = concept_paths
            .get(concept.concept_id.as_str())
            .map(String::as_str)
            .unwrap_or_default();
        let mut classifier_attributes = BTreeSet::new();
        let mut classifier_found = false;
        for relationship in concept.relationships.iter() {
            let (
                crate::uml::DeclaredField::Valid { value: kind, .. },
                crate::uml::DeclaredField::Valid { value: href, .. },
            ) = (&relationship.kind, &relationship.target)
            else {
                continue;
            };
            if *kind != crate::model::RelationshipKind::InstanceOf {
                continue;
            }
            let target = crate::okf::resolve_href(path, href);
            let Some(target_okf) = context.okf.concept(&target) else {
                let range = relationship
                    .syntax
                    .target_token()
                    .map(|token| token.range())
                    .unwrap_or_else(|| relationship.syntax.syntax().range());
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    range,
                    crate::diagnostic::DiagCode::InstanceOfUnresolved,
                    format!("'instance of' target '{href}' resolves to no document"),
                    true,
                )?);
                continue;
            };
            if !crate::model::ElementType::parse(&target_okf.ty).is_classifier() {
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    relationship.syntax.report_range(),
                    crate::diagnostic::DiagCode::InstanceOfNonClassifier,
                    format!("'instance of' target '{target}' is not a classifier"),
                    true,
                )?);
                continue;
            }
            classifier_found = true;
            if let Some(classifier) = declared.concept(&target) {
                classifier_attributes.extend(classifier.attributes.iter().filter_map(
                    |attribute| match &attribute.name {
                        crate::uml::DeclaredField::Valid { value, .. } => Some(value.clone()),
                        _ => None,
                    },
                ));
            }
        }
        if !classifier_found {
            continue;
        }
        for slot in concept.slots.iter() {
            let crate::uml::DeclaredField::Valid { value: name, .. } = &slot.name else {
                continue;
            };
            if !classifier_attributes.contains(name) {
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    slot.syntax
                        .name_token()
                        .map(|token| trimmed_token_range(&token))
                        .unwrap_or_else(|| slot.syntax.syntax().range()),
                    crate::diagnostic::DiagCode::SlotUnknownAttribute,
                    format!("slot '{name}' names no classifier attribute"),
                    true,
                )?);
            }
        }
    }

    for concept in declared.concepts() {
        let Some(okf) = context.okf.concept(&concept.concept_id) else {
            continue;
        };
        if !crate::model::ElementType::parse(&okf.ty).is_view() {
            continue;
        }
        let path = concept_paths
            .get(concept.concept_id.as_str())
            .map(String::as_str)
            .unwrap_or_default();
        for inline in concept.inline_instances.iter() {
            let crate::uml::DeclaredField::Valid {
                value: classifier, ..
            } = &inline.classifier
            else {
                continue;
            };
            let target = crate::okf::resolve_href(path, classifier);
            let Some(target_okf) = context.okf.concept(&target) else {
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    inline
                        .syntax
                        .classifier_token()
                        .map(|token| token.range())
                        .unwrap_or_else(|| inline.syntax.syntax().range()),
                    crate::diagnostic::DiagCode::InstanceOfUnresolved,
                    format!("'instance of' target '{classifier}' resolves to no document"),
                    true,
                )?);
                continue;
            };
            if !crate::model::ElementType::parse(&target_okf.ty).is_classifier() {
                diagnostics.push(declared_diagnostic_range(
                    context,
                    path,
                    inline
                        .syntax
                        .classifier_token()
                        .map(|token| token.range())
                        .unwrap_or_else(|| inline.syntax.syntax().range()),
                    crate::diagnostic::DiagCode::InstanceOfNonClassifier,
                    format!("'instance of' target '{target}' is not a classifier"),
                    true,
                )?);
                continue;
            }
            let classifier_attributes = declared
                .concept(&target)
                .map(|classifier| {
                    classifier
                        .attributes
                        .iter()
                        .filter_map(|attribute| match &attribute.name {
                            crate::uml::DeclaredField::Valid { value, .. } => Some(value.clone()),
                            _ => None,
                        })
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            for slot in inline.slots.iter() {
                let crate::uml::DeclaredField::Valid { value: name, .. } = &slot.name else {
                    continue;
                };
                if !classifier_attributes.contains(name) {
                    diagnostics.push(declared_diagnostic_range(
                        context,
                        path,
                        slot.syntax
                            .name_token()
                            .map(|token| trimmed_token_range(&token))
                            .unwrap_or_else(|| slot.syntax.syntax().range()),
                        crate::diagnostic::DiagCode::SlotUnknownAttribute,
                        format!("slot '{name}' names no classifier attribute"),
                        true,
                    )?);
                }
            }
        }
    }

    let claimed = declared
        .concepts()
        .filter_map(|concept| {
            let ty = context
                .okf
                .concept(&concept.concept_id)
                .map(|okf| crate::model::ElementType::parse(&okf.ty))?;
            (!ty.is_view()).then_some(concept.concept_id.clone())
        })
        .collect::<BTreeSet<_>>();
    for concept in declared.concepts() {
        if concept.layout.is_empty() {
            continue;
        }
        let path = concept_paths
            .get(concept.concept_id.as_str())
            .map(String::as_str)
            .unwrap_or_default();
        let mut groups = BTreeSet::new();
        for group in concept.member_groups.iter() {
            collect_declared_group_names(group, &mut groups);
        }
        let mut horizontal = BTreeMap::<String, Vec<String>>::new();
        let mut vertical = BTreeMap::<String, Vec<String>>::new();
        let mut first_placement_syntax = None;
        for layout in concept.layout.iter() {
            let crate::uml::DeclaredField::Valid { value, syntax } = layout else {
                continue;
            };
            for (operand, operand_syntax) in declared_layout_operands(value) {
                let mut unresolved = Vec::new();
                collect_unresolved_layout_refs(operand, path, &claimed, &groups, &mut unresolved);
                for name in unresolved {
                    diagnostics.push(declared_diagnostic(
                        context,
                        path,
                        operand_syntax,
                        crate::diagnostic::DiagCode::UnresolvedLayoutRef,
                        format!("layout operand '{name}' resolves no member group"),
                        true,
                    )?);
                }
            }
            let crate::uml::DeclaredLayoutStatement::Placement {
                operands,
                directions,
            } = value
            else {
                continue;
            };
            first_placement_syntax.get_or_insert_with(|| {
                syntax
                    .children()
                    .find(|element| element.kind() == syntax::UmlSyntaxKind::LayoutPlacement)
                    .and_then(SyntaxElement::into_node)
                    .unwrap_or_else(|| syntax.clone())
            });
            for (index, direction) in directions.iter().enumerate() {
                let (
                    crate::uml::DeclaredField::Valid {
                        value: direction, ..
                    },
                    Some(crate::uml::DeclaredField::Valid { value: left, .. }),
                    Some(crate::uml::DeclaredField::Valid { value: right, .. }),
                ) = (direction, operands.get(index), operands.get(index + 1))
                else {
                    continue;
                };
                let (Some(left), Some(right)) = (
                    layout_operand_key(left, path),
                    layout_operand_key(right, path),
                ) else {
                    continue;
                };
                use crate::layout::Direction;
                match direction {
                    Direction::LeftOf => horizontal.entry(left).or_default().push(right),
                    Direction::RightOf => horizontal.entry(right).or_default().push(left),
                    Direction::Above => vertical.entry(left).or_default().push(right),
                    Direction::Below => vertical.entry(right).or_default().push(left),
                    Direction::AboveLeft => {
                        horizontal
                            .entry(left.clone())
                            .or_default()
                            .push(right.clone());
                        vertical.entry(left).or_default().push(right);
                    }
                    Direction::AboveRight => {
                        horizontal
                            .entry(right.clone())
                            .or_default()
                            .push(left.clone());
                        vertical.entry(left).or_default().push(right);
                    }
                    Direction::BelowLeft => {
                        horizontal
                            .entry(left.clone())
                            .or_default()
                            .push(right.clone());
                        vertical.entry(right).or_default().push(left);
                    }
                    Direction::BelowRight => {
                        horizontal
                            .entry(right.clone())
                            .or_default()
                            .push(left.clone());
                        vertical.entry(right).or_default().push(left);
                    }
                }
            }
        }
        if directed_cycle(&horizontal) || directed_cycle(&vertical) {
            if let Some(syntax) = first_placement_syntax {
                diagnostics.push(declared_diagnostic(
                    context,
                    path,
                    &syntax,
                    crate::diagnostic::DiagCode::LayoutCycle,
                    "layout placement constraints form a cycle (contradictory ordering)".into(),
                    false,
                )?);
            }
        }
    }
    Ok(())
}

fn is_communication_party(ty: &crate::model::ElementType) -> bool {
    matches!(
        ty,
        crate::model::ElementType::Uml(crate::model::UmlMetaclass::Actor)
            | crate::model::ElementType::Uml(crate::model::UmlMetaclass::UseCase)
    )
}

fn collect_declared_group_names(
    group: &crate::uml::DeclaredMemberGroup,
    names: &mut BTreeSet<String>,
) {
    if let crate::uml::DeclaredField::Valid { value, .. } = &group.name {
        names.insert(value.clone());
    }
    for child in group.children.iter() {
        collect_declared_group_names(child, names);
    }
}

fn declared_layout_operands(
    statement: &crate::uml::DeclaredLayoutStatement,
) -> Vec<(&crate::layout::Operand, &SyntaxNode<UmlLanguage>)> {
    match statement {
        crate::uml::DeclaredLayoutStatement::Placement { operands, .. } => operands
            .iter()
            .filter_map(|operand| match operand {
                crate::uml::DeclaredField::Valid { value, syntax } => Some((value, syntax)),
                _ => None,
            })
            .collect(),
        crate::uml::DeclaredLayoutStatement::Alignment { left, right } => [left, right]
            .into_iter()
            .filter_map(|anchored| match anchored {
                crate::uml::DeclaredField::Valid { value, syntax } => {
                    Some((&value.operand, syntax))
                }
                _ => None,
            })
            .collect(),
        crate::uml::DeclaredLayoutStatement::Standalone(operand) => match operand {
            crate::uml::DeclaredField::Valid { value, syntax } => vec![(value, syntax)],
            _ => Vec::new(),
        },
    }
}

fn collect_unresolved_layout_refs(
    operand: &crate::layout::Operand,
    path: &str,
    claimed: &BTreeSet<String>,
    groups: &BTreeSet<String>,
    unresolved: &mut Vec<String>,
) {
    use crate::layout::{NameRef, OperandRef};
    match &operand.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => {
            if !claimed.contains(&crate::okf::resolve_href(path, slug)) {
                unresolved.push(slug.clone());
            }
        }
        OperandRef::Name(NameRef::Bare(name)) => {
            let slug = crate::slug::slugify(name, "");
            let exact = claimed.contains(&slug);
            let basename_count = claimed
                .iter()
                .filter(|candidate| candidate.rsplit('/').next() == Some(slug.as_str()))
                .count();
            if !groups.contains(name) && !exact && basename_count != 1 {
                unresolved.push(name.clone());
            }
        }
        OperandRef::InlineGroup { items, .. } => {
            for item in items {
                collect_unresolved_layout_refs(item, path, claimed, groups, unresolved);
            }
        }
        OperandRef::Paren(inner) => {
            collect_unresolved_layout_refs(inner, path, claimed, groups, unresolved)
        }
    }
}

fn layout_operand_key(operand: &crate::layout::Operand, path: &str) -> Option<String> {
    use crate::layout::{NameRef, OperandRef};
    match &operand.ref_ {
        OperandRef::Name(NameRef::Link { slug, .. }) => Some(crate::okf::resolve_href(path, slug)),
        OperandRef::Name(NameRef::Bare(name)) => Some(name.clone()),
        OperandRef::Paren(inner) => layout_operand_key(inner, path),
        OperandRef::InlineGroup { .. } => None,
    }
}

fn directed_cycle(graph: &BTreeMap<String, Vec<String>>) -> bool {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, Vec<String>>,
        state: &mut BTreeMap<String, u8>,
    ) -> bool {
        state.insert(node.to_owned(), 1);
        for next in graph.get(node).into_iter().flatten() {
            match state.get(next).copied().unwrap_or_default() {
                1 => return true,
                0 if visit(next, graph, state) => return true,
                _ => {}
            }
        }
        state.insert(node.to_owned(), 2);
        false
    }
    let mut state = BTreeMap::new();
    graph.keys().any(|node| {
        state.get(node).copied().unwrap_or_default() == 0 && visit(node, graph, &mut state)
    })
}
