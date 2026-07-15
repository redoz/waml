import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import { createRawSnippet } from "svelte";
import type { ModelNode, ModelEdge } from "@waml/okf";
import InspectorReadonly from "./InspectorReadonly.svelte";

const nodes: ModelNode[] = [
  { key: "a", type: "uml.Class", concept: { id: "a", type: "uml.Class", title: "Order", body: "" }, stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
  { key: "b", type: "uml.Class", concept: { id: "b", type: "uml.Class", title: "OrderLine", body: "" }, stereotypes: [], attributes: [], position: { x: 0, y: 0 } },
];
const edges: ModelEdge[] = [
  { id: "e1", kind: "associates", from: "a", to: "b", fromEnd: {}, toEnd: {}, bidirectional: false },
];

test("node selection renders the read-only object body plus externalRefs", () => {
  const externalRefs = createRawSnippet(() => ({ render: () => `<div data-testid="ext">refs</div>` }));
  render(InspectorReadonly, { props: { selection: { type: "node", id: "a" }, nodes, edges, externalRefs } });
  expect(screen.getByText("Order")).toBeTruthy();
  expect(screen.getByTestId("ext")).toBeTruthy();
});

test("edge selection renders the read-only relationship body", () => {
  render(InspectorReadonly, { props: { selection: { type: "edge", id: "e1" }, nodes, edges } });
  expect(screen.getByText("associates")).toBeTruthy();
});

test("null selection renders no editable controls", () => {
  const { container } = render(InspectorReadonly, { props: { selection: null, nodes, edges } });
  expect(container.querySelector("input")).toBeNull();
});
