# UAML VS Code Extension — Server Path Resolution

**Date:** 2026-07-12
**Status:** Design approved

## Problem

The UAML VS Code extension spawns the `uaml` binary to run the language
server. Today the only way to point it at that binary is the `uaml.serverPath`
setting, whose default is the bare name `"uaml"` (expects it on `PATH`).

For local development via F5, the demo scaffolding pinned `uaml.serverPath` to
an **absolute worktree path** in `lsp-demo/.vscode/settings.json`. That path is
correct on exactly one machine/worktree and breaks for anyone who clones the
repo. It also cannot be made relative: `settings.json` does not support
`${workspaceFolder}` substitution.

When resolution fails, the failure is opaque: the client spawns whatever
`serverPath` says, the process is missing, and VS Code surfaces a cryptic
`write EPIPE` / "connection disposed" during initialize with no hint about the
cause. (This exact failure cost roughly an hour to diagnose.)

## Goals

- Remove the hardcoded absolute path so F5 works for anyone who cloned the repo
  and ran `cargo build`.
- Resolve the server binary **once**, into a single in-memory value that every
  UAML operation reads. No resolution/precedence logic sprinkled across the
  codebase.
- On failure, show a clear, actionable message instead of an EPIPE crash loop.

## Non-goals (documented as future work, NOT planned here)

- **Step 2 — `vsce package`:** produce an installable `.vsix` and populate a
  bundled binary at `<extensionPath>/server/`.
- **Step 3 — GitHub release artifact:** publish the `uaml` binary as a build
  artifact so a packaged extension can download or ship it per platform.
- **No marketplace** publication for now.

The bundled-binary resolution candidate (below) is wired in this step as a
cheap `existsSync` check so step 2 becomes "just drop the file in place." No
bundling itself happens now.

## Resolution model

`uaml.serverPath` (the VS Code setting) is the **source of truth** — the one
documented, user-facing knob. `UAML_SERVER_PATH` (env var) is **not** a second
competing setting; it is a dev override layer that exists solely because
`launch.json`'s `env` block *can* substitute `${workspaceFolder}` while
`settings.json`'s `serverPath` cannot. Env is the only way to feed a
per-clone-relative path into a committed dev file.

Effective command, first hit wins (**env wins** over config):

1. **`UAML_SERVER_PATH` env var** — if set, used verbatim. The F5 debug hammer.
2. **`uaml.serverPath` config** — if the user set it explicitly (detected via
   `.inspect()`, distinguishing an explicit value from the `"uaml"` default).
3. **Bundled binary** at `<extensionPath>/server/uaml` (`uaml.exe` on Windows)
   — used if `existsSync`. Dead until step 2 ships a binary there.
4. **`"uaml"`** — bare name, resolved off `PATH`. Final fallback.

### Preflight

Before starting the client, verify the resolved command is actually runnable:

- Candidates 1–3 return a concrete path → `existsSync` check.
- Candidate 4 (bare `"uaml"`) → `spawnSync("uaml", ["--version"])`, treat
  `ENOENT` as not-runnable.

If nothing is runnable, **do not start the client** (no spawn, no EPIPE, no
crash loop). Instead show `window.showErrorMessage` with:

- the reason (which candidates were tried and why each failed), and
- actionable next steps: set `uaml.serverPath`, install the `uaml` binary, or
  run `cargo build`, plus a button that opens the UAML settings.

## Architecture

One new module, `packages/vscode/src/serverPath.ts`, is the **only** place
resolution and precedence live:

```ts
interface ServerPathResolution {
  command: string;                               // resolved path or "uaml"
  source: "env" | "config" | "bundled" | "path";
  runnable: boolean;                             // preflight result
  reason?: string;                               // why not, for the notification
}

function resolveServerPath(ctx: ExtensionContext): ServerPathResolution;
```

`resolveServerPath` is pure over its inputs (env, VS Code config, filesystem),
which makes it testable without a full VS Code test harness.

`activate()` calls it **exactly once**:

1. `const r = resolveServerPath(context)`
2. `if (!r.runnable)` → `showErrorMessage(r.reason + actionable)` and **return**
   (client never starts).
3. else cache `r.command` as the single in-memory source of truth (module
   variable exposed via `getServerCommand()`) and start the language client
   with `command: r.command`, `args: ["lsp"]`, `transport: stdio`.

Every future UAML operation that needs the binary imports `getServerCommand()`.
It never re-resolves. Resolution exists in one file and runs one time per
activation.

### Runtime config changes

Because resolution runs once at activation, editing `uaml.serverPath` requires
a window reload to take effect — standard behavior for LSP extensions. No live
`onDidChangeConfiguration` watcher in this step (YAGNI; addable later). The
not-runnable message mentions that a reload is needed after fixing the setting.

## Scaffolding changes (delivers the actual goal)

- `packages/vscode/.vscode/launch.json` — add an `env` block:
  ```json
  "env": { "UAML_SERVER_PATH": "${workspaceFolder}/../../target/debug/uaml.exe" }
  ```
  `${workspaceFolder}` is `packages/vscode`, so this resolves to
  `<repo>/target/debug/uaml.exe` for any clone. No hardcoding.
- `lsp-demo/.vscode/settings.json` — **delete** the absolute `serverPath` pin.
  With env providing the path, the file no longer needs it and becomes
  committable.
- Commit the F5 scaffolding (launch.json, lsp-demo/) so anyone who clones the
  repo and runs `cargo build` can F5 and see live diagnostics.

## Testing

The extension currently has no test harness (`"test": "echo no tests"`).

- Extract `resolveServerPath` as a pure function so it can be exercised with
  plain Node — feed synthetic env / config / fs inputs, assert the chosen
  `command`, `source`, and `runnable`/`reason`.
- Cases: env set; config explicitly set; bundled present; nothing found
  (not-runnable + reason); env-wins-over-config precedence.
- No full VS Code integration harness in this step — the `activate()` wiring is
  thin and the logic under test is the pure resolver.

## Risks

- **Relying on env for dev:** if a developer launches the extension outside the
  provided F5 config and hasn't set `UAML_SERVER_PATH` or `uaml.serverPath`,
  resolution falls to `"uaml"` on `PATH`. If absent, the actionable error fires
  — acceptable, and strictly better than the current silent EPIPE.
- **Bundled path is dead code until step 2.** Kept minimal (one `existsSync`)
  to avoid speculative machinery.
