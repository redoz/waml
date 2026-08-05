use waml::{
    analysis::{analyze_okf, DomainAnalysisContext},
    diagnostic::DiagCode,
    model::{
        EndpointRef, FragmentKind, InteractionUseId, MessageId, MessageKind, OperandSpec, SeqChild,
        SeqNode,
    },
    solve::interaction::{measure_interaction, solve_interaction, InteractionConfig},
    source::SourceBundle,
    uml,
};
use waml_syntax::{AstNode, TextRange, TextSize};

fn analyze(pairs: impl IntoIterator<Item = (&'static str, &'static str)>) -> uml::Analysis {
    let source = SourceBundle::try_from_pairs(pairs).unwrap();
    let okf = analyze_okf(&source, None, 1).unwrap();
    uml::analyze(
        DomainAnalysisContext {
            source: &source,
            catalog: &okf.catalog,
            markdown: &okf.markdown,
            okf: &okf.bundle,
            session_revision: 1,
        },
        None,
    )
    .unwrap()
}

fn interaction<'a>(analysis: &'a uml::Analysis, key: &str) -> &'a waml::model::SequenceDoc {
    analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == key)
        .unwrap()
}

fn diagnostic<'a>(
    analysis: &'a uml::Analysis,
    code: DiagCode,
    message: &str,
) -> &'a waml::diagnostic::Diagnostic {
    analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == code && diagnostic.message.contains(message))
        .unwrap()
}

fn range_of(source: &str, needle: &str) -> TextRange {
    let start = source.find(needle).unwrap();
    TextRange::new(
        TextSize::new(start as u32),
        TextSize::new((start + needle.len()) as u32),
    )
    .unwrap()
}

fn range_of_last(source: &str, needle: &str) -> TextRange {
    let start = source.rfind(needle).unwrap();
    TextRange::new(
        TextSize::new(start as u32),
        TextSize::new((start + needle.len()) as u32),
    )
    .unwrap()
}

#[test]
fn endpoint_kinds_resolve() {
    let analysis = analyze([
        (
            "a.md",
            "---\ntype: uml.Class\n---\n# A\n",
        ),
        (
            "b.md",
            "---\ntype: uml.Class\n---\n# B\n",
        ),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Lifelines\n- [B](./b.md) as b\n\n## Gates\n- request\n\n## Messages\n- @request signals b `inside`\n",
        ),
        (
            "s.md",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Gates\n- frame\n\n## Messages\n- ref [Target](./target.md) as auth\n  - bind b to b\n- a calls b `one()`\n- outside signals b `two`\n- @frame signals b `three`\n- auth@request signals b `four`\n- a calls b async `five()`\n- a returns `six` to b\n- a creates b: `B`\n- a destroys b\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "s")
        .unwrap();

    assert_eq!(doc.edges[0].from, EndpointRef::Lifeline { id: "a".into() });
    assert_eq!(doc.edges[1].from, EndpointRef::Outside);
    assert_eq!(
        doc.edges[2].from,
        EndpointRef::LocalGate {
            gate: "frame".into()
        }
    );
    assert_eq!(
        doc.edges[3].from,
        EndpointRef::UseGate {
            interaction_use: InteractionUseId("u0".into()),
            gate: "request".into(),
        }
    );
    assert_eq!(
        doc.edges.iter().map(|edge| edge.kind).collect::<Vec<_>>(),
        [
            MessageKind::SyncCall,
            MessageKind::AsyncSignal,
            MessageKind::AsyncSignal,
            MessageKind::AsyncSignal,
            MessageKind::AsyncCall,
            MessageKind::Reply,
            MessageKind::Create,
            MessageKind::Delete,
        ]
    );
}

#[test]
fn outside_to_outside_is_diagnosed_and_excluded_from_runtime_projection() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        (
            "s.md",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Lifelines\n- [A](./a.md) as a\n\n## Messages\n- outside signals outside `invalid`\n- outside signals a `valid`\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "s")
        .unwrap();

    assert_eq!(doc.edges.len(), 1);
    assert_eq!(doc.edges[0].id, MessageId(1));
    assert_eq!(doc.edges[0].from, EndpointRef::Outside);
    assert_eq!(
        doc.edges[0].to,
        Some(EndpointRef::Lifeline { id: "a".into() })
    );
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::InvalidSequenceEndpoint));
}

#[test]
fn returns_follow_the_locked_candidate_algorithm() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "s.md",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `first()` as first\n- b returns `ok` to a for first\n- a calls b async `second()` as second\n- b returns `ok` for second\n- a calls b `third()`\n- a calls b `fourth()`\n- b returns `ambiguous`\n- b returns `unknown` for missing\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "s")
        .unwrap();

    assert_eq!(doc.edges[1].returns_call, Some(MessageId(0)));
    assert_eq!(doc.edges[3].returns_call, Some(MessageId(2)));
    assert_eq!(doc.edges[6].returns_call, None);
    assert_eq!(doc.edges[7].returns_call, None);
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::AmbiguousReturn));
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::UnknownCallIdentity));
}

#[test]
fn return_diagnostics_pin_the_exact_authored_message() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "returns.md",
            "---\ntype: uml.Sequence\n---\n# Returns\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `one()` as duplicate\n- a calls b `two()` as duplicate\n- b returns `ambiguous explicit` for duplicate\n- b returns `unknown` for missing\n- a returns `unmatched`\n- a calls b `three()`\n- a calls b `four()`\n- b returns `ambiguous inferred`\n- a calls b `done()` as done\n- b returns `done` for done\n- b returns `completed` for done\n- a calls b `conflict()` as conflict\n- a returns `conflicting` for conflict\n",
        ),
    ]);
    let declared = analysis.declared.concept("returns").unwrap();
    for (code, message, message_index) in [
        (
            DiagCode::DuplicateCallIdentity,
            "duplicate call identity",
            0,
        ),
        (DiagCode::AmbiguousReturn, "not unique", 2),
        (DiagCode::UnknownCallIdentity, "unknown call identity", 3),
        (DiagCode::UnmatchedReturn, "no eligible", 4),
        (DiagCode::AmbiguousReturn, "more than one", 7),
        (DiagCode::CompletedReturn, "already has", 10),
        (DiagCode::ConflictingReturn, "conflict", 12),
    ] {
        let found = diagnostic(&analysis, code, message);
        assert_eq!(
            found.range,
            Some(declared.messages[message_index].syntax.syntax().range()),
            "{code:?} must point at message {message_index}"
        );
        assert!(found.line > 1);
    }
}

#[test]
fn fragment_operand_rules_are_exact() {
    let valid = "---\ntype: uml.Sequence\n---\n# Valid\n\n## Messages\n- alt\n  - when `a`\n  - else\n- opt\n  - when `a`\n- loop\n  - when `a`\n- break\n  - when `a`\n- par\n  - branch `a`\n  - branch\n- critical\n  - branch\n- assert\n  - branch\n- neg\n  - branch\n";
    let invalid = "---\ntype: uml.Sequence\n---\n# Invalid\n\n## Messages\n- alt\n  - else\n  - when `late`\n  - else\n- opt\n  - else\n- loop\n  - branch\n- break\n  - when `a`\n  - when `b`\n- par\n  - branch\n- critical\n  - branch\n  - branch\n- assert\n  - when `a`\n- neg\n  - else\n";
    let analysis = analyze([("valid.md", valid), ("invalid.md", invalid)]);
    let invalid_count = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InvalidFragmentOperands)
        .count();
    assert_eq!(invalid_count, 8);
    assert!(analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InvalidFragmentOperands)
        .all(|diagnostic| diagnostic.range.is_some()));
    let valid_doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "valid")
        .unwrap();
    assert_eq!(
        valid_doc
            .nodes
            .iter()
            .filter_map(|node| match node {
                SeqNode::Fragment { kind, .. } => Some(*kind),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            FragmentKind::Alt,
            FragmentKind::Opt,
            FragmentKind::Loop,
            FragmentKind::Break,
            FragmentKind::Par,
            FragmentKind::Critical,
            FragmentKind::Assert,
            FragmentKind::Neg,
        ]
    );
}

#[test]
fn nested_fragments_keep_order_and_branch_boundaries() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "nested.md",
            "---\ntype: uml.Sequence\n---\n# Nested\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- alt\n  - when `ready`\n    - a calls b `outer()`\n    - par\n      - branch `left`\n        - a signals b `left`\n      - branch `right`\n        - b signals a `right`\n  - else\n    - b returns `fallback` to a\n- a signals b `after`\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "nested")
        .unwrap();
    assert!(matches!(
        doc.items.as_slice(),
        [SeqChild::Fragment { node }, SeqChild::Message { edge }]
            if node == "f0" && edge.0 == 4
    ));
    let first_alt_operand = doc
        .nodes
        .iter()
        .find_map(|node| match node {
            SeqNode::Operand {
                id,
                spec: OperandSpec::Guard(_),
                items,
            } if id == "f0.o0" => Some(items),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        first_alt_operand.as_slice(),
        [SeqChild::Message { edge }, SeqChild::Fragment { node }]
            if edge.0 == 0 && node == "f1"
    ));
    for (id, expected) in [("f1.o0", 1usize), ("f1.o1", 2usize)] {
        let items = doc
            .nodes
            .iter()
            .find_map(|node| match node {
                SeqNode::Operand {
                    id: operand_id,
                    items,
                    ..
                } if operand_id == id => Some(items),
                _ => None,
            })
            .unwrap();
        assert!(matches!(items.as_slice(), [SeqChild::Message { edge }] if edge.0 == expected));
    }
}

#[test]
fn parallel_branches_do_not_infer_returns_from_siblings() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "branches.md",
            "---\ntype: uml.Sequence\n---\n# Branches\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- par\n  - branch `call`\n    - a calls b `work()` as work\n  - branch `return`\n    - b returns `wrong sibling` to a\n- b returns `after join` for work\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "branches")
        .unwrap();
    assert_eq!(doc.edges[1].returns_call, None);
    assert_eq!(doc.edges[2].returns_call, Some(MessageId(0)));
}

#[test]
fn conditional_join_keeps_calls_that_can_remain_open() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "conditional.md",
            "---\ntype: uml.Sequence\n---\n# Conditional\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `work()` as work\n- opt\n  - when `early`\n    - b returns `early` for work\n- b returns `later` for work\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "conditional")
        .unwrap();
    assert_eq!(doc.edges[1].returns_call, Some(MessageId(0)));
    assert_eq!(doc.edges[2].returns_call, Some(MessageId(0)));
}

/// A multi-operand `opt` is malformed (`InvalidFragmentOperands`) but still
/// lowers, and the return walker must NOT readmit the incoming open-call set
/// for it: only the lone-operand `opt`/`loop`/`break` shape readmits. If the
/// incoming call were readmitted the trailing return would find a match and no
/// `UnmatchedReturn` would be reported.
#[test]
fn a_multi_operand_opt_does_not_readmit_the_incoming_open_calls() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "multi.md",
            "---\ntype: uml.Sequence\n---\n# Multi\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `work()`\n- opt\n  - when `x`\n    - b returns `first`\n  - when `y`\n    - b returns `second`\n- b returns `third`\n",
        ),
    ]);
    diagnostic(
        &analysis,
        DiagCode::UnmatchedReturn,
        "return has no eligible preceding call",
    );
}

/// An operand-less fragment is malformed but still lowers. It contributes no
/// branch outcome, so unless the fragment readmits its incoming state the
/// open-call set is wiped — the trailing return then has nothing to match.
#[test]
fn an_operand_less_fragment_wipes_the_open_calls() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "empty.md",
            "---\ntype: uml.Sequence\n---\n# Empty\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `work()`\n- critical\n- b returns `late`\n",
        ),
    ]);
    diagnostic(
        &analysis,
        DiagCode::UnmatchedReturn,
        "return has no eligible preceding call",
    );
}

#[test]
fn interaction_use_resolves_without_flattening() {
    let analysis = analyze([
        ("order.md", "---\ntype: uml.Class\n---\n# Order\n"),
        ("payment.md", "---\ntype: uml.Class\n---\n# Payment\n"),
        (
            "audit.md",
            "---\ntype: uml.Sequence\n---\n# Audit\n",
        ),
        (
            "authorize-payment.md",
            "---\ntype: uml.Sequence\n---\n# Authorize\n\n## Lifelines\n- [Order](./order.md) as caller\n- [Payment](./payment.md) as payment\n\n## Gates\n- request\n\n## Messages\n- ref [Audit](./audit.md) as audit\n- @request calls payment `authorize()` as authorization\n- payment returns `approved` for authorization\n",
        ),
        (
            "checkout.md",
            "---\ntype: uml.Sequence\n---\n# Checkout\n\n## Lifelines\n- [Order](./order.md) as order\n- [Payment](./payment.md) as payment\n\n## Messages\n- ref [Authorize](./authorize-payment.md) as auth\n  - bind order to caller\n  - bind payment to payment\n- order calls auth@request `authorize()` as authorization\n",
        ),
    ]);
    let checkout = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "checkout")
        .unwrap();
    let auth = &checkout.interaction_uses[0];
    assert_eq!(auth.target, "authorize-payment");
    assert_eq!(auth.alias, "auth");
    assert_eq!(
        auth.bindings
            .iter()
            .map(|binding| (binding.local.as_str(), binding.target.as_str()))
            .collect::<Vec<_>>(),
        [("order", "caller"), ("payment", "payment")]
    );
    assert_eq!(auth.gates, ["request"]);
    assert!(matches!(
        checkout.items.first(),
        Some(SeqChild::InteractionUse { interaction_use }) if interaction_use == &auth.id
    ));
    assert_eq!(
        checkout.edges.len(),
        1,
        "referenced messages must not be flattened"
    );

    let target = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "authorize-payment")
        .unwrap();
    assert_eq!(target.interaction_uses[0].target, "audit");
    assert!(target
        .edges
        .iter()
        .all(|edge| edge.value.as_deref() != Some("audit")));
}

#[test]
fn invalid_interaction_uses_keep_declared_siblings() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        (
            "good.md",
            "---\ntype: uml.Sequence\n---\n# Good\n",
        ),
        (
            "parent.md",
            "---\ntype: uml.Sequence\n---\n# Parent\n\n## Lifelines\n- [A](./a.md) as a\n\n## Messages\n- ref [Missing](./missing.md) as missing\n- ref [Good](./good.md) as good\n- a signals outside `later`\n",
        ),
    ]);
    let declared = analysis.declared.concept("parent").unwrap();
    assert_eq!(declared.interaction_uses.len(), 2);
    let parent = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "parent")
        .unwrap();
    assert_eq!(
        parent
            .interaction_uses
            .iter()
            .map(|interaction_use| interaction_use.alias.as_str())
            .collect::<Vec<_>>(),
        ["good"]
    );
    assert!(matches!(
        parent.items.last(),
        Some(SeqChild::Message { .. })
    ));
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::InvalidInteractionUse));
}

#[test]
fn invalid_reference_validation_never_silently_removes_later_valid_uses() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Lifelines\n- [B](./b.md) as target_b\n\n## Messages\n- target_b signals outside `participates`\n",
        ),
        ("empty.md", "---\ntype: uml.Sequence\n---\n# Empty\n"),
        (
            "uses.md",
            "---\ntype: uml.Sequence\n---\n# Uses\n\n## Lifelines\n- [A](./a.md) as local_a\n\n## Messages\n- ref [Missing](./missing.md) as invalid_link\n- ref [Target](./target.md) as mismatch\n  - bind local_a to target_b\n- ref [Empty](./empty.md) as valid\n",
        ),
    ]);
    let doc = interaction(&analysis, "uses");
    assert_eq!(doc.interaction_uses.len(), 1);
    assert_eq!(doc.interaction_uses[0].alias, "valid");
    assert_eq!(doc.interaction_uses[0].id, InteractionUseId("u2".into()));
    let rejected = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.file == "uses.md" && diagnostic.code == DiagCode::InvalidInteractionUse
        })
        .collect::<Vec<_>>();
    assert!(rejected
        .iter()
        .any(|diagnostic| diagnostic.message.contains("unresolved")));
    assert!(rejected
        .iter()
        .any(|diagnostic| diagnostic.message.contains("different classifiers")));
    assert!(rejected.iter().all(|diagnostic| diagnostic.range.is_some()));
}

#[test]
fn interaction_use_cycles_and_binding_errors_are_diagnosed() {
    let analysis = analyze([
        ("a-class.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b-class.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Lifelines\n- [A](./a-class.md) as ta\n- [B](./b-class.md) as tb\n\n## Gates\n- idle\n- idle\n\n## Messages\n- ta calls tb `work()`\n",
        ),
        (
            "bindings.md",
            "---\ntype: uml.Sequence\n---\n# Bindings\n\n## Lifelines\n- [A](./a-class.md) as pa\n- [B](./b-class.md) as pb\n\n## Messages\n- ref [Target](./target.md) as duplicate-local\n  - bind pa to ta\n  - bind pa to tb\n- ref [Target](./target.md) as duplicate-target\n  - bind pa to ta\n  - bind pb to ta\n- ref [Target](./target.md) as mismatch\n  - bind pa to tb\n  - bind pb to ta\n- ref [Target](./target.md) as missing-binding\n  - bind pa to ta\n- ref [Target](./target.md) as pa\n  - bind pa to ta\n  - bind pb to tb\n- ref [Target](./target.md) as gate-use\n  - bind pa to ta\n  - bind pb to tb\n- pa calls gate-use@idle `outer()`\n",
        ),
        ("cycle-a.md", "---\ntype: uml.Sequence\n---\n# A\n\n## Messages\n- ref [B](./cycle-b.md) as b\n"),
        ("cycle-b.md", "---\ntype: uml.Sequence\n---\n# B\n\n## Messages\n- ref [A](./cycle-a.md) as a\n"),
        ("cycle-x.md", "---\ntype: uml.Sequence\n---\n# X\n\n## Messages\n- ref [Y](./cycle-y.md) as y\n"),
        ("cycle-y.md", "---\ntype: uml.Sequence\n---\n# Y\n\n## Messages\n- ref [Z](./cycle-z.md) as z\n"),
        ("cycle-z.md", "---\ntype: uml.Sequence\n---\n# Z\n\n## Messages\n- ref [X](./cycle-x.md) as x\n"),
    ]);
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::DuplicateGate));
    assert!(
        analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagCode::InvalidInteractionUse)
            .count()
            >= 5
    );
    assert!(
        analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == DiagCode::InteractionUseCycle)
            .count()
            >= 5
    );
}

#[test]
fn every_binding_issue_drops_the_use_and_reports_it() {
    // One fixture per `UseBindingIssue` variant. Each asserts BOTH halves of
    // the shared verdict: the offending use is dropped from the projection
    // (the graph copy) and the exact message lands on the exact span (the
    // diagnosed copy). A drop-vs-report divergence fails one half or the other.
    macro_rules! uses_source {
        ($binds:literal) => {
            concat!(
                "---\ntype: uml.Sequence\n---\n# Uses\n\n## Lifelines\n",
                "- [A](./a.md) as pa\n- [A](./a.md) as pb\n- [A](./a.md) as pc\n\n",
                "## Messages\n- ref [Target](./target.md) as bad\n",
                $binds,
                "- ref [Target](./target.md) as ok\n  - bind pa to ta\n  - bind pb to tb\n",
            )
        };
    }
    for (source, message, binding_index) in [
        (
            uses_source!("  - bind pa to ta\n  - bind pa to tb\n"),
            "interaction use 'bad' has duplicate bindings",
            Some(1),
        ),
        (
            uses_source!("  - bind pa to ta\n  - bind pb to tb\n  - bind nope to tc\n"),
            "interaction use 'bad' has an unknown binding endpoint",
            Some(2),
        ),
        (
            uses_source!("  - bind pa to ta\n  - bind pb to tb\n  - bind pc to tc\n"),
            "interaction use 'bad' binds different classifiers",
            Some(2),
        ),
        (
            uses_source!("  - bind pa to ta\n"),
            "interaction use 'bad' is missing a participating lifeline binding",
            None,
        ),
    ] {
        let analysis = analyze([
            ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
            ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
            (
                "target.md",
                "---\ntype: uml.Sequence\n---\n# Target\n\n## Lifelines\n- [A](./a.md) as ta\n- [A](./a.md) as tb\n- [B](./b.md) as tc\n\n## Messages\n- ta calls tb `work()`\n",
            ),
            ("uses.md", source),
        ]);

        // The graph drop: only the well-bound use survives into the model.
        let aliases = interaction(&analysis, "uses")
            .interaction_uses
            .iter()
            .map(|use_| use_.alias.as_str())
            .collect::<Vec<_>>();
        assert_eq!(aliases, ["ok"], "for {message}");

        // The diagnostic: exact message, exact span.
        let declared = analysis.declared.concept("uses").unwrap();
        let expected = match binding_index {
            Some(index) => declared.interaction_uses[0].bindings[index]
                .syntax
                .syntax()
                .range(),
            // The whole-use issue is pinned to the `bad` use's own ref link,
            // which is the first `./target.md` in the fixture.
            None => range_of(source, "./target.md"),
        };
        let found = diagnostic(&analysis, DiagCode::InvalidInteractionUse, message);
        assert_eq!(found.range, Some(expected), "for {message}");
    }
}

#[test]
fn binding_diagnostics_pin_the_exact_bind_or_ref() {
    let source = "---\ntype: uml.Sequence\n---\n# Bindings\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- ref [Target](./target.md) as duplicate\n  - bind a to ta\n  - bind a to tb\n- ref [Target](./target.md) as unknown\n  - bind missing to ta\n  - bind b to tb\n- ref [Target](./target.md) as mismatch\n  - bind a to tb\n  - bind b to ta\n- ref [Target](./target.md) as missing\n  - bind a to ta\n";
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Lifelines\n- [A](./a.md) as ta\n- [B](./b.md) as tb\n\n## Messages\n- ta signals tb `work`\n",
        ),
        (
            "bindings-focused.md",
            source,
        ),
    ]);
    let declared = analysis.declared.concept("bindings-focused").unwrap();
    for (message, expected) in [
        (
            "duplicate' has duplicate bindings",
            declared.interaction_uses[0].bindings[1]
                .syntax
                .syntax()
                .range(),
        ),
        (
            "unknown' has an unknown binding",
            declared.interaction_uses[1].bindings[0]
                .syntax
                .syntax()
                .range(),
        ),
        (
            "mismatch' binds different",
            declared.interaction_uses[2].bindings[0]
                .syntax
                .syntax()
                .range(),
        ),
        (
            "missing' is missing a participating",
            range_of_last(source, "./target.md"),
        ),
    ] {
        let found = diagnostic(&analysis, DiagCode::InvalidInteractionUse, message);
        assert_eq!(found.range, Some(expected));
        assert!(found.line > 1);
    }
}

#[test]
fn gate_diagnostics_pin_the_exact_gate_or_message() {
    let analysis = analyze([
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Gates\n- idle\n- idle\n",
        ),
        (
            "gates-focused.md",
            "---\ntype: uml.Sequence\n---\n# Gates\n\n## Messages\n- ref [Target](./target.md) as target\n- target@missing signals outside `missing`\n- target@idle signals outside `disconnected`\n",
        ),
    ]);
    let target = analysis.declared.concept("target").unwrap();
    let parent = analysis.declared.concept("gates-focused").unwrap();
    assert_eq!(
        diagnostic(&analysis, DiagCode::DuplicateGate, "duplicate gate").range,
        Some(target.gates[1].syntax.syntax().range())
    );
    assert_eq!(
        diagnostic(&analysis, DiagCode::InvalidInteractionUse, "has no gate").range,
        Some(parent.messages[0].syntax.syntax().range())
    );
    assert_eq!(
        diagnostic(
            &analysis,
            DiagCode::InvalidInteractionUse,
            "no inner connection"
        )
        .range,
        Some(parent.messages[1].syntax.syntax().range())
    );
}

#[test]
fn call_and_return_can_share_one_interaction_use_gate() {
    let analysis = analyze([
        ("client.md", "---\ntype: uml.Class\n---\n# Client\n"),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Gates\n- request\n\n## Messages\n- @request signals outside `inside`\n",
        ),
        (
            "parent.md",
            "---\ntype: uml.Sequence\n---\n# Parent\n\n## Lifelines\n- [Client](./client.md) as client\n\n## Messages\n- ref [Target](./target.md) as auth\n- client calls auth@request `authorize()` as authorization\n- auth@request returns `accepted` to client for authorization\n",
        ),
    ]);
    assert!(!analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::InvalidInteractionUse));
    let doc = interaction(&analysis, "parent");
    assert_eq!(doc.edges.len(), 2);
    assert_eq!(doc.items.len(), 3);
    assert_eq!(doc.interaction_uses[0].gates, ["request"]);
    assert_eq!(doc.edges[1].returns_call, Some(doc.edges[0].id));

    let cfg = InteractionConfig::default();
    let sizes = measure_interaction(doc, &cfg);
    let (solved, diagnostics) = solve_interaction(doc, &sizes, &cfg);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let gate_rows = solved.interaction_uses[0]
        .gates
        .iter()
        .filter(|gate| gate.name == "request")
        .map(|gate| gate.y)
        .collect::<Vec<_>>();
    assert_eq!(gate_rows, [solved.messages[0].y, solved.messages[1].y]);
}

#[test]
fn direct_interaction_use_cycle_reports_each_valid_authored_ref() {
    let source =
        "---\ntype: uml.Sequence\n---\n# Direct\n\n## Messages\n- ref [Missing](./missing.md) as invalid\n- ref [Direct](./direct.md) as self_ref\n";
    let analysis = analyze([("direct.md", source)]);
    let cycles = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InteractionUseCycle)
        .collect::<Vec<_>>();
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].range, Some(range_of(source, "./direct.md")));
}

#[test]
fn invalid_interaction_use_alias_is_excluded_from_cycle_graph() {
    let a = "---\ntype: uml.Sequence\n---\n# A\n\n## Messages\n- ref [B](./b.md) as outside\n";
    let b = "---\ntype: uml.Sequence\n---\n# B\n\n## Messages\n- ref [A](./a.md) as a\n";
    let analysis = analyze([("a.md", a), ("b.md", b)]);

    assert_eq!(
        analysis
            .declared
            .concept("a")
            .unwrap()
            .interaction_uses
            .len(),
        1
    );
    assert_eq!(
        analysis
            .declared
            .concept("b")
            .unwrap()
            .interaction_uses
            .len(),
        1
    );
    assert!(interaction(&analysis, "a").interaction_uses.is_empty());
    assert_eq!(interaction(&analysis, "b").interaction_uses.len(), 1);
    assert_eq!(interaction(&analysis, "b").interaction_uses[0].alias, "a");
    assert!(analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.file == "a.md" && diagnostic.code == DiagCode::ReservedSequenceName
    }));
    assert!(!analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::InteractionUseCycle));
}

#[test]
fn indirect_three_document_cycle_reports_each_valid_authored_ref() {
    let a = "---\ntype: uml.Sequence\n---\n# A\n\n## Messages\n- ref [Missing](./missing.md) as invalid\n- ref [B](./b.md) as b\n";
    let b = "---\ntype: uml.Sequence\n---\n# B\n\n## Messages\n- ref [C](./c.md) as c\n";
    let c = "---\ntype: uml.Sequence\n---\n# C\n\n## Messages\n- ref [A](./a.md) as a\n";
    let analysis = analyze([("a.md", a), ("b.md", b), ("c.md", c)]);
    let cycles = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InteractionUseCycle)
        .collect::<Vec<_>>();
    assert_eq!(cycles.len(), 3);
    for (file, range) in [
        ("a.md", range_of(a, "./b.md")),
        ("b.md", range_of(b, "./c.md")),
        ("c.md", range_of(c, "./a.md")),
    ] {
        assert!(cycles
            .iter()
            .any(|diagnostic| { diagnostic.file == file && diagnostic.range == Some(range) }));
    }
}

#[test]
fn invalid_runtime_entries_do_not_renumber_valid_siblings() {
    let bad = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        ("target.md", "---\ntype: uml.Sequence\n---\n# Target\n"),
        (
            "stable.md",
            "---\ntype: uml.Sequence\n---\n# Stable\n\n## Lifelines\n- [A](./a.md) as outside\n- [A](./a.md) as a\n- [B](./b.md) as a\n- [B](./b.md) as b\n\n## Messages\n- missing signals b `invalid`\n- a signals b `valid`\n- ref [Missing](./missing.md) as missing\n- ref [Target](./target.md) as target\n",
        ),
    ]);
    let fixed = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        ("target.md", "---\ntype: uml.Sequence\n---\n# Target\n"),
        (
            "stable.md",
            "---\ntype: uml.Sequence\n---\n# Stable\n\n## Lifelines\n- [A](./a.md) as repaired\n- [A](./a.md) as a\n- [B](./b.md) as duplicate-repaired\n- [B](./b.md) as b\n\n## Messages\n- repaired signals b `repaired`\n- a signals b `valid`\n- ref [Target](./target.md) as repaired-use\n- ref [Target](./target.md) as target\n",
        ),
    ]);
    let bad_doc = interaction(&bad, "stable");
    let fixed_doc = interaction(&fixed, "stable");
    assert_eq!(
        bad_doc
            .nodes
            .iter()
            .filter(|node| matches!(node, SeqNode::Lifeline { .. }))
            .count(),
        2
    );
    assert_eq!(bad_doc.edges[0].id, MessageId(1));
    assert_eq!(fixed_doc.edges[1].id, MessageId(1));
    assert_eq!(
        bad_doc.interaction_uses[0].id,
        InteractionUseId("u1".into())
    );
    assert_eq!(
        fixed_doc.interaction_uses[1].id,
        InteractionUseId("u1".into())
    );
}

#[test]
fn future_duplicate_call_identity_does_not_ambiguous_match() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "future.md",
            "---\ntype: uml.Sequence\n---\n# Future\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a calls b `first()` as duplicate\n- b returns `first` for duplicate\n- a calls b `later()` as duplicate\n",
        ),
    ]);
    let doc = analysis
        .projection
        .interactions
        .iter()
        .find(|doc| doc.key == "future")
        .unwrap();
    assert_eq!(doc.edges[1].returns_call, Some(MessageId(0)));
    assert!(!analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::AmbiguousReturn));
}

#[test]
fn alt_with_only_else_is_invalid() {
    let analysis = analyze([(
        "else-only.md",
        "---\ntype: uml.Sequence\n---\n# Else only\n\n## Messages\n- alt\n  - else\n",
    )]);
    assert!(analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::InvalidFragmentOperands));
}

#[test]
fn parallel_sibling_branches_do_not_impose_lifetime_order() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "s.md",
            "---\ntype: uml.Sequence\n---\n# S\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- par\n  - branch `use`\n    - a signals b `use`\n  - branch `create`\n    - a creates b: `B`\n",
        ),
    ]);

    assert!(!analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagCode::InvalidLifelineLifetime));
}

#[test]
fn lifetime_diagnostics_pin_the_exact_authored_message() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "lifetime.md",
            "---\ntype: uml.Sequence\n---\n# Lifetime\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a signals b `before`\n- a creates b: `B`\n- a creates b: `B again`\n- a destroys outside\n",
        ),
    ]);
    let declared = analysis.declared.concept("lifetime").unwrap();
    for (code, message, message_index) in [
        (DiagCode::InvalidLifelineLifetime, "outside its lifetime", 0),
        (DiagCode::InvalidLifelineLifetime, "more than once", 2),
        (DiagCode::InvalidSequenceEndpoint, "must be local", 3),
    ] {
        let found = diagnostic(&analysis, code, message);
        assert_eq!(
            found.range,
            Some(declared.messages[message_index].syntax.syntax().range())
        );
        assert!(found.line > 1);
    }
}

#[test]
fn repeated_delete_reports_the_exact_second_delete() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "delete.md",
            "---\ntype: uml.Sequence\n---\n# Delete\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- a destroys b\n- a destroys b\n",
        ),
    ]);
    let declared = analysis.declared.concept("delete").unwrap();
    let diagnostics = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InvalidLifelineLifetime)
        .collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "lifeline is created or deleted more than once"
    );
    assert_eq!(
        diagnostics[0].range,
        Some(declared.messages[1].syntax.syntax().range())
    );
}

#[test]
fn parallel_sibling_deletes_report_the_second_authored_delete() {
    let source = "---\ntype: uml.Sequence\n---\n# Parallel delete\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- par\n  - branch `first`\n    - a destroys b\n  - branch `second`\n    - a destroys b\n";
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        ("parallel-delete.md", source),
    ]);
    let deletes = &analysis
        .declared
        .concept("parallel-delete")
        .unwrap()
        .messages;
    let diagnostics = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InvalidLifelineLifetime)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1, "{:?}", analysis.diagnostics);
    assert_eq!(
        diagnostics[0].message,
        "lifeline is created or deleted more than once"
    );
    assert_eq!(
        diagnostics[0].range,
        Some(deletes[1].syntax.syntax().range())
    );
}

#[test]
fn delete_after_parallel_join_is_a_repeated_delete() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "post-join-delete.md",
            "---\ntype: uml.Sequence\n---\n# Post-join delete\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- par\n  - branch `delete`\n    - a destroys b\n  - branch `work`\n    - a signals a `work`\n- a destroys b\n",
        ),
    ]);
    let declared = analysis.declared.concept("post-join-delete").unwrap();
    let diagnostics = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::InvalidLifelineLifetime)
        .collect::<Vec<_>>();

    assert_eq!(diagnostics.len(), 1, "{:?}", analysis.diagnostics);
    assert_eq!(
        diagnostics[0].message,
        "lifeline is created or deleted more than once"
    );
    assert_eq!(
        diagnostics[0].range,
        Some(declared.messages[2].syntax.syntax().range())
    );
}

#[test]
fn reserved_sequence_names_stay_declared_and_out_of_runtime() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        ("target.md", "---\ntype: uml.Sequence\n---\n# Target\n"),
        (
            "names.md",
            "---\ntype: uml.Sequence\n---\n# Names\n\n## Lifelines\n- [A](./a.md) as outside\n- [A](./a.md) as bad@lifeline\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Gates\n- outside\n- bad@gate\n- good\n\n## Messages\n- ref [Target](./target.md) as outside\n- ref [Target](./target.md) as bad@use\n- a calls b `first()` as outside\n- a calls b `second()` as bad@call\n- b returns `first` to a for outside\n- b returns `second` to a for bad@call\n- a signals b `kept`\n",
        ),
    ]);
    let declared = analysis.declared.concept("names").unwrap();
    assert_eq!(declared.lifelines.len(), 4);
    assert_eq!(declared.gates.len(), 3);
    assert_eq!(declared.interaction_uses.len(), 2);
    assert_eq!(declared.messages.len(), 5);

    let reserved = analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagCode::ReservedSequenceName)
        .collect::<Vec<_>>();
    assert_eq!(reserved.len(), 10, "{:?}", analysis.diagnostics);
    assert!(!analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagCode::InvalidInteractionUse
            && diagnostic
                .message
                .contains("unresolved sequence interaction use")
    }));
    for kind in ["lifeline", "gate", "interaction-use alias", "call identity"] {
        assert_eq!(
            reserved
                .iter()
                .filter(|diagnostic| diagnostic.message.contains(kind))
                .count(),
            if kind == "call identity" { 4 } else { 2 },
            "missing reserved-name diagnostics for {kind}: {reserved:?}"
        );
    }

    let doc = interaction(&analysis, "names");
    let runtime_lifelines = doc
        .nodes
        .iter()
        .filter_map(|node| match node {
            SeqNode::Lifeline { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(runtime_lifelines, ["a", "b"]);
    assert_eq!(doc.gates, ["good"]);
    assert!(doc.interaction_uses.is_empty());
    assert_eq!(doc.edges.len(), 1);
    assert_eq!(doc.edges[0].kind, MessageKind::AsyncSignal);
}

#[test]
fn invalid_target_gate_name_cannot_resolve_through_an_interaction_use() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        (
            "target.md",
            "---\ntype: uml.Sequence\n---\n# Target\n\n## Gates\n- outside\n\n## Messages\n- @outside signals outside `inside`\n",
        ),
        (
            "parent.md",
            "---\ntype: uml.Sequence\n---\n# Parent\n\n## Lifelines\n- [A](./a.md) as a\n\n## Messages\n- ref [Target](./target.md) as target\n- a signals target@outside `unsafe`\n",
        ),
    ]);

    assert!(analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagCode::InvalidInteractionUse
            && diagnostic.message.contains("has no gate 'outside'")
    }));
    let parent = interaction(&analysis, "parent");
    assert!(parent.edges.is_empty());
    assert!(parent.interaction_uses[0].gates.is_empty());
}

#[test]
fn repairing_an_earlier_operand_does_not_renumber_later_operands() {
    let bad = analyze([(
        "operands.md",
        "---\ntype: uml.Sequence\n---\n# Operands\n\n## Messages\n- alt\n  - when\n  - when `later`\n",
    )]);
    let fixed = analyze([(
        "operands.md",
        "---\ntype: uml.Sequence\n---\n# Operands\n\n## Messages\n- alt\n  - when `fixed`\n  - when `later`\n",
    )]);
    let bad_fragment = interaction(&bad, "operands")
        .nodes
        .iter()
        .find_map(|node| match node {
            SeqNode::Fragment { operands, .. } => Some(operands),
            _ => None,
        })
        .unwrap();
    let fixed_fragment = interaction(&fixed, "operands")
        .nodes
        .iter()
        .find_map(|node| match node {
            SeqNode::Fragment { operands, .. } => Some(operands),
            _ => None,
        })
        .unwrap();
    assert_eq!(bad_fragment, &["f0.o1"]);
    assert_eq!(fixed_fragment, &["f0.o0", "f0.o1"]);
}

#[test]
fn alt_lifetime_states_join_only_definitely_alive_lifelines() {
    let analysis = analyze([
        ("a.md", "---\ntype: uml.Class\n---\n# A\n"),
        ("b.md", "---\ntype: uml.Class\n---\n# B\n"),
        (
            "sibling.md",
            "---\ntype: uml.Sequence\n---\n# Sibling\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- alt\n  - when `create`\n    - a creates b: `B`\n  - else\n    - a signals b `sibling use`\n",
        ),
        (
            "implicit.md",
            "---\ntype: uml.Sequence\n---\n# Implicit\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- alt\n  - when `create`\n    - a creates b: `B`\n- a signals b `after implicit path`\n",
        ),
        (
            "definite.md",
            "---\ntype: uml.Sequence\n---\n# Definite\n\n## Lifelines\n- [A](./a.md) as a\n- [B](./b.md) as b\n\n## Messages\n- alt\n  - when `first`\n    - a creates b: `B`\n  - else\n    - a creates b: `B`\n- a signals b `after all branches`\n",
        ),
    ]);
    for (concept_id, message_index) in [("sibling", 1), ("implicit", 1)] {
        let declared = analysis.declared.concept(concept_id).unwrap();
        let diagnostics = analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.file == format!("{concept_id}.md")
                    && diagnostic.code == DiagCode::InvalidLifelineLifetime
            })
            .collect::<Vec<_>>();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].range,
            Some(declared.messages[message_index].syntax.syntax().range())
        );
    }
    assert!(!analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.file == "definite.md" && diagnostic.code == DiagCode::InvalidLifelineLifetime
    }));
}

#[test]
fn sequence_describes_resolves_through_the_shared_link_ref_parser() {
    let analysis = analyze([
        ("m/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
        (
            "m/checkout.md",
            "---\ntype: uml.Sequence\ndescribes: [Order](./order.md)\n---\n# Checkout\n\n## Lifelines\n- [Order](./order.md) as o\n\n## Messages\n- outside calls o `place()`\n",
        ),
    ]);

    assert_eq!(
        interaction(&analysis, "m/checkout").describes.as_deref(),
        Some("m/order")
    );
}
