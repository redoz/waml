import { afterEach, describe, it, expect, vi } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import ts from "typescript";
import type { ExtensionContext } from "vscode";

const vscodeHarness = vi.hoisted(() => ({
  inspection: undefined as ConfigInspection | undefined,
  errors: [] as string[],
  settingsOpened: 0,
}));

const clientHarness = vi.hoisted(() => ({
  instances: [] as Array<{
    id: string;
    name: string;
    serverOptions: unknown;
    clientOptions: unknown;
    starts: number;
    stops: number;
    startError?: Error;
  }>,
  nextStartError: undefined as Error | undefined,
}));

vi.mock("vscode", () => ({
  workspace: {
    getConfiguration: () => ({
      inspect: () => vscodeHarness.inspection,
    }),
  },
  window: {
    showErrorMessage: async (message: string) => {
      vscodeHarness.errors.push(message);
      return undefined;
    },
  },
  commands: {
    executeCommand: async () => {
      vscodeHarness.settingsOpened += 1;
    },
  },
}));

vi.mock("vscode-languageclient/node", () => ({
  TransportKind: { stdio: 0 },
  LanguageClient: class {
    private readonly state;

    constructor(id: string, name: string, serverOptions: unknown, clientOptions: unknown) {
      this.state = {
        id,
        name,
        serverOptions,
        clientOptions,
        starts: 0,
        stops: 0,
        startError: clientHarness.nextStartError,
      };
      clientHarness.nextStartError = undefined;
      clientHarness.instances.push(this.state);
    }

    async start(): Promise<void> {
      this.state.starts += 1;
      if (this.state.startError) throw this.state.startError;
    }

    async stop(): Promise<void> {
      this.state.stops += 1;
    }
  },
}));
import {
  resolveServerPath,
  getServerCommand,
  setServerCommand,
  type ServerPathContext,
  type ConfigInspection,
} from "./serverPath";
import { activate, deactivate } from "./extension";

const extensionSourcePath = join(__dirname, "extension.ts");
const extensionSource = readFileSync(extensionSourcePath, "utf8");
const extensionFile = ts.createSourceFile(
  extensionSourcePath,
  extensionSource,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TS,
);

function importedModules(): string[] {
  return extensionFile.statements
    .filter(ts.isImportDeclaration)
    .map((declaration) => declaration.moduleSpecifier)
    .filter(ts.isStringLiteral)
    .map((specifier) => specifier.text);
}

function extensionContext(): ExtensionContext {
  return { extensionPath: "/extension" } as ExtensionContext;
}

afterEach(async () => {
  await deactivate();
  delete process.env.WAML_SERVER_PATH;
  vscodeHarness.inspection = undefined;
  vscodeHarness.errors.length = 0;
  vscodeHarness.settingsOpened = 0;
  clientHarness.instances.length = 0;
  clientHarness.nextStartError = undefined;
});

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
  it("uses WAML_SERVER_PATH when set (runnable when the file exists)", () => {
    const r = resolveServerPath(
      makeCtx({
        env: { WAML_SERVER_PATH: "/tmp/waml" },
        fileExists: (p) => p === "/tmp/waml",
      }),
    );
    expect(r.source).toBe("env");
    expect(r.command).toBe("/tmp/waml");
    expect(r.runnable).toBe(true);
    expect(r.reason).toBeUndefined();
  });

  it("uses an explicit waml.serverPath config value", () => {
    const inspection: ConfigInspection = { defaultValue: "waml", globalValue: "/opt/waml" };
    const r = resolveServerPath(
      makeCtx({
        configInspection: inspection,
        fileExists: (p) => p === "/opt/waml",
      }),
    );
    expect(r.source).toBe("config");
    expect(r.command).toBe("/opt/waml");
    expect(r.runnable).toBe(true);
  });

  it("ignores the default config value and falls through to the bundled binary", () => {
    const r = resolveServerPath(
      makeCtx({
        configInspection: { defaultValue: "waml" },
        platform: "win32",
        fileExists: (p) => p.includes("server"),
      }),
    );
    expect(r.source).toBe("bundled");
    expect(r.command).toContain("waml.exe");
    expect(r.runnable).toBe(true);
  });

  it("returns not-runnable with a reason when nothing is found", () => {
    const r = resolveServerPath(makeCtx());
    expect(r.source).toBe("path");
    expect(r.command).toBe("waml");
    expect(r.runnable).toBe(false);
    expect(r.reason).toBeTruthy();
  });

  it("lets env win over an explicit config value", () => {
    const r = resolveServerPath(
      makeCtx({
        env: { WAML_SERVER_PATH: "/env/waml" },
        configInspection: { defaultValue: "waml", globalValue: "/config/waml" },
        fileExists: () => true,
      }),
    );
    expect(r.source).toBe("env");
    expect(r.command).toBe("/env/waml");
  });
});

describe("getServerCommand / setServerCommand", () => {
  it("round-trips the cached command", () => {
    setServerCommand("/cached/waml");
    expect(getServerCommand()).toBe("/cached/waml");
  });
});

describe("VS Code stdio transport isolation", () => {
  it("does not import parser, syntax, WASM, or retired TypeScript domains", () => {
    expect(importedModules()).not.toContain("@waml/parser");
    for (const moduleName of importedModules()) {
      expect(moduleName).not.toMatch(/(^@waml\/|(?:^|[/_-])(?:wasm|parser|syntax)(?:$|[/_.-]))/i);
    }
  });
});

describe("extension lifecycle", () => {
  it("starts the configured executable once with stdio, restart policy, and markdown initialization", async () => {
    process.env.WAML_SERVER_PATH = __filename;

    await activate(extensionContext());

    expect(clientHarness.instances).toHaveLength(1);
    expect(clientHarness.instances[0]).toMatchObject({
      id: "waml",
      name: "WAML",
      serverOptions: {
        command: __filename,
        args: ["lsp"],
        transport: 0,
      },
      clientOptions: {
        documentSelector: [{ language: "markdown" }],
        initializationOptions: undefined,
        connectionOptions: { maxRestartCount: 4 },
      },
      starts: 1,
      stops: 0,
    });
  });

  it("pins the installed client's bounded default error and crash-restart behavior", () => {
    const dependencyPath = createRequire(__filename).resolve(
      "vscode-languageclient/lib/common/client.js",
    );
    const dependencySource = readFileSync(dependencyPath, "utf8");
    const dependencyFile = ts.createSourceFile(
      dependencyPath,
      dependencySource,
      ts.ScriptTarget.Latest,
      true,
      ts.ScriptKind.JS,
    );
    const declaration = dependencyFile.statements.find(
      (statement): statement is ts.ClassDeclaration =>
        ts.isClassDeclaration(statement) && statement.name?.text === "DefaultErrorHandler",
    );
    expect(declaration).toBeDefined();
    if (!declaration) return;

    const ErrorAction = { Continue: 1, Shutdown: 2 };
    const CloseAction = { DoNotRestart: 1, Restart: 2 };
    const Handler = Function(
      "ErrorAction",
      "CloseAction",
      `${declaration.getText(dependencyFile)}; return DefaultErrorHandler;`,
    )(ErrorAction, CloseAction) as new (
      client: { name: string },
      maxRestartCount: number,
    ) => {
      error(error: Error, message: unknown, count: number): { action: number };
      closed(): { action: number; message?: string };
    };
    const handler = new Handler({ name: "WAML" }, 4);

    expect(handler.error(new Error("bad frame"), undefined, 3).action).toBe(ErrorAction.Continue);
    expect(handler.error(new Error("bad frame"), undefined, 4).action).toBe(ErrorAction.Shutdown);
    for (let crash = 0; crash < 4; crash += 1) {
      expect(handler.closed().action).toBe(CloseAction.Restart);
    }
    const exhausted = handler.closed();
    expect(exhausted.action).toBe(CloseAction.DoNotRestart);
    expect(exhausted.message).toContain("crashed 5 times");
  });

  it("reports an unresolved executable without constructing a client", async () => {
    process.env.WAML_SERVER_PATH = join(__dirname, "missing-waml");

    await activate(extensionContext());

    expect(clientHarness.instances).toHaveLength(0);
    expect(vscodeHarness.errors).toHaveLength(1);
    expect(vscodeHarness.errors[0]).toContain("WAML language server not started");
  });

  it("stops the previous client before a repeated activation starts another", async () => {
    process.env.WAML_SERVER_PATH = __filename;
    await activate(extensionContext());
    const first = clientHarness.instances[0];

    await activate(extensionContext());

    expect(first.stops).toBe(1);
    expect(clientHarness.instances).toHaveLength(2);
    expect(clientHarness.instances[1].starts).toBe(1);
  });

  it("cleans up and reports a launch failure", async () => {
    process.env.WAML_SERVER_PATH = __filename;
    clientHarness.nextStartError = new Error("spawn failed");

    await activate(extensionContext());

    expect(clientHarness.instances[0]).toMatchObject({ starts: 1, stops: 1 });
    expect(vscodeHarness.errors).toHaveLength(1);
    expect(vscodeHarness.errors[0]).toContain("spawn failed");
  });

  it("deactivate stops the active client once and clears it", async () => {
    process.env.WAML_SERVER_PATH = __filename;
    await activate(extensionContext());
    const active = clientHarness.instances[0];

    await deactivate();
    await deactivate();

    expect(active.stops).toBe(1);
  });
});

describe("extension package", () => {
  it("ships runtime JavaScript and declarations without TypeScript tests", () => {
    const packed = spawnSync("npm", ["pack", "--dry-run", "--json"], {
      cwd: join(__dirname, ".."),
      encoding: "utf8",
      shell: process.platform === "win32",
    });
    expect(packed.status, packed.stderr).toBe(0);
    const result = JSON.parse(packed.stdout) as Array<{
      files: Array<{ path: string }>;
    }>;
    const files = result[0].files.map((file) => file.path);

    expect(files).toContain("package.json");
    expect(files).toContain("dist/extension.js");
    expect(files).toContain("dist/extension.d.ts");
    expect(files.some((path) => path.endsWith(".test.ts"))).toBe(false);
    expect(files.some((path) => path.startsWith("src/"))).toBe(false);
  });
});
