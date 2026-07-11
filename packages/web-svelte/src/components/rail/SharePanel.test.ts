import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import SharePanel from "./SharePanel.svelte";

test("shows the url and wires Copy + Export", async () => {
  const onCopy = vi.fn();
  const onExportImage = vi.fn();
  render(SharePanel, { props: { shareUrl: "https://x/#m=1", onCopy, onExportImage } });
  expect((screen.getByLabelText("Share URL") as HTMLInputElement).value).toBe("https://x/#m=1");
  await fireEvent.click(screen.getByRole("button", { name: "Copy" }));
  expect(onCopy).toHaveBeenCalledTimes(1);
  await fireEvent.click(screen.getByRole("button", { name: /Export as image/ }));
  expect(onExportImage).toHaveBeenCalledTimes(1);
});
