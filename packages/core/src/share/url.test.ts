import { describe, it, expect } from "vitest";
import { gzipSync, strToU8 } from "fflate";
import { encodeModel, decodeModel, buildShareUrl, readSharedName } from "./url";
import type { ModelGraph } from "@mc/okf";

const node = (key: string, title: string): ModelGraph["nodes"][0] =>
  ({ key, title, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });
const edge = (id: string, from: string, to: string): ModelGraph["edges"][0] =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: { navigable: true }, bidirectional: false });

const graph: ModelGraph = {
  nodes: [
    { ...node("orders", "Orders"), position: { x: 10, y: 20 },
      attributes: [{ name: "order_id", type: { name: "STRING" }, multiplicity: "1" }] },
    { ...node("customers", "Customers"), position: { x: 300, y: 40 } },
  ],
  edges: [edge("e1", "orders", "customers")],
  diagrams: [],
};

describe("share url", () => {
  it("round-trips a model through encode/decode (URL-safe)", () => {
    const payload = encodeModel(graph);
    expect(payload).toMatch(/^[A-Za-z0-9_-]+$/); // url-safe, no +/=
    const back = decodeModel(payload)!;
    expect(back.nodes.map(n => n.key)).toEqual(["orders", "customers"]);
    expect(back.edges[0]).toMatchObject({ from: "orders", to: "customers", kind: "associates" });
    expect(back.nodes[0].position).toEqual({ x: 10, y: 20 }); // layout preserved
    expect(back.nodes[0].attributes).toEqual(graph.nodes[0].attributes);
  });

  it("drops canvas-only handle hints from a public link", () => {
    const withHandles: ModelGraph = {
      ...graph,
      edges: [{ ...edge("e1", "orders", "customers"), sourceHandle: "right", targetHandle: "left" }],
    };
    const back = decodeModel(encodeModel(withHandles))!;
    expect(back.edges[0].sourceHandle).toBeUndefined();
    expect(back.edges[0].targetHandle).toBeUndefined();
  });

  it("returns null for a corrupt payload", () => {
    expect(decodeModel("not-a-real-payload")).toBeNull();
    expect(decodeModel("")).toBeNull();
  });

  it("carries the model name in the link and reads it back", () => {
    const url = buildShareUrl(graph, "My SaaS / Subscription OKF with OWOX");
    expect(url).toContain("&n=");
    // Load the hash as if the recipient opened the link.
    history.replaceState(null, "", url.slice(url.indexOf("#")));
    expect(readSharedName()).toBe("My SaaS / Subscription OKF with OWOX");
  });

  it("omits the name param when no name is given, and reads null", () => {
    const url = buildShareUrl(graph);
    expect(url).not.toContain("&n=");
    history.replaceState(null, "", url.slice(url.indexOf("#")));
    expect(readSharedName()).toBeNull();
  });

  it("decodes a legacy (mart-era) share payload via migration", () => {
    const legacyJson = JSON.stringify({
      storageId: null,
      nodes: [{ key: "n1", title: "Orders", inputSource: "SQL", schema: [{ name: "id", type: "STRING", pk: true }],
                position: { x: 1, y: 2 }, status: "pending", owoxId: null }],
      edges: [{ id: "e1", from: "n1", to: "n2", keys: [], bidirectional: false, cardinality: "N:1" }],
    });
    const bytes = gzipSync(strToU8(legacyJson));
    let bin = ""; for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
    const payload = btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    const g = decodeModel(payload)!;
    expect(g.nodes[0].type).toBe("uml.Class");
    expect(g.nodes[0].attributes[0].name).toBe("id");
    expect(g.edges[0].kind).toBe("associates");
    expect(g.diagrams).toEqual([]);
  });
});
