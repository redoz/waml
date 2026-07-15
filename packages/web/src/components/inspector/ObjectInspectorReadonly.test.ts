// packages/web/src/components/inspector/ObjectInspectorReadonly.test.ts
import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import type { ModelNode } from "@waml/okf";
import ObjectInspectorReadonly from "./ObjectInspectorReadonly.svelte";

const node: ModelNode = {
  key: "order",
  type: "uml.Class",
  concept: { id: "order", type: "uml.Class", title: "Order", description: "A placed order", body: "" },
  stereotypes: ["aggregateRoot"],
  attributes: [{ name: "total", type: { name: "Money" }, multiplicity: "1" }],
  position: { x: 0, y: 0 },
};

test("renders node fields as static text with no editable controls", () => {
  const { container } = render(ObjectInspectorReadonly, { props: { node } });
  expect(screen.getByText("Order")).toBeTruthy();
  expect(screen.getByText("A placed order")).toBeTruthy();
  expect(screen.getByText("uml.Class")).toBeTruthy();
  expect(screen.getByText("«aggregateRoot»")).toBeTruthy();
  expect(container.querySelector("input")).toBeNull();
  expect(container.querySelector("textarea")).toBeNull();
  expect(container.querySelector("select")).toBeNull();
});

test("shows an abstract badge only when the node is abstract", () => {
  const { rerender } = render(ObjectInspectorReadonly, { props: { node } });
  expect(screen.queryByText("abstract")).toBeNull();
  rerender({ node: { ...node, abstract: true } });
  expect(screen.getByText("abstract")).toBeTruthy();
});

test("shows the enum values list for uml.Enum", () => {
  render(ObjectInspectorReadonly, {
    props: { node: { ...node, type: "uml.Enum", values: ["NEW", "PAID"] } },
  });
  expect(screen.getByText("NEW")).toBeTruthy();
  expect(screen.getByText("PAID")).toBeTruthy();
});
