//! Syntax-tree navigation and `DeclaredField` construction: the leaf
//! primitives every other phase of the UML analysis is built from.

use crate::uml::syntax::{self, UmlLanguage};
use std::sync::Arc;
use waml_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange, TextSize};

pub(crate) fn trimmed_token_range(token: &SyntaxToken<UmlLanguage>) -> TextRange {
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

pub(crate) fn field_value<T>(field: &crate::uml::DeclaredField<UmlLanguage, T>) -> Option<&T> {
    match field {
        crate::uml::DeclaredField::Valid { value, .. } => Some(value),
        _ => None,
    }
}

/// [`items`] for several kinds at once, in a single traversal.
///
/// `analyze` needs fifteen different node kinds out of every document's UML
/// tree, and it needed them badly enough to walk the whole tree fifteen times
/// per document per keystroke, over every document in the bundle. This walks
/// it once.
///
/// The buckets are exactly what the separate calls produced. `items` stops
/// descending into a node once that node matches the kind it is looking for,
/// but keeps descending for every *other* kind, so the fused walk carries a
/// per-kind `seeking` flag down each branch and only suppresses the branch for
/// the one kind that matched. Order is the same pre-order in both.
pub(crate) fn items_by_kind<const N: usize>(
    node: &SyntaxNode<UmlLanguage>,
    kinds: [syntax::UmlSyntaxKind; N],
) -> [Vec<SyntaxNode<UmlLanguage>>; N] {
    let mut found = std::array::from_fn(|_| Vec::new());
    let mut seeking = [true; N];
    collect_by_kind(node, &kinds, &mut seeking, &mut found);
    found
}

fn collect_by_kind<const N: usize>(
    node: &SyntaxNode<UmlLanguage>,
    kinds: &[syntax::UmlSyntaxKind; N],
    seeking: &mut [bool; N],
    found: &mut [Vec<SyntaxNode<UmlLanguage>>; N],
) {
    if !seeking.iter().any(|active| *active) {
        return;
    }
    for child in node.children() {
        let SyntaxElement::Node(child) = child else {
            continue;
        };
        // Kinds are distinct, so at most one bucket can claim a node.
        let matched = kinds
            .iter()
            .position(|kind| *kind == child.kind())
            .filter(|index| seeking[*index]);
        if let Some(index) = matched {
            found[index].push(child.clone());
            seeking[index] = false;
            collect_by_kind(&child, kinds, seeking, found);
            seeking[index] = true;
        } else {
            collect_by_kind(&child, kinds, seeking, found);
        }
    }
}

pub(crate) fn items(
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

pub(crate) fn direct_section_items(
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

pub(crate) fn valid<T>(
    node: SyntaxNode<UmlLanguage>,
    value: T,
) -> crate::uml::DeclaredField<UmlLanguage, T> {
    crate::uml::DeclaredField::Valid {
        value,
        syntax: node,
    }
}

pub(crate) fn invalid<T>(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredField<UmlLanguage, T> {
    crate::uml::DeclaredField::Invalid {
        syntax: node,
        diagnostics: Arc::from([crate::diagnostic::DiagCode::DroppableContent]),
    }
}

pub(crate) fn has_recovery(node: &SyntaxNode<UmlLanguage>) -> bool {
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

pub(crate) fn first_recovery_node(
    node: &SyntaxNode<UmlLanguage>,
) -> Option<SyntaxNode<UmlLanguage>> {
    node.children().find_map(|element| {
        let child = element.into_node()?;
        if child.kind() == syntax::UmlSyntaxKind::SkippedTokensSyntax {
            Some(child)
        } else {
            first_recovery_node(&child)
        }
    })
}

pub(crate) fn has_missing_kind(
    node: &SyntaxNode<UmlLanguage>,
    kind: syntax::UmlSyntaxKind,
) -> bool {
    node.children().any(|e| {
        e.kind() == kind
            && e.into_token()
                .is_some_and(|token| token.flags().is_missing())
    })
}

pub(crate) fn direct_child(
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

pub(crate) fn field_from_token(
    node: &SyntaxNode<UmlLanguage>,
    kind: syntax::UmlSyntaxKind,
) -> Option<String> {
    token_in(node, kind)
        .filter(|token| !token.flags().is_missing())
        .map(|token| token.text().write_to_string())
}

pub(crate) fn strip_expression(value: String) -> String {
    value
        .strip_prefix('`')
        .and_then(|value| value.strip_suffix('`'))
        .unwrap_or(&value)
        .to_string()
}

pub(crate) fn declared_expression_slot(
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

pub(crate) fn declared_optional_expression_slot(
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

pub(crate) fn declared_text_slot(
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

pub(crate) fn declared_required_token(
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

pub(crate) fn link_parts(node: &SyntaxNode<UmlLanguage>) -> Option<(String, String)> {
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

pub(crate) fn behavior_depth(node: &SyntaxNode<UmlLanguage>) -> usize {
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

pub(crate) fn invalid_recovery<T>(
    node: SyntaxNode<UmlLanguage>,
) -> crate::uml::DeclaredField<UmlLanguage, T> {
    invalid(first_recovery_node(&node).unwrap_or(node))
}

pub(crate) fn has_direct_recovery(node: &SyntaxNode<UmlLanguage>) -> bool {
    node.children()
        .filter_map(SyntaxElement::into_node)
        .any(|child| {
            child.kind() == syntax::UmlSyntaxKind::BehaviorRecovery && has_recovery(&child)
        })
}

pub(crate) trait MapDeclaredField<L: waml_syntax::SyntaxLanguage, T> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{analysis::prepare_candidate, source::SourceBundle};

    /// Exactly the kinds `analyze` fuses into one traversal.
    const FUSED: [syntax::UmlSyntaxKind; 14] = [
        syntax::UmlSyntaxKind::Attribute,
        syntax::UmlSyntaxKind::Value,
        syntax::UmlSyntaxKind::Slot,
        syntax::UmlSyntaxKind::Relationship,
        syntax::UmlSyntaxKind::Member,
        syntax::UmlSyntaxKind::InlineInstance,
        syntax::UmlSyntaxKind::LayoutStatement,
        syntax::UmlSyntaxKind::FlowNode,
        syntax::UmlSyntaxKind::Lifeline,
        syntax::UmlSyntaxKind::Gate,
        syntax::UmlSyntaxKind::Message,
        syntax::UmlSyntaxKind::SequenceOperand,
        syntax::UmlSyntaxKind::SequenceFragment,
        syntax::UmlSyntaxKind::InteractionUse,
    ];

    const FIXTURES: [(&str, &str); 8] = [
        (
            "class.md",
            include_str!("../../../tests/fixtures/parser-platform/class.md"),
        ),
        (
            "object.md",
            include_str!("../../../tests/fixtures/parser-platform/object.md"),
        ),
        (
            "enum.md",
            include_str!("../../../tests/fixtures/parser-platform/enum.md"),
        ),
        (
            "diagram.md",
            include_str!("../../../tests/fixtures/parser-platform/diagram.md"),
        ),
        (
            "sequence.md",
            include_str!("../../../tests/fixtures/parser-platform/sequence.md"),
        ),
        (
            "activity.md",
            include_str!("../../../tests/fixtures/parser-platform/activity.md"),
        ),
        (
            "state-machine.md",
            include_str!("../../../tests/fixtures/parser-platform/state-machine.md"),
        ),
        // Deliberately included: recovery trees are where a fused walk is most
        // likely to diverge from fourteen separate ones.
        (
            "recovery-sequence.md",
            include_str!("../../../tests/fixtures/parser-platform/recovery/sequence.md"),
        ),
    ];

    /// The fused walk is only a speedup if it is also the same answer.
    ///
    /// `items` stops descending at a node of the kind it wants but keeps
    /// descending for every other kind, so a naive "stop at the first match"
    /// fusion would silently drop nested declarations. This pins the buckets
    /// node-for-node, in order, against one `items` call per kind.
    #[test]
    fn fused_kind_walk_matches_one_walk_per_kind() {
        let candidate =
            prepare_candidate(SourceBundle::try_from_pairs(FIXTURES).unwrap(), None, 1).unwrap();
        let mut visited = 0_usize;
        let mut collected = 0_usize;
        for document in candidate.source().documents() {
            let Some(id) = candidate.okf().catalog.id_for_path(document.path()) else {
                continue;
            };
            let Some(snapshot) = candidate.uml().syntax.document(id) else {
                continue;
            };
            visited += 1;
            let root = snapshot.syntax().root();
            let fused = items_by_kind(&root, FUSED);
            for (index, kind) in FUSED.iter().enumerate() {
                let separate = items(root.clone(), *kind);
                collected += separate.len();
                assert_eq!(
                    fused[index].len(),
                    separate.len(),
                    "{kind:?} count differs in {}",
                    document.path()
                );
                for (left, right) in fused[index].iter().zip(separate.iter()) {
                    assert_eq!(
                        left.range(),
                        right.range(),
                        "{kind:?} node order differs in {}",
                        document.path()
                    );
                }
            }
        }
        assert_eq!(visited, FIXTURES.len(), "every fixture is a UML document");
        assert!(collected > 0, "the fixtures declare something to find");
    }
}
