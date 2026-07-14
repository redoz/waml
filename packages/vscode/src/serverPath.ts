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
  const exeName = ctx.platform === "win32" ? "waml.exe" : "waml";

  // 1. WAML_SERVER_PATH env var — the F5 dev override. Env wins.
  const envPath = ctx.env.WAML_SERVER_PATH?.trim();
  if (envPath) {
    const runnable = ctx.fileExists(envPath);
    return {
      command: envPath,
      source: "env",
      runnable,
      reason: runnable
        ? undefined
        : `WAML_SERVER_PATH points at "${envPath}" but no file exists there. Run \`cargo build\` or fix the path, then reload the window.`,
    };
  }

  // 2. Explicit waml.serverPath config (ignore the "waml" default value).
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
        : `waml.serverPath is set to "${explicit}" but no file exists there. Fix the setting, then reload the window.`,
    };
  }

  // 3. Bundled binary at <extensionPath>/server/waml[.exe] (dead until step 2).
  const bundled = join(ctx.extensionPath, "server", exeName);
  if (ctx.fileExists(bundled)) {
    return { command: bundled, source: "bundled", runnable: true };
  }

  // 4. Bare "waml" on PATH — final fallback.
  const runnable = ctx.probeCommand("waml");
  return {
    command: "waml",
    source: "path",
    runnable,
    reason: runnable
      ? undefined
      : 'Could not find the "waml" binary on your PATH. Set waml.serverPath to its full path, install waml, or run `cargo build` and launch via the provided F5 config.',
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
