import { test, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { DEFAULT_DISPLAY, type DiagramDisplay } from "@uaml/okf";
import Dock from "./Dock.svelte";
import { hints } from "../../state/hints.svelte";

const baseProps = (display: DiagramDisplay, onDisplayChange = vi.fn()) => ({
  activeTool: "select" as const,
  onToolChange: vi.fn(),
  onClear: vi.fn(),
  clearDisabled: false,
  display,
  onDisplayChange,
});

async function openPanel() {
  await fireEvent.click(screen.getByRole("button", { name: "Diagram properties" }));
}

beforeEach(() => {
  localStorage.clear();
  hints.show = false;
  document.documentElement.removeAttribute("data-show-shortcuts");
});

test("the ERD toggle is gone; a Diagram properties button opens a left-anchored flyout", async () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  // No ERD view toggle anymore.
  expect(screen.queryByRole("button", { name: /ERD view/i })).toBeNull();
  // Panel is closed until the properties button is clicked.
  expect(screen.queryByRole("dialog", { name: "Diagram properties" })).toBeNull();
  await openPanel();
  expect(screen.getByRole("dialog", { name: "Diagram properties" })).toBeTruthy();
});

test("panel renders the active diagram's display values", async () => {
  const display: DiagramDisplay = {
    showAttributes: true,
    attributeDetail: "name-only",
    associationLabels: "hidden",
    emphasizeMultiplicity: true,
    showStereotype: false,
  };
  render(Dock, { props: baseProps(display) });
  await openPanel();
  expect((screen.getByRole("switch", { name: "Show attributes" }) as HTMLElement).getAttribute("aria-checked")).toBe("true");
  expect((screen.getByRole("radio", { name: "Name only" }) as HTMLElement).getAttribute("aria-checked")).toBe("true");
  expect((screen.getByRole("radio", { name: "Hide labels" }) as HTMLElement).getAttribute("aria-checked")).toBe("true");
  expect((screen.getByRole("switch", { name: "Emphasize multiplicity" }) as HTMLElement).getAttribute("aria-checked")).toBe("true");
  expect((screen.getByRole("switch", { name: "Show stereotype" }) as HTMLElement).getAttribute("aria-checked")).toBe("false");
});

test("toggling Show attributes calls onDisplayChange with the flipped value", async () => {
  const onDisplayChange = vi.fn();
  render(Dock, { props: baseProps({ ...DEFAULT_DISPLAY, showAttributes: true }, onDisplayChange) });
  await openPanel();
  await fireEvent.click(screen.getByRole("switch", { name: "Show attributes" }));
  expect(onDisplayChange).toHaveBeenCalledWith({ showAttributes: false });
});

test("choosing an Associations option calls onDisplayChange", async () => {
  const onDisplayChange = vi.fn();
  render(Dock, { props: baseProps({ ...DEFAULT_DISPLAY, associationLabels: "all" }, onDisplayChange) });
  await openPanel();
  await fireEvent.click(screen.getByRole("radio", { name: "Hide labels" }));
  expect(onDisplayChange).toHaveBeenCalledWith({ associationLabels: "hidden" });
});

test("Attribute detail is disabled when Show attributes is off", async () => {
  const onDisplayChange = vi.fn();
  render(Dock, { props: baseProps({ ...DEFAULT_DISPLAY, showAttributes: false }, onDisplayChange) });
  await openPanel();
  const nameType = screen.getByRole("radio", { name: "Name + type" }) as HTMLButtonElement;
  expect(nameType.disabled).toBe(true);
  await fireEvent.click(nameType);
  expect(onDisplayChange).not.toHaveBeenCalled();
});

test("the shortcuts toggle button flips hints.show, aria-pressed, and the root attribute", async () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  const btn = screen.getByRole("button", { name: "Show keyboard shortcuts" });
  expect(btn.getAttribute("aria-pressed")).toBe("false");
  expect(document.documentElement.hasAttribute("data-show-shortcuts")).toBe(false);

  await fireEvent.click(btn);
  expect(hints.show).toBe(true);
  expect(btn.getAttribute("aria-pressed")).toBe("true");
  expect(document.documentElement.hasAttribute("data-show-shortcuts")).toBe(true);
});

test("pressing ? toggles the hints; ? while typing in an input is ignored", async () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  await fireEvent.keyDown(window, { key: "?" });
  expect(hints.show).toBe(true);

  // Typing ? inside an input must NOT toggle.
  const input = document.createElement("input");
  document.body.appendChild(input);
  await fireEvent.keyDown(input, { key: "?" });
  expect(hints.show).toBe(true); // unchanged
  input.remove();
});

test("tool buttons render their key-hint glyph", () => {
  render(Dock, { props: baseProps(DEFAULT_DISPLAY) });
  // V / N / C glyphs are present in the DOM (hidden via CSS, but rendered).
  const glyphs = Array.from(document.querySelectorAll("kbd")).map((k) => k.textContent);
  expect(glyphs).toEqual(expect.arrayContaining(["V", "N", "C"]));
});
