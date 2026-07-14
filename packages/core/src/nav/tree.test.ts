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
    node("", "", "uml.Package"),
    node("sales", "sales", "uml.Package"),
  ].map((p, i) => ({ ...p, members: i === 0 ? ["sales"] : ["order", "billing-rules", "overview"] })),
} as unknown as ModelGraph;

test("diagrams float to the top of a package regardless of members order", () => {
  const rows = buildNavTree(g, "sales");
  expect(rows.map((r) => r.key)).toEqual(["overview", "order", "billing-rules"]);
  expect(rows[0].kind).toBe("diagram");
  expect(rows[2].kind).toBe("note");
});

test("packageOf finds the owning package", () => {
  expect(packageOf(g, "order")).toBe("sales");
  expect(packageOf(g, "sales")).toBe("");
});
