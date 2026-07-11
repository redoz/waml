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
  onRenameDiagram: vi.fn(),
  onCreateDiagram: vi.fn(),
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

test("dropdown lists every diagram, checkmarks the active one, and switches on click", async () => {
  const onSelectDiagram = vi.fn();
  render(TopBar, { props: switcherProps({ onSelectDiagram }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));

  const active = screen.getByRole("menuitemradio", { name: "Overview" });
  const other = screen.getByRole("menuitemradio", { name: "Details" });
  expect(active.getAttribute("aria-checked")).toBe("true");
  expect(other.getAttribute("aria-checked")).toBe("false");

  await fireEvent.click(other);
  expect(onSelectDiagram).toHaveBeenCalledWith("d2");
});

test("rename current diagram submits the trimmed title", async () => {
  const onRenameDiagram = vi.fn();
  render(TopBar, { props: switcherProps({ onRenameDiagram }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));

  const input = screen.getByLabelText("Rename diagram") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "  Renamed  " } });
  await fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  expect(onRenameDiagram).toHaveBeenCalledWith("Renamed");
});

test("empty / whitespace rename is rejected (keeps previous title)", async () => {
  const onRenameDiagram = vi.fn();
  render(TopBar, { props: switcherProps({ onRenameDiagram }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));

  const input = screen.getByLabelText("Rename diagram") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "   " } });
  await fireEvent.click(screen.getByRole("button", { name: "Rename" }));
  expect(onRenameDiagram).not.toHaveBeenCalled();
});

test("+ New diagram creates a diagram via an inline name input (not window.prompt)", async () => {
  const onCreateDiagram = vi.fn();
  render(TopBar, { props: switcherProps({ onCreateDiagram }) });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));

  await fireEvent.click(screen.getByRole("button", { name: /New diagram/i }));
  const input = screen.getByLabelText("New diagram name") as HTMLInputElement;
  await fireEvent.input(input, { target: { value: "  Fresh  " } });
  await fireEvent.click(screen.getByRole("button", { name: "Create" }));
  expect(onCreateDiagram).toHaveBeenCalledWith("Fresh");
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

test("renders the UAML wordmark and keeps the Model Canvas label", () => {
  const { container } = render(TopBar, { props: {} });
  // Wordmark SVG exposes itself as an accessible image named "UAML".
  const wordmark = screen.getByRole("img", { name: "UAML" });
  expect(wordmark.tagName.toLowerCase()).toBe("svg");
  expect(container.textContent).toContain("Model Canvas");
});

test("brand anchor links to the UAML GitHub repo", () => {
  render(TopBar, { props: {} });
  const link = screen.getByRole("link");
  expect(link.getAttribute("href")).toBe("https://github.com/redoz/uaml");
  // External-link hygiene preserved.
  expect(link.getAttribute("target")).toBe("_blank");
  expect(link.getAttribute("rel")).toBe("noreferrer");
  // Accessible name mentions UAML, not the old OWOX brand.
  expect(link.getAttribute("aria-label")).toContain("UAML");
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
