# App Composition Root Reorganization: Final Fix Report

Date: 2026-08-02

Worktree: `C:\dev\waml\.worktrees\app-composition-root-reorg`

Branch: `codex/app-composition-root-reorg`

## Result

The final review fix wave is complete. It changes two source files. It does not
change runtime behavior. No high-level native `open_dir` rollback test was added.
The test would need a new failure seam or a fragile filesystem fixture.

TokenSave was used before source-file inspection. Its reported savings for this
review were approximately 33,758 tokens.

## Reviewer Claim Verification

### 1. Private shell helpers

The reviewer claim was correct.

TokenSave caller queries and an absolute-path `rg` check found these call sites:

- `sync_diagram_switcher_current` is called only by `sync_document_shell` in
  `app/shell.rs`.
- `set_shortcuts_overlay` is called only by `toggle_shortcuts_overlay` and
  `close_page_overlays` in `app/shell.rs`.
- `sync_tree_gap` is called only by `sync_dock_slots` in `app/shell.rs`.

All three methods changed from `pub(super) fn` to private `fn`. No caller moved
and no public API changed.

### 2. Workspace failure comments

The reviewer claim was correct.

The old `open_dir` comment said that only model-load failure can return `false`.
The implementation has four failure groups:

- a required save can fail;
- the bundle load can fail;
- native asset-root policy construction or canonicalization can fail;
- `open_bundle` can fail during replacement-session analysis.

The comment now lists these groups and states that the caller keeps the current
screen visible.

The old `open_bundle` comment said that the function always returns `true`.
`EditorSession::replace` can return an analysis error. The comment now states
that replacement-session analysis can return `false`. It also states that file
reading and `SourceBundle` construction have already completed.

No runtime code changed for this item.

### 3. High-level native rollback test

The requested test is not practical under the approved constraints. This is a
technical YAGNI deferral for a defense-in-depth minor.

The native call path is:

1. `open_dir` saves the current document when required.
2. `load::read_bundle` reads Markdown files and constructs a `SourceBundle`.
3. `MarkdownAssetPolicy::native` validates and canonicalizes the next root.
4. `open_dir` installs the candidate Markdown asset host.
5. `open_bundle` calls `EditorSession::replace`.
6. If session analysis fails, `restore_markdown_asset_host_after_open` restores
   the previous host.

After step 4, the only normal `false` result is the analysis error in step 5.
The current parser accepts malformed Markdown as syntax with diagnostics. Small,
portable disk fixtures do not produce this failure. The remaining failure paths
are internal analysis invariants, test-only analysis probes, or a source that is
too large for the syntax offset range. Filesystem name tricks would be
platform-dependent. An injected failure would require a new controller,
production test hook, or other test seam.

The existing test
`failed_open_restores_the_previous_markdown_asset_root` directly verifies the
rollback helper with real shared asset hosts and pointer identity. The production
`open_dir` call path was inspected and still passes the `open_bundle` result to
that helper before it updates `open_dir` or recents. This direct test remains the
proportionate coverage.

## Verification

All commands ran from the assigned worktree through `rtk`.

### Baseline

```text
rtk cargo test -p waml-editor app::tests::shell
Exit 0: 3 passed, 1005 filtered out (12 suites, 3.46s)

rtk cargo test -p waml-editor app::tests::workspace
Exit 0: 9 passed, 999 filtered out (12 suites, 0.00s)
```

### Per-item checks after the edits

```text
rtk cargo test -p waml-editor app::tests::shell
Exit 0: 3 passed, 1005 filtered out (12 suites, 3.45s)

rtk cargo test -p waml-editor app::tests::workspace
Exit 0: 9 passed, 999 filtered out (12 suites, 0.00s)

rtk cargo test -p waml-editor failed_open_restores_the_previous_markdown_asset_root
Exit 0: 1 passed, 1007 filtered out (12 suites, 0.00s)
```

### Formatting and required focused suites

```text
rtk cargo fmt --all -- --check
Exit 0

rtk cargo fmt --all
Exit 0

rtk cargo test -p waml-editor app::tests
Exit 0: 47 passed, 4 ignored, 957 filtered out (12 suites, 4.83s)

rtk cargo test -p waml-editor app::tests::workspace
Exit 0: 9 passed, 999 filtered out (12 suites, 0.00s)

rtk cargo test -p waml-editor app::tests::shell
Exit 0: 3 passed, 1005 filtered out (12 suites, 3.46s)
```

### Full suite and Clippy

```text
rtk cargo test -p waml-editor
Exit 0: 1004 passed, 4 ignored (13 suites, 14.16s)

rtk cargo clippy -p waml-editor --all-targets -- -D warnings
Exit 0: 0 Clippy errors
```

The raw Clippy run finished successfully. Cargo printed two pre-existing
duplicate-package selection warnings for Makepad's `bitflags` and `cfg-if`
packages. These are Cargo graph warnings, not Clippy lint warnings. `-D warnings`
reported no source warnings.

## Diff Self-review

`git diff --check` and `git diff --cached --check` both exited with status 0.

The code commit contains:

- `crates/waml-editor/src/app/shell.rs`: three visibility reductions only;
- `crates/waml-editor/src/app/workspace.rs`: two documentation corrections only.

The code diff has 9 insertions and 8 deletions across 2 files. The call-site
search found no cross-module use of the three methods. No controller, trait,
dependency, test hook, or filesystem fixture was added. The worktree was clean
after the code commit.

## Commits

- Review base: `22675c7621cefa5779a8dac0c029050b01c6dc0f`
  (`refactor(editor): expose app event phases`)
- Final code fix: `28cb9a1830f206306fefb22b53e262b7f078d8f7`
  (`refactor(editor): tighten app internals`)

This report is committed separately so it can record the final code-fix hash.
