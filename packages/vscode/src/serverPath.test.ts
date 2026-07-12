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
