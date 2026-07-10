import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { RightRail } from "./RightRail";

describe("RightRail", () => {
  it("renders the Inspect and Share entries", () => {
    render(<RightRail active={null} onOpen={() => {}} />);
    ["Inspect", "Share"].forEach(l =>
      expect(screen.getByRole("button", { name: l })).toBeTruthy());
  });

  it("calls onOpen with the clicked panel id", () => {
    const onOpen = vi.fn();
    render(<RightRail active={null} onOpen={onOpen} />);
    fireEvent.click(screen.getByRole("button", { name: "Share" }));
    expect(onOpen).toHaveBeenCalledWith("share");
  });

  it("marks the active entry with aria-current", () => {
    render(<RightRail active="inspect" onOpen={() => {}} />);
    expect(screen.getByRole("button", { name: "Inspect" }).getAttribute("aria-current")).toBe("true");
    expect(screen.getByRole("button", { name: "Share" }).getAttribute("aria-current")).toBeNull();
  });
});
