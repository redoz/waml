use super::{
    declared::{DeclaredAttribute, DeclaredBundle},
    syntax::{parser, UmlLanguage},
};
use crate::{
    analysis::{AnalysisError, ClaimSet, DomainAnalysisContext, SyntaxSet, SyntaxSnapshot},
    diagnostic::Diagnostic,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use waml_syntax::{AstNode, MarkdownStructureMap, SyntaxElement, SyntaxNode};
pub struct Analysis {
    pub claims: ClaimSet,
    pub syntax: SyntaxSet<UmlLanguage>,
    pub declared: DeclaredBundle,
    pub projection: super::Projection,
    pub diagnostics: Arc<[Diagnostic]>,
    pub structures: Arc<BTreeMap<crate::analysis::DocumentId, Arc<MarkdownStructureMap>>>,
    session_revision: u64,
}
impl Analysis {
    pub fn session_revision(&self) -> u64 {
        self.session_revision
    }
}
pub fn analyze(
    context: DomainAnalysisContext<'_>,
    _previous: Option<&Analysis>,
) -> Result<Analysis, AnalysisError> {
    if context.session_revision != context.catalog.session_revision()
        || !Arc::ptr_eq(context.catalog, context.shell.catalog())
    {
        return Err(AnalysisError::Specialization {
            name: "uml",
            reason: "UML analysis context does not share the shell catalog revision".into(),
        });
    }
    let claimed: Vec<_> = context
        .okf
        .concepts()
        .iter()
        .filter(|c| super::recognizes(c))
        .collect();
    let claims = ClaimSet::from_concept_ids(claimed.iter().map(|c| c.id.clone()));
    let mut snapshots = BTreeMap::new();
    let mut declared = DeclaredBundle::default();
    let mut diagnostics = Vec::new();
    for concept in claimed {
        let id = context
            .catalog
            .documents()
            .iter()
            .find_map(|(id, document)| {
                (crate::okf::id_of(document.path().as_str()) == concept.id).then_some(*id)
            })
            .ok_or_else(|| AnalysisError::CatalogInvariant {
                reason: "claimed concept has no document".into(),
            })?;
        let shell_snapshot =
            context
                .shell
                .document(id)
                .ok_or_else(|| AnalysisError::CatalogInvariant {
                    reason: "claimed concept has no shell syntax snapshot".into(),
                })?;
        let document = shell_snapshot.document().clone();
        let catalog_document =
            context
                .catalog
                .document(id)
                .ok_or_else(|| AnalysisError::CatalogInvariant {
                    reason: "claimed concept has no catalog document".into(),
                })?;
        let source_document = context.source.document(document.path()).ok_or_else(|| {
            AnalysisError::CatalogInvariant {
                reason: "claimed concept has no source document".into(),
            }
        })?;
        if !Arc::ptr_eq(catalog_document, &document)
            || !Arc::ptr_eq(document.text().shared(), source_document.text_arc())
        {
            return Err(AnalysisError::Specialization {
                name: "uml",
                reason: "UML document does not share shell/catalog/source provenance".into(),
            });
        }
        let structure =
            context
                .structures
                .get(&id)
                .ok_or_else(|| AnalysisError::CatalogInvariant {
                    reason: "claimed concept has no Markdown structure map".into(),
                })?;
        let tree = parser::parse(document.text().clone(), structure);
        let attributes = attributes(tree.root());
        let values = items(tree.root(), super::syntax::UmlSyntaxKind::Value);
        let slots = items(tree.root(), super::syntax::UmlSyntaxKind::Slot);
        let relationships = items(tree.root(), super::syntax::UmlSyntaxKind::Relationship);
        let members = items(tree.root(), super::syntax::UmlSyntaxKind::Member);
        let member_groups = direct_section_items(
            tree.root(),
            super::syntax::UmlSyntaxKind::MembersSection,
            super::syntax::UmlSyntaxKind::MemberGroup,
        );
        let inline_instances = items(tree.root(), super::syntax::UmlSyntaxKind::InlineInstance);
        let mut fields = Vec::new();
        for syntax in attributes {
            let name = syntax.name_token().text().write_to_string();
            let name_node = syntax.syntax().clone();
            let ty = syntax.type_syntax().map(|node| node.0);
            let multiplicity = syntax.multiplicity().map(|node| node.0);
            let visibility = syntax
                .visibility_token()
                .and_then(|token| token.text().write_to_string().chars().next())
                .and_then(crate::model::Visibility::from_marker)
                .map(|value| crate::uml::DeclaredField::Valid {
                    value,
                    syntax: name_node.clone(),
                })
                .unwrap_or(crate::uml::DeclaredField::Absent);
            let name_field = if name.is_empty() {
                crate::uml::DeclaredField::Incomplete {
                    syntax: name_node.clone(),
                    expected: crate::uml::ExpectedSyntax::TypeReference,
                }
            } else {
                crate::uml::DeclaredField::Valid {
                    value: name.clone(),
                    syntax: name_node.clone(),
                }
            };
            let ty_field = match ty {
                Some(node) => {
                    let value = node
                        .children()
                        .find_map(|e| e.into_token())
                        .map(|t| t.text().write_to_string())
                        .unwrap_or_default();
                    crate::uml::DeclaredField::Valid {
                        value: crate::model::TypeRef {
                            name: value,
                            ref_: None,
                        },
                        syntax: node,
                    }
                }
                None => crate::uml::DeclaredField::Incomplete {
                    syntax: name_node.clone(),
                    expected: crate::uml::ExpectedSyntax::TypeReference,
                },
            };
            let mult_field = match multiplicity {
                Some(node) => {
                    let missing_close = node.children().any(|element| {
                        element.kind() == super::syntax::UmlSyntaxKind::CloseBracketToken
                            && element
                                .into_token()
                                .is_some_and(|token| token.flags().is_missing())
                    });
                    let raw = node
                        .children()
                        .filter_map(|e| e.into_token())
                        .map(|t| t.text().write_to_string())
                        .collect::<String>();
                    let value = raw.trim_matches(['[', ']']).to_string();
                    match (!missing_close)
                        .then(|| crate::multiplicity::Multiplicity::parse(&value))
                        .flatten()
                    {
                        Some(value) => crate::uml::DeclaredField::Valid {
                            value,
                            syntax: node,
                        },
                        None => crate::uml::DeclaredField::Invalid {
                            syntax: node,
                            diagnostics: Arc::from([
                                crate::diagnostic::DiagCode::MalformedAttribute,
                            ]),
                        },
                    }
                }
                None => crate::uml::DeclaredField::Absent,
            };
            fields.push(crate::uml::DeclaredAttribute {
                syntax,
                visibility,
                name: name_field,
                ty: ty_field,
                multiplicity: mult_field,
            });
        }
        declared.concepts.insert(
            concept.id.clone(),
            crate::uml::DeclaredConcept {
                concept_id: concept.id.clone(),
                attributes: fields.into(),
                values: values
                    .into_iter()
                    .map(declared_value)
                    .collect::<Vec<_>>()
                    .into(),
                slots: slots
                    .into_iter()
                    .map(declared_slot)
                    .collect::<Vec<_>>()
                    .into(),
                relationships: relationships
                    .into_iter()
                    .map(declared_relationship)
                    .collect::<Vec<_>>()
                    .into(),
                members: members
                    .into_iter()
                    .map(declared_member)
                    .collect::<Vec<_>>()
                    .into(),
                member_groups: member_groups
                    .into_iter()
                    .map(declared_member_group)
                    .collect::<Vec<_>>()
                    .into(),
                inline_instances: inline_instances
                    .into_iter()
                    .map(declared_inline_instance)
                    .collect::<Vec<_>>()
                    .into(),
            },
        );
        for diagnostic in tree.diagnostics() {
            let start = diagnostic.range.start();
            let end = diagnostic.range.end();
            let start_line = document
                .line_index()
                .line_col(document.text(), start)
                .expect("parser diagnostic start is a document offset");
            let end_line = document
                .line_index()
                .line_col(document.text(), end)
                .expect("parser diagnostic end is a document offset");
            diagnostics.push(
                crate::diagnostic::Diagnostic::new(
                    crate::diagnostic::DiagCode::MalformedAttribute,
                    diagnostic.message.to_string(),
                    document.path().as_str(),
                    start_line.line as usize + 1,
                )
                .with_span((
                    start_line.byte_column as usize,
                    (if start_line.line == end_line.line {
                        end_line.byte_column
                    } else {
                        start_line.byte_column
                    }) as usize,
                ))
                .with_provenance(id, document.revision(), diagnostic.range),
            );
        }
        snapshots.insert(id, Arc::new(SyntaxSnapshot::new(document.clone(), tree)));
    }
    let projection = declared_projection(&context, &declared, &mut diagnostics);
    Ok(Analysis {
        claims,
        syntax: SyntaxSet::from_snapshots(context.catalog.clone(), snapshots),
        declared,
        projection,
        diagnostics: diagnostics.into(),
        structures: context.structures.clone(),
        session_revision: context.session_revision,
    })
}

fn declared_projection(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    diagnostics: &mut Vec<Diagnostic>,
) -> super::Projection {
    let claimed: BTreeSet<_> = declared.concepts().map(|c| c.concept_id.as_str()).collect();
    let mut model = crate::model::Model::default();
    for concept in declared.concepts() {
        let okf = context
            .okf
            .concept(&concept.concept_id)
            .expect("declared concept is claimed OKF concept");
        let path = context
            .catalog
            .documents()
            .iter()
            .find_map(|(_, d)| {
                (crate::okf::id_of(d.path().as_str()) == concept.concept_id)
                    .then_some(d.path().as_str().to_string())
            })
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
                ) => Some(crate::model::Slot {
                    name: name.clone(),
                    value: value.clone(),
                    ref_: None,
                }),
                _ => None,
            })
            .collect();
        let ty = crate::model::ElementType::parse(&okf.ty);
        if ty == crate::model::ElementType::Diagram {
            let mut member_keys = Vec::new();
            for member in concept.members.iter() {
                let crate::uml::DeclaredField::Valid { value: href, .. } = &member.target else {
                    continue;
                };
                let target = crate::okf::resolve_href(&path, href);
                if claimed.contains(target.as_str()) {
                    member_keys.push(target);
                } else {
                    let range = member
                        .syntax
                        .target_token()
                        .expect("valid member target token")
                        .range();
                    let id = context
                        .catalog
                        .id_for_path(
                            &crate::source::BundlePath::parse(path.clone())
                                .expect("catalog path valid"),
                        )
                        .expect("catalog document");
                    let document = context.catalog.document(id).expect("catalog document");
                    let line = document
                        .line_index()
                        .line_col(document.text(), range.start())
                        .expect("member range");
                    diagnostics.push(
                        Diagnostic::new(
                            crate::diagnostic::DiagCode::UnresolvedTarget,
                            format!("unresolved UML member '{href}'"),
                            path.clone(),
                            line.line as usize + 1,
                        )
                        .with_span((
                            line.byte_column as usize,
                            line.byte_column as usize + range.end().to_usize()
                                - range.start().to_usize(),
                        ))
                        .with_provenance(id, document.revision(), range),
                    );
                }
            }
            let groups = if concept.member_groups.is_empty() {
                vec![crate::model::DiagramGroup {
                    name: String::new(),
                    members: member_keys,
                    children: vec![],
                }]
            } else {
                concept
                    .member_groups
                    .iter()
                    .filter_map(|group| {
                        lower_member_group(group, &path, &claimed, &concept.concept_id)
                    })
                    .collect()
            };
            model.diagrams.push(crate::model::Diagram {
                key: concept.concept_id.clone(),
                title: okf
                    .title
                    .clone()
                    .unwrap_or_else(|| concept.concept_id.clone()),
                profile: String::new(),
                description: okf.description.clone(),
                groups,
                layout: vec![],
                display: Default::default(),
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
            let ends_valid = match kind {
                crate::model::RelationshipKind::Aggregates
                | crate::model::RelationshipKind::Composes => matches!(
                    (&relationship.from_end, &relationship.to_end),
                    (
                        crate::uml::DeclaredField::Valid { .. },
                        crate::uml::DeclaredField::Valid { .. }
                    )
                ),
                crate::model::RelationshipKind::Associates => matches!(
                    (&relationship.from_end, &relationship.to_end),
                    (
                        crate::uml::DeclaredField::Absent,
                        crate::uml::DeclaredField::Absent
                    ) | (
                        crate::uml::DeclaredField::Valid { .. },
                        crate::uml::DeclaredField::Valid { .. }
                    )
                ),
                _ => matches!(
                    (&relationship.from_end, &relationship.to_end),
                    (
                        crate::uml::DeclaredField::Absent,
                        crate::uml::DeclaredField::Absent
                    )
                ),
            };
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
                let range = relationship
                    .syntax
                    .target_token()
                    .expect("valid declared target token")
                    .range();
                let id = context
                    .catalog
                    .id_for_path(
                        &crate::source::BundlePath::parse(path.clone())
                            .expect("catalog path valid"),
                    )
                    .expect("catalog document");
                let document = context.catalog.document(id).expect("catalog document");
                let line = document
                    .line_index()
                    .line_col(document.text(), range.start())
                    .expect("relationship range");
                diagnostics.push(
                    Diagnostic::new(
                        crate::diagnostic::DiagCode::UnresolvedTarget,
                        format!("unresolved UML target '{href}'"),
                        path.clone(),
                        line.line as usize + 1,
                    )
                    .with_span((
                        line.byte_column as usize,
                        line.byte_column as usize
                            + (range.end().to_usize() - range.start().to_usize()),
                    ))
                    .with_provenance(id, document.revision(), range),
                );
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
            if let InlineInstanceValidity::Unresolved { classifier } = validity {
                let range = inline
                    .syntax
                    .classifier_token()
                    .expect("valid declared classifier token")
                    .range();
                let id = context
                    .catalog
                    .id_for_path(
                        &crate::source::BundlePath::parse(path.clone())
                            .expect("catalog path valid"),
                    )
                    .expect("catalog document");
                let document = context.catalog.document(id).expect("catalog document");
                let line = document
                    .line_index()
                    .line_col(document.text(), range.start())
                    .expect("inline range");
                diagnostics.push(
                    Diagnostic::new(
                        crate::diagnostic::DiagCode::UnresolvedTarget,
                        format!("unresolved inline classifier '{classifier}'"),
                        path.clone(),
                        line.line as usize + 1,
                    )
                    .with_span((
                        line.byte_column as usize,
                        line.byte_column as usize
                            + (range.end().to_usize() - range.start().to_usize()),
                    ))
                    .with_provenance(id, document.revision(), range),
                );
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
                        value: value.clone(),
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
    model
}

struct ValidInlineInstance<'a> {
    name: &'a str,
    target: String,
}

enum InlineInstanceValidity<'a> {
    Invalid,
    Unresolved { classifier: &'a str },
    Valid(ValidInlineInstance<'a>),
}

fn inline_instance_validity<'a>(
    inline: &'a crate::uml::DeclaredInlineInstance,
    path: &str,
    claimed: &BTreeSet<&str>,
) -> InlineInstanceValidity<'a> {
    if inline.slots.iter().any(|slot| {
        !matches!(
            (&slot.name, &slot.value),
            (
                crate::uml::DeclaredField::Valid { .. },
                crate::uml::DeclaredField::Valid { .. }
            )
        )
    }) {
        return InlineInstanceValidity::Invalid;
    }
    let (
        crate::uml::DeclaredField::Valid {
            value: classifier, ..
        },
        crate::uml::DeclaredField::Valid { value: name, .. },
    ) = (&inline.classifier, &inline.name)
    else {
        return InlineInstanceValidity::Invalid;
    };
    let target = crate::okf::resolve_href(path, classifier);
    if claimed.contains(target.as_str()) {
        InlineInstanceValidity::Valid(ValidInlineInstance { name, target })
    } else {
        InlineInstanceValidity::Unresolved { classifier }
    }
}

fn lower_member_group(
    group: &crate::uml::DeclaredMemberGroup,
    path: &str,
    claimed: &BTreeSet<&str>,
    owner: &str,
) -> Option<crate::model::DiagramGroup> {
    let crate::uml::DeclaredField::Valid { value: name, .. } = &group.name else {
        return None;
    };
    let mut members = group
        .members
        .iter()
        .filter_map(|member| match &member.target {
            crate::uml::DeclaredField::Valid { value, .. } => {
                let target = crate::okf::resolve_href(path, value);
                claimed.contains(target.as_str()).then_some(target)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    members.extend(group.inline_instances.iter().filter_map(
        |inline| match inline_instance_validity(inline, path, claimed) {
            InlineInstanceValidity::Valid(ValidInlineInstance { name, .. }) => {
                Some(format!("{owner}#{name}"))
            }
            InlineInstanceValidity::Invalid | InlineInstanceValidity::Unresolved { .. } => None,
        },
    ));
    Some(crate::model::DiagramGroup {
        name: name.clone(),
        members,
        children: group
            .children
            .iter()
            .filter_map(|child| lower_member_group(child, path, claimed, owner))
            .collect(),
    })
}

fn items(
    node: SyntaxNode<UmlLanguage>,
    kind: super::syntax::UmlSyntaxKind,
) -> Vec<SyntaxNode<UmlLanguage>> {
    let mut found = Vec::new();
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            if child.kind() == kind {
                found.push(child);
            } else {
                found.extend(items(child, kind));
            }
        }
    }
    found
}
fn direct_section_items(
    node: SyntaxNode<UmlLanguage>,
    section: super::syntax::UmlSyntaxKind,
    kind: super::syntax::UmlSyntaxKind,
) -> Vec<SyntaxNode<UmlLanguage>> {
    for child in node.children().filter_map(SyntaxElement::into_node) {
        if child.kind() == section {
            return child
                .children()
                .filter_map(SyntaxElement::into_node)
                .filter(|node| node.kind() == kind)
                .collect();
        }
        let found = direct_section_items(child, section, kind);
        if !found.is_empty() {
            return found;
        }
    }
    Vec::new()
}
fn valid<T>(node: SyntaxNode<UmlLanguage>, value: T) -> crate::uml::DeclaredField<UmlLanguage, T> {
    crate::uml::DeclaredField::Valid {
        value,
        syntax: node,
    }
}
fn invalid<T>(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredField<UmlLanguage, T> {
    crate::uml::DeclaredField::Invalid {
        syntax: node,
        diagnostics: Arc::from([crate::diagnostic::DiagCode::DroppableContent]),
    }
}
fn has_recovery(node: &SyntaxNode<UmlLanguage>) -> bool {
    node.children().any(|e| {
        matches!(
            e.kind(),
            super::syntax::UmlSyntaxKind::BadToken
                | super::syntax::UmlSyntaxKind::SkippedTokensSyntax
        ) || e.into_node().is_some_and(|child| has_recovery(&child))
    })
}
fn has_missing_kind(node: &SyntaxNode<UmlLanguage>, kind: super::syntax::UmlSyntaxKind) -> bool {
    node.children().any(|e| {
        e.kind() == kind
            && e.into_token()
                .is_some_and(|token| token.flags().is_missing())
    })
}
fn declared_value(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredValue {
    let syntax = super::syntax::ValueSyntax(node.clone());
    crate::uml::DeclaredValue {
        value: syntax
            .value_token()
            .filter(|t| !t.flags().is_missing() && !t.text().write_to_string().is_empty())
            .map(|t| valid(node.clone(), t.text().write_to_string()))
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::TypeReference,
            }),
        syntax,
    }
}
fn declared_slot(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredSlot {
    let syntax = super::syntax::SlotSyntax(node.clone());
    let field = |token: Option<waml_syntax::SyntaxToken<UmlLanguage>>, expected| {
        token
            .filter(|t| !t.flags().is_missing() && !t.text().write_to_string().is_empty())
            .map(|t| valid(node.clone(), t.text().write_to_string()))
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected,
            })
    };
    if has_recovery(&node) {
        return crate::uml::DeclaredSlot {
            name: invalid(node.clone()),
            value: invalid(node.clone()),
            syntax,
        };
    }
    let name = field(syntax.name_token(), crate::uml::ExpectedSyntax::ColonToken);
    if syntax.colon_token().is_none_or(|t| t.flags().is_missing()) {
        return crate::uml::DeclaredSlot {
            name,
            value: crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::ColonToken,
            },
            syntax,
        };
    }
    let mut after_colon = false;
    let value_parts = node
        .children()
        .filter(|e| {
            if e.kind() == super::syntax::UmlSyntaxKind::ColonToken {
                after_colon = true;
                return false;
            }
            after_colon && !matches!(e.kind(), super::syntax::UmlSyntaxKind::NewlineToken)
        })
        .count();
    if value_parts != 1 {
        return crate::uml::DeclaredSlot {
            name: invalid(node.clone()),
            value: invalid(node.clone()),
            syntax,
        };
    }
    let mut value = field(syntax.value_token(), crate::uml::ExpectedSyntax::LinkTarget);
    if let Some(token) = syntax.value_token() {
        let raw = token.text().write_to_string();
        if (syntax.value_kind() == super::syntax::SlotValueKind::Quoted
            && (raw.len() < 2 || !raw.ends_with('"')))
            || (syntax.value_kind() == super::syntax::SlotValueKind::Link
                && !token.flags().is_missing()
                && raw.is_empty())
        {
            value = invalid(node.clone());
        }
    }
    crate::uml::DeclaredSlot {
        name,
        value,
        syntax,
    }
}
fn declared_relationship(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredRelationship {
    let syntax = super::syntax::RelationshipSyntax(node.clone());
    let mut kind = syntax
        .kind_token()
        .and_then(|t| crate::model::RelationshipKind::parse(&t.text().write_to_string()))
        .map(|value| valid(node.clone(), value))
        .unwrap_or_else(|| invalid(node.clone()));
    let target = if syntax.link().as_ref().is_some_and(has_recovery) {
        invalid(node.clone())
    } else {
        match syntax.target_token() {
            Some(token) if token.flags().is_missing() => crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::RelationshipTarget,
            },
            Some(token) if token.text().write_to_string().is_empty() => invalid(node.clone()),
            Some(token) => valid(node.clone(), token.text().write_to_string()),
            None => crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::RelationshipTarget,
            },
        }
    };
    let name = if let Some(label) = syntax.name_label_token() {
        let raw = label.text().write_to_string();
        if !raw.ends_with('"') || raw.len() < 2 {
            invalid(node.clone())
        } else {
            valid(
                node.clone(),
                crate::model::AssocName::Label(raw.trim_matches('"').to_string()),
            )
        }
    } else if let Some(link) = syntax.name_link() {
        let target = link
            .children()
            .find(|e| e.kind() == super::syntax::UmlSyntaxKind::LinkTargetToken)
            .and_then(|e| e.into_token());
        match target {
            Some(token)
                if !has_recovery(&link)
                    && !token.flags().is_missing()
                    && !token.text().write_to_string().is_empty() =>
            {
                valid(
                    node.clone(),
                    crate::model::AssocName::Assoc(token.text().write_to_string()),
                )
            }
            Some(token) if token.flags().is_missing() && !has_recovery(&link) => {
                crate::uml::DeclaredField::Incomplete {
                    syntax: node.clone(),
                    expected: crate::uml::ExpectedSyntax::RelationshipTarget,
                }
            }
            _ => invalid(node.clone()),
        }
    } else {
        crate::uml::DeclaredField::Absent
    };
    let end = |end: Option<super::syntax::RelationshipEndSyntax>| {
        let Some(end) = end else {
            return crate::uml::DeclaredField::Absent;
        };
        let raw = end.multiplicity_token().text().write_to_string();
        let Some(multiplicity) = crate::multiplicity::Multiplicity::parse(&raw) else {
            return invalid(end.0);
        };
        valid(
            end.0.clone(),
            crate::model::RelEnd {
                multiplicity: Some(multiplicity),
                role: end.role_token().map(|t| t.text().write_to_string()),
                navigable: None,
            },
        )
    };
    let mut from_end = end(syntax.from_end());
    let mut to_end = end(syntax.to_end());
    match (&kind, syntax.colon_token().is_some()) {
        (crate::uml::DeclaredField::Valid { value, .. }, false)
            if matches!(
                value,
                crate::model::RelationshipKind::Aggregates
                    | crate::model::RelationshipKind::Composes
            ) =>
        {
            from_end = crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::ValidMultiplicity,
            };
            to_end = crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::ValidMultiplicity,
            };
        }
        (crate::uml::DeclaredField::Valid { value, .. }, true) if !value.is_ended() => {
            from_end = invalid(node.clone());
            to_end = invalid(node.clone());
        }
        _ => {}
    }
    if has_recovery(&node) {
        kind = invalid(node.clone());
    }
    crate::uml::DeclaredRelationship {
        syntax,
        kind,
        target,
        name,
        from_end,
        to_end,
    }
}
fn declared_member(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredMember {
    let syntax = super::syntax::MemberSyntax(node.clone());
    let target = if has_recovery(&node) {
        invalid(node.clone())
    } else {
        match syntax.target_token() {
            Some(token) if token.flags().is_missing() => crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::LinkTarget,
            },
            Some(token) if token.text().write_to_string().is_empty() => invalid(node.clone()),
            Some(token) => valid(node.clone(), token.text().write_to_string()),
            None => crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::LinkTarget,
            },
        }
    };
    crate::uml::DeclaredMember { syntax, target }
}
fn declared_member_group(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredMemberGroup {
    let syntax = super::syntax::MemberGroupSyntax(node.clone());
    let name = syntax
        .heading_token()
        .filter(|t| !t.flags().is_missing() && !t.text().write_to_string().is_empty())
        .map(|t| valid(node.clone(), t.text().write_to_string()))
        .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
            syntax: node.clone(),
            expected: crate::uml::ExpectedSyntax::LinkTarget,
        });
    crate::uml::DeclaredMemberGroup {
        syntax,
        name,
        members: node
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter(|n| n.kind() == super::syntax::UmlSyntaxKind::Member)
            .map(declared_member)
            .collect::<Vec<_>>()
            .into(),
        inline_instances: node
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter(|n| n.kind() == super::syntax::UmlSyntaxKind::InlineInstance)
            .map(declared_inline_instance)
            .collect::<Vec<_>>()
            .into(),
        children: node
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter(|n| n.kind() == super::syntax::UmlSyntaxKind::MemberGroup)
            .map(declared_member_group)
            .collect::<Vec<_>>()
            .into(),
    }
}
fn declared_inline_instance(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredInlineInstance {
    let syntax = super::syntax::InlineInstanceSyntax(node.clone());
    let classifier = if syntax.link().as_ref().is_some_and(has_recovery) {
        invalid(node.clone())
    } else {
        match syntax.classifier_token() {
            Some(token) if token.flags().is_missing() => crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::LinkTarget,
            },
            Some(token) if token.text().write_to_string().is_empty() => invalid(node.clone()),
            Some(token) => valid(node.clone(), token.text().write_to_string()),
            None => crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::LinkTarget,
            },
        }
    };
    let slots = syntax
        .slots()
        .map(|slot| {
            let slot_node = slot.0.clone();
            let name = slot
                .0
                .children()
                .find(|e| e.kind() == super::syntax::UmlSyntaxKind::IdentifierToken)
                .and_then(|e| e.into_token())
                .filter(|token| {
                    !token.flags().is_missing() && !token.text().write_to_string().is_empty()
                })
                .map(|t| valid(slot_node.clone(), t.text().write_to_string()))
                .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                    syntax: slot_node.clone(),
                    expected: crate::uml::ExpectedSyntax::LinkTarget,
                });
            let value_element = slot
                .0
                .children()
                .filter(|e| {
                    matches!(
                        e.kind(),
                        super::syntax::UmlSyntaxKind::IdentifierToken
                            | super::syntax::UmlSyntaxKind::TypeToken
                            | super::syntax::UmlSyntaxKind::Link
                    )
                })
                .last();
            let mut value = match value_element {
                Some(SyntaxElement::Token(token))
                    if token.flags().is_missing() || token.text().write_to_string().is_empty() =>
                {
                    crate::uml::DeclaredField::Incomplete {
                        syntax: slot_node.clone(),
                        expected: crate::uml::ExpectedSyntax::LinkTarget,
                    }
                }
                Some(SyntaxElement::Token(token)) => {
                    let raw = token.text().write_to_string();
                    if raw.starts_with('"') && (raw.len() < 2 || !raw.ends_with('"')) {
                        invalid(slot_node.clone())
                    } else {
                        valid(slot_node.clone(), raw)
                    }
                }
                Some(SyntaxElement::Node(link)) if has_recovery(&link) => {
                    invalid(slot_node.clone())
                }
                Some(SyntaxElement::Node(link)) => match link
                    .children()
                    .find(|x| x.kind() == super::syntax::UmlSyntaxKind::LinkTargetToken)
                    .and_then(|x| x.into_token())
                {
                    Some(token) if token.flags().is_missing() => {
                        crate::uml::DeclaredField::Incomplete {
                            syntax: slot_node.clone(),
                            expected: crate::uml::ExpectedSyntax::LinkTarget,
                        }
                    }
                    Some(token) if token.text().write_to_string().is_empty() => {
                        invalid(slot_node.clone())
                    }
                    Some(token) => valid(slot_node.clone(), token.text().write_to_string()),
                    None => crate::uml::DeclaredField::Incomplete {
                        syntax: slot_node.clone(),
                        expected: crate::uml::ExpectedSyntax::LinkTarget,
                    },
                },
                None => crate::uml::DeclaredField::Incomplete {
                    syntax: slot_node.clone(),
                    expected: crate::uml::ExpectedSyntax::LinkTarget,
                },
            };
            if has_recovery(&slot_node) {
                value = invalid(slot_node.clone());
            } else if has_missing_kind(&slot_node, super::syntax::UmlSyntaxKind::SetToToken)
                && !matches!(value, crate::uml::DeclaredField::Invalid { .. })
            {
                value = crate::uml::DeclaredField::Incomplete {
                    syntax: slot_node.clone(),
                    expected: crate::uml::ExpectedSyntax::LinkTarget,
                };
            }
            crate::uml::DeclaredSlot {
                syntax: super::syntax::SlotSyntax(slot_node),
                name,
                value,
            }
        })
        .collect::<Vec<_>>()
        .into();
    let name = if has_recovery(&node) {
        invalid(node.clone())
    } else if has_missing_kind(&node, super::syntax::UmlSyntaxKind::AsToken) {
        crate::uml::DeclaredField::Incomplete {
            syntax: node.clone(),
            expected: crate::uml::ExpectedSyntax::LinkTarget,
        }
    } else {
        syntax
            .name_token()
            .filter(|t| !t.flags().is_missing() && !t.text().write_to_string().is_empty())
            .map(|t| valid(node.clone(), t.text().write_to_string()))
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: node.clone(),
                expected: crate::uml::ExpectedSyntax::LinkTarget,
            })
    };
    crate::uml::DeclaredInlineInstance {
        syntax,
        classifier,
        name,
        slots,
    }
}

fn attributes(node: SyntaxNode<UmlLanguage>) -> Vec<super::syntax::AttributeSyntax> {
    let mut found = Vec::new();
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            if let Some(attribute) = super::syntax::AttributeSyntax::cast(child.clone()) {
                found.push(attribute);
            } else {
                found.extend(attributes(child));
            }
        }
    }
    found
}
