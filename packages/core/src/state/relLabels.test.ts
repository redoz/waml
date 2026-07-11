import { describe, it, expect, beforeEach } from "vitest";
import { loadRelLabelMode, persistRelLabelMode } from "./relLabels";

describe("relLabels", () => {
  beforeEach(() => localStorage.clear());
  it("defaults to all", () => expect(loadRelLabelMode()).toBe("all"));
  it("round-trips hidden", () => { persistRelLabelMode("hidden"); expect(loadRelLabelMode()).toBe("hidden"); });
  it("coerces persisted legacy modes to all", () => {
    localStorage.setItem("mc.relLabels.v1", "defined");
    expect(loadRelLabelMode()).toBe("all");
  });
});
