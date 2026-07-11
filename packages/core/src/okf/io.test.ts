import { describe, it, expect } from "vitest";
import { bundleToZip, zipToFiles, graphToBundleFiles, filesToGraph } from "./io";
import type { ModelGraph } from "@uaml/okf";

const node = (key: string, title: string): ModelGraph["nodes"][0] =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const edge = (id: string, from: string, to: string): ModelGraph["edges"][0] =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: { navigable: true }, bidirectional: false });

describe("zip round-trip", () => {
  it("zips and unzips bundle files losslessly", () => {
    const files = { "demo/index.md": "# Demo\n", "demo/orders.md": "# Orders\n" };
    const buf = bundleToZip(files);
    expect(buf).toBeInstanceOf(Uint8Array);
    expect(zipToFiles(buf)).toEqual(files);
  });
});

describe("graphToBundleFiles", () => {
  const graph: ModelGraph = {
    nodes: [{ ...node("orders", "Orders"), attributes: [{ name: "id", type: { name: "STRING" }, multiplicity: "1" }] }],
    edges: [],
    diagrams: [],
  };

  it("appends a UAML attribution footer to the bundle index only", () => {
    const files = graphToBundleFiles(graph, "Demo");
    const indexKey = Object.keys(files).find(k => k.endsWith("index.md"))!;
    expect(files[indexKey]).toContain("Generated with");
    expect(files[indexKey]).toContain("UAML");
    expect(files[indexKey]).toContain("github.com/redoz/uaml");
    const martKey = Object.keys(files).find(k => k.endsWith("orders.md"))!;
    expect(files[martKey]).not.toContain("Generated with"); // per-mart docs stay clean
  });
});

describe("graph → bundle → graph round-trip", () => {
  it("preserves node keys and edge kind", () => {
    const graph: ModelGraph = {
      nodes: [node("orders", "Orders"), node("customers", "Customers")],
      edges: [edge("e1", "orders", "customers")],
      diagrams: [],
    };
    const back = filesToGraph(graphToBundleFiles(graph, "Demo"));
    expect(back.nodes.map(n => n.key).sort()).toEqual(["customers", "orders"]);
    expect(back.edges).toHaveLength(1);
    expect(back.edges[0].kind).toBe("associates");
  });
});
