import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { tick } from "svelte";
import InspectorPanel from "./InspectorPanel.svelte";

function setup(props: Record<string, unknown> = {}) {
  return render(InspectorPanel, {
    props: {
      open: true,
      pinned: false,
      title: "Inspector",
      onTogglePin: vi.fn(),
      onClose: vi.fn(),
      ...props,
    },
  });
}

describe("InspectorPanel", () => {
  it("renders nothing when closed", () => {
    setup({ open: false });
    expect(screen.queryByRole("complementary", { name: "Inspector" })).toBeNull();
  });

  it("shows the title and fires onClose", async () => {
    const onClose = vi.fn();
    setup({ onClose });
    expect(screen.getByRole("complementary", { name: "Inspector" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Close inspector" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("fires onTogglePin when the pin control is clicked", async () => {
    const onTogglePin = vi.fn();
    setup({ onTogglePin });
    await fireEvent.click(screen.getByRole("button", { name: /pin/i }));
    expect(onTogglePin).toHaveBeenCalledTimes(1);
  });

  it("is opaque (not translucent) when unpinned", () => {
    setup({ pinned: false });
    expect(screen.getByRole("complementary").classList.contains("opacity-40")).toBe(false);
  });

  it("is translucent when pinned and idle (no hover / no focus)", () => {
    setup({ pinned: true });
    expect(screen.getByRole("complementary").classList.contains("opacity-40")).toBe(true);
  });

  it("becomes opaque on hover and translucent again a short delay after the pointer leaves", async () => {
    setup({ pinned: true, hideDelay: 20 });
    const aside = screen.getByRole("complementary");
    expect(aside.classList.contains("opacity-40")).toBe(true);

    await fireEvent.pointerEnter(aside);
    expect(aside.classList.contains("opacity-40")).toBe(false);

    await fireEvent.pointerLeave(aside);
    // Stays opaque immediately after leaving — the delay avoids flicker.
    expect(aside.classList.contains("opacity-40")).toBe(false);

    await new Promise((r) => setTimeout(r, 40));
    await tick();
    expect(aside.classList.contains("opacity-40")).toBe(true);
  });

  it("becomes opaque while focused within the panel", async () => {
    setup({ pinned: true });
    const aside = screen.getByRole("complementary");
    expect(aside.classList.contains("opacity-40")).toBe(true);
    await fireEvent.focusIn(aside);
    expect(aside.classList.contains("opacity-40")).toBe(false);
  });
});
