use super::{
    declared::{DeclaredAttribute, DeclaredBundle},
    syntax::{parser, UmlLanguage},
};
use crate::{
    analysis::{AnalysisError, ClaimSet, DomainAnalysisContext, SyntaxSet, SyntaxSnapshot},
    diagnostic::Diagnostic,
};
use std::{collections::BTreeMap, sync::Arc};
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
                    .map(|node| declared_value(node, document.text().shared()))
                    .collect::<Vec<_>>()
                    .into(),
                slots: slots
                    .into_iter()
                    .map(|node| declared_slot(node, document.text().shared()))
                    .collect::<Vec<_>>()
                    .into(),
                relationships: relationships
                    .into_iter()
                    .map(|node| declared_relationship(node, document.text().shared()))
                    .collect::<Vec<_>>()
                    .into(),
                members: members
                    .into_iter()
                    .map(|node| declared_member(node, document.text().shared()))
                    .collect::<Vec<_>>()
                    .into(),
                inline_instances: inline_instances
                    .into_iter()
                    .map(|node| declared_inline_instance(node, document.text().shared()))
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
    let mut projection = super::project(context.okf);
    for node in &mut projection.nodes {
        if let Some(concept) = declared.concept(&node.key) {
            node.attributes = concept
                .attributes
                .iter()
                .filter_map(DeclaredAttribute::validated)
                .collect();
        }
    }
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
fn raw(node: &SyntaxNode<UmlLanguage>, source: &str) -> String {
    let range = node.range();
    source[range.start().to_usize()..range.end().to_usize()].to_owned()
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
fn declared_value(node: SyntaxNode<UmlLanguage>, source: &str) -> crate::uml::DeclaredValue {
    let value = crate::grammar::parse_value_line(&raw(&node, source)).unwrap_or_default();
    crate::uml::DeclaredValue {
        syntax: super::syntax::ValueSyntax(node.clone()),
        value: if value.is_empty() {
            invalid(node)
        } else {
            valid(node, value)
        },
    }
}
fn declared_slot(node: SyntaxNode<UmlLanguage>, source: &str) -> crate::uml::DeclaredSlot {
    let line = raw(&node, source);
    match crate::grammar::parse_slot_line(&line) {
        Ok(slot) => crate::uml::DeclaredSlot {
            syntax: super::syntax::SlotSyntax(node.clone()),
            name: valid(node.clone(), slot.name),
            value: valid(node, crate::grammar::render_slot_value(&slot.value)),
        },
        Err(_) => crate::uml::DeclaredSlot {
            syntax: super::syntax::SlotSyntax(node.clone()),
            name: invalid(node.clone()),
            value: invalid(node),
        },
    }
}
fn declared_relationship(
    node: SyntaxNode<UmlLanguage>,
    source: &str,
) -> crate::uml::DeclaredRelationship {
    let line = raw(&node, source);
    match crate::grammar::parse_relationship_line(&line) {
        Ok(rel) => crate::uml::DeclaredRelationship {
            syntax: super::syntax::RelationshipSyntax(node.clone()),
            kind: valid(node.clone(), rel.kind),
            target: valid(node, rel.target_slug),
        },
        Err(_) => crate::uml::DeclaredRelationship {
            syntax: super::syntax::RelationshipSyntax(node.clone()),
            kind: invalid(node.clone()),
            target: invalid(node),
        },
    }
}
fn declared_member(node: SyntaxNode<UmlLanguage>, source: &str) -> crate::uml::DeclaredMember {
    let line = raw(&node, source);
    match crate::grammar::parse_member_line(&line) {
        Ok(member) => crate::uml::DeclaredMember {
            syntax: super::syntax::MemberSyntax(node.clone()),
            target: valid(node, member.slug),
        },
        Err(_) => crate::uml::DeclaredMember {
            syntax: super::syntax::MemberSyntax(node.clone()),
            target: invalid(node),
        },
    }
}
fn declared_inline_instance(
    node: SyntaxNode<UmlLanguage>,
    source: &str,
) -> crate::uml::DeclaredInlineInstance {
    let line = raw(&node, source);
    match crate::grammar::parse_inline_instance(&line) {
        Ok(instance) => crate::uml::DeclaredInlineInstance {
            syntax: super::syntax::InlineInstanceSyntax(node.clone()),
            classifier: valid(node.clone(), instance.classifier.slug),
            name: valid(node, instance.name),
        },
        Err(_) => crate::uml::DeclaredInlineInstance {
            syntax: super::syntax::InlineInstanceSyntax(node.clone()),
            classifier: invalid(node.clone()),
            name: invalid(node),
        },
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
