import { describe, it, expect, beforeAll } from "vitest";
import { initWasm } from "@waml/wasm";
import { createModelStore, type Bundle } from "./model";
import { ALL_DIAGRAM_KEY } from "./diagrams";
import { resolveDisplay } from "@waml/okf";

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
    expect(g.nodes.map((n) => n.key).sort()).toEqual(["m/customer", "m/order"]);
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

  it("addNode broadcasts the new node at the drop position, not the origin", () => {
    const s = createModelStore(fresh());
    let broadcastPos: { x: number; y: number } | undefined;
    s.subscribe(() => {
      const created = s.get().nodes.find((x) => x.key !== "m/order" && x.key !== "m/customer");
      if (created) broadcastPos = created.position;
    });
    s.addNode({ x: 10, y: 20 });
    expect(broadcastPos).toEqual({ x: 10, y: 20 });
  });

  it("updateNode(scalar) edits the bundle via apply_ops", () => {
    const s = createModelStore(fresh());
    const order = s.get().nodes.find((n) => n.key === "m/order")!;
    s.updateNode("m/order", { concept: { ...order.concept, title: "Sales Order" } });
    expect(s.get().nodes.find((n) => n.key === "m/order")!.concept.title).toBe("Sales Order");
    expect(s.getBundle().find(([p]) => p.endsWith("order.md"))![1]).toContain("Sales Order");
  });

  it("addEdge writes a relationship into the source document", () => {
    const s = createModelStore(fresh());
    const e = s.addEdge("m/order", "m/customer");
    expect(e).not.toBeNull();
    expect(s.get().edges.some((x) => x.from === "m/order" && x.to === "m/customer")).toBe(true);
    expect(s.getBundle().find(([p]) => p.endsWith("order.md"))![1]).toContain("associates");
  });

  it("removeNode drops the document and its outgoing edges", () => {
    const s = createModelStore(fresh());
    s.addEdge("m/order", "m/customer");
    s.removeNode("m/order");
    expect(s.get().nodes.some((n) => n.key === "m/order")).toBe(false);
    expect(s.get().edges.length).toBe(0);
  });

  it("position-only updateNode leaves the bundle byte-identical (overlay only)", () => {
    const s = createModelStore(fresh());
    const before = JSON.stringify(s.getBundle());
    s.updateNode("m/order", { position: { x: 99, y: 88 } });
    expect(JSON.stringify(s.getBundle())).toBe(before);
    expect(s.get().nodes.find((n) => n.key === "m/order")!.position).toEqual({ x: 99, y: 88 });
  });

  it("edge handle change leaves the bundle byte-identical (overlay only)", () => {
    const s = createModelStore(fresh());
    const e = s.addEdge("m/order", "m/customer")!;
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
    expect(s.get().nodes.map((n) => n.key)).toEqual(["m/widget"]);
    expect(s.get().nodes[0].position).toEqual({ x: 0, y: 0 });
  });

  it("retitlePackage writes the root index.md H1 and updates path", () => {
    const s = createModelStore([
      ["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
    ] as Bundle);
    s.retitlePackage("", "Acme Domain");
    expect(s.get().path).toBe("Acme Domain");
    const idx = s.getBundle().find(([p]) => p === "index.md");
    expect(idx?.[1]).toContain("# Acme Domain");
  });
});

describe("package mutators + ghost state", () => {
  it("ghost package appears then materializes on first child", () => {
    const store = createModelStore([["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]]);
    const key = store.createGhostPackage("", "Sales");
    expect(key).toBe("sales");
    expect(store.get().packages.some((p) => p.key === "sales")).toBe(true);
    // ghost is NOT in the bundle yet
    expect(store.getBundle().some(([p]) => p.startsWith("sales/"))).toBe(false);
    // add first child -> materialized in the bundle, ghost pruned
    store.createNodeInPackage("sales", "uml.Class", "Customer");
    expect(store.getBundle().some(([p]) => p.startsWith("sales/"))).toBe(true);
    expect(store.get().packages.some((p) => p.key === "sales")).toBe(true);
  });

  it("moveNode relocates a doc via pkg.move", () => {
    const store = createModelStore([["sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]]);
    store.moveNode("order", "billing");
    expect(store.getBundle().some(([p]) => p === "billing/order.md")).toBe(true);
  });

  it("insertPackage re-roots docs under the target path", () => {
    const store = createModelStore([]);
    const ok = store.insertPackage("", "orders", [
      ["t/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
    ]);
    expect(ok).toBe(true);
    expect(store.getBundle().some(([p]) => p === "orders/order.md")).toBe(true);
  });

  it("insertPackage returns false and surfaces an error on a path collision", () => {
    const errors: string[] = [];
    const store = createModelStore(
      [["sales/orders/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]],
      { onError: (e) => errors.push(e) },
    );
    const ok = store.insertPackage("sales", "orders", [["t/x.md", "---\ntype: uml.Class\ntitle: X\n---\n# X\n"]]);
    expect(ok).toBe(false);
    expect(errors.join()).toContain("already exists");
  });
});

describe("updateDiagram", () => {
  it("updateDiagram persists display on a real diagram doc", () => {
    const bundle: Bundle = [
      ["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
      ["dia.md", "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [Order](./order.md)\n"],
    ];
    const store = createModelStore(bundle);
    const key = store.get().diagrams.find((d) => d.title === "D")!.key;
    store.updateDiagram(key, { display: resolveDisplay({ showAttributes: false }) });
    const after = store.get().diagrams.find((d) => d.key === key)!;
    expect(after.display?.showAttributes).toBe(false);
  });

  it("updateDiagram on the implicit All diagram is a silent no-op", () => {
    const store = createModelStore([["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]]);
    const before = store.get().diagrams; // [] — no authored Diagram docs; "All" is synthesized downstream, not here
    store.updateDiagram(ALL_DIAGRAM_KEY, { display: resolveDisplay({ showAttributes: false }) });
    const after = store.get().diagrams;
    expect(after).toHaveLength(0); // nothing was persisted
    expect(after).toEqual(before);
  });
});
