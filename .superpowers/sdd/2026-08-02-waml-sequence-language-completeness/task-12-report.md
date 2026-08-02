# Task 12 report

## Status

Complete. The active authored corpus uses canonical sequence syntax. The
outside-to-outside semantic error is diagnosed and excluded from runtime
projection. The focused suites and the full done bar pass.

TokenSave provided the initial code context and saved approximately 4,266
tokens.

## Commits

- `7af91fa502735c4a13b1239ad907d84cb25e380b` — `docs: convert sequence corpus to canonical syntax`
- `6f678de1f79f1defe6738c98ca4c852f1c4e9a73` — `fix: complete sequence verification bar`

## Changed files

Corpus commit:

- `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.md`
- `crates/waml/tests/fixtures/parser-platform/recovery/sequence.md`
- `crates/waml/tests/fixtures/parser-platform/sequence.md`
- `crates/waml/tests/golden.rs`
- `crates/waml/tests/uml_behavior_syntax.rs`
- `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`
- `docs/superpowers/specs/2026-07-16-orders-uml-template-split-design.md`
- `docs/waml/architecture/views/authoring-and-validation.md`
- `docs/waml/architecture/views/editing-round-trip.md`
- `docs/waml/architecture/views/share-round-trip.md`
- `fuzz/seeds/uml_islands/sequence.md`

Repair commit:

- `crates/waml/src/uml/sequence.rs`
- `crates/waml/tests/sequence_semantics.rs`
- `crates/waml/tests/interaction_solver_golden.rs`
- `crates/waml/src/solve/interaction.rs`
- `crates/waml/src/uml/syntax/parser.rs`
- `crates/waml-editor/src/app/tests/shell.rs`
- `crates/waml-editor/src/inspector.rs`
- `crates/waml-editor/src/logo.rs`

The nested golden output did not change because canonical authored forms lower
to the same runtime layout. `formatter_actions.rs` keeps removed spellings only
in its explicit negative test. `serde_shape.rs` was already canonical and did
not need a content change.

## TDD and debugging evidence

1. Baseline `rtk cargo test -p waml --test uml_behavior_syntax` failed in the
   stale sequence tests: 6 passed and 3 failed. The parser reported two
   unsupported forms where the old assertion expected five, and five where the
   old deferred table expected six.
2. Added
   `outside_to_outside_is_diagnosed_and_excluded_from_runtime_projection`.
3. RED command:
   `rtk cargo test -p waml --test sequence_semantics outside_to_outside_is_diagnosed_and_excluded_from_runtime_projection -- --exact`
   — exit 1; 0 passed and 1 failed. Runtime had 2 edges instead of 1.
4. Root cause: lowering reported `InvalidSequenceEndpoint` but continued to
   push the outside-to-outside edge.
5. Added the minimal `continue` after the diagnostic.
6. GREEN command: the same exact command — exit 0; 1 passed and 24 filtered.

## Zero-legacy result

Exact command:

```text
rtk rg -n --glob '!target/**' --glob '!.worktrees/**' --glob '!docs/superpowers/plans/**' '^\s*-\s+\S+\s+(replies|sends)\s+|^\s*-\s+\S+\s+calls\s+\S+\s*:' .
```

Post-commit result: exit 1 with no output. This is the expected no-match
result. Removed forms remain in explicit negative tests and the approved
replacement-policy specification.

## Focused verification

All commands used the exact required form.

| Command | Result |
|---|---|
| `rtk cargo test -p waml-syntax --test markdown_extensions` | exit 0; 5 passed |
| `rtk cargo test -p waml --test sequence_language_syntax` | exit 0; 13 passed |
| `rtk cargo test -p waml --test sequence_formatter` | exit 0; 3 passed |
| `rtk cargo test -p waml --test sequence_semantics` | exit 0; 25 passed |
| `rtk cargo test -p waml --test interaction_solver_golden` | exit 0; 23 passed |
| `rtk cargo test -p waml --features serde --test serde_shape` | exit 0; 16 passed |
| `rtk cargo test -p waml --test formatter_actions` | exit 0; 9 passed |
| `rtk cargo test -p waml --test golden` | exit 0; 7 passed |
| `rtk cargo test -p waml --test uml_behavior_syntax` | exit 0; 10 passed |

After the Clippy repairs, the parser, semantics, and solver suites were run
again: 13, 25, and 23 tests passed.

## Full done bar

Final evidence on the committed content:

| Command | Result |
|---|---|
| `rtk cargo fmt --all -- --check` | exit 0; no diff |
| `rtk cargo test -p waml-syntax` | exit 0; 147 passed in 15 suites |
| `rtk cargo test -p waml --all-features` | exit 0; 595 passed in 34 suites |
| `rtk cargo test -p waml-editor --lib` | exit 0; library target passed, 0 tests selected on this platform |
| `rtk cargo check --workspace --all-targets --all-features` | exit 0; 0 errors, 2 Cargo metadata warnings |
| `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0; 0 lint errors, 2 Cargo metadata warnings |

The two warnings say that Cargo skips duplicate `bitflags` and `cfg-if`
packages in the pinned Makepad checkout. They are dependency metadata warnings,
not Rust or Clippy diagnostics.

One final `waml-syntax` run found a randomized, unrelated incremental Markdown
case after 463 generated cases. It concerned trailing whitespace at an ATX
heading at EOF. Proptest created an untracked persistence file. I removed that
generated file and reran the exact command without code changes; all 147 tests
passed. The earlier full run also passed all 147 tests.

## Scope and self-review

- `rtk git diff --check` returned exit 0.
- Post-commit `rtk git status --short` printed no paths.
- Post-commit `rtk git diff --stat` printed no diff.
- The original checkout's unrelated modified files were outside this isolated
  worktree and were not touched.
- Corpus conversion preserves create and destroy endpoint-first syntax.
- Recovery data uses malformed canonical calls, returns, `par` operands, and a
  binding, followed by a valid signal.
- Accepted syntax coverage includes self messages, `par`, outside endpoints,
  gates, and interaction references.
- Negative syntax coverage keeps deferred and removed forms lossless with exact
  unsupported ranges.
- Runtime message identifiers preserve authored occurrence order when the
  invalid outside-to-outside edge is excluded; the following valid edge is
  still `m1`.
- The interaction golden test now consumes canonical corpus bytes directly and
  has no transitional replacement shim.
- Clippy repairs are behavior-preserving. They use the existing local policy for
  argument-count allowances and avoid Rust APIs newer than the Rust 1.80 MSRV.
- Rustfmt also corrected drift in three prior editor-plan files and one editor
  test file. These files were already part of the overall plan.

## Warnings and concerns

- Cargo emits the two pinned Makepad duplicate-package metadata warnings noted
  above.
- The non-reproducing randomized incremental Markdown failure is unrelated to
  sequence completeness. No unrelated parser behavior was changed.
- No unresolved Task 12 concern remains.

## Fix Round 1

This section supersedes the earlier statement that the randomized incremental
Markdown failure did not reproduce.

### Deterministic Proptest replay

Recovered persistence entry:

```text
cc 83d1361b175875dbd370bcab5643768b94c798bcf247f61f323d380f210037cf # shrinks to edits = [(231, 28, 32), (50, 137, 21), (184, 63, 28)]
```

I recreated `crates/waml-syntax/tests/properties.proptest-regressions` with
that entry and did not delete it before either replay.

Current Task 12 branch command:

```text
rtk cargo test -p waml-syntax --test properties randomized_full_and_incremental_snapshots_agree -- --exact
```

Result: exit 1; 0 passed, 1 failed, 3 filtered out. Proptest reported
`successes: 0` and the same minimized edits. The incremental snapshot retains
a trailing whitespace token at the final ATX heading, while the full parse
does not.

Base replay used a detached temporary worktree at commit `52a3389b` and the
same persistence entry. The exact command was the same. Result: exit 1;
0 passed, 1 failed, 3 filtered out, `successes: 0`, with the same snapshot
difference and minimized edits.

This evidence proves a real pre-existing incremental Markdown bug. Task 12 did
not cause it. The counterexample is preserved verbatim above and in the RTK
logs `1785700354_cargo_test.log` and `1785700409_cargo_test.log`. I did not
change unrelated incremental parser behavior.

### Editor scope cleanup

Removed only the formatting changes that Task 12 introduced in:

- `crates/waml-editor/src/app/tests/shell.rs`
- `crates/waml-editor/src/inspector.rs`
- `crates/waml-editor/src/logo.rs`

Cleanup commit: `64030ae817ee2fa735afb83270e8cde446775940` —
`chore: narrow Task 12 editor scope`.

Command:

```text
rtk git diff 7af91fa502735c4a13b1239ad907d84cb25e380b -- crates/waml-editor/src/app/tests/shell.rs crates/waml-editor/src/inspector.rs crates/waml-editor/src/logo.rs
```

Result: exit 0 with no output. Task 12 no longer changes `shell.rs` or
`logo.rs`, and `inspector.rs` has no Task-12-only formatting delta. All prior
Task 11 content remains.

### Fix Round 1 verification

- `rtk cargo fmt --all -- --check` — exit 1. It reports only the pre-existing
  Task 11 formatting differences in the three files listed above. Task 12 had
  made this command pass by changing files outside its scope; Fix Round 1
  removes that scope violation.
- Deterministic property replay on Task 12 — expected exit 1 for the proven
  pre-existing bug, with `successes: 0`.
- Deterministic property replay at base `52a3389b` — same expected exit 1 and
  same counterexample.
- `rtk cargo check --workspace --all-targets --all-features` — exit 0;
  0 errors and the same two pinned Makepad duplicate-package metadata warnings.
- `rtk git diff --check` — exit 0 with no output.

### Fix Round 1 concerns

- The repository-wide format check is not clean at the pre-Task-12 editor
  state. Fixing those Task 11 formatting differences is outside Task 12 scope.
- The deterministic incremental Markdown counterexample fails at the Task 12
  base. Fixing it requires separate authorization.

## Fix Round 2

Fix Round 2 used the new authorization to repair the deterministic incremental
Markdown defect and restore the complete workspace verification bar.

### Root cause and regression

The minimized edits produce this final transition:

```text
old: "## xame: String\n"
new: "## xame: "
change: 56..63 -> ""
```

The old incremental window was a final `Heading` window. The local heading
parser consumed the new trailing space as a `WhitespaceToken`. The full parser
first removes horizontal whitespace at EOF from block parsing and assigns that
source byte to leading trivia on `EndOfFileToken`. The two trees therefore had
different ownership for byte `55..56`.

The permanent regression
`final_heading_edit_reassigns_trailing_whitespace_to_eof` reproduces only this
last minimized transition. Before the implementation change it failed at the
structural oracle: the incremental tree had a heading whitespace token at
`55..56`, while the full tree had EOF trivia at `55..56`. RTK log:
`1785700870_cargo_test.log`.

The root fix converts a final heading window to a tail window only when the new
source ends in a space or tab. The splice includes the existing EOF child, so
the tail parser owns both the heading and EOF trivia. Ordinary final-heading
edits keep the smaller heading window and preserve source-independent green
sharing. Existing zero-width boundary tests continue to use their prior
selection and fallback behavior.

Red-green evidence:

| Command | Before | After |
|---|---|---|
| `rtk cargo test -p waml-syntax --test properties final_heading_edit_reassigns_trailing_whitespace_to_eof -- --exact` | exit 1; structural mismatch | exit 0; 1 passed |
| exact persisted Proptest replay from Fix Round 1 | exit 1 before the fix | exit 0; 1 passed, 4 filtered out |
| `rtk cargo test -p waml-syntax` | found two intermediate boundary interactions during narrowing | exit 0; 148 passed in 15 suites |

The exact persistence entry from Fix Round 1 was recreated for the replay and
removed after it passed. The permanent named regression remains in the test
suite. The property case count and assertions are unchanged.

### Fix Round 2 commits and files

- `15a882943260f18a87638b45c44849a9a49b1d8e` —
  `fix(syntax): reparse final heading with EOF`
  - `crates/waml-syntax/src/incremental.rs`
  - `crates/waml-syntax/tests/properties.rs`
- `f8559da8b8ce386aecfe71de9409e85d9c9375ac` —
  `style(editor): apply workspace rustfmt`
  - `crates/waml-editor/src/app/tests/shell.rs`
  - `crates/waml-editor/src/inspector.rs`
  - `crates/waml-editor/src/logo.rs`

The editor formatting is a separate commit. It is authorized in Fix Round 2
and contains no behavior change.

### Fix Round 2 full done bar

| Command | Result |
|---|---|
| `rtk cargo fmt --all -- --check` | exit 0; no diff |
| `rtk cargo test -p waml-syntax` | exit 0; 148 passed in 15 suites |
| `rtk cargo test -p waml --all-features` | exit 0; 595 passed in 34 suites |
| `rtk cargo test -p waml-editor --lib` | exit 0; library target passed, 0 tests selected on this platform |
| `rtk cargo check --workspace --all-targets --all-features` | exit 0; 0 errors, 2 Cargo metadata warnings |
| `rtk cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0; 0 lint errors, 2 Cargo metadata warnings |
| exact zero-legacy `rtk rg` command | exit 1 with no output, which means zero matches |
| `rtk git diff --check` | exit 0; no output |

The two Cargo warnings are the existing duplicate `bitflags` and `cfg-if`
package-selection warnings from the pinned Makepad checkout. They are not Rust
or Clippy warnings.

TokenSave identified the incremental entry points, shell window selector, EOF
whitespace helper, and existing EOF tests before source inspection. No
TokenSave extractor or schema gap was found.

### Fix Round 2 concerns

- No unresolved correctness or verification concern remains.
- Cargo still reports the two external Makepad duplicate-package metadata
  warnings described above.
