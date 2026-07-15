import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import CentralEditPanelHost from "./CentralEditPanelHost.svelte";
import { DEFAULT_DISPLAY, type ModelNode, type ModelEdge, type Diagram } from "@waml/okf";

const node = (key: string, title: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title, description: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as unknown as ModelNode;

const edge = (id: string, from: string, to: string): ModelEdge =>
  ({ id, kind: "associates", from, to, fromEnd: {}, toEnd: {}, bidirectional: false });

const diagram: Diagram = { key: "orders", title: "Orders", profile: "uml-domain", members: [] };

const props = (over = {}) => ({
  state: null,
  nodes: [node("customer", "Customer"), node("order", "Order")],
  edges: [edge("e1", "customer", "order")],
  display: { ...DEFAULT_DISPLAY },
  diagram,
  candidateStereotypes: [] as string[],
  editable: true,
  profileName: "uml-domain",
  options: [
    { key: "orders", label: "Orders", kind: "diagram" as const },
    { key: "customer", label: "Customer", kind: "node" as const },
    { key: "order", label: "Order", kind: "node" as const },
    { key: "e1", label: "Customer → Order", kind: "edge" as const },
  ],
  showPreview: false,
  onSelectElement: vi.fn(),
  onUpdateNode: vi.fn(),
  onUpdateEdge: vi.fn(),
  onDisplayChange: vi.fn(),
  onUpdateDiagram: vi.fn(),
  onClose: vi.fn(),
  ...over,
});

test("null state renders nothing", () => {
  render(CentralEditPanelHost, { props: props({ state: null }) });
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("element state mounts ObjectInspector, header picker reflects the node", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" } }),
  });
  expect(screen.getByRole("combobox", { name: "Select element" }).textContent).toContain("Customer");
  // ObjectInspector's Title field is present inside the host.
  expect(screen.getByLabelText("Title")).toBeTruthy();
});

test("picking another element from the header fires onSelectElement", async () => {
  const onSelectElement = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" }, onSelectElement }),
  });
  await fireEvent.click(screen.getByRole("combobox", { name: "Select element" }));
  await fireEvent.click(screen.getByRole("option", { name: "Order" }));
  expect(onSelectElement).toHaveBeenCalledWith("order", "node");
});

test("clicking an association row repoints the dialog at that edge", async () => {
  const onSelectElement = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" }, onSelectElement }),
  });
  await fireEvent.click(screen.getByRole("button", { name: /Order/ }));
  expect(onSelectElement).toHaveBeenCalledWith("e1", "edge");
});

test("editing the title in the element body calls onUpdateNode with the node key", async () => {
  const onUpdateNode = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" }, onUpdateNode }),
  });
  await fireEvent.input(screen.getByLabelText("Title"), { target: { value: "Client" } });
  expect(onUpdateNode).toHaveBeenCalledWith(
    "customer",
    expect.objectContaining({ concept: expect.objectContaining({ title: "Client" }) }),
  );
});

test("element state with an unknown key closes and renders nothing", () => {
  const onClose = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "ghost" }, onClose }),
  });
  expect(onClose).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("diagram state mounts the display controls titled 'Diagram properties'", () => {
  render(CentralEditPanelHost, { props: props({ state: { kind: "diagram" } }) });
  expect(screen.getByRole("combobox", { name: "Select element" }).textContent).toContain("Orders");
  expect(screen.getByRole("switch", { name: "Show attributes" })).toBeTruthy();
  // Same full-height chrome as the element/edge edit panels.
  expect(screen.getByRole("dialog", { name: "Diagram properties" }).className).toContain("h-[95vh]");
});

test("toggling a display control in the diagram body calls onDisplayChange", async () => {
  const onDisplayChange = vi.fn();
  render(CentralEditPanelHost, {
    props: props({
      state: { kind: "diagram" },
      display: { ...DEFAULT_DISPLAY, showAttributes: true },
      onDisplayChange,
    }),
  });
  await fireEvent.click(screen.getByRole("switch", { name: "Show attributes" }));
  expect(onDisplayChange).toHaveBeenCalledWith({ showAttributes: false });
});

test("edge state mounts the RelationshipInspector titled Relationship", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "e1" } }),
  });
  expect(screen.getByRole("combobox", { name: "Select element" }).textContent).toContain("Customer → Order");
  // RelationshipInspector's Kind control is present inside the host.
  expect(screen.getByLabelText("Kind")).toBeTruthy();
});

test("editing an edge calls onUpdateEdge with the edge id", async () => {
  const onUpdateEdge = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "e1" }, onUpdateEdge }),
  });
  await fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "composes" } });
  expect(onUpdateEdge).toHaveBeenCalledWith("e1", { kind: "composes" });
});

test("a since-deleted edge closes the panel", () => {
  const onClose = vi.fn();
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "gone" }, onClose }),
  });
  expect(onClose).toHaveBeenCalled();
});

test("showPreview renders the transparent cutout for an element", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" }, showPreview: true }),
  });
  expect(screen.getByTestId("central-preview")).toBeTruthy();
});

test("showPreview renders the transparent cutout for an edge", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "edge", edgeKey: "e1" }, showPreview: true }),
  });
  expect(screen.getByTestId("central-preview")).toBeTruthy();
});
