//! Accept-set invariant: a candidate a provider offers at a position must be a
//! value the diagnostic guarding that position accepts. Each candidate is
//! applied to the document and the result re-analysed; the guarding diagnostic
//! codes must not appear.

use waml::analysis::{prepare_candidate, PreparedCandidate, TextSize};
use waml::diagnostic::DiagCode;
use waml::source::{BundlePath, SourceBundle};
use waml::uml::{completions, ActionContext};

/// The codes that reject a wrong value at one of the positions the providers
/// answer. A candidate that makes any of these fire is a drift bug.
const GUARDS: &[DiagCode] = &[
    DiagCode::UnknownLifelineHandle,
    DiagCode::UnknownSequenceEndpoint,
    DiagCode::InvalidSequenceEndpoint,
    DiagCode::UnknownFlowTarget,
    DiagCode::UnknownCallIdentity,
    DiagCode::UnresolvedLayoutRef,
    DiagCode::UnresolvedTarget,
    DiagCode::SlotUnknownAttribute,
    // A binding endpoint and an interaction-use link are checked against the
    // *used* interaction, not this document.
    DiagCode::InvalidInteractionUse,
    // A declaration site must not offer a name already declared.
    DiagCode::DuplicateFlowNode,
    // A candidate that satisfies a reference check but wrecks the construct's
    // shape is just as wrong, so the shape codes guard too.
    DiagCode::MalformedLayout,
    DiagCode::MalformedAttribute,
    DiagCode::MalformedMessage,
    DiagCode::MalformedFlowBullet,
];

const SUPPORT: [(&str, &str); 3] = [
    (
        "a.md",
        "---\ntype: uml.Class\ntitle: A\n---\n# A\n\n## Attributes\n\n- status: Number\n",
    ),
    ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
    (
        "inner.md",
        "---\ntype: uml.Sequence\ntitle: Inner\n---\n# Inner\n\n## Lifelines\n\n- [A](./a.md) as customer\n",
    ),
];

fn candidate_for(text: &str, revision: u64) -> PreparedCandidate {
    let mut pairs = vec![("doc.md", text)];
    pairs.extend(SUPPORT);
    prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, revision).unwrap()
}

/// A guarding diagnostic, identified by code *and* message. The message is what
/// keeps an already-incomplete construct from masking a wrong candidate: a
/// half-authored `- A above B with ` already carries `MalformedLayout` for the
/// hint it is missing, so a candidate that swaps that for `MalformedLayout` on
/// an unknown hint is only visible if the reason is compared too.
type Guard = (DiagCode, String);

fn guard_codes(text: &str) -> Vec<Guard> {
    candidate_for(text, 2)
        .uml()
        .diagnostics
        .iter()
        .filter(|diagnostic| GUARDS.contains(&diagnostic.code))
        .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
        .collect()
}

/// A candidate's insert text and the range it replaces.
type CandidateEdit = (String, std::ops::Range<usize>);

/// Every offset in `text` at which a candidate is offered, paired with its
/// candidates. Sweeping every offset is what makes this a property test rather
/// than a hand-picked table.
fn offered_everywhere(text: &str) -> Vec<(usize, Vec<CandidateEdit>)> {
    let candidate = candidate_for(text, 1);
    let id = candidate
        .okf()
        .catalog
        .id_for_path(&BundlePath::parse("doc.md").unwrap())
        .unwrap();
    (0..=text.len())
        .filter(|offset| text.is_char_boundary(*offset))
        .filter_map(|offset| {
            let found = completions(
                ActionContext::from_prepared(&candidate).unwrap(),
                id,
                TextSize::try_from_usize(offset).unwrap(),
            )
            .unwrap();
            (!found.is_empty()).then(|| {
                (
                    offset,
                    found
                        .into_iter()
                        .map(|completion| {
                            (
                                completion.insert.to_string(),
                                completion.replace.start().to_usize()
                                    ..completion.replace.end().to_usize(),
                            )
                        })
                        .collect(),
                )
            })
        })
        .collect()
}

fn corpus() -> Vec<String> {
    vec![
        // A sequence with a half-typed endpoint.
        concat!(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [A](./a.md) as buyer\n- [B](./b.md) as order\n\n",
            "## Messages\n\n- buyer calls order `submit()` as submission\n- buyer calls \n"
        )
        .to_owned(),
        // An activity with an unfinished transition.
        concat!(
            "---\ntype: uml.Activity\ntitle: F\n---\n# F\n\n",
            "## Nodes\n\n### Receive\n\n- transitions to Check\n\n### Check\n\n- transitions to \n"
        )
        .to_owned(),
        // A diagram with an unfinished layout statement.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- A above \n"
        )
        .to_owned(),
        // An empty layout bullet: the statement start, where the grammar wants
        // an operand and any plain word would parse as one.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- \n"
        )
        .to_owned(),
        // A complete layout statement -- an alignment, which admits no trailing
        // word at all. Nothing but a complete statement reaches the locator's
        // `layout_continuation` rule, so without a case like this one the sweep
        // never visits the position that rule answers.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- A aligned with B\n"
        )
        .to_owned(),
        // An inline group left open after its separator: `,` is an atom of the
        // shape grammar, so the position after it wants another operand.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- row of A, \n"
        )
        .to_owned(),
        // A layout hint slot, where only a hint phrase parses.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [A](./a.md)\n- [B](./b.md)\n\n## Layout\n\n- A above B with \n"
        )
        .to_owned(),
        // A class with an unfinished relationship.
        concat!(
            "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n",
            "## Relationships\n\n-  [A](./a.md)\n"
        )
        .to_owned(),
        // An interaction use with an unfinished binding: both halves of a
        // binding are endpoints, but of different interactions.
        concat!(
            "---\ntype: uml.Sequence\ntitle: S\n---\n# S\n\n",
            "## Lifelines\n\n- [A](./a.md) as buyer\n\n",
            "## Messages\n\n- ref [Inner](./inner.md) as inner\n  - bind buyer to \n"
        )
        .to_owned(),
        // A flow node heading -- a declaration site, where an identity the
        // document already declares is a duplicate.
        concat!(
            "---\ntype: uml.Activity\ntitle: F\n---\n# F\n\n",
            "## Nodes\n\n### Receive\n\n- transitions to Receive\n\n### decision \n"
        )
        .to_owned(),
        // An instance's own slots, checked against its `instance of` classifier.
        concat!(
            "---\ntype: uml.InstanceSpecification\ntitle: O\n---\n# O\n\n",
            "## Slots\n\n- \n\n## Relationships\n\n- instance of [A](./a.md)\n"
        )
        .to_owned(),
        // A diagram whose member link text is nothing like the document it
        // names, plus a group whose name carries a space -- neither is
        // reachable as a layout operand the way it is written.
        concat!(
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n",
            "## Members\n\n- [Status](./a.md)\n\n### Core People\n\n- [B](./b.md)\n\n",
            "## Layout\n\n- \n"
        )
        .to_owned(),
    ]
}

#[test]
fn every_offered_candidate_is_accepted_by_the_diagnostic_that_guards_its_position() {
    for text in corpus() {
        let baseline = guard_codes(&text);
        for (offset, candidates) in offered_everywhere(&text) {
            for (insert, replace) in candidates {
                let mut applied = text.clone();
                applied.replace_range(replace.clone(), &insert);
                let after = guard_codes(&applied);
                let introduced = after
                    .iter()
                    .filter(|code| !baseline.contains(code))
                    .collect::<Vec<_>>();
                assert!(
                    introduced.is_empty(),
                    "offset {offset}: inserting {insert:?} over {replace:?} raised {introduced:?}\n--- document ---\n{applied}"
                );
            }
        }
    }
}
