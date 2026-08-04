# Issue 26 — Concept→path resolution scans the catalog per lookup

## Context

`analyze` (crates/waml/src/uml/analysis.rs:248) builds a concept-id→document
index exactly once, with a comment explaining why (analysis.rs:264-272,
landed in d30af731):

```rust
// Index the catalog by concept id once, instead of scanning every document
// per claimed concept (O(concepts × documents)).
let mut concept_documents: BTreeMap<String, DocumentId> = BTreeMap::new();
```

But the index is a local — it is dropped before `validate_declared_semantics`
(called at analysis.rs:675) and `declared_projection` (analysis.rs:676) run,
and both of those, plus `sequence.rs`, re-derive the same mapping with a
linear scan of `context.catalog.documents()` per concept lookup. Each scan
also recomputes `okf::id_of` on every document path it visits.

## Verdict evidence (verified 2026-08-04, worktree HEAD after 258e6392)

Linear scan sites, each O(documents) per call:

- crates/waml/src/uml/analysis.rs:986-992 — `validate_declared_semantics`, per declared concept (relationships pass)
- crates/waml/src/uml/analysis.rs:1059-1065 — same fn, instance-specification pass, per concept
- crates/waml/src/uml/analysis.rs:1151-1157 — same fn, inline-instances pass, per concept
- crates/waml/src/uml/analysis.rs:1244-1250 — same fn, member-groups pass, per concept
- crates/waml/src/uml/analysis.rs:1493-1499 — `declared_projection`, per declared concept
- crates/waml/src/uml/sequence.rs:200-209 — `path_for_concept`, called at:
  - sequence.rs:257 (`interaction_use_graph`, per declared concept)
  - sequence.rs:297 and sequence.rs:582 (per interaction-use target)

Aggregate cost is O(concepts × documents) per analyze, several times over —
precisely what the d30af731 comment says was being eliminated. Verdict:
**APPROVE**.

## Design decisions

1. **Index shape**: reuse the existing `concept_documents:
   BTreeMap<String, DocumentId>` built in `analyze`. All six sites only need
   the document *path*, so resolve `DocumentId → &Document` through
   `context.catalog.document(id)` at the use site, or — simpler — build the
   index as `BTreeMap<String, String>` (concept id → path string) once and
   pass `&BTreeMap<String, String>`. Chosen: **concept id → path `String`**,
   because every consumer wants a path and it avoids lifetime plumbing of
   `&Document` borrows through `sequence::lower` (which already carries eight
   arguments under `#[allow(clippy::too_many_arguments)]`).
   `analyze` itself still needs the `DocumentId`; keep building the id map
   and derive the path map from it in one pass (or store `(DocumentId,
   String)` values and keep one map).
2. **First-wins semantics preserved**: the existing index comments
   "First document wins on a duplicate id, matching the scan this replaces."
   `.entry(..).or_insert(..)` over `documents().iter()` iterates in
   `BTreeMap` id order; the removed scans iterate `.values()` of the same
   map, so first-wins order is identical. Keep the comment.
3. **Threading, not globals**: pass the map as an extra `&BTreeMap<String,
   String>` parameter down `validate_declared_semantics`,
   `declared_projection`, and `sequence::lower` →
   `interaction_use_graph` / interaction-use target resolution. Do not
   introduce a context struct in this change — the issue's optional
   `ValidationCtx` is a larger refactor; a single extra parameter matches
   the surrounding style (these fns already take `context`, `declared`,
   `diagnostics`).
4. **`path_for_concept` becomes a map lookup** (or is deleted and callers
   use `concept_paths.get(id)` directly). Delete it if nothing else calls
   it — prefer deletion over a one-line wrapper.
5. **Behaviour must be unchanged**: all sites fall back to
   `unwrap_or_default()` / `unwrap_or_else(|| target.clone())` on a miss;
   keep those fallbacks exactly.

## Tasks

### Task 1: Build a concept-id→path map in analyze and thread it to validate_declared_semantics

- In `analyze` (crates/waml/src/uml/analysis.rs:248), alongside the existing
  `concept_documents` index (analysis.rs:266-272), build
  `concept_paths: BTreeMap<String, String>` mapping
  `okf::id_of(document.path())` → `document.path().to_string()`, first
  document wins (same loop; one pass total).
- Change `validate_declared_semantics` (analysis.rs:976) to take
  `concept_paths: &BTreeMap<String, String>`; replace the four scans at
  analysis.rs:986-992, 1059-1065, 1151-1157, 1244-1250 with
  `concept_paths.get(concept.concept_id.as_str()).map(String::as_str).unwrap_or_default()`.
- Update the call at analysis.rs:675.
- Test: `cargo test -p waml` — existing UML analysis fixtures cover these
  diagnostics paths; no behaviour change expected, so no new fixture, but
  confirm the declared-semantics diagnostic tests still pass unchanged.

### Task 2: Thread the map through declared_projection

- Change `declared_projection` (analysis.rs:1476) to take
  `concept_paths: &BTreeMap<String, String>`; replace the scan at
  analysis.rs:1493-1499 with a map lookup (`.cloned().unwrap_or_default()`).
- Update the call at analysis.rs:676.
- Test: `cargo test -p waml` — projection snapshot/fixture tests must be
  byte-identical (first-wins order is preserved per design decision 2).

### Task 3: Replace path_for_concept in sequence.rs with the threaded map

- Change `sequence::lower` (crates/waml/src/uml/sequence.rs:381) to take
  `concept_paths: &BTreeMap<String, String>` (it already carries
  `#[allow(clippy::too_many_arguments)]`); update the sole call site in
  `declared_projection` (analysis.rs:1580).
- Thread the map into `interaction_use_graph` (sequence.rs:250) and update
  its call at sequence.rs:507.
- Replace the three `path_for_concept` calls (sequence.rs:257, 297, 582)
  with map lookups, preserving the existing fallbacks
  (`else { continue }` at 257; `unwrap_or_else(|| target.clone())` at
  297/582 becomes `.cloned().unwrap_or_else(|| target.clone())`).
- Delete `path_for_concept` (sequence.rs:200-209).
- Gate: `cargo test --workspace` plus the vscode extension test/lint/build,
  per repo gate convention. Sequence-diagram fixtures must be unchanged.
