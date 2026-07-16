import { test, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Dock from "./Dock.svelte";
import { hints } from "../../state/hints.svelte";

const baseProps = (onOpenProperties = vi.fn()) => ({
  activeTool: "select" as const,
  onToolChange: vi.fn(),
  onClear: vi.fn(),
  clearDisabled: false,
  onOpenProperties,
});

beforeEach(() => {
  localStorage.clear();
  hints.show = false;
  document.documentElement.removeAttribute("data-show-shortcuts");
});

test("the ERD toggle is gone; the Diagram properties button has no inline popover", async () => {
  render(Dock, { props: baseProps() });
  // No ERD view toggle anymore.
  expect(screen.queryByRole("button", { name: /ERD view/i })).toBeNull();
  const btn = screen.getByRole("button", { name: "Diagram properties" });
  // The old inline flyout is gone: no dialog before or after clicking the button.
  expect(screen.queryByRole("dialog", { name: "Diagram properties" })).toBeNull();
  await fireEvent.click(btn);
  expect(screen.queryByRole("dialog", { name: "Diagram properties" })).toBeNull();
});

test("clicking the Diagram properties button fires onOpenProperties", async () => {
  const onOpenProperties = vi.fn();
  render(Dock, { props: baseProps(onOpenProperties) });
  await fireEvent.click(screen.getByRole("button", { name: "Diagram properties" }));
  expect(onOpenProperties).toHaveBeenCalledTimes(1);
});

test("the shortcuts toggle button flips hints.show, aria-pressed, and the root attribute", async () => {
  render(Dock, { props: baseProps() });
  const btn = screen.getByRole("button", { name: "Show keyboard shortcuts" });
  expect(btn.getAttribute("aria-pressed")).toBe("false");
  expect(document.documentElement.hasAttribute("data-show-shortcuts")).toBe(false);

  await fireEvent.click(btn);
  expect(hints.show).toBe(true);
  expect(btn.getAttribute("aria-pressed")).toBe("true");
  expect(document.documentElement.hasAttribute("data-show-shortcuts")).toBe(true);
});

test("pressing ? toggles the hints; ? while typing in an input is ignored", async () => {
  render(Dock, { props: baseProps() });
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
  render(Dock, { props: baseProps() });
  // V / N / C glyphs are present in the DOM (hidden via CSS, but rendered).
  const glyphs = Array.from(document.querySelectorAll("kbd")).map((k) => k.textContent);
  expect(glyphs).toEqual(expect.arrayContaining(["V", "N", "C"]));
});

test("defaults to a 14px left offset", () => {
  const { container } = render(Dock, { props: baseProps() });
  const dock = container.querySelector("[data-dock]") as HTMLElement;
  expect(dock.style.left).toBe("14px");
});

test("leftOffset slides the dock clear of the docked rail", () => {
  const { container } = render(Dock, { props: { ...baseProps(), leftOffset: 352 } });
  const dock = container.querySelector("[data-dock]") as HTMLElement;
  expect(dock.style.left).toBe("352px");
});
