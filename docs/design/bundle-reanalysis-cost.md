# What a keystroke costs in a bundle of N documents

Audit finding **A14** claimed that "every edit triggers bundle-wide semantic
reanalysis — the project's own open P2 and the ceiling on document scale", with
no number anywhere saying where that ceiling actually is.

This note records the measurement that settles it, and the three fixes it
found. The short version:

- **The claim holds.** Per-edit cost is Θ(N) in the number of documents in the
  bundle, and it is *edit-independent*: an edit that changes zero bytes costs
  the same as a real one.
- **The syntax-reuse machinery works, and it is not the problem.** On a
  one-character edit in a 200-document bundle, 199 of 200 Markdown snapshots
  and 121 of 122 UML island trees are reused *by pointer*. Everything
  downstream of the syntax is then rebuilt for all 200 documents anyway.
- **The wall was at ~65 documents** for a 16 ms (60 fps) keystroke budget and
  **~200 documents** before an edit takes the 100 ms that reads as lag.
- **Three fixes moved it to ~215 and ~1000 documents** — 7.0x at 200 documents,
  6.4x at 500 — without adding a cache that can go stale. One of them was a
  full extra `parse_markdown` of every untyped document, per keystroke.
- It is still Θ(N). The remaining constant is smaller, not gone. What removing
  it would take is spelled out at the end.

## How to re-run it

```
cargo bench -p waml --bench bundle_reanalysis
```

Source: `crates/waml/benches/bundle_reanalysis.rs`. Dependency-free
(`harness = false` plus `std::time::Instant`), matching
`waml-syntax`'s `markdown_reparse`.

**Release only.** A debug build measures a different program.

**Not a CI gate, on purpose.** Timing gates flap on shared runners, and this
repo's dev boxes run several compiles concurrently. Read the `us/doc` column,
which is self-normalising, rather than the absolute milliseconds.

### The corpus

`docs/waml`, this repository's own architecture bundle: 180 documents,
282,411 bytes of real WAML — frontmatter, title headings, `## Attributes` and
`## Relationships` islands, relative cross-document links, five document types
plus 76 documents with no `type:` at all (index pages, contracts, notes).
Bundles larger than the corpus replicate it, each replica under its own
top-level directory (`r0/`, `r1/`, …) so the relative links still resolve
inside each replica.

Two caveats on reading the table:

- Rows below 180 documents are a **prefix** of the sorted corpus, which skews
  small and class-heavy. The 200 and 500 rows hold the full mix. Take the slope
  from the 200/500 pair; the 10 and 50 rows are there for shape, not for
  extrapolation.
- The edit lands at the end of the first member line under an `## Attributes`
  or `## Relationships` heading — inside a real UML island, so it genuinely
  invalidates that document's island tree. Typing into prose lets every island
  in the bundle be reused and flatters the pipeline.

| Column | Meaning |
|---|---|
| `best ms` | Cost of one keystroke: apply the edit, then `prepare_candidate` with the previous analysis offered for reuse. |
| `cold ms` | The same bundle analysed with `previous: None`. Not the keystroke cost — the reference for how much reuse buys. |
| `us/doc` | **The scaling verdict.** The edit is identical in every row, so a pipeline that reused what the edit cannot have touched would show this *falling* as N grows. Flat means Θ(N). |
| `no-op (floor)` | An edit that changes no bytes, against a fresh `SourceText`. Pure per-edit bookkeeping. |

## The scaling curve

Release, one machine, one sitting, `insert 1 char` inside an island:

| documents | bytes | before, ms | after, ms | speedup | after us/doc |
|---:|---:|---:|---:|---:|---:|
| 10 | 12,681 | 1.82 | 1.29 | 1.4x | 129 |
| 50 | 48,052 | 6.88 | 3.28 | 2.1x | 66 |
| 200 | 301,123 | 101.66 | 14.45 | 7.0x | 72 |
| 500 | 804,853 | 289.96 | 45.03 | 6.4x | 90 |

Cold analysis of the same bundles, for reference: 6.57 → 6.07 ms, 23.39 →
19.76, 238.58 → 152.02, 661.43 → 424.92.

Absolute milliseconds move with machine load — this repo's dev boxes run
several compiles at once, and a loaded run of the same build showed 35 ms at
200 documents against 14 ms quiet. The `edit/cold` ratio held at 0.07-0.10
across both. Compare ratios and `us/doc` between runs; compare milliseconds
only within one.

`us/doc` is flat across the range in both columns. For a fixed one-character
edit, that is Θ(N): the work tracks the bundle, not the edit. The `no-op
(floor)` row is within a few percent of `insert 1 char` at every size, before
and after — essentially **all** of the cost is independent of what was typed.

The reuse ratio makes the same point from the other side. A keystroke costs
0.10-0.11 of a cold analysis after the fixes (0.42-0.45 before). Reuse is
real, and it is a constant factor, not a change of order.

### Where the wall is

Taking the per-document slope from the 200/500 pair, which share a content mix:

| | 16 ms (60 fps) | 100 ms (reads as lag) |
|---|---:|---:|
| before | ~65 documents | ~200 documents |
| after | ~215 documents | ~1000 documents |

`docs/waml` itself is 180 documents. Before these fixes, this repository's own
architecture bundle was already past the point where typing in it drops frames,
and within sight of the point where it feels laggy.

## The reuse census

Printed by the bench, for a one-character edit inside an island of one document
in a 200-document bundle:

```
markdown snapshots reused: 199/200 documents
UML island trees reused:   121/122 islands
```

That is what the machinery named in the finding actually reuses:

- **`PreviousAnalyses`** (`analysis.rs`) carries the previous `OkfAnalysis` and
  `uml::Analysis` in by shared reference.
- `analyze_okf_inner` (`analysis.rs`) reuses a `DocumentVersion` whenever the
  source text `Arc` is pointer-identical, and reuses the previous
  `MarkdownSyntaxSnapshot` whenever the revision then matches. Only the edited
  document reparses. **This works.**
- `recover_island_tree` (`uml/analysis.rs`) reuses the previous island's
  `SyntaxTree` whenever its source and content ranges translate unchanged
  across the edit, via `ChangeMap::translate_unchanged`. **This works.**
- `analysis_metadata` (`uml/analysis.rs`) reuses `Diagram` projections for
  diagrams that do not depend on an affected concept, and `ProjectionFreshness`
  marks islands whose projection is retained-but-stale.

And here is what it recomputes anyway, for every document, on every keystroke —
the gap the finding names. Each of these is proportional to N, not to the edit:

| file :: function | what it redoes per document |
|---|---|
| `okf/shell.rs` :: `validate` | `exact_tree_source` writes the whole green tree out and compares it to the source; `validate_structure`; `shell_fields` reparses the frontmatter node; a `SourceSlice` for the body |
| `okf/shell.rs` :: `project` -> `project_concept` | frontmatter promotion, `first_h1`, `citation_start`, `extract_links`, `extract_legacy_sources`, index parsing — a fresh `Concept` for every document |
| `uml/analysis/validate.rs` :: `validate_document_types` | for every untyped or unknown-typed document, `type_scalar_range` -> `frontmatter.rs` :: `validated_frontmatter_syntax`, which **parsed the entire document from scratch again** |
| `uml/analysis.rs` :: `analyze`, the `for concept in claimed` loop | per document: `syntax::compose_full_from_islands` rebuilds the composite UML tree; **fifteen** `syntax_util::items` / `attributes` full red-tree traversals; the whole `DeclaredConcept` via `extract::declared_*`; `translate_layout_diagnostics`; `translate_parser_diagnostics` |
| `uml/analysis/validate.rs` :: `validate_declared_semantics` | every relationship of every concept, re-resolved |
| `uml/analysis/project.rs` :: `declared_projection` | the whole `Model` projection, from every document's declarations |
| `uml/analysis.rs` :: `trace_indexes` | every flow edge of the whole projection |
| `uml/analysis.rs` :: `analysis_metadata` | every Markdown document, every island, every diagram, twice over `projection.diagrams` |
| `uml/highlight.rs` :: `build_code_syntax` | every Markdown document and every island tree |

The syntax layer is incremental. Everything the syntax layer *feeds* is not.

## Where the time went

Attribution at 200 and 500 documents, one-character insert, by temporary
instrumentation on each phase. Percentages are of the measured per-edit total,
so they do not sum to 100 — the remainder is catalog bookkeeping and allocation
outside any named phase.

**Before**, 200 documents, 101.7 ms:

| ms | share | phase | file :: function |
|---:|---:|---|---|
| 59.8 | 58.8% | type-scalar reparse | `validate.rs` :: `type_scalar_range` |
| 23.4 | 23.0% | 15 tree walks x N documents | `syntax_util.rs` :: `items`, `attributes` |
| 7.4 | 7.3% | green-tree source check | `okf/shell.rs` :: `validate` |
| 5.3 | 5.2% | declared-concept build | `uml/analysis/extract.rs` :: `declared_*` |
| 2.6 | 2.5% | Markdown parse/reuse loop | `analysis.rs` :: `analyze_okf_inner` |
| 2.4 | 2.3% | island composition | `uml/syntax` :: `compose_full_from_islands` |
| 0.9 | 0.9% | model projection | `uml/analysis/project.rs` :: `declared_projection` |
| 0.8 | 0.8% | OKF concept projection | `okf/shell.rs` :: `project` |

**After**, 200 documents, 14.4 ms (and 500 documents, 45.0 ms):

| ms @200 | share | ms @500 | phase |
|---:|---:|---:|---|
| 6.72 | 40.1% | 18.73 | declared-concept build |
| 2.51 | 15.0% | 0.40 | Markdown parse/reuse loop |
| 1.79 | 10.7% | 6.16 | island composition |
| 1.69 | 10.1% | 4.72 | fused tree walk (`items_by_kind`) |
| 1.25 | 7.5% | 3.91 | green-tree source check |
| 0.84 | 5.0% | 4.01 | model projection |
| 0.69 | 4.1% | 1.94 | OKF concept projection |
| 0.32 | 1.9% | 1.07 | `validate_document_types` |
| 0.16 | 1.0% | 0.43 | `validate_declared_semantics` |
| 0.08 | 0.5% | 0.37 | `build_code_syntax` |
| 0.03 | 0.2% | 0.14 | island recovery |
| 0.02 | 0.1% | 0.10 | `analysis_metadata` |

(The Markdown loop's 2.51 ms at 200 documents against 0.40 ms at 500 is the
edited document falling back to a full Markdown parse in one case and taking
the incremental path in the other; it is a property of that one document, not
of N.)

## What was fixed

### 1. `validate_document_types` reparsed every untyped document

`type_scalar_range` needed one thing: the source range of the `type:` scalar,
so an "unknown UML type" diagnostic could point at it. It got there through
`frontmatter::inspect_frontmatter_string_scalar`, which takes a `&str` and
therefore calls `parse_markdown` on the **whole document**. That ran for every
document whose type is unknown or obsolete — which includes every document with
no `type:` key at all, 76 of the 180 in `docs/waml`.

So a keystroke anywhere in the bundle full-parsed 42% of the bundle, to
re-derive diagnostics that had not changed.

`validated_frontmatter_syntax` is now split, and
`inspect_parsed_frontmatter_string_scalar` reads the same bytes through the
same rules off the `MarkdownSyntaxSnapshot` the shell already produced.
`frontmatter.rs` :: `parsed_and_reparsed_frontmatter_scalars_agree` pins the
two readers to the same answer across typed, quoted, untyped, non-scalar,
unfenced and frontmatter-less documents.

178 ms -> 1.1 ms at 500 documents.

### 2. Fifteen tree walks per document became one

`analyze` pulled fifteen node kinds out of each document's composed UML tree
with fifteen separate calls to `syntax_util::items`, each a full red-tree
traversal, for every document, on every keystroke.

`items_by_kind` does it in one traversal. The subtlety is that `items` stops
descending into a node once that node matches the kind it wants, but keeps
descending for every *other* kind — so a naive "stop at the first match" fusion
would silently drop nested declarations. The fused walk carries a per-kind
`seeking` flag down each branch and suppresses only the branch for the kind
that matched. `syntax_util.rs` ::
`fused_kind_walk_matches_one_walk_per_kind` pins the buckets node-for-node, in
order, against one `items` call per kind, over eight parser fixtures including
a recovery tree.

`direct_section_items` (member groups) is deliberately not fused: it stops at
the first `MembersSection` and takes only its direct children, which is a
different walk.

23.4 ms -> 1.7 ms at 200 documents.

### 3. The shell no longer re-verifies documents it already verified

`okf::shell::validate` runs `exact_tree_source` on every document — writing the
whole green tree out and comparing it byte for byte to the source. It is a
real invariant check and it belongs in production; it just does not need to run
twice on the same bytes.

`derive` now takes the previous `OkfAnalysis`, and skips the check for a
document whose `DocumentVersion` **and** `MarkdownSyntaxSnapshot` both arrive by
pointer from an analysis in which the same check already passed. This is reuse
by pointer identity, not a cache key: a changed document gets a new `Arc` on
both sides, so there is nothing that can go stale.
`analysis.rs` :: `shell_projection_rejects_tree_source_mismatch` now asserts
the rejection **with** a previous analysis offered, not only without one.

7.4 ms -> 1.3 ms at 200 documents.

## Why the rest is structural

The remaining Θ(N) is not an oversight either. The largest item, at 40% of what
is left, is building a `DeclaredConcept` for every document
(`uml/analysis/extract.rs`), and memoising it per document would be **wrong**:

```rust
// extract.rs :: declared_attribute
context.okf.concept(&resolved).filter(|target| crate::uml::recognizes(target))
```

A type reference resolves against the *bundle*. Adding, deleting or retyping a
document elsewhere changes whether `[Money](./money.md)` resolves, and so
changes the `DeclaredConcept` of a document nobody touched. A cache keyed on
the document's island trees alone would go stale in exactly the way that is
worst: silently, and only for cross-document references.

The correct version of that fix is a per-document memo keyed on
`(island tree identities, the set of recognised concept ids)` — the second half
changing only when documents are added, removed or retyped, never on an
in-document edit. That would collapse island composition, the fused walk and
the declared build (64% of what remains) to nothing for untouched documents,
and would turn the per-edit cost from Θ(N) into Θ(edited document + dependents).
It is a real design change with a real staleness surface, and this pipeline had
four correctness defects fixed on 2026-08-21; it wants its own pass, its own
invalidation tests, and probably a debug-mode oracle that recomputes and
compares, in the shape `plan_window_reparse` already uses.

The rest of the residue:

- **`declared_projection` and `build_code_syntax` are rebuilt, never patched.**
  Same story as `projection::from_tree` in `incremental-reparse-cost.md`:
  making them incremental means giving them change awareness, which
  `AffectedAnalysis` already computes and nobody downstream of it consumes for
  this purpose.
- **`okf::shell::project` re-derives every `Concept`** from frontmatter on
  every edit. It is now 4% of a keystroke; the same pointer-identity reuse that
  fixed `validate` would work here, but it needs `Concept` to be shareable
  (`Arc`), which is a wider change than this pass.
- **`shell_fields` still runs per document** even when validation is skipped,
  because `ShellDocument` is rebuilt each time. Folding it into the same reuse
  is the natural next increment.

## The honest summary

Typing one character into a WAML bundle re-derives the semantics of every
document in it. The syntax layer beneath is genuinely incremental and reuses
almost everything; the semantic layer above it throws that reuse away and
recomputes from scratch. These three fixes removed the work that was
*accidental* — a redundant parse, fourteen redundant traversals, and a
redundant invariant check — and bought about 7x. The work that is left is the
finding's real content: the analysis is a pure function of the whole bundle,
and until a `DeclaredConcept` can be reused across an edit, its cost will keep
tracking the bundle rather than the keystroke.
