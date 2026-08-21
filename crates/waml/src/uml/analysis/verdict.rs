//! Rules with more than one consumer, per the ownership rule in the parent
//! module: each verdict is derived once here and read by both the admission
//! (projection) side and the diagnostic (validation) side.

use crate::uml::syntax::UmlLanguage;
use std::collections::BTreeSet;
use waml_syntax::AstNode;

/// Verdict for a relationship's `: <near> to <far>` ends, shared by admission
/// (`declared_projection`, which drops on non-`Ok`) and diagnostics
/// (`validate_declared_semantics`, which reports on non-`Ok`). Neither
/// re-derives the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EndVerdict {
    /// Ends match what `kind` requires: both present when ended, both absent
    /// when unended (or, for `associates`, absent on either side is also
    /// tolerated — the classifier-vs-communication-link distinction is a
    /// diagnostic-only concern layered on top, not an admission rule).
    Ok,
    /// A kind that takes ends was declared with neither end usable: an ended
    /// kind (`aggregates`/`composes`) with no ends at all, or any such kind
    /// whose ends were authored (a colon is present) but left blank.
    EndsRequired,
    /// A non-ended kind declared with at least one end.
    EndsForbidden,
    /// Exactly one of the two ends is present.
    OneEnded,
    /// A kind that takes ends was declared with an end the grammar could not
    /// parse (`Invalid`/`Incomplete`, e.g. `0` or an inverted range). The end
    /// was authored, so it must not be read as "no end given" and silently
    /// dropped into an end-less relationship.
    EndsUnparsable,
}

pub(crate) fn relationship_end_verdict(
    kind: crate::model::RelationshipKind,
    from_end: &crate::uml::DeclaredField<UmlLanguage, crate::model::RelEnd>,
    to_end: &crate::uml::DeclaredField<UmlLanguage, crate::model::RelEnd>,
) -> EndVerdict {
    // An end is "given" only when it fully parsed to `Valid`, and "not given"
    // only when no end text was authored at all — `Absent`. `Incomplete` (a
    // colon with blank multiplicity) and `Invalid` (text the grammar could not
    // read: `0`, an inverted range) are both authored ends. Neither may be
    // folded into the not-given case, which would admit a malformed
    // relationship with its ends silently discarded.
    let present = |end: &crate::uml::DeclaredField<UmlLanguage, crate::model::RelEnd>| {
        matches!(end, crate::uml::DeclaredField::Valid { .. })
    };
    let truly_absent = |end: &crate::uml::DeclaredField<UmlLanguage, crate::model::RelEnd>| {
        matches!(end, crate::uml::DeclaredField::Absent)
    };
    let unparsable = |end: &crate::uml::DeclaredField<UmlLanguage, crate::model::RelEnd>| {
        matches!(end, crate::uml::DeclaredField::Invalid { .. })
    };
    let (from_present, to_present) = (present(from_end), present(to_end));
    let either_unparsable = unparsable(from_end) || unparsable(to_end);
    // An unparsable end outranks the missing-end verdicts: `1 to 0` has one
    // valid and one unreadable end, and reporting it as "only one end" would
    // name the wrong defect — the author gave two ends, one of which the
    // grammar could not read.
    if either_unparsable && (kind == crate::model::RelationshipKind::Associates || kind.is_ended())
    {
        EndVerdict::EndsUnparsable
    } else if kind == crate::model::RelationshipKind::Associates {
        if from_present && to_present {
            EndVerdict::Ok
        } else if from_present || to_present {
            EndVerdict::OneEnded
        } else if truly_absent(from_end) && truly_absent(to_end) {
            // Neither end was authored at all (no colon): an end-less
            // association, which `associates` permits.
            EndVerdict::Ok
        } else {
            // A colon was authored but the multiplicity text is blank
            // (`- associates [Line](./line.md):`). The ends are `Incomplete`,
            // not `Absent` — the author asked for ends and gave none, so this
            // is a malformed relationship, not an end-less association.
            EndVerdict::EndsRequired
        }
    } else if kind.is_ended() {
        // aggregates / composes
        if from_present && to_present {
            EndVerdict::Ok
        } else if from_present || to_present {
            EndVerdict::OneEnded
        } else {
            EndVerdict::EndsRequired
        }
    } else {
        // Non-ended kinds forbid a colon entirely; the lowering pass marks
        // both ends `Invalid` (not `Absent`) when a colon was present but the
        // kind does not take ends, so an authored-but-unparsable end is a
        // forbidden-ends declaration, not an absent one.
        if truly_absent(from_end) && truly_absent(to_end) {
            EndVerdict::Ok
        } else {
            EndVerdict::EndsForbidden
        }
    }
}

pub(crate) struct ValidInlineInstance<'a> {
    pub(crate) name: &'a str,
    pub(crate) target: String,
}

pub(crate) enum InlineInstanceValidity<'a> {
    Invalid,
    Unresolved,
    Valid(ValidInlineInstance<'a>),
}

pub(crate) fn inline_instance_validity<'a>(
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

pub(crate) fn lower_member_group(
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
        role: crate::model::DiagramGroupRole::Generic,
        members,
        children: group
            .children
            .iter()
            .filter_map(|child| lower_member_group(child, path, claimed, owner))
            .collect(),
    })
}
