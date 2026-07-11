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
