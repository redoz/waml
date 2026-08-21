# Issue 28 — Guards and lowerers: single authority instead of re-lexing

## Status — 2026-08-21: PARTIAL — tasks B, C and D are still outstanding

Triage verdict from the A39 planning-hygiene pass. This is real, unfinished
work, not stale paper. `2026-08-04-issue-triage-index.md` already re-scoped it
("Do not implement issue 28 as written") but nothing has been done since.

**Task A — frontmatter open-fence helper: SUBSUMED.** `frontmatter-yaml-alignment`
(`completed/2026-08-04-frontmatter-yaml-alignment.md`) extracted the shared
fence helper; `classify_frontmatter` is now a single definition at
`crates/waml-syntax/src/markdown/parser.rs:86`. Caveat recorded in the triage
index: the `markdown/mod.rs` shell-structure scan was outside the YAML plan's
file list — `crates/waml-syntax/src/markdown/mod.rs:228` still only *documents*
that it agrees with the parser.

**Task B — record the resolution in `BracketMatch`: NOT DONE.**
`crates/waml-syntax/src/markdown/inline.rs` still has `struct BracketMatch`
(line 262) with no `resolution` field, no `BracketResolution` enum, and
`parse_link` (line 922) still re-derives; the
`ParseError::StructuralInvariant` constructions the plan deletes are still
there (lines 384, 689). Drift between the two copies is still promoted to a
whole-document error.

**Task C — route okf and uml frontmatter reads through `parse_closed_syntax`:
NOT DONE.** Two independent extractors survive:
`crates/waml/src/okf/lower.rs:508 fn frontmatter_value` and
`crates/waml/src/uml/lower.rs:909 fn frontmatter_value`, the latter still
`split_once(':')`-based with its own `decode_scalar` (line 921).

**Task D — strengthen the debug oracle: NOT DONE.**
`crates/waml-syntax/src/incremental.rs:334` still compares
`oracle.structure.islands.len()` — the island *count*, not the tree.

**Before implementing:** re-verify B/C/D line numbers against `main`, as the
triage index instructs. The ordering constraints it records (this plan vs
issues 34, 35, 36) no longer bind — those all landed.


## Context

Three independent duplications where a consumer re-derives, from raw text, a fact
the parser already computed. Each pair has verified drift today or a
`StructuralInvariant` failure mode when it drifts tomorrow.

## Verdict evidence (verified 2026-08-04, worktree HEAD 2fdb5ff9)

### A. Frontmatter fence recognition — three copies, BOM drift is LIVE
- `crates/waml-syntax/src/incremental.rs:1290` `frontmatter_fences` — requires
  `source[start..open_end].trim() == "---"` (line 1305). No BOM handling.
- `crates/waml-syntax/src/markdown/parser.rs:93-95` `classify_frontmatter` —
  BOM-aware (`bom = usize::from(source.starts_with('\u{feff}')) * 3`).
- `crates/waml-syntax/src/markdown/mod.rs:183` — third BOM-aware copy in the
  shell-structure scan.
- **Empirically verified** with temporary probe tests in
  `incremental/low_level_tests.rs` (added, run, reverted): editing `body` in
  `"\u{feff}---\ntype: x\n---\nbody\n"` returns
  `Full(FrontmatterBoundaryChanged)`; the identical no-BOM document stays
  `Incremental`. The divergence is **fail-safe** — `frontmatter_fences` returns
  `None`, the guard fails closed, and every edit to a BOM'd document pays a
  full reparse. A performance/incrementality bug, not a correctness bug.
- `parser.rs:1522` already carries a "must stay in sync with
  `incremental.rs`'s `frontmatter_fences`" comment — the coupling is known and
  manually maintained.

### B. Link resolution duplicated in inline.rs — drift becomes a whole-document error
- `crates/waml-syntax/src/markdown/inline.rs:723` `bracket_match_end` decides
  inline-vs-reference and validates the reference label.
- `crates/waml-syntax/src/markdown/inline.rs:760` `parse_link` re-derives the
  same decision from raw text; any disagreement is promoted to
  `ParseError::StructuralInvariant` at lines 808, 821, 828 — failing the whole
  document. `BracketMatch` (line 257) records only `start/open/label_end/end/image`,
  not *what* matched.

### C. Frontmatter-entry extraction — three copies that already disagree
- `crates/waml/src/frontmatter.rs:272` `parse_closed_syntax` — the richest
  copy: handles `FrontmatterQuotedValueToken` (line 306) and types values via
  `parse_value` (Str/Num/Bool).
- `crates/waml/src/okf/lower.rs:508` `frontmatter_value` — no
  `FrontmatterQuotedValueToken` arm (loop at 548-557), so a quoted value yields
  `None`; and it drops non-`Str` values (561-564). Already disagrees.
- `crates/waml/src/uml/lower.rs:693` `frontmatter_value` — pure text hack:
  `split_once(':')` on the entry line plus its own `decode_scalar` (705),
  which disagrees with `frontmatter.rs::decode_quoted_scalar` on
  single-quoted scalars and escapes.

### D. Debug oracle is weaker than claimed
- `crates/waml-syntax/src/incremental.rs:981-1000` — debug-only oracle
  compares island *counts* and one diagnostic code, not trees. A splice that
  produces the right island count but the wrong tree passes.

**VERDICT: APPROVE** — all four sub-claims verified; A softened from
"live divergence" to "live fail-safe incrementality loss".

## Ordering / conflict flags

- **THIS PLAN IS ENABLING WORK FOR
  `2026-08-04-frontmatter-yaml-alignment.md`.** That plan rewrites frontmatter
  parsing in `waml-syntax/src/markdown/parser.rs` (nested
  `FrontmatterMapping`/`FrontmatterSequence` nodes), makes scalar classification
  the single source of truth for `waml/src/frontmatter.rs`, and extends
  `FmValue` with `Map`/`Null` — touching every copy this plan deduplicates.
  **Land this plan FIRST.** Tasks A and C collapse the three fence-recognition
  copies and the three frontmatter-entry extractors into one authority each, so
  the YAML work then lands in one place instead of three. Running the YAML plan
  first means implementing its changes three times and re-doing this plan
  against a moved target.
- **`inline.rs` (task B) is shared with issues 22 and 34.** Issue 22 threads a
  nesting-`depth` parameter through `parse_inlines`; issue 34 Task 3 indexes the
  pair vectors. Recommended order: **22 → this plan → 34 (T3)**.
- **`incremental.rs` (tasks A and D) is shared with issues 29 and 35.** Issue 29
  Task 3 converts `rebuild` to return `Result<_, ParseError>`; issue 35 Tasks 5-6
  hoist the `full` fallback and extract `plan_window_reparse` — the largest
  restructure of that file. Recommended order: **29 (T3) → this plan → 35 (T5-6)**.
  Note that issue 35's extraction moves the ~25 `return full(reason)` sites this
  plan's task D sits near; landing 35 first would invalidate task D's line
  references.

## Design decisions

1. **One fence authority in waml-syntax.** Extract a single
   `frontmatter_open_fence(source) -> Option<Range>` helper (BOM-aware) next to
   `crate::shell::frontmatter_close_fence_line` (the close side is already
   shared). `incremental.rs::frontmatter_fences`, `parser.rs:93`, and
   `mod.rs:183` all call it. Do **not** try to derive the fences from the old
   tree on the incremental side yet — the guard needs both old and new text and
   the new tree does not exist yet; sharing the lexing helper removes the drift
   without restructuring the guard.
2. **Record the match in `BracketMatch`.** Add a
   `resolution: BracketResolution` field (`Inline(DestinationParts)` /
   `Reference { normalized: LabelKey, explicit_end: usize }`) filled by
   `bracket_match_end`; `parse_link` consumes it instead of re-deriving. The
   three `StructuralInvariant` constructions become unreachable and are
   deleted.
3. **One frontmatter-entry authority in `waml::frontmatter`.**
   `okf/lower.rs::frontmatter_value` calls `parse_closed_syntax` on the
   Frontmatter node and looks up the wanted key; `uml/lower.rs` keeps its
   entry *ranges* (it edits text) but resolves the *value* through
   `parse_closed_syntax` too, deleting its private `decode_scalar`. Behaviour
   change (quoted values now readable via okf, typed values consistent) is
   intended and tested.
4. **Strengthen the debug oracle to full-tree comparison.** Debug-only, so
   cost is acceptable; compare a structural fingerprint of the spliced tree
   against the oracle tree (kinds + ranges + token text), not island counts.

## Tasks

### Extract a shared frontmatter open-fence helper and fix the BOM incrementality loss
- In `crates/waml-syntax/src/shell.rs`, add
  `pub(crate) fn frontmatter_open_fence(source: &str) -> Option<Line>` (or a
  range pair) implementing the BOM-aware `---` open-fence test currently at
  `markdown/parser.rs:93-95`, reusing/adjacent to
  `frontmatter_close_fence_line`.
- Rewrite `incremental.rs::frontmatter_fences` (line 1290) to use it instead
  of `trim() == "---"`; keep the existing close-fence call.
- Replace the inline copies at `markdown/parser.rs:93` and
  `markdown/mod.rs:183` with calls to the helper; update the sync comment at
  `parser.rs:1522` to name the shared helper.
- Tests: in `incremental/low_level_tests.rs`, add
  `bom_frontmatter_body_edit_stays_incremental` (the probe from this triage:
  `"\u{feff}---\ntype: x\n---\nbody\n"` body edit must be
  `ReparseOutcome::Incremental`) and a control asserting a BOM'd fence edit
  still forces `FrontmatterBoundaryChanged`. Run the property suite
  (`cargo test -p waml-syntax`) — it exercises the debug oracle.

### Record the resolution in BracketMatch and delete the parse_link re-derivation
- In `crates/waml-syntax/src/markdown/inline.rs`: add
  `enum BracketResolution { Inline(DestinationParts), Reference { label: /* normalized */ , end: usize } }`
  and a `resolution` field on `BracketMatch` (line 257).
- Change `bracket_match_end` (723) to return the resolution alongside the end
  offset; store it in the `matches.push` at line 697.
- Change `parse_link` (760) to match on `matched.resolution` — delete the
  re-derivation block (785-841's else-arm re-lex) and the three
  `ParseError::StructuralInvariant` constructions (808, 821, 828). The
  reference-definition lookup still happens once, in `bracket_match_end`;
  `parse_link` may keep a lookup by the recorded normalized label but must
  treat a miss as unreachable via an invariant *derived from the recorded
  match*, not a re-lex.
- Tests: existing inline/link fixtures must pass unchanged
  (`cargo test -p waml-syntax`). Add one regression: a link whose label
  requires normalization and an empty-`[]` collapsed reference, asserting both
  still resolve — these are the paths where the two copies could have drifted.

### Route okf and uml frontmatter reads through parse_closed_syntax
- `crates/waml/src/frontmatter.rs`: keep `parse_closed_syntax` (272) as the
  single authority; expose a small
  `pub(crate) fn closed_value(node, key) -> Option<FmValue>` convenience if
  helpful.
- `crates/waml/src/okf/lower.rs:508` `frontmatter_value`: replace the manual
  entry walk (516-568) with `parse_closed_syntax` on the Frontmatter child +
  key lookup; preserve the current "unclosed frontmatter -> Ok(None)"
  contract (parse_closed_syntax already returns None when the close fence is
  missing). Decide and document `FmValue` handling: keep returning
  `Some` only for `Str`, but quoted strings now decode correctly.
- `crates/waml/src/uml/lower.rs:693` `frontmatter_value`: resolve the value
  via the tree (parse the shell it already builds in `frontmatter_entries`)
  instead of `split_once(':')`; delete `decode_scalar` (705) once unused.
  `frontmatter_entries`' byte ranges stay as-is for the editing paths
  (`set_frontmatter`).
- Tests: in `crates/waml`, add cases for a double-quoted value, a
  single-quoted value, a numeric value, and a bool value read through both
  the okf and uml paths, asserting the three former copies now agree.

### Strengthen the debug oracle to full-tree comparison
- `crates/waml-syntax/src/incremental.rs:981-1000`: replace the island-count
  `debug_assert_eq` with a structural comparison of `tree` against
  `oracle.tree` — a recursive green/red walk asserting kind, range, and token
  text equality (annotations excluded; they are transferred, not reparsed).
  Keep the malformed-diagnostic assert.
- Keep it `#[cfg(debug_assertions)]`-only; the comment at 977-980 stays.
- Tests: run the full property suite in debug; any newly-exposed splice
  divergence found by the stronger oracle is fixed forward in this task or
  filed as its own issue if unrelated to A-C.
