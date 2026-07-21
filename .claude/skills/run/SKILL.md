---
name: run
description: Launch the native waml-editor via scripts/run-native.ps1 — empty (start screen) or on a preset fixture. Use when the user asks to run / start / launch the editor, or wants a preset loaded.
---

# run

Launch the native `waml-editor` window. The script `scripts/run-native.ps1`
kills any running instance, rebuilds (`cargo build`), and only then runs — so a
stale window can't show old code and compile errors surface at the terminal.

## Presets

| Preset | Meaning | Command |
|--------|---------|---------|
| `empty` | No bundle → start screen | `pwsh scripts/run-native.ps1 -Empty` |
| `mini` (default) | `crates/waml-editor/tests/fixtures/mini` | `pwsh scripts/run-native.ps1` |
| `<path>` | Any fixture path | `pwsh scripts/run-native.ps1 <path>` |

`mini` is the only bundled fixture today. If the user names a preset that isn't
`empty`/`mini` and isn't an existing path, tell them and list what's available
(`crates/waml-editor/tests/fixtures/`) rather than guessing.

## How to run

1. Pick the command from the table based on the user's argument (no arg → `mini`).
2. Run it with `run_in_background: true` — it's a GUI window that blocks while
   open. The script already frees the exe lock and prebuilds; don't add your own
   kill/build steps.
3. Watch the background output file until you see
   `Running \`target\debug\waml-editor.exe ...\`` — that means the window opened.
   If the build fails, quote the shortest decisive error line.

## Notes

- Run from a checkout where `scripts/run-native.ps1` exists (main or a worktree).
- To screenshot the running window: `pwsh scripts/capture-window.ps1 -Out x.png -Process waml-editor`.
