# Qodana SARIF Fixes Design

## Goal

Resolve all 358 findings recorded in `qodana.sarif.json` without changing intended behavior.

## Approach

Treat the SARIF file as the fixed input set. Partition findings by source file so parallel workers never edit the same file. Use three workers per wave. Each worker reads only its assigned SARIF entries and source files, identifies the cause, applies the smallest fix, formats touched Rust files, and runs focused checks.

Most findings are mechanical Rust inspections. Keep those edits mechanical. For semantic findings such as dependency upgrades, `unwrap()` replacement, method naming, and trait-member ordering, preserve public behavior and run package tests or checks that cover the changed code.

## Safety

- Work only in `C:\dev\waml\.worktrees\qodana-sarif`.
- Do not edit `qodana.sarif.json`; it is the baseline.
- Do not edit files owned by another task.
- Do not suppress inspections or add broad allowances.
- Preserve unrelated user changes.
- Keep worker reports terse to protect controller context.

## Verification

Each task runs focused formatting, build, and test commands. After all tasks, run workspace formatting, build, tests, and a residual scan against the original SARIF locations and patterns. If available, rerun Qodana and require zero remaining findings from the original rule set.
