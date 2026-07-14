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
