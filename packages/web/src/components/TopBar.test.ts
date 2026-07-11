import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import TopBar from "./TopBar.svelte";

test("goal button label reflects goalSet and fires onOpenGoal", async () => {
  const onOpenGoal = vi.fn();
  render(TopBar, { props: { goalSet: false, onOpenGoal } });
  const btn = screen.getByRole("button", { name: "Business goal" });
  expect(btn.textContent).toContain("Set business goal");
  await fireEvent.click(btn);
  expect(onOpenGoal).toHaveBeenCalledTimes(1);
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
