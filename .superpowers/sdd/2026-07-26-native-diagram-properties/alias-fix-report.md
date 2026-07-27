# Native Save Clean/Dirty Alias Fix

Date: 2026-07-27

Baseline: `cb1b56d`

Implementation commit: this report is committed with
`fix(editor): reject clean-dirty save aliases`

## Finding

`save_bundle_atomic` skipped clean logical paths before registering their canonical
filesystem targets. A dirty path through a directory symlink and a clean path through the
real directory could therefore resolve to the same file without triggering duplicate-target
detection. Saving the dirty alias replaced the shared file while the clean logical document
still retained its loaded snapshot.

## Fix

- Canonicalize and register every current path in a read-only validation pass before dirty
  filtering.
- Reject any pair of logical paths that resolves to the same target before directory creation
  or file replacement.
- Continue limiting disk-content conflict checks and write planning to dirty paths, preserving
  unrelated external edits to clean files.
- Preserve the existing containment checks, atomic replacement, and already-desired retry
  behavior.

## TDD evidence

Added
`clean_and_dirty_aliases_are_rejected_before_mutating_shared_target`, which creates
`linked/diagram.md` and `real/diagram.md` as aliases of one real file. The linked path is dirty
and the real path is clean.

Before the production change, the focused test failed because `save_bundle_atomic` returned
`Ok(())`; the shared target was rewritten. After the validation-pass change, it returns
`InvalidInput` and the target remains byte-for-byte at the loaded content.

The Windows test host successfully created the directory symlink for the observed RED/GREEN
cycle. On hosts that cannot create directory symlinks because of platform permissions, the
fixture follows the existing native-save convention and skips.

## Verification

- Focused regression: 1 passed.
- Native-save module: 13 passed.
- Full `waml-editor`: 683 passed across 5 suites.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p waml-editor --all-targets -- -D warnings`: passed with no lint errors.
  Cargo emitted two pre-existing duplicate-Makepad-checkout package warnings.
- `git diff --check`: passed.

## Residual considerations

- The pre-existing filesystem time-of-check/time-of-use window between canonical validation and
  replacement remains; fully closing it requires platform-specific handle-relative traversal.
- Directory-link fixture coverage depends on the host permitting symlink creation.
