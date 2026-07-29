# Task 21 Authority Boundary Correction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Task 21's unsound Rust-source interpreter with compiler- and Cargo-enforced parser boundaries, while restoring the runtime code and diagnostic that the interpreter distorted.

**Architecture:** `waml::uml::syntax` owns a private parser module and exposes one `pub(in crate::uml)` full-parse facade to legitimate UML consumers. A small integration suite checks only finite retired surfaces, Cargo's declared workspace dependency graph, and rustc privacy failures; ordinary review remains responsible for deliberate architecture changes inside authorized parser-owning modules.

**Tech Stack:** Rust 2021, Cargo metadata v1, `serde_json`, `waml-syntax` green/red trees, WAML integration tests, PowerShell commands through `rtk`.

## Global Constraints

- Work only in the `parser-platform-implementation` worktree.
- Preserve the approved parser-platform architecture, public `waml` contracts, Rust 2021, and the workspace MSRV.
- `SourceBundle` remains the sole source/document authority; its mutation helpers remain `pub(crate)`.
- `prepare_candidate` remains the public analysis choke point.
- Exact `SyntaxTree` writing, canonical `uml::Formatter` output, and semantic `Model` serialization remain distinct.
- Keep the `waml-syntax` crate and its public domain-neutral syntax API.
- Preserve generic OKF behavior, Index/Log separation, static specialization composition, recovery diagnostics, and atomic editor/CLI/LSP preparation.
- Do not add incremental parsing, `TextChange`, `ChangeMap`, safe-window selection, green reuse, remapping, retention, previous snapshots, or any other Task 22 behavior.
- Do not add a broad Rust source scanner, parser classifier, type inference, macro expansion, taint propagation, call-graph interpreter, or semantic-source authority proof.
- Do not manufacture RED by breaking already-green production code. Use the real intermediate visibility failure, the current target-less diagnostic, or a temporary test-only probe.
- Do not modify, stage, or commit unrelated work. In particular, never stage `.superpowers/sdd/2026-07-28-parser-platform-implementation/task-7-report.md`.
- Prefix every shell command with `rtk`.
- Before every commit, inspect `rtk git status --short` and `rtk git diff --cached --name-only`; the staged list must equal the paths explicitly named by that task.
- Implementation begins only after this plan is committed. The execution preflight must prove the plan is tracked and that the known Task 7 report is the only worktree change.

---

### Task 1: Add the Private Full-Parse Facade and Migrate Its Callers

**Files:**
- Modify: `crates/waml/src/uml/syntax/mod.rs`
- Modify: `crates/waml/src/uml/syntax/parser.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/uml/lower.rs`
- Modify: `crates/waml/src/uml/rename.rs`

**Interfaces:**
- Consumes: `waml_syntax::{MarkdownStructureMap, SourceText, SyntaxTree}` and the existing `UmlLanguage`.
- Produces:

```rust
pub(in crate::uml) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>>
```

- Keeps the implementation entry point at:

```rust
pub(super) fn parse(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>>
```

- `parser` itself is declared `mod parser;`, not `pub`, `pub(crate)`, or `pub(in crate::uml)`.
- All four current raw callers migrate: `uml/analysis.rs::analyze`, `uml/lower.rs::UmlLoweringState::reparse`, `uml/lower.rs::referrers_source`, and `uml/rename.rs::rename_typed_references`.

- [ ] **Step 1: Verify the implementation handoff**

Run:

```powershell
rtk git ls-files --error-unmatch docs/superpowers/plans/2026-07-29-task-21-authority-boundary-implementation.md
rtk git status --short
```

Expected: the first command prints the plan path, and status reports only the
known unstaged modification to
`.superpowers/sdd/2026-07-28-parser-platform-implementation/task-7-report.md`.
Stop if the plan is untracked or any unexpected path is dirty.

- [ ] **Step 2: Make the real visibility change first**

In `crates/waml/src/uml/syntax/mod.rs`, change:

```rust
pub(in crate::uml) mod parser;
```

to:

```rust
mod parser;
```

In `crates/waml/src/uml/syntax/parser.rs`, change:

```rust
pub fn parse(
```

to:

```rust
pub(super) fn parse(
```

Do not add the facade or change callers yet. This is the first half of the real implementation, not a throwaway production break.

- [ ] **Step 3: Run the compile RED**

Run:

```powershell
rtk cargo check -p waml --all-features
```

Expected: FAIL with `E0603` privacy errors at the four direct `parser::parse` call sites in analysis, lowering, and rename. A dependency, manifest, or unrelated type error is not the expected RED.

- [ ] **Step 4: Add the narrow facade**

At the top of `crates/waml/src/uml/syntax/mod.rs`, add the required imports and the facade:

```rust
use std::sync::Arc;

use waml_syntax::{MarkdownStructureMap, SourceText, SyntaxTree};

mod ast;
mod kind;
mod parser;

pub(in crate::uml) fn parse_full(
    text: SourceText,
    structure: &MarkdownStructureMap,
) -> Arc<SyntaxTree<UmlLanguage>> {
    parser::parse(text, structure)
}
```

Keep all existing AST/kind re-exports and `UmlLanguage` unchanged below this block. Do not re-export `parser::parse`, add a compatibility alias, or make `parse_full` public outside `crate::uml`.

- [ ] **Step 5: Route analysis through the facade**

In `crates/waml/src/uml/analysis.rs`, replace:

```rust
syntax::{parser, UmlLanguage},
```

with:

```rust
syntax::{self, UmlLanguage},
```

and replace:

```rust
let tree = parser::parse(document.text().clone(), structure);
```

with:

```rust
let tree = syntax::parse_full(document.text().clone(), structure);
```

- [ ] **Step 6: Route lowering and rename through the facade**

In both raw call sites in `crates/waml/src/uml/lower.rs`, replace:

```rust
super::syntax::parser::parse(text, &shell.structure)
```

and:

```rust
super::syntax::parser::parse(text, &parsed.structure)
```

with:

```rust
super::syntax::parse_full(text, &shell.structure)
```

and:

```rust
super::syntax::parse_full(text, &parsed.structure)
```

In `crates/waml/src/uml/rename.rs`, replace:

```rust
let tree = super::syntax::parser::parse(text, &shell.structure);
```

with:

```rust
let tree = super::syntax::parse_full(text, &shell.structure);
```

- [ ] **Step 7: Prove there are no legitimate raw callers**

Run:

```powershell
rtk rg -n 'syntax::parser|parser::parse' crates/waml/src --glob '*.rs'
```

Expected: the only match is `parser::parse(text, structure)` inside `crates/waml/src/uml/syntax/mod.rs`. There must be no raw call in analysis, lower, rename, tests, or compatibility code.

- [ ] **Step 8: Run the compile GREEN and focused parser gates**

Run:

```powershell
rtk cargo check -p waml --all-features
rtk cargo test -p waml --test uml_diagram_syntax --test layout_atom_api --test semantic_diagnostics
```

Expected: PASS. Existing exact syntax writing, typed UML syntax, recovery, layout compatibility, and semantic diagnostics remain unchanged.

- [ ] **Step 9: Commit only the facade slice**

Run:

```powershell
rtk git add -- crates/waml/src/uml/syntax/mod.rs crates/waml/src/uml/syntax/parser.rs crates/waml/src/uml/analysis.rs crates/waml/src/uml/lower.rs crates/waml/src/uml/rename.rs
rtk git diff --cached --name-only
rtk git status --short
rtk git commit -m "refactor(parser): seal UML parser facade"
```

Expected staged paths: exactly the five files listed above. The dirty Task 7 report and every unrelated path remain unstaged.

---

### Task 2: Replace the Authority Interpreter with Honest Boundary Checks

**Files:**
- Replace: `crates/waml/tests/no_legacy_authority.rs` (currently 2,506 lines)
- Delete: `crates/waml/tests/support/authority_guard.rs` (currently 5,953 lines)
- Delete: `crates/waml/tests/fixtures/authority-guard/`
- Modify: `crates/waml/Cargo.toml`

**Interfaces:**
- Consumes: Cargo metadata v1 JSON, `env!("CARGO")`, `env!("CARGO_MANIFEST_DIR")`, and Rust's `E0603` privacy diagnostic.
- Produces four focused tests:
  - `retired_legacy_files_and_public_surface_are_absent`
  - `only_waml_directly_depends_on_waml_syntax`
  - `raw_parser_module_is_private_to_external_crates`
  - `full_parse_facade_is_private_to_external_crates`
- The replacement suite proves only named file/surface removal, direct workspace dependency ownership, and external privacy. It makes no claims about arbitrary Rust programs.

- [ ] **Step 1: Implement the privacy harness and run a temporary test-only RED probe**

Replace the entire contents of `crates/waml/tests/no_legacy_authority.rs` with
only the standard-library imports, the complete external fixture helper below,
and the temporary RED probe shown afterward. Remove the old
`authority_guard` module declaration, old one-argument `compile_external`
helper, and all old interpreter tests in this same test-only rewrite. A test
filter still compiles the entire integration-test crate, so none of the old
suite may coexist with the new helper signature during this RED.

Add the complete external fixture helper:

```rust
fn compile_external(case: &str, source: &str) -> std::process::Output {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "waml-authority-{case}-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("src")).expect("create external fixture");

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_string_lossy()
        .replace('\\', "/");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\n\
             name = \"authority-api-{case}\"\n\
             version = \"0.0.0\"\n\
             edition = \"2021\"\n\n\
             [dependencies]\n\
             waml = {{ path = \"{manifest_dir}\" }}\n"
        ),
    )
    .expect("write external fixture manifest");
    fs::write(root.join("src/main.rs"), source).expect("write external fixture source");

    let output = Command::new(env!("CARGO"))
        .args(["check", "--quiet", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .output()
        .expect("run external cargo check");
    fs::remove_dir_all(&root).expect("remove external fixture");
    output
}

fn assert_privacy_failure(case: &str, source: &str, expected_item: &str) {
    let output = compile_external(case, source);
    assert!(!output.status.success(), "{case} unexpectedly compiled");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[E0603]"),
        "{case} was not an E0603 privacy failure:\n{stderr}"
    );
    assert!(
        stderr.contains("private"),
        "{case} did not report privacy:\n{stderr}"
    );
    assert!(
        stderr.contains(expected_item),
        "{case} did not name `{expected_item}`:\n{stderr}"
    );
    for unrelated in [
        "failed to get",
        "no matching package",
        "failed to parse manifest",
        "could not find `Cargo.toml`",
    ] {
        assert!(
            !stderr.contains(unrelated),
            "{case} failed for an unrelated reason:\n{stderr}"
        );
    }
}
```

Import `BTreeSet`, `fs`, `Path`, `PathBuf`, `Command`, `SystemTime`, and
`UNIX_EPOCH` from the standard library. Then temporarily call the helper with
an actually public item:

```rust
#[test]
fn red_probe_rejects_a_successful_external_compile() {
    assert_privacy_failure(
        "public-control",
        "fn main() { let _ = waml::uml::recognizes; }",
        "recognizes",
    );
}
```

Run:

```powershell
rtk cargo test -p waml --test no_legacy_authority red_probe_rejects_a_successful_external_compile -- --exact
```

Expected: FAIL because the external crate compiles successfully. This proves
the helper requires a real privacy failure rather than merely a non-zero Cargo
status. Remove this temporary probe immediately after observing RED; do not
change visibility of a green production item.

- [ ] **Step 2: Replace the file/export/surface check with finite literals**

With the old module declaration and interpreter tests already removed in Step
1, remove the temporary RED probe and add the permanent finite check below.
Keep the new fixture helpers unchanged:

```rust
#[test]
fn retired_legacy_files_and_public_surface_are_absent() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for retired in ["grammar.rs", "parse.rs", "syntax.rs", "serialize.rs"] {
        assert!(
            !manifest.join("src").join(retired).exists(),
            "retired legacy authority file still exists: {retired}"
        );
    }

    let lib = fs::read_to_string(manifest.join("src/lib.rs")).expect("read waml lib.rs");
    for export in [
        "pub mod grammar;",
        "pub mod parse;",
        "pub mod syntax;",
        "pub mod serialize;",
    ] {
        assert!(!lib.contains(export), "retired root export remains: {export}");
    }

    let public_surface = [
        ("pub struct ", "Document"),
        ("pub struct ", "Section"),
        ("pub struct ", "Line"),
        ("pub struct ", "ErrorNode"),
        ("pub fn ", "parse_document"),
        ("pub fn ", "build_model"),
        ("pub fn ", "build_model_from_source"),
        ("pub fn ", "project_okf"),
        ("pub fn ", "serialize_document"),
    ];
    for entry in fs::read_dir(manifest.join("src")).expect("read waml src") {
        let path = entry.expect("read src entry").path();
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read root Rust module");
        for (prefix, retired) in public_surface {
            let needle = format!("{prefix}{retired}");
            let remains = source.match_indices(&needle).any(|(start, _)| {
                match source[start + needle.len()..].chars().next() {
                    None => true,
                    Some(next) => !(next.is_ascii_alphanumeric() || next == '_'),
                }
            });
            assert!(
                !remains,
                "retired public surface `{retired}` remains in {}",
                path.display()
            );
        }
    }
}
```

This deliberately checks root modules and exact named public identifiers. The
identifier-boundary rule must allow retained names such as `DocumentId`,
`DocumentRevision`, `DocumentVersion`, and `DocumentCatalog`. Do not recurse
through arbitrary modules, parse Rust syntax, infer aliases, classify
parser-like functions, or reject the private formatter's unrelated `Section`
helper.

- [ ] **Step 3: Let Cargo define the dependency boundary**

Add a `workspace_root()` helper by walking two parents up from `CARGO_MANIFEST_DIR`. Invoke:

```rust
let output = Command::new(env!("CARGO"))
    .args(["metadata", "--format-version", "1", "--no-deps"])
    .current_dir(workspace_root())
    .output()
    .expect("run cargo metadata");
assert!(
    output.status.success(),
    "cargo metadata failed:\n{}",
    String::from_utf8_lossy(&output.stderr)
);
let metadata: serde_json::Value =
    serde_json::from_slice(&output.stdout).expect("parse cargo metadata JSON");
```

Collect `workspace_members` package IDs, then collect the names of workspace packages whose `dependencies` contain `name == "waml-syntax"` and whose `kind` is `null`, `"dev"`, or `"build"`. Assert:

```rust
let workspace_members = metadata["workspace_members"]
    .as_array()
    .expect("workspace_members array")
    .iter()
    .map(|id| id.as_str().expect("workspace member package id"))
    .collect::<BTreeSet<_>>();
let direct_users = metadata["packages"]
    .as_array()
    .expect("packages array")
    .iter()
    .filter(|package| {
        workspace_members.contains(
            package["id"]
                .as_str()
                .expect("workspace package id"),
        )
    })
    .filter(|package| {
        package["dependencies"]
            .as_array()
            .expect("package dependencies array")
            .iter()
            .any(|dependency| {
                dependency["name"].as_str() == Some("waml-syntax")
                    && matches!(
                        dependency["kind"].as_str(),
                        None | Some("dev") | Some("build")
                    )
            })
    })
    .map(|package| {
        package["name"]
            .as_str()
            .expect("workspace package name")
            .to_owned()
    })
    .collect::<BTreeSet<_>>();

assert_eq!(
    direct_users,
    BTreeSet::from(["waml".to_owned()]),
    "workspace packages with a direct waml-syntax dependency changed"
);
```

Do not read or reconstruct `Cargo.toml` files. Transitive host use through `waml` is allowed; a direct editor, CLI, DTO, or other workspace dependency must report the offending package name.

- [ ] **Step 4: Add both external privacy checks**

Add:

```rust
#[test]
fn raw_parser_module_is_private_to_external_crates() {
    assert_privacy_failure(
        "raw-parser",
        "fn main() { let _ = waml::uml::syntax::parser::parse; }",
        "parser",
    );
}

#[test]
fn full_parse_facade_is_private_to_external_crates() {
    assert_privacy_failure(
        "full-parse-facade",
        "fn main() { let _ = waml::uml::syntax::parse_full; }",
        "parse_full",
    );
}
```

Expected: both fail to compile specifically with `E0603`. Do not accept unresolved-import, dependency, or manifest errors as success.

- [ ] **Step 5: Delete the interpreter and its entire fixture maze**

Delete:

```text
crates/waml/tests/support/authority_guard.rs
crates/waml/tests/fixtures/authority-guard/
```

The fixture deletion includes both `outside-member/` and the entire nested `workspace/` tree. After the scan confirms no remaining use, remove `syn = { version = "2", features = ["full", "visit"] }` from `crates/waml/Cargo.toml`. Keep `serde_json`, which the Cargo metadata test uses.

Run:

```powershell
rtk rg -n 'authority_guard|analyze_sources|analyze_workspace|syn::' crates/waml
```

Expected: no match. Do not replace these with another support module or scanner.

- [ ] **Step 6: Run the honest authority suite GREEN**

Run:

```powershell
rtk cargo test -p waml --test no_legacy_authority
rtk cargo check -p waml --all-features
```

Expected: four focused authority tests PASS; both external cases prove actual privacy, metadata reports exactly `{waml}`, and no interpreter fixture is compiled.

- [ ] **Step 7: Commit only the authority-suite correction**

Run:

```powershell
rtk git add -A -- crates/waml/Cargo.toml crates/waml/tests/no_legacy_authority.rs crates/waml/tests/support/authority_guard.rs crates/waml/tests/fixtures/authority-guard
rtk git diff --cached --name-only
rtk git status --short
rtk git commit -m "test(parser): enforce honest authority boundaries"
```

Expected staged paths: the Cargo manifest, replacement test, deleted support file, and deleted fixture tree only. The Task 7 report remains unstaged.

---

### Task 3: Restore Runtime Code and Record Final Task 21 Evidence

**Files:**
- Modify: `crates/waml-editor/src/inspector.rs`
- Modify: `crates/waml/src/uml/lower.rs`
- Modify: `.superpowers/sdd/2026-07-28-parser-platform-implementation/task-21-report.md`
- Modify: `.superpowers/sdd/2026-07-28-parser-platform-implementation/progress.md`

**Interfaces:**
- Consumes: `UmlLoweringState::tree(candidate, target, op)`.
- Produces the exact missing-target reason:

```text
no claimed concept '{target}'
```

- Restores ordinary `Vec::with_capacity` and `Vec::new` expressions in editor production code.
- Preserves all Task 21 compatibility contracts and records evidence without retaining claims about AST interpretation, taint, macros, arbitrary dispatch, or source scanning.

- [ ] **Step 1: Add the exact diagnostic regression**

Add a small `#[cfg(test)] mod tests` at the end of `crates/waml/src/uml/lower.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_tree_error_names_requested_concept() {
        let source = SourceBundle::default();
        let mut state = UmlLoweringState {
            current_paths: BTreeMap::new(),
            touched_islands: BTreeMap::new(),
        };

        let error = match state.tree(&source, "missing-order", "attr.add") {
            Err(error) => error,
            Ok(_) => panic!("missing concept must fail"),
        };

        assert_eq!(error.op, "attr.add");
        assert_eq!(error.reason, "no claimed concept 'missing-order'");
    }
}
```

This unit test directly covers the private lookup that lost the target and does not depend on an earlier public-operation precondition.

- [ ] **Step 2: Run the known diagnostic RED**

Run:

```powershell
rtk cargo test -p waml --lib uml::lower::tests::missing_tree_error_names_requested_concept -- --exact
```

Expected: FAIL because the current reason is exactly `no claimed concept` without `'missing-order'`.

- [ ] **Step 3: Restore the target-bearing reason**

In `UmlLoweringState::tree`, replace:

```rust
.ok_or_else(|| EditError::at(op, "no claimed concept"))?;
```

with:

```rust
.ok_or_else(|| EditError::at(op, format!("no claimed concept '{target}'")))?;
```

Run:

```powershell
rtk cargo test -p waml --lib uml::lower::tests::missing_tree_error_names_requested_concept -- --exact
```

Expected: PASS with the exact operation and reason assertions.

- [ ] **Step 4: Restore ordinary editor collection expressions**

In `crates/waml-editor/src/inspector.rs`, replace:

```rust
let mut rows = std::vec::Vec::with_capacity(node_keys.len() + 1);
let mut associations = std::vec::Vec::new();
```

with:

```rust
let mut rows = Vec::with_capacity(node_keys.len() + 1);
let mut associations = Vec::new();
```

Do not otherwise refactor inspector behavior or imports.

- [ ] **Step 5: Run focused WAML and editor gates**

Run:

```powershell
rtk cargo test -p waml --test no_legacy_authority
rtk cargo test -p waml --test layout_atom_api --test uml_diagram_syntax --test semantic_diagnostics
rtk cargo test -p waml --test golden --test serde_shape --test layout_serde_roundtrip
rtk cargo test -p waml --test href_contract --test uml_lowering_authority --test uml_lowering_order
rtk cargo test -p waml-editor --all-features
```

Expected: PASS. These gates preserve the Task 21 public layout compatibility, parser/recovery behavior, semantic diagnostics, exact/golden/serde distinctions, authored href behavior, lowering authority/order, and editor behavior.

- [ ] **Step 6: Run full final verification**

Run:

```powershell
rtk cargo test --workspace --all-features
rtk cargo check --workspace --all-features
rtk cargo clippy --workspace --all-targets --all-features
rtk cargo fmt --all -- --check
rtk git diff --check
rtk rg -n 'syntax::parser|parser::parse' crates/waml/src --glob '*.rs'
rtk rg -n 'authority_guard|analyze_sources|analyze_workspace|syn::' crates/waml
rtk git diff --name-only
rtk git status --short
```

Expected:

- workspace tests, check, clippy, format check, and diff check PASS;
- the parser-path scan returns only the facade's internal `parser::parse(text, structure)` call;
- the interpreter scan returns no match;
- changed paths are limited to this plan's Task 1-3 files plus pre-existing unrelated dirty paths;
- `task-7-report.md` is still present exactly as a pre-existing unstaged change; and
- no Task 22 API or implementation appears in the diff.

- [ ] **Step 7: Rewrite the Task 21 report as honest final evidence**

Update `.superpowers/sdd/2026-07-28-parser-platform-implementation/task-21-report.md` so it:

- retains the historical parser-platform migration, semantic, compatibility, and runtime evidence that remains true;
- replaces the interpreter/taint/call-graph/macro/control-flow claims with the four focused authority tests;
- states that Rust privacy enforces the raw parser and `parse_full` boundary;
- states that Cargo metadata reports exactly `waml` as the direct workspace `waml-syntax` dependent;
- records deletion of the 2,506-line suite, 5,953-line support interpreter, fixture maze, and unused direct `syn` dev-dependency;
- records `mod parser`, `pub(super) parser::parse`, and `pub(in crate::uml) syntax::parse_full`;
- records restored `Vec` expressions and exact `no claimed concept '{target}'`;
- records every command and result from Steps 5 and 6;
- states explicitly that `SourceBundle` mutation and `prepare_candidate` boundaries are unchanged and no Task 22 work was included; and
- continues to state that the unrelated dirty Task 7 report was preserved and never staged.

Do not preserve the old claim that a residual AST pass proves arbitrary Rust parser/serializer authority.

- [ ] **Step 8: Update the SDD progress ledger**

Append these exact status lines to `.superpowers/sdd/2026-07-28-parser-platform-implementation/progress.md`:

```text
Task 21: authority-boundary correction complete (private full-parse facade; focused file, Cargo metadata, and rustc privacy checks)
Task 21: runtime restoration and final gates complete; no Task 22 implementation included
```

Do not edit any Task 1-20 entry and do not touch `task-7-report.md`.

- [ ] **Step 9: Commit the runtime and evidence slice**

Run:

```powershell
rtk git add -- crates/waml-editor/src/inspector.rs crates/waml/src/uml/lower.rs .superpowers/sdd/2026-07-28-parser-platform-implementation/task-21-report.md .superpowers/sdd/2026-07-28-parser-platform-implementation/progress.md
rtk git diff --cached --name-only
rtk git status --short
rtk git diff --cached --check
rtk git commit -m "fix(parser): restore honest Task 21 boundary"
```

Expected staged paths: exactly the four files named above. Never use `git add -A`, `git add .`, or a broad `.superpowers` path here; the dirty Task 7 report must remain unstaged.

- [ ] **Step 10: Perform the final self-review**

Review the complete three-commit diff against `docs/superpowers/specs/2026-07-29-task-21-authority-boundary-design.md` and confirm:

1. all ten acceptance criteria map to evidence in the report;
2. no public/raw parser alias or second preparation path exists;
3. no broad source interpreter or fixture maze remains;
4. types and visibility exactly match `parse_full(SourceText, &MarkdownStructureMap) -> Arc<SyntaxTree<UmlLanguage>>`, `pub(in crate::uml)`, and `pub(super)`;
5. the exact target-bearing diagnostic and ordinary `Vec` expressions are present;
6. Task 21 compatibility tests remain;
7. no Task 22 behavior is present; and
8. the staged/committed history never includes `task-7-report.md`.
