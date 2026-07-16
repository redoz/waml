import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import NavigatorPanel from "./NavigatorPanel.svelte";
import type { ModelGraph } from "@waml/okf";

const node = (key: string, title: string, type = "uml.Class") => ({
  key, type, concept: { id: key, type, title, body: "" },
  stereotypes: [], attributes: [], position: { x: 0, y: 0 },
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
  open: true, mode: "centered" as const, title: "acme-model",
  graph, scopeKey: "sales", activeDiagramKey: "overview", palette: ["uml.Class"],
  onClose: vi.fn(), onToggleMode: vi.fn(), onTogglePin: vi.fn(), onScope: vi.fn(), onSelectDiagram: vi.fn(),
  ...over,
});

test("renders nothing when closed", () => {
  render(NavigatorPanel, { props: props({ open: false }) });
  expect(screen.queryByLabelText("Search model")).toBeNull();
});

test("centered mode mounts the body and a dismissing scrim", () => {
  render(NavigatorPanel, { props: props() });
  expect(screen.getByLabelText("Search model")).toBeTruthy();
  expect(screen.getByRole("dialog", { name: "acme-model" })).toBeTruthy();
});

test("pin button fires onToggleMode", async () => {
  const onToggleMode = vi.fn();
  render(NavigatorPanel, { props: props({ onToggleMode }) });
  await fireEvent.click(screen.getByRole("button", { name: /pin navigator to left|dock/i }));
  expect(onToggleMode).toHaveBeenCalledTimes(1);
});

test("close button fires onClose", async () => {
  const onClose = vi.fn();
  render(NavigatorPanel, { props: props({ onClose }) });
  await fireEvent.click(screen.getByRole("button", { name: /^close$/i }));
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("Escape closes when no input is focused", async () => {
  const onClose = vi.fn();
  render(NavigatorPanel, { props: props({ onClose }) });
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("first Escape blurs a focused input, second closes", async () => {
  const onClose = vi.fn();
  render(NavigatorPanel, { props: props({ onClose }) });
  const input = screen.getByLabelText("Search model") as HTMLInputElement;
  input.focus();
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).not.toHaveBeenCalled();
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(onClose).toHaveBeenCalledTimes(1);
});

test("docked mode exposes a resize handle and a pin toggle (no center toggle)", () => {
  render(NavigatorPanel, { props: props({ mode: "docked" }) });
  expect(screen.getByLabelText("Model navigator")).toBeTruthy();
  expect(screen.getByRole("button", { name: /keep solid|dim when idle/i })).toBeTruthy();
  expect(screen.queryByRole("button", { name: /center/i })).toBeNull();
  expect(screen.getByTestId("nav-resize")).toBeTruthy();
});

test("docked pin toggle fires onTogglePin", async () => {
  const onTogglePin = vi.fn();
  render(NavigatorPanel, { props: props({ mode: "docked", onTogglePin }) });
  await fireEvent.click(screen.getByRole("button", { name: /keep solid|dim when idle/i }));
  expect(onTogglePin).toHaveBeenCalledTimes(1);
});

test("docked translucency tracks pinned (dims when idle unless pinned)", () => {
  const { unmount } = render(NavigatorPanel, { props: props({ mode: "docked", pinned: false }) });
  expect(screen.getByLabelText("Model navigator").classList.contains("opacity-40")).toBe(true);
  unmount();
  render(NavigatorPanel, { props: props({ mode: "docked", pinned: true }) });
  expect(screen.getByLabelText("Model navigator").classList.contains("opacity-40")).toBe(false);
});

test("docked collapse toggle hides the body", async () => {
  render(NavigatorPanel, { props: props({ mode: "docked" }) });
  expect(screen.getByLabelText("Search model")).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: /collapse/i }));
  expect(screen.queryByLabelText("Search model")).toBeNull();
});
