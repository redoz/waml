# Issue 21 — Incremental reference-use scan drops the rest of the line

## Context

`crates/waml-syntax/src/markdown/reparse.rs` contains `reference_labels(line)`
(lines 113–143), the line scanner shared by two conservative guards:

- `change_may_affect_reference_use` (line 52) — forces a named full-parse
  fallback when an edited line uses a label defined in the old tree.
- `window_reparse_may_lose_reference_resolution` (line 77) — forces fallback
  when a window line uses a label whose definition lies outside the window.

Both guards exist because a shell-window reparse resolves reference links only
against definitions inside the window; a missed use means the incremental
splice can resolve links differently from a full parse.

## Verdict evidence (APPROVE)

`reparse.rs:132-133`, current worktree HEAD (2fdb5ff9):

```rust
} else if after.starts_with('(') {
    ("", after.len())
```

When a bracket pair is followed by `(`, the branch treats it as an inline link
(correct: no reference label) but sets `consumed = after.len()`, so
`rest = &after[consumed..]` (line 140) becomes empty and the loop ends. For a
line like `[a](x) see [b][id]`, the `[b][id]` use is never scanned, so neither
guard fires. If `[id]:` is defined outside the edited window (or its
definition line is edited), the incremental result diverges from the full
parse. The debug oracle compares island counts, not link resolution, so it
cannot catch the divergence; `assert_snapshot_matches_full_oracle` in
`tests/markdown_incremental.rs` does compare `reference_destinations`, but no
existing fixture puts an inline link before a reference use on one line (no
test line matches this shape — grep confirms).

Recent fix `ee43301c fix(waml-syntax): stop incremental reparse losing
reference resolution` added the guards but did not touch this branch; the bug
is live.

## Design decisions

- **Consume only the inline-link tail, not the whole line.** In the
  `after.starts_with('(')` branch, find the matching `)` and set
  `consumed = close_paren_index + 1`. Balanced-paren tracking (CommonMark
  allows nested balanced parens in destinations) is preferred; if no closing
  `)` exists on the line, falling back to consuming the whole remainder is
  acceptable and stays conservative in the safe direction only if we instead
  consume just the `(` (consumed = 1) — choose **consumed = 1 on unbalanced**
  so later `[` pairs are still scanned. Over-scanning can only produce false
  positives (extra full-parse fallbacks), which the module doc explicitly
  declares acceptable; under-scanning is the correctness bug.
- **Keep the scanner heuristic.** No attempt to fully model CommonMark inline
  precedence here; the guard must only never miss a real reference use.
- **Test at both levels**: a unit-style test of the guard behaviour via a
  public incremental edit, plus an oracle fixture asserting incremental ==
  full including `reference_destinations`.

### Task 1 — Fix `reference_labels` inline-link consumption

- File: `crates/waml-syntax/src/markdown/reparse.rs`, lines 132–133.
- Replace `("", after.len())` with logic that scans `after` from the `(`:
  track paren depth, stop at the balancing `)`; on success
  `("", close + 1)`; if the line ends before balance, `("", 1)` so the scan
  resumes right after the `(` and later bracket pairs are still seen.
- Confirm `rest = &after[consumed..]` (line 140) keeps the remainder
  (`" see [b][id]"`) and the next iteration extracts `id`.
- Add a doc comment on the branch stating the conservative direction.

### Task 2 — Regression fixtures: incremental == full with mixed inline + reference uses

- File: `crates/waml-syntax/tests/markdown_incremental.rs`.
- New test `inline_link_before_reference_use_still_forces_reference_fallback`:
  - old: `"[id]: /one\n\nsee [a](x) then [b][id]\n"`
  - new: `"[id]: /two\n\nsee [a](x) then [b][id]\n"` (edit only the
    definition line via a `TextChange`).
  - Drive through the public incremental update API as the neighbouring tests
    do (`definition_change_updates_non_contiguous_reference_dependents`,
    line 226 pattern) and assert
    `reference_destinations(&update.snapshot) == vec!["/two"]` and equals the
    full-parse oracle (`assert_snapshot_matches_full_oracle`).
- Second case editing an untouched-window scenario: definition line edited
  while `see [a](x) then [b][id]` sits in a separate paragraph, mirroring the
  issue text.
- Add the shape `[a](x) [b][id]` to the deterministic case list in
  `deterministic_incremental_cases_match_the_full_parse_oracle` (line 499) if
  the list format admits it.

### Task 3 — Property coverage and gate

- Check `crates/waml-syntax/tests/properties.rs` incremental-vs-full property
  generators: if line generation cannot produce an inline link followed by a
  reference use on one line, extend the generator alphabet minimally so this
  class is reachable. Do not commit `properties.proptest-regressions`.
- Gate: `cargo test -p waml-syntax` then `cargo test --workspace`.
- Verify the new tests fail on the unfixed scanner (revert Task 1 locally,
  observe red) before finalising.
