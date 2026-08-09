# Final Fix Report

## Status

Complete. One generic header click now changes only the generic document's
source/rendered surface. It does not also change Markdown emphasis or replace
the generic header chrome.

## Root cause

`GenericOkfView::handle` recognized the shared header button action, changed
the generic surface, and then passed the same immutable `Actions` batch to
`SourceView::handle`. `SourceView` recognized the same button action as its
emphasis action. It changed `Code` to `Layout` and projected source-view chrome
over the generic chrome.

## RED evidence

The action-level regression test was added before the dispatch fix:

```text
rtk cargo test -p waml-editor \
  generic_okf_view::tests::source_toggle_action_changes_only_the_generic_surface \
  -- --nocapture

FAILED
the generic surface toggle must not also toggle emphasis
left: Layout
right: Code
```

The unit harness was first corrected to mount a real `IconButton`. This made
the test prove that the action reached the production click detector before it
asserted the incorrect second state change.

Strict Clippy also reproduced the reviewed factory-signature issue before the
refactor:

```text
rtk cargo clippy -p waml-editor --all-targets -- -D warnings
error: this function has too many arguments (8/7)
  --> crates/waml-editor/src/documents.rs:196:1
```

## Implementation

- Return from `GenericOkfView::handle` after it consumes its header action.
- Keep the generic chrome at `View rendered` with no right dock after source
  becomes visible.
- Pass the existing `OpenCtx` to `open_on_surface`. The dispatcher now accepts
  three arguments and needs no Clippy allow.
- Rename the shared button accessors to `header_view_action_button` and
  `view_action_button` because the action can change source/rendered state or
  Markdown emphasis.
- Update header comments so chrome projection, not active-view icon mutation,
  is the documented icon and tooltip authority.
- Add a factory-level `Layout` test for direct source creation and locator
  reopen.

## Files

- `crates/waml-editor/src/generic_okf_view.rs`
- `crates/waml-editor/src/documents.rs`
- `crates/waml-editor/src/document_header.rs`
- `crates/waml-editor/src/doc_view.rs`
- `crates/waml-editor/src/source_toggle_view.rs`
- `crates/waml-editor/src/source_view.rs`
- `.superpowers/sdd/2026-08-09-markdown-emphasis-toggle/final-fix-report.md`

## GREEN evidence

```text
rtk cargo test -p waml-editor generic_okf_view::tests -- --nocapture
6 passed

rtk cargo test -p waml-editor \
  documents::tests::layout_emphasis_reaches_direct_and_reopened_source_factories \
  -- --nocapture
1 passed

rtk cargo test -p waml-editor source_view::tests -- --nocapture
17 passed

rtk cargo test -p waml-editor source_toggle_view::tests -- --nocapture
4 passed

rtk cargo test -p waml-editor document_header::tests -- --nocapture
20 passed

rtk cargo test -p waml-editor doc_view::tests -- --nocapture
20 passed

rtk cargo test -p waml-editor
1133 passed, 5 ignored

rtk cargo clippy -p waml-editor --all-targets -- -D warnings
0 Clippy errors

rtk cargo fmt --all -- --check
exit 0

rtk git diff --check
exit 0
```

Cargo prints two duplicate-package notices for Makepad's vendored `bitflags`
and `cfg-if` manifests. They are dependency-resolution notices, not Rust or
Clippy warnings.

## Commit

`fix(editor): consume generic view action once`

The fixes and this report are committed together. The final handoff records
the hash assigned to that commit.

## Self-review

- Removing the early return makes the action regression fail with `Layout`
  instead of `Code` and lets source chrome overwrite generic chrome.
- The regression uses a real `WidgetAction` and real `IconButton`; it does not
  assert on a mock.
- The factory test derives its expected destination action independently:
  `Layout` must offer `Code` with `Use code emphasis` on both paths.
- `OpenCtx` retains the same analysis, UML, asset host, emphasis, limits, and
  mask values at each caller boundary.
- No configuration write, per-tab persistence, settings UI, or unrelated
  production abstraction was added.
- TokenSave provided the dispatch, factory, and affected-test graph context and
  saved approximately 37,817 tokens.

## Concerns

None in the changed code. The Makepad duplicate-package notices remain
external to this fix wave.
