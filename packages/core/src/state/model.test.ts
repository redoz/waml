import { describe, it, expect, beforeAll } from "vitest";
import { initWasm } from "@uaml/okf";
import { createModelStore, type Bundle } from "./model";

beforeAll(async () => {
  await initWasm();
});

const doc = (slug: string, title: string, body = ""): [string, string] => [
  `m/${slug}.md`,
  `---\ntype: uml.Class\ntitle: ${title}\n---\n\n# ${title}\n${body}`,
];

function fresh(): Bundle {
  return [doc("order", "Order"), doc("customer", "Customer")];
}

describe("bundle-as-truth model store", () => {
  it("get() derives a ModelGraph from the bundle", () => {
    const s = createModelStore(fresh());
    const g = s.get();
    expect(g.nodes.map((n) => n.key).sort()).toEqual(["customer", "order"]);
    expect(g.nodes[0].position).toEqual({ x: 0, y: 0 });
  });

  it("addNode mutates the bundle (re-derivable) and emits", () => {
    const s = createModelStore(fresh());
    let fired = 0;
    s.subscribe(() => fired++);
    const n = s.addNode({ x: 10, y: 20 });
    expect(fired).toBeGreaterThan(0);
    expect(s.get().nodes.some((x) => x.key === n.key)).toBe(true);
    // Re-derive from the raw bundle: the new node persisted as a document.
    const rederived = createModelStore(s.getBundle());
    expect(rederived.get().nodes.some((x) => x.key === n.key)).toBe(true);
    // The position rode the overlay, not the bundle.
    expect(s.get().nodes.find((x) => x.key === n.key)!.position).toEqual({ x: 10, y: 20 });
  });

  it("updateNode(scalar) edits the bundle via apply_ops", () => {
    const s = createModelStore(fresh());
    s.updateNode("order", { title: "Sales Order" });
    expect(s.get().nodes.find((n) => n.key === "order")!.title).toBe("Sales Order");
    expect(s.getBundle().find(([p]) => p.endsWith("order.md"))![1]).toContain("Sales Order");
  });

  it("addEdge writes a relationship into the source document", () => {
    const s = createModelStore(fresh());
    const e = s.addEdge("order", "customer");
    expect(e).not.toBeNull();
    expect(s.get().edges.some((x) => x.from === "order" && x.to === "customer")).toBe(true);
    expect(s.getBundle().find(([p]) => p.endsWith("order.md"))![1]).toContain("associates");
  });

  it("removeNode drops the document and its outgoing edges", () => {
    const s = createModelStore(fresh());
    s.addEdge("order", "customer");
    s.removeNode("order");
    expect(s.get().nodes.some((n) => n.key === "order")).toBe(false);
    expect(s.get().edges.length).toBe(0);
  });

  it("position-only updateNode leaves the bundle byte-identical (overlay only)", () => {
    const s = createModelStore(fresh());
    const before = JSON.stringify(s.getBundle());
    s.updateNode("order", { position: { x: 99, y: 88 } });
    expect(JSON.stringify(s.getBundle())).toBe(before);
    expect(s.get().nodes.find((n) => n.key === "order")!.position).toEqual({ x: 99, y: 88 });
  });

  it("edge handle change leaves the bundle byte-identical (overlay only)", () => {
    const s = createModelStore(fresh());
    const e = s.addEdge("order", "customer")!;
    const before = JSON.stringify(s.getBundle());
    s.updateEdge(e.id, { sourceHandle: "r", targetHandle: "l" });
    expect(JSON.stringify(s.getBundle())).toBe(before);
    const edge = s.get().edges.find((x) => x.id === e.id)!;
    expect(edge.sourceHandle).toBe("r");
    expect(edge.targetHandle).toBe("l");
  });

  it("a rejected op leaves bundle + derived graph unchanged and surfaces the error", () => {
    const errors: string[] = [];
    const s = createModelStore(fresh(), { onError: (e) => errors.push(e) });
    const bundleBefore = JSON.stringify(s.getBundle());
    const graphBefore = JSON.stringify(s.get());
    // removeNode of a non-existent slug → node.rm errors ("no document"); the
    // store must keep its prior state and report the error, never partial-mutate.
    s.removeNode("ghost");
    expect(errors.length).toBe(1);
    expect(JSON.stringify(s.getBundle())).toBe(bundleBefore);
    expect(JSON.stringify(s.get())).toBe(graphBefore);
  });

  it("load() swaps the whole bundle and resets positions", () => {
    const s = createModelStore(fresh());
    s.updateNode("order", { position: { x: 5, y: 5 } });
    s.load([doc("widget", "Widget")]);
    expect(s.get().nodes.map((n) => n.key)).toEqual(["widget"]);
    expect(s.get().nodes[0].position).toEqual({ x: 0, y: 0 });
  });
});
