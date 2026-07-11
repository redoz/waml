import { test, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import Canvas from "./Canvas.svelte";

// End-to-end chrome mount check: rendering the provider-wrapped Canvas brings up
// the TopBar, and clicking the first-class top-bar Share button opens the modal
// Share dialog (Share no longer lives in the right rail).
test("mounts the TopBar; clicking top-bar Share opens the Share dialog", async () => {
  render(Canvas);
  expect(screen.getByRole("button", { name: /Templates/ })).toBeTruthy();
  await fireEvent.click(screen.getByRole("button", { name: /^Share$/ }));
  expect(screen.getByLabelText("Share URL")).toBeTruthy();
});
