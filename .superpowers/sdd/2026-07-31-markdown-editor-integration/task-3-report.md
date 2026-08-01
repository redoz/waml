# Task 3 report: host snapshot synchronization

Status: **DONE**

Date: 2026-08-01

## Outcome

Task 3 adds parser-free synchronization from application snapshots to the
widget-local Markdown session. Acknowledged local edits preserve selection,
pixel scroll, preferred state, local history, and IME state. Older host
snapshots return `IgnoredStale` without state changes.

Application history and accepted external replacements install the supplied
`MarkdownDocumentSnapshot` and syntax `Arc` values directly. A valid supplied
change map translates selections and preserves pixel scroll. A missing or
unsafe selection map installs a caret at byte offset zero and resets pixel
scroll. Accepted replacements clear local undo/redo, IME state, and the
preferred vertical column.

The boundary rejects acknowledgement revision mismatches, equal-revision text
or syntax identity mismatches, invalid change maps, and change lists that do
not produce the supplied snapshot. These errors are typed as
`MarkdownEditError::HostSnapshot` and do not mutate the session.

Geometry scroll-anchor restoration and motion-cut assertions remain in Task 4,
as required by compatibility decision 5. Task 3 changes no widget geometry or
motion behavior.

## RED/GREEN evidence

- The focused RED gate reported 27 expected compile errors for the missing
  host-sync types, method, and public scroll-state accessors.
- `rtk cargo test -p waml-markdown-editor --test document_ops host_`: 12 passed.
- `rtk cargo test -p waml-markdown-editor --test document_ops`: 34 passed.
- `rtk cargo test -p waml-markdown-editor --test unicode_ime`: 16 passed.
- `rtk cargo test -p waml-markdown-editor --test motion`: 9 passed.
- `rtk cargo test -p waml-markdown-editor --tests`: 216 passed across 14 suites.
- `rtk cargo clippy -p waml-markdown-editor --tests -- -D warnings`: no code
  errors or warnings. Cargo reported only the two existing duplicate-package
  warnings from the Makepad checkout.
- `rtk cargo fmt --all -- --check` and `rtk git diff --check`: passed.

The unrelated untracked
`crates/waml-syntax/tests/properties.proptest-regressions` file remains
unchanged and unstaged.

TokenSave semantic exploration and impact review saved approximately 71,000
tokens.
