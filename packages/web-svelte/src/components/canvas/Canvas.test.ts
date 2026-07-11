import { test, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Canvas from "./Canvas.svelte";

// End-to-end chrome mount check for the final Task 19 wiring: rendering the
// provider-wrapped Canvas must bring up TopBar + the right rail, and opening the
// Share rail entry must reveal the SharePanel hosted inside the ModelSheet.
test("mounts TopBar + RightRail; opening Share reveals the share panel", async () => {
  render(Canvas);
  expect(screen.getByRole("button", { name: /Templates/ })).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: /^Share$/ }));
  expect(screen.getByLabelText("Share URL")).toBeTruthy();
});
