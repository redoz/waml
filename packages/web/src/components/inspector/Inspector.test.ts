import { test, expect, vi } from "vitest";
import { render, screen } from "@testing-library/svelte";
import type { ModelNode } from "@uaml/okf";
import Inspector from "./Inspector.svelte";

const nodes = [{
  concept: { id: "n1", type: "uml.Class", title: "Order", body: "" },
  key: "n1", title: "Order", type: "uml.Class", stereotypes: [], attributes: [],
  position: { x: 0, y: 0 },
}] as ModelNode[];

test("embedded node selection shows the object title field", () => {
  render(Inspector, {
    props: {
      selection: { type: "node", id: "n1" }, nodes, edges: [],
      onUpdateNode: vi.fn(), onUpdateEdge: vi.fn(), onClose: vi.fn(), embedded: true,
    },
  });
  expect(screen.getByDisplayValue("Order")).toBeTruthy();
});

test("embedded null selection shows the empty state", () => {
  render(Inspector, {
    props: {
      selection: null, nodes, edges: [],
      onUpdateNode: vi.fn(), onUpdateEdge: vi.fn(), onClose: vi.fn(), embedded: true,
    },
  });
  // React source text (Inspector.tsx EmptyState) — brief prose paraphrased this.
  expect(document.body.textContent).toContain("Select an object or relationship to edit.");
});
