# Root Run Launchers Design

## Goal

Make the native editor launchers easy to find from the repository root.

## Changes

- Rename `scripts/run-native.ps1` to `run.ps1`.
- Rename `scripts/run-native.sh` to `run.sh`.
- Do not keep compatibility launchers at the old paths.
- Keep all current arguments and launch behavior.
- Update the README and the active local `run` skill to use the new paths.
- Do not change historical specifications and completed plans.

## Path Handling

Each launcher must treat its own directory as the repository root. This keeps launches correct from the main checkout and from Git worktrees.

## Verification

- Parse both PowerShell files without errors.
- Run Bash syntax validation on `run.sh`.
- Check that active documentation uses the root launchers.
- Check that the old launcher files no longer exist.
