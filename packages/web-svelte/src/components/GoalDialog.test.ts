import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { NICHE_PRESETS } from "@mc/core/state/goal";
import GoalDialog from "./GoalDialog.svelte";

test("Apply enables after choosing a niche and typing a goal", async () => {
  const onConfirm = vi.fn();
  render(GoalDialog, { props: { current: null, onConfirm, onClear: vi.fn(), onClose: vi.fn() } });
  const apply = screen.getByRole("button", { name: "Apply" }) as HTMLButtonElement;
  expect(apply.disabled).toBe(true);
  await fireEvent.click(screen.getByRole("button", { name: NICHE_PRESETS[0].label }));
  await fireEvent.input(screen.getByPlaceholderText(/type your own goal/), { target: { value: "Grow revenue" } });
  expect(apply.disabled).toBe(false);
  await fireEvent.click(apply);
  expect(onConfirm).toHaveBeenCalledWith({ niche: NICHE_PRESETS[0].label, goal: "Grow revenue" });
});
