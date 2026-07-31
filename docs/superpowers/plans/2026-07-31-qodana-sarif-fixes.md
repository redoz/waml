# Qodana SARIF Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:systematic-debugging`, `superpowers:test-driven-development`, and `superpowers:verification-before-completion`. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve all 358 findings in `qodana.sarif.json`.

**Architecture:** Nine disjoint file-ownership tasks run in three parallel waves. Workers filter SARIF results to their assigned files, make minimal fixes, and run package-focused verification. Controller reviews combined diffs and runs final workspace verification.

**Tech Stack:** Rust, Cargo, Qodana SARIF 2.1.0.

## Global Constraints

- Work only in `C:\dev\waml\.worktrees\qodana-sarif`.
- Treat `qodana.sarif.json` as read-only baseline evidence.
- Fix findings; do not suppress inspections.
- Preserve behavior and unrelated changes.
- Use TokenSave before source-file reads.
- Workers must not commit or edit outside owned files.

---

### Task 1: UML analysis qualifications (140 findings)

**Files:** Modify `crates/waml/src/uml/analysis.rs`.

- [ ] Filter SARIF to this file and record all 140 findings.
- [ ] Confirm each redundant path resolves identically without its prefix.
- [ ] Remove only reported unnecessary qualifications.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p waml --lib`.

### Task 2: CLI I/O findings (46 findings)

**Files:** Modify `crates/waml-cli/src/io.rs`.

- [ ] Filter SARIF to this file and record 45 qualification findings plus one `RsUnwrap`.
- [ ] Remove only reported unnecessary qualifications.
- [ ] Replace the reported `unwrap()` with `?` only after confirming compatible error propagation.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p waml-cli`.

### Task 3: Editor app/session qualifications (38 findings)

**Files:** Modify `crates/waml-editor/src/app.rs` and `crates/waml-editor/src/editor_session.rs`.

- [ ] Filter SARIF to these files and record all 38 findings.
- [ ] Remove only reported unnecessary qualifications.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo check -p waml-editor`.

### Task 4: Remaining editor findings (48 findings)

**Files:** Modify only:
`crates/waml-editor/src/app/actions.rs`,
`behavior_doc_view.rs`,
`tree_panel.rs`,
`canvas/viewport.rs`,
`class_diagram_view.rs`,
`card/mod.rs`,
`document_host.rs`,
`scene.rs`,
`canvas/class/selection.rs`,
`script_gate.rs`,
`bin/icon_harness.rs`,
`canvas/class/placement.rs`,
`shortcuts_overlay.rs`,
`inspector.rs`.

- [ ] Filter SARIF to these files and record all 48 findings.
- [ ] Apply reported qualification, parentheses, and trait-member-order fixes without unrelated refactors.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo check -p waml-editor --all-targets`.

### Task 5: WAML integration tests (42 findings)

**Files:** Modify only:
`crates/waml/tests/uml_classifier_syntax.rs`,
`uml_diagram_syntax.rs`,
`interaction_solver_golden.rs`,
`syntax_actions.rs`,
`specialization_composition.rs`,
`golden.rs`,
`okf_lowering_order.rs`,
`compat_lowering_order.rs`,
`uml_lowering_order.rs`.

- [ ] Filter SARIF to these files and record all 42 findings.
- [ ] Apply qualification fixes.
- [ ] Replace six reported `unwrap()` calls with compatible `?` propagation.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p waml --tests`.

### Task 6: Syntax and parser-platform findings (19 findings)

**Files:** Modify only:
`crates/waml/tests/parser_platform_properties.rs`,
`crates/waml-syntax/tests/shell_roundtrip.rs`,
`crates/waml-syntax/tests/properties.rs`,
`crates/waml-syntax/tests/incremental.rs`,
`crates/waml-syntax/src/incremental.rs`.

- [ ] Filter SARIF to these files and record all 19 findings.
- [ ] Apply reported qualification, parentheses, assertion, thread-local, and trait-order fixes.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p waml-syntax`.
- [ ] Run `cargo test -p waml --test parser_platform_properties`.

### Task 7: Remaining core and DTO findings (16 findings)

**Files:** Modify only:
`crates/waml/src/model.rs`,
`ops/mod.rs`,
`compat.rs`,
`solve/sizing.rs`,
`solve/geometry.rs`,
`uml/lower.rs`,
`uml/syntax/ast.rs`,
`uml/syntax/mod.rs`,
`crates/waml-ops-dto/src/lib.rs`.

- [ ] Filter SARIF to these files and record all 16 findings.
- [ ] Apply reported qualification, parentheses, and assertion fixes.
- [ ] Rename the `from_*` instance method and all of its callers to a behavior-neutral name.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p waml --lib`.
- [ ] Run `cargo test -p waml-ops-dto`.

### Task 8: CLI LSP findings (4 findings)

**Files:** Modify `crates/waml-cli/src/lsp/server.rs` and `crates/waml-cli/src/lsp/map.rs`.

- [ ] Filter SARIF to these files and record all four findings.
- [ ] Apply reported qualification, parentheses, and trait-member-order fixes.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo test -p waml-cli`.

### Task 9: Cargo manifest findings (5 findings)

**Files:** Modify `Cargo.toml`, `Cargo.lock`, `crates/waml/Cargo.toml`, and `crates/waml-editor/Cargo.toml`.

- [ ] Filter SARIF to the three manifest files and record four version findings plus one unused dependency.
- [ ] Remove `pulldown-cmark` from `crates/waml/Cargo.toml` only after confirming that crate has no use.
- [ ] Update only the four reported dependency versions to the reported compatible versions.
- [ ] Refresh `Cargo.lock`.
- [ ] Run `cargo check --workspace --all-targets`.

### Final verification

- [ ] Confirm each original SARIF result maps to an applied change or a documented false positive.
- [ ] Run `cargo fmt --all -- --check`.
- [ ] Run `cargo build --workspace --all-targets`.
- [ ] Run `cargo test --workspace`.
- [ ] Rerun Qodana when tooling is available and compare remaining findings with the 358-result baseline.
- [ ] Review the full branch diff for scope, behavior changes, and inspection suppression.
