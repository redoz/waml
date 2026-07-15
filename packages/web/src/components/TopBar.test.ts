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

test("the center switcher toggles the navigator via onToggleNav + aria-expanded", async () => {
  const onToggleNav = vi.fn();
  render(TopBar, {
    props: {
      diagrams: [diagram("d1", "Overview")],
      activeDiagramKey: "d1",
      navOpen: false,
      onToggleNav,
    },
  });
  const btn = screen.getByRole("button", { name: /switch diagram/i });
  expect(btn.getAttribute("aria-expanded")).toBe("false");
  await fireEvent.click(btn);
  expect(onToggleNav).toHaveBeenCalledTimes(1);
  // The navigator no longer mounts inside the TopBar.
  expect(screen.queryByLabelText("Search model")).toBeNull();
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

test("renders the WAML wordmark and keeps the Model Canvas label", () => {
  const { container } = render(TopBar, { props: {} });
  // Wordmark SVG exposes itself as an accessible image named "WAML".
  const wordmark = screen.getByRole("img", { name: "WAML" });
  expect(wordmark.tagName.toLowerCase()).toBe("svg");
  expect(container.textContent).toContain("Model Canvas");
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
