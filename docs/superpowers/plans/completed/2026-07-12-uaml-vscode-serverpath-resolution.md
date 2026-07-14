# UAML VS Code Server Path Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve the `uaml` language-server binary once, from a clear precedence chain, with an actionable error instead of an EPIPE crash loop, and make F5 work for any fresh clone that ran `cargo build`.

**Architecture:** One new pure module `packages/vscode/src/serverPath.ts` owns all resolution/precedence logic as `resolveServerPath(ctx)` over injected inputs (env, VS Code config inspection, filesystem probes), so it is unit-testable without a VS Code host. `activate()` in `extension.ts` builds the real inputs, calls the resolver exactly once, and either shows an actionable error (client never starts) or caches the command and starts the language client. F5 scaffolding (`launch.json` env override, de-pinned `lsp-demo` settings) is committed so a fresh clone can debug.

**Tech Stack:** TypeScript (CommonJS, `tsc` build), `vscode-languageclient`, Node builtins (`node:path`, `node:fs`, `node:child_process`), Vitest (the repo's existing test runner, used by `@uaml/core` and `@uaml/okf`).

## Global Constraints

- Resolution/precedence logic lives in **exactly one file**: `packages/vscode/src/serverPath.ts`. No resolution logic anywhere else.
- Resolution order, first hit wins, **env wins over config**: (1) `UAML_SERVER_PATH` env, (2) explicit `uaml.serverPath` config (distinguished from the `"uaml"` default via `.inspect()`), (3) bundled `<extensionPath>/server/uaml[.exe]` via `existsSync`, (4) bare `"uaml"` on `PATH`.
- Preflight: concrete paths (candidates 1–3) use `existsSync`; the bare-`"uaml"` case (candidate 4) uses `spawnSync("uaml", ["--version"])` and treats `ENOENT` as not-runnable.
- If not runnable: `window.showErrorMessage` with the reason + actionable steps + an "Open UAML Settings" button, and **do not start the client** (no spawn).
- `activate()` calls `resolveServerPath` **exactly once**; the resolved command is the single in-memory source of truth, exposed via `getServerCommand()`. No re-resolution, no `onDidChangeConfiguration` watcher (YAGNI — documented; window reload required after changing the setting).
- Client starts with `command: <resolved>`, `args: ["lsp"]`, `transport: TransportKind.stdio`.
- Green gate for this package: `pnpm -C packages/vscode build` (tsc) **and** `pnpm -C packages/vscode test` (Vitest). Rust is unaffected.
- Out of scope (spec "Non-goals" / future work — do NOT implement): `vsce package`/`.vsix`, actually placing a bundled binary under `server/`, GitHub release artifacts, marketplace publication. The bundled-binary candidate is wired as a cheap `existsSync` only.
- Commit messages: Conventional Commits, scope `vscode`. **Do not** add any `Co-Authored-By: Claude` trailer.

---

## File Structure

| File | Responsibility | Change |
| --- | --- | --- |
| `packages/vscode/src/serverPath.ts` | Pure resolver + precedence + preflight + cached-command accessors. The only home of resolution logic. | Create |
| `packages/vscode/src/serverPath.test.ts` | Vitest unit tests over synthetic env/config/fs inputs. | Create |
| `packages/vscode/src/extension.ts` | `activate()` builds real inputs, calls resolver once, shows error or starts client. | Modify (rewrite `activate`) |
| `packages/vscode/package.json` | `test` script → `vitest run`; add `vitest` devDependency. | Modify |
| `packages/vscode/tsconfig.json` | Exclude `*.test.ts` from the `tsc` build (Vitest runs them independently). | Modify |
| `packages/vscode/.vscode/launch.json` | Add `env` block so F5 sets `UAML_SERVER_PATH` per-clone. Currently **untracked**. | Modify + commit |
| `lsp-demo/.vscode/settings.json` | Drop the absolute `serverPath` pin so the file is clone-portable. Currently **untracked**. | Modify + commit |
| `lsp-demo/order.md` | Existing demo doc (intentional broken line for live diagnostics). Currently **untracked**. | Commit as-is |

**Repo reality confirmed before writing this plan:**
- `packages/vscode/src/extension.ts` is a 28-line single `activate()` that today reads `workspace.getConfiguration("uaml").get<string>("serverPath", "uaml")` and starts the client unconditionally.
- `package.json` declares config `uaml.serverPath` default `"uaml"`; `"build": "tsc -p tsconfig.json"`, `"test": "echo \"no tests\" && exit 0"`.
- `tsconfig.json` extends `../../tsconfig.base.json`, `module: CommonJS`, `moduleResolution: Node`, `include: ["src"]`.
- Vitest `2.1.9` resolves from `packages/vscode` (hoisted from root devDependencies); `@uaml/core` uses `"test": "vitest run --passWithNoTests"`.
- `@types/node` resolves from `packages/vscode` (transitive via `vscode-languageclient/node`); a throwaway `tsc` probe using `node:path` + `NodeJS.Platform` compiled cleanly.
- `target/debug/uaml.exe` exists (built via `cargo build`). `.gitignore` ignores `dist/`, `/target`, `node_modules/`.
- `packages/vscode/.vscode/` and `lsp-demo/` are both **untracked** today (`git status`: `?? lsp-demo/`, `?? packages/vscode/.vscode/`). `lsp-demo/.vscode/settings.json` currently pins an absolute worktree path.

---

## Task 1: Pure server-path resolver + unit tests

**Files:**
- Create: `packages/vscode/src/serverPath.ts`
- Test: `packages/vscode/src/serverPath.test.ts`
- Modify: `packages/vscode/package.json` (test script + `vitest` devDependency)
- Modify: `packages/vscode/tsconfig.json` (exclude test files from `tsc`)

**Interfaces:**
- Consumes: nothing (first task).
- Produces (relied on by Task 2):
  - `type ServerPathSource = "env" | "config" | "bundled" | "path";`
  - `interface ServerPathResolution { command: string; source: ServerPathSource; runnable: boolean; reason?: string; }`
  - `interface ConfigInspection { defaultValue?: string; globalValue?: string; workspaceValue?: string; workspaceFolderValue?: string; }`
  - `interface ServerPathContext { env: Record<string, string | undefined>; extensionPath: string; platform: NodeJS.Platform; configInspection: ConfigInspection | undefined; fileExists: (path: string) => boolean; probeCommand: (command: string) => boolean; }`
  - `function resolveServerPath(ctx: ServerPathContext): ServerPathResolution;`
  - `function setServerCommand(command: string): void;`
  - `function getServerCommand(): string;`

- [ ] **Step 1: Wire the test harness (package.json + tsconfig)**

In `packages/vscode/package.json`, change the `test` script and add the `vitest` devDependency (mirrors `@uaml/core`):

```json
{
  "name": "@uaml/vscode",
  "private": true,
  "version": "0.0.0",
  "license": "Apache-2.0",
  "type": "commonjs",
  "displayName": "UAML",
  "description": "Live UAML diagnostics for Markdown documents.",
  "engines": { "vscode": "^1.90.0" },
  "categories": ["Programming Languages", "Linters"],
  "activationEvents": ["onLanguage:markdown"],
  "main": "./dist/extension.js",
  "contributes": {
    "configuration": {
      "title": "UAML",
      "properties": {
        "uaml.serverPath": {
          "type": "string",
          "default": "uaml",
          "description": "Path to the uaml executable that provides the language server."
        }
      }
    }
  },
  "scripts": {
    "build": "tsc -p tsconfig.json",
    "test": "vitest run"
  },
  "dependencies": { "vscode-languageclient": "^9.0.1" },
  "devDependencies": { "@types/vscode": "^1.90.0", "typescript": "^5.6.0", "vitest": "^2.1.0" }
}
```

In `packages/vscode/tsconfig.json`, exclude test files so `tsc` (the shipped build) does not type-check Vitest test files under `moduleResolution: Node`:

```json
{
  "extends": "../../tsconfig.base.json",
  "compilerOptions": {
    "module": "CommonJS",
    "moduleResolution": "Node",
    "outDir": "dist",
    "rootDir": "src",
    "lib": ["ES2022"]
  },
  "include": ["src"],
  "exclude": ["src/**/*.test.ts"]
}
```

Then reconcile the lockfile (Vitest is already hoisted from the root, so this only records the declared devDependency — no network fetch expected):

Run: `pnpm install`
Expected: completes; `@uaml/vscode` now declares `vitest`. (`vitest` binary already resolves — confirmed `pnpm -C packages/vscode exec vitest --version` → `vitest/2.1.9`.)

- [ ] **Step 2: Write the failing test**

Create `packages/vscode/src/serverPath.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import {
  resolveServerPath,
  getServerCommand,
  setServerCommand,
  type ServerPathContext,
  type ConfigInspection,
} from "./serverPath";

function makeCtx(overrides: Partial<ServerPathContext> = {}): ServerPathContext {
  return {
    env: {},
    extensionPath: "/ext",
    platform: "linux",
    configInspection: undefined,
    fileExists: () => false,
    probeCommand: () => false,
    ...overrides,
  };
}

describe("resolveServerPath", () => {
  it("uses UAML_SERVER_PATH when set (runnable when the file exists)", () => {
    const r = resolveServerPath(
      makeCtx({
        env: { UAML_SERVER_PATH: "/tmp/uaml" },
        fileExists: (p) => p === "/tmp/uaml",
      }),
    );
    expect(r.source).toBe("env");
    expect(r.command).toBe("/tmp/uaml");
    expect(r.runnable).toBe(true);
    expect(r.reason).toBeUndefined();
  });

  it("uses an explicit uaml.serverPath config value", () => {
    const inspection: ConfigInspection = { defaultValue: "uaml", globalValue: "/opt/uaml" };
    const r = resolveServerPath(
      makeCtx({
        configInspection: inspection,
        fileExists: (p) => p === "/opt/uaml",
      }),
    );
    expect(r.source).toBe("config");
    expect(r.command).toBe("/opt/uaml");
    expect(r.runnable).toBe(true);
  });

  it("ignores the default config value and falls through to the bundled binary", () => {
    const r = resolveServerPath(
      makeCtx({
        configInspection: { defaultValue: "uaml" },
        platform: "win32",
        fileExists: (p) => p.includes("server"),
      }),
    );
    expect(r.source).toBe("bundled");
    expect(r.command).toContain("uaml.exe");
    expect(r.runnable).toBe(true);
  });

  it("returns not-runnable with a reason when nothing is found", () => {
    const r = resolveServerPath(makeCtx());
    expect(r.source).toBe("path");
    expect(r.command).toBe("uaml");
    expect(r.runnable).toBe(false);
    expect(r.reason).toBeTruthy();
  });

  it("lets env win over an explicit config value", () => {
    const r = resolveServerPath(
      makeCtx({
        env: { UAML_SERVER_PATH: "/env/uaml" },
        configInspection: { defaultValue: "uaml", globalValue: "/config/uaml" },
        fileExists: () => true,
      }),
    );
    expect(r.source).toBe("env");
    expect(r.command).toBe("/env/uaml");
  });
});

describe("getServerCommand / setServerCommand", () => {
  it("round-trips the cached command", () => {
    setServerCommand("/cached/uaml");
    expect(getServerCommand()).toBe("/cached/uaml");
  });
});
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `pnpm -C packages/vscode test`
Expected: FAIL — Vitest cannot resolve `./serverPath` (e.g. `Failed to resolve import "./serverPath"` / "Cannot find module").

- [ ] **Step 4: Write the minimal implementation**

Create `packages/vscode/src/serverPath.ts`:

```ts
import { join } from "node:path";

export type ServerPathSource = "env" | "config" | "bundled" | "path";

export interface ServerPathResolution {
  command: string;
  source: ServerPathSource;
  runnable: boolean;
  reason?: string;
}

/** Subset of vscode WorkspaceConfiguration.inspect<string>() that we consume. */
export interface ConfigInspection {
  defaultValue?: string;
  globalValue?: string;
  workspaceValue?: string;
  workspaceFolderValue?: string;
}

/**
 * Everything resolveServerPath needs, injected so the function stays pure over
 * (env, config, filesystem) and is testable without the `vscode` runtime.
 */
export interface ServerPathContext {
  env: Record<string, string | undefined>;
  extensionPath: string;
  platform: NodeJS.Platform;
  configInspection: ConfigInspection | undefined;
  fileExists: (path: string) => boolean;
  /** Probe a bare command on PATH: return false when it is missing (ENOENT). */
  probeCommand: (command: string) => boolean;
}

export function resolveServerPath(ctx: ServerPathContext): ServerPathResolution {
  const exeName = ctx.platform === "win32" ? "uaml.exe" : "uaml";

  // 1. UAML_SERVER_PATH env var — the F5 dev override. Env wins.
  const envPath = ctx.env.UAML_SERVER_PATH?.trim();
  if (envPath) {
    const runnable = ctx.fileExists(envPath);
    return {
      command: envPath,
      source: "env",
      runnable,
      reason: runnable
        ? undefined
        : `UAML_SERVER_PATH points at "${envPath}" but no file exists there. Run \`cargo build\` or fix the path, then reload the window.`,
    };
  }

  // 2. Explicit uaml.serverPath config (ignore the "uaml" default value).
  const insp = ctx.configInspection;
  const explicit = insp?.workspaceFolderValue ?? insp?.workspaceValue ?? insp?.globalValue;
  if (explicit !== undefined && explicit.trim() !== "") {
    const runnable = ctx.fileExists(explicit);
    return {
      command: explicit,
      source: "config",
      runnable,
      reason: runnable
        ? undefined
        : `uaml.serverPath is set to "${explicit}" but no file exists there. Fix the setting, then reload the window.`,
    };
  }

  // 3. Bundled binary at <extensionPath>/server/uaml[.exe] (dead until step 2).
  const bundled = join(ctx.extensionPath, "server", exeName);
  if (ctx.fileExists(bundled)) {
    return { command: bundled, source: "bundled", runnable: true };
  }

  // 4. Bare "uaml" on PATH — final fallback.
  const runnable = ctx.probeCommand("uaml");
  return {
    command: "uaml",
    source: "path",
    runnable,
    reason: runnable
      ? undefined
      : 'Could not find the "uaml" binary on your PATH. Set uaml.serverPath to its full path, install uaml, or run `cargo build` and launch via the provided F5 config.',
  };
}

let cachedCommand: string | undefined;

/** Cache the resolved command as the single in-memory source of truth. */
export function setServerCommand(command: string): void {
  cachedCommand = command;
}

/** Read the command resolved at activation. Throws if activation never set it. */
export function getServerCommand(): string {
  if (cachedCommand === undefined) {
    throw new Error("Server command not resolved; call resolveServerPath in activate() first.");
  }
  return cachedCommand;
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `pnpm -C packages/vscode test`
Expected: PASS — 6 tests pass (5 resolver cases + get/set round-trip).

- [ ] **Step 6: Run the build to verify tsc stays green**

Run: `pnpm -C packages/vscode build`
Expected: exit 0, no output errors. (`serverPath.ts` compiles; `serverPath.test.ts` is excluded from the build.)

- [ ] **Step 7: Commit**

```bash
git add packages/vscode/src/serverPath.ts packages/vscode/src/serverPath.test.ts packages/vscode/package.json packages/vscode/tsconfig.json pnpm-lock.yaml
git commit -m "feat(vscode): add pure resolveServerPath with precedence + preflight"
```

---

## Task 2: Wire activate() to the resolver

**Files:**
- Modify: `packages/vscode/src/extension.ts` (rewrite `activate`)

**Interfaces:**
- Consumes (from Task 1): `resolveServerPath(ctx: ServerPathContext)`, `setServerCommand(command)`, `getServerCommand()`, and the `ServerPathContext` shape.
- Produces: no new exported symbols; wires the resolver into extension activation.

- [ ] **Step 1: Rewrite `extension.ts`**

Replace the entire contents of `packages/vscode/src/extension.ts` with:

```ts
import { workspace, window, commands, ExtensionContext } from "vscode";
import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import {
  resolveServerPath,
  setServerCommand,
  getServerCommand,
  type ServerPathContext,
} from "./serverPath";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext): void {
  const ctx: ServerPathContext = {
    env: process.env,
    extensionPath: context.extensionPath,
    platform: process.platform,
    configInspection: workspace.getConfiguration("uaml").inspect<string>("serverPath"),
    fileExists: (p) => existsSync(p),
    probeCommand: (command) => {
      const result = spawnSync(command, ["--version"], { stdio: "ignore" });
      const code = (result.error as NodeJS.ErrnoException | undefined)?.code;
      return code !== "ENOENT";
    },
  };

  const resolution = resolveServerPath(ctx);
  if (!resolution.runnable) {
    void window
      .showErrorMessage(
        `UAML language server not started. ${resolution.reason ?? ""} ` +
          "After fixing this, reload the window (Developer: Reload Window).",
        "Open UAML Settings",
      )
      .then((choice) => {
        if (choice === "Open UAML Settings") {
          void commands.executeCommand("workbench.action.openSettings", "uaml.serverPath");
        }
      });
    return;
  }

  setServerCommand(resolution.command);
  const serverOptions: ServerOptions = {
    command: getServerCommand(),
    // Only the subcommand — TransportKind.stdio makes the client append `--stdio`.
    args: ["lsp"],
    transport: TransportKind.stdio,
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ language: "markdown" }],
  };
  client = new LanguageClient("uaml", "UAML", serverOptions, clientOptions);
  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
```

- [ ] **Step 2: Run the build to verify it compiles**

Run: `pnpm -C packages/vscode build`
Expected: exit 0, no errors. (`process`, `node:fs`, `node:child_process` type-check via the hoisted `@types/node`; `inspect<string>()` returns a structurally-compatible object.)

- [ ] **Step 3: Run the tests to confirm nothing regressed**

Run: `pnpm -C packages/vscode test`
Expected: PASS — same 6 tests still pass (resolver logic unchanged; `extension.ts` is not imported by the tests).

- [ ] **Step 4 (manual smoke, optional — not a gate): F5 sanity**

Not automatable from CI. If verifying interactively: with `target/debug/uaml.exe` present, press F5 in `packages/vscode`; live diagnostics appear on `lsp-demo/order.md`. Rename `target/debug/uaml.exe` temporarily and confirm the actionable error dialog (with "Open UAML Settings") appears instead of an EPIPE crash. Restore the binary afterward.

- [ ] **Step 5: Commit**

```bash
git add packages/vscode/src/extension.ts
git commit -m "feat(vscode): resolve server path once and fail loudly instead of EPIPE"
```

---

## Task 3: Commit the F5 scaffolding (portable launch config)

**Files:**
- Modify: `packages/vscode/.vscode/launch.json` (add `env` block; currently untracked)
- Modify: `lsp-demo/.vscode/settings.json` (remove absolute `serverPath` pin; currently untracked)
- Commit: `lsp-demo/order.md` (existing demo doc, untracked)

**Interfaces:**
- Consumes: the `UAML_SERVER_PATH` env override honored by Task 1's resolver (candidate 1).
- Produces: no code symbols — committed scaffolding so a fresh clone + `cargo build` can F5.

- [ ] **Step 1: Add the `env` block to `launch.json`**

Replace the contents of `packages/vscode/.vscode/launch.json` with:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "name": "Run UAML Extension",
      "type": "extensionHost",
      "request": "launch",
      "args": [
        "${workspaceFolder}/../../lsp-demo",
        "--extensionDevelopmentPath=${workspaceFolder}"
      ],
      "outFiles": ["${workspaceFolder}/dist/**/*.js"],
      "env": {
        "UAML_SERVER_PATH": "${workspaceFolder}/../../target/debug/uaml.exe"
      }
    }
  ]
}
```

`${workspaceFolder}` is `packages/vscode`, so this resolves to `<repo>/target/debug/uaml.exe` for any clone — no hardcoded absolute path.

- [ ] **Step 2: Drop the absolute `serverPath` pin from the demo settings**

Replace the contents of `lsp-demo/.vscode/settings.json` with an empty object (the env override now supplies the path; the file no longer pins a machine-specific path and becomes clone-portable):

```json
{}
```

- [ ] **Step 3: Verify the pin is gone and the env override is present**

Run: `git diff --no-index /dev/null lsp-demo/.vscode/settings.json | grep -c "C:/dev/uaml" || true`
Expected: prints `0` (no absolute worktree path remains).

Run: `grep -c "UAML_SERVER_PATH" packages/vscode/.vscode/launch.json`
Expected: prints `1`.

- [ ] **Step 4: Commit the scaffolding (these paths are new/untracked)**

```bash
git add packages/vscode/.vscode/launch.json lsp-demo/.vscode/settings.json lsp-demo/order.md
git commit -m "chore(vscode): commit portable F5 scaffolding via UAML_SERVER_PATH"
```

---

## Verification / Green gate (whole plan)

From the repo root, both must pass:

```bash
pnpm -C packages/vscode build   # tsc → exit 0
pnpm -C packages/vscode test    # vitest run → 6 passing
```

`pnpm -r test` (the workspace-wide gate) now also runs the vscode Vitest suite because the `test` script changed from `echo "no tests"` to `vitest run`. Rust is untouched.

## Notes / deviations from the spec (for the reviewer)

- **`resolveServerPath` signature.** The spec writes `resolveServerPath(ctx: ExtensionContext)`. Implemented as `resolveServerPath(ctx: ServerPathContext)` — a small injected interface (env, config inspection, `fileExists`, `probeCommand`, `extensionPath`, `platform`). This is required to satisfy the spec's own stated goal that the function be "pure over its inputs … testable without a full VS Code test harness": importing the `vscode` runtime module into `serverPath.ts` would make Vitest fail to load it. `activate()` maps the real `ExtensionContext` + Node/VS Code APIs into `ServerPathContext`. All precedence/preflight logic still lives solely in `serverPath.ts`.
- **`lsp-demo/.vscode/settings.json` becomes `{}`** rather than being deleted — the spec frames it as "delete the pin … the file becomes committable," implying the file stays.
- **`launch.json` env path is Windows-specific** (`uaml.exe`), taken verbatim from the spec. Adequate for the current (Windows) dev setup; a cross-platform path is out of scope here.
- **Test tooling = Vitest**, the repo's existing runner (`@uaml/core`, `@uaml/okf` both use `vitest run`), not Node's `node:test`. Vitest already resolves from `packages/vscode`.
