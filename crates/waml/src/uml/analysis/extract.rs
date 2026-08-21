//! Extraction: one `declared_*` function per UML syntax node, lowering a
//! parse tree into the `DeclaredConcept` family. No rule lives here — a
//! malformed field becomes `Invalid`/`Incomplete` and the verdict is left
//! to validation and projection.

use super::syntax_util::{
    behavior_depth, declared_expression_slot, declared_optional_expression_slot,
    declared_required_token, declared_text_slot, direct_child, field_from_token, field_value,
    first_recovery_node, has_direct_recovery, has_missing_kind, has_recovery, invalid,
    invalid_recovery, items, link_parts, strip_expression, valid, MapDeclaredField,
};
use crate::analysis::DomainAnalysisContext;
use crate::uml::{
    declared::DeclaredAttribute,
    syntax::{self, UmlLanguage},
};
use std::sync::Arc;
use waml_syntax::{AstNode, SyntaxElement, SyntaxNode, SyntaxToken};

pub(crate) fn declared_attribute(
    context: &DomainAnalysisContext<'_>,
    document: &crate::analysis::DocumentVersion,
    syntax: syntax::AttributeSyntax,
) -> DeclaredAttribute {
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
                    let resolved = crate::okf::resolve_href(document.path().as_str(), &href);
                    (
                        label.text().write_to_string(),
                        context
                            .okf
                            .concept(&resolved)
                            .filter(|target| crate::uml::recognizes(target))
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
                    diagnostics: Arc::from([crate::diagnostic::DiagCode::MalformedAttribute]),
                },
            }
        }
        None => crate::uml::DeclaredField::Absent,
    };
    DeclaredAttribute {
        syntax,
        visibility,
        name: name_field,
        ty: ty_field,
        multiplicity: mult_field,
    }
}

pub(crate) fn declared_flow_node(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredFlowNode {
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
    let traces = syntax
        .traces()
        .map(|trace| {
            let link = trace.link();
            let field = |kind, expected| {
                link.as_ref()
                    .and_then(|link| {
                        field_from_token(link, kind).map(|value| valid(link.clone(), value))
                    })
                    .unwrap_or_else(|| crate::uml::DeclaredField::Incomplete {
                        syntax: trace.0.clone(),
                        expected,
                    })
            };
            crate::uml::DeclaredFlowTrace {
                syntax: trace.clone(),
                label: field(
                    syntax::UmlSyntaxKind::LinkTextToken,
                    crate::uml::ExpectedSyntax::LinkTarget,
                ),
                href: field(
                    syntax::UmlSyntaxKind::LinkTargetToken,
                    crate::uml::ExpectedSyntax::LinkTarget,
                ),
            }
        })
        .collect::<Vec<_>>();
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
        traces: traces.into(),
    }
}

pub(crate) fn declared_lifeline(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredLifeline {
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
                            expected: crate::uml::declared::expected_for_slot(
                                syntax::UmlSyntaxKind::LifelineAlias,
                            ),
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

pub(crate) fn declared_message(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredMessage {
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
        crate::uml::declared::expected_for_slot(slot_kind),
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

pub(crate) fn declared_gate(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredGate {
    let syntax = syntax::GateSyntax(node.clone());
    let name = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::GateName,
        syntax::UmlSyntaxKind::IdentifierToken,
        crate::uml::declared::expected_for_slot(syntax::UmlSyntaxKind::GateName),
    );
    crate::uml::DeclaredGate { syntax, name }
}

pub(crate) fn declared_fragment(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredFragment {
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

pub(crate) fn declared_operand(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredOperand {
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

fn declared_binding(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredBinding {
    let syntax = syntax::BindingSyntax(node.clone());
    let local = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::BindingLocal,
        syntax::UmlSyntaxKind::LocalToken,
        crate::uml::declared::expected_for_slot(syntax::UmlSyntaxKind::BindingLocal),
    );
    let target = declared_required_token(
        &node,
        syntax::UmlSyntaxKind::BindingTarget,
        syntax::UmlSyntaxKind::TargetToken,
        crate::uml::declared::expected_for_slot(syntax::UmlSyntaxKind::BindingTarget),
    );
    crate::uml::DeclaredBinding {
        syntax,
        local,
        target,
    }
}

pub(crate) fn declared_interaction_use(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredInteractionUse {
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
        crate::uml::declared::expected_for_slot(syntax::UmlSyntaxKind::InteractionUseAlias),
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

pub(crate) fn declared_value(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredValue {
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

pub(crate) fn declared_slot(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredSlot {
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
    let name = field(syntax.name_token(), crate::uml::ExpectedSyntax::SlotName);
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
    let mut value = field(syntax.value_token(), crate::uml::ExpectedSyntax::SlotValue);
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

pub(crate) fn declared_relationship(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredRelationship {
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
        if raw.trim().is_empty() {
            // No multiplicity text at all (`: 1 to`): the end was expected but
            // never written. That is a missing end, not an unreadable one —
            // `Invalid` is reserved for text the grammar could not read (`0`,
            // an inverted range), so the two produce different diagnostics.
            return crate::uml::DeclaredField::Incomplete {
                syntax: end.0,
                expected: crate::uml::ExpectedSyntax::ValidMultiplicity,
            };
        }
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

pub(crate) fn declared_layout(
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

pub(crate) fn declared_member(node: SyntaxNode<UmlLanguage>) -> crate::uml::DeclaredMember {
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

pub(crate) fn declared_member_group(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredMemberGroup {
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

pub(crate) fn declared_inline_instance(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredInlineInstance {
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
