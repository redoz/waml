# Issue 24 — okf::Bundle linear-scan accessors make per-edit work quadratic

## Context

`okf::Bundle` stores concepts/indexes/logs/directories as `Vec`s and every accessor is a
linear `iter().find()` (crates/waml/src/okf.rs:279-299). These accessors are called inside
loops on the per-edit path (`okf::shell::derive` runs on every accepted edit), turning
per-edit cost quadratic in bundle size:

- **Directory build** (crates/waml/src/okf/shell.rs:241-266): for each address, filters
  *all* addresses for children and *all* concepts for direct members — O(A·(A+C)).
- **default_member_order** (shell.rs:527-546): `concepts.iter().find()` per member id —
  O(C) per concept, called once per directory.
- **Authored-order merge** (shell.rs:273-283): `default_order.contains(&member)` and
  `members.contains(&member)` inside loops — O(M²) per authored index.
- **index_md.rs:88,98** (`reindex_source`): calls `parsed.index(member)` /
  `parsed.concept(member)` per member — O(N) scan per member via the Bundle accessors.
- **SourceBundle::document_by_concept_id** (crates/waml/src/source.rs:357-361): scans all
  documents per lookup.
- **Model::node** (crates/waml/src/model.rs:1143-1145): same scan pattern.

## Verdict evidence (verified 2026-08-04)

- okf.rs:279-299 — all four accessors are `iter().find()`. Confirmed.
- Sorted-at-construction claim: **holds for Bundle**. shell.rs:238 sorts `concepts` by id,
  :239 sorts `logs` by directory, :266 sorts `directories` by address; `indexes` are pushed
  while iterating the already-sorted `directories` (shell.rs:268-296), so they are sorted by
  directory address too. `DirectoryAddress` is `#[derive(... Ord ...)] struct
  DirectoryAddress(String)` (okf.rs:20-22), so its `Ord` agrees with `as_str()` comparison.
- **Correction to the issue**: `Model.nodes` is NOT sorted — nodes are pushed in document
  iteration order (uml/analysis.rs:1727, 1860) and that order may be meaningful downstream.
  `binary_search_by` is NOT safe for `Model::node` without an ordering audit. Excluded from
  the binary-search change; see Task 5.
- `SourceBundle` already owns `by_path: BTreeMap<BundlePath, usize>` (source.rs:235) and
  `BundlePath::concept_id()` is `strip_suffix(".md")` (source.rs:57-59), so
  `document_by_concept_id(id)` is exactly a `by_path` lookup of `"{id}.md"` — no new index
  structure needed.

**VERDICT: APPROVE** (with the model.rs sub-item narrowed as above).

## Design decisions

1. **binary_search_by, not HashMap**: the vectors are already sorted at construction and
   the sort is the existing serialization/derive invariant; binary search adds no state to
   keep coherent and keeps `Bundle` `Clone`/`serde` shape unchanged.
2. **Guard the invariant**: add `debug_assert!(is_sorted)` (or a debug-only check in the
   accessors' construction site, `derive`) so a future unsorted construction fails loudly in
   tests instead of silently returning `None`.
3. **Directory build**: replace the per-address filters with two single passes — group
   addresses by `parent()` into a `BTreeMap<DirectoryAddress, Vec<DirectoryAddress>>`, and
   group sorted concepts by `concept_parent` the same way. O((A+C) log A) total.
4. **Merge sets**: authored-order merge keeps output order (authored-first) but does
   membership via `BTreeSet<&str>` built once from `default_order`, plus a seen-set.
5. **document_by_concept_id**: use `by_path` with a constructed `BundlePath("{id}.md")`.
   Must go through the existing `BundlePath` constructor/validation, not a raw tuple build.
6. **Model::node stays linear** for now; changing node order is out of scope and risky
   (draw/serialization order). Record as a follow-up if profiling shows it hot.

## Tasks

### Task 1: Bundle accessors use binary search

- File: crates/waml/src/okf.rs (impl Bundle, lines ~279-299).
- `concept`: `self.concepts.binary_search_by(|c| c.id.as_str().cmp(id)).ok().map(|i| &self.concepts[i])`.
- `index`, `log`, `directory`: same shape keyed on `directory.as_str()` / `address.as_str()`.
- Add debug-only sortedness asserts where the Bundle is assembled (shell.rs:297-302) for all
  four vectors (indexes by directory address).
- Tests: unit tests in okf.rs or okf/shell.rs asserting accessor hit/miss for first, middle,
  last, and absent keys on a multi-directory fixture; existing okf tests must stay green.

### Task 2: Directory build grouped in single passes

- File: crates/waml/src/okf/shell.rs:241-266.
- Build `children_by_parent` and `concepts_by_parent` maps in one pass each, then assemble
  `Directory` entries from the maps. Preserve exact current ordering (children sorted,
  concepts sorted) — output must be byte-identical to the old code.
- Tests: existing shell/derive tests; add one fixture with nested directories asserting
  child_directories and concepts contents/order unchanged.

### Task 3: default_member_order and authored merge drop the scans

- File: crates/waml/src/okf/shell.rs:527-546 and :273-283.
- `default_member_order`: replace `concepts.iter().find()` with `binary_search_by` on the
  sorted `concepts` slice (keep the existing `expect("directory concept exists")` semantics).
- Authored merge: build `BTreeSet<&str>` from `default_order` for membership; track pushed
  members in a `BTreeSet<&str>` instead of `members.contains`. Output order unchanged.
- Tests: existing authored-index tests (shell.rs test mod) must pass unchanged; add a case
  where authored order contains duplicates and stale members to pin the dedup behavior.

### Task 4: SourceBundle::document_by_concept_id via by_path

- File: crates/waml/src/source.rs:357-361.
- Construct the path `format!("{id}.md")`, parse with `BundlePath::parse`, and look up
  `by_path`; on parse failure return `None`.
- Before relying on this, confirm the equivalence: a concept id must never produce a path
  that `BundlePath::parse` rejects but the old linear scan would have matched. Verify against
  the `BundlePath` validation rules and add tests for the edge cases (id with a leading `/`,
  empty id, id containing a path separator). If the equivalence does NOT hold, stop and
  reconsider the approach — do not paper over it with a fallback scan, which would reinstate
  the linear cost this task exists to remove.
- Tests: hit/miss/invalid-id cases in source.rs tests.

### Task 5: Record Model::node decision

- File: crates/waml/src/model.rs:1143 — leave the linear scan, add a short comment noting
  nodes are in authored/document order (uml/analysis.rs:1727,1860) so binary search is not
  applicable; revisit only with profiling evidence.
- No test changes.

## Gate

`cargo test --workspace` plus the vscode extension test/lint/build, per project gate.
