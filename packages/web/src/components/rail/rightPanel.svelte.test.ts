import { test, expect } from "vitest";
import { createRightPanel } from "./rightPanel.svelte";

test("open sets active; close clears it", () => {
  const panel = createRightPanel();
  expect(panel.active).toBe(null);
  panel.open("share");
  expect(panel.active).toBe("share");
  panel.open("inspect");
  expect(panel.active).toBe("inspect");
  panel.close();
  expect(panel.active).toBe(null);
});
