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

    pub fn referrers(&self, target: &str) -> Vec<String> {
        let target_document = self
            .syntax
            .catalog()
            .documents()
            .iter()
            .find_map(|(id, document)| {
                (crate::okf::id_of(document.path().as_str()) == target).then_some((*id, document))
            })
            .or_else(|| {
                let mut matches = self
                    .syntax
                    .catalog()
                    .documents()
                    .iter()
                    .filter(|(_, document)| document_slug(document.path().as_str()) == target);
                match (matches.next(), matches.next()) {
                    (Some((id, document)), None) => Some((*id, document)),
                    _ => None,
                }
            });
        let target_slug = target_document
            .map(|(_, document)| document_slug(document.path().as_str()))
            .unwrap_or(target);
        let href = format!("./{target_slug}.md");
        let target_id = target_document.map(|(id, _)| id);
        let mut output = Vec::new();
        for source_document in self.syntax.catalog().documents().values() {
            let document = source_document.id();
            if Some(document) == target_id {
                continue;
            }
            let Some(snapshot) = self.syntax.document(document) else {
                continue;
            };
            let source = snapshot.document().text().shared();
            let referenced = [
                super::syntax::UmlSyntaxKind::Attribute,
                super::syntax::UmlSyntaxKind::Relationship,
                super::syntax::UmlSyntaxKind::Member,
                super::syntax::UmlSyntaxKind::InlineInstance,
                super::syntax::UmlSyntaxKind::LayoutStatement,
            ]
            .into_iter()
            .flat_map(|kind| syntax_nodes(snapshot.syntax(), kind))
            .any(|syntax| {
                let range = syntax.range();
                let authored = &source[range.start().to_usize()..range.end().to_usize()];
                authored.contains(&href)
                    || (syntax.kind() == super::syntax::UmlSyntaxKind::LayoutStatement
                        && authored
                            .split(|character: char| {
                                character.is_whitespace()
                                    || matches!(character, '[' | ']' | '(' | ')' | ',')
                            })
                            .any(|word| word == target_slug))
            });
            if referenced {
                output.push(document_slug(source_document.path().as_str()).to_owned());
            }
        }
        output.sort();
        output.dedup();
        output
    }
}

fn document_slug(path: &str) -> &str {
    let segment = path.rsplit(['/', '\\']).next().unwrap_or(path);
    segment.strip_suffix(".md").unwrap_or(segment)
}

fn syntax_nodes(
    tree: &waml_syntax::SyntaxTree<UmlLanguage>,
    kind: super::syntax::UmlSyntaxKind,
) -> Vec<SyntaxNode<UmlLanguage>> {
    fn collect(
        node: &SyntaxNode<UmlLanguage>,
        kind: super::syntax::UmlSyntaxKind,
        output: &mut Vec<SyntaxNode<UmlLanguage>>,
    ) {
        for child in node.children().filter_map(SyntaxElement::into_node) {
            if child.kind() == kind {
                output.push(child.clone());
            }
            collect(&child, kind, output);
        }
    }
    let mut output = Vec::new();
    collect(&tree.root(), kind, &mut output);
    output
}
pub fn analyze(
    context: DomainAnalysisContext<'_>,
    _previous: Option<&Analysis>,
) -> Result<Analysis, AnalysisError> {
    validate_shared_context(&context)?;
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
        let layout = items(tree.root(), super::syntax::UmlSyntaxKind::LayoutStatement);
        let flow_nodes = items(tree.root(), super::syntax::UmlSyntaxKind::FlowNode);
        let lifelines = items(tree.root(), super::syntax::UmlSyntaxKind::Lifeline);
        let messages = items(tree.root(), super::syntax::UmlSyntaxKind::Message);
        let sequence_operands = items(tree.root(), super::syntax::UmlSyntaxKind::SequenceOperand);
        let sequence_fragments = items(tree.root(), super::syntax::UmlSyntaxKind::SequenceFragment);
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
                    let value = raw.trim_matches(['[', ']', '{', '}']).to_string();
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
        let layout_fields = layout.into_iter().map(declared_layout).collect::<Vec<_>>();
        for field in &layout_fields {
            let syntax = match field {
                crate::uml::DeclaredField::Valid { .. } | crate::uml::DeclaredField::Absent => {
                    continue
                }
                crate::uml::DeclaredField::Incomplete { syntax, .. }
                | crate::uml::DeclaredField::Invalid { syntax, .. } => syntax,
            };
            let range = syntax.range();
            let start = document
                .line_index()
                .line_col(document.text(), range.start())
                .expect("layout diagnostic start is a document offset");
            let end = document
                .line_index()
                .line_col(document.text(), range.end())
                .expect("layout diagnostic end is a document offset");
            diagnostics.push(
                crate::diagnostic::Diagnostic::new(
                    crate::diagnostic::DiagCode::MalformedLayout,
                    "malformed layout statement",
                    document.path().as_str(),
                    start.line as usize + 1,
                )
                .with_span((
                    start.byte_column as usize,
                    (if start.line == end.line {
                        end.byte_column
                    } else {
                        start.byte_column
                    }) as usize,
                ))
                .with_provenance(id, document.revision(), range),
            );
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
                layout: layout_fields.into(),
                flow_nodes: flow_nodes
                    .into_iter()
                    .map(declared_flow_node)
                    .collect::<Vec<_>>()
                    .into(),
                lifelines: lifelines
                    .into_iter()
                    .map(declared_lifeline)
                    .collect::<Vec<_>>()
                    .into(),
                messages: messages
                    .into_iter()
                    .map(declared_message)
                    .collect::<Vec<_>>()
                    .into(),
                sequence_operands: sequence_operands
                    .into_iter()
                    .chain(sequence_fragments)
                    .map(declared_sequence_operand)
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
                    match diagnostic.code {
                        super::syntax::UmlSyntaxDiagnosticCode::MalformedFlow
                        | super::syntax::UmlSyntaxDiagnosticCode::MalformedIndentation => {
                            crate::diagnostic::DiagCode::MalformedFlowBullet
                        }
                        super::syntax::UmlSyntaxDiagnosticCode::MalformedLifeline => {
                            crate::diagnostic::DiagCode::MalformedLifeline
                        }
                        super::syntax::UmlSyntaxDiagnosticCode::MalformedMessage
                        | super::syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm => {
                            crate::diagnostic::DiagCode::MalformedMessage
                        }
                        super::syntax::UmlSyntaxDiagnosticCode::UnresolvedTarget => {
                            crate::diagnostic::DiagCode::UnresolvedTarget
                        }
                        _ => crate::diagnostic::DiagCode::MalformedAttribute,
                    },
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

fn validate_shared_context(context: &DomainAnalysisContext<'_>) -> Result<(), AnalysisError> {
    if context.session_revision != context.catalog.session_revision()
        || !Arc::ptr_eq(context.catalog, context.shell.catalog())
    {
        return Err(AnalysisError::Specialization {
            name: "uml",
            reason: "UML analysis context does not share the shell catalog revision".into(),
        });
    }
    Ok(())
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
        if matches!(
            ty,
            crate::model::ElementType::Behavior(crate::model::BehaviorKind::Activity)
                | crate::model::ElementType::Behavior(crate::model::BehaviorKind::StateMachine)
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
        } else if ty == crate::model::ElementType::Behavior(crate::model::BehaviorKind::Sequence) {
            lower_sequence_behavior(
                context,
                concept,
                okf,
                &path,
                &claimed,
                &mut model,
                diagnostics,
            );
        } else if ty == crate::model::ElementType::Diagram {
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
            let groups = concept
                .member_groups
                .iter()
                .filter_map(|group| lower_member_group(group, &path, &claimed, &concept.concept_id))
                .collect();
            model.diagrams.push(crate::model::Diagram {
                key: concept.concept_id.clone(),
                title: okf
                    .title
                    .clone()
                    .unwrap_or_else(|| concept.concept_id.clone()),
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
                        } => Some(crate::syntax::LayoutStatement::Placement {
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
                                    crate::uml::DeclaredField::Valid { value, .. } => {
                                        Some(value.clone())
                                    }
                                    _ => None,
                                })
                                .collect(),
                        }),
                        crate::uml::DeclaredField::Valid {
                            value: crate::uml::DeclaredLayoutStatement::Alignment { left, right },
                            ..
                        } => Some(crate::syntax::LayoutStatement::Alignment {
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
                            value: crate::uml::DeclaredLayoutStatement::Standalone(operand),
                            ..
                        } => match operand {
                            crate::uml::DeclaredField::Valid { value, .. } => {
                                Some(crate::syntax::LayoutStatement::Standalone(value.clone()))
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect(),
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

fn resolve_slug(path: &str, slug: &str, claimed: &BTreeSet<&str>) -> Option<String> {
    let target = crate::okf::resolve_href(path, &format!("./{slug}.md"));
    claimed.contains(target.as_str()).then_some(target)
}

fn resolve_describes(
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
) -> Option<String> {
    okf.extra
        .get_str("describes")
        .and_then(crate::grammar::parse_link_ref)
        .and_then(|link| resolve_slug(path, &link.slug, claimed))
}

fn behavior_diagnostic(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    syntax: &SyntaxNode<UmlLanguage>,
    code: crate::diagnostic::DiagCode,
    message: String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let id = context
        .catalog
        .id_for_path(&crate::source::BundlePath::parse(path.to_string()).expect("catalog path"))
        .expect("catalog document");
    let document = context.catalog.document(id).expect("catalog document");
    let range = items(syntax.clone(), super::syntax::UmlSyntaxKind::Link)
        .into_iter()
        .find_map(|link| {
            link.children()
                .find(|element| element.kind() == super::syntax::UmlSyntaxKind::LinkTargetToken)
                .map(|element| match element {
                    SyntaxElement::Node(node) => node.range(),
                    SyntaxElement::Token(token) => token.range(),
                })
        })
        .or_else(|| {
            syntax
                .children()
                .find(|element| element.kind() == super::syntax::UmlSyntaxKind::TargetToken)
                .map(|element| match element {
                    SyntaxElement::Node(node) => node.range(),
                    SyntaxElement::Token(token) => token.range(),
                })
        })
        .unwrap_or_else(|| syntax.range());
    let start = document
        .line_index()
        .line_col(document.text(), range.start())
        .expect("behavior range");
    let end = document
        .line_index()
        .line_col(document.text(), range.end())
        .expect("behavior range");
    diagnostics.push(
        Diagnostic::new(code, message, path, start.line as usize + 1)
            .with_span((
                start.byte_column as usize,
                if start.line == end.line {
                    end.byte_column as usize
                } else {
                    start.byte_column as usize
                },
            ))
            .with_provenance(id, document.revision(), range),
    );
}

fn field_value<T>(field: &crate::uml::DeclaredField<UmlLanguage, T>) -> Option<&T> {
    match field {
        crate::uml::DeclaredField::Valid { value, .. } => Some(value),
        _ => None,
    }
}

fn lower_flow_behavior(
    context: &DomainAnalysisContext<'_>,
    concept: &crate::uml::DeclaredConcept,
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
    model: &mut crate::model::Model,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let flavor = match crate::model::ElementType::parse(&okf.ty) {
        crate::model::ElementType::Behavior(crate::model::BehaviorKind::Activity) => {
            crate::model::FlowFlavor::Activity
        }
        _ => crate::model::FlowFlavor::StateMachine,
    };
    let mut local = BTreeMap::new();
    for node in concept.flow_nodes.iter() {
        let Some(identity) = field_value(&node.identity) else {
            continue;
        };
        if local.insert(identity.clone(), ()).is_some() {
            behavior_diagnostic(
                context,
                path,
                &node.syntax.0,
                crate::diagnostic::DiagCode::DuplicateFlowNode,
                format!("duplicate flow node '{identity}'"),
                diagnostics,
            );
        }
    }
    let mut node_keys = Vec::new();
    let mut edge_keys = Vec::new();
    for node in concept.flow_nodes.iter() {
        let (Some(kind), Some(identity)) = (field_value(&node.kind), field_value(&node.identity))
        else {
            continue;
        };
        let key = format!("{}#{}", concept.concept_id, identity);
        let object_ref = field_value(&node.object_ref).and_then(|slug| {
            let resolved = resolve_slug(path, slug, claimed);
            if resolved.is_none() {
                behavior_diagnostic(
                    context,
                    path,
                    &node.syntax.0,
                    crate::diagnostic::DiagCode::UnresolvedTarget,
                    format!("unresolved UML object target '{slug}'"),
                    diagnostics,
                );
            }
            resolved
        });
        let refines = field_value(&node.refines).and_then(|slug| {
            let resolved = resolve_slug(path, slug, claimed);
            if resolved.is_none() {
                behavior_diagnostic(
                    context,
                    path,
                    &node.syntax.0,
                    crate::diagnostic::DiagCode::UnresolvedTarget,
                    format!("unresolved refined behavior '{slug}'"),
                    diagnostics,
                );
            }
            resolved
        });
        node_keys.push(key.clone());
        model.activity_nodes.push(crate::model::ActivityNode {
            key,
            id: identity.clone(),
            behavior: concept.concept_id.clone(),
            kind: *kind,
            object_ref,
            partition: field_value(&node.partition).cloned(),
            entry: field_value(&node.entry).cloned(),
            do_: field_value(&node.do_).cloned(),
            exit: field_value(&node.exit).cloned(),
            refines,
            notes: node.notes.iter().filter_map(field_value).cloned().collect(),
        });
        for transition in node.transitions.iter() {
            let Some(target) = field_value(&transition.target) else {
                continue;
            };
            let (to, to_ref) = match target {
                crate::syntax::FlowTargetRef::Local(name) => {
                    if !local.contains_key(name) {
                        behavior_diagnostic(
                            context,
                            path,
                            &transition.syntax.0,
                            crate::diagnostic::DiagCode::UnresolvedTarget,
                            format!("unresolved flow node '{name}'"),
                            diagnostics,
                        );
                        continue;
                    }
                    (format!("{}#{}", concept.concept_id, name), None)
                }
                crate::syntax::FlowTargetRef::Link(link) => {
                    let Some(target) = resolve_slug(path, &link.slug, claimed) else {
                        behavior_diagnostic(
                            context,
                            path,
                            &transition.syntax.0,
                            crate::diagnostic::DiagCode::UnresolvedTarget,
                            format!("unresolved flow target '{}'", link.slug),
                            diagnostics,
                        );
                        continue;
                    };
                    (link.title.clone(), Some(target))
                }
            };
            let carries = field_value(&transition.carries).and_then(|slug| {
                let resolved = resolve_slug(path, slug, claimed);
                if resolved.is_none() {
                    behavior_diagnostic(
                        context,
                        path,
                        &transition.syntax.0,
                        crate::diagnostic::DiagCode::UnresolvedTarget,
                        format!("unresolved carried type '{slug}'"),
                        diagnostics,
                    );
                }
                resolved
            });
            let edge_key = format!("{}#e{}", concept.concept_id, model.flow_edges.len());
            edge_keys.push(edge_key.clone());
            model.flow_edges.push(crate::model::FlowEdge {
                key: edge_key,
                kind: if carries.is_some() || *kind == crate::model::FlowNodeKind::Object {
                    crate::model::FlowEdgeKind::ObjectFlow
                } else {
                    crate::model::FlowEdgeKind::ControlFlow
                },
                behavior: concept.concept_id.clone(),
                from: format!("{}#{}", concept.concept_id, identity),
                to,
                to_ref,
                trigger: field_value(&transition.trigger).cloned(),
                guard: field_value(&transition.guard).cloned(),
                is_else: transition.is_else,
                effect: field_value(&transition.effect).cloned(),
                carries,
            });
        }
    }
    model.flows.push(crate::model::FlowDoc {
        key: concept.concept_id.clone(),
        title: okf
            .title
            .clone()
            .unwrap_or_else(|| concept.concept_id.clone()),
        flavor,
        describes: resolve_describes(okf, path, claimed),
        nodes: node_keys,
        edges: edge_keys,
    });
}

fn lower_sequence_behavior(
    context: &DomainAnalysisContext<'_>,
    concept: &crate::uml::DeclaredConcept,
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
    model: &mut crate::model::Model,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut nodes = Vec::new();
    let mut handles = BTreeSet::new();
    for lifeline in concept.lifelines.iter() {
        let (Some(slug), Some(title)) =
            (field_value(&lifeline.target), field_value(&lifeline.title))
        else {
            continue;
        };
        let alias = field_value(&lifeline.alias).cloned();
        let id = alias.clone().unwrap_or_else(|| title.clone());
        let ref_ = resolve_slug(path, slug, claimed);
        if ref_.is_none() {
            behavior_diagnostic(
                context,
                path,
                &lifeline.syntax.0,
                crate::diagnostic::DiagCode::UnresolvedTarget,
                format!("unresolved lifeline target '{slug}'"),
                diagnostics,
            );
        }
        handles.insert(id.clone());
        handles.insert(title.clone());
        nodes.push(crate::model::SeqNode::Lifeline {
            id,
            title: title.clone(),
            alias,
            ref_,
        });
    }
    enum Ordered<'a> {
        Message(&'a crate::uml::DeclaredMessage),
        Operand(&'a crate::uml::DeclaredSequenceOperand),
    }
    let mut ordered = concept
        .messages
        .iter()
        .map(|value| (value.syntax.0.range().start(), Ordered::Message(value)))
        .chain(
            concept
                .sequence_operands
                .iter()
                .map(|value| (value.syntax.0.range().start(), Ordered::Operand(value))),
        )
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(start, _)| *start);
    let mut edges = Vec::new();
    let mut root = Vec::new();
    let mut fragment_stack: Vec<(usize, usize)> = Vec::new();
    let mut operand_stack: Vec<(usize, usize)> = Vec::new();
    for (_, item) in ordered {
        match item {
            Ordered::Operand(value) => {
                if let Some(kind) = field_value(&value.fragment) {
                    while fragment_stack
                        .last()
                        .is_some_and(|(depth, _)| *depth >= value.depth)
                    {
                        fragment_stack.pop();
                    }
                    while operand_stack
                        .last()
                        .is_some_and(|(depth, _)| *depth >= value.depth)
                    {
                        operand_stack.pop();
                    }
                    let id = format!("f{}", nodes.len());
                    let index = nodes.len();
                    nodes.push(crate::model::SeqNode::Fragment {
                        id: id.clone(),
                        kind: *kind,
                        operands: Vec::new(),
                    });
                    let child = crate::model::SeqChild::Fragment { node: id };
                    if let Some((_, operand)) = operand_stack.last() {
                        if let crate::model::SeqNode::Operand { items, .. } = &mut nodes[*operand] {
                            items.push(child);
                        }
                    } else {
                        root.push(child);
                    }
                    fragment_stack.push((value.depth, index));
                } else {
                    while operand_stack
                        .last()
                        .is_some_and(|(depth, _)| *depth >= value.depth)
                    {
                        operand_stack.pop();
                    }
                    let Some((_, fragment)) = fragment_stack
                        .iter()
                        .rev()
                        .find(|(depth, _)| *depth < value.depth)
                        .copied()
                    else {
                        continue;
                    };
                    let id = format!("o{}", nodes.len());
                    let index = nodes.len();
                    nodes.push(crate::model::SeqNode::Operand {
                        id: id.clone(),
                        guard: field_value(&value.guard).cloned(),
                        items: Vec::new(),
                    });
                    if let crate::model::SeqNode::Fragment { operands, .. } = &mut nodes[fragment] {
                        operands.push(id);
                    }
                    operand_stack.push((value.depth, index));
                }
            }
            Ordered::Message(value) => {
                let (Some(from), Some(verb), Some(to)) = (
                    field_value(&value.from),
                    field_value(&value.verb),
                    field_value(&value.to),
                ) else {
                    continue;
                };
                if !handles.contains(from) || !handles.contains(to) {
                    behavior_diagnostic(
                        context,
                        path,
                        &value.syntax.0,
                        crate::diagnostic::DiagCode::UnresolvedTarget,
                        format!("unresolved sequence participant '{from}' or '{to}'"),
                        diagnostics,
                    );
                    continue;
                }
                while operand_stack
                    .last()
                    .is_some_and(|(depth, _)| *depth >= value.depth)
                {
                    operand_stack.pop();
                }
                let id = format!("m{}", edges.len());
                edges.push(crate::model::SeqEdge {
                    id: id.clone(),
                    from: from.clone(),
                    verb: *verb,
                    to: to.clone(),
                    signature: field_value(&value.signature).cloned(),
                });
                let child = crate::model::SeqChild::Message { edge: id };
                if let Some((_, operand)) = operand_stack.last() {
                    if let crate::model::SeqNode::Operand { items, .. } = &mut nodes[*operand] {
                        items.push(child);
                    }
                } else {
                    root.push(child);
                }
            }
        }
    }
    model.interactions.push(crate::model::SequenceDoc {
        key: concept.concept_id.clone(),
        title: okf
            .title
            .clone()
            .unwrap_or_else(|| concept.concept_id.clone()),
        describes: resolve_describes(okf, path, claimed),
        nodes,
        edges,
        items: root,
    });
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
    let name = match &group.name {
        crate::uml::DeclaredField::Absent => String::new(),
        crate::uml::DeclaredField::Valid { value, .. } => value.clone(),
        crate::uml::DeclaredField::Incomplete { .. }
        | crate::uml::DeclaredField::Invalid { .. } => return None,
    };
    enum Item<'a> {
        Member(&'a crate::uml::DeclaredMember),
        Inline(&'a crate::uml::DeclaredInlineInstance),
    }
    let mut items = group
        .members
        .iter()
        .map(|member| (member.syntax.syntax().range().start(), Item::Member(member)))
        .chain(
            group
                .inline_instances
                .iter()
                .map(|inline| (inline.syntax.syntax().range().start(), Item::Inline(inline))),
        )
        .collect::<Vec<_>>();
    items.sort_by_key(|(start, _)| *start);
    let members = items
        .into_iter()
        .filter_map(|(_, item)| match item {
            Item::Member(member) => match &member.target {
                crate::uml::DeclaredField::Valid { value, .. } => {
                    let target = crate::okf::resolve_href(path, value);
                    claimed.contains(target.as_str()).then_some(target)
                }
                _ => None,
            },
            Item::Inline(inline) => match inline_instance_validity(inline, path, claimed) {
                InlineInstanceValidity::Valid(ValidInlineInstance { name, .. }) => {
                    Some(format!("{owner}#{name}"))
                }
                InlineInstanceValidity::Invalid | InlineInstanceValidity::Unresolved { .. } => None,
            },
        })
        .collect::<Vec<_>>();
    Some(crate::model::DiagramGroup {
        name,
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
    node.children().any(|e| match e {
        SyntaxElement::Token(token) => {
            token.kind() == super::syntax::UmlSyntaxKind::BadToken
                && token.flags().is_bad()
                && !token.flags().is_missing()
        }
        SyntaxElement::Node(child) => {
            (child.kind() == super::syntax::UmlSyntaxKind::SkippedTokensSyntax
                && child.range().start() != child.range().end())
                || has_recovery(&child)
        }
    })
}
fn first_recovery_node(node: &SyntaxNode<UmlLanguage>) -> Option<SyntaxNode<UmlLanguage>> {
    node.children().find_map(|element| {
        let child = element.into_node()?;
        if child.kind() == super::syntax::UmlSyntaxKind::SkippedTokensSyntax {
            Some(child)
        } else {
            first_recovery_node(&child)
        }
    })
}
fn has_missing_kind(node: &SyntaxNode<UmlLanguage>, kind: super::syntax::UmlSyntaxKind) -> bool {
    node.children().any(|e| {
        e.kind() == kind
            && e.into_token()
                .is_some_and(|token| token.flags().is_missing())
    })
}

fn direct_child(
    node: &SyntaxNode<UmlLanguage>,
    kind: super::syntax::UmlSyntaxKind,
) -> Option<SyntaxNode<UmlLanguage>> {
    node.children()
        .find(|element| element.kind() == kind)
        .and_then(SyntaxElement::into_node)
}

fn token_in(
    node: &SyntaxNode<UmlLanguage>,
    kind: super::syntax::UmlSyntaxKind,
) -> Option<waml_syntax::SyntaxToken<UmlLanguage>> {
    node.children().find_map(|element| {
        if element.kind() == kind {
            element.into_token()
        } else {
            element.into_node().and_then(|child| token_in(&child, kind))
        }
    })
}

fn field_from_token(
    node: &SyntaxNode<UmlLanguage>,
    kind: super::syntax::UmlSyntaxKind,
) -> Option<String> {
    token_in(node, kind)
        .filter(|token| !token.flags().is_missing())
        .map(|token| token.text().write_to_string())
}

fn strip_expression(value: String) -> String {
    value
        .strip_prefix('`')
        .and_then(|value| value.strip_suffix('`'))
        .unwrap_or(&value)
        .to_string()
}

fn declared_expression_slot(
    slot: SyntaxNode<UmlLanguage>,
    token_kind: super::syntax::UmlSyntaxKind,
) -> crate::uml::DeclaredField<UmlLanguage, String> {
    field_from_token(&slot, token_kind)
        .map(strip_expression)
        .map(|value| valid(slot.clone(), value))
        .unwrap_or_else(|| {
            if has_recovery(&slot) {
                invalid(slot)
            } else {
                crate::uml::DeclaredField::Incomplete {
                    syntax: slot,
                    expected: crate::uml::ExpectedSyntax::MessageTarget,
                }
            }
        })
}

fn declared_optional_expression_slot(
    slot: SyntaxNode<UmlLanguage>,
    token_kind: super::syntax::UmlSyntaxKind,
) -> crate::uml::DeclaredField<UmlLanguage, String> {
    if let Some(value) = field_from_token(&slot, token_kind) {
        return valid(slot, strip_expression(value));
    }
    if has_recovery(&slot) {
        return invalid_recovery(slot);
    }
    let present = slot
        .child_at(0)
        .and_then(SyntaxElement::into_token)
        .is_some_and(|token| !token.flags().is_missing());
    if present {
        crate::uml::DeclaredField::Incomplete {
            syntax: slot,
            expected: crate::uml::ExpectedSyntax::MessageTarget,
        }
    } else {
        crate::uml::DeclaredField::Absent
    }
}

fn declared_text_slot(
    slot: Option<SyntaxNode<UmlLanguage>>,
) -> crate::uml::DeclaredField<UmlLanguage, String> {
    let Some(slot) = slot else {
        return crate::uml::DeclaredField::Absent;
    };
    field_from_token(&slot, super::syntax::UmlSyntaxKind::ExpressionToken)
        .map(strip_expression)
        .or_else(|| field_from_token(&slot, super::syntax::UmlSyntaxKind::IdentifierToken))
        .map(|value| valid(slot.clone(), value))
        .unwrap_or_else(|| {
            if has_recovery(&slot) {
                invalid(slot)
            } else {
                crate::uml::DeclaredField::Incomplete {
                    syntax: slot,
                    expected: crate::uml::ExpectedSyntax::MessageTarget,
                }
            }
        })
}

fn declared_required_token(
    node: &SyntaxNode<UmlLanguage>,
    slot_kind: super::syntax::UmlSyntaxKind,
    token_kind: super::syntax::UmlSyntaxKind,
    expected: crate::uml::ExpectedSyntax,
) -> crate::uml::DeclaredField<UmlLanguage, String> {
    let slot = direct_child(node, slot_kind).expect("production has fixed occurrence");
    field_from_token(&slot, token_kind)
        .filter(|value| !value.is_empty())
        .map(|value| valid(slot.clone(), value))
        .unwrap_or_else(|| {
            if has_recovery(&slot) {
                invalid(slot)
            } else {
                crate::uml::DeclaredField::Incomplete {
                    syntax: slot,
                    expected,
                }
            }
        })
}

fn link_parts(node: &SyntaxNode<UmlLanguage>) -> Option<(String, String)> {
    if has_recovery(node) {
        return None;
    }
    let title = field_from_token(node, super::syntax::UmlSyntaxKind::LinkTextToken)?;
    let href = field_from_token(node, super::syntax::UmlSyntaxKind::LinkTargetToken)?;
    let slug = href
        .strip_prefix("./")
        .unwrap_or(&href)
        .strip_suffix(".md")
        .unwrap_or(href.strip_prefix("./").unwrap_or(&href))
        .to_string();
    Some((title, slug))
}

fn behavior_depth(node: &SyntaxNode<UmlLanguage>) -> usize {
    node.child_at(0)
        .and_then(SyntaxElement::into_token)
        .filter(|token| token.kind() == super::syntax::UmlSyntaxKind::BulletToken)
        .map(|token| {
            token
                .leading_trivia()
                .iter()
                .map(|trivia| trivia.text.write_to_string().len())
                .sum::<usize>()
                / 2
        })
        .unwrap_or(0)
}

fn invalid_recovery<T>(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredField<UmlLanguage, T> {
    invalid(first_recovery_node(&node).unwrap_or(node))
}

fn has_direct_recovery(node: &SyntaxNode<UmlLanguage>) -> bool {
    node.children()
        .filter_map(SyntaxElement::into_node)
        .any(|child| {
            child.kind() == super::syntax::UmlSyntaxKind::BehaviorRecovery && has_recovery(&child)
        })
}

fn declared_flow_node(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredFlowNode {
    let syntax = super::syntax::FlowNodeSyntax(node.clone());
    let kind_slot = direct_child(&node, super::syntax::UmlSyntaxKind::FlowNodeKindSlot);
    let kind = match kind_slot {
        Some(slot) => match field_from_token(&slot, super::syntax::UmlSyntaxKind::NodeKindToken) {
            Some(token) => crate::model::FlowNodeKind::from_keyword(&token)
                .map(|value| valid(slot.clone(), value))
                .unwrap_or_else(|| invalid(slot)),
            None => valid(slot, crate::model::FlowNodeKind::Plain),
        },
        None => valid(node.clone(), crate::model::FlowNodeKind::Plain),
    };
    let identity_slot = direct_child(&node, super::syntax::UmlSyntaxKind::FlowIdentity)
        .expect("flow node has fixed identity occurrence");
    let identity = if has_direct_recovery(&node) {
        invalid_recovery(node.clone())
    } else if let Some(link) = direct_child(&identity_slot, super::syntax::UmlSyntaxKind::Link) {
        match link_parts(&link) {
            Some((title, _)) => valid(identity_slot.clone(), title),
            None => invalid(identity_slot.clone()),
        }
    } else {
        field_from_token(&identity_slot, super::syntax::UmlSyntaxKind::IdentityToken)
            .filter(|value| !value.is_empty())
            .map(|value| valid(identity_slot.clone(), value))
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: identity_slot.clone(),
                expected: crate::uml::ExpectedSyntax::FlowTarget,
            })
    };
    let object_ref = if matches!(field_value(&kind), Some(crate::model::FlowNodeKind::Object)) {
        direct_child(&identity_slot, super::syntax::UmlSyntaxKind::Link)
            .and_then(|link| link_parts(&link).map(|(_, slug)| (link, slug)))
            .map(|(link, slug)| valid(link, slug))
            .unwrap_or_else(|| {
                if has_recovery(&identity_slot) {
                    invalid(identity_slot.clone())
                } else {
                    crate::uml::DeclaredField::Incomplete {
                        syntax: identity_slot.clone(),
                        expected: crate::uml::ExpectedSyntax::LinkTarget,
                    }
                }
            })
    } else {
        crate::uml::DeclaredField::Absent
    };
    let mut entry = crate::uml::DeclaredField::Absent;
    let mut do_ = crate::uml::DeclaredField::Absent;
    let mut exit = crate::uml::DeclaredField::Absent;
    let mut refines = crate::uml::DeclaredField::Absent;
    let mut partition = crate::uml::DeclaredField::Absent;
    for block in node
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|child| child.kind() == super::syntax::UmlSyntaxKind::FlowInternal)
    {
        let keyword = field_from_token(&block, super::syntax::UmlSyntaxKind::InternalKeywordToken);
        let value_slot = direct_child(&block, super::syntax::UmlSyntaxKind::FlowInternalValue);
        match keyword.as_deref() {
            Some("entry") => entry = declared_text_slot(value_slot),
            Some("do") => do_ = declared_text_slot(value_slot),
            Some("exit") => exit = declared_text_slot(value_slot),
            Some("partition") => partition = declared_text_slot(value_slot),
            Some("refines") => {
                refines = direct_child(&block, super::syntax::UmlSyntaxKind::Link)
                    .and_then(|link| link_parts(&link).map(|(_, slug)| valid(link, slug)))
                    .unwrap_or_else(|| invalid(block.clone()))
            }
            _ => {}
        }
    }
    let transitions = node
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|child| child.kind() == super::syntax::UmlSyntaxKind::FlowTransition)
        .map(declared_flow_transition)
        .collect::<Vec<_>>();
    let notes = node
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|child| child.kind() == super::syntax::UmlSyntaxKind::Value)
        .filter_map(|value| {
            direct_child(&value, super::syntax::UmlSyntaxKind::FlowNoteValue)
                .map(|slot| declared_text_slot(Some(slot)))
        })
        .collect::<Vec<_>>();
    crate::uml::DeclaredFlowNode {
        syntax,
        kind,
        identity,
        object_ref,
        entry,
        do_,
        exit,
        refines,
        partition,
        notes: notes.into(),
        transitions: transitions.into(),
    }
}

fn declared_flow_transition(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredFlowTransition {
    let syntax = super::syntax::FlowTransitionSyntax(node.clone());
    let text_field = |kind, token_kind| {
        direct_child(&node, kind)
            .map(|slot| declared_optional_expression_slot(slot, token_kind))
            .unwrap_or(crate::uml::DeclaredField::Absent)
    };
    let target_slot = direct_child(&node, super::syntax::UmlSyntaxKind::FlowTarget)
        .expect("transition has fixed target occurrence");
    let target = if has_recovery(&node) {
        invalid_recovery(node.clone())
    } else if has_missing_kind(&node, super::syntax::UmlSyntaxKind::ToToken)
        || has_missing_kind(&node, super::syntax::UmlSyntaxKind::FlowKeywordToken)
    {
        invalid(target_slot.clone())
    } else if let Some(link) = direct_child(&target_slot, super::syntax::UmlSyntaxKind::Link) {
        link_parts(&link)
            .map(|(title, slug)| {
                valid(
                    target_slot.clone(),
                    crate::syntax::FlowTargetRef::Link(crate::syntax::LinkRef { title, slug }),
                )
            })
            .unwrap_or_else(|| invalid(target_slot.clone()))
    } else {
        field_from_token(&target_slot, super::syntax::UmlSyntaxKind::TargetToken)
            .filter(|value| !value.is_empty())
            .map(|value| {
                valid(
                    target_slot.clone(),
                    crate::syntax::FlowTargetRef::Local(value),
                )
            })
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: target_slot.clone(),
                expected: crate::uml::ExpectedSyntax::FlowTarget,
            })
    };
    let carries = direct_child(&node, super::syntax::UmlSyntaxKind::FlowCarries)
        .and_then(|slot| {
            let link = direct_child(&slot, super::syntax::UmlSyntaxKind::Link)?;
            if link.range().start() == link.range().end() {
                None
            } else {
                Some(
                    link_parts(&link)
                        .map(|(_, slug)| valid(link.clone(), slug))
                        .unwrap_or_else(|| invalid_recovery(link)),
                )
            }
        })
        .unwrap_or(crate::uml::DeclaredField::Absent);
    let is_else = syntax.else_token().is_some();
    crate::uml::DeclaredFlowTransition {
        syntax,
        trigger: text_field(
            super::syntax::UmlSyntaxKind::FlowTrigger,
            super::syntax::UmlSyntaxKind::TriggerToken,
        ),
        guard: if is_else {
            crate::uml::DeclaredField::Absent
        } else {
            text_field(
                super::syntax::UmlSyntaxKind::FlowGuard,
                super::syntax::UmlSyntaxKind::GuardToken,
            )
        },
        is_else,
        target,
        carries,
        effect: text_field(
            super::syntax::UmlSyntaxKind::FlowEffect,
            super::syntax::UmlSyntaxKind::EffectToken,
        ),
    }
}

fn declared_lifeline(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredLifeline {
    let syntax = super::syntax::LifelineSyntax(node.clone());
    let link = direct_child(&node, super::syntax::UmlSyntaxKind::Link)
        .expect("lifeline has fixed link occurrence");
    let parts = link_parts(&link);
    let missing_link = link.range().start() == link.range().end();
    let target = if has_recovery(&node) {
        invalid_recovery(node.clone())
    } else if missing_link {
        crate::uml::DeclaredField::Incomplete {
            syntax: link.clone(),
            expected: crate::uml::ExpectedSyntax::LinkTarget,
        }
    } else {
        parts
            .as_ref()
            .map(|(_, slug)| valid(link.clone(), slug.clone()))
            .unwrap_or_else(|| invalid(link.clone()))
    };
    let title = if has_recovery(&node) {
        invalid_recovery(node.clone())
    } else if missing_link {
        crate::uml::DeclaredField::Incomplete {
            syntax: link.clone(),
            expected: crate::uml::ExpectedSyntax::LinkTarget,
        }
    } else {
        parts
            .map(|(title, _)| valid(link.clone(), title))
            .unwrap_or_else(|| invalid(link))
    };
    let alias = direct_child(&node, super::syntax::UmlSyntaxKind::LifelineAlias)
        .map(|slot| {
            field_from_token(&slot, super::syntax::UmlSyntaxKind::AliasToken)
                .filter(|value| !value.is_empty())
                .map(|value| valid(slot.clone(), value))
                .unwrap_or_else(|| {
                    let as_present = node
                        .child_at(super::syntax::LifelineSyntax::AS_SLOT)
                        .and_then(SyntaxElement::into_token)
                        .is_some_and(|token| !token.flags().is_missing());
                    if as_present {
                        crate::uml::DeclaredField::Incomplete {
                            syntax: slot,
                            expected: crate::uml::ExpectedSyntax::MessageTarget,
                        }
                    } else {
                        crate::uml::DeclaredField::Absent
                    }
                })
        })
        .unwrap_or(crate::uml::DeclaredField::Absent);
    crate::uml::DeclaredLifeline {
        syntax,
        target,
        title,
        alias,
    }
}

fn declared_message(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredMessage {
    let syntax = super::syntax::MessageSyntax(node.clone());
    let from = declared_required_token(
        &node,
        super::syntax::UmlSyntaxKind::MessageSource,
        super::syntax::UmlSyntaxKind::SourceToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    );
    let to = declared_required_token(
        &node,
        super::syntax::UmlSyntaxKind::MessageTarget,
        super::syntax::UmlSyntaxKind::TargetToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    );
    let verb_slot = direct_child(&node, super::syntax::UmlSyntaxKind::MessageVerb)
        .expect("message has fixed verb occurrence");
    let verb = if has_recovery(&node) {
        invalid_recovery(node.clone())
    } else {
        field_from_token(&verb_slot, super::syntax::UmlSyntaxKind::VerbToken)
            .and_then(|value| crate::model::MessageVerb::parse(&value))
            .map(|value| valid(verb_slot.clone(), value))
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: verb_slot,
                expected: crate::uml::ExpectedSyntax::MessageTarget,
            })
    };
    let signature_slot = direct_child(&node, super::syntax::UmlSyntaxKind::MessageSignature)
        .expect("message has fixed signature occurrence");
    let signature = if field_from_token(
        &signature_slot,
        super::syntax::UmlSyntaxKind::SignatureToken,
    )
    .is_some()
    {
        declared_expression_slot(signature_slot, super::syntax::UmlSyntaxKind::SignatureToken)
    } else if node
        .child_at(super::syntax::MessageSyntax::COLON_SLOT)
        .and_then(SyntaxElement::into_token)
        .is_some_and(|token| !token.flags().is_missing())
    {
        if has_recovery(&node) {
            invalid_recovery(node.clone())
        } else {
            crate::uml::DeclaredField::Incomplete {
                syntax: signature_slot,
                expected: crate::uml::ExpectedSyntax::MessageTarget,
            }
        }
    } else {
        crate::uml::DeclaredField::Absent
    };
    crate::uml::DeclaredMessage {
        syntax,
        from,
        verb,
        to,
        signature,
        depth: behavior_depth(&node),
    }
}

fn declared_sequence_operand(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredSequenceOperand {
    let syntax = super::syntax::SequenceOperandSyntax(node.clone());
    let depth = behavior_depth(&node);
    if node.kind() == super::syntax::UmlSyntaxKind::SequenceFragment {
        let slot = direct_child(&node, super::syntax::UmlSyntaxKind::FragmentKind)
            .expect("fragment has fixed kind occurrence");
        let fragment = if has_recovery(&node) {
            invalid_recovery(node.clone())
        } else {
            field_from_token(&slot, super::syntax::UmlSyntaxKind::FragmentKindToken)
                .and_then(|value| crate::model::FragmentKind::parse(&value))
                .map(|value| valid(slot.clone(), value))
                .unwrap_or_else(|| invalid(slot))
        };
        crate::uml::DeclaredSequenceOperand {
            syntax,
            fragment,
            guard: crate::uml::DeclaredField::Absent,
            is_else: false,
            depth,
        }
    } else {
        let keyword = field_from_token(&node, super::syntax::UmlSyntaxKind::OperandKeywordToken);
        let guard = if has_recovery(&node) {
            invalid_recovery(node.clone())
        } else if keyword.as_deref() == Some("else") {
            crate::uml::DeclaredField::Absent
        } else {
            direct_child(&node, super::syntax::UmlSyntaxKind::OperandGuard)
                .map(|slot| {
                    declared_expression_slot(slot, super::syntax::UmlSyntaxKind::GuardToken)
                })
                .unwrap_or(crate::uml::DeclaredField::Absent)
        };
        crate::uml::DeclaredSequenceOperand {
            syntax,
            fragment: crate::uml::DeclaredField::Absent,
            guard,
            is_else: keyword.as_deref() == Some("else"),
            depth,
        }
    }
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
fn declared_layout(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredField<UmlLanguage, crate::uml::DeclaredLayoutStatement> {
    let syntax = super::syntax::LayoutStatementSyntax(node.clone());
    let atoms = syntax.typed_atoms().collect::<Vec<_>>();
    let missing_atom = atoms.iter().any(|atom| {
        let token = match atom {
            super::syntax::LayoutAtomSyntax::Word(token)
            | super::syntax::LayoutAtomSyntax::Link(token)
            | super::syntax::LayoutAtomSyntax::Quote(token)
            | super::syntax::LayoutAtomSyntax::OpenParen(token)
            | super::syntax::LayoutAtomSyntax::CloseParen(token)
            | super::syntax::LayoutAtomSyntax::Comma(token) => token,
        };
        token.flags().is_missing() || token.text().write_to_string().trim().is_empty()
    });
    if has_recovery(&node) {
        let recovery = first_recovery_node(&node).expect("recovery predicate found a node");
        return crate::uml::DeclaredField::Incomplete {
            syntax: recovery,
            expected: crate::uml::ExpectedSyntax::LayoutOperand,
        };
    }
    if atoms.is_empty() || missing_atom {
        return crate::uml::DeclaredField::Incomplete {
            syntax: node,
            expected: crate::uml::ExpectedSyntax::LayoutOperand,
        };
    }
    match parse_layout_atoms(&atoms) {
        Some(crate::syntax::LayoutStatement::Placement {
            operands,
            directions,
        }) => {
            let Some(slots) = syntax.placement() else {
                return invalid(node);
            };
            let operand_slots = slots.operands().map(|slot| slot.0).collect::<Vec<_>>();
            let direction_slots = slots.directions().map(|slot| slot.0).collect::<Vec<_>>();
            if operand_slots.len() != operands.len() || direction_slots.len() != directions.len() {
                return invalid(node);
            }
            valid(
                node.clone(),
                crate::uml::DeclaredLayoutStatement::Placement {
                    operands: operands
                        .into_iter()
                        .zip(operand_slots)
                        .map(|(value, slot)| valid(slot, value))
                        .collect::<Vec<_>>()
                        .into(),
                    directions: directions
                        .into_iter()
                        .zip(direction_slots)
                        .map(|(value, slot)| valid(slot, value))
                        .collect::<Vec<_>>()
                        .into(),
                },
            )
        }
        Some(crate::syntax::LayoutStatement::Alignment { left, right }) => {
            let Some(alignment) = syntax.alignment() else {
                return invalid(node);
            };
            let slots = alignment.anchored().map(|slot| slot.0).collect::<Vec<_>>();
            let [left_slot, right_slot] = slots.as_slice() else {
                return invalid(node);
            };
            valid(
                node.clone(),
                crate::uml::DeclaredLayoutStatement::Alignment {
                    left: valid(left_slot.clone(), left),
                    right: valid(right_slot.clone(), right),
                },
            )
        }
        Some(crate::syntax::LayoutStatement::Standalone(operand)) => {
            let Some(slot) = syntax
                .standalone()
                .and_then(|standalone| standalone.operand())
            else {
                return invalid(node);
            };
            valid(
                node.clone(),
                crate::uml::DeclaredLayoutStatement::Standalone(valid(slot.0, operand)),
            )
        }
        None => invalid(node),
    }
}

/// The layout declaration cursor consumes the lossless UML atoms directly.
/// It intentionally never reconstructs a Markdown body or calls the legacy
/// string lexer/parser: whitespace is trivia in the syntax tree, while every
/// grammatical decision is made from the authored token kind and spelling.
struct LayoutCursor<'a> {
    atoms: &'a [super::syntax::LayoutAtomSyntax],
    pos: usize,
}

impl<'a> LayoutCursor<'a> {
    fn peek(&self) -> Option<&'a super::syntax::LayoutAtomSyntax> {
        self.atoms.get(self.pos)
    }
    fn bump(&mut self) -> Option<&'a super::syntax::LayoutAtomSyntax> {
        let atom = self.peek();
        if atom.is_some() {
            self.pos += 1;
        }
        atom
    }
    fn word(&self) -> Option<String> {
        match self.peek()? {
            // The lossless lexer attaches leading horizontal trivia to its
            // following atom so the green tree round-trips byte-for-byte.
            // Treat that trivia as trivia here; do not concatenate atoms into
            // a synthetic source string merely to get this behavior.
            super::syntax::LayoutAtomSyntax::Word(token) => {
                Some(token.text().write_to_string().trim().to_string())
            }
            _ => None,
        }
    }
    fn eat_word(&mut self, expected: &str) -> bool {
        self.word()
            .is_some_and(|word| word.eq_ignore_ascii_case(expected))
            && {
                self.pos += 1;
                true
            }
    }
    fn eat_comma(&mut self) -> bool {
        matches!(self.peek(), Some(super::syntax::LayoutAtomSyntax::Comma(_))) && {
            self.pos += 1;
            true
        }
    }
    fn done(&self) -> bool {
        self.pos == self.atoms.len()
    }
}

fn parse_layout_atoms(
    atoms: &[super::syntax::LayoutAtomSyntax],
) -> Option<crate::syntax::LayoutStatement> {
    let mut cur = LayoutCursor { atoms, pos: 0 };
    let first = parse_layout_anchored(&mut cur)?;
    if cur.eat_word("aligned") {
        if !cur.eat_word("with") {
            return None;
        }
        let right = parse_layout_anchored(&mut cur)?;
        return cur
            .done()
            .then_some(crate::syntax::LayoutStatement::Alignment { left: first, right });
    }
    let first = match first.edge {
        Some(_) => return None,
        None => first.operand,
    };
    let Some(direction) = parse_layout_direction(&mut cur) else {
        return cur
            .done()
            .then_some(crate::syntax::LayoutStatement::Standalone(first));
    };
    let mut operands = vec![first, parse_layout_operand(&mut cur)?];
    let mut directions = vec![direction];
    while let Some(direction) = parse_layout_direction(&mut cur) {
        directions.push(direction);
        operands.push(parse_layout_operand(&mut cur)?);
    }
    cur.done()
        .then_some(crate::syntax::LayoutStatement::Placement {
            operands,
            directions,
        })
}

fn parse_layout_anchored(cur: &mut LayoutCursor<'_>) -> Option<crate::syntax::Anchored> {
    let edge = match cur.word().map(|word| word.to_ascii_lowercase()).as_deref() {
        Some("top") => Some(crate::syntax::Edge::Top),
        Some("bottom") => Some(crate::syntax::Edge::Bottom),
        Some("left") => Some(crate::syntax::Edge::Left),
        Some("right") => Some(crate::syntax::Edge::Right),
        Some("center") => Some(crate::syntax::Edge::Center),
        _ => None,
    };
    if let Some(edge) = edge {
        cur.bump();
        if cur.eat_word("of") {
            return Some(crate::syntax::Anchored {
                edge: Some(edge),
                operand: parse_layout_operand(cur)?,
            });
        }
        cur.pos -= 1;
    }
    Some(crate::syntax::Anchored {
        edge: None,
        operand: parse_layout_operand(cur)?,
    })
}

fn parse_layout_direction(cur: &mut LayoutCursor<'_>) -> Option<crate::syntax::Direction> {
    let word = cur.word()?.to_ascii_lowercase();
    match word.as_str() {
        "above" | "below" => {
            cur.bump();
            let diagonal = cur.word().map(|word| word.to_ascii_lowercase());
            let direction = match (word.as_str(), diagonal.as_deref()) {
                ("above", Some("left")) => crate::syntax::Direction::AboveLeft,
                ("above", Some("right")) => crate::syntax::Direction::AboveRight,
                ("below", Some("left")) => crate::syntax::Direction::BelowLeft,
                ("below", Some("right")) => crate::syntax::Direction::BelowRight,
                ("above", _) => return Some(crate::syntax::Direction::Above),
                _ => return Some(crate::syntax::Direction::Below),
            };
            cur.bump();
            if cur.eat_word("of") {
                Some(direction)
            } else {
                None
            }
        }
        "left" | "right" => {
            cur.bump();
            if !cur.eat_word("of") {
                return None;
            }
            Some(if word == "left" {
                crate::syntax::Direction::LeftOf
            } else {
                crate::syntax::Direction::RightOf
            })
        }
        _ => None,
    }
}

fn parse_layout_operand(cur: &mut LayoutCursor<'_>) -> Option<crate::syntax::Operand> {
    let ref_ = parse_layout_ref(cur)?;
    let axis = if cur.eat_word("as") {
        Some(parse_layout_axis(cur)?)
    } else {
        None
    };
    let hints = if cur.eat_word("with") {
        parse_layout_hints(cur)?
    } else {
        vec![]
    };
    Some(crate::syntax::Operand { ref_, axis, hints })
}

fn parse_layout_axis(cur: &mut LayoutCursor<'_>) -> Option<crate::syntax::Axis> {
    let word = cur.word()?.to_ascii_lowercase();
    cur.bump();
    match word.as_str() {
        "row" => Some(crate::syntax::Axis::Row),
        "column" => Some(crate::syntax::Axis::Column),
        _ => None,
    }
}

fn parse_layout_hints(cur: &mut LayoutCursor<'_>) -> Option<Vec<crate::syntax::Hint>> {
    let mut hints = vec![parse_layout_hint(cur)?];
    while cur.eat_comma() || cur.eat_word("and") {
        hints.push(parse_layout_hint(cur)?);
    }
    Some(hints)
}

fn parse_layout_hint(cur: &mut LayoutCursor<'_>) -> Option<crate::syntax::Hint> {
    use crate::syntax::{Flag, Hint, Margin, Shape};
    let word = cur.word()?.to_ascii_lowercase();
    cur.bump();
    match word.as_str() {
        "frame" => Some(Hint::Shape(Shape::Frame)),
        "box" => Some(Hint::Shape(Shape::Box)),
        "shrink" => Some(Hint::Shape(Shape::Shrink)),
        "emphasized" => Some(Hint::Flag(Flag::Emphasized)),
        "collapsed" => Some(Hint::Flag(Flag::Collapsed)),
        "no" | "small" | "medium" | "large" => {
            let margin = match word.as_str() {
                "no" => Margin::No,
                "small" => Margin::Small,
                "medium" => Margin::Medium,
                _ => Margin::Large,
            };
            (cur.eat_word("margin") || cur.eat_word("margins")).then_some(Hint::Margin(margin))
        }
        _ => None,
    }
}

fn parse_layout_ref(cur: &mut LayoutCursor<'_>) -> Option<crate::syntax::OperandRef> {
    use super::syntax::LayoutAtomSyntax::{Link, OpenParen, Quote, Word};
    use crate::syntax::{Axis, NameRef, OperandRef};
    match cur.bump()? {
        OpenParen(_) => {
            let operand = parse_layout_operand(cur)?;
            matches!(
                cur.bump(),
                Some(super::syntax::LayoutAtomSyntax::CloseParen(_))
            )
            .then_some(OperandRef::Paren(Box::new(operand)))
        }
        Link(token) => {
            let raw = token.text().write_to_string();
            let raw = raw.trim();
            let (title, path) = raw.strip_prefix('[')?.split_once("](./")?;
            let slug = path.strip_suffix(".md)")?;
            Some(OperandRef::Name(NameRef::Link {
                title: title.to_string(),
                slug: slug.to_string(),
            }))
        }
        Quote(token) => {
            let raw = token.text().write_to_string();
            let raw = raw.trim();
            Some(OperandRef::Name(NameRef::Bare(
                raw.strip_prefix('"')?.strip_suffix('"')?.to_string(),
            )))
        }
        Word(token) => {
            let word = token.text().write_to_string().trim().to_string();
            let lower = word.to_ascii_lowercase();
            if lower == "column" || lower == "row" {
                if !cur.eat_word("of") {
                    return None;
                }
                let axis = if lower == "column" {
                    Axis::Column
                } else {
                    Axis::Row
                };
                let mut items = vec![parse_layout_operand(cur)?];
                while cur.eat_comma() {
                    items.push(parse_layout_operand(cur)?);
                }
                Some(OperandRef::InlineGroup { axis, items })
            } else {
                Some(OperandRef::Name(NameRef::Bare(word)))
            }
        }
        _ => None,
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
    let name = match syntax.heading_token() {
        Some(token)
            if !token.flags().is_missing() && !token.text().write_to_string().is_empty() =>
        {
            valid(node.clone(), token.text().write_to_string())
        }
        Some(_) => crate::uml::DeclaredField::Incomplete {
            syntax: node.clone(),
            expected: crate::uml::ExpectedSyntax::LinkTarget,
        },
        None if node
            .children()
            .all(|element| element.kind() != super::syntax::UmlSyntaxKind::HeadingMarkerToken) =>
        {
            crate::uml::DeclaredField::Absent
        }
        None => crate::uml::DeclaredField::Incomplete {
            syntax: node.clone(),
            expected: crate::uml::ExpectedSyntax::LinkTarget,
        },
    };
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
