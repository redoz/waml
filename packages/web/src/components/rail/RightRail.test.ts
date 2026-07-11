import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import RightRail from "./RightRail.svelte";

describe("RightRail", () => {
  it("renders only the Inspect entry (Share moved to the top bar)", () => {
    render(RightRail, { props: { active: null, onOpen: () => {} } });
    expect(screen.getByRole("button", { name: "Inspect" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Share" })).toBeNull();
  });

  it("calls onOpen with the clicked panel id", async () => {
    const onOpen = vi.fn();
    render(RightRail, { props: { active: null, onOpen } });
    await fireEvent.click(screen.getByRole("button", { name: "Inspect" }));
    expect(onOpen).toHaveBeenCalledWith("inspect");
  });

  it("marks the active entry with aria-current", () => {
    render(RightRail, { props: { active: "inspect", onOpen: () => {} } });
    expect(screen.getByRole("button", { name: "Inspect" }).getAttribute("aria-current")).toBe("true");
  });
});
