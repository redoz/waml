import { describe, it, expect } from "vitest";
import { gatedPanelId } from "./useRightPanel";

describe("gatedPanelId — rail gating logic", () => {
  // ── signed-out gating ──────────────────────────────────────────────────────
  it("signed out: My Models routes to enable", () => {
    expect(gatedPanelId("models", false)).toBe("enable");
  });

  it("signed out: History routes to enable", () => {
    expect(gatedPanelId("history", false)).toBe("enable");
  });

  it("signed out: Share opens directly (no gate)", () => {
    expect(gatedPanelId("share", false)).toBe("share");
  });

  it("signed out: Inspect opens directly (no gate)", () => {
    expect(gatedPanelId("inspect", false)).toBe("inspect");
  });

  // ── signed-in — all panels open directly ──────────────────────────────────
  it("signed in: My Models opens directly", () => {
    expect(gatedPanelId("models", true)).toBe("models");
  });

  it("signed in: History opens directly", () => {
    expect(gatedPanelId("history", true)).toBe("history");
  });

  it("signed in: Share opens directly", () => {
    expect(gatedPanelId("share", true)).toBe("share");
  });

  it("signed in: Inspect opens directly", () => {
    expect(gatedPanelId("inspect", true)).toBe("inspect");
  });
});
