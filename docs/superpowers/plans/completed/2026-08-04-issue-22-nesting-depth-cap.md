# Issue 22 — Cap Markdown nesting depth to stop stack-overflow kills

## Context

A hostile or accidental document with ~10k nested containers (`>` repeated) or
~10k nested inline emphasis (`*a *a *a …`) overflows the stack. A stack
overflow is uncatchable: it aborts the native process, kills the LSP, and
poisons the wasm instance for the rest of the session. This is the known open
defect "parser overflow at 10k quotes" recorded as a markdown-reading-view
follow-up.

## Verdict evidence (verified 2026-08-04 at worktree HEAD 2fdb5ff9)

The block builder itself is iterative (`crates/waml-syntax/src/markdown/block.rs:72`
uses an explicit `Vec<BlockFrame>` stack), so it happily builds a green tree of
unbounded depth. Every downstream consumer then recurses on that depth:

- `crates/waml-syntax/src/markdown/inline.rs:79` — `rebuild` self-recurses per
  child node (calls at lines 140, 185, 197); depth = block-tree depth.
- `crates/waml-syntax/src/markdown/inline.rs:265` — `parse_inlines`
  self-recurses per matched strikethrough/emphasis/link pair (calls at lines
  305, 331, 848 and via `rebuild` line 126/177); depth = inline nesting, which
  is independent of block depth.
- `crates/waml-syntax/src/markdown/projection.rs:232` — `visit` self-recurses
  per child; it already threads a `container_depth: usize` (line 166) but only
  uses it for classification, never as a bound.
- `crates/waml-syntax/src/incremental.rs:190` — `collect_occurrences`
  self-recurses per child node (line 228).
- `crates/waml-syntax/src/red.rs:251` — `SyntaxTree::rewrite` walks the tree
  recursively.

No cap exists anywhere: `grep -rni "max_depth|nesting|depth_limit|recursion"`
over `crates/waml-syntax/src` finds only proptest strategy names in
`incremental/properties.rs`. The codebase already sets the standard for this
class of bug: `recover_exact_source` (`crates/waml-syntax/src/incremental.rs:595`)
is deliberately iterative and is tested at 2,048 nesting levels
(`incremental.rs:1453`).

**VERDICT: APPROVE.** One bound at the two parse entry points (block container
depth, inline nesting depth) makes the tree depth finite, which bounds every
recursive consumer at once — the cheapest durable fix, and the cmark-gfm
precedent.

## Ordering / conflict flags

`crates/waml-syntax/src/markdown/inline.rs` is edited by three approved plans:
this one (threads a `depth` parameter through `parse_inlines`), issue 28
(records a `resolution` on `BracketMatch`, deletes `parse_link`'s re-derivation),
and issue 34 Task 3 (indexes the pair vectors to remove the quadratic scans).

**Land this plan FIRST of the three** — the depth parameter changes
`parse_inlines`' signature and every recursive call site, which is the widest
mechanical change; the other two are then applied on top. Recommended file
order: **22 → 28 → 34 (T3)**. Issue 21 touches only `reparse.rs` and is
independent of all three.

## Design decisions

- **Where the cap lives**: in `block.rs::parse_strict`'s event loop (container
  depth = `stack.len()`) and in `inline.rs::parse_inlines` (an explicit `depth`
  parameter). Not in the scan seam — `scan/mod.rs` documents the seam as a pure
  event vocabulary, and the tree builder is the first place that owns
  diagnostics (`BlockParse::diagnostics`, `block.rs:14`).
- **Cap values**: `MD_MAX_CONTAINER_DEPTH = 64`, `MD_MAX_INLINE_DEPTH = 32`,
  named constants in `markdown/mod.rs` with a comment tying them to the wasm
  stack budget. 64 + 32 nesting is far beyond any real document and far below
  the smallest stack (wasm default ~1 MB; the recursive frames here are large —
  `parse_inlines` holds several `Vec`s per frame).
- **Behaviour beyond the block cap**: when a `ScanEvent::Start` for a container
  kind (`BlockQuote`, `ListItem`, `List`, `Table`, footnote-definition kinds)
  arrives while `stack.len() >= MD_MAX_CONTAINER_DEPTH`, do not push a frame;
  count the suppressed opens so the matching `ScanEvent::End`s are swallowed
  symmetrically (a simple `suppressed: usize` counter — the event stream is
  balanced per the seam contract). The suppressed region's source bytes still
  land in the parent frame as raw text via the existing cursor/flush path, so
  `recover_exact_source`'s exact-source invariant holds. Emit one
  `TreeDiagnostic` per document (not per suppressed event) spanning the first
  suppressed range: "Markdown nesting exceeds the supported depth of 64;
  deeper structure is treated as plain text."
- **Behaviour beyond the inline cap**: `parse_inlines` takes `depth: usize`;
  when `depth == MD_MAX_INLINE_DEPTH`, skip the emphasis/strikethrough/link
  match arms entirely and fall through to the plain-text flush — the delimiters
  become literal text. Emit the same style of diagnostic once (deduplicate via
  a `bool` on `InlineContext`, which already owns `diagnostics`).
- **Not a `ParseError`**: a deep document is malformed input, not a structural
  invariant violation; it must produce a usable tree + diagnostic, never an
  error that blanks the document (resilience rule: one bad document must not
  cost the session).
- **Downstream recursion left as-is**: `projection::visit`,
  `collect_occurrences`, `rebuild`, and `red::rewrite` stay recursive; the
  entry-point cap bounds their depth. Converting them to iterative is a bigger
  change with no additional safety once the cap exists.
- **Incremental parity**: `parse_window` re-parses through the same
  `block::parse` / inline path, so reparse inherits the cap automatically. The
  existing property tests (`incremental/properties.rs` quote/list nesting
  strategies) must still pass — their nesting depths are shallow.

### Task 1 — Cap container depth in the block builder

- File: `crates/waml-syntax/src/markdown/block.rs` (constants in
  `crates/waml-syntax/src/markdown/mod.rs`).
- Add `MD_MAX_CONTAINER_DEPTH: usize = 64` to `markdown/mod.rs`.
- In `parse_strict`'s event loop (`block.rs:93` onward): add
  `let mut suppressed = 0usize;`. In the `ScanEvent::Start` arm, if
  `stack.len() >= MD_MAX_CONTAINER_DEPTH` and `start_kind` returns a container
  kind, increment `suppressed`, record the first suppressed range, and skip the
  push. In the `ScanEvent::End` arm, if `suppressed > 0`, decrement and skip.
- After the loop, if a suppressed range was recorded, push one diagnostic into
  the `BlockParse` diagnostics naming the cap and spanning that range.
- Verify the suppressed region's text still reaches the tree through the
  existing text/cursor handling so `recover_exact_source` recovers the exact
  source (assert this in the test).
- Tests (in `block.rs` or `parser_tests.rs`): (a) a 10,000-deep `>` document
  parses without overflow, yields a tree, exact source recovery holds, and
  exactly one depth diagnostic is present; (b) a 63-deep document produces no
  diagnostic and an unchanged tree (snapshot or structural assert).

### Task 2 — Cap inline nesting in parse_inlines

- File: `crates/waml-syntax/src/markdown/inline.rs`.
- Add `MD_MAX_INLINE_DEPTH: usize = 32` (same home as Task 1's constant).
- Thread `depth: usize` through `parse_inlines` (signature at line 265) and its
  recursive calls (lines 305, 331, 848, and the `rebuild` call sites at 89,
  126, 177 pass `0`). At `depth >= MD_MAX_INLINE_DEPTH`, skip the
  strikethrough/emphasis/link arms so delimiters flush as plain text.
- Add `depth_diagnostic_emitted: bool` to `InlineContext`; emit one diagnostic
  the first time the cap bites, spanning the delimiter that was demoted.
- Tests: (a) a paragraph of 10,000 repetitions of `*a ` (and one of `~~a `)
  parses without overflow with one diagnostic; (b) 31-deep nested emphasis
  round-trips unchanged with no diagnostic.

### Task 3 — End-to-end overflow regression tests through the public entry points

- File: new tests in `crates/waml-syntax/tests/` alongside the existing
  integration tests (or extend an existing depth-focused test module).
- Through the public parse API (the same path the LSP and editor use): parse a
  document of 10,000 `>` markers, one of 10,000 nested emphasis, and one
  mixing both; assert Ok, assert a depth diagnostic, assert
  `recover_exact_source`-style round-trip.
- Exercise the downstream recursive consumers on the capped tree: run the
  projection (`projection::visit` path), an incremental edit
  (`collect_occurrences` via the incremental module), and a `rewrite` pass, to
  prove the one-bound-covers-all claim.
- Run in the normal test profile (test threads have 2 MB stacks on most
  platforms — smaller than main-thread stacks — so a green run here is a
  stronger guarantee than a main-thread repro).
- Gate: `cargo test --workspace`. Do not commit
  `tests/properties.proptest-regressions`.
