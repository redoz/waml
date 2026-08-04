use super::{
    declared::{DeclaredAttribute, DeclaredBundle},
    syntax::{self, UmlLanguage},
};
use crate::{
    analysis::{
        single_text_change, AffectedAnalysis, AnalysisError, ClaimSet, DiagnosticSource,
        DocumentId, DomainAnalysisContext, ProjectionFreshness, RevisionedDiagnostic, SyntaxSet,
        SyntaxSnapshot,
    },
    diagnostic::Diagnostic,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::Arc,
};
use waml_syntax::{
    AstNode, ChangeMap, SyntaxElement, SyntaxIdentity, SyntaxNode, SyntaxToken, SyntaxTree,
    TextRange, TextSize, WamlSectionKind,
};
pub struct Analysis {
    pub claims: ClaimSet,
    pub syntax: SyntaxSet<UmlLanguage>,
    pub island_syntax: UmlIslandSyntaxSet,
    pub declared: DeclaredBundle,
    pub projection: super::Projection,
    pub diagnostics: Arc<[Diagnostic]>,
    pub markdown: crate::analysis::MarkdownSyntaxSet,
    affected: AffectedAnalysis,
    projection_freshness: BTreeMap<SyntaxIdentity, ProjectionFreshness>,
    diagram_projections: BTreeMap<Arc<str>, Arc<crate::model::Diagram>>,
    revisioned_diagnostics: Arc<[RevisionedDiagnostic]>,
    session_revision: u64,
}

pub struct UmlIslandSyntaxSnapshot {
    owner: SyntaxIdentity,
    kind: WamlSectionKind,
    source_range: TextRange,
    content_range: TextRange,
    syntax: Arc<SyntaxTree<UmlLanguage>>,
}

type UmlIslandSyntaxKey = (SyntaxIdentity, TextRange);
type UmlIslandDocument = HashMap<UmlIslandSyntaxKey, Arc<UmlIslandSyntaxSnapshot>>;
type UmlIslandDocuments = BTreeMap<DocumentId, Arc<UmlIslandDocument>>;

impl UmlIslandSyntaxSnapshot {
    pub fn owner(&self) -> SyntaxIdentity {
        self.owner
    }

    pub fn kind(&self) -> WamlSectionKind {
        self.kind
    }

    pub fn content_range(&self) -> TextRange {
        self.content_range
    }

    pub fn source_range(&self) -> TextRange {
        self.source_range
    }

    pub fn syntax(&self) -> &Arc<SyntaxTree<UmlLanguage>> {
        &self.syntax
    }
}

pub struct UmlIslandSyntaxSet {
    documents: Arc<UmlIslandDocuments>,
}

impl UmlIslandSyntaxSet {
    pub fn document(&self, id: DocumentId) -> Option<&Arc<UmlIslandDocument>> {
        self.documents.get(&id)
    }

    pub fn by_owner(
        &self,
        id: DocumentId,
        owner: SyntaxIdentity,
    ) -> Option<&Arc<UmlIslandSyntaxSnapshot>> {
        self.document(id)?
            .values()
            .find(|snapshot| snapshot.owner == owner)
    }
}

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    use super::*;

    pub fn island_syntax_with_replaced_tree(
        analysis: &Analysis,
        document: crate::analysis::DocumentId,
        owner: SyntaxIdentity,
        syntax: Arc<waml_syntax::SyntaxTree<UmlLanguage>>,
    ) -> Result<UmlIslandSyntaxSet, AnalysisError> {
        let snapshot = analysis
            .island_syntax
            .by_owner(document, owner)
            .ok_or_else(|| AnalysisError::Specialization {
                name: "uml",
                reason: "test UML island syntax replacement is missing".into(),
            })?;
        if syntax.write_to_string() != snapshot.syntax.write_to_string() {
            return Err(AnalysisError::Specialization {
                name: "uml",
                reason: "test UML island syntax replacement does not match island text".into(),
            });
        }
        let mut documents = analysis.island_syntax.documents.as_ref().clone();
        let mut islands = documents.get(&document).unwrap().as_ref().clone();
        islands.insert(
            (snapshot.owner, snapshot.content_range),
            Arc::new(UmlIslandSyntaxSnapshot {
                owner: snapshot.owner,
                kind: snapshot.kind,
                source_range: snapshot.source_range,
                content_range: snapshot.content_range,
                syntax,
            }),
        );
        documents.insert(document, Arc::new(islands));
        Ok(UmlIslandSyntaxSet {
            documents: Arc::new(documents),
        })
    }
}
impl Analysis {
    pub fn session_revision(&self) -> u64 {
        self.session_revision
    }

    pub fn affected(&self) -> &AffectedAnalysis {
        &self.affected
    }

    pub fn projection_freshness(&self, island: SyntaxIdentity) -> ProjectionFreshness {
        self.projection_freshness
            .get(&island)
            .copied()
            .unwrap_or(ProjectionFreshness::Current)
    }

    pub fn diagram(&self, key: &str) -> Option<&Arc<crate::model::Diagram>> {
        self.diagram_projections.get(key)
    }

    pub fn revisioned_diagnostics(&self) -> &[RevisionedDiagnostic] {
        &self.revisioned_diagnostics
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
                syntax::UmlSyntaxKind::Attribute,
                syntax::UmlSyntaxKind::Relationship,
                syntax::UmlSyntaxKind::Member,
                syntax::UmlSyntaxKind::InlineInstance,
                syntax::UmlSyntaxKind::LayoutStatement,
            ]
            .into_iter()
            .flat_map(|kind| syntax_nodes(snapshot.syntax(), kind))
            .any(|syntax| {
                let range = syntax.range();
                let authored = &source[range.start().to_usize()..range.end().to_usize()];
                authored.contains(&href)
                    || (syntax.kind() == syntax::UmlSyntaxKind::LayoutStatement
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
    kind: syntax::UmlSyntaxKind,
) -> Vec<SyntaxNode<UmlLanguage>> {
    fn collect(
        node: &SyntaxNode<UmlLanguage>,
        kind: syntax::UmlSyntaxKind,
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
    previous: Option<&Analysis>,
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
    let mut island_snapshots = BTreeMap::new();
    let mut declared = DeclaredBundle::default();
    let mut diagnostics = Vec::new();
    // Index the catalog by concept id once, instead of scanning every document
    // per claimed concept (O(concepts × documents)).
    let mut concept_documents: BTreeMap<String, DocumentId> = BTreeMap::new();
    let mut concept_paths: BTreeMap<String, String> = BTreeMap::new();
    for (id, document) in context.catalog.documents().iter() {
        // First document wins on a duplicate id, matching the scan this replaces.
        let concept_id = crate::okf::id_of(document.path().as_str());
        concept_documents.entry(concept_id.clone()).or_insert(*id);
        concept_paths
            .entry(concept_id)
            .or_insert_with(|| document.path().as_str().to_string());
    }
    for concept in claimed {
        let id = *concept_documents.get(concept.id.as_str()).ok_or_else(|| {
            AnalysisError::CatalogInvariant {
                reason: "claimed concept has no document".into(),
            }
        })?;
        let markdown_snapshot =
            context
                .markdown
                .document(id)
                .ok_or_else(|| AnalysisError::CatalogInvariant {
                    reason: "claimed concept has no Markdown syntax snapshot".into(),
                })?;
        let document = context.catalog.document(id).cloned().ok_or_else(|| {
            AnalysisError::CatalogInvariant {
                reason: "claimed concept has no catalog document".into(),
            }
        })?;
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
        let structure = markdown_snapshot.structure();
        let previous_document = previous.and_then(|analysis| analysis.syntax.document(id));
        let previous_islands = previous.and_then(|analysis| analysis.island_syntax.document(id));
        let changes = previous_document
            .map(|snapshot| single_text_change(snapshot.document().text(), document.text()))
            .unwrap_or_default();
        let change_map = previous_document
            .and_then(|snapshot| ChangeMap::checked(snapshot.document().text(), &changes).ok());
        let mut islands = HashMap::new();
        let mut island_trees = HashMap::new();
        for island in structure.islands.iter() {
            let source_range =
                TextRange::new(island.heading_range.start(), island.content_range.end()).map_err(
                    |_| AnalysisError::CatalogInvariant {
                        reason: "UML Markdown island has a reversed source range".into(),
                    },
                )?;
            let reusable = previous_islands
                .and_then(|snapshots| {
                    snapshots
                        .values()
                        .find(|snapshot| snapshot.owner == island.owner)
                })
                .filter(|snapshot| snapshot.kind == island.kind)
                .filter(|snapshot| {
                    if previous_document
                        .is_some_and(|previous| Arc::ptr_eq(previous.document(), &document))
                    {
                        return snapshot.source_range == source_range
                            && snapshot.content_range == island.content_range;
                    }
                    change_map.as_ref().is_some_and(|map| {
                        map.translate_unchanged(snapshot.source_range) == Some(source_range)
                            && map.translate_unchanged(snapshot.content_range)
                                == Some(island.content_range)
                    })
                });
            let island_tree = match reusable.map(|snapshot| snapshot.syntax.clone()) {
                Some(tree) => tree,
                // Seam invariant (structure map identifies its islands): a
                // break here must surface as an error, not panic the editor
                // in-process or poison the wasm instance.
                None => syntax::parse_authoritative_island(
                    document.text().clone(),
                    structure,
                    island.owner,
                    island.content_range,
                )
                .ok_or_else(|| AnalysisError::Specialization {
                    name: "uml",
                    reason: "validated Markdown structure does not identify its UML island".into(),
                })?,
            };
            let key = (island.owner, island.content_range);
            island_trees.insert(key, island_tree.clone());
            islands.insert(
                key,
                Arc::new(UmlIslandSyntaxSnapshot {
                    owner: island.owner,
                    kind: island.kind,
                    source_range,
                    content_range: island.content_range,
                    syntax: island_tree,
                }),
            );
        }
        let tree =
            syntax::compose_full_from_islands(document.text().clone(), structure, &island_trees)
                .ok_or_else(|| AnalysisError::Specialization {
                    name: "uml",
                    reason:
                        "UML compatibility tree could not be composed from authoritative islands"
                            .into(),
                })?;
        island_snapshots.insert(id, Arc::new(islands));
        let attributes = attributes(tree.root());
        let values = items(tree.root(), syntax::UmlSyntaxKind::Value);
        let slots = items(tree.root(), syntax::UmlSyntaxKind::Slot);
        let relationships = items(tree.root(), syntax::UmlSyntaxKind::Relationship);
        let members = items(tree.root(), syntax::UmlSyntaxKind::Member);
        let member_groups = direct_section_items(
            tree.root(),
            syntax::UmlSyntaxKind::MembersSection,
            syntax::UmlSyntaxKind::MemberGroup,
        );
        let inline_instances = items(tree.root(), syntax::UmlSyntaxKind::InlineInstance);
        let layout = items(tree.root(), syntax::UmlSyntaxKind::LayoutStatement);
        let flow_nodes = items(tree.root(), syntax::UmlSyntaxKind::FlowNode);
        let lifelines = items(tree.root(), syntax::UmlSyntaxKind::Lifeline);
        let gates = items(tree.root(), syntax::UmlSyntaxKind::Gate);
        let messages = items(tree.root(), syntax::UmlSyntaxKind::Message);
        let sequence_operands = items(tree.root(), syntax::UmlSyntaxKind::SequenceOperand);
        let sequence_fragments = items(tree.root(), syntax::UmlSyntaxKind::SequenceFragment);
        let interaction_uses = items(tree.root(), syntax::UmlSyntaxKind::InteractionUse);
        let mut fields = Vec::new();
        for syntax in attributes {
            let name = syntax.name_token().text().write_to_string();
            let name_node = syntax.syntax().clone();
            let ty = syntax.type_syntax();
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
                Some(typed) => {
                    let node = typed.syntax().clone();
                    let (name, ref_) = match (typed.link_text_token(), typed.link_target_token()) {
                        (Some(label), Some(href)) => {
                            let href = href.text().write_to_string();
                            let resolved =
                                crate::okf::resolve_href(document.path().as_str(), &href);
                            (
                                label.text().write_to_string(),
                                context
                                    .okf
                                    .concept(&resolved)
                                    .filter(|target| super::recognizes(target))
                                    .map(|_| resolved),
                            )
                        }
                        _ => (
                            node.children()
                                .find_map(|e| e.into_token())
                                .map(|t| t.text().write_to_string())
                                .unwrap_or_default(),
                            None,
                        ),
                    };
                    crate::uml::DeclaredField::Valid {
                        value: crate::model::TypeRef { name, ref_ },
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
                        element.kind() == syntax::UmlSyntaxKind::CloseBracketToken
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
            fields.push(DeclaredAttribute {
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
                .map_err(|_| AnalysisError::CatalogInvariant {
                    reason: "layout diagnostic start is not a document offset".into(),
                })?;
            let end = document
                .line_index()
                .line_col(document.text(), range.end())
                .map_err(|_| AnalysisError::CatalogInvariant {
                    reason: "layout diagnostic end is not a document offset".into(),
                })?;
            diagnostics.push(
                Diagnostic::new(
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
                gates: gates
                    .into_iter()
                    .map(declared_gate)
                    .collect::<Vec<_>>()
                    .into(),
                messages: messages
                    .into_iter()
                    .map(declared_message)
                    .collect::<Vec<_>>()
                    .into(),
                fragments: sequence_fragments
                    .into_iter()
                    .map(declared_fragment)
                    .collect::<Vec<_>>()
                    .into(),
                operands: sequence_operands
                    .into_iter()
                    .map(declared_operand)
                    .collect::<Vec<_>>()
                    .into(),
                interaction_uses: interaction_uses
                    .into_iter()
                    .map(declared_interaction_use)
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
                .map_err(|_| AnalysisError::CatalogInvariant {
                    reason: "parser diagnostic start is not a document offset".into(),
                })?;
            let end_line = document
                .line_index()
                .line_col(document.text(), end)
                .map_err(|_| AnalysisError::CatalogInvariant {
                    reason: "parser diagnostic end is not a document offset".into(),
                })?;
            diagnostics.push(
                Diagnostic::new(
                    match diagnostic.code {
                        syntax::UmlSyntaxDiagnosticCode::MalformedFlow
                        | syntax::UmlSyntaxDiagnosticCode::MalformedIndentation => {
                            crate::diagnostic::DiagCode::MalformedFlowBullet
                        }
                        syntax::UmlSyntaxDiagnosticCode::MalformedLifeline => {
                            crate::diagnostic::DiagCode::MalformedLifeline
                        }
                        syntax::UmlSyntaxDiagnosticCode::MalformedMessage
                        | syntax::UmlSyntaxDiagnosticCode::UnsupportedSequenceForm => {
                            crate::diagnostic::DiagCode::MalformedMessage
                        }
                        syntax::UmlSyntaxDiagnosticCode::UnresolvedTarget => {
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
        let snapshot = previous
            .and_then(|analysis| analysis.syntax.document(id))
            .filter(|previous_snapshot| Arc::ptr_eq(previous_snapshot.document(), &document))
            .cloned()
            .unwrap_or_else(|| Arc::new(SyntaxSnapshot::new(document.clone(), tree)));
        snapshots.insert(id, snapshot);
    }
    validate_declared_semantics(&context, &declared, &concept_paths, &mut diagnostics)?;
    let projection = declared_projection(&context, &declared, &mut diagnostics)?;
    let metadata = analysis_metadata(
        &context,
        previous,
        &island_snapshots,
        &projection,
        &diagnostics,
    );
    Ok(Analysis {
        claims,
        syntax: SyntaxSet::from_snapshots(context.catalog.clone(), snapshots),
        island_syntax: UmlIslandSyntaxSet {
            documents: Arc::new(island_snapshots),
        },
        declared,
        projection,
        diagnostics: diagnostics.into(),
        markdown: context.markdown.clone(),
        affected: metadata.affected,
        projection_freshness: metadata.projection_freshness,
        diagram_projections: metadata.diagram_projections,
        revisioned_diagnostics: metadata.revisioned_diagnostics,
        session_revision: context.session_revision,
    })
}

struct AnalysisMetadata {
    affected: AffectedAnalysis,
    projection_freshness: BTreeMap<SyntaxIdentity, ProjectionFreshness>,
    diagram_projections: BTreeMap<Arc<str>, Arc<crate::model::Diagram>>,
    revisioned_diagnostics: Arc<[RevisionedDiagnostic]>,
}

fn analysis_metadata(
    context: &DomainAnalysisContext<'_>,
    previous: Option<&Analysis>,
    islands: &UmlIslandDocuments,
    projection: &super::Projection,
    diagnostics: &[Diagnostic],
) -> AnalysisMetadata {
    let mut affected_documents = BTreeSet::new();
    for (document, current) in context.markdown.documents() {
        let changed = previous
            .and_then(|analysis| analysis.markdown.document(*document))
            .map_or(true, |prior| !Arc::ptr_eq(prior, current));
        if changed {
            affected_documents.insert(*document);
        }
    }

    let mut affected_islands = BTreeSet::new();
    let mut failed_islands = BTreeMap::new();
    for (document, current) in islands {
        let prior = previous.and_then(|analysis| analysis.island_syntax.document(*document));
        for snapshot in current.values() {
            let prior_snapshot = prior.and_then(|prior| {
                prior
                    .values()
                    .find(|candidate| candidate.owner == snapshot.owner)
            });
            let changed = prior_snapshot.map_or(true, |prior| {
                !Arc::ptr_eq(&prior.syntax, &snapshot.syntax)
                    || prior.source_range != snapshot.source_range
                    || prior.content_range != snapshot.content_range
            });
            if changed {
                affected_islands.insert(snapshot.owner);
            }
        }
        if let Some(prior) = prior {
            for snapshot in prior.values() {
                if !current
                    .values()
                    .any(|candidate| candidate.owner == snapshot.owner)
                {
                    affected_islands.insert(snapshot.owner);
                }
            }
        }
    }

    for diagnostic in diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == crate::diagnostic::Severity::Error)
    {
        let (Some(document), Some(revision), Some(range)) = (
            diagnostic.document,
            diagnostic.document_revision,
            diagnostic.range,
        ) else {
            continue;
        };
        let Some(document_islands) = islands.get(&document) else {
            continue;
        };
        for snapshot in document_islands.values().filter(|snapshot| {
            affected_islands.contains(&snapshot.owner)
                && ranges_overlap(snapshot.source_range, range)
        }) {
            failed_islands.insert(snapshot.owner, (document, revision));
        }
    }

    let affected_concepts = concept_ids(context, affected_documents.iter().copied());
    let failed_concepts = concept_ids(
        context,
        failed_islands.values().map(|(document, _)| *document),
    );
    let mut affected_diagrams = BTreeSet::new();
    for diagram in projection.diagrams.iter().chain(
        previous
            .into_iter()
            .flat_map(|analysis| analysis.projection.diagrams.iter()),
    ) {
        if diagram_depends_on(diagram, &affected_concepts) {
            affected_diagrams.insert(diagram.key.clone());
        }
    }
    let mut stale_diagrams = BTreeSet::new();
    for diagram in projection.diagrams.iter().chain(
        previous
            .into_iter()
            .flat_map(|analysis| analysis.projection.diagrams.iter()),
    ) {
        if diagram_depends_on(diagram, &failed_concepts) {
            stale_diagrams.insert(diagram.key.clone());
        }
    }

    let mut diagram_projections = BTreeMap::new();
    for diagram in &projection.diagrams {
        let reuse = previous
            .and_then(|analysis| analysis.diagram(&diagram.key))
            .filter(|_| {
                !affected_diagrams.contains(&diagram.key) || stale_diagrams.contains(&diagram.key)
            });
        let projection = reuse.cloned().unwrap_or_else(|| Arc::new(diagram.clone()));
        diagram_projections.insert(Arc::<str>::from(diagram.key.as_str()), projection);
    }

    let projection_freshness = islands
        .values()
        .flat_map(|document| document.values())
        .map(|snapshot| {
            let freshness = failed_islands
                .get(&snapshot.owner)
                .map(|(_, failed_revision)| ProjectionFreshness::RetainedStale {
                    failed_revision: *failed_revision,
                })
                .unwrap_or(ProjectionFreshness::Current);
            (snapshot.owner, freshness)
        })
        .collect();

    let revisioned_diagnostics = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            Some(RevisionedDiagnostic {
                document: diagnostic.document?,
                revision: diagnostic.document_revision?,
                range: diagnostic.range?,
                source: DiagnosticSource::Semantic,
                severity: diagnostic.severity,
                code: Arc::from(diagnostic.code.as_str()),
                message: Arc::from(diagnostic.message.as_str()),
            })
        })
        .collect::<Vec<_>>()
        .into();

    AnalysisMetadata {
        affected: AffectedAnalysis {
            documents: affected_documents.into_iter().collect::<Vec<_>>().into(),
            islands: affected_islands.into_iter().collect::<Vec<_>>().into(),
            diagrams: affected_diagrams
                .into_iter()
                .map(Arc::<str>::from)
                .collect::<Vec<_>>()
                .into(),
        },
        projection_freshness,
        diagram_projections,
        revisioned_diagnostics,
    }
}

fn concept_ids(
    context: &DomainAnalysisContext<'_>,
    documents: impl IntoIterator<Item = DocumentId>,
) -> BTreeSet<String> {
    documents
        .into_iter()
        .filter_map(|document| context.catalog.document(document))
        .map(|document| crate::okf::id_of(document.path().as_str()))
        .collect()
}

fn diagram_depends_on(diagram: &crate::model::Diagram, concepts: &BTreeSet<String>) -> bool {
    concepts.contains(&diagram.key)
        || diagram
            .groups
            .iter()
            .any(|group| group_depends_on(group, concepts))
}

fn group_depends_on(group: &crate::model::DiagramGroup, concepts: &BTreeSet<String>) -> bool {
    group.members.iter().any(|member| concepts.contains(member))
        || group
            .children
            .iter()
            .any(|child| group_depends_on(child, concepts))
}

fn ranges_overlap(left: TextRange, right: TextRange) -> bool {
    left.start() <= right.end() && right.start() <= left.end()
}

fn validate_shared_context(context: &DomainAnalysisContext<'_>) -> Result<(), AnalysisError> {
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

fn declared_diagnostic(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    syntax: &SyntaxNode<UmlLanguage>,
    code: crate::diagnostic::DiagCode,
    message: String,
    warning: bool,
) -> Result<Diagnostic, AnalysisError> {
    declared_diagnostic_range(context, path, syntax.range(), code, message, warning)
}

fn declared_diagnostic_range(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    range: TextRange,
    code: crate::diagnostic::DiagCode,
    message: String,
    warning: bool,
) -> Result<Diagnostic, AnalysisError> {
    // Seam invariants (declared syntax belongs to a cataloged document): a
    // break becomes an AnalysisError instead of panicking the caller's process.
    let seam = |reason: &str| AnalysisError::CatalogInvariant {
        reason: format!("{reason}: {path}").into(),
    };
    let bundle_path = crate::source::BundlePath::parse(path)
        .map_err(|_| seam("analyzed path is not a valid bundle path"))?;
    let id = context
        .catalog
        .id_for_path(&bundle_path)
        .ok_or_else(|| seam("analyzed path is not cataloged"))?;
    let document = context
        .catalog
        .document(id)
        .ok_or_else(|| seam("cataloged path has no document"))?;
    let line = document
        .line_index()
        .line_col(document.text(), range.start())
        .map_err(|_| seam("declared syntax range is not a document offset"))?;
    let diagnostic = if warning {
        Diagnostic::warn(code, message, path, line.line as usize + 1)
    } else {
        Diagnostic::new(code, message, path, line.line as usize + 1)
    };
    Ok(diagnostic
        .with_span((
            line.byte_column as usize,
            line.byte_column as usize + range.len().to_usize(),
        ))
        .with_provenance(id, document.revision(), range))
}

fn trimmed_token_range(token: &SyntaxToken<UmlLanguage>) -> TextRange {
    let authored = token.text().write_to_string();
    let trimmed = authored.trim();
    // Lossless tokens may own leading trivia in their red range while
    // `text()` exposes only the significant spelling.
    let leading = token
        .range()
        .len()
        .to_usize()
        .saturating_sub(authored.len())
        + authored.len()
        - authored.trim_start().len();
    let start = (token.range().start()
        + TextSize::try_from(leading).expect("token leading trivia fits TextSize"))
    .expect("trimmed token start fits TextSize");
    let end = (start + TextSize::try_from(trimmed.len()).expect("token spelling fits TextSize"))
        .expect("trimmed token end fits TextSize");
    TextRange::new(start, end).expect("trimmed token range is ordered")
}

fn validate_declared_semantics(
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
                if let Some(token) = relationship.syntax.target_token() {
                    diagnostics.push(declared_diagnostic_range(
                        context,
                        path,
                        token.range(),
                        crate::diagnostic::DiagCode::MalformedRelationship,
                        "'associates' between classifiers requires ': <near> to <far>' multiplicity ends (ends are optional only on an actor↔use-case communication link)".into(),
                        false,
                    )?);
                }
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
                    relationship
                        .syntax
                        .target_token()
                        .map(|token| token.range())
                        .unwrap_or_else(|| relationship.syntax.syntax().range()),
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

fn declared_projection(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<super::Projection, AnalysisError> {
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
            crate::uml::sequence::lower(
                context,
                declared,
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

fn resolve_slug(path: &str, slug: &str, claimed: &BTreeSet<&str>) -> Option<String> {
    let target = crate::okf::resolve_href(path, slug);
    claimed.contains(target.as_str()).then_some(target)
}

fn resolve_describes(
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
) -> Option<String> {
    okf.extra
        .get_str("describes")
        .and_then(parse_link_ref)
        .and_then(|link| resolve_slug(path, &link.slug, claimed))
}

fn parse_link_ref(value: &str) -> Option<crate::layout::LinkRef> {
    let value = value.trim();
    let title_end = value.find("](")?;
    let title = value.strip_prefix('[')?.get(..title_end - 1)?;
    let href = value.get(title_end + 2..)?.strip_suffix(')')?;
    Some(crate::layout::LinkRef {
        title: title.to_owned(),
        slug: href.to_owned(),
    })
}

fn normalize_slot_value(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_owned()
}

fn parse_link_in_text(value: &str) -> Option<crate::layout::LinkRef> {
    let start = value.find('[')?;
    let relative_end = value[start..].find(')')?;
    parse_link_ref(&value[start..=start + relative_end])
}

pub(crate) fn behavior_diagnostic(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    syntax: &SyntaxNode<UmlLanguage>,
    code: crate::diagnostic::DiagCode,
    message: String,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Seam invariant (behavior syntax belongs to a cataloged document). This
    // helper fans out through the sequence/flow lowering paths, which have no
    // error channel; on a seam break, drop this one diagnostic instead of
    // panicking the in-process editor / poisoning the wasm instance. Debug
    // builds still assert so tests catch the broken seam.
    let Some(id) = crate::source::BundlePath::parse(path.to_string())
        .ok()
        .and_then(|bundle_path| context.catalog.id_for_path(&bundle_path))
    else {
        debug_assert!(false, "behavior diagnostic path is not cataloged: {path}");
        return;
    };
    let Some(document) = context.catalog.document(id) else {
        debug_assert!(false, "cataloged path has no document: {path}");
        return;
    };
    let range = items(syntax.clone(), syntax::UmlSyntaxKind::Link)
        .into_iter()
        .find_map(|link| {
            link.children()
                .find(|element| element.kind() == syntax::UmlSyntaxKind::LinkTargetToken)
                .map(|element| match element {
                    SyntaxElement::Node(node) => node.range(),
                    SyntaxElement::Token(token) => token.range(),
                })
        })
        .or_else(|| {
            syntax
                .children()
                .find(|element| element.kind() == syntax::UmlSyntaxKind::TargetToken)
                .map(|element| match element {
                    SyntaxElement::Node(node) => node.range(),
                    SyntaxElement::Token(token) => token.range(),
                })
        })
        .unwrap_or_else(|| syntax.range());
    let (Some(start), Some(end)) = (
        document
            .line_index()
            .line_col(document.text(), range.start())
            .ok(),
        document
            .line_index()
            .line_col(document.text(), range.end())
            .ok(),
    ) else {
        debug_assert!(false, "behavior range is not a document offset: {path}");
        return;
    };
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
                crate::layout::FlowTargetRef::Local(name) => {
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
                crate::layout::FlowTargetRef::Link(link) => {
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

struct ValidInlineInstance<'a> {
    name: &'a str,
    target: String,
}

enum InlineInstanceValidity<'a> {
    Invalid,
    Unresolved,
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
        InlineInstanceValidity::Unresolved
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
                InlineInstanceValidity::Invalid | InlineInstanceValidity::Unresolved => None,
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
    kind: syntax::UmlSyntaxKind,
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
    section: syntax::UmlSyntaxKind,
    kind: syntax::UmlSyntaxKind,
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
            token.kind() == syntax::UmlSyntaxKind::BadToken
                && token.flags().is_bad()
                && !token.flags().is_missing()
        }
        SyntaxElement::Node(child) => {
            (child.kind() == syntax::UmlSyntaxKind::SkippedTokensSyntax
                && child.range().start() != child.range().end())
                || has_recovery(&child)
        }
    })
}
fn first_recovery_node(node: &SyntaxNode<UmlLanguage>) -> Option<SyntaxNode<UmlLanguage>> {
    node.children().find_map(|element| {
        let child = element.into_node()?;
        if child.kind() == syntax::UmlSyntaxKind::SkippedTokensSyntax {
            Some(child)
        } else {
            first_recovery_node(&child)
        }
    })
}
fn has_missing_kind(node: &SyntaxNode<UmlLanguage>, kind: syntax::UmlSyntaxKind) -> bool {
    node.children().any(|e| {
        e.kind() == kind
            && e.into_token()
                .is_some_and(|token| token.flags().is_missing())
    })
}

fn direct_child(
    node: &SyntaxNode<UmlLanguage>,
    kind: syntax::UmlSyntaxKind,
) -> Option<SyntaxNode<UmlLanguage>> {
    node.children()
        .find(|element| element.kind() == kind)
        .and_then(SyntaxElement::into_node)
}

fn token_in(
    node: &SyntaxNode<UmlLanguage>,
    kind: syntax::UmlSyntaxKind,
) -> Option<SyntaxToken<UmlLanguage>> {
    node.children().find_map(|element| {
        if element.kind() == kind {
            element.into_token()
        } else {
            element.into_node().and_then(|child| token_in(&child, kind))
        }
    })
}

fn field_from_token(node: &SyntaxNode<UmlLanguage>, kind: syntax::UmlSyntaxKind) -> Option<String> {
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
    token_kind: syntax::UmlSyntaxKind,
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
    token_kind: syntax::UmlSyntaxKind,
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
    field_from_token(&slot, syntax::UmlSyntaxKind::ExpressionToken)
        .map(strip_expression)
        .or_else(|| field_from_token(&slot, syntax::UmlSyntaxKind::IdentifierToken))
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
    slot_kind: syntax::UmlSyntaxKind,
    token_kind: syntax::UmlSyntaxKind,
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
    let title = field_from_token(node, syntax::UmlSyntaxKind::LinkTextToken)?;
    let href = field_from_token(node, syntax::UmlSyntaxKind::LinkTargetToken)?;
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
        .filter(|token| token.kind() == syntax::UmlSyntaxKind::BulletToken)
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
            child.kind() == syntax::UmlSyntaxKind::BehaviorRecovery && has_recovery(&child)
        })
}

fn declared_flow_node(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredFlowNode {
    let syntax = syntax::FlowNodeSyntax(node.clone());
    let kind_slot = direct_child(&node, syntax::UmlSyntaxKind::FlowNodeKindSlot);
    let kind = match kind_slot {
        Some(slot) => match field_from_token(&slot, syntax::UmlSyntaxKind::NodeKindToken) {
            Some(token) => crate::model::FlowNodeKind::from_keyword(&token)
                .map(|value| valid(slot.clone(), value))
                .unwrap_or_else(|| invalid(slot)),
            None => valid(slot, crate::model::FlowNodeKind::Plain),
        },
        None => valid(node.clone(), crate::model::FlowNodeKind::Plain),
    };
    let identity_slot = direct_child(&node, syntax::UmlSyntaxKind::FlowIdentity)
        .expect("flow node has fixed identity occurrence");
    let identity = if has_direct_recovery(&node) {
        invalid_recovery(node.clone())
    } else if let Some(link) = direct_child(&identity_slot, syntax::UmlSyntaxKind::Link) {
        match link_parts(&link) {
            Some((title, _)) => valid(identity_slot.clone(), title),
            None => invalid(identity_slot.clone()),
        }
    } else {
        field_from_token(&identity_slot, syntax::UmlSyntaxKind::IdentityToken)
            .filter(|value| !value.is_empty())
            .map(|value| valid(identity_slot.clone(), value))
            .unwrap_or_else(|| match field_value(&kind) {
                Some(crate::model::FlowNodeKind::Initial) => {
                    valid(identity_slot.clone(), "initial".to_owned())
                }
                Some(crate::model::FlowNodeKind::Final) => {
                    valid(identity_slot.clone(), "final".to_owned())
                }
                _ => crate::uml::DeclaredField::Incomplete {
                    syntax: identity_slot.clone(),
                    expected: crate::uml::ExpectedSyntax::FlowTarget,
                },
            })
    };
    let object_ref = if matches!(field_value(&kind), Some(crate::model::FlowNodeKind::Object)) {
        direct_child(&identity_slot, syntax::UmlSyntaxKind::Link)
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
        .filter(|child| child.kind() == syntax::UmlSyntaxKind::FlowInternal)
    {
        let keyword = field_from_token(&block, syntax::UmlSyntaxKind::InternalKeywordToken);
        let value_slot = direct_child(&block, syntax::UmlSyntaxKind::FlowInternalValue);
        match keyword.as_deref() {
            Some("entry") => entry = declared_text_slot(value_slot),
            Some("do") => do_ = declared_text_slot(value_slot),
            Some("exit") => exit = declared_text_slot(value_slot),
            Some("partition") => partition = declared_text_slot(value_slot),
            Some("refines") => {
                refines = direct_child(&block, syntax::UmlSyntaxKind::Link)
                    .and_then(|link| link_parts(&link).map(|(_, slug)| valid(link, slug)))
                    .unwrap_or_else(|| invalid(block.clone()))
            }
            _ => {}
        }
    }
    let transitions = node
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|child| child.kind() == syntax::UmlSyntaxKind::FlowTransition)
        .map(declared_flow_transition)
        .collect::<Vec<_>>();
    let notes = node
        .children()
        .filter_map(SyntaxElement::into_node)
        .filter(|child| child.kind() == syntax::UmlSyntaxKind::Value)
        .filter_map(|value| {
            direct_child(&value, syntax::UmlSyntaxKind::FlowNoteValue)
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
    let syntax = syntax::FlowTransitionSyntax(node.clone());
    let text_field = |kind, token_kind| {
        direct_child(&node, kind)
            .map(|slot| declared_optional_expression_slot(slot, token_kind))
            .unwrap_or(crate::uml::DeclaredField::Absent)
    };
    let target_slot = direct_child(&node, syntax::UmlSyntaxKind::FlowTarget)
        .expect("transition has fixed target occurrence");
    let target = if has_recovery(&node) {
        invalid_recovery(node.clone())
    } else if has_missing_kind(&node, syntax::UmlSyntaxKind::ToToken)
        || has_missing_kind(&node, syntax::UmlSyntaxKind::FlowKeywordToken)
    {
        invalid(target_slot.clone())
    } else if let Some(link) = direct_child(&target_slot, syntax::UmlSyntaxKind::Link) {
        link_parts(&link)
            .map(|(title, slug)| {
                valid(
                    target_slot.clone(),
                    crate::layout::FlowTargetRef::Link(crate::layout::LinkRef { title, slug }),
                )
            })
            .unwrap_or_else(|| invalid(target_slot.clone()))
    } else {
        field_from_token(&target_slot, syntax::UmlSyntaxKind::TargetToken)
            .filter(|value| !value.is_empty())
            .map(|value| {
                valid(
                    target_slot.clone(),
                    crate::layout::FlowTargetRef::Local(value),
                )
            })
            .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                syntax: target_slot.clone(),
                expected: crate::uml::ExpectedSyntax::FlowTarget,
            })
    };
    let carries = direct_child(&node, syntax::UmlSyntaxKind::FlowCarries)
        .and_then(|slot| {
            let link = direct_child(&slot, syntax::UmlSyntaxKind::Link)?;
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
            syntax::UmlSyntaxKind::FlowTrigger,
            syntax::UmlSyntaxKind::TriggerToken,
        ),
        guard: if is_else {
            crate::uml::DeclaredField::Absent
        } else {
            text_field(
                syntax::UmlSyntaxKind::FlowGuard,
                syntax::UmlSyntaxKind::GuardToken,
            )
        },
        is_else,
        target,
        carries,
        effect: text_field(
            syntax::UmlSyntaxKind::FlowEffect,
            syntax::UmlSyntaxKind::EffectToken,
        ),
    }
}

fn declared_lifeline(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredLifeline {
    let syntax = syntax::LifelineSyntax(node.clone());
    let link = direct_child(&node, syntax::UmlSyntaxKind::Link)
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
    let alias = direct_child(&node, syntax::UmlSyntaxKind::LifelineAlias)
        .map(|slot| {
            field_from_token(&slot, syntax::UmlSyntaxKind::AliasToken)
                .filter(|value| !value.is_empty())
                .map(|value| valid(slot.clone(), value))
                .unwrap_or_else(|| {
                    let as_present = node
                        .child_at(syntax::LifelineSyntax::AS_SLOT)
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
    let syntax = syntax::MessageSyntax(node.clone());
    let source = declared_endpoint_field(declared_required_token(
        &node,
        syntax::UmlSyntaxKind::MessageSource,
        syntax::UmlSyntaxKind::SourceToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    ));
    let verb_slot = direct_child(&node, syntax::UmlSyntaxKind::MessageVerb)
        .expect("message has fixed verb occurrence");
    let kind = match field_from_token(&verb_slot, syntax::UmlSyntaxKind::VerbToken).as_deref() {
        Some("calls") if syntax.async_token().is_some() => valid(
            verb_slot.clone(),
            crate::uml::DeclaredMessageKind::AsyncCall,
        ),
        Some("calls") => valid(verb_slot.clone(), crate::uml::DeclaredMessageKind::SyncCall),
        Some("signals") => valid(
            verb_slot.clone(),
            crate::uml::DeclaredMessageKind::AsyncSignal,
        ),
        Some("returns") => valid(verb_slot.clone(), crate::uml::DeclaredMessageKind::Reply),
        Some("creates") => valid(verb_slot.clone(), crate::uml::DeclaredMessageKind::Create),
        Some("destroys") => valid(verb_slot.clone(), crate::uml::DeclaredMessageKind::Delete),
        Some(_) => invalid(verb_slot.clone()),
        None if has_recovery(&verb_slot) => invalid_recovery(verb_slot.clone()),
        None => crate::uml::DeclaredField::Incomplete {
            syntax: verb_slot.clone(),
            expected: crate::uml::ExpectedSyntax::MessageTarget,
        },
    };
    let target = if syntax.target_token().is_some() {
        declared_endpoint_field(declared_required_token(
            &node,
            syntax::UmlSyntaxKind::MessageTarget,
            syntax::UmlSyntaxKind::TargetToken,
            crate::uml::ExpectedSyntax::MessageTarget,
        ))
    } else if has_recovery(&node)
        && !matches!(
            &kind,
            crate::uml::DeclaredField::Valid {
                value: crate::uml::DeclaredMessageKind::Reply,
                ..
            }
        )
    {
        invalid_recovery(node.clone())
    } else {
        crate::uml::DeclaredField::Absent
    };
    let value_slot = direct_child(&node, syntax::UmlSyntaxKind::MessageValue)
        .expect("message has fixed value occurrence");
    let value = if field_from_token(&value_slot, syntax::UmlSyntaxKind::ValueToken).is_some() {
        declared_expression_slot(value_slot, syntax::UmlSyntaxKind::ValueToken)
    } else if node
        .child_at(syntax::MessageSyntax::COLON_SLOT)
        .and_then(SyntaxElement::into_token)
        .is_some_and(|token| !token.flags().is_missing())
    {
        if has_recovery(&node) {
            invalid_recovery(node.clone())
        } else {
            crate::uml::DeclaredField::Incomplete {
                syntax: value_slot,
                expected: crate::uml::ExpectedSyntax::MessageTarget,
            }
        }
    } else {
        crate::uml::DeclaredField::Absent
    };
    let call_id = declared_optional_message_token(
        &node,
        syntax.as_token().is_some(),
        syntax::UmlSyntaxKind::MessageCallId,
        syntax::UmlSyntaxKind::CallIdToken,
    );
    let return_to = declared_endpoint_field(declared_optional_message_token(
        &node,
        syntax.to_token().is_some(),
        syntax::UmlSyntaxKind::MessageReturnTarget,
        syntax::UmlSyntaxKind::ReturnTargetToken,
    ));
    let return_for = declared_optional_message_token(
        &node,
        syntax.for_token().is_some(),
        syntax::UmlSyntaxKind::MessageReturnCall,
        syntax::UmlSyntaxKind::ReturnCallToken,
    );
    crate::uml::DeclaredMessage {
        syntax,
        source,
        kind,
        target,
        value,
        call_id,
        return_to,
        return_for,
        depth: behavior_depth(&node),
    }
}

fn declared_optional_message_token(
    node: &SyntaxNode<UmlLanguage>,
    keyword_present: bool,
    slot_kind: syntax::UmlSyntaxKind,
    token_kind: syntax::UmlSyntaxKind,
) -> crate::uml::DeclaredField<UmlLanguage, String> {
    if !keyword_present {
        return crate::uml::DeclaredField::Absent;
    }
    declared_required_token(
        node,
        slot_kind,
        token_kind,
        crate::uml::ExpectedSyntax::MessageTarget,
    )
}

fn declared_endpoint(value: &str) -> Option<crate::uml::DeclaredEndpointRef> {
    if value == "outside" {
        return Some(crate::uml::DeclaredEndpointRef::Outside);
    }
    if let Some(gate) = value.strip_prefix('@') {
        return (!gate.is_empty()).then(|| crate::uml::DeclaredEndpointRef::LocalGate(gate.into()));
    }
    if let Some((use_alias, gate)) = value.split_once('@') {
        return (!use_alias.is_empty() && !gate.is_empty() && !gate.contains('@')).then(|| {
            crate::uml::DeclaredEndpointRef::UseGate {
                use_alias: use_alias.into(),
                gate: gate.into(),
            }
        });
    }
    (!value.is_empty() && !value.contains('@'))
        .then(|| crate::uml::DeclaredEndpointRef::Lifeline(value.into()))
}

fn declared_endpoint_field(
    field: crate::uml::DeclaredField<UmlLanguage, String>,
) -> crate::uml::DeclaredField<UmlLanguage, crate::uml::DeclaredEndpointRef> {
    match field {
        crate::uml::DeclaredField::Absent => crate::uml::DeclaredField::Absent,
        crate::uml::DeclaredField::Valid { value, syntax } => declared_endpoint(&value)
            .map(|value| valid(syntax.clone(), value))
            .unwrap_or_else(|| invalid(syntax)),
        crate::uml::DeclaredField::Incomplete { syntax, expected } => {
            crate::uml::DeclaredField::Incomplete { syntax, expected }
        }
        crate::uml::DeclaredField::Invalid {
            syntax,
            diagnostics,
        } => crate::uml::DeclaredField::Invalid {
            syntax,
            diagnostics,
        },
    }
}

fn declared_gate(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredGate {
    let syntax = syntax::GateSyntax(node.clone());
    let name = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::GateName,
        syntax::UmlSyntaxKind::IdentifierToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    );
    crate::uml::DeclaredGate { syntax, name }
}

fn declared_fragment(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredFragment {
    let syntax = syntax::SequenceFragmentSyntax(node.clone());
    let slot = direct_child(&node, syntax::UmlSyntaxKind::FragmentKind)
        .expect("fragment has fixed kind occurrence");
    let kind = field_from_token(&slot, syntax::UmlSyntaxKind::FragmentKindToken)
        .and_then(|value| crate::model::FragmentKind::parse(&value))
        .map(|value| valid(slot.clone(), value))
        .unwrap_or_else(|| invalid(slot));
    crate::uml::DeclaredFragment {
        syntax,
        kind,
        depth: behavior_depth(&node),
    }
}

fn declared_operand(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredOperand {
    let syntax = syntax::SequenceOperandSyntax(node.clone());
    let keyword = field_from_token(&node, syntax::UmlSyntaxKind::OperandKeywordToken);
    let spec = if has_recovery(&node) {
        invalid_recovery(node.clone())
    } else {
        match keyword.as_deref() {
            Some("when") => direct_child(&node, syntax::UmlSyntaxKind::OperandGuard)
                .map(|slot| declared_expression_slot(slot, syntax::UmlSyntaxKind::GuardToken))
                .unwrap_or_else(|| invalid(node.clone()))
                .map(crate::uml::DeclaredOperandSpec::Guard),
            Some("else") => valid(node.clone(), crate::uml::DeclaredOperandSpec::Else),
            Some("branch") => valid(
                node.clone(),
                crate::uml::DeclaredOperandSpec::Branch {
                    label: field_from_token(&node, syntax::UmlSyntaxKind::BranchLabelToken)
                        .map(strip_expression),
                },
            ),
            _ => invalid(node.clone()),
        }
    };
    crate::uml::DeclaredOperand {
        syntax,
        spec,
        depth: behavior_depth(&node),
    }
}

trait MapDeclaredField<L: waml_syntax::SyntaxLanguage, T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> crate::uml::DeclaredField<L, U>;
}

impl<L: waml_syntax::SyntaxLanguage, T> MapDeclaredField<L, T> for crate::uml::DeclaredField<L, T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> crate::uml::DeclaredField<L, U> {
        match self {
            crate::uml::DeclaredField::Absent => crate::uml::DeclaredField::Absent,
            crate::uml::DeclaredField::Valid { value, syntax } => {
                crate::uml::DeclaredField::Valid {
                    value: f(value),
                    syntax,
                }
            }
            crate::uml::DeclaredField::Incomplete { syntax, expected } => {
                crate::uml::DeclaredField::Incomplete { syntax, expected }
            }
            crate::uml::DeclaredField::Invalid {
                syntax,
                diagnostics,
            } => crate::uml::DeclaredField::Invalid {
                syntax,
                diagnostics,
            },
        }
    }
}

fn declared_binding(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredBinding {
    let syntax = syntax::BindingSyntax(node.clone());
    let local = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::BindingLocal,
        syntax::UmlSyntaxKind::LocalToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    );
    let target = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::BindingTarget,
        syntax::UmlSyntaxKind::TargetToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    );
    crate::uml::DeclaredBinding {
        syntax,
        local,
        target,
    }
}

fn declared_interaction_use(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredInteractionUse {
    let syntax = syntax::InteractionUseSyntax(node.clone());
    let link_node = direct_child(&node, syntax::UmlSyntaxKind::Link)
        .expect("interaction use has fixed link occurrence");
    let link = field_from_token(&link_node, syntax::UmlSyntaxKind::LinkTargetToken)
        .map(|value| valid(link_node.clone(), value))
        .unwrap_or_else(|| invalid(link_node));
    let alias = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::InteractionUseAlias,
        syntax::UmlSyntaxKind::AliasToken,
        crate::uml::ExpectedSyntax::MessageTarget,
    );
    let bindings = items(node.clone(), syntax::UmlSyntaxKind::Binding)
        .into_iter()
        .map(declared_binding)
        .collect::<Vec<_>>()
        .into();
    crate::uml::DeclaredInteractionUse {
        syntax,
        link,
        alias,
        bindings,
        depth: behavior_depth(&node),
    }
}

fn declared_value(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredValue {
    let syntax = syntax::ValueSyntax(node.clone());
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
    let syntax = syntax::SlotSyntax(node.clone());
    let field = |token: Option<SyntaxToken<UmlLanguage>>, expected| {
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
    if syntax
        .colon_token()
        .map_or(true, |t| t.flags().is_missing())
    {
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
            if e.kind() == syntax::UmlSyntaxKind::ColonToken {
                after_colon = true;
                return false;
            }
            after_colon && !matches!(e.kind(), syntax::UmlSyntaxKind::NewlineToken)
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
        if (syntax.value_kind() == syntax::SlotValueKind::Quoted
            && (raw.len() < 2 || !raw.ends_with('"')))
            || (syntax.value_kind() == syntax::SlotValueKind::Link
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
    let syntax = syntax::RelationshipSyntax(node.clone());
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
            .find(|e| e.kind() == syntax::UmlSyntaxKind::LinkTargetToken)
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
    let end = |end: Option<syntax::RelationshipEndSyntax>| {
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
    let mut from_end = end(syntax.source_end());
    let mut to_end = end(syntax.to_end());
    match (&kind, syntax.colon_token().is_some()) {
        (
            crate::uml::DeclaredField::Valid {
                value:
                    crate::model::RelationshipKind::Aggregates
                    | crate::model::RelationshipKind::Composes,
                ..
            },
            false,
        ) => {
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
    let syntax = syntax::LayoutStatementSyntax(node.clone());
    if has_recovery(&node) {
        let recovery = first_recovery_node(&node).expect("recovery predicate found a node");
        return crate::uml::DeclaredField::Incomplete {
            syntax: recovery,
            expected: crate::uml::ExpectedSyntax::LayoutOperand,
        };
    }
    if let Some(slots) = syntax.placement() {
        let operands = slots
            .operands()
            .map(|slot| slot.value().map(|value| valid(slot.0, value)))
            .collect::<Option<Vec<_>>>();
        let directions = slots
            .directions()
            .map(|slot| slot.value().map(|value| valid(slot.0, value)))
            .collect::<Option<Vec<_>>>();
        return match (operands, directions) {
            (Some(operands), Some(directions))
                if operands.len() == directions.len().saturating_add(1) =>
            {
                valid(
                    node,
                    crate::uml::DeclaredLayoutStatement::Placement {
                        operands: operands.into(),
                        directions: directions.into(),
                    },
                )
            }
            _ => invalid(node),
        };
    }
    if let Some(alignment) = syntax.alignment() {
        return match (alignment.left(), alignment.right()) {
            (Some(left), Some(right)) => match (left.value(), right.value()) {
                (Some(left_value), Some(right_value)) => valid(
                    node,
                    crate::uml::DeclaredLayoutStatement::Alignment {
                        left: valid(left.0, left_value),
                        right: valid(right.0, right_value),
                    },
                ),
                _ => invalid(node),
            },
            _ => invalid(node),
        };
    }
    if let Some(standalone) = syntax.standalone() {
        return match standalone.operand() {
            Some(operand) => match operand.value() {
                Some(value) => valid(
                    node,
                    crate::uml::DeclaredLayoutStatement::Standalone(valid(operand.0, value)),
                ),
                None => invalid(node),
            },
            None => invalid(node),
        };
    }
    crate::uml::DeclaredField::Incomplete {
        syntax: node,
        expected: crate::uml::ExpectedSyntax::LayoutOperand,
    }
}
fn declared_member(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredMember {
    let syntax = syntax::MemberSyntax(node.clone());
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
    let syntax = syntax::MemberGroupSyntax(node.clone());
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
            .all(|element| element.kind() != syntax::UmlSyntaxKind::HeadingMarkerToken) =>
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
            .filter(|n| n.kind() == syntax::UmlSyntaxKind::Member)
            .map(declared_member)
            .collect::<Vec<_>>()
            .into(),
        inline_instances: node
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter(|n| n.kind() == syntax::UmlSyntaxKind::InlineInstance)
            .map(declared_inline_instance)
            .collect::<Vec<_>>()
            .into(),
        children: node
            .children()
            .filter_map(SyntaxElement::into_node)
            .filter(|n| n.kind() == syntax::UmlSyntaxKind::MemberGroup)
            .map(declared_member_group)
            .collect::<Vec<_>>()
            .into(),
    }
}
fn declared_inline_instance(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredInlineInstance {
    let syntax = syntax::InlineInstanceSyntax(node.clone());
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
                .find(|e| e.kind() == syntax::UmlSyntaxKind::IdentifierToken)
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
                        syntax::UmlSyntaxKind::IdentifierToken
                            | syntax::UmlSyntaxKind::TypeToken
                            | syntax::UmlSyntaxKind::Link
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
                    .find(|x| x.kind() == syntax::UmlSyntaxKind::LinkTargetToken)
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
            } else if has_missing_kind(&slot_node, syntax::UmlSyntaxKind::SetToToken)
                && !matches!(value, crate::uml::DeclaredField::Invalid { .. })
            {
                value = crate::uml::DeclaredField::Incomplete {
                    syntax: slot_node.clone(),
                    expected: crate::uml::ExpectedSyntax::LinkTarget,
                };
            }
            crate::uml::DeclaredSlot {
                syntax: syntax::SlotSyntax(slot_node),
                name,
                value,
            }
        })
        .collect::<Vec<_>>()
        .into();
    let name = if has_recovery(&node) {
        invalid(node.clone())
    } else if has_missing_kind(&node, syntax::UmlSyntaxKind::AsToken) {
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

fn attributes(node: SyntaxNode<UmlLanguage>) -> Vec<syntax::AttributeSyntax> {
    let mut found = Vec::new();
    for child in node.children() {
        if let SyntaxElement::Node(child) = child {
            if let Some(attribute) = syntax::AttributeSyntax::cast(child.clone()) {
                found.push(attribute);
            } else {
                found.extend(attributes(child));
            }
        }
    }
    found
}
