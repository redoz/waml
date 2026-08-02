# Root Launcher Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the native editor launchers to `run.ps1` and `run.sh` at the repository root.

**Architecture:** Keep each launcher implementation unchanged except for repository-root discovery. Remove the old files instead of adding compatibility wrappers. Update only active documentation; historical plans and specifications stay unchanged.

**Tech Stack:** PowerShell, Bash, Cargo, Markdown

## Global Constraints

- Do not keep compatibility launchers at the old paths.
- Keep all current arguments and launch behavior.
- Each launcher must treat its own directory as the repository root.
- Do not change historical specifications and completed plans.

---

### Task 1: Move the launchers and active references

**Files:**
- Create: `run.ps1`
- Create: `run.sh`
- Delete: `scripts/run-native.ps1`
- Delete: `scripts/run-native.sh`
- Modify: `README.md:12-13`
- Modify: `.claude/skills/run/SKILL.md:3-36`

**Interfaces:**
- Consumes: The current PowerShell parameters `Fixture`, `Empty`, `Optimized`, `Title`, and `Color`; the current Bash `-o` and `--optimized` options and fixture argument.
- Produces: Root commands `pwsh ./run.ps1` and `./run.sh` with the same behavior and exit codes.

- [ ] **Step 1: Run the structural check and verify it fails**

```powershell
$required = @('run.ps1', 'run.sh')
$removed = @('scripts/run-native.ps1', 'scripts/run-native.sh')
if (($required | Where-Object { -not (Test-Path $_) }).Count -or
    ($removed | Where-Object { Test-Path $_ }).Count) { exit 1 }
```

Expected: exit code 1 because the root launchers do not exist and the old launchers still exist.

- [ ] **Step 2: Move the PowerShell launcher and update root discovery**

Move the full content to `run.ps1`. Use these header and root lines:

```powershell
#!/usr/bin/env pwsh
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./run.ps1 [path-to-fixture]
#        ./run.ps1 -Empty       # no bundle -> start screen
#        ./run.ps1 -Optimized   # release build (optimized)

$root = $PSScriptRoot
```

Keep the parameter block, process isolation, build, and run commands unchanged.

- [ ] **Step 3: Move the Bash launcher and update root discovery**

Move the full content to `run.sh`. Use these header and root lines:

```bash
#!/usr/bin/env bash
# Launch the native waml-editor on a fixture (defaults to tests/fixtures/mini).
# Usage: ./run.sh [-o|--optimized] [path-to-fixture]
#        -o / --optimized   release build (optimized)
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
```

Keep option parsing and the Cargo command unchanged.

- [ ] **Step 4: Update active documentation**

Change README commands to `run.ps1` and `run.sh`. Change every `scripts/run-native.ps1` reference in `.claude/skills/run/SKILL.md` to `run.ps1`. Do not edit old plans or specifications.

- [ ] **Step 5: Run syntax and structure verification**

```powershell
$tokens = $null
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile(
    (Resolve-Path ./run.ps1),
    [ref]$tokens,
    [ref]$errors
) | Out-Null
if ($errors.Count) { $errors; exit 1 }
pwsh -NoProfile -File ./run.ps1 -? *> $null
```

Do not use the last command if PowerShell routes `-?` into a build. The parser check is the safe PowerShell syntax gate.

```bash
bash -n ./run.sh
```

Run the structural check from Step 1 again. Expected: exit code 0.

- [ ] **Step 6: Check active and historical references**

```powershell
rg -n 'scripts[/\\]run-native' README.md .claude/skills/run/SKILL.md
rg -n 'run\.ps1|run\.sh' README.md .claude/skills/run/SKILL.md
```

Expected: the first command returns no matches. The second command shows only root launcher paths.

- [ ] **Step 7: Commit the focused change**

```bash
git add run.ps1 run.sh scripts/run-native.ps1 scripts/run-native.sh README.md .claude/skills/run/SKILL.md docs/superpowers/plans/2026-08-02-root-run-launchers.md
git commit -m "chore: move run launchers to root"
```
