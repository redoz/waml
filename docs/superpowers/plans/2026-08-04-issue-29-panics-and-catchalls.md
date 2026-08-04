# Issue 29 — Content-reachable panics and catch-alls on domain enums

## Context

Five code-smell findings share one theme: invariants enforced by distance (a filter 500 lines
away, a stringly re-parsed id, a u8 round-trip) instead of by the type system, plus panics on
paths reachable from document content or the per-keystroke reparse path. All five were
re-verified at HEAD (2fdb5ff9) on 2026-08-04; none have been fixed by recent commits.

Verdicts (all APPROVE):

1. `crates/waml/src/uml/sequence.rs:1294-1296` — `.expect("each runtime fragment has a typed
   declared fragment")` in `validate_fragments`. Holds only because the fragment builder at
   `sequence.rs:756` (`let Some(kind) = value(&fragment.kind).copied() else { continue; }`)
   applies the same `value(&fragment.kind).is_some()` filter used at :1289. Nothing couples
   the two sites; a future edit to either filter makes this a content-reachable panic.
2. `crates/waml/src/uml/analysis.rs:651` — `_ => crate::diagnostic::DiagCode::MalformedAttribute`
   catch-all over `syntax::UmlSyntaxDiagnosticCode`; six variants already explicit above it.
   Any future syntax diagnostic code silently degrades to MalformedAttribute.
3. `crates/waml-syntax/src/incremental.rs:282-340` — five `.unwrap()`s on width arithmetic
   (`checked_add(...).unwrap()`, `TextRange::new(...).unwrap()`, at :282 (x2), :312-313, :335-340)
   inside `rebuild`, on the per-keystroke annotation-copy path. The file otherwise threads
   `Result` through ~15 `map_err`s, and `ParseError::WidthOverflow` already exists
   (`crates/waml-syntax/src/shell.rs:12`). Correct degraded behavior is the full-parse fallback.
4. `crates/waml/src/solve/route.rs:1002-1018` — `disc_to_side`/`side_disc` round-trip `Side`
   through `u8` with a `_ => Side::Bottom` catch-all, purely so `Side` can key a BTreeMap
   (`route.rs:1113`, `:1130`) and be compared (`:566`, `:2215`). `Side` at `route.rs:942` derives
   only `Clone, Copy, PartialEq`.
5. `crates/waml/src/uml/sequence.rs:69-77, 929` — `MessageId` is built as
   `MessageId(format!("m{message_index}"))` (:929) and `report_message` (:69-77) re-parses it
   with `strip_prefix('m').and_then(parse)`, silently dropping the diagnostic (`return`) on
   parse failure or out-of-range index. `MessageId` is `pub struct MessageId(pub String)`
   (`crates/waml/src/model.rs:602`).

## Ordering / conflict flags

This plan spreads across four files that other approved plans also edit:

- **Task 3 (`waml-syntax/src/incremental.rs`)** — shared with issue 28 (tasks A
  and D) and issue 35 (Tasks 5-6). Land **this task first** of the three: it is a
  localized `unwrap` → `?` conversion, whereas issue 35 restructures the whole
  function around it. Order: **29 (T3) → 28 → 35 (T5-6)**.
- **Task 4 (`waml/src/solve/route.rs`)** — issue 36 Task 4 edits a disjoint
  region of the same file and already carries a "land issue 29 first" note.
  Order: **29 (T4) → 36 (T4)**.
- **Tasks 1, 2, 5 (`waml/src/uml/analysis.rs`, `sequence.rs`)** — shared with
  issues 26, 27, and 35. Issue 27 restructures `sequence.rs`'s interaction-use
  and fragment-walk code that Tasks 1 and 5 touch. Order: **26 → 27 → 29 → 35**.

## Design decisions

- Item 1: prefer the debug_assert-and-skip form over restructuring `SeqNode::Fragment` — it is
  the minimal change that converts a latent panic into a skipped validation plus a debug-build
  signal, and `validate_fragments` is pure validation (skipping is safe degradation).
- Item 3: convert `rebuild` (and its callers inside `copy_annotations`/the incremental path) to
  return `Result<_, ParseError>`, mapping every width overflow to `ParseError::WidthOverflow`.
  The existing incremental entry point already falls back to full parse on `Err`; verify and rely
  on that rather than inventing a new mechanism.
- Item 4: derive `Eq, Ord, PartialOrd, Hash` on `Side` and delete both helper fns. The u8 disc
  existed only to satisfy Ord/BTreeMap; the derived order (Left < Right < Top < Bottom by
  declaration order) matches side_disc exactly, so behavior is unchanged.
- Item 5: change `MessageId` to carry the index as `usize` rather than re-parsing a formatted
  string. Keep the `m{index}` rendering wherever a display string is required (Display impl).
  This removes the silent-drop branch in `report_message` entirely; the remaining
  `concept.messages.get(index)` miss should become a `debug_assert!` plus skip, since an
  out-of-range index would be a builder bug, not document content.

### Task 1: Couple validate_fragments to the fragment builder's filter

- File: `crates/waml/src/uml/sequence.rs`
- Replace the `.expect(...)` at :1294-1296 with:
  `let Some(declared_fragment) = declared_fragments.next() else { debug_assert!(false, "runtime fragment without typed declared fragment"); continue; };`
- Add a comment at both filter sites (:756 and :1289) naming the other, so the coupling is
  discoverable.
- Test: existing sequence fixture tests must stay green (`cargo test -p waml uml::sequence`).
  Add a unit test only if a malformed-kind fixture can produce a fragment-count mismatch;
  otherwise the debug_assert is the guard.

### Task 2: Finish the UmlSyntaxDiagnosticCode match

- File: `crates/waml/src/uml/analysis.rs:636-652`
- Enumerate the remaining `UmlSyntaxDiagnosticCode` variants explicitly (check the enum in
  `waml-syntax`; map each to its semantically correct `DiagCode`, keeping MalformedAttribute
  where that is genuinely right) and delete the `_ =>` arm so a new variant is a compile error.
- Test: `cargo test -p waml` — plus a compile-time guarantee (exhaustive match) is the point.

### Task 3: Return WidthOverflow from incremental rebuild instead of unwrapping

- File: `crates/waml-syntax/src/incremental.rs:275-350` (`rebuild` and its call sites)
- Change `rebuild` to `-> Result<GreenNode<L>, ParseError>`; replace the five
  `checked_add(..).unwrap()` / `TextRange::new(..).unwrap()` sites (:282, :312-313, :335-340)
  with `.ok_or(ParseError::WidthOverflow)?` / `?`.
- Propagate through the caller(s) in the annotation-copy path; confirm the incremental entry
  point already falls back to full parse on `Err` (it threads Result elsewhere in this file) and
  wire it in the same way.
- Test: `cargo test -p waml-syntax`, including the incremental property tests
  (`tests/properties.rs`). Do not commit `proptest-regressions`.

### Task 4: Give Side a real Ord and delete the u8 round-trip

- File: `crates/waml/src/solve/route.rs`
- Add `Eq, PartialOrd, Ord, Hash` to the derive at :942 (declaration order Left, Right, Top,
  Bottom already matches `side_disc`, so ordering semantics are preserved).
- Delete `disc_to_side` (:1002) and `side_disc` (:1011); update the four use sites (:566,
  :1113, :1130, :2215) to use `Side` directly (`.cmp`, BTreeMap key `(key, side)`).
- Test: `cargo test -p waml solve::` — routing snapshot/fixture tests assert identical output.

### Task 5: Carry the message index in MessageId

- Files: `crates/waml/src/model.rs:602`, `crates/waml/src/uml/sequence.rs:69-77,929`,
  `crates/waml/src/solve/interaction.rs` (uses at :83, :435), plus any Display/serialize sites.
- Change `pub struct MessageId(pub String)` to carry `usize` (e.g. `pub struct MessageId(pub usize)`)
  with `impl Display` rendering `m{index}`; grep all constructors/consumers and update. If any
  boundary (export, LSP, serde) depends on the string form, render via Display at that boundary.
- In `report_message`, drop the strip_prefix/parse block; index directly with
  `concept.messages.get(id.0)`, and make the miss a `debug_assert!` + return (builder bug, not
  content).
- Test: `cargo test -p waml` full crate; sequence diagnostics fixtures must still point at the
  declared message spans.

## Gate

`cargo test --workspace` plus the vscode extension test/lint/build, per repo convention.
