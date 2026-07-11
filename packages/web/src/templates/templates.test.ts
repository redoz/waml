import { describe, it, expect } from "vitest";
import { TEMPLATES } from "./index";

describe("built-in templates", () => {
  it("every template graph is new-shape", () => {
    for (const t of TEMPLATES) {
      expect(Array.isArray(t.graph.diagrams)).toBe(true);
      for (const n of t.graph.nodes) {
        expect(n.type).toBe("uml.Class");
        expect(Array.isArray(n.attributes)).toBe(true);
        expect((n as unknown as Record<string, unknown>).schema).toBeUndefined();
      }
      for (const e of t.graph.edges) {
        expect(e.kind).toBe("associates");
        expect((e as unknown as Record<string, unknown>).keys).toBeUndefined();
      }
    }
  });
  it("default N:1 cardinality became */1 end multiplicities", () => {
    const withEdges = TEMPLATES.find(t => t.graph.edges.length > 0)!;
    const e = withEdges.graph.edges[0];
    expect(e.fromEnd.multiplicity).toBe("*");
    expect(e.toEnd.multiplicity).toBe("1");
  });
});
