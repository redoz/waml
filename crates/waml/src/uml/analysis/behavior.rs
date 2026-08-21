//! Behavior lowering: activity/state-machine flows and their transition
//! traces, projected onto the model.

use super::diagnostics::behavior_diagnostic;
use super::link::{resolve_describes, resolve_slug};
use super::syntax_util::field_value;
use crate::uml::syntax;
use crate::{analysis::DomainAnalysisContext, diagnostic::Diagnostic};
use std::collections::{BTreeMap, BTreeSet};
use waml_syntax::SyntaxElement;

fn trace_fragment_exists(
    context: &DomainAnalysisContext<'_>,
    concept_id: &str,
    fragment: &str,
) -> bool {
    let fragment = crate::okf::fragment_slug(fragment);
    let Some((document_id, _)) = context
        .catalog
        .documents()
        .iter()
        .find(|(_, document)| crate::okf::id_of(document.path().as_str()) == concept_id)
    else {
        return false;
    };
    let Some(snapshot) = context.markdown.document(*document_id) else {
        return false;
    };
    let source = snapshot.text().shared();
    snapshot.queries().headings().any(|heading| {
        let start = heading.content_range.start().to_usize();
        let end = heading.content_range.end().to_usize();
        crate::okf::fragment_slug(&source.as_str()[start..end]) == fragment
    })
}

fn resolve_trace_target(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    href: &str,
) -> Result<crate::model::TraceTarget, (crate::diagnostic::DiagCode, String)> {
    if let Some((authored_scheme, _)) = href.split_once(':') {
        let authored_scheme = authored_scheme.to_ascii_lowercase();
        if authored_scheme != "https" {
            return Err((
                crate::diagnostic::DiagCode::UnsupportedTraceScheme,
                format!("unsupported transition trace scheme '{authored_scheme}'"),
            ));
        }
        let Ok(url) = url::Url::parse(href) else {
            return Err((
                crate::diagnostic::DiagCode::MalformedTraceTarget,
                format!("malformed HTTPS transition trace '{href}'"),
            ));
        };
        if url.scheme() != "https" || url.host_str().is_none() {
            return Err((
                crate::diagnostic::DiagCode::MalformedTraceTarget,
                format!("malformed HTTPS transition trace '{href}'"),
            ));
        }
        return Ok(crate::model::TraceTarget::Https {
            url: href.to_string(),
        });
    }
    if href.is_empty() || href.contains('?') {
        return Err((
            crate::diagnostic::DiagCode::MalformedTraceTarget,
            format!("malformed transition trace target '{href}'"),
        ));
    }
    let (document_href, fragment) = match href.split_once('#') {
        Some((_, "")) => {
            return Err((
                crate::diagnostic::DiagCode::MalformedTraceTarget,
                format!("transition trace has an empty fragment '{href}'"),
            ));
        }
        Some((document, fragment)) => (document, Some(fragment)),
        None => (href, None),
    };
    let concept_id = crate::okf::resolve_href(path, document_href);
    if context.okf.concept(&concept_id).is_none() {
        return Err((
            crate::diagnostic::DiagCode::MissingTraceDocument,
            format!("missing transition trace document '{concept_id}'"),
        ));
    }
    if let Some(fragment) = fragment {
        let normalized_fragment = crate::okf::fragment_slug(fragment);
        if !trace_fragment_exists(context, &concept_id, &normalized_fragment) {
            return Err((
                crate::diagnostic::DiagCode::UnresolvedTraceFragment,
                format!("unresolved transition trace fragment '#{fragment}'"),
            ));
        }
        Ok(crate::model::TraceTarget::InternalFragment {
            concept_id,
            fragment: normalized_fragment,
        })
    } else {
        Ok(crate::model::TraceTarget::InternalDocument { concept_id })
    }
}

fn lower_transition_trace(
    context: &DomainAnalysisContext<'_>,
    path: &str,
    trace: &crate::uml::DeclaredFlowTrace,
    diagnostics: &mut Vec<Diagnostic>,
) -> crate::model::TransitionTrace {
    let label = field_value(&trace.label).cloned().unwrap_or_default();
    let href = field_value(&trace.href).cloned().unwrap_or_default();
    let target = match resolve_trace_target(context, path, &href) {
        Ok(target) => target,
        Err((code, message)) => {
            behavior_diagnostic(context, path, &trace.syntax.0, code, message, diagnostics);
            if matches!(
                code,
                crate::diagnostic::DiagCode::MissingTraceDocument
                    | crate::diagnostic::DiagCode::UnresolvedTraceFragment
            ) {
                crate::model::TraceTarget::Unresolved { href: href.clone() }
            } else {
                crate::model::TraceTarget::Invalid { href: href.clone() }
            }
        }
    };
    let range = trace
        .syntax
        .link()
        .and_then(|link| {
            link.children()
                .find(|element| element.kind() == syntax::UmlSyntaxKind::LinkTargetToken)
        })
        .map(|element| match element {
            SyntaxElement::Node(node) => node.range(),
            SyntaxElement::Token(token) => token.range(),
        })
        .unwrap_or_else(|| trace.syntax.0.range());
    crate::model::TransitionTrace {
        label,
        href,
        target,
        source: crate::model::TraceSource {
            path: path.to_string(),
            start: range.start().to_usize(),
            end: range.end().to_usize(),
        },
    }
}

pub(crate) fn lower_flow_behavior(
    context: &DomainAnalysisContext<'_>,
    concept: &crate::uml::DeclaredConcept,
    okf: &crate::okf::Concept,
    path: &str,
    claimed: &BTreeSet<&str>,
    model: &mut crate::model::Model,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let flavor = match crate::model::ElementType::parse(&okf.ty) {
        crate::model::ElementType::Diagram(crate::model::DiagramKind::Activity) => {
            crate::model::FlowFlavor::Activity
        }
        crate::model::ElementType::Diagram(crate::model::DiagramKind::StateMachine) => {
            crate::model::FlowFlavor::StateMachine
        }
        _ => return,
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
        for (source_occurrence, transition) in node.transitions.iter().enumerate() {
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
            let traces = transition
                .traces
                .iter()
                .map(|trace| lower_transition_trace(context, path, trace, diagnostics))
                .collect();
            model.flow_edges.push(crate::model::FlowEdge {
                key: edge_key,
                kind: if carries.is_some() || *kind == crate::model::FlowNodeKind::Object {
                    crate::model::FlowEdgeKind::ObjectFlow
                } else {
                    crate::model::FlowEdgeKind::ControlFlow
                },
                behavior: concept.concept_id.clone(),
                from: format!("{}#{}", concept.concept_id, identity),
                source_occurrence,
                to,
                to_ref,
                trigger: field_value(&transition.trigger).cloned(),
                guard: field_value(&transition.guard).cloned(),
                is_else: transition.is_else,
                effect: field_value(&transition.effect).cloned(),
                carries,
                traces,
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
