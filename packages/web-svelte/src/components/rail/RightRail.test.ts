import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import RightRail from "./RightRail.svelte";

describe("RightRail", () => {
  it("renders the Inspect and Share entries", () => {
    render(RightRail, { props: { active: null, onOpen: () => {} } });
    ["Inspect", "Share"].forEach(l =>
      expect(screen.getByRole("button", { name: l })).toBeTruthy());
  });

  it("calls onOpen with the clicked panel id", async () => {
    const onOpen = vi.fn();
    render(RightRail, { props: { active: null, onOpen } });
    await fireEvent.click(screen.getByRole("button", { name: "Share" }));
    expect(onOpen).toHaveBeenCalledWith("share");
  });

  it("marks the active entry with aria-current", () => {
    render(RightRail, { props: { active: "inspect", onOpen: () => {} } });
    expect(screen.getByRole("button", { name: "Inspect" }).getAttribute("aria-current")).toBe("true");
    expect(screen.getByRole("button", { name: "Share" }).getAttribute("aria-current")).toBeNull();
  });
});
