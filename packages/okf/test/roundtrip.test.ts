import { describe, it, expect } from "vitest";
import { serializeBundle, parseBundle } from "../src/index";
import type { ModelGraph } from "../src/types";

const node = (key: string, title: string, attrs: ModelGraph["nodes"][0]["attributes"] = []): ModelGraph["nodes"][0] =>
  ({ key, title, type: "uml.Class", stereotypes: [], position: { x: 0, y: 0 }, attributes: attrs });

describe("okf round-trip (interim legacy format)", () => {
  it("serializes to files and parses back to an equivalent graph", () => {
    const graph: ModelGraph = {
      nodes: [
        node("fb", "Facebook Ads", [{ name: "campaign_id", type: { name: "STRING" }, multiplicity: "1" }]),
        node("camp", "Campaigns", [{ name: "id", type: { name: "STRING" }, multiplicity: "1" }]),
      ],
      edges: [{ id: "e1", kind: "associates", from: "fb", to: "camp",
                fromEnd: {}, toEnd: { navigable: true }, bidirectional: false }],
      diagrams: [],
    };
    const bundle = serializeBundle(graph, "Demo");
    expect(Object.keys(bundle.files)).toContain("demo/index.md");
    const back = parseBundle(bundle.files);
    expect(back.nodes.map(n => n.key).sort()).toEqual(["campaigns", "facebook-ads"]);
    expect(back.nodes.find(n => n.key === "campaigns")!.attributes[0])
      .toEqual({ name: "id", type: { name: "STRING" }, multiplicity: "1" });
    expect(back.edges).toHaveLength(1);
    expect(back.edges[0]).toMatchObject({ from: "facebook-ads", to: "campaigns", kind: "associates" });
  });

  it("survives 1/* end multiplicities via the [N:1] suffix", () => {
    const graph: ModelGraph = {
      nodes: [node("tx", "Transactions"), node("blocks", "Blocks")],
      edges: [{ id: "e1", kind: "associates", from: "tx", to: "blocks",
                fromEnd: { multiplicity: "*" }, toEnd: { multiplicity: "1", navigable: true }, bidirectional: false }],
      diagrams: [],
    };
    const back = parseBundle(serializeBundle(graph, "Demo").files);
    expect(back.edges[0].fromEnd.multiplicity).toBe("*");
    expect(back.edges[0].toEnd.multiplicity).toBe("1");
  });

  it("keeps both nodes when two titles slugify to the same value", () => {
    const graph: ModelGraph = {
      nodes: [node("posts", "Posts Answers"), node("answers", "Posts & Answers")],
      edges: [{ id: "e1", kind: "associates", from: "posts", to: "answers",
                fromEnd: {}, toEnd: { navigable: true }, bidirectional: false }],
      diagrams: [],
    };
    const { files } = serializeBundle(graph, "Demo");
    expect(Object.keys(files).filter(f => !f.endsWith("index.md"))).toHaveLength(2);
    const back = parseBundle(files);
    expect(back.nodes).toHaveLength(2);
    expect(new Set(back.nodes.map(n => n.key)).size).toBe(2);
    expect(back.edges).toHaveLength(1);
    expect(back.edges[0].from).not.toBe(back.edges[0].to);
  });

  it("collapses mutual join lines into one bidirectional edge", () => {
    const front = (t: string) => `---\ntitle: "${t}"\n---\n# ${t}\n`;
    const g = parseBundle({
      "p/a.md": front("A") + "\n## Joins\n- [B](./b.md)\n",
      "p/b.md": front("B") + "\n## Joins\n- [A](./a.md)\n",
    });
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].bidirectional).toBe(true);
    expect(g.edges[0].fromEnd.navigable).toBe(true);
    expect(g.edges[0].toEnd.navigable).toBe(true);
  });
});
