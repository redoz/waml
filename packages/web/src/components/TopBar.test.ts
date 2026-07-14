import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import TopBar from "./TopBar.svelte";
import type { ModelGraph } from "@waml/okf";

const diagram = (key: string, title: string) => ({
  key,
  title,
  profile: "uml-domain",
  members: [] as string[],
});

// A minimal graph for the mounted Navigator sheet — packages carry the
// concept-Node shape (title on concept.title), matching the real model.
const navGraph = {
  path: "acme-model",
  nodes: [],
  edges: [],
  diagrams: [{ key: "d1", title: "Overview", profile: "uml-domain", members: [] }],
  packages: [
    {
      key: "",
      type: "uml.Package",
      concept: { id: "", type: "uml.Package", title: "", body: "" },
      stereotypes: [],
      attributes: [],
      position: { x: 0, y: 0 },
      members: ["d1"],
    },
  ],
} as unknown as ModelGraph;

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

test("the center switcher opens the Navigator sheet (not the old inline list)", async () => {
  render(TopBar, {
    props: {
      diagrams: [diagram("d1", "Overview")],
      activeDiagramKey: "d1",
      graph: navGraph,
      palette: ["uml.Class"],
      onSelectDiagram: vi.fn(),
      onScope: vi.fn(),
    },
  });
  await fireEvent.click(screen.getByRole("button", { name: /switch diagram/i }));
  // Navigator's search field is the tell that the new sheet mounted.
  expect(screen.getByLabelText("Search model")).toBeTruthy();
  expect(screen.getByText("acme-model")).toBeTruthy();
  // The old inline diagram-list radios are gone.
  expect(screen.queryByRole("menuitemradio")).toBeNull();
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
