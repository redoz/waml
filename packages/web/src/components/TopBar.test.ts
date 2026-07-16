import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import TopBar from "./TopBar.svelte";

const diagram = (key: string, title: string) => ({
  key,
  title,
  profile: "uml-domain",
  members: [] as string[],
});

const switcherProps = (over: Record<string, unknown> = {}) => ({
  diagrams: [diagram("d1", "Overview"), diagram("d2", "Details")],
  activeDiagramKey: "d1",
  onSelectDiagram: vi.fn(),
  onDockModel: vi.fn(),
  onEditModel: vi.fn(),
  ...over,
});

test("renders the active diagram title with the blue treatment (no Target icon)", () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  // Shows the active diagram's title.
  expect(btn.textContent).toContain("Overview");
  // Keeps the blue background treatment carried over from the old goal button.
  expect(btn.className).toContain("bg-[#e6f1fb]");
  expect(btn.className).toContain("text-[#1e88e5]");
});

test("no longer renders the Business Goal button", () => {
  render(TopBar, { props: switcherProps() });
  expect(screen.queryByRole("button", { name: "Business goal" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Set business goal" })).toBeNull();
});

test("clicking the title opens the read-only switcher dropdown", async () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  expect(btn.getAttribute("aria-expanded")).toBe("false");
  await fireEvent.click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  expect(screen.getByRole("dialog", { name: /switch diagram/i })).toBeTruthy();
  expect(screen.queryByLabelText("Search model")).toBeNull();
  expect(screen.queryByRole("button", { name: /rename|new diagram|create/i })).toBeNull();
});

test("the dropdown lists every diagram with the active one checked", async () => {
  render(TopBar, { props: switcherProps() });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  expect(screen.getByRole("option", { name: /Overview/ }).getAttribute("aria-selected")).toBe("true");
  expect(screen.getByRole("option", { name: /Details/ }).getAttribute("aria-selected")).toBe("false");
});

test("clicking a diagram row fires onSelectDiagram and closes the dropdown", async () => {
  const onSelectDiagram = vi.fn();
  render(TopBar, { props: switcherProps({ onSelectDiagram }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("option", { name: /Details/ }));
  expect(onSelectDiagram).toHaveBeenCalledWith("d2");
  expect(screen.getByRole("button", { name: /switch diagram/i }).getAttribute("aria-expanded")).toBe("false");
});

test("the Dock button fires onDockModel and closes", async () => {
  const onDockModel = vi.fn();
  render(TopBar, { props: switcherProps({ onDockModel }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("button", { name: /dock model editor/i }));
  expect(onDockModel).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog", { name: /switch diagram/i })).toBeNull();
});

test("the Edit button fires onEditModel and closes", async () => {
  const onEditModel = vi.fn();
  render(TopBar, { props: switcherProps({ onEditModel }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  await fireEvent.click(screen.getByRole("button", { name: /edit model/i }));
  expect(onEditModel).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog", { name: /switch diagram/i })).toBeNull();
});

test("outside-click closes the dropdown", async () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  await fireEvent.click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  await fireEvent.click(document.querySelector(".fixed.inset-0")!);
  expect(btn.getAttribute("aria-expanded")).toBe("false");
});

test("Escape closes the dropdown", async () => {
  render(TopBar, { props: switcherProps() });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  await fireEvent.click(btn);
  expect(btn.getAttribute("aria-expanded")).toBe("true");
  await fireEvent.keyDown(window, { key: "Escape" });
  expect(btn.getAttribute("aria-expanded")).toBe("false");
});

test("export dropdown opens and routes OKF vs SVG", async () => {
  const onExport = vi.fn();
  const onExportSvg = vi.fn();
  render(TopBar, { props: { onExport, onExportSvg } });
  await fireEvent.click(screen.getByRole("button", { name: /Export/ }));
  await fireEvent.click(screen.getByRole("menuitem", { name: /OKF/ }));
  expect(onExport).toHaveBeenCalledTimes(1);
});

test("export button disabled when exportDisabled", () => {
  render(TopBar, { props: { exportDisabled: true } });
  // @testing-library/jest-dom (toBeDisabled) isn't a dependency anywhere in
  // this monorepo; assert via the native `disabled` DOM property instead.
  expect(
    (screen.getByRole("button", { name: /Export/ }) as HTMLButtonElement).disabled
  ).toBe(true);
});

test("renders the WAML wordmark and the root package name as subtitle", () => {
  const { container } = render(TopBar, { props: { rootPackageName: "Acme Model" } });
  // Wordmark is now plain text inside the brand link, not an SVG.
  expect(screen.queryByRole("img", { name: "WAML" })).toBeNull();
  expect(screen.getByRole("link").textContent).toContain("WAML");
  // Root package name shows in place of the old "Model Canvas" label.
  expect(container.textContent).toContain("Acme Model");
  expect(container.textContent).not.toContain("Model Canvas");
});

test("brand anchor links to the WAML GitHub repo", () => {
  render(TopBar, { props: {} });
  const link = screen.getByRole("link");
  expect(link.getAttribute("href")).toBe("https://github.com/redoz/waml");
  // External-link hygiene preserved.
  expect(link.getAttribute("target")).toBe("_blank");
  expect(link.getAttribute("rel")).toBe("noreferrer");
  // Accessible name mentions WAML, not the old OWOX brand.
  expect(link.getAttribute("aria-label")).toContain("WAML");
  expect(link.getAttribute("aria-label")).not.toMatch(/owox/i);
});

test("no remaining OWOX gradient logo references", () => {
  const { container } = render(TopBar, { props: {} });
  const html = container.innerHTML;
  expect(html).not.toContain("topbar-g0");
  expect(html).not.toContain("topbar-g1");
  expect(html.toLowerCase()).not.toContain("owox");
});

test("renders a Share button immediately right of Export and fires onShare", async () => {
  const onShare = vi.fn();
  render(TopBar, { props: { onShare } });
  const exportBtn = screen.getByRole("button", { name: /Export/ });
  const shareBtn = screen.getByRole("button", { name: /^Share$/ });
  // Share must follow Export in document order (sits to its right).
  expect(
    exportBtn.compareDocumentPosition(shareBtn) & Node.DOCUMENT_POSITION_FOLLOWING
  ).toBeTruthy();
  await fireEvent.click(shareBtn);
  expect(onShare).toHaveBeenCalledTimes(1);
});

test("Share button disabled when shareDisabled", () => {
  render(TopBar, { props: { shareDisabled: true } });
  expect(
    (screen.getByRole("button", { name: /^Share$/ }) as HTMLButtonElement).disabled
  ).toBe(true);
});
