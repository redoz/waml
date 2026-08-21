# What an incremental reparse actually costs

Audit finding **A40** claimed the incremental Markdown reparse path carries an
Omega(n) per-edit floor of "approximately 4 full-document passes", with no
benchmark anywhere to confirm or deny it.

This note records the measurement that settles it. The short version:

- **The Omega(n) claim holds.** Cost tracks document size, not edit size.
- **The "approximately 4 passes" figure is wrong** — it is about 23 — but the
  audit's *conclusion* was right, and its cost estimate was right by accident.
- **An incremental reparse costs ~2.6-3.6x a full parse from scratch.** Typing
  one character into a document is measurably *more* expensive than throwing
  the tree away and reparsing the whole thing.
- The actual parsing in an incremental reparse is **1.6% of its runtime**.
  Everything else is edit-independent bookkeeping.

## How to re-run it

```
cargo bench -p waml-syntax --bench markdown_reparse
```

Source: `crates/waml-syntax/benches/markdown_reparse.rs`. Dependency-free
(`harness = false` plus `std::time::Instant`), matching the existing
`markdown_parse` bench.

**Release only.** `plan_window_reparse` keeps a `#[cfg(debug_assertions)]`
oracle that full-parses the document on every successful window reparse, so a
debug timing does not measure a slower version of this program, it measures a
different one. `cargo bench` gets this right; `cargo test` would not.

**Not a CI gate, on purpose.** Timing gates are flaky on shared runners, and
this repo's own dev boxes run several compiles concurrently. Read the
`ratio` and `us/KiB` columns, which are self-normalising, rather than the
absolute milliseconds.

### Reading the output

The corpus is the vendored GFM 0.29 specification
(`crates/waml-syntax/tests/fixtures/gfm-0.29/spec.txt`, 216,680 bytes) —
real long-form prose with frontmatter, nested headings, fenced code, tables and
link reference definitions. Prefixes are cut at a blank line before a heading so
each size is a well-formed document, never a severed fence.

| Column | Meaning |
|---|---|
| `outcome` | What the bridge actually did. A `FULL` row has a ratio near 1.0 *by construction* and says nothing about incremental cost. |
| `ratio` | incremental / full-parse-from-scratch, measured back to back. Below 1.0 is the win. |
| `us/KiB` | **The scaling verdict.** The edit is the same size in every row, so a genuinely incremental reparse would show this *falling* as the document grows. Flat means Theta(n). |

`no-op (floor)` submits an edit that changes nothing, against a freshly
allocated `SourceText`. It parses nothing at all, so whatever it costs is pure
bookkeeping — and it costs the same as a real edit.

## The scaling curve

`insert 1 char mid`, WAML_DEFAULT dialect, release, one machine, one sitting:

| bytes | best ms | us/KiB | inc/full |
|---:|---:|---:|---:|
| 8,393 | 12.35 | 1507 | 2.95 |
| 14,073 | 19.90 | 1448 | 2.70 |
| 30,441 | 35.06 | 1179 | 2.84 |
| 57,086 | 67.82 | 1216 | 3.11 |
| 100,438 | 139.76 | 1425 | 3.51 |
| 216,680 | 339.60 | 1604 | 3.63 |

25.8x the document, 27.5x the time: a log-log slope of 1.07. The `us/KiB`
column is flat across the whole range. For a fixed one-byte edit, that is
Theta(n) — the incremental path is doing work proportional to the document, and
the only thing "incremental" buys is a constant factor, which is *above* one.

The `no-op (floor)` row is within 1% of `insert 1 char mid` at every size. One
hundred percent of the cost is independent of the edit.

## Where the time goes

Attribution at 100,438 bytes, one-character insert, ~150 ms total, measured by
temporary instrumentation on each pass:

| ms | share | pass | file :: function |
|---:|---:|---|---|
| 53.2 | 35% | annotation transfer | `incremental.rs` :: `transfer_mapped_annotations` |
| 20.8 | 14% | query index rebuild | `markdown/snapshot.rs` :: `queries` |
| 19.5 | 13% | subtree restore | `markdown/reparse.rs` :: `restore_unchanged_subtrees` |
| 15.1 | 10% | island identity carry | `markdown/reparse.rs` :: `preserve_unchanged_island_identities` |
| 11.1 | 7.4% | green rebase | `incremental.rs` :: `rebase_unchanged_green`, over every root child |
| 7.5 | 5.0% | projection, final | `markdown/projection.rs` :: `from_tree` |
| 6.8 | 4.5% | projection, discarded | `markdown/projection.rs` :: `from_tree`, from `plan_window_reparse` |
| **2.45** | **1.6%** | **the actual parse** | **`shell.rs` :: `parse_window`** |
| 2.2 | 1.5% | shared-green statistic | `incremental.rs` :: `count_shared_source_independent_greens` |
| 1.8 | 1.2% | inline owner re-stamp | `markdown/reparse.rs` :: `restamp_inline_owners` |
| 1.24 | 0.8% | source verification | `incremental.rs` :: `verify_exact_source` |
| 1.09 | 0.7% | write-back check | `markdown/snapshot.rs` :: `tree.write_to_string()` |
| 0.87 | 0.6% | shell scans, old + new | `markdown/mod.rs` :: `shell_map` |
| 0.34 | 0.2% | reference maps, twice | `markdown/reference.rs` :: `MarkdownReferenceMap::from_tree` |
| 0.28 | 0.2% | source-backing check | `markdown/snapshot.rs` :: `source_backed_green_uses` |
| 0.006 | ~0 | change reconstruction | `incremental.rs` :: `changes_reconstruct` |

## The pass census

Counting individual full-document traversals rather than functions, a single
successful incremental reparse makes **23** of them (22 after the fix below).
Six are full tree *rebuilds*, allocating a fresh green node per node.

In `reparse_okf_markdown_with_structure` (`incremental.rs`):

1. `shell_map(new_text)` — block scan of the new source
2. `verify_exact_source` — green walk of the old tree
3. `changes_reconstruct` — prefix and tail comparison of the whole source

In `plan_window_reparse` (`incremental.rs`):

4. `shell_map(old)` — block scan of the old source
5. `change_may_affect_reference_use` -> `MarkdownReferenceMap::from_tree`
6. `window_reparse_may_lose_reference_resolution` -> `MarkdownReferenceMap::from_tree`
   — the *same* map as (5), rebuilt from scratch
7. `rebase_unchanged_green`, looped over every root child — deep **rebuild**
8. `transfer_mapped_annotations` -> `collect_occurrences(previous)` — red walk
9. `transfer_mapped_annotations` -> `collect_occurrences(candidate)` — red walk
   **(dead; removed, see below)**
10. `transfer_mapped_annotations` -> `rebuild` — green **rebuild**
11. `count_shared_source_independent_greens` -> `collect` — green walk
12. `count_shared_source_independent_greens` -> `count` — green walk
13. `projection::from_tree` — red walk, discarded but for an island count

In `reparse_markdown` (`markdown/snapshot.rs`):

14. `restore_unchanged_subtrees` -> `collect_reusable` — green walk
15. `restore_unchanged_subtrees` -> `restore_candidate_node` — green **rebuild**
16. `preserve_unchanged_island_identities` -> `collect_island_identities` — red walk
17. `preserve_unchanged_island_identities` -> `collect_island_replacements` — red walk
18. `restamp_inline_owners` -> `restamp_node` — green **rebuild**
19. `projection::from_tree` — red walk, on the final tree
20. `snapshot::from_tree` -> `write_to_string` — green walk plus a full String build
21. `snapshot::from_tree` -> `source_backed_green_uses` — green walk
22. `snapshot::from_tree` -> `queries` -> `collect_queries` — red walk
23. `snapshot::from_tree` -> `queries` -> `collect_spans` — red walk

Plus three full sorts inside `queries` and six inside `projection::from_tree`.

## What was fixed

Pass (9) was **provably dead**: `collect_occurrences` was run over the whole
candidate red tree into a `candidate_occurrences` map that nothing ever read.
`rebuild` walks the candidate green tree itself and re-derives each occurrence
ordinal as it goes. A write-only `path: &mut Vec<u32>` in `rebuild` went with
it.

`transfer_mapped_annotations` was the single largest item in a reparse, and this
removed one of its three traversals:

| | share of total reparse |
|---|---|
| before | 32.5% (100 KiB and 216 KiB, three runs) |
| after | 22.5% |

End to end, `insert 1 char mid` improved from 1507 to 1363 us/KiB at 8 KiB and
from 1604 to 1411 us/KiB at 216 KiB — about 10%, matching the pass-share
arithmetic. Ratios against a full parse fell from 2.95 to 2.61 and from 3.63 to
2.77.

Shares are quoted rather than absolute milliseconds because the measuring box
had other compiles running; a within-run share is load-independent, a wall clock
is not.

## Why the rest of the floor is structural

The remaining 22 passes are not oversights. Each is load-bearing:

- **Absolute offsets in the green tree.** `GreenText::SourceSlice` stores an
  owning `SourceText` handle and an absolute `TextRange`. Any edit changes the
  document's `Arc<String>` identity, so *every* token in the document must be
  rebuilt to point at the new source — even tokens before the edit, whose
  offsets did not move. That is pass (7), and it is the reason a no-op edit
  costs as much as a real one. Rowan avoids this by storing widths relative to
  the parent, which makes an untouched subtree reusable by pointer. Adopting
  relative offsets would remove passes (7), (15) and (18) and shrink (2), (20)
  and (21) to nothing — but it is a rewrite of the green layer, not a patch.

- **Three metadata-migration passes doing the same job.**
  `transfer_mapped_annotations`, `restore_unchanged_subtrees` and
  `preserve_unchanged_island_identities` each rebuild the entire tree to carry
  one class of metadata from old to new, keyed by translated range. They run in
  sequence, and the second and third partly undo the deep copy the first made.
  Fusing them into one traversal is tractable and would be the next real win;
  it is also delicate, since this path had four correctness defects fixed on
  2026-08-21 (`5006b721`, `5dbb73c2`, plus the seeds in
  `crates/waml-syntax/proptest-regressions/incremental/properties.txt`).

- **Derived indexes are rebuilt, never patched.** `projection::from_tree` and
  `snapshot::queries` recompute the structure map and the link/image/span
  indexes from the whole tree on every revision. Making them incremental means
  giving them change-range awareness, which the tree already carries as
  `affected_ranges`.

- **Invariant checks that run in production.** `verify_exact_source`,
  `changes_reconstruct`, `write_to_string` and `source_backed_green_uses`
  together cost ~1.8% here. They are cheap and they are what keeps the bridge
  honest; they are not where the money is.

Also confirmed, and part of the same finding: **multi-block edits silently fall
back to a full parse.** In the bench, `paste ~4 KiB mid` falls back at every
size (`MarkdownContainerBoundaryChanged`), `delete across block` falls back from
30 KiB up, and at 100 KiB even a single-character insert at end-of-document
fell back. The incremental path serves a narrower set of edits than its name
suggests.

## The honest summary

Today, `reparse_markdown` is a full parse plus about twenty extra traversals of
the document. Until the green layer stores relative offsets, no amount of
guard-tightening changes that: the floor is in the data structure, not in the
control flow. A caller who wants a fast edit on a large document is currently
better served by `parse_markdown`.
