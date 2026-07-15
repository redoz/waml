import { test, expect } from "vitest";
import { buildNavTree, packageOf } from "./tree";
import type { ModelGraph } from "@waml/okf";

const node = (key: string, title: string, type = "uml.Class") => ({
  key,
  type,
  concept: { id: key, type, title, body: "" },
  stereotypes: [],
  attributes: [],
  position: { x: 0, y: 0 },
});

const g = {
  path: "acme",
  nodes: [node("order", "Order"), node("billing-rules", "Billing rules", "uml.Note")],
  edges: [],
  diagrams: [{ key: "overview", title: "Sales overview", profile: "uml-domain", members: [] }],
  packages: [
    { ...node("", "", "uml.Package"), members: ["sales", "billing"] },
    {
      ...node("sales", "sales", "uml.Package"),
      // Flow/interaction doc keys live in their owning package's `members`
      // array just like diagrams and classifiers do (parse.rs's
      // build_packages does not exclude behavior docs).
      members: ["order", "billing-rules", "overview", "checkout-flow", "checkout-seq"],
    },
    { ...node("billing", "billing", "uml.Package"), members: [] },
  ],
  flows: [{ key: "checkout-flow", title: "Checkout flow", flavor: "activity", nodes: [], edges: [] }],
  interactions: [{ key: "checkout-seq", title: "Checkout sequence", lifelines: [], messages: [] }],
} as unknown as ModelGraph;

test("diagrams float to the top of a package regardless of members order", () => {
  const rows = buildNavTree(g, "sales");
  expect(rows.map((r) => r.key)).toEqual(["overview", "order", "billing-rules", "checkout-flow", "checkout-seq"]);
  expect(rows[0].kind).toBe("diagram");
  expect(rows[2].kind).toBe("note");
});

test("packageOf finds the owning package", () => {
  expect(packageOf(g, "order")).toBe("sales");
  expect(packageOf(g, "sales")).toBe("");
});

test("flow/interaction docs appear as rows nested in their owning package, with correct kind/title", () => {
  const rows = buildNavTree(g, "sales");
  const flowRow = rows.find((r) => r.key === "checkout-flow");
  const seqRow = rows.find((r) => r.key === "checkout-seq");
  expect(flowRow).toEqual({ key: "checkout-flow", title: "Checkout flow", kind: "flow", depth: 0 });
  expect(seqRow).toEqual({ key: "checkout-seq", title: "Checkout sequence", kind: "sequence", depth: 0 });
});

test("flow/interaction rows are scoped like any other package member", () => {
  // Scoped into the owning package: present.
  const salesKeys = buildNavTree(g, "sales").map((r) => r.key);
  expect(salesKeys).toContain("checkout-flow");
  expect(salesKeys).toContain("checkout-seq");

  // Scoped into an unrelated sibling package: absent.
  const billingKeys = buildNavTree(g, "billing").map((r) => r.key);
  expect(billingKeys).not.toContain("checkout-flow");
  expect(billingKeys).not.toContain("checkout-seq");

  // Scoped at the model root: present (nested one level deeper, since the
  // whole subtree from root is fully expanded), and their kind survives
  // recursion into the sub-package.
  const rootRows = buildNavTree(g, "");
  const rootFlowRow = rootRows.find((r) => r.key === "checkout-flow");
  const rootSeqRow = rootRows.find((r) => r.key === "checkout-seq");
  expect(rootFlowRow).toEqual({ key: "checkout-flow", title: "Checkout flow", kind: "flow", depth: 1 });
  expect(rootSeqRow).toEqual({ key: "checkout-seq", title: "Checkout sequence", kind: "sequence", depth: 1 });
});

test("no flows/interactions -> no flow/sequence rows (fields are optional)", () => {
  const bare = { ...g, flows: undefined, interactions: undefined } as unknown as ModelGraph;
  const rows = buildNavTree(bare, "sales");
  expect(rows.some((r) => r.kind === "flow" || r.kind === "sequence")).toBe(false);
});
