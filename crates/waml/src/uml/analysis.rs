//! Ownership rule for UML validation: every rule has exactly ONE verdict
//! function, placed in the module that owns the concept it validates.
//! Projection (admission) consumes the verdict to admit or silently drop a
//! declaration from the model; the diagnostic pass consumes the SAME verdict
//! to report it to the user. Neither admission nor diagnostics re-derives a
//! rule of its own, and `sequence::lower` never invents one either — it calls
//! back into the owning module's verdict function. New UML rules must follow
//! this shape: one verdict function, two consumers.
//!
//! This file is the orchestrator: it owns the `Analysis` result, the UML
//! island syntax cache, and the `analyze` pipeline that drives the phases.
//! The phases themselves live in submodules and depend on each other in one
//! direction only:
//!
//! - `syntax_util` — syntax-tree navigation and `DeclaredField` construction
//! - `link` — the `[title](href)` link-ref parser and slug resolution
//! - `extract` — syntax nodes to the `declared::Declared*` family
//! - `diagnostics` — declared facts and parser errors to `Diagnostic`s
//! - `verdict` — the rules read by more than one consumer
//! - `validate` — declared semantics to `Diagnostic`s
//! - `behavior` — activity/state-machine flow and trace lowering
//! - `use_case_groups` — use-case group projection
//! - `project` — the declared bundle to a `Projection` of the model

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
    ChangeMap, SyntaxElement, SyntaxIdentity, SyntaxNode, SyntaxTree, TextRange, WamlSectionKind,
};

mod behavior;
mod diagnostics;
mod extract;
mod link;
mod project;
mod syntax_util;
mod use_case_groups;
mod validate;
mod verdict;

use diagnostics::{translate_layout_diagnostics, translate_parser_diagnostics};
use extract::{
    declared_attribute, declared_flow_node, declared_fragment, declared_gate,
    declared_inline_instance, declared_interaction_use, declared_layout, declared_lifeline,
    declared_member, declared_member_group, declared_message, declared_operand,
    declared_relationship, declared_slot, declared_value,
};
use project::declared_projection;
use syntax_util::{attributes, direct_section_items, items};
use validate::{validate_declared_semantics, validate_document_types, validate_shared_context};

pub(crate) use diagnostics::behavior_diagnostic;
pub(crate) use link::resolve_describes;

/// Field visibility rule: a field is `pub` when it is a direct analysis
/// output a frontend reads as-is (claims, syntax trees, the declared bundle,
/// the projected model, diagnostics, the Markdown syntax set). A field stays
/// private, behind a getter, when reading it correctly requires applying an
/// invariant the getter enforces — a default for an island with no recorded
/// freshness (`projection_freshness`), a key lookup into a map
/// (`diagram_projections`), or internal incremental-analysis bookkeeping
/// callers must not mutate directly (`affected`, `revisioned_diagnostics`,
/// `session_revision`). New fields follow whichever side of that line they
/// fall on.
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
    trace_records: Vec<TraceRecord>,
    trace_outgoing: BTreeMap<String, std::ops::Range<usize>>,
    trace_incoming: BTreeMap<(String, Option<String>), Vec<usize>>,
    session_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceRecord {
    pub behavior: String,
    pub flow_edge_key: String,
    pub trace_index: usize,
    pub label: String,
    pub href: String,
    pub target: crate::model::TraceTarget,
    pub source: crate::model::TraceSource,
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

    pub(crate) fn resolved_diagram_member_keys(&self, key: &str) -> Vec<String> {
        fn collect(group: &crate::model::DiagramGroup, members: &mut BTreeSet<String>) {
            members.extend(group.members.iter().cloned());
            for child in &group.children {
                collect(child, members);
            }
        }

        let mut members = BTreeSet::new();
        if let Some(diagram) = self.diagram(key) {
            for group in &diagram.groups {
                collect(group, &mut members);
            }
        }
        members.into_iter().collect()
    }

    pub fn revisioned_diagnostics(&self) -> &[RevisionedDiagnostic] {
        &self.revisioned_diagnostics
    }

    pub fn traces_from(&self, flow_edge_key: &str) -> &[TraceRecord] {
        self.trace_outgoing
            .get(flow_edge_key)
            .map_or(&[], |range| &self.trace_records[range.clone()])
    }

    pub fn traces_to(&self, concept_id: &str, fragment: Option<&str>) -> Vec<&TraceRecord> {
        let mut indices = if let Some(fragment) = fragment {
            self.trace_incoming
                .get(&(concept_id.to_owned(), Some(fragment.to_owned())))
                .cloned()
                .unwrap_or_default()
        } else {
            self.trace_incoming
                .iter()
                .filter(|((target, _), _)| target == concept_id)
                .flat_map(|(_, indices)| indices.iter().copied())
                .collect()
        };
        indices.sort_unstable();
        indices
            .into_iter()
            .map(|index| &self.trace_records[index])
            .collect()
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
                super::unique_match(self.syntax.catalog().documents().iter(), |(_, document)| {
                    document_slug(document.path().as_str()) == target
                })
                .map(|(id, document)| (*id, document))
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

/// Orchestrates one UML analysis pass. Phases, in order:
/// 1. Validate the shared context and index the catalog by concept id.
/// 2. Per claimed concept: recover each island's syntax tree
///    (`recover_island_tree`, reusing the previous analysis where the edit
///    left it unchanged), compose the full compatibility tree, and lower its
///    syntax into a `DeclaredConcept` via the `declared_*` family
///    (`declared_attribute` is the one case built inline, since it needs
///    both `context.okf` and the document rather than a bare syntax node).
/// 3. Translate parser diagnostics for the concept's tree
///    (`translate_parser_diagnostics`).
/// 4. Once every concept's declared bundle is assembled: validate declared
///    semantics, project the domain model (`declared_projection`), and
///    compute incremental-analysis metadata (`analysis_metadata`) — which
///    islands/diagrams are affected or stale relative to `previous`.
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
    validate_document_types(&context, &mut diagnostics)?;
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
            let (source_range, island_tree) = recover_island_tree(
                &document,
                structure,
                island,
                previous_islands,
                previous_document,
                change_map.as_ref(),
            )?;
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
        let fields: Vec<DeclaredAttribute> = attributes
            .into_iter()
            .map(|syntax| declared_attribute(&context, &document, syntax))
            .collect();
        let layout_fields = layout.into_iter().map(declared_layout).collect::<Vec<_>>();
        translate_layout_diagnostics(&document, id, &tree, &layout_fields, &mut diagnostics)?;
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
        translate_parser_diagnostics(&document, id, &tree, &mut diagnostics)?;
        let snapshot = previous
            .and_then(|analysis| analysis.syntax.document(id))
            .filter(|previous_snapshot| Arc::ptr_eq(previous_snapshot.document(), &document))
            .cloned()
            .unwrap_or_else(|| Arc::new(SyntaxSnapshot::new(document.clone(), tree)));
        snapshots.insert(id, snapshot);
    }
    validate_declared_semantics(&context, &declared, &concept_paths, &mut diagnostics)?;
    let projection = declared_projection(&context, &declared, &concept_paths, &mut diagnostics)?;
    let (trace_records, trace_outgoing, trace_incoming) = trace_indexes(&projection);
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
        trace_records,
        trace_outgoing,
        trace_incoming,
        session_revision: context.session_revision,
    })
}

type TraceIncoming = BTreeMap<(String, Option<String>), Vec<usize>>;

fn trace_indexes(
    projection: &super::Projection,
) -> (
    Vec<TraceRecord>,
    BTreeMap<String, std::ops::Range<usize>>,
    TraceIncoming,
) {
    let mut records = Vec::new();
    let mut outgoing = BTreeMap::new();
    let mut incoming = BTreeMap::new();
    for edge in &projection.flow_edges {
        let start = records.len();
        for (trace_index, trace) in edge.traces.iter().enumerate() {
            let record_index = records.len();
            match &trace.target {
                crate::model::TraceTarget::InternalDocument { concept_id } => {
                    incoming
                        .entry((concept_id.clone(), None))
                        .or_insert_with(Vec::new)
                        .push(record_index);
                }
                crate::model::TraceTarget::InternalFragment {
                    concept_id,
                    fragment,
                } => {
                    incoming
                        .entry((concept_id.clone(), Some(fragment.clone())))
                        .or_insert_with(Vec::new)
                        .push(record_index);
                }
                crate::model::TraceTarget::Https { .. }
                | crate::model::TraceTarget::Unresolved { .. }
                | crate::model::TraceTarget::Invalid { .. } => {}
            }
            records.push(TraceRecord {
                behavior: edge.behavior.clone(),
                flow_edge_key: edge.key.clone(),
                trace_index,
                label: trace.label.clone(),
                href: trace.href.clone(),
                target: trace.target.clone(),
                source: trace.source.clone(),
            });
        }
        outgoing.insert(edge.key.clone(), start..records.len());
    }
    (records, outgoing, incoming)
}

/// Recovers the authoritative syntax tree for one UML island, reusing the
/// previous analysis's tree when the island's source and content ranges
/// translate unchanged across the edit, and otherwise falling back to a
/// fresh authoritative parse of the island.
///
/// Seam invariant (structure map identifies its islands): a break in the
/// fallback parse must surface as an error, not panic the editor in-process
/// or poison the wasm instance.
fn recover_island_tree(
    document: &Arc<crate::analysis::DocumentVersion>,
    structure: &waml_syntax::MarkdownStructureMap,
    island: &waml_syntax::WamlLanguageIsland,
    previous_islands: Option<&Arc<UmlIslandDocument>>,
    previous_document: Option<&Arc<SyntaxSnapshot<UmlLanguage>>>,
    change_map: Option<&ChangeMap>,
) -> Result<(TextRange, Arc<SyntaxTree<UmlLanguage>>), AnalysisError> {
    let source_range = TextRange::new(island.heading_range.start(), island.content_range.end())
        .map_err(|_| AnalysisError::CatalogInvariant {
            reason: "UML Markdown island has a reversed source range".into(),
        })?;
    let reusable = previous_islands
        .and_then(|snapshots| {
            snapshots
                .values()
                .find(|snapshot| snapshot.owner == island.owner)
        })
        .filter(|snapshot| snapshot.kind == island.kind)
        .filter(|snapshot| {
            if previous_document.is_some_and(|previous| Arc::ptr_eq(previous.document(), document))
            {
                return snapshot.source_range == source_range
                    && snapshot.content_range == island.content_range;
            }
            change_map.is_some_and(|map| {
                map.translate_unchanged(snapshot.source_range) == Some(source_range)
                    && map.translate_unchanged(snapshot.content_range) == Some(island.content_range)
            })
        });
    let island_tree = match reusable.map(|snapshot| snapshot.syntax.clone()) {
        Some(tree) => tree,
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
    Ok((source_range, island_tree))
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
            .flat_map(|analysis| analysis.diagram_projections.values().map(Arc::as_ref)),
    ) {
        if diagram_depends_on(diagram, &affected_concepts) {
            affected_diagrams.insert(diagram.key.clone());
        }
    }
    let mut stale_diagrams = BTreeSet::new();
    for diagram in projection.diagrams.iter().chain(
        previous
            .into_iter()
            .flat_map(|analysis| analysis.diagram_projections.values().map(Arc::as_ref)),
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
    if let Some(previous) = previous {
        for (key, diagram) in &previous.diagram_projections {
            if stale_diagrams.contains(&diagram.key)
                && !diagram_projections.contains_key(diagram.key.as_str())
            {
                diagram_projections.insert(key.clone(), diagram.clone());
            }
        }
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
