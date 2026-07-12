import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import CentralEditPanelHost from "./CentralEditPanelHost.svelte";
import { DEFAULT_DISPLAY, type ModelNode } from "@uaml/okf";

const node = (key: string, title: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title, description: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as unknown as ModelNode;

const props = (over = {}) => ({
  state: null,
  nodes: [node("customer", "Customer")],
  display: { ...DEFAULT_DISPLAY },
  profileName: "uml-domain",
  onUpdateNode: vi.fn(),
  onDisplayChange: vi.fn(),
  onClose: vi.fn(),
  ...over,
});

test("null state renders nothing", () => {
  render(CentralEditPanelHost, { props: props({ state: null }) });
  expect(screen.queryByRole("dialog")).toBeNull();
});

test("element state mounts ObjectInspector titled by the node", () => {
  render(CentralEditPanelHost, {
    props: props({ state: { kind: "element", nodeKey: "customer" } }),
  });
  expect(screen.getByRole("heading", { name: "Customer" })).toBeTruthy();
  // ObjectInspector's Title field is present inside the host.
  expect(screen.getByLabelText("Title")).toBeTruthy();
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
  expect(screen.getByRole("heading", { name: "Diagram properties" })).toBeTruthy();
  expect(screen.getByRole("switch", { name: "Show attributes" })).toBeTruthy();
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
