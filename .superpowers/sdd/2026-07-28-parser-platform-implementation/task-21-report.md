# Task 21 Report: Final Parser Authority Boundary

## Outcome

Task 21 retains the parser-platform migration while correcting its enforcement
boundary. The legacy parser and serializer remain deleted, retained consumers
continue to use the parser-platform APIs, and the former partial Rust interpreter
has been replaced by four focused tests whose claims are backed directly by file
absence, Cargo metadata, and rustc privacy.

The raw UML parser is now sealed behind:

```rust
mod parser;

pub(in crate::uml) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    parser::parse(text, structure)
}
```

The implementation entry point is `pub(super)`. Rust privacy, rather than source
interpretation, enforces both the raw-parser and `parse_full` boundaries. All
legitimate UML analysis, lowering, and rename callers use the single
`syntax::parse_full` facade.

The correction also:

- replaced the 2,506-line authority suite with four focused authority tests;
- deleted the 5,953-line support interpreter and the complete adversarial fixture
  maze;
- removed the unused direct `syn` development dependency;
- restored ordinary `Vec::with_capacity` and `Vec::new` editor expressions; and
- restored the exact lowering diagnostic `no claimed concept '{target}'`.

`SourceBundle` remains the sole source authority, its mutation remains
crate-private, and `prepare_candidate` remains the public analysis choke point.
Generic OKF handling, Index/Log separation, static specialization, recovery, and
atomic editor/CLI/LSP preparation are unchanged. No Task 22 incremental behavior
or API is included.

Exact `SyntaxTree` writing still reproduces authored bytes, `uml::Formatter`
still emits canonical WAML, and semantic `Model` serialization still represents
the model contract. These remain three distinct responsibilities.

## Changed files in the authority correction

Commit `e38a3d3b`:

- `crates/waml/src/uml/syntax/mod.rs`
- `crates/waml/src/uml/syntax/parser.rs`
- `crates/waml/src/uml/analysis.rs`
- `crates/waml/src/uml/lower.rs`
- `crates/waml/src/uml/rename.rs`

Commit `f536e2c8`:

- `crates/waml/Cargo.toml`
- `crates/waml/tests/no_legacy_authority.rs`
- deleted `crates/waml/tests/support/authority_guard.rs`
- deleted `crates/waml/tests/fixtures/authority-guard/`

Runtime/evidence slice:

- `crates/waml/src/uml/lower.rs`
- `crates/waml-editor/src/inspector.rs`
- `.superpowers/sdd/2026-07-28-parser-platform-implementation/task-21-report.md`
- `.superpowers/sdd/2026-07-28-parser-platform-implementation/progress.md`

The unrelated dirty
`.superpowers/sdd/2026-07-28-parser-platform-implementation/task-7-report.md`
was preserved, never modified by this correction, and never staged.

## RED -> GREEN evidence

### Private facade

After making the parser module private and its implementation `pub(super)`,
`rtk cargo check -p waml --all-features` failed with exactly four expected
`E0603` privacy errors at the former raw call sites. After adding the
`pub(in crate::uml)` facade and migrating those callers, the check passed.

### Focused authority suite

The temporary public-control probe:

```powershell
rtk cargo test -p waml --test no_legacy_authority red_probe_rejects_a_successful_external_compile -- --exact
```

failed as intended with `public-control unexpectedly compiled` (`0 passed,
1 failed`). This proved the harness rejects a successful external compile.
After removing the probe, the replacement suite passed all four tests:

- retired legacy files, root exports, and exact retired symbols are absent;
- Cargo metadata reports exactly `waml` as the direct workspace
  `waml-syntax` dependent;
- an external crate cannot name the raw parser module; and
- an external crate cannot name `parse_full`.

### Target-bearing diagnostic

The new private-lookup regression:

```powershell
rtk cargo test -p waml --lib uml::lower::tests::missing_tree_error_names_requested_concept -- --exact
```

failed for the expected assertion:

```text
left:  "no claimed concept"
right: "no claimed concept 'missing-order'"
test result: FAILED. 0 passed; 1 failed; 269 filtered out
```

After the minimal restoration, the same command passed: `1 passed, 269
filtered out`. The test also verifies the operation remains `attr.add`.

## Focused final verification

All commands below completed successfully:

| Command | Result |
| --- | --- |
| `rtk cargo test -p waml --test no_legacy_authority` | 4 passed, 1 suite |
| `rtk cargo test -p waml --test layout_atom_api --test uml_diagram_syntax --test semantic_diagnostics` | 21 passed, 3 suites |
| `rtk cargo test -p waml --test golden --test serde_shape --test layout_serde_roundtrip` | 6 passed, 3 suites |
| `rtk cargo test -p waml --test href_contract --test uml_lowering_authority --test uml_lowering_order` | 13 passed, 3 suites |
| `rtk cargo test -p waml-editor --all-features` | 737 passed, 5 suites |

These gates retain public layout compatibility, parsing and recovery,
diagnostics, exact/golden/serde distinctions, authored href behavior, lowering
authority and order, and editor behavior.

## Full final verification

| Command | Result |
| --- | --- |
| `rtk cargo test --workspace --all-features` | 1,268 passed, 41 suites |
| `rtk cargo check --workspace --all-features` | PASS, 0 errors; 4 warning groups across 4 crates, plus two duplicate-package notices |
| `rtk cargo clippy --workspace --all-targets --all-features` | PASS, 0 errors; 29 warnings |
| `rtk cargo fmt --all -- --check` | PASS |
| `rtk git diff --check` | PASS |
| `rtk rg -n 'syntax::parser|parser::parse' crates/waml/src --glob '*.rs'` | one match: `crates/waml/src/uml/syntax/mod.rs:13: parser::parse(text, structure)` |
| `rtk rg -n 'authority_guard|analyze_sources|analyze_workspace|syn::' crates/waml` | no matches |

The warnings are retained lint/dependency warnings, not errors introduced by
this correction. The parser-path result is solely the facade's internal
delegation. The interpreter scan is empty.

## Acceptance criteria mapping and self-review

1. The focused file/surface test proves the four legacy files, root exports,
   and exact retired symbols remain absent.
2. Source review and the parser-path scan show `mod parser`,
   `pub(super) parser::parse`, the exact
   `parse_full(SourceText, &MarkdownStructureMap) ->
   Arc<SyntaxTree<UmlLanguage>>` signature with `pub(in crate::uml)`, and no
   second raw caller.
3. Two real external Cargo fixtures fail for the expected rustc privacy reason.
4. The Cargo metadata test reports exactly `waml` as the direct workspace
   `waml-syntax` dependent.
5. Existing API boundaries and the complete gates confirm crate-private
   `SourceBundle` mutation and public `prepare_candidate` remain unchanged;
   no second preparation path exists.
6. The 5,953-line interpreter, 2,506-line old suite, fixture maze, and direct
   `syn` dependency are absent; the scan returns no interpreter symbols.
7. Exact tree writing, canonical formatter output, and semantic model
   serialization remain distinct and are covered by golden/serde/parser gates.
8. The exact target-bearing error and ordinary `Vec` expressions are present.
9. Every focused and full gate above passes with the recorded counts.
10. The three-commit diff contains no incremental parse entry point,
    `TextChange`, `ChangeMap`, reuse window, retention gate, or other Task 22
    behavior.

Self-review found no public/raw parser alias, no broad source scanner, no
interpreter fixture maze, no second preparation path, and no Task 22 behavior.
The Task 21 compatibility suites remain present and passing. The protected
Task 7 report is outside every staged/committed Task 21 correction slice.

## Commit chain

- `e38a3d3b` `refactor(parser): seal UML parser facade`
- `f536e2c8` `test(parser): enforce honest authority boundaries`
- `38d4fe92` `fix(parser): restore honest Task 21 boundary`

## Final correction review — lockfile evidence

- The focused Cargo update removed the stale direct `syn 2.0.119` lockfile
  entry from package `waml`; no unrelated lockfile entries were regenerated.
- `rtk cargo test --locked -p waml --test no_legacy_authority`: 4 passed,
  1 suite.
- `rtk cargo check --locked -p waml`: PASS, 0 errors; two pre-existing
  duplicate-package warnings.
- After committing this correction, `rtk git status --short` reports only the
  protected dirty original-plan `task-7-report.md`. `rtk git diff --name-only
  38d4fe92..HEAD` reports only `Cargo.lock` and this Task 21 report.

## Concerns

None. Existing check/clippy and duplicate-package warnings are reported above
without claiming warning-free output.
