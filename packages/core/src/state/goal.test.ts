import { describe, it, expect, beforeEach } from "vitest";
import { NICHE_PRESETS, loadGoal, persistGoal } from "./goal";

beforeEach(() => localStorage.clear());

describe("goal state", () => {
  it("ships 20 niches, each with 5 goals and a unique id", () => {
    expect(NICHE_PRESETS).toHaveLength(20);
    const ids = new Set<string>();
    for (const n of NICHE_PRESETS) {
      expect(n.id).toBeTruthy();
      expect(n.label).toBeTruthy();
      expect(n.goals).toHaveLength(5);
      ids.add(n.id);
    }
    expect(ids.size).toBe(NICHE_PRESETS.length);
  });

  it("round-trips a goal through localStorage", () => {
    expect(loadGoal()).toBeNull();
    persistGoal({ niche: "E-commerce / Retail", goal: "Increase ROAS while holding CPC" });
    expect(loadGoal()).toEqual({ niche: "E-commerce / Retail", goal: "Increase ROAS while holding CPC" });
  });

  it("persist(null) clears the stored goal", () => {
    persistGoal({ niche: "SaaS", goal: "Reduce churn" });
    persistGoal(null);
    expect(loadGoal()).toBeNull();
  });

  it("returns null on malformed stored JSON", () => {
    localStorage.setItem("mc.goal.v1", "{not json");
    expect(loadGoal()).toBeNull();
  });
});
