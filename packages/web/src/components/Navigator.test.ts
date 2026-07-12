import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Navigator from "./Navigator.svelte";

// Node/package fixture helper — mirrors the concept-Node shape (title lives on
// `concept.title`, never a flat `title`), matching nav/tree.test.ts.
const node = (key: string, title: string, type = "uml.Class") => ({
  key,
  type,
  concept: { id: key, type, title, body: "" },
  stereotypes: [],
  attributes: [],
  position: { x: 0, y: 0 },
});

const graph = {
  path: "acme-model",
  nodes: [node("customer", "Customer")],
  edges: [],
  diagrams: [{ key: "overview", title: "Sales overview", profile: "uml-domain", members: [] }],
  packages: [
    { ...node("", "", "uml.Package"), members: ["sales"] },
    { ...node("sales", "sales", "uml.Package"), members: ["overview", "customer"] },
  ],
};

const props = (over = {}) => ({
  graph,
  scopeKey: "sales",
  activeDiagramKey: "overview",
  palette: ["uml.Class"],
  onScope: vi.fn(),
  onSelectDiagram: vi.fn(),
  ...over,
});

test("renders scope breadcrumb and floated diagram first", () => {
  render(Navigator, { props: props() });
  expect(screen.getByText("acme-model")).toBeTruthy();
  const rows = screen.getAllByRole("treeitem");
  expect(rows[0].textContent).toContain("Sales overview");
});

test("clicking a diagram row selects it; package crumb rescopes", async () => {
  const onSelectDiagram = vi.fn();
  const onScope = vi.fn();
  render(Navigator, { props: props({ onSelectDiagram, onScope }) });
  await fireEvent.click(screen.getByRole("treeitem", { name: /Sales overview/ }));
  expect(onSelectDiagram).toHaveBeenCalledWith("overview");
  await fireEvent.click(screen.getByRole("button", { name: "acme-model" }));
  expect(onScope).toHaveBeenCalledWith("");
});
