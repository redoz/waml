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
    context: &DomainAnalysisContext<'_>,
    concept: &DeclaredConcept,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagCode,
    message: impl Into<String>,
    path: &str,
    id: &MessageId,
) {
    let Some(index) =
        id.0.strip_prefix('m')
            .and_then(|value| value.parse::<usize>().ok())
    else {
        return;
    };
    let Some(declared) = concept.messages.get(index) else {
        return;
    };
    report_at(
        context,
        diagnostics,
        code,
        message,
        path,
        declared.syntax.syntax(),
    );
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

fn path_for_concept(context: &DomainAnalysisContext<'_>, concept_id: &str) -> Option<String> {
    context
        .catalog
        .documents()
        .iter()
        .find_map(|(_, document)| {
            (crate::okf::id_of(document.path().as_str()) == concept_id)
                .then(|| document.path().as_str().to_string())
        })
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

fn interaction_use_graph(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    claimed: &BTreeSet<&str>,
) -> InteractionUseGraph {
    let mut graph = BTreeMap::new();
    for concept in declared.concepts() {
        let Some(path) = path_for_concept(context, &concept.concept_id) else {
            continue;
        };
        let mut lifelines = BTreeSet::new();
        let mut local_classifiers = BTreeMap::new();
        for lifeline in concept.lifelines.iter() {
            let (Some(link), Some(title)) = (value(&lifeline.target), value(&lifeline.title))
            else {
                continue;
            };
            let handle = value(&lifeline.alias).unwrap_or(title);
            if handle == "outside" || handle.contains('@') || !lifelines.insert(handle.clone()) {
                continue;
            }
            let classifier = crate::okf::resolve_href(&path, link);
            if claimed.contains(classifier.as_str()) {
                local_classifiers.insert(handle.clone(), classifier);
            }
        }
        let mut aliases = lifelines.clone();
        for (index, interaction_use) in concept.interaction_uses.iter().enumerate() {
            let (Some(link), Some(alias)) =
                (value(&interaction_use.link), value(&interaction_use.alias))
            else {
                continue;
            };
            if !aliases.insert(alias.clone()) {
                continue;
            }
            let target = crate::okf::resolve_href(&path, link);
            let Some(target_concept) = declared.concept(&target) else {
                continue;
            };
            let target_is_sequence = context.okf.concept(&target).is_some_and(|concept| {
                crate::model::ElementType::parse(&concept.ty)
                    == crate::model::ElementType::Behavior(crate::model::BehaviorKind::Sequence)
            });
            if !target_is_sequence {
                continue;
            }
            let target_path = path_for_concept(context, &target).unwrap_or_else(|| target.clone());
            let target_lifelines = lifeline_classifier_map(target_concept, &target_path);
            let participating = participating_lifelines(target_concept);
            let mut locals = BTreeSet::new();
            let mut targets = BTreeSet::new();
            let mut valid = true;
            for binding in interaction_use.bindings.iter() {
                let (Some(local), Some(bound_target)) =
                    (value(&binding.local), value(&binding.target))
                else {
                    valid = false;
                    continue;
                };
                if !locals.insert(local.clone()) || !targets.insert(bound_target.clone()) {
                    valid = false;
                }
                if !lifelines.contains(local) || !target_lifelines.contains_key(bound_target) {
                    valid = false;
                }
                if let (Some(local_classifier), Some(target_classifier)) = (
                    local_classifiers.get(local),
                    target_lifelines.get(bound_target),
                ) {
                    if local_classifier != target_classifier {
                        valid = false;
                    }
                }
            }
            if participating.iter().any(|handle| !targets.contains(handle)) {
                valid = false;
            }
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
    model: &mut crate::model::Model,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut nodes = Vec::new();
    let mut lifelines = BTreeSet::new();
    let mut lifeline_classifiers = BTreeMap::new();
    for lifeline in concept.lifelines.iter() {
        let (Some(slug), Some(title)) = (value(&lifeline.target), value(&lifeline.title)) else {
            continue;
        };
        let alias = value(&lifeline.alias).cloned();
        let id = alias.clone().unwrap_or_else(|| title.clone());
        if !validate_sequence_identifier(
            context,
            diagnostics,
            &id,
            "lifeline name",
            path,
            lifeline.syntax.syntax(),
        ) {
            continue;
        }
        if !lifelines.insert(id.clone()) {
            report_at(
                context,
                diagnostics,
                DiagCode::DuplicateSequenceName,
                format!("duplicate sequence lifeline name '{id}'"),
                path,
                lifeline.syntax.syntax(),
            );
            continue;
        }
        let target = crate::okf::resolve_href(path, slug);
        let ref_ = claimed.contains(target.as_str()).then_some(target);
        if let Some(classifier) = &ref_ {
            lifeline_classifiers.insert(id.clone(), classifier.clone());
        }
        nodes.push(SeqNode::Lifeline {
            id,
            title: title.clone(),
            alias,
            ref_,
        });
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
    let use_graph = interaction_use_graph(context, declared, claimed);
    for (declared_use_index, declared_use) in concept.interaction_uses.iter().enumerate() {
        let (Some(link), Some(alias)) = (value(&declared_use.link), value(&declared_use.alias))
        else {
            continue;
        };
        let target = crate::okf::resolve_href(path, link);
        let target_concept = declared.concept(&target);
        let target_is_sequence = context.okf.concept(&target).is_some_and(|concept| {
            crate::model::ElementType::parse(&concept.ty)
                == crate::model::ElementType::Behavior(crate::model::BehaviorKind::Sequence)
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
        let binding_entries = declared_use
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
            .collect::<Vec<_>>();
        let bindings = binding_entries
            .iter()
            .map(|(binding, _)| binding.clone())
            .collect::<Vec<_>>();
        if let Some(target_concept) = target_concept {
            let target_path = path_for_concept(context, &target).unwrap_or_else(|| target.clone());
            let target_lifelines = lifeline_classifier_map(target_concept, &target_path);
            let participating = participating_lifelines(target_concept);
            let mut locals = BTreeSet::new();
            let mut targets = BTreeSet::new();
            for (binding, declared_binding) in &binding_entries {
                if !locals.insert(binding.local.clone()) || !targets.insert(binding.target.clone())
                {
                    report_at(
                        context,
                        diagnostics,
                        DiagCode::InvalidInteractionUse,
                        format!("interaction use '{alias}' has duplicate bindings"),
                        path,
                        declared_binding.syntax.syntax(),
                    );
                    valid_use = false;
                }
                if !lifelines.contains(&binding.local)
                    || !target_lifelines.contains_key(&binding.target)
                {
                    report_at(
                        context,
                        diagnostics,
                        DiagCode::InvalidInteractionUse,
                        format!("interaction use '{alias}' has an unknown binding endpoint"),
                        path,
                        declared_binding.syntax.syntax(),
                    );
                    valid_use = false;
                }
                if let (Some(local_classifier), Some(target_classifier)) = (
                    lifeline_classifiers.get(&binding.local),
                    target_lifelines.get(&binding.target),
                ) {
                    if local_classifier != target_classifier {
                        report_at(
                            context,
                            diagnostics,
                            DiagCode::InvalidInteractionUse,
                            format!("interaction use '{alias}' binds different classifiers"),
                            path,
                            declared_binding.syntax.syntax(),
                        );
                        valid_use = false;
                    }
                }
            }
            if participating.iter().any(|handle| !targets.contains(handle)) {
                report_at(
                    context,
                    diagnostics,
                    DiagCode::InvalidInteractionUse,
                    format!(
                        "interaction use '{alias}' is missing a participating lifeline binding"
                    ),
                    path,
                    declared_use.syntax.syntax(),
                );
                valid_use = false;
            }
        }
        let is_graph_link = use_graph
            .get(&concept.concept_id)
            .is_some_and(|links| links.contains(&(declared_use_index, target.clone())));
        if valid_use && !is_graph_link {
            valid_use = false;
        }
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
                let id = MessageId(format!("m{message_index}"));
                edges.push(SeqEdge {
                    id: id.clone(),
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
        describes: okf
            .extra
            .get_str("describes")
            .and_then(|authored| authored.split("](").nth(1))
            .and_then(|href| href.strip_suffix(')'))
            .map(|href| crate::okf::resolve_href(path, href))
            .filter(|target| claimed.contains(target.as_str())),
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
                    context,
                    concept,
                    diagnostics,
                    DiagCode::DuplicateCallIdentity,
                    format!("duplicate call identity '{call_id}'"),
                    path,
                    &edges[*index].id,
                );
            }
        }
    }
    let edge_by_id = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| (edge.id.clone(), index))
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
        context,
        concept,
        diagnostics,
        path,
    );
}

#[allow(clippy::too_many_arguments)]
fn walk_return_items(
    items: &[SeqChild],
    open: &mut BTreeSet<usize>,
    edges: &mut [SeqEdge],
    edge_by_id: &BTreeMap<MessageId, usize>,
    node_by_id: &BTreeMap<String, &SeqNode>,
    call_ids: &BTreeMap<String, Vec<usize>>,
    context: &DomainAnalysisContext<'_>,
    concept: &DeclaredConcept,
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
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
                    MessageKind::Reply => resolve_one_return(
                        index,
                        open,
                        edges,
                        call_ids,
                        context,
                        concept,
                        diagnostics,
                        path,
                    ),
                    MessageKind::AsyncSignal | MessageKind::Create | MessageKind::Delete => {}
                }
            }
            SeqChild::Fragment { node } => {
                let Some(SeqNode::Fragment { kind, operands, .. }) = node_by_id.get(node).copied()
                else {
                    continue;
                };
                if operands.len() == 1 {
                    let incoming = open.clone();
                    if let Some(SeqNode::Operand { items, .. }) =
                        node_by_id.get(&operands[0]).copied()
                    {
                        walk_return_items(
                            items,
                            open,
                            edges,
                            edge_by_id,
                            node_by_id,
                            call_ids,
                            context,
                            concept,
                            diagnostics,
                            path,
                        );
                    }
                    if matches!(
                        kind,
                        FragmentKind::Opt | FragmentKind::Loop | FragmentKind::Break
                    ) {
                        open.extend(incoming);
                    }
                    continue;
                }
                let incoming = open.clone();
                let mut joined = BTreeSet::new();
                for operand in operands {
                    let mut branch = incoming.clone();
                    if let Some(SeqNode::Operand { items, .. }) = node_by_id.get(operand).copied() {
                        walk_return_items(
                            items,
                            &mut branch,
                            edges,
                            edge_by_id,
                            node_by_id,
                            call_ids,
                            context,
                            concept,
                            diagnostics,
                            path,
                        );
                    }
                    joined.extend(branch);
                }
                if *kind == FragmentKind::Alt
                    && !operands.iter().any(|operand| {
                        matches!(
                            node_by_id.get(operand).copied(),
                            Some(SeqNode::Operand {
                                spec: OperandSpec::Else,
                                ..
                            })
                        )
                    })
                {
                    joined.extend(incoming);
                }
                *open = joined;
            }
            SeqChild::InteractionUse { .. } => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_one_return(
    index: usize,
    open: &mut BTreeSet<usize>,
    edges: &mut [SeqEdge],
    call_ids: &BTreeMap<String, Vec<usize>>,
    context: &DomainAnalysisContext<'_>,
    concept: &DeclaredConcept,
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
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
                    context,
                    concept,
                    diagnostics,
                    DiagCode::UnknownCallIdentity,
                    format!("unknown call identity '{authored_for}'"),
                    path,
                    &edges[index].id,
                );
                None
            }
            _ => {
                report_message(
                    context,
                    concept,
                    diagnostics,
                    DiagCode::AmbiguousReturn,
                    format!("call identity '{authored_for}' is not unique"),
                    path,
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
                    context,
                    concept,
                    diagnostics,
                    DiagCode::UnmatchedReturn,
                    "return has no eligible preceding call",
                    path,
                    &edges[index].id,
                );
                None
            }
            _ => {
                report_message(
                    context,
                    concept,
                    diagnostics,
                    DiagCode::AmbiguousReturn,
                    "return matches more than one preceding call",
                    path,
                    &edges[index].id,
                );
                None
            }
        }
    };
    let Some(candidate) = selected else { return };
    if !open.contains(&candidate) {
        report_message(
            context,
            concept,
            diagnostics,
            DiagCode::CompletedReturn,
            "call already has an explicit return",
            path,
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
            context,
            concept,
            diagnostics,
            DiagCode::ConflictingReturn,
            "return endpoints conflict with the selected call",
            path,
            &edges[index].id,
        );
        return;
    }
    edges[index].to = Some(edges[candidate].from.clone());
    edges[index].returns_call = Some(edges[candidate].id.clone());
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
    let mut declared_fragments = concept
        .fragments
        .iter()
        .filter(|fragment| value(&fragment.kind).is_some());
    for node in nodes {
        let SeqNode::Fragment { kind, operands, .. } = node else {
            continue;
        };
        let declared_fragment = declared_fragments
            .next()
            .expect("each runtime fragment has a typed declared fragment");
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
    #[allow(clippy::too_many_arguments)]
    fn repeated_deletes(
        items: &[SeqChild],
        nodes: &BTreeMap<String, &SeqNode>,
        edges: &BTreeMap<MessageId, &SeqEdge>,
        deleted: &mut BTreeSet<String>,
        repeated: &mut BTreeSet<MessageId>,
        context: &DomainAnalysisContext<'_>,
        concept: &DeclaredConcept,
        diagnostics: &mut Vec<Diagnostic>,
        path: &str,
    ) {
        for item in items {
            match item {
                SeqChild::Message { edge } => {
                    let Some(SeqEdge {
                        kind: MessageKind::Delete,
                        to: Some(EndpointRef::Lifeline { id }),
                        ..
                    }) = edges.get(edge).copied()
                    else {
                        continue;
                    };
                    if !deleted.insert(id.clone()) {
                        repeated.insert(edge.clone());
                        report_message(
                            context,
                            concept,
                            diagnostics,
                            DiagCode::InvalidLifelineLifetime,
                            "lifeline is created or deleted more than once",
                            path,
                            edge,
                        );
                    }
                }
                SeqChild::InteractionUse { .. } => {}
                SeqChild::Fragment { node } => {
                    let Some(SeqNode::Fragment { kind, operands, .. }) = nodes.get(node).copied()
                    else {
                        continue;
                    };
                    let incoming = deleted.clone();
                    if *kind == FragmentKind::Par {
                        for operand in operands {
                            let Some(SeqNode::Operand { items, .. }) = nodes.get(operand).copied()
                            else {
                                continue;
                            };
                            repeated_deletes(
                                items,
                                nodes,
                                edges,
                                deleted,
                                repeated,
                                context,
                                concept,
                                diagnostics,
                                path,
                            );
                        }
                        continue;
                    }
                    let mut outcomes = Vec::new();
                    for operand in operands {
                        let Some(SeqNode::Operand { items, .. }) = nodes.get(operand).copied()
                        else {
                            continue;
                        };
                        let mut branch = incoming.clone();
                        repeated_deletes(
                            items,
                            nodes,
                            edges,
                            &mut branch,
                            repeated,
                            context,
                            concept,
                            diagnostics,
                            path,
                        );
                        outcomes.push(branch);
                    }
                    let has_else = operands.iter().any(|operand| {
                        matches!(
                            nodes.get(operand).copied(),
                            Some(SeqNode::Operand {
                                spec: OperandSpec::Else,
                                ..
                            })
                        )
                    });
                    if matches!(kind, FragmentKind::Alt) && !has_else
                        || matches!(
                            kind,
                            FragmentKind::Opt | FragmentKind::Loop | FragmentKind::Break
                        )
                    {
                        outcomes.push(incoming);
                    }
                    if let Some(first) = outcomes.first().cloned() {
                        *deleted = outcomes.iter().skip(1).fold(first, |mut joined, branch| {
                            joined.retain(|id| branch.contains(id));
                            joined
                        });
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        items: &[SeqChild],
        nodes: &BTreeMap<String, &SeqNode>,
        edges: &BTreeMap<MessageId, &SeqEdge>,
        alive: &mut BTreeSet<String>,
        repeated_deletes: &BTreeSet<MessageId>,
        context: &DomainAnalysisContext<'_>,
        concept: &DeclaredConcept,
        diagnostics: &mut Vec<Diagnostic>,
        path: &str,
    ) {
        fn created_in(
            items: &[SeqChild],
            nodes: &BTreeMap<String, &SeqNode>,
            edges: &BTreeMap<MessageId, &SeqEdge>,
            out: &mut BTreeSet<String>,
        ) {
            for item in items {
                match item {
                    SeqChild::Message { edge } => {
                        if let Some(SeqEdge {
                            kind: MessageKind::Create,
                            to: Some(EndpointRef::Lifeline { id }),
                            ..
                        }) = edges.get(edge).copied()
                        {
                            out.insert(id.clone());
                        }
                    }
                    SeqChild::Fragment { node } => {
                        if let Some(SeqNode::Fragment { operands, .. }) = nodes.get(node).copied() {
                            for operand in operands {
                                if let Some(SeqNode::Operand { items, .. }) =
                                    nodes.get(operand).copied()
                                {
                                    created_in(items, nodes, edges, out);
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
                    let Some(message) = edges.get(edge).copied() else {
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
                            context,
                            concept,
                            diagnostics,
                            DiagCode::InvalidLifelineLifetime,
                            format!("lifeline '{id}' is used outside its lifetime"),
                            path,
                            edge,
                        );
                    }
                    if let Some(EndpointRef::Lifeline { id }) = &message.to {
                        match message.kind {
                            MessageKind::Create => {
                                if alive.contains(id) {
                                    report_message(
                                        context,
                                        concept,
                                        diagnostics,
                                        DiagCode::InvalidLifelineLifetime,
                                        "lifeline is created or deleted more than once",
                                        path,
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
                    let Some(SeqNode::Fragment { kind, operands, .. }) = nodes.get(node).copied()
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
                                nodes.get(operand).copied()
                            {
                                created_in(items, nodes, edges, &mut creates);
                            }
                            creates
                        })
                        .collect::<Vec<_>>();
                    for (operand_index, operand) in operands.iter().enumerate() {
                        let Some(SeqNode::Operand { items, .. }) = nodes.get(operand).copied()
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
                            nodes,
                            edges,
                            &mut branch,
                            repeated_deletes,
                            context,
                            concept,
                            diagnostics,
                            path,
                        );
                        outcomes.push(branch);
                    }
                    let has_else = operands.iter().any(|operand| {
                        matches!(
                            nodes.get(operand).copied(),
                            Some(SeqNode::Operand {
                                spec: OperandSpec::Else,
                                ..
                            })
                        )
                    });
                    if matches!(kind, FragmentKind::Alt) && !has_else
                        || matches!(
                            kind,
                            FragmentKind::Opt | FragmentKind::Loop | FragmentKind::Break
                        )
                    {
                        outcomes.push(incoming.clone());
                    }
                    if let Some(first) = outcomes.first().cloned() {
                        *alive = outcomes.iter().skip(1).fold(first, |mut joined, branch| {
                            joined.retain(|id| branch.contains(id));
                            joined
                        });
                    }
                }
            }
        }
    }
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
                context,
                concept,
                diagnostics,
                DiagCode::InvalidSequenceEndpoint,
                "create and delete targets must be local lifelines",
                path,
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
        .map(|edge| (edge.id.clone(), edge))
        .collect::<BTreeMap<_, _>>();
    let mut repeated_delete_messages = BTreeSet::new();
    repeated_deletes(
        items,
        &node_by_id,
        &edge_by_id,
        &mut BTreeSet::new(),
        &mut repeated_delete_messages,
        context,
        concept,
        diagnostics,
        path,
    );
    walk(
        items,
        &node_by_id,
        &edge_by_id,
        &mut alive,
        &repeated_delete_messages,
        context,
        concept,
        diagnostics,
        path,
    );
}
