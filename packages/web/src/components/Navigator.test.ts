import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Navigator from "./Navigator.svelte";
import type { ModelGraph } from "@uaml/okf";

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
} as unknown as ModelGraph;

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

test("Ctrl-T rotates the type chip through palette without an inline hint", async () => {
  render(Navigator, { props: props({ palette: ["uml.Class", "uml.Interface"] }) });
  const chip = screen.getByRole("button", { name: /Filter by type/ });
  expect(chip.textContent).toContain("All");
  await fireEvent.keyDown(window, { key: "t", ctrlKey: true });
  expect(chip.textContent).toContain("Class");
  expect(chip.textContent).not.toMatch(/Ctrl/i);
});

test("dropping a reordered row persists new member order", async () => {
  const onReorder = vi.fn();
  render(Navigator, { props: props({ onReorder }) });
  const rows = screen.getAllByRole("treeitem");
  await fireEvent.dragStart(rows[1]); // customer
  await fireEvent.drop(rows[0]); // above overview's slot
  expect(onReorder).toHaveBeenCalled();
  const [pkgKey, order] = onReorder.mock.calls[0];
  expect(pkgKey).toBe("sales");
  expect(order).toContain("customer");
});

test("classifier with one containing diagram jumps; edit-props calls stub", async () => {
  const onViewInDiagram = vi.fn();
  const onEditProperties = vi.fn();
  const g2 = structuredClone(graph);
  g2.diagrams[0].members = ["customer"];
  render(Navigator, { props: props({ graph: g2, onViewInDiagram, onEditProperties }) });
  await fireEvent.click(screen.getByRole("treeitem", { name: /Customer/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /View in diagram/ }));
  expect(onViewInDiagram).toHaveBeenCalledWith("customer", "overview");
  await fireEvent.click(screen.getByRole("treeitem", { name: /Customer/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /View \/ edit properties/ }));
  expect(onEditProperties).toHaveBeenCalledWith("customer");
});

test("classifier in no diagram shows Add to new diagram", async () => {
  const onAddToNewDiagram = vi.fn();
  render(Navigator, { props: props({ onAddToNewDiagram }) });
  await fireEvent.click(screen.getByRole("treeitem", { name: /Customer/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /Add to new diagram/ }));
  expect(onAddToNewDiagram).toHaveBeenCalledWith("customer");
});
