# Task 7 Report: Repository Rollout

Date: 2026-08-10

Worktree: `C:\dev\waml\.worktrees\use-case-diagram-rendering`

Branch: `codex/use-case-diagram-rendering`

Commit: `2604455771ba2db2a209907b63d22e62b9527caf`

## Result

Task 7 is complete.

- All repository Markdown diagram headers in scope use canonical UML diagram types.
- The three real workflow view documents use `uml.UseCaseDiagram`.
- Direct activity, state-machine, and sequence documents use their matching canonical diagram types.
- Other legacy `Diagram` documents use `uml.ClassDiagram` unless use-case inspection selected `uml.UseCaseDiagram`.
- Embedded Rust Markdown literals and expected values use canonical types, except for deliberate migration and obsolete-diagnostic inputs.
- No type aliases were added.
- The approved `docs/superpowers/specs/2026-08-09-canonical-uml-diagram-types-and-upgrade-design.md` file is unchanged.

## RED Evidence

### Repository docs inspection

Command:

```text
rtk cargo run -p waml-cli -- upgrade docs --check
```

Initial result: exit 1.

The command stopped with `the bundle does not pass strict validation`. The cause was valid unrelated structured frontmatter in `affected-analysis.md`:

```yaml
sources:
  - { id: affected-analysis, resource: analysis.rs, title: AffectedAnalysis }
```

The migration reader rejected every parser diagnostic, including diagnostics for YAML structures that the migration does not need.

### Structured-frontmatter regression

Test:

```text
rtk cargo test -p waml --test upgrade_inspection unrelated_structured_frontmatter_does_not_block_legacy_inspection -- --exact --nocapture
```

RED result: the inspector returned `InvalidLegacyBundle` for the valid `sources:` sequence.

GREEN change: `parse_frontmatter_source` now extracts the required frontmatter scalars through the tolerant syntax tree. It no longer rejects unrelated parser diagnostics. The existing closing-fence and scalar-kind checks still reject unclosed frontmatter and a non-string `type` value.

### No-op plan regression

Test: `no_op_plan_does_not_validate_unrelated_model_errors` in `crates/waml-cli/tests/upgrade_plan.rs`.

RED result: a bundle with no legacy diagram type still failed strict validation because it contained a deliberate malformed UML fixture.

GREEN change: `plan_upgrade_with_migrations` returns the original bytes and no reports when no migration detects a candidate. If a migration runs, the existing strict full-bundle validation still runs before a write. This preserves the atomic-validation guarantee.

### Canonical sequence regression

The first full `waml` run had 14 sequence semantic failures after the sequence test literals became `uml.SequenceDiagram`.

Cause: interaction-use validation still compared targets with the obsolete internal `Behavior(Sequence)` type.

GREEN change: both interaction-use checks now compare with `Diagram(Sequence)`. The focused sequence suite then passed all 34 tests.

## Migration Execution

The CLI changed valid physical bundles so it preserved all bytes outside each `type` scalar.

Successful CLI write scopes included:

- `docs/waml/use-cases`
- `docs`
- each valid editor fixture bundle under `crates/waml-editor/tests/fixtures`
- valid behavior fixture bundles under `crates/waml/tests/fixtures/behavior`
- valid standalone parser-platform documents
- valid standalone fuzz seeds

The initial root writes for `crates` and `fuzz` did not run because those trees contain deliberate malformed and recovery fixtures. This failure is correct: the CLI must not write over a bundle that needs a migration and fails strict validation.

The following invalid or recovery fixture scalars were changed manually with exact scalar-only patches:

- `crates/waml/tests/fixtures/orders-domain.md`
- `crates/waml/tests/fixtures/parser-platform/diagram.md`
- `crates/waml/tests/fixtures/parser-platform/recovery/diagram.md`
- `fuzz/seeds/uml_islands/diagram.md`

After all legacy physical scalars were gone, the no-op short circuit allowed `upgrade crates --check`, `upgrade fuzz --check`, and `upgrade . --check` to inspect these deliberate invalid fixtures without weakening validation for a real migration.

## Changed Categories

### Physical Markdown

- Three use-case workflow views: `uml.UseCaseDiagram`.
- Fourteen architecture views: canonical class, activity, or sequence diagram types.
- Five editor diagram fixtures: `uml.ClassDiagram`.
- Behavior fixtures: `uml.ActivityDiagram`, `uml.StateMachineDiagram`, and `uml.SequenceDiagram`.
- Parser-platform and nested bundle fixtures: canonical behavior or class diagram types.
- Fuzz seeds: canonical activity, sequence, and class diagram types.

### Embedded Markdown and expectations

- Current language and editor tests now author canonical diagram headers.
- Parser claim-state tables now list the five canonical diagram types as supported.
- The obsolete literals remain only in upgrade inspection, rewrite, plan, CLI migration, and obsolete-diagnostic tests.
- Current design and language examples before the approved 2026-08-09 design were updated. The approved design itself was not changed.

### Load-bearing fixes

- `crates/waml/src/frontmatter.rs`: tolerate unrelated valid structured frontmatter during migration scalar inspection.
- `crates/waml-cli/src/upgrade.rs`: skip strict model validation for a true no-op plan.
- `crates/waml/src/uml/sequence.rs`: accept canonical sequence diagram targets for interaction uses.
- Regression coverage was added to `upgrade_inspection.rs` and `upgrade_plan.rs`.

## Verification

### Migration and stale-header gates

```text
rtk cargo run -p waml-cli -- upgrade docs --check
```

Final result: exit 0.

```text
rtk cargo run -p waml-cli -- upgrade crates --check
rtk cargo run -p waml-cli -- upgrade fuzz --check
rtk cargo run -p waml-cli -- upgrade . --check
```

Final result: all exit 0 with no planned changes.

Exact stale-header gate:

```text
rtk rg -n --glob "*.md" '^type:\s*(Diagram|uml\.(Activity|StateMachine|Sequence))$' docs/waml docs/uaml-spec.md crates/waml/tests/fixtures crates/waml-editor/tests/fixtures fuzz/seeds
```

Result: exit 1 with no output. This is the expected no-match result.

### Focused tests

```text
rtk cargo test -p waml --test upgrade_inspection --test frontmatter_rewrite
```

Result: 16 passed.

```text
rtk cargo test -p waml --test upgrade_inspection --test frontmatter_rewrite --test sequence_semantics --test parser_platform_properties --test uml_attribute_syntax -j 1
```

Final result: 78 passed in 5 suites.

```text
rtk cargo test -p waml-cli --test upgrade_plan -j 1
rtk cargo test -p waml-cli --test cli_e2e upgrade_ -j 1
```

Final result: 8 passed and 7 passed.

```text
rtk cargo test -p waml-ops-dto -j 1
```

Result: 19 passed in 2 suites.

### Broad suites

```text
rtk cargo test -p waml
```

Result: 966 passed and 1 ignored in 41 suites.

```text
rtk cargo test -p waml-editor -j 1
```

Result: 1,111 passed, 1 failed, and 5 ignored.

The one failure is `source_view::tests::mounted_widget_draw_translates_every_painted_layer_and_embedded_state_once` at `crates/waml-editor/src/source_view.rs:1555`. A focused rerun failed again. The source file and test are untouched by Task 7. The test authors `type: Runbook` and compares translated paint evidence. It does not use a migrated diagram type. This is an unrelated, pre-existing editor geometry assertion failure.

```text
rtk cargo test -p waml-cli -j 1
```

Result before Cargo stopped at the failed integration binary: 147 unit tests passed; the CLI end-to-end binary had 33 passed and 1 failed.

The one failure is the unchanged `check_accepts_generic_okf_without_uml_diagnostics` test. It expects no warning for `notes.Decision`. The unchanged WAML semantic diagnostic contract expects an `unknown-type` warning for another generic type, `vendor.Widget`. Neither the CLI test, `crates/waml/tests/semantic_diagnostics.rs`, nor `crates/waml/src/uml/analysis.rs` is in the Task 7 diff. This is an unrelated, pre-existing contract conflict. All seven upgrade-specific CLI end-to-end tests and all eight upgrade-plan tests pass.

### Format, lint, compile, and diff

```text
rtk cargo fmt --all -- --check
```

Result: exit 0 after rustfmt wrapped two migrated string assignments.

```text
rtk cargo clippy -p waml -p waml-cli -p waml-editor -p waml-ops-dto --all-targets -j 1 -- -D warnings
```

Result: 0 errors. Cargo emitted only the two existing duplicate Makepad package-selection warnings.

```text
rtk cargo check --workspace -j 1
```

Result: 0 errors across 21 crates. Cargo emitted the same two duplicate package-selection warnings.

```text
rtk git diff --check
```

Result: exit 0.

## Self-review

- The final diff has 63 files. Most changes are one scalar or one embedded literal.
- The CLI performed valid physical bundle writes. Manual edits were limited to exact scalars in invalid or recovery fixtures and embedded Rust literals.
- Strict validation still runs before every real migration write.
- A no-op plan keeps byte-for-byte input and produces no migration reports.
- Unclosed frontmatter and non-string `type` values still fail migration inspection.
- Deliberate obsolete inputs remain isolated to migration and diagnostic tests.
- No approved design content, alias table, or unrelated editor source was changed.

## Concerns

- The full editor suite has one reproducible unrelated geometry assertion failure in untouched code.
- The full CLI suite has one reproducible unrelated generic-type diagnostic expectation conflict in untouched code and tests.
- Cargo prints two existing duplicate dependency warnings from the Makepad checkout. They do not fail clippy or check.

TokenSave reduced code exploration by approximately 64,528 tokens during this task.

## Fix Round 1: Review Findings

Date: 2026-08-10

Commit: `2bab35dd4b7b395d4dc142d8ddcda43cc99463f4`

### Result

Both review findings are fixed.

- Migration detection no longer converts frontmatter inspection errors to `false`.
- Detection now has typed states for no frontmatter, no `type` scalar, a string scalar, and malformed frontmatter.
- A detected migration fails if any document has malformed frontmatter.
- A true no-op still bypasses unrelated invalid fixtures when no legacy type exists.
- Valid `sources:` sequences with inline maps remain inspectable and byte-preserving.
- Active document-type references and both property allowlists use the five canonical diagram names.
- Neither approved 2026-08-09 design file changed.

### Finding 1: RED

Command:

```text
rtk cargo test -p waml-cli --test upgrade_plan -j 1 -- --nocapture
```

Initial result: 9 passed and 2 failed.

The failures were:

- `closed_malformed_frontmatter_with_legacy_type_is_not_a_no_op`
- `legacy_document_beside_closed_malformed_frontmatter_is_rejected_during_detection`

Both calls returned an `Ok` plan. The old detector called `replace_frontmatter_string_scalar` and used `is_ok_and`, so every inspection error became `false`. The no-op short circuit then returned success.

A stronger same-document regression also failed:

```text
rtk cargo test -p waml-cli --test upgrade_plan legacy_type_with_valid_structured_frontmatter_is_rewritten -- --exact --nocapture
```

Result: 0 passed and 1 failed. A legacy type beside valid `sources:` YAML was treated as a no-op because the general rewrite helper rejected parser recovery diagnostics.

### Finding 1: GREEN

The new frontmatter scalar inspector uses these typed results:

- no frontmatter;
- no requested scalar;
- string scalar;
- a result error for malformed or non-string frontmatter.

The inspector accepts only the known valid inline-map sequence recovery. It rejects `BadToken` recovery, malformed scalar recovery, duplicate keys, invalid indentation, unclosed fences, and non-string `type` values.

The WAML migration layer retains a legacy signal with a malformed result. It scans the complete bundle before it decides:

- no legacy signal: return a byte-identical no-op without strict model validation;
- legacy signal plus any malformed frontmatter: return `InvalidLegacyBundle`;
- legacy signal with valid frontmatter: run the transform and the existing strict full-candidate validation.

The CLI detector now calls this result-bearing WAML detector directly. It has no `is_ok_and` error discard.

Command:

```text
rtk cargo test -p waml-cli --test upgrade_plan -j 1
```

Final result: 12 passed.

The new cases cover:

- a closed malformed document with `type: Diagram`;
- a valid legacy document beside an unrelated malformed scalar entry;
- a valid legacy document beside unrelated structured `sources:` YAML;
- a legacy scalar in the same document as structured `sources:` YAML, with exact byte preservation outside the scalar.

Task 4 compatibility first found one RED regression:

```text
rtk cargo test -p waml --test upgrade_inspection --test frontmatter_rewrite -j 1
```

Intermediate result: 15 passed and 1 failed. The new scalar inspector initially treated `[Diagram]` as a string-shaped bare token.

The inspector now uses the existing frontmatter value reader. It identifies `[Diagram]` as a list and returns the required non-string error.

Final result for the same command: 16 passed.

### Finding 2

The following active references now use canonical names:

- `docs/uaml-spec.md`: document roles, the five diagram dispatch types, flow and interaction headings, and interaction-use targets.
- `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`: structure view types and all behavior document types.
- `crates/waml/tests/parser_platform_properties.rs`: both generated claim-state allowlists now contain `uml.ClassDiagram`, `uml.UseCaseDiagram`, `uml.ActivityDiagram`, `uml.StateMachineDiagram`, and `uml.SequenceDiagram`.
- `crates/waml-cli/tests/lsp_e2e.rs`: the active test comment now names `uml.ActivityDiagram`.

Command:

```text
rtk cargo test -p waml --test parser_platform_properties -j 1
```

Result: 15 passed.

Exact active literal inventory:

```text
rtk rg -n --pcre2 'type:\s*(?:Diagram|uml\.(?:Activity|StateMachine|Sequence))(?!Diagram)|["`](?:Diagram|uml\.(?:Activity|StateMachine|Sequence))["`]' docs/uaml-spec.md docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md crates/waml/tests crates/waml-cli/tests crates/waml-editor/tests
```

The two active documentation files have no matches. Remaining matches are deliberate:

- `upgrade_plan.rs`, `cli_e2e.rs`, `upgrade_inspection.rs`, and `frontmatter_rewrite.rs`: migration input and rewrite coverage;
- `semantic_diagnostics.rs`: obsolete-type diagnostic coverage;
- `parser_platform_properties.rs`: explicit retired near-misses;
- `uml_diagram_syntax.rs`: explicit retired-type recognition checks.

### Final Verification

```text
rtk cargo test -p waml --test upgrade_inspection --test frontmatter_rewrite --test parser_platform_properties -j 1
```

Result: 31 passed in 3 suites.

```text
rtk cargo test -p waml-cli --test cli_e2e upgrade_ -j 1
```

Result: 7 passed and 27 filtered out.

```text
rtk cargo test -p waml -j 1
```

Result: 966 passed and 1 ignored in 41 suites.

```text
rtk cargo run -p waml-cli -- upgrade . --check
```

Result: exit 0 with no planned changes.

```text
rtk cargo fmt --all -- --check
rtk cargo clippy -p waml -p waml-cli --all-targets -j 1 -- -D warnings
rtk cargo check -p waml -p waml-cli --all-targets -j 1
rtk git diff --check
```

Results: format passed; clippy had 0 errors; check had 0 errors across 3 crates; diff check passed. Cargo emitted only the two existing duplicate Makepad package-selection warnings.

### Changed Files

- `crates/waml/src/frontmatter.rs`
- `crates/waml/src/upgrade.rs`
- `crates/waml-cli/src/upgrade.rs`
- `crates/waml-cli/tests/upgrade_plan.rs`
- `crates/waml-cli/tests/lsp_e2e.rs`
- `crates/waml/tests/parser_platform_properties.rs`
- `docs/uaml-spec.md`
- `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`

### Self-review and Concerns

- Detection scans every document before it returns. File ordering cannot hide a malformed neighbor.
- The typed scalar path reads only the requested top-level scalar. It does not add a general YAML value dependency.
- Real migrations still use strict full-candidate validation before a write.
- True no-op plans remain byte-identical and do not validate deliberate invalid model fixtures.
- The known unrelated editor geometry assertion and generic OKF CLI expectation conflict from the original Task 7 report remain outside this fix-round scope. The reviewer confirmed those classifications.
- No new concern remains for either review finding.

TokenSave reduced fix-round exploration by approximately 30,447 tokens.

## Fix Round 2: Inline Flow-Map Validation

Date: 2026-08-10

### Result

- Upgrade inspection now validates the complete syntax of brace-wrapped sequence maps.
- The validator supports the flat `sources:` maps used by repository documents. It requires comma-separated `key: value` fields and handles quoted scalar commas.
- A closed malformed map fails inspection both with and without a legacy-type neighbor.
- Valid structured frontmatter remains supported. The parser grammar did not change.
- Active sequence-language documentation now uses `uml.SequenceDiagram`.

### RED Evidence

```text
rtk cargo test -p waml-cli --test upgrade_plan brace_wrapped_malformed_map -- --nocapture
```

Result before the fix: 0 passed, 2 failed. Both plans incorrectly returned success.

### GREEN Evidence

```text
rtk cargo test -p waml --test frontmatter_rewrite --test upgrade_inspection -j 1
rtk cargo test -p waml-cli --test upgrade_plan -j 1
rtk cargo test -p waml-cli --test cli_e2e upgrade_ -j 1
```

Result: 17 passed in 2 suites, 14 passed, and 7 passed with 27 filtered out.

```text
rtk cargo fmt --all -- --check
rtk cargo check -p waml -p waml-cli --all-targets -j 1
rtk cargo clippy -p waml -p waml-cli --all-targets -j 1 -- -D warnings
rtk git diff --check
```

Result: all exit 0. Cargo reports only the two existing duplicate Makepad dependency warnings.

The exact stale type-literal inventory across `docs/waml` returned exit 1 with no output, which is the expected no-match result.

### Files and Self-review

- `crates/waml/src/frontmatter.rs`: adds the narrow flat flow-map validator and applies it to recovery diagnostics and all brace-wrapped sequence values.
- `crates/waml/src/upgrade.rs`: treats malformed closed frontmatter with any string `type` scalar as migration-relevant while preserving the atomic temporary-repair test.
- `crates/waml/tests/frontmatter_rewrite.rs`: directly proves malformed flow maps cannot be rewritten.
- `crates/waml-cli/tests/upgrade_plan.rs`: covers malformed maps without a legacy scalar and beside a legacy document.
- `docs/waml/goals/uml/sequence/language.md`: replaces the active stale sequence type.
- The approved 2026-08-09 design did not change.
- No new concern remains.
