import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import type { ModelEdge, ModelNode } from "@waml/okf";
import RelationshipInspectorReadonly from "./RelationshipInspectorReadonly.svelte";

const node = (key: string, title: string): ModelNode =>
  ({ key, type: "uml.Class", concept: { id: key, type: "uml.Class", title, body: "" }, stereotypes: [], attributes: [], position: { x: 0, y: 0 } });

const edge: ModelEdge = {
  id: "e1",
  kind: "associates",
  from: "a",
  to: "b",
  fromEnd: { multiplicity: "1" },
  toEnd: { multiplicity: "*", navigable: true },
  bidirectional: false,
};

test("renders endpoints, kind, and multiplicities as static text", () => {
  const { container } = render(RelationshipInspectorReadonly, {
    props: { edge, fromNode: node("a", "Order"), toNode: node("b", "OrderLine") },
  });
  expect(screen.getByText("Order")).toBeTruthy();
  expect(screen.getByText("OrderLine")).toBeTruthy();
  expect(screen.getByText("associates")).toBeTruthy();
  expect(screen.getByText("Order multiplicity")).toBeTruthy();
  expect(container.querySelector("input")).toBeNull();
  expect(container.querySelector("select")).toBeNull();
});

test("hides end fields for specializes (no ended kinds)", () => {
  render(RelationshipInspectorReadonly, {
    props: {
      edge: { ...edge, kind: "specializes", fromEnd: {}, toEnd: {} },
      fromNode: node("a", "Child"),
      toNode: node("b", "Parent"),
    },
  });
  expect(screen.queryByText("Child multiplicity")).toBeNull();
});
