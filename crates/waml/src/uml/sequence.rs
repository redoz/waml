//! Ownership rule for UML validation: every rule has exactly ONE verdict
//! function, placed in the module that owns the concept it validates. This
//! module never invents a validation rule of its own — it calls the owning
//! module's verdict function (e.g. `analysis::relationship_end_verdict`) and
//! either admits/drops (projection) or reports (diagnostics) based on the
//! same verdict every other consumer sees. New UML rules must follow this
//! shape: one verdict function, two consumers.

use super::{
    DeclaredBundle, DeclaredConcept, DeclaredEndpointRef, DeclaredField, DeclaredMessageKind,
    DeclaredOperandSpec,
};
use crate::{
    analysis::DomainAnalysisContext,
    diagnostic::{DiagCode, Diagnostic},
    model::{
        EndpointRef, FragmentKind, InteractionUseId, MessageId, MessageKind, OperandSpec,
        SeqBinding, SeqChild, SeqEdge, SeqInteractionUse, SeqNode, SequenceDoc,
    },
};
use std::collections::{BTreeMap, BTreeSet};
use waml_syntax::AstNode;

fn value<T>(field: &DeclaredField<super::syntax::UmlLanguage, T>) -> Option<&T> {
    match field {
        DeclaredField::Valid { value, .. } => Some(value),
        _ => None,
    }
}

fn report_at(
    context: &DomainAnalysisContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagCode,
    message: impl Into<String>,
    path: &str,
    syntax: &waml_syntax::SyntaxNode<super::syntax::UmlLanguage>,
) {
    super::analysis::behavior_diagnostic(context, path, syntax, code, message.into(), diagnostics);
}

fn is_sequence_identifier(name: &str) -> bool {
    name != "outside" && !name.contains('@')
}

fn validate_sequence_identifier(
    context: &DomainAnalysisContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    name: &str,
    role: &str,
    path: &str,
    syntax: &waml_syntax::SyntaxNode<super::syntax::UmlLanguage>,
) -> bool {
    if is_sequence_identifier(name) {
        return true;
    }
    report_at(
        context,
        diagnostics,
        DiagCode::ReservedSequenceName,
        format!("reserved sequence {role} '{name}'"),
        path,
        syntax,
    );
    false
}

fn report_message(
    ctx: &FoldCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagCode,
    message: impl Into<String>,
    id: &MessageId,
) {
    let Some(declared) = ctx.concept.messages.get(id.0) else {
        debug_assert!(false, "message id {id} out of range of declared messages");
        return;
    };
    report_at(
        ctx.context,
        diagnostics,
        code,
        message,
        ctx.path,
        declared.syntax.syntax(),
    );
}

/// Read-only context threaded through the fragment-fold walkers
/// (`resolve_returns`/`walk_return_items`, `repeated_deletes`, `walk`): who to
/// blame a diagnostic on (`context`, `concept`) and which document it came
/// from (`path`). Bundled so the walkers stay under clippy's argument-count
/// threshold instead of each carrying three loose parameters.
struct FoldCtx<'a> {
    context: &'a DomainAnalysisContext<'a>,
    concept: &'a DeclaredConcept,
    path: &'a str,
}

/// Read-only id -> node/edge lookup shared by the fragment-fold walkers.
struct SeqLookup<'a> {
    nodes: &'a BTreeMap<String, &'a SeqNode>,
    edges: &'a BTreeMap<MessageId, &'a SeqEdge>,
}

/// Whether a fragment's `incoming` state (the state as of the fragment's
/// start) must also be admitted as one of the merge outcomes, alongside each
/// operand branch. `opt`/`loop`/`break` may execute zero times; an `alt`
/// without an explicit `else` operand may take none of its guarded branches.
fn fragment_readmits_incoming(kind: FragmentKind, has_else: bool) -> bool {
    matches!(
        kind,
        FragmentKind::Opt | FragmentKind::Loop | FragmentKind::Break
    ) || (kind == FragmentKind::Alt && !has_else)
}

/// Join a fragment's per-operand outcomes with union: an item is in the
/// merged state if it is in *any* outcome. Used for state that only grows
/// monotonically as messages are seen (e.g. "still-open calls").
fn merge_union<T: Ord + Clone>(outcomes: Vec<BTreeSet<T>>) -> BTreeSet<T> {
    let mut merged = BTreeSet::new();
    for outcome in outcomes {
        merged.extend(outcome);
    }
    merged
}

/// Join a fragment's per-operand outcomes with intersection: an item is in
/// the merged state only if it is in *every* outcome. Used for state that
/// must hold on every path (e.g. "lifeline is alive").
fn merge_intersect<T: Ord + Clone>(outcomes: Vec<BTreeSet<T>>) -> BTreeSet<T> {
    let mut iter = outcomes.into_iter();
    let Some(first) = iter.next() else {
        return BTreeSet::new();
    };
    iter.fold(first, |mut joined, branch| {
        joined.retain(|id| branch.contains(id));
        joined
    })
}

fn message_kind(kind: DeclaredMessageKind) -> MessageKind {
    match kind {
        DeclaredMessageKind::SyncCall => MessageKind::SyncCall,
        DeclaredMessageKind::AsyncCall => MessageKind::AsyncCall,
        DeclaredMessageKind::AsyncSignal => MessageKind::AsyncSignal,
        DeclaredMessageKind::Reply => MessageKind::Reply,
        DeclaredMessageKind::Create => MessageKind::Create,
        DeclaredMessageKind::Delete => MessageKind::Delete,
    }
}

struct Endpoints<'a> {
    context: &'a DomainAnalysisContext<'a>,
    lifelines: &'a BTreeSet<String>,
    gates: &'a BTreeSet<String>,
    uses: &'a BTreeMap<String, usize>,
    interaction_uses: &'a mut [SeqInteractionUse],
    target_gates: &'a BTreeMap<String, BTreeSet<String>>,
    target_connected_gates: &'a BTreeMap<String, BTreeSet<String>>,
    path: &'a str,
    diagnostics: &'a mut Vec<Diagnostic>,
}

impl Endpoints<'_> {
    fn resolve(
        &mut self,
        endpoint: &DeclaredEndpointRef,
        syntax: &waml_syntax::SyntaxNode<super::syntax::UmlLanguage>,
    ) -> Option<EndpointRef> {
        match endpoint {
            DeclaredEndpointRef::Lifeline(id) => {
                if !self.lifelines.contains(id) {
                    super::analysis::behavior_diagnostic(
                        self.context,
                        self.path,
                        syntax,
                        DiagCode::UnknownSequenceEndpoint,
                        format!("unknown sequence lifeline '{id}'"),
                        self.diagnostics,
                    );
                    return None;
                }
                Some(EndpointRef::Lifeline { id: id.clone() })
            }
            DeclaredEndpointRef::Outside => Some(EndpointRef::Outside),
            DeclaredEndpointRef::LocalGate(gate) => {
                if !self.gates.contains(gate) {
                    super::analysis::behavior_diagnostic(
                        self.context,
                        self.path,
                        syntax,
                        DiagCode::UnknownSequenceEndpoint,
                        format!("unknown local gate '{gate}'"),
                        self.diagnostics,
                    );
                    return None;
                }
                Some(EndpointRef::LocalGate { gate: gate.clone() })
            }
            DeclaredEndpointRef::UseGate { use_alias, gate } => {
                let Some(&index) = self.uses.get(use_alias) else {
                    super::analysis::behavior_diagnostic(
                        self.context,
                        self.path,
                        syntax,
                        DiagCode::UnknownSequenceEndpoint,
                        format!("unknown interaction-use alias '{use_alias}'"),
                        self.diagnostics,
                    );
                    return None;
                };
                let target = &self.interaction_uses[index].target;
                if !self
                    .target_gates
                    .get(target)
                    .is_some_and(|gates| gates.contains(gate))
                {
                    super::analysis::behavior_diagnostic(
                        self.context,
                        self.path,
                        syntax,
                        DiagCode::InvalidInteractionUse,
                        format!("interaction use '{use_alias}' has no gate '{gate}'"),
                        self.diagnostics,
                    );
                    return None;
                } else if !self
                    .target_connected_gates
                    .get(target)
                    .is_some_and(|gates| gates.contains(gate))
                {
                    super::analysis::behavior_diagnostic(
                        self.context,
                        self.path,
                        syntax,
                        DiagCode::InvalidInteractionUse,
                        format!(
                            "interaction use '{use_alias}' gate '{gate}' has no inner connection"
                        ),
                        self.diagnostics,
                    );
                    return None;
                }
                Some(EndpointRef::UseGate {
                    interaction_use: self.interaction_uses[index].id.clone(),
                    gate: gate.clone(),
                })
            }
        }
    }
}

fn lifeline_classifier_map(concept: &DeclaredConcept, path: &str) -> BTreeMap<String, String> {
    concept
        .lifelines
        .iter()
        .filter_map(|lifeline| {
            let title = value(&lifeline.title)?;
            let handle = value(&lifeline.alias).unwrap_or(title);
            let classifier = crate::okf::resolve_href(path, value(&lifeline.target)?);
            Some((handle.clone(), classifier))
        })
        .collect()
}

/// The verdict for one declared lifeline. This is the ONE derivation of which
/// lifelines a sequence concept admits and what classifier each is bound to:
/// `lower` consumes it to report the rejections and build the nodes, and
/// `interaction_use_graph` consumes it silently. Neither re-derives the rule.
enum LifelineAdmission {
    Admitted {
        id: String,
        title: String,
        alias: Option<String>,
        /// The resolved classifier, when it is a claimed concept.
        classifier: Option<String>,
    },
    /// No link or no title: there is nothing to name, and nothing to report.
    Unnamed,
    /// The handle is a reserved sequence name.
    ReservedName(String),
    /// A lifeline with this handle was already admitted.
    Duplicate(String),
}

fn lifeline_admissions<'a>(
    concept: &'a DeclaredConcept,
    path: &str,
    claimed: &BTreeSet<&str>,
) -> Vec<(&'a super::DeclaredLifeline, LifelineAdmission)> {
    let mut admitted = BTreeSet::new();
    concept
        .lifelines
        .iter()
        .map(|lifeline| {
            let (Some(slug), Some(title)) = (value(&lifeline.target), value(&lifeline.title))
            else {
                return (lifeline, LifelineAdmission::Unnamed);
            };
            let alias = value(&lifeline.alias).cloned();
            let id = alias.clone().unwrap_or_else(|| title.clone());
            let admission = if !is_sequence_identifier(&id) {
                LifelineAdmission::ReservedName(id)
            } else if !admitted.insert(id.clone()) {
                LifelineAdmission::Duplicate(id)
            } else {
                let target = crate::okf::resolve_href(path, slug);
                LifelineAdmission::Admitted {
                    id,
                    title: title.clone(),
                    alias,
                    classifier: claimed.contains(target.as_str()).then_some(target),
                }
            };
            (lifeline, admission)
        })
        .collect()
}

/// The admitted lifeline handles and their claimed classifiers, for callers
/// that only need the outcome of [`lifeline_admissions`].
fn admitted_lifelines(
    concept: &DeclaredConcept,
    path: &str,
    claimed: &BTreeSet<&str>,
) -> (BTreeSet<String>, BTreeMap<String, String>) {
    let mut handles = BTreeSet::new();
    let mut classifiers = BTreeMap::new();
    for (_, admission) in lifeline_admissions(concept, path, claimed) {
        if let LifelineAdmission::Admitted { id, classifier, .. } = admission {
            if let Some(classifier) = classifier {
                classifiers.insert(id.clone(), classifier);
            }
            handles.insert(id);
        }
    }
    (handles, classifiers)
}

/// The bindings of an interaction use whose local and target endpoints both
/// resolved, paired with the syntax they came from. Shared so the silent and
/// the diagnosed copy check the same set.
fn resolved_binding_entries(
    declared_use: &super::DeclaredInteractionUse,
) -> Vec<(SeqBinding, &super::DeclaredBinding)> {
    declared_use
        .bindings
        .iter()
        .filter_map(|binding| {
            Some((
                SeqBinding {
                    local: value(&binding.local)?.clone(),
                    target: value(&binding.target)?.clone(),
                },
                binding,
            ))
        })
        .collect()
}

fn participating_lifelines(concept: &DeclaredConcept) -> BTreeSet<String> {
    let mut first: BTreeMap<String, bool> = BTreeMap::new();
    for message in concept.messages.iter() {
        let kind = value(&message.kind).copied();
        if let Some(DeclaredEndpointRef::Lifeline(handle)) = value(&message.source) {
            first.entry(handle.clone()).or_insert(false);
        }
        let target = if kind == Some(DeclaredMessageKind::Reply) {
            value(&message.return_to)
        } else {
            value(&message.target)
        };
        if let Some(DeclaredEndpointRef::Lifeline(handle)) = target {
            first
                .entry(handle.clone())
                .or_insert(kind == Some(DeclaredMessageKind::Create));
        }
    }
    first
        .into_iter()
        .filter_map(|(handle, introduced_by_create)| (!introduced_by_create).then_some(handle))
        .collect()
}

type InteractionUseGraph = BTreeMap<String, Vec<(usize, String)>>;

/// A single defect found while checking an interaction use's lifeline bindings.
/// This is the ONE verdict for the rule: the silent (graph) copy and the
/// diagnosed (`lower`) copy both consume it instead of re-deriving it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UseBindingIssue {
    /// The same local lifeline or the same target lifeline is bound twice.
    DuplicateBinding,
    /// The local lifeline or the bound target lifeline does not exist.
    UnknownEndpoint,
    /// The local and target lifelines are bound to different classifiers.
    ClassifierMismatch,
    /// A lifeline that participates in the target interaction has no binding.
    MissingParticipant,
}

/// Validate an interaction use's lifeline bindings against the target
/// interaction's lifelines and participating set. `bindings` must already be
/// filtered to entries with both a local and a target endpoint resolved; the
/// returned index is the position of the offending binding within that slice
/// (`None` for the whole-use `MissingParticipant` issue).
fn validate_use_bindings(
    bindings: &[SeqBinding],
    lifelines: &BTreeSet<String>,
    lifeline_classifiers: &BTreeMap<String, String>,
    target_lifelines: &BTreeMap<String, String>,
    participating: &BTreeSet<String>,
) -> Vec<(UseBindingIssue, Option<usize>)> {
    let mut issues = Vec::new();
    let mut locals = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for (index, binding) in bindings.iter().enumerate() {
        if !locals.insert(binding.local.clone()) || !targets.insert(binding.target.clone()) {
            issues.push((UseBindingIssue::DuplicateBinding, Some(index)));
        }
        if !lifelines.contains(&binding.local) || !target_lifelines.contains_key(&binding.target) {
            issues.push((UseBindingIssue::UnknownEndpoint, Some(index)));
        }
        if let (Some(local_classifier), Some(target_classifier)) = (
            lifeline_classifiers.get(&binding.local),
            target_lifelines.get(&binding.target),
        ) {
            if local_classifier != target_classifier {
                issues.push((UseBindingIssue::ClassifierMismatch, Some(index)));
            }
        }
    }
    if participating.iter().any(|handle| !targets.contains(handle)) {
        issues.push((UseBindingIssue::MissingParticipant, None));
    }
    issues
}

fn interaction_use_graph(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    claimed: &BTreeSet<&str>,
    concept_paths: &BTreeMap<String, String>,
) -> InteractionUseGraph {
    let mut graph = BTreeMap::new();
    for concept in declared.concepts() {
        let Some(path) = concept_paths.get(concept.concept_id.as_str()).cloned() else {
            continue;
        };
        let (lifelines, local_classifiers) = admitted_lifelines(concept, &path, claimed);
        let mut aliases = lifelines.clone();
        for (index, interaction_use) in concept.interaction_uses.iter().enumerate() {
            let (Some(link), Some(alias)) =
                (value(&interaction_use.link), value(&interaction_use.alias))
            else {
                continue;
            };
            if !is_sequence_identifier(alias) || !aliases.insert(alias.clone()) {
                continue;
            }
            let target = crate::okf::resolve_href(&path, link);
            let Some(target_concept) = declared.concept(&target) else {
                continue;
            };
            let target_is_sequence = context.okf.concept(&target).is_some_and(|concept| {
                crate::model::ElementType::parse(&concept.ty)
                    == crate::model::ElementType::Diagram(crate::model::DiagramKind::Sequence)
            });
            if !target_is_sequence {
                continue;
            }
            let target_path = concept_paths
                .get(target.as_str())
                .cloned()
                .unwrap_or_else(|| target.clone());
            let target_lifelines = lifeline_classifier_map(target_concept, &target_path);
            let participating = participating_lifelines(target_concept);
            let binding_entries = resolved_binding_entries(interaction_use);
            let has_unresolved_binding = binding_entries.len() != interaction_use.bindings.len();
            let resolved_bindings = binding_entries
                .iter()
                .map(|(binding, _)| binding.clone())
                .collect::<Vec<_>>();
            let issues = validate_use_bindings(
                &resolved_bindings,
                &lifelines,
                &local_classifiers,
                &target_lifelines,
                &participating,
            );
            let valid = !has_unresolved_binding && issues.is_empty();
            if valid {
                graph
                    .entry(concept.concept_id.clone())
                    .or_insert_with(Vec::new)
                    .push((index, target));
            }
        }
    }
    graph
}

fn graph_reaches(
    graph: &InteractionUseGraph,
    current: &str,
    goal: &str,
    visited: &mut BTreeSet<String>,
) -> bool {
    if current == goal {
        return true;
    }
    if !visited.insert(current.to_string()) {
        return false;
    }
    graph.get(current).is_some_and(|links| {
        links
            .iter()
            .any(|(_, target)| graph_reaches(graph, target, goal, visited))
    })
}

enum Ordered<'a> {
    Message(&'a super::DeclaredMessage),
    Fragment(&'a super::DeclaredFragment),
    Operand(&'a super::DeclaredOperand),
    InteractionUse(&'a super::DeclaredInteractionUse),
}

fn add_child(
    nodes: &mut [SeqNode],
    operand_stack: &[(usize, usize)],
    root: &mut Vec<SeqChild>,
    child: SeqChild,
) {
    if let Some((_, index)) = operand_stack.last() {
        if let SeqNode::Operand { items, .. } = &mut nodes[*index] {
            items.push(child);
            return;
        }
    }
    root.push(child);
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    concept: &DeclaredConcept,
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
    concept_paths: &BTreeMap<String, String>,
    model: &mut crate::model::Model,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut nodes = Vec::new();
    let mut lifelines = BTreeSet::new();
    let mut lifeline_classifiers = BTreeMap::new();
    for (lifeline, admission) in lifeline_admissions(concept, path, claimed) {
        match admission {
            LifelineAdmission::Unnamed => {}
            LifelineAdmission::ReservedName(id) => {
                validate_sequence_identifier(
                    context,
                    diagnostics,
                    &id,
                    "lifeline name",
                    path,
                    lifeline.syntax.syntax(),
                );
            }
            LifelineAdmission::Duplicate(id) => report_at(
                context,
                diagnostics,
                DiagCode::DuplicateSequenceName,
                format!("duplicate sequence lifeline name '{id}'"),
                path,
                lifeline.syntax.syntax(),
            ),
            LifelineAdmission::Admitted {
                id,
                title,
                alias,
                classifier,
            } => {
                lifelines.insert(id.clone());
                if let Some(classifier) = &classifier {
                    lifeline_classifiers.insert(id.clone(), classifier.clone());
                }
                nodes.push(SeqNode::Lifeline {
                    id,
                    title,
                    alias,
                    ref_: classifier,
                });
            }
        }
    }

    let mut gates = Vec::new();
    let mut gate_names = BTreeSet::new();
    for declared_gate in concept.gates.iter() {
        let Some(gate) = value(&declared_gate.name) else {
            continue;
        };
        if !validate_sequence_identifier(
            context,
            diagnostics,
            gate,
            "gate",
            path,
            declared_gate.syntax.syntax(),
        ) {
            continue;
        }
        if !gate_names.insert(gate.clone()) {
            report_at(
                context,
                diagnostics,
                DiagCode::DuplicateGate,
                format!("duplicate gate '{gate}'"),
                path,
                declared_gate.syntax.syntax(),
            );
        } else {
            gates.push(gate.clone());
        }
    }
    let target_gates = declared
        .concepts()
        .map(|target| {
            (
                target.concept_id.clone(),
                target
                    .gates
                    .iter()
                    .filter_map(|gate| {
                        value(&gate.name)
                            .filter(|name| is_sequence_identifier(name))
                            .cloned()
                    })
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let target_connected_gates = declared
        .concepts()
        .map(|target| {
            let mut connected = BTreeSet::new();
            for message in target.messages.iter() {
                for endpoint in [
                    value(&message.source),
                    value(&message.target),
                    value(&message.return_to),
                ]
                .into_iter()
                .flatten()
                {
                    if let DeclaredEndpointRef::LocalGate(gate) = endpoint {
                        if is_sequence_identifier(gate) {
                            connected.insert(gate.clone());
                        }
                    }
                }
            }
            (target.concept_id.clone(), connected)
        })
        .collect::<BTreeMap<_, _>>();

    let mut interaction_uses = Vec::new();
    let mut use_aliases = BTreeMap::new();
    let mut authored_use_aliases = BTreeSet::new();
    let use_graph = interaction_use_graph(context, declared, claimed, concept_paths);
    for (declared_use_index, declared_use) in concept.interaction_uses.iter().enumerate() {
        let (Some(link), Some(alias)) = (value(&declared_use.link), value(&declared_use.alias))
        else {
            continue;
        };
        let target = crate::okf::resolve_href(path, link);
        let target_concept = declared.concept(&target);
        let target_is_sequence = context.okf.concept(&target).is_some_and(|concept| {
            crate::model::ElementType::parse(&concept.ty)
                == crate::model::ElementType::Diagram(crate::model::DiagramKind::Sequence)
        });
        let mut valid_use = target_concept.is_some() && target_is_sequence;
        if !valid_use {
            report_at(
                context,
                diagnostics,
                DiagCode::InvalidInteractionUse,
                format!("unresolved sequence interaction use '{link}'"),
                path,
                declared_use.syntax.syntax(),
            );
        }
        if !validate_sequence_identifier(
            context,
            diagnostics,
            alias,
            "interaction-use alias",
            path,
            declared_use.syntax.syntax(),
        ) {
            valid_use = false;
        }
        if lifelines.contains(alias) || !authored_use_aliases.insert(alias.clone()) {
            report_at(
                context,
                diagnostics,
                DiagCode::DuplicateSequenceName,
                format!("duplicate sequence interaction-use alias '{alias}'"),
                path,
                declared_use.syntax.syntax(),
            );
            valid_use = false;
        }
        for binding in declared_use.bindings.iter() {
            if value(&binding.local).is_none() || value(&binding.target).is_none() {
                report_at(
                    context,
                    diagnostics,
                    DiagCode::InvalidInteractionUse,
                    format!("interaction use '{alias}' has an invalid binding"),
                    path,
                    binding.syntax.syntax(),
                );
                valid_use = false;
            }
        }
        let binding_entries = resolved_binding_entries(declared_use);
        let bindings = binding_entries
            .iter()
            .map(|(binding, _)| binding.clone())
            .collect::<Vec<_>>();
        if let Some(target_concept) = target_concept {
            let target_path = concept_paths
                .get(target.as_str())
                .cloned()
                .unwrap_or_else(|| target.clone());
            let target_lifelines = lifeline_classifier_map(target_concept, &target_path);
            let participating = participating_lifelines(target_concept);
            let issues = validate_use_bindings(
                &bindings,
                &lifelines,
                &lifeline_classifiers,
                &target_lifelines,
                &participating,
            );
            for (issue, index) in &issues {
                let syntax = match index {
                    Some(index) => binding_entries[*index].1.syntax.syntax(),
                    None => declared_use.syntax.syntax(),
                };
                let message = match issue {
                    UseBindingIssue::DuplicateBinding => {
                        format!("interaction use '{alias}' has duplicate bindings")
                    }
                    UseBindingIssue::UnknownEndpoint => {
                        format!("interaction use '{alias}' has an unknown binding endpoint")
                    }
                    UseBindingIssue::ClassifierMismatch => {
                        format!("interaction use '{alias}' binds different classifiers")
                    }
                    UseBindingIssue::MissingParticipant => format!(
                        "interaction use '{alias}' is missing a participating lifeline binding"
                    ),
                };
                report_at(
                    context,
                    diagnostics,
                    DiagCode::InvalidInteractionUse,
                    message,
                    path,
                    syntax,
                );
            }
            if !issues.is_empty() {
                valid_use = false;
            }
        }
        let is_graph_link = use_graph
            .get(&concept.concept_id)
            .is_some_and(|links| links.contains(&(declared_use_index, target.clone())));
        // The graph is built by `interaction_use_graph`, which walks concepts
        // under their own resolved paths and skips any concept it cannot find a
        // path for; this lowering runs under the caller's path, which may be
        // empty. The two derivations can therefore disagree for a concept
        // without a path entry, so the missing edge is not an invariant
        // violation — it simply means the cycle check below (which traverses
        // the graph) cannot vouch for this use, and it stays unadmitted. This
        // reports nothing: there is no defect in the document to name.
        valid_use = valid_use && is_graph_link;
        if valid_use
            && graph_reaches(
                &use_graph,
                &target,
                &concept.concept_id,
                &mut BTreeSet::new(),
            )
        {
            report_at(
                context,
                diagnostics,
                DiagCode::InteractionUseCycle,
                format!("interaction use '{alias}' enters a reference cycle"),
                path,
                declared_use.syntax.syntax(),
            );
            valid_use = false;
        }
        if !valid_use {
            continue;
        }
        let id = InteractionUseId(format!("u{declared_use_index}"));
        use_aliases.insert(alias.clone(), interaction_uses.len());
        interaction_uses.push(SeqInteractionUse {
            id,
            target,
            alias: alias.clone(),
            bindings,
            gates: Vec::new(),
        });
    }

    let mut ordered = concept
        .messages
        .iter()
        .map(|item| (item.syntax.syntax().range().start(), Ordered::Message(item)))
        .chain(concept.fragments.iter().map(|item| {
            (
                item.syntax.syntax().range().start(),
                Ordered::Fragment(item),
            )
        }))
        .chain(
            concept
                .operands
                .iter()
                .map(|item| (item.syntax.syntax().range().start(), Ordered::Operand(item))),
        )
        .chain(concept.interaction_uses.iter().map(|item| {
            (
                item.syntax.syntax().range().start(),
                Ordered::InteractionUse(item),
            )
        }))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(start, _)| *start);

    let mut endpoints = Endpoints {
        context,
        lifelines: &lifelines,
        gates: &gate_names,
        uses: &use_aliases,
        interaction_uses: &mut interaction_uses,
        target_gates: &target_gates,
        target_connected_gates: &target_connected_gates,
        path,
        diagnostics,
    };
    let mut edges = Vec::new();
    let mut root = Vec::new();
    let mut fragment_stack: Vec<(usize, usize)> = Vec::new();
    let mut operand_stack: Vec<(usize, usize)> = Vec::new();
    let fragment_indices = concept
        .fragments
        .iter()
        .enumerate()
        .map(|(index, fragment)| (fragment.syntax.syntax().range().start(), index))
        .collect::<BTreeMap<_, _>>();
    let message_indices = concept
        .messages
        .iter()
        .enumerate()
        .map(|(index, message)| (message.syntax.syntax().range().start(), index))
        .collect::<BTreeMap<_, _>>();
    let mut operand_indices = BTreeMap::new();
    let mut next_operand_index = BTreeMap::new();
    for operand in concept.operands.iter() {
        let start = operand.syntax.syntax().range().start();
        let owner = concept
            .fragments
            .iter()
            .filter(|fragment| {
                fragment.syntax.syntax().range().start() < start && fragment.depth < operand.depth
            })
            .max_by_key(|fragment| (fragment.depth, fragment.syntax.syntax().range().start()))
            .map(|fragment| fragment.syntax.syntax().range().start());
        if let Some(owner) = owner {
            let next = next_operand_index.entry(owner).or_insert(0usize);
            operand_indices.insert(start, *next);
            *next += 1;
        }
    }

    for (_, item) in ordered {
        match item {
            Ordered::Fragment(fragment) => {
                // Coupled with `validate_fragments`'s `declared_fragments` filter, which
                // applies this same predicate to stay aligned with the runtime fragments
                // built here.
                let Some(kind) = value(&fragment.kind).copied() else {
                    continue;
                };
                while fragment_stack
                    .last()
                    .is_some_and(|(depth, _)| *depth >= fragment.depth)
                {
                    fragment_stack.pop();
                }
                while operand_stack
                    .last()
                    .is_some_and(|(depth, _)| *depth >= fragment.depth)
                {
                    operand_stack.pop();
                }
                let fragment_index = fragment_indices[&fragment.syntax.syntax().range().start()];
                let id = format!("f{fragment_index}");
                let index = nodes.len();
                nodes.push(SeqNode::Fragment {
                    id: id.clone(),
                    kind,
                    operands: Vec::new(),
                });
                add_child(
                    &mut nodes,
                    &operand_stack,
                    &mut root,
                    SeqChild::Fragment { node: id },
                );
                fragment_stack.push((fragment.depth, index));
            }
            Ordered::Operand(operand) => {
                while operand_stack
                    .last()
                    .is_some_and(|(depth, _)| *depth >= operand.depth)
                {
                    operand_stack.pop();
                }
                let Some((_, fragment_index)) = fragment_stack
                    .iter()
                    .rev()
                    .find(|(depth, _)| *depth < operand.depth)
                    .copied()
                else {
                    continue;
                };
                let Some(spec) = value(&operand.spec) else {
                    continue;
                };
                let spec = match spec {
                    DeclaredOperandSpec::Guard(value) => OperandSpec::Guard(value.clone()),
                    DeclaredOperandSpec::Else => OperandSpec::Else,
                    DeclaredOperandSpec::Branch { label } => OperandSpec::Branch {
                        label: label.clone(),
                    },
                };
                let operand_index = operand_indices[&operand.syntax.syntax().range().start()];
                let id = match &nodes[fragment_index] {
                    SeqNode::Fragment { id, .. } => format!("{id}.o{operand_index}"),
                    _ => unreachable!(),
                };
                let index = nodes.len();
                nodes.push(SeqNode::Operand {
                    id: id.clone(),
                    spec,
                    items: Vec::new(),
                });
                if let SeqNode::Fragment { operands, .. } = &mut nodes[fragment_index] {
                    operands.push(id);
                }
                operand_stack.push((operand.depth, index));
            }
            Ordered::InteractionUse(declared_use) => {
                while operand_stack
                    .last()
                    .is_some_and(|(depth, _)| *depth >= declared_use.depth)
                {
                    operand_stack.pop();
                }
                if let Some(alias) = value(&declared_use.alias) {
                    if let Some(&index) = use_aliases.get(alias) {
                        add_child(
                            &mut nodes,
                            &operand_stack,
                            &mut root,
                            SeqChild::InteractionUse {
                                interaction_use: endpoints.interaction_uses[index].id.clone(),
                            },
                        );
                    }
                }
            }
            Ordered::Message(message) => {
                while operand_stack
                    .last()
                    .is_some_and(|(depth, _)| *depth >= message.depth)
                {
                    operand_stack.pop();
                }
                let (Some(source), Some(kind)) = (value(&message.source), value(&message.kind))
                else {
                    continue;
                };
                let kind = message_kind(*kind);
                let call_identity = if kind == MessageKind::Reply {
                    value(&message.return_for)
                } else {
                    value(&message.call_id)
                };
                if call_identity.is_some_and(|identity| {
                    !validate_sequence_identifier(
                        context,
                        endpoints.diagnostics,
                        identity,
                        "call identity",
                        path,
                        message.syntax.syntax(),
                    )
                }) {
                    continue;
                }
                let Some(from) = endpoints.resolve(source, message.syntax.syntax()) else {
                    continue;
                };
                let authored_target = if kind == MessageKind::Reply {
                    value(&message.return_to)
                } else {
                    value(&message.target)
                };
                let to = match authored_target {
                    Some(target) => {
                        let Some(target) = endpoints.resolve(target, message.syntax.syntax())
                        else {
                            continue;
                        };
                        Some(target)
                    }
                    None => None,
                };
                if matches!(from, EndpointRef::Outside) && matches!(to, Some(EndpointRef::Outside))
                {
                    report_at(
                        context,
                        endpoints.diagnostics,
                        DiagCode::InvalidSequenceEndpoint,
                        "outside cannot be both message endpoints",
                        path,
                        message.syntax.syntax(),
                    );
                    continue;
                }
                for (interaction_use, gate) in
                    std::iter::once(&from)
                        .chain(to.iter())
                        .filter_map(|endpoint| match endpoint {
                            EndpointRef::UseGate {
                                interaction_use,
                                gate,
                            } => Some((interaction_use.clone(), gate.clone())),
                            _ => None,
                        })
                {
                    if let Some(interaction_use) = endpoints
                        .interaction_uses
                        .iter_mut()
                        .find(|candidate| candidate.id == interaction_use)
                    {
                        if !interaction_use.gates.contains(&gate) {
                            interaction_use.gates.push(gate);
                        }
                    }
                }
                let message_index = message_indices[&message.syntax.syntax().range().start()];
                let id = MessageId(message_index);
                edges.push(SeqEdge {
                    id,
                    from,
                    kind,
                    to,
                    value: value(&message.value).cloned(),
                    call_id: call_identity.cloned(),
                    returns_call: None,
                });
                add_child(
                    &mut nodes,
                    &operand_stack,
                    &mut root,
                    SeqChild::Message { edge: id },
                );
            }
        }
    }
    resolve_returns(
        context,
        concept,
        &mut edges,
        &nodes,
        &root,
        diagnostics,
        path,
    );
    validate_fragments(context, concept, &nodes, diagnostics, path);
    validate_lifetimes(context, concept, &edges, &nodes, &root, diagnostics, path);

    model.interactions.push(SequenceDoc {
        key: concept.concept_id.clone(),
        title: okf
            .title
            .clone()
            .unwrap_or_else(|| concept.concept_id.clone()),
        describes: super::analysis::resolve_describes(okf, path, claimed),
        nodes,
        edges,
        gates,
        interaction_uses,
        items: root,
    });
}

fn resolve_returns(
    context: &DomainAnalysisContext<'_>,
    concept: &DeclaredConcept,
    edges: &mut [SeqEdge],
    nodes: &[SeqNode],
    items: &[SeqChild],
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    let ctx = FoldCtx {
        context,
        concept,
        path,
    };
    let mut call_ids: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, edge) in edges.iter().enumerate() {
        if matches!(edge.kind, MessageKind::SyncCall | MessageKind::AsyncCall) {
            if let Some(call_id) = &edge.call_id {
                call_ids.entry(call_id.clone()).or_default().push(index);
            }
        }
    }
    for (call_id, entries) in &call_ids {
        if entries.len() > 1 {
            for index in entries {
                report_message(
                    &ctx,
                    diagnostics,
                    DiagCode::DuplicateCallIdentity,
                    format!("duplicate call identity '{call_id}'"),
                    &edges[*index].id,
                );
            }
        }
    }
    let edge_by_id = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.id, index))
        .collect::<BTreeMap<_, _>>();
    let node_by_id = nodes
        .iter()
        .filter_map(|node| match node {
            SeqNode::Fragment { id, .. } | SeqNode::Operand { id, .. } => Some((id.clone(), node)),
            SeqNode::Lifeline { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut open = BTreeSet::new();
    walk_return_items(
        items,
        &mut open,
        edges,
        &edge_by_id,
        &node_by_id,
        &call_ids,
        &ctx,
        diagnostics,
    );
}

// `edges` is a mutable slice indexed by position (return-resolution mutates
// it in place), so it can't share `SeqLookup`'s by-reference map shape with
// `node_by_id` — that keeps this one at 8 read/write parameters.
#[allow(clippy::too_many_arguments)]
fn walk_return_items(
    items: &[SeqChild],
    open: &mut BTreeSet<usize>,
    edges: &mut [SeqEdge],
    edge_by_id: &BTreeMap<MessageId, usize>,
    node_by_id: &BTreeMap<String, &SeqNode>,
    call_ids: &BTreeMap<String, Vec<usize>>,
    ctx: &FoldCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for item in items {
        match item {
            SeqChild::Message { edge } => {
                let Some(&index) = edge_by_id.get(edge) else {
                    continue;
                };
                match edges[index].kind {
                    MessageKind::SyncCall | MessageKind::AsyncCall => {
                        open.insert(index);
                    }
                    MessageKind::Reply => {
                        resolve_one_return(index, open, edges, call_ids, ctx, diagnostics)
                    }
                    MessageKind::AsyncSignal | MessageKind::Create | MessageKind::Delete => {}
                }
            }
            SeqChild::Fragment { node } => {
                let Some(SeqNode::Fragment { kind, operands, .. }) = node_by_id.get(node).copied()
                else {
                    continue;
                };
                let has_else = operands.iter().any(|operand| {
                    matches!(
                        node_by_id.get(operand).copied(),
                        Some(SeqNode::Operand {
                            spec: OperandSpec::Else,
                            ..
                        })
                    )
                });
                let mut outcomes = Vec::new();
                for operand in operands {
                    let mut branch = open.clone();
                    if let Some(SeqNode::Operand { items, .. }) = node_by_id.get(operand).copied() {
                        walk_return_items(
                            items,
                            &mut branch,
                            edges,
                            edge_by_id,
                            node_by_id,
                            call_ids,
                            ctx,
                            diagnostics,
                        );
                    }
                    outcomes.push(branch);
                }
                // This walker's readmit rule is operand-count dependent, unlike
                // `repeated_deletes`/`walk` below, which both apply
                // `fragment_readmits_incoming` for every operand count. Here a
                // lone operand readmits only for the zero-times kinds
                // (`opt`/`loop`/`break`) — never for an else-less `alt` — while
                // any other operand count readmits only for an else-less `alt`,
                // never for the zero-times kinds. That divergence pre-dates the
                // fragment-fold refactor; preserved rather than silently
                // unified.
                let readmits = fragment_readmits_incoming(*kind, has_else)
                    && (*kind == FragmentKind::Alt) != (operands.len() == 1);
                if readmits {
                    outcomes.push(open.clone());
                }
                // Unconditional, including for an operand-less fragment: an
                // authored fragment with no operands wipes the open-call set
                // unless it readmits, matching this walker's pre-refactor
                // behaviour.
                *open = merge_union(outcomes);
            }
            SeqChild::InteractionUse { .. } => {}
        }
    }
}

fn resolve_one_return(
    index: usize,
    open: &mut BTreeSet<usize>,
    edges: &mut [SeqEdge],
    call_ids: &BTreeMap<String, Vec<usize>>,
    ctx: &FoldCtx<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let authored_to = edges[index].to.clone();
    let selected = if let Some(authored_for) = edges[index].call_id.clone() {
        let preceding = call_ids
            .get(&authored_for)
            .map(|entries| {
                entries
                    .iter()
                    .copied()
                    .filter(|candidate| *candidate < index)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        match preceding.as_slice() {
            [candidate] => Some(*candidate),
            [] => {
                report_message(
                    ctx,
                    diagnostics,
                    DiagCode::UnknownCallIdentity,
                    format!("unknown call identity '{authored_for}'"),
                    &edges[index].id,
                );
                None
            }
            _ => {
                report_message(
                    ctx,
                    diagnostics,
                    DiagCode::AmbiguousReturn,
                    format!("call identity '{authored_for}' is not unique"),
                    &edges[index].id,
                );
                None
            }
        }
    } else {
        let candidates = open
            .iter()
            .copied()
            .filter(|candidate| {
                edges[*candidate].to.as_ref() == Some(&edges[index].from)
                    && match authored_to.as_ref() {
                        None => true,
                        Some(to) => to == &edges[*candidate].from,
                    }
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [candidate] => Some(*candidate),
            [] => {
                report_message(
                    ctx,
                    diagnostics,
                    DiagCode::UnmatchedReturn,
                    "return has no eligible preceding call",
                    &edges[index].id,
                );
                None
            }
            _ => {
                report_message(
                    ctx,
                    diagnostics,
                    DiagCode::AmbiguousReturn,
                    "return matches more than one preceding call",
                    &edges[index].id,
                );
                None
            }
        }
    };
    let Some(candidate) = selected else { return };
    if !open.contains(&candidate) {
        report_message(
            ctx,
            diagnostics,
            DiagCode::CompletedReturn,
            "call already has an explicit return",
            &edges[index].id,
        );
        return;
    }
    let source_matches = edges[candidate].to.as_ref() == Some(&edges[index].from);
    let to_matches = match authored_to.as_ref() {
        None => true,
        Some(to) => to == &edges[candidate].from,
    };
    if !source_matches || !to_matches {
        report_message(
            ctx,
            diagnostics,
            DiagCode::ConflictingReturn,
            "return endpoints conflict with the selected call",
            &edges[index].id,
        );
        return;
    }
    edges[index].to = Some(edges[candidate].from.clone());
    edges[index].returns_call = Some(edges[candidate].id);
    open.remove(&candidate);
}

fn validate_fragments(
    context: &DomainAnalysisContext<'_>,
    concept: &DeclaredConcept,
    nodes: &[SeqNode],
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    let by_id = nodes
        .iter()
        .filter_map(|node| match node {
            SeqNode::Operand { id, spec, .. } => Some((id.as_str(), spec)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    // Coupled with the fragment builder's filter below; both must apply the same
    // `value(&fragment.kind).is_some()` predicate or this count can drift.
    let mut declared_fragments = concept
        .fragments
        .iter()
        .filter(|fragment| value(&fragment.kind).is_some());
    for node in nodes {
        let SeqNode::Fragment { kind, operands, .. } = node else {
            continue;
        };
        // Coupled with the filter above: `declared_fragments` is only advanced for
        // fragments with a typed `kind`, matching how runtime `SeqNode::Fragment`s are built.
        let Some(declared_fragment) = declared_fragments.next() else {
            debug_assert!(false, "runtime fragment without typed declared fragment");
            continue;
        };
        let specs = operands
            .iter()
            .filter_map(|id| by_id.get(id.as_str()).copied())
            .collect::<Vec<_>>();
        let valid = match kind {
            FragmentKind::Alt => {
                !specs.is_empty()
                    && specs
                        .iter()
                        .any(|spec| matches!(spec, OperandSpec::Guard(_)))
                    && specs.iter().enumerate().all(|(index, spec)| match spec {
                        OperandSpec::Guard(_) => true,
                        OperandSpec::Else => index + 1 == specs.len(),
                        OperandSpec::Branch { .. } => false,
                    })
            }
            FragmentKind::Opt | FragmentKind::Loop | FragmentKind::Break => {
                matches!(specs.as_slice(), [OperandSpec::Guard(_)])
            }
            FragmentKind::Par => {
                specs.len() >= 2
                    && specs
                        .iter()
                        .all(|spec| matches!(spec, OperandSpec::Branch { .. }))
            }
            FragmentKind::Critical | FragmentKind::Assert | FragmentKind::Neg => {
                matches!(specs.as_slice(), [OperandSpec::Branch { .. }])
            }
        };
        if !valid {
            super::analysis::behavior_diagnostic(
                context,
                path,
                declared_fragment.syntax.syntax(),
                DiagCode::InvalidFragmentOperands,
                format!("invalid operands for '{}' fragment", kind.as_str()),
                diagnostics,
            );
        }
    }
}

fn validate_lifetimes(
    context: &DomainAnalysisContext<'_>,
    concept: &DeclaredConcept,
    edges: &[SeqEdge],
    nodes: &[SeqNode],
    items: &[SeqChild],
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    fn repeated_deletes(
        items: &[SeqChild],
        lookup: &SeqLookup<'_>,
        deleted: &mut BTreeSet<String>,
        repeated: &mut BTreeSet<MessageId>,
        ctx: &FoldCtx<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for item in items {
            match item {
                SeqChild::Message { edge } => {
                    let Some(SeqEdge {
                        kind: MessageKind::Delete,
                        to: Some(EndpointRef::Lifeline { id }),
                        ..
                    }) = lookup.edges.get(edge).copied()
                    else {
                        continue;
                    };
                    if !deleted.insert(id.clone()) {
                        repeated.insert(*edge);
                        report_message(
                            ctx,
                            diagnostics,
                            DiagCode::InvalidLifelineLifetime,
                            "lifeline is created or deleted more than once",
                            edge,
                        );
                    }
                }
                SeqChild::InteractionUse { .. } => {}
                SeqChild::Fragment { node } => {
                    let Some(SeqNode::Fragment { kind, operands, .. }) =
                        lookup.nodes.get(node).copied()
                    else {
                        continue;
                    };
                    let incoming = deleted.clone();
                    if *kind == FragmentKind::Par {
                        for operand in operands {
                            let Some(SeqNode::Operand { items, .. }) =
                                lookup.nodes.get(operand).copied()
                            else {
                                continue;
                            };
                            repeated_deletes(items, lookup, deleted, repeated, ctx, diagnostics);
                        }
                        continue;
                    }
                    let mut outcomes = Vec::new();
                    for operand in operands {
                        let Some(SeqNode::Operand { items, .. }) =
                            lookup.nodes.get(operand).copied()
                        else {
                            continue;
                        };
                        let mut branch = incoming.clone();
                        repeated_deletes(items, lookup, &mut branch, repeated, ctx, diagnostics);
                        outcomes.push(branch);
                    }
                    let has_else = operands.iter().any(|operand| {
                        matches!(
                            lookup.nodes.get(operand).copied(),
                            Some(SeqNode::Operand {
                                spec: OperandSpec::Else,
                                ..
                            })
                        )
                    });
                    if fragment_readmits_incoming(*kind, has_else) {
                        outcomes.push(incoming);
                    }
                    if !outcomes.is_empty() {
                        *deleted = merge_intersect(outcomes);
                    }
                }
            }
        }
    }

    fn walk(
        items: &[SeqChild],
        lookup: &SeqLookup<'_>,
        alive: &mut BTreeSet<String>,
        repeated_deletes: &BTreeSet<MessageId>,
        ctx: &FoldCtx<'_>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        fn created_in(items: &[SeqChild], lookup: &SeqLookup<'_>, out: &mut BTreeSet<String>) {
            for item in items {
                match item {
                    SeqChild::Message { edge } => {
                        if let Some(SeqEdge {
                            kind: MessageKind::Create,
                            to: Some(EndpointRef::Lifeline { id }),
                            ..
                        }) = lookup.edges.get(edge).copied()
                        {
                            out.insert(id.clone());
                        }
                    }
                    SeqChild::Fragment { node } => {
                        if let Some(SeqNode::Fragment { operands, .. }) =
                            lookup.nodes.get(node).copied()
                        {
                            for operand in operands {
                                if let Some(SeqNode::Operand { items, .. }) =
                                    lookup.nodes.get(operand).copied()
                                {
                                    created_in(items, lookup, out);
                                }
                            }
                        }
                    }
                    SeqChild::InteractionUse { .. } => {}
                }
            }
        }
        for item in items {
            match item {
                SeqChild::Message { edge } => {
                    let Some(message) = lookup.edges.get(edge).copied() else {
                        continue;
                    };
                    let mut invalid = None;
                    let mut require_alive = |endpoint: &EndpointRef| {
                        if let EndpointRef::Lifeline { id } = endpoint {
                            if !alive.contains(id) {
                                invalid.get_or_insert_with(|| id.clone());
                            }
                        }
                    };
                    require_alive(&message.from);
                    if message.kind != MessageKind::Create
                        && !(message.kind == MessageKind::Delete && repeated_deletes.contains(edge))
                    {
                        if let Some(target) = &message.to {
                            require_alive(target);
                        }
                    }
                    if let Some(id) = invalid {
                        report_message(
                            ctx,
                            diagnostics,
                            DiagCode::InvalidLifelineLifetime,
                            format!("lifeline '{id}' is used outside its lifetime"),
                            edge,
                        );
                    }
                    if let Some(EndpointRef::Lifeline { id }) = &message.to {
                        match message.kind {
                            MessageKind::Create => {
                                if alive.contains(id) {
                                    report_message(
                                        ctx,
                                        diagnostics,
                                        DiagCode::InvalidLifelineLifetime,
                                        "lifeline is created or deleted more than once",
                                        edge,
                                    );
                                }
                                alive.insert(id.clone());
                            }
                            MessageKind::Delete => {
                                alive.remove(id);
                            }
                            _ => {}
                        }
                    }
                }
                SeqChild::InteractionUse { .. } => {}
                SeqChild::Fragment { node } => {
                    let Some(SeqNode::Fragment { kind, operands, .. }) =
                        lookup.nodes.get(node).copied()
                    else {
                        continue;
                    };
                    let incoming = alive.clone();
                    let mut outcomes = Vec::new();
                    let operand_creates = operands
                        .iter()
                        .map(|operand| {
                            let mut creates = BTreeSet::new();
                            if let Some(SeqNode::Operand { items, .. }) =
                                lookup.nodes.get(operand).copied()
                            {
                                created_in(items, lookup, &mut creates);
                            }
                            creates
                        })
                        .collect::<Vec<_>>();
                    for (operand_index, operand) in operands.iter().enumerate() {
                        let Some(SeqNode::Operand { items, .. }) =
                            lookup.nodes.get(operand).copied()
                        else {
                            continue;
                        };
                        let mut branch = incoming.clone();
                        if *kind == FragmentKind::Par {
                            for (sibling_index, creates) in operand_creates.iter().enumerate() {
                                if sibling_index != operand_index {
                                    branch.extend(creates.iter().cloned());
                                }
                            }
                        }
                        walk(
                            items,
                            lookup,
                            &mut branch,
                            repeated_deletes,
                            ctx,
                            diagnostics,
                        );
                        outcomes.push(branch);
                    }
                    let has_else = operands.iter().any(|operand| {
                        matches!(
                            lookup.nodes.get(operand).copied(),
                            Some(SeqNode::Operand {
                                spec: OperandSpec::Else,
                                ..
                            })
                        )
                    });
                    if fragment_readmits_incoming(*kind, has_else) {
                        outcomes.push(incoming.clone());
                    }
                    if !outcomes.is_empty() {
                        *alive = merge_intersect(outcomes);
                    }
                }
            }
        }
    }
    let ctx = FoldCtx {
        context,
        concept,
        path,
    };
    let node_by_id = nodes
        .iter()
        .filter_map(|node| match node {
            SeqNode::Fragment { id, .. } | SeqNode::Operand { id, .. } => Some((id.clone(), node)),
            SeqNode::Lifeline { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut creates: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, edge) in edges.iter().enumerate() {
        let target = edge.to.as_ref();
        if matches!(edge.kind, MessageKind::Create | MessageKind::Delete)
            && !matches!(target, Some(EndpointRef::Lifeline { .. }))
        {
            report_message(
                &ctx,
                diagnostics,
                DiagCode::InvalidSequenceEndpoint,
                "create and delete targets must be local lifelines",
                &edge.id,
            );
            continue;
        }
        if let Some(EndpointRef::Lifeline { id }) = target {
            match edge.kind {
                MessageKind::Create => creates.entry(id.clone()).or_default().push(index),
                MessageKind::Delete => {}
                _ => {}
            }
        }
    }
    let mut alive = nodes
        .iter()
        .filter_map(|node| match node {
            SeqNode::Lifeline { id, .. } if !creates.contains_key(id) => Some(id.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let edge_by_id = edges
        .iter()
        .map(|edge| (edge.id, edge))
        .collect::<BTreeMap<_, _>>();
    let lookup = SeqLookup {
        nodes: &node_by_id,
        edges: &edge_by_id,
    };
    let mut repeated_delete_messages = BTreeSet::new();
    repeated_deletes(
        items,
        &lookup,
        &mut BTreeSet::new(),
        &mut repeated_delete_messages,
        &ctx,
        diagnostics,
    );
    walk(
        items,
        &lookup,
        &mut alive,
        &repeated_delete_messages,
        &ctx,
        diagnostics,
    );
}

#[cfg(test)]
mod use_binding_verdict_tests {
    use super::*;

    fn binding(local: &str, target: &str) -> SeqBinding {
        SeqBinding {
            local: local.to_string(),
            target: target.to_string(),
        }
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn map(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn ok_bindings_produce_no_issues() {
        let bindings = vec![binding("a", "x")];
        let lifelines = set(&["a"]);
        let lifeline_classifiers = map(&[("a", "Cls")]);
        let target_lifelines = map(&[("x", "Cls")]);
        let participating = set(&["x"]);
        let issues = validate_use_bindings(
            &bindings,
            &lifelines,
            &lifeline_classifiers,
            &target_lifelines,
            &participating,
        );
        assert!(issues.is_empty());
    }

    #[test]
    fn duplicate_binding_is_flagged() {
        let bindings = vec![binding("a", "x"), binding("a", "y")];
        let lifelines = set(&["a"]);
        let target_lifelines = map(&[("x", "Cls"), ("y", "Cls")]);
        let issues = validate_use_bindings(
            &bindings,
            &lifelines,
            &BTreeMap::new(),
            &target_lifelines,
            &BTreeSet::new(),
        );
        assert!(
            issues
                .iter()
                .any(|(issue, index)| *issue == UseBindingIssue::DuplicateBinding
                    && *index == Some(1))
        );
    }

    #[test]
    fn unknown_endpoint_is_flagged() {
        let bindings = vec![binding("missing", "x")];
        let lifelines = set(&["a"]);
        let target_lifelines = map(&[("x", "Cls")]);
        let issues = validate_use_bindings(
            &bindings,
            &lifelines,
            &BTreeMap::new(),
            &target_lifelines,
            &BTreeSet::new(),
        );
        assert!(issues
            .iter()
            .any(|(issue, index)| *issue == UseBindingIssue::UnknownEndpoint && *index == Some(0)));
    }

    #[test]
    fn classifier_mismatch_is_flagged() {
        let bindings = vec![binding("a", "x")];
        let lifelines = set(&["a"]);
        let lifeline_classifiers = map(&[("a", "ClsA")]);
        let target_lifelines = map(&[("x", "ClsB")]);
        let issues = validate_use_bindings(
            &bindings,
            &lifelines,
            &lifeline_classifiers,
            &target_lifelines,
            &BTreeSet::new(),
        );
        assert!(issues.iter().any(|(issue, index)| {
            *issue == UseBindingIssue::ClassifierMismatch && *index == Some(0)
        }));
    }

    #[test]
    fn missing_participant_is_flagged() {
        let bindings = vec![binding("a", "x")];
        let lifelines = set(&["a"]);
        let target_lifelines = map(&[("x", "Cls"), ("y", "Cls")]);
        let participating = set(&["x", "y"]);
        let issues = validate_use_bindings(
            &bindings,
            &lifelines,
            &BTreeMap::new(),
            &target_lifelines,
            &participating,
        );
        assert!(issues.iter().any(
            |(issue, index)| *issue == UseBindingIssue::MissingParticipant && index.is_none()
        ));
    }
}
