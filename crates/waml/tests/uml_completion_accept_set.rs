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
];

const SUPPORT: [(&str, &str); 2] = [
    ("a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"),
    ("b.md", "---\ntype: uml.Class\ntitle: B\n---\n# B\n"),
];

fn candidate_for(text: &str, revision: u64) -> PreparedCandidate {
    let mut pairs = vec![("doc.md", text)];
    pairs.extend(SUPPORT);
    prepare_candidate(SourceBundle::try_from_pairs(pairs).unwrap(), None, revision).unwrap()
}

fn guard_codes(text: &str) -> Vec<DiagCode> {
    candidate_for(text, 2)
        .uml()
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .filter(|code| GUARDS.contains(code))
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
        // A class with an unfinished relationship.
        concat!(
            "---\ntype: uml.Class\ntitle: C\n---\n# C\n\n",
            "## Relationships\n\n-  [A](./a.md)\n"
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
