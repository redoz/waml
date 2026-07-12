import { test, expect } from "vitest";
import { filterNav, matchSpan } from "./search";
import type { ModelGraph } from "@uaml/okf";

const node = (key: string, title: string, type = "uml.Class") => ({
  key,
  type,
  concept: { id: key, type, title, body: "" },
  stereotypes: [],
  attributes: [],
  position: { x: 0, y: 0 },
});
const pkg = (key: string, title: string, members: string[]) => ({ ...node(key, title, "uml.Package"), members });

const g = {
  path: "acme",
  nodes: [node("order", "Order"), node("payment", "Payment")],
  edges: [],
  diagrams: [],
  packages: [
    pkg("", "", ["sales", "billing"]),
    pkg("sales", "sales", ["order"]),
    pkg("billing", "billing", ["payment"]),
  ],
} as unknown as ModelGraph;

test("zero in scope but matches elsewhere -> empty-scope state with elsewhere tree", () => {
  const r = filterNav(g, "sales", "payment", "all");
  expect(r.state).toBe("empty-scope");
  expect(r.inScope).toHaveLength(0);
  expect(r.elsewhere.some((row) => row.title.toLowerCase().includes("payment"))).toBe(true);
  // ancestor package kept full-strength
  expect(r.elsewhere.some((row) => row.kind === "package")).toBe(true);
});

test("no matches anywhere -> empty-all", () => {
  expect(filterNav(g, "sales", "zzzzz", "all").state).toBe("empty-all");
});

test("empty query returns all in-scope rows", () => {
  const r = filterNav(g, "sales", "", "all");
  expect(r.state).toBe("matches");
  expect(r.inScope.map((row) => row.key)).toEqual(["order"]);
});

test("type filter keeps only the matching metaclass (plus ancestor packages)", () => {
  const r = filterNav(g, "", "", "uml.Class");
  expect(r.inScope.some((row) => row.key === "order")).toBe(true);
  expect(r.inScope.some((row) => row.kind === "package")).toBe(true);
});

test("matchSpan locates the case-insensitive hit", () => {
  expect(matchSpan("Order", "rd")).toEqual([1, 3]);
  expect(matchSpan("Order", "zz")).toBeNull();
});
