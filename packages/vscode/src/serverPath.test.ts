import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import ts from "typescript";
import {
  resolveServerPath,
  getServerCommand,
  setServerCommand,
  type ServerPathContext,
  type ConfigInspection,
} from "./serverPath";

const extensionSourcePath = join(__dirname, "extension.ts");
const extensionSource = readFileSync(extensionSourcePath, "utf8");
const extensionFile = ts.createSourceFile(
  extensionSourcePath,
  extensionSource,
  ts.ScriptTarget.Latest,
  true,
  ts.ScriptKind.TS,
);

function initializerFor(name: string): ts.Expression {
  let result: ts.Expression | undefined;
  function visit(node: ts.Node): void {
    if (ts.isVariableDeclaration(node)) {
      if (ts.isIdentifier(node.name) && node.name.text === name && node.initializer) {
        result = node.initializer;
      }
    }
    if (!result) ts.forEachChild(node, visit);
  }
  visit(extensionFile);
  if (result) return result;
  throw new Error(`Missing ${name} initializer in extension.ts`);
}

function objectProperty(object: ts.ObjectLiteralExpression, name: string): ts.Expression {
  const property = object.properties.find(
    (candidate): candidate is ts.PropertyAssignment =>
      ts.isPropertyAssignment(candidate) &&
      ts.isIdentifier(candidate.name) &&
      candidate.name.text === name,
  );
  if (!property) throw new Error(`Missing ${name} property`);
  return property.initializer;
}

function importedModules(): string[] {
  return extensionFile.statements
    .filter(ts.isImportDeclaration)
    .map((declaration) => declaration.moduleSpecifier)
    .filter(ts.isStringLiteral)
    .map((specifier) => specifier.text);
}

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
  it("keeps launch ownership in vscode-languageclient with exact waml lsp stdio options", () => {
    const initializer = initializerFor("serverOptions");
    expect(ts.isObjectLiteralExpression(initializer)).toBe(true);
    if (!ts.isObjectLiteralExpression(initializer)) return;

    expect(objectProperty(initializer, "command").getText(extensionFile)).toBe(
      "getServerCommand()",
    );
    expect(objectProperty(initializer, "args").getText(extensionFile)).toBe('["lsp"]');
    expect(objectProperty(initializer, "transport").getText(extensionFile)).toBe(
      "TransportKind.stdio",
    );
  });

  it("starts one language client and delegates shutdown to that client", () => {
    expect(extensionSource.match(/new LanguageClient\(/g)).toHaveLength(1);
    expect(extensionSource.match(/\bclient\.start\(\)/g)).toHaveLength(1);
    expect(extensionSource.match(/\bclient\?\.stop\(\)/g)).toHaveLength(1);
  });

  it("does not import parser, syntax, WASM, or retired TypeScript domains", () => {
    expect(importedModules()).not.toContain("@waml/parser");
    for (const moduleName of importedModules()) {
      expect(moduleName).not.toMatch(
        /(^@waml\/|(?:^|[/_-])(?:wasm|parser|syntax)(?:$|[/_.-]))/i,
      );
    }
  });
});
