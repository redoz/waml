import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ModelSheet from "./ModelSheet.svelte";

describe("ModelSheet", () => {
  it("renders nothing when active is null", () => {
    const { container } = render(ModelSheet, {
      props: { active: null, title: "Inspect", onClose: vi.fn() },
    });
    expect(container.querySelector('[role="dialog"]')).toBeNull();
  });

  it("shows the title and fires onClose", async () => {
    const onClose = vi.fn();
    render(ModelSheet, { props: { active: "inspect", title: "Inspect", onClose } });
    expect(screen.getByRole("dialog", { name: "Inspect" })).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("does NOT render the dimming overlay for a non-modal panel (inspect)", () => {
    const { container } = render(ModelSheet, {
      props: { active: "inspect", modal: false, title: "Inspect", onClose: vi.fn() },
    });
    expect(container.querySelector(".bg-black\\/50")).toBeNull();
  });

  it("DOES render the dimming overlay by default (modal defaults true)", () => {
    const { container } = render(ModelSheet, {
      props: { active: "share", title: "Share model", onClose: vi.fn() },
    });
    expect(container.querySelector(".bg-black\\/50")).not.toBeNull();
  });

  it("overlay click triggers onClose", async () => {
    const onClose = vi.fn();
    const { container } = render(ModelSheet, {
      props: { active: "share", modal: true, title: "Share model", onClose },
    });
    const overlay = container.querySelector(".bg-black\\/50") as HTMLElement;
    await fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
