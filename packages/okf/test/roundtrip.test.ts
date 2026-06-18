import { describe, it, expect } from "vitest";
import { serializeBundle, parseBundle } from "../src/index";
import type { ModelGraph } from "../src/types";

const graph: ModelGraph = {
  storageId: "stor_1",
  nodes: [
    { key: "fb", title: "Facebook Ads", inputSource: "CONNECTOR", description: "ads",
      schema: [{ name: "campaign_id", type: "STRING", pk: false }], position: { x: 10, y: 20 },
      status: "pending", owoxId: null },
    { key: "camp", title: "Campaigns", inputSource: "VIEW", schema: [{ name: "id", type: "STRING", pk: true }],
      position: { x: 200, y: 20 }, status: "pending", owoxId: null },
  ],
  edges: [{ id: "e1", from: "fb", to: "camp", keys: [{ left: "campaign_id", right: "id" }], bidirectional: false }],
};

describe("okf round-trip", () => {
  it("serializes to files and parses back to an equivalent graph", () => {
    const bundle = serializeBundle(graph, "Demo");
    expect(Object.keys(bundle.files)).toContain("demo/index.md");
    expect(Object.keys(bundle.files)).toContain("demo/facebook-ads.md");
    expect(bundle.files["demo/facebook-ads.md"]).toContain("## Joins");
    const back = parseBundle(bundle.files);
    expect(back.nodes.map(n => n.key).sort()).toEqual(["camp", "fb"]);
    expect(back.edges).toHaveLength(1);
    expect(back.edges[0]).toMatchObject({ from: "fb", to: "camp", keys: [{ left: "campaign_id", right: "id" }] });
  });
  it("collapses mutual Joins lines into one bidirectional edge", () => {
    const files = {
      "p/a.md": frontless("a", "A") + "\n## Joins\n- [B](./b.md) — `x = y`\n",
      "p/b.md": frontless("b", "B") + "\n## Joins\n- [A](./a.md) — `y = x`\n",
    };
    const g = parseBundle(files);
    expect(g.edges).toHaveLength(1);
    expect(g.edges[0].bidirectional).toBe(true);
  });
});
function frontless(key: string, title: string) {
  return `---\ntype: "OWOX Data Mart"\ntitle: "${title}"\nowox:\n  key: "${key}"\n  inputSource: "SQL"\n  position: { x: 0, y: 0 }\n---\n# ${title}`;
}
