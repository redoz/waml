# `analysis` <-> `uml`: why the pipeline vocabulary is not a crate

Audit finding **A12 ("Layering")**: *"`analysis.rs` is a hub, not a layer ... it
owns `DocumentId`, dragging `diagnostic.rs` and `edit/` into mutual
dependency. 116 files couple to it."* Roadmap item 9 adds: *"Extract
`DocumentId`/`DocumentVersion` below `diagnostic`/`edit` to dissolve the
`analysis.rs` cycles."*

Most of that was done. `DocumentId` moved to `crate::source`, which killed the
`analysis` <-> `diagnostic` cycle outright; the `AnalysisError` conversions
moved to `edit/code.rs`; the UML-derived back-patch into `OkfAnalysis` was fixed
on the `OkfAnalysis` side. What was left, and what the remediation ledger
recorded as the last open item, was this:

> Remaining and NOT breakable by moving code: analysis <-> uml, which needs the
> pipeline vocabulary extracted into a tier below both -- a crate split, not a
> module move.

**That prescription is refused.** The crate split is not blocked by effort or
appetite; it is blocked by arithmetic. This file records the measurement, so
the next person does not re-derive it, and names what is enforced instead.

## Method

The 28 top-level modules of `crates/waml/src` were parsed for every `crate::`
path in **shipped** source -- line comments stripped, each file truncated at its
first top-level `#[cfg(test)]`, and the two files declared as
`#[cfg(test)] mod ...;` (`edit/port_tests.rs`, `solve/quality.rs`) excluded
entirely. This is the same production-only rule `tests/okf_layering.rs` already
applies, for the same reason: tests exercise a layer *through* the tiers above
it, and are not bound by its layering rule.

Braced imports (`use crate::{a::X, b::Y}`) are expanded, so `crate::{analysis::..}`
counts as an edge to `analysis` -- a naive `crate::analysis` grep misses roughly
half the real edges, and was the reason an earlier pass of this measurement
reported a much cleaner graph than exists.

## What the graph actually looks like

Two non-trivial strongly-connected components:

| SCC | Members | Internal edges |
|-----|---------|----------------|
| 1 | `action`, `analysis`, `edit`, `index_md`, `model`, `okf`, `uml`, `validate` | 20 |
| 2 | `extension`, `profile`, `view` | 5 |

`analysis` and `uml` are two of the **eight** members of SCC 1. The edges
inside it, by number of distinct call/import sites:

```
  action   -> analysis    1     okf      -> analysis   15
  action   -> edit        5     okf      -> edit        9
  analysis -> okf         1     okf      -> index_md    1
  analysis -> uml         5     uml      -> action      3
  edit     -> analysis    7     uml      -> analysis   15
  edit     -> model       1     uml      -> edit       11
  edit     -> okf         2     uml      -> model     164
  edit     -> uml         3     uml      -> okf        47
  index_md -> okf         2     validate -> analysis    1
  index_md -> validate    1     model    -> okf         1
```

## The counterfactual that decides it

A crate boundary cannot straddle a strongly-connected component: if `A` and `B`
cycle, they ship in one crate, full stop. So the question is not "is the
`analysis` <-> `uml` edge pair ugly" but "does removing it shrink the SCC".

Delete **both directions** -- pretend the pipeline vocabulary already lives in a
`waml-pipeline` crate below both, and that `analysis` no longer names `uml` at
all -- and recompute:

```
BASELINE       SCC 1 = 8 members
COUNTERFACTUAL SCC 1 = 8 members   (identical membership)
```

Nothing moves. `uml` stays in the cycle through `uml -> edit` (11 sites) and
`edit -> uml` (3), and through `uml -> okf` (47) then `okf -> analysis` (15).
The prescribed crate split buys **zero** acyclicity, at a cost of a new crate,
a new public surface, and a rewrite of 15 import sites.

Single-edge sensitivity, for anyone who wants the cheap wins later -- the
largest SCC that survives removing one edge:

| removed edge | largest SCC after |
|---|---|
| `okf -> index_md` (1 site) | **6** |
| `index_md -> validate`, `model -> okf`, `uml -> action`, `validate -> analysis` | 7 |
| *every other edge, including both `analysis`/`uml` edges* | 8 |

The `analysis` <-> `uml` pair the audit singled out is, by this measure, among
the **least** load-bearing edges in the cycle. Actually splitting a crate out
below `uml` means moving all eight SCC members -- 35,720 lines across 42 files,
35% of the crate's 101,077 lines -- including `uml -> model` (164 sites) and
`uml -> okf` (47), which are the domain model itself, not "pipeline
vocabulary".

## And the edge is not a defect anyway

Item by item, in shipped code:

**`analysis` -> `uml` is two names.** `analysis.rs`'s entire dependency on every
tier above its substrate is `crate::uml::Analysis` (four sites: a field in
`PreviousAnalyses`, a field in `PreparedCandidate`, the `uml()` accessor return,
the `into_parts` tuple) and `crate::uml::analyze` (one call, in
`prepare_candidate_inner_with_markdown_updates`). That is a composition root
naming its one specialization. It is pure naming: there is no UML *logic* in
the pipeline. The audit's "hard-codes the UML specialization" is true, and is
what a composition root is for.

**`uml` -> `analysis` is 15 items, 13 of which are a published facade.**

| kind | items |
|---|---|
| specialization facade (13) | `DomainAnalysisContext` (8 files), `AnalysisError` (5), `DocumentVersion` (3), `MarkdownSyntaxSet` (2), `AffectedAnalysis`, `ClaimSet`, `DiagnosticSource`, `DocumentRevision`, `ProjectionFreshness`, `RevisionedDiagnostic`, `SyntaxSet`, `SyntaxSnapshot`, `single_text_change` |
| composition-root product (2) | `OkfAnalysis`, `PreparedCandidate` -- both only in `uml/format.rs` |

That the first group is a *facade* rather than an accident is not an opinion:
`tests/specialization_composition.rs` builds a second, entirely non-UML
specialization (`FutureLanguage`/`FutureAnalysis`) from **outside** the crate,
using `DomainAnalysisContext`, `AnalysisError`, `ClaimSet`, `DocumentVersion`,
`DocumentCatalog`, `PreviousAnalyses` and `validate_disjoint_claims` -- the same
vocabulary `uml` uses, reached through `waml::analysis::` like any other
consumer. A facade with two independent implementors, one of them written
specifically to prove the facade is not UML-shaped, is a facade being used as
designed.

`DocumentRevision` is not even owned here: `analysis.rs:34` re-exports it from
`waml_syntax`. So part of the "vocabulary" already lives a crate below.

**`OkfAnalysis`/`PreparedCandidate` in `uml/format.rs` is the one item that
genuinely points up**, and it is three lines: `ActionContext::from_prepared`
unpacks a candidate into the `(okf, uml, revision)` triple that
`ActionContext::new` already takes. It has one shipped caller (`fmt.rs:23`) and
~35 test call sites. Moving it has nowhere good to go -- `analysis.rs` would
then name a third UML type, and `action.rs` would gain a `uml` edge it does not
have -- which is the usual sign that an adapter is already where it belongs.
Adapters point up; that is what makes them adapters.

## What is enforced instead

The split's real payload was never the crate. It was two invariants, and both
hold today and are now checked, in `crates/waml/tests/analysis_layering.rs`:

1. **The pipeline stays specialization-agnostic.** `src/analysis.rs` may name
   `crate::diagnostic`, `crate::okf`, `crate::source` -- its substrate and
   below -- plus exactly `uml::Analysis` and `uml::analyze`. A third UML item
   fails the test. That is the precise line between "composes a specialization"
   and "knows about one", which is the hub the audit named.
2. **A specialization never re-enters the pipeline.** No shipped file under
   `src/uml/` may name `prepare_candidate`,
   `prepare_candidate_with_markdown_updates`, `analyze_okf`, or
   `PreviousAnalyses`. Vocabulary yes; entry points no. (Tests under `uml/`
   *do* call `prepare_candidate` to build fixtures -- four of them -- which the
   production-only truncation permits, exactly as `okf_layering.rs` permits it.)

Both tests were verified to fail against injected violations, not merely to
pass: adding `crate::model::ElementType` and `crate::uml::Projection` to
`analysis.rs`, and a `use crate::analysis::prepare_candidate;` to
`uml/lower.rs`, produced three correctly-located failures.

This is the `okf_layering.rs` bargain. That test exists because the previous
layering violation this repository shipped -- `okf::Index` embedding a view
type -- was invisible precisely because the rule lived in a header comment. A
rule that is checked is worth more than a crate boundary that is merely drawn,
and it costs 200 lines instead of 35,000.

## If the split is ever wanted anyway

It is cheap to *start*, and that is worth recording, because the reason to
refuse it is the SCC, not the file. `analysis.rs` is already sorted
vocabulary-first: everything from `DocumentVersion` (line 43) through
`validate_disjoint_claims` (line 556) is vocabulary, and
the composition root begins at `PreviousAnalyses` (line 611) and runs to the
end. A prefix cut at line ~600 is the whole module move, with one straggler --
`single_text_change` (line 1448, `pub(crate)`) -- to carry along.

What that move would *not* do is let the prefix become a crate, until
`uml <-> edit`, `uml -> okf -> analysis`, `uml -> action -> edit` and
`model -> okf` are all gone too. Start there, not here.

## What the audit got wrong

* The prescription. "A crate split, not a module move" assumed the
  `analysis` <-> `uml` pair was the thing keeping them together. It is one of 20
  edges in an eight-module cycle, and removing it changes the cycle by nothing.
* "116 files couple to it" (assessment) / "114 files name `analysis::`"
  (ledger). True as a count and misleading as a finding: the overwhelming
  majority consume `prepare_candidate` and the pipeline facade, which is what a
  facade is for. Only five coupled for `DocumentId` -- the one item that really
  was misfiled, and it moved.
