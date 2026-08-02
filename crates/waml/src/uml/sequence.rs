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

fn report(
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagCode,
    message: impl Into<String>,
    path: &str,
) {
    diagnostics.push(Diagnostic::new(code, message, path, 1));
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
    fn resolve(&mut self, endpoint: &DeclaredEndpointRef) -> EndpointRef {
        match endpoint {
            DeclaredEndpointRef::Lifeline(id) => {
                if !self.lifelines.contains(id) {
                    report(
                        self.diagnostics,
                        DiagCode::UnknownSequenceEndpoint,
                        format!("unknown sequence lifeline '{id}'"),
                        self.path,
                    );
                }
                EndpointRef::Lifeline { id: id.clone() }
            }
            DeclaredEndpointRef::Outside => EndpointRef::Outside,
            DeclaredEndpointRef::LocalGate(gate) => {
                if !self.gates.contains(gate) {
                    report(
                        self.diagnostics,
                        DiagCode::UnknownSequenceEndpoint,
                        format!("unknown local gate '{gate}'"),
                        self.path,
                    );
                }
                EndpointRef::LocalGate { gate: gate.clone() }
            }
            DeclaredEndpointRef::UseGate { use_alias, gate } => {
                let Some(&index) = self.uses.get(use_alias) else {
                    report(
                        self.diagnostics,
                        DiagCode::UnknownSequenceEndpoint,
                        format!("unknown interaction-use alias '{use_alias}'"),
                        self.path,
                    );
                    return EndpointRef::UseGate {
                        interaction_use: InteractionUseId(use_alias.clone()),
                        gate: gate.clone(),
                    };
                };
                let target = &self.interaction_uses[index].target;
                if !self
                    .target_gates
                    .get(target)
                    .is_some_and(|gates| gates.contains(gate))
                {
                    report(
                        self.diagnostics,
                        DiagCode::InvalidInteractionUse,
                        format!("interaction use '{use_alias}' has no gate '{gate}'"),
                        self.path,
                    );
                } else if !self
                    .target_connected_gates
                    .get(target)
                    .is_some_and(|gates| gates.contains(gate))
                {
                    report(
                        self.diagnostics,
                        DiagCode::InvalidInteractionUse,
                        format!(
                            "interaction use '{use_alias}' gate '{gate}' has no inner connection"
                        ),
                        self.path,
                    );
                } else if !self.interaction_uses[index].gates.contains(gate) {
                    self.interaction_uses[index].gates.push(gate.clone());
                }
                EndpointRef::UseGate {
                    interaction_use: self.interaction_uses[index].id.clone(),
                    gate: gate.clone(),
                }
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

fn interaction_use_enters_cycle(
    context: &DomainAnalysisContext<'_>,
    declared: &DeclaredBundle,
    origin: &str,
    target: &str,
) -> bool {
    fn reaches(
        context: &DomainAnalysisContext<'_>,
        declared: &DeclaredBundle,
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
        let Some(concept) = declared.concept(current) else {
            return false;
        };
        let Some(path) = path_for_concept(context, current) else {
            return false;
        };
        concept.interaction_uses.iter().any(|interaction_use| {
            value(&interaction_use.link).is_some_and(|link| {
                let next = crate::okf::resolve_href(&path, link);
                reaches(context, declared, &next, goal, visited)
            })
        })
    }
    reaches(context, declared, target, origin, &mut BTreeSet::new())
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
        if id == "outside" || id.contains('@') {
            report(
                diagnostics,
                DiagCode::ReservedSequenceName,
                format!("reserved sequence lifeline name '{id}'"),
                path,
            );
        }
        if !lifelines.insert(id.clone()) {
            report(
                diagnostics,
                DiagCode::DuplicateSequenceName,
                format!("duplicate sequence lifeline name '{id}'"),
                path,
            );
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
    for gate in concept.gates.iter().filter_map(|gate| value(&gate.name)) {
        if !gate_names.insert(gate.clone()) {
            report(
                diagnostics,
                DiagCode::DuplicateGate,
                format!("duplicate gate '{gate}'"),
                path,
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
                    .filter_map(|gate| value(&gate.name).cloned())
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
                        connected.insert(gate.clone());
                    }
                }
            }
            (target.concept_id.clone(), connected)
        })
        .collect::<BTreeMap<_, _>>();

    let mut interaction_uses = Vec::new();
    let mut use_aliases = BTreeMap::new();
    let mut authored_use_aliases = BTreeSet::new();
    for declared_use in concept.interaction_uses.iter() {
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
            report(
                diagnostics,
                DiagCode::InvalidInteractionUse,
                format!("unresolved sequence interaction use '{link}'"),
                path,
            );
        }
        if lifelines.contains(alias) || !authored_use_aliases.insert(alias.clone()) {
            report(
                diagnostics,
                DiagCode::DuplicateSequenceName,
                format!("duplicate sequence interaction-use alias '{alias}'"),
                path,
            );
            valid_use = false;
        }
        let bindings = declared_use
            .bindings
            .iter()
            .filter_map(|binding| {
                Some(SeqBinding {
                    local: value(&binding.local)?.clone(),
                    target: value(&binding.target)?.clone(),
                })
            })
            .collect::<Vec<_>>();
        if let Some(target_concept) = target_concept {
            let target_path = path_for_concept(context, &target).unwrap_or_else(|| target.clone());
            let target_lifelines = lifeline_classifier_map(target_concept, &target_path);
            let participating = participating_lifelines(target_concept);
            let mut locals = BTreeSet::new();
            let mut targets = BTreeSet::new();
            for binding in &bindings {
                if !locals.insert(binding.local.clone()) || !targets.insert(binding.target.clone())
                {
                    report(
                        diagnostics,
                        DiagCode::InvalidInteractionUse,
                        format!("interaction use '{alias}' has duplicate bindings"),
                        path,
                    );
                    valid_use = false;
                }
                if !lifelines.contains(&binding.local)
                    || !target_lifelines.contains_key(&binding.target)
                {
                    report(
                        diagnostics,
                        DiagCode::InvalidInteractionUse,
                        format!("interaction use '{alias}' has an unknown binding endpoint"),
                        path,
                    );
                    valid_use = false;
                }
                if let (Some(local_classifier), Some(target_classifier)) = (
                    lifeline_classifiers.get(&binding.local),
                    target_lifelines.get(&binding.target),
                ) {
                    if local_classifier != target_classifier {
                        report(
                            diagnostics,
                            DiagCode::InvalidInteractionUse,
                            format!("interaction use '{alias}' binds different classifiers"),
                            path,
                        );
                        valid_use = false;
                    }
                }
            }
            if participating.iter().any(|handle| !targets.contains(handle)) {
                report(
                    diagnostics,
                    DiagCode::InvalidInteractionUse,
                    format!(
                        "interaction use '{alias}' is missing a participating lifeline binding"
                    ),
                    path,
                );
                valid_use = false;
            }
        }
        if valid_use
            && interaction_use_enters_cycle(context, declared, &concept.concept_id, &target)
        {
            report(
                diagnostics,
                DiagCode::InteractionUseCycle,
                format!("interaction use '{alias}' enters a reference cycle"),
                path,
            );
            valid_use = false;
        }
        if !valid_use {
            continue;
        }
        let id = InteractionUseId(format!("u{}", interaction_uses.len()));
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
    let mut fragment_count = 0usize;

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
                let id = format!("f{fragment_count}");
                fragment_count += 1;
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
                let id = match &nodes[fragment_index] {
                    SeqNode::Fragment { id, operands, .. } => format!("{id}.o{}", operands.len()),
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
                let from = endpoints.resolve(source);
                let kind = message_kind(*kind);
                let authored_target = if kind == MessageKind::Reply {
                    value(&message.return_to)
                } else {
                    value(&message.target)
                };
                let to = authored_target.map(|target| endpoints.resolve(target));
                if matches!(from, EndpointRef::Outside) && matches!(to, Some(EndpointRef::Outside))
                {
                    report(
                        endpoints.diagnostics,
                        DiagCode::InvalidSequenceEndpoint,
                        "outside cannot be both message endpoints",
                        path,
                    );
                }
                let id = MessageId(format!("m{}", edges.len()));
                edges.push(SeqEdge {
                    id: id.clone(),
                    from,
                    kind,
                    to,
                    value: value(&message.value).cloned(),
                    call_id: if kind == MessageKind::Reply {
                        value(&message.return_for).cloned()
                    } else {
                        value(&message.call_id).cloned()
                    },
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
    drop(endpoints);

    resolve_returns(&mut edges, &nodes, &root, diagnostics, path);
    validate_fragments(context, concept, &nodes, diagnostics, path);
    validate_lifetimes(&edges, diagnostics, path);

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
            report(
                diagnostics,
                DiagCode::DuplicateCallIdentity,
                format!("duplicate call identity '{call_id}'"),
                path,
            );
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
        diagnostics,
        path,
    );
}

fn walk_return_items(
    items: &[SeqChild],
    open: &mut BTreeSet<usize>,
    edges: &mut [SeqEdge],
    edge_by_id: &BTreeMap<MessageId, usize>,
    node_by_id: &BTreeMap<String, &SeqNode>,
    call_ids: &BTreeMap<String, Vec<usize>>,
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
                    MessageKind::Reply => {
                        resolve_one_return(index, open, edges, call_ids, diagnostics, path)
                    }
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
                            diagnostics,
                            path,
                        );
                    }
                    joined.extend(branch);
                }
                *open = joined;
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
    diagnostics: &mut Vec<Diagnostic>,
    path: &str,
) {
    let authored_to = edges[index].to.clone();
    let selected = if let Some(authored_for) = edges[index].call_id.clone() {
        match call_ids
            .get(&authored_for)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            [candidate] if *candidate < index => Some(*candidate),
            [] => {
                report(
                    diagnostics,
                    DiagCode::UnknownCallIdentity,
                    format!("unknown call identity '{authored_for}'"),
                    path,
                );
                None
            }
            [_] => {
                report(
                    diagnostics,
                    DiagCode::UnknownCallIdentity,
                    format!("call identity '{authored_for}' does not precede the return"),
                    path,
                );
                None
            }
            _ => {
                report(
                    diagnostics,
                    DiagCode::AmbiguousReturn,
                    format!("call identity '{authored_for}' is not unique"),
                    path,
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
                    && authored_to
                        .as_ref()
                        .is_none_or(|to| to == &edges[*candidate].from)
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [candidate] => Some(*candidate),
            [] => {
                report(
                    diagnostics,
                    DiagCode::UnmatchedReturn,
                    "return has no eligible preceding call",
                    path,
                );
                None
            }
            _ => {
                report(
                    diagnostics,
                    DiagCode::AmbiguousReturn,
                    "return matches more than one preceding call",
                    path,
                );
                None
            }
        }
    };
    let Some(candidate) = selected else { return };
    if !open.contains(&candidate) {
        report(
            diagnostics,
            DiagCode::CompletedReturn,
            "call already has an explicit return",
            path,
        );
        return;
    }
    let source_matches = edges[candidate].to.as_ref() == Some(&edges[index].from);
    let to_matches = authored_to
        .as_ref()
        .is_none_or(|to| to == &edges[candidate].from);
    if !source_matches || !to_matches {
        report(
            diagnostics,
            DiagCode::ConflictingReturn,
            "return endpoints conflict with the selected call",
            path,
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

fn validate_lifetimes(edges: &[SeqEdge], diagnostics: &mut Vec<Diagnostic>, path: &str) {
    let mut creates: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut deletes: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, edge) in edges.iter().enumerate() {
        let target = edge.to.as_ref();
        if matches!(edge.kind, MessageKind::Create | MessageKind::Delete)
            && !matches!(target, Some(EndpointRef::Lifeline { .. }))
        {
            report(
                diagnostics,
                DiagCode::InvalidSequenceEndpoint,
                "create and delete targets must be local lifelines",
                path,
            );
            continue;
        }
        if let Some(EndpointRef::Lifeline { id }) = target {
            match edge.kind {
                MessageKind::Create => creates.entry(id.clone()).or_default().push(index),
                MessageKind::Delete => deletes.entry(id.clone()).or_default().push(index),
                _ => {}
            }
        }
    }
    for positions in creates.values().chain(deletes.values()) {
        if positions.len() > 1 {
            report(
                diagnostics,
                DiagCode::InvalidLifelineLifetime,
                "lifeline is created or deleted more than once",
                path,
            );
        }
    }
    for (index, edge) in edges.iter().enumerate() {
        for endpoint in std::iter::once(&edge.from).chain(edge.to.iter()) {
            let EndpointRef::Lifeline { id } = endpoint else {
                continue;
            };
            if creates
                .get(id)
                .and_then(|positions| positions.first())
                .is_some_and(|created| index < *created)
                || deletes
                    .get(id)
                    .and_then(|positions| positions.first())
                    .is_some_and(|deleted| index > *deleted)
            {
                report(
                    diagnostics,
                    DiagCode::InvalidLifelineLifetime,
                    format!("lifeline '{id}' is used outside its lifetime"),
                    path,
                );
            }
        }
    }
}
