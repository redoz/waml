import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import WelcomeDialog from "./WelcomeDialog.svelte";

test("Start blank and Import OKF fire their handlers", async () => {
  const onStartBlank = vi.fn();
  const onImport = vi.fn();
  render(WelcomeDialog, { props: { onUseTemplate: vi.fn(), onStartBlank, onImport } });
  await fireEvent.click(screen.getByRole("button", { name: /Start blank/ }));
  expect(onStartBlank).toHaveBeenCalledTimes(1);
  await fireEvent.click(screen.getByRole("button", { name: /Import OKF/ }));
  expect(onImport).toHaveBeenCalledTimes(1);
});
