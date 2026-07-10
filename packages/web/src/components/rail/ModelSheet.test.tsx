import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { ModelSheet } from "./ModelSheet";

describe("ModelSheet", () => {
  it("renders nothing when active is null", () => {
    const { container } = render(
      <ModelSheet active={null} title="Inspect" onClose={() => {}}>
        <div>content</div>
      </ModelSheet>,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders the panel with header and children when active", () => {
    render(
      <ModelSheet active="inspect" title="Inspect" onClose={() => {}}>
        <div>inspector content</div>
      </ModelSheet>,
    );
    expect(screen.getByRole("dialog", { name: "Inspect" })).toBeTruthy();
    expect(screen.getByText("inspector content")).toBeTruthy();
  });

  it("does NOT render the dimming overlay for the inspect panel (modal=false)", () => {
    const { container } = render(
      <ModelSheet active="inspect" modal={false} title="Inspect" onClose={() => {}}>
        <div>inspector content</div>
      </ModelSheet>,
    );
    // The overlay has bg-black/50 — it must be absent for non-modal inspect
    const overlay = container.querySelector(".bg-black\\/50");
    expect(overlay).toBeNull();
  });

  it("DOES render the dimming overlay for a modal panel (share)", () => {
    const { container } = render(
      <ModelSheet active="share" modal={true} title="Share model" onClose={() => {}}>
        <div>share panel</div>
      </ModelSheet>,
    );
    const overlay = container.querySelector(".bg-black\\/50");
    expect(overlay).not.toBeNull();
  });

  it("overlay click triggers onClose", () => {
    const onClose = vi.fn();
    const { container } = render(
      <ModelSheet active="share" modal={true} title="Share model" onClose={onClose}>
        <div>share panel</div>
      </ModelSheet>,
    );
    const overlay = container.querySelector(".bg-black\\/50") as HTMLElement;
    overlay.click();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("close button triggers onClose", () => {
    const onClose = vi.fn();
    render(
      <ModelSheet active="share" modal={true} title="Share model" onClose={onClose}>
        <div>share panel</div>
      </ModelSheet>,
    );
    screen.getByRole("button", { name: "Close" }).click();
    expect(onClose).toHaveBeenCalledOnce();
  });
});
