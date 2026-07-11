import { describe, it, expect, beforeEach } from "vitest";
import { loadViewMode, persistViewMode } from "./viewMode";

describe("viewMode persistence", () => {
  beforeEach(() => localStorage.clear());

  it("defaults to compact when nothing is stored", () => {
    expect(loadViewMode()).toBe("compact");
  });

  it("round-trips a persisted mode", () => {
    persistViewMode("erd");
    expect(loadViewMode()).toBe("erd");
  });

  it("falls back to compact for an unrecognised stored value", () => {
    localStorage.setItem("mc.viewMode.v1", "bogus");
    expect(loadViewMode()).toBe("compact");
  });
});
