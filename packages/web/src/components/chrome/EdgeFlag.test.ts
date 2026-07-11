import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import EdgeFlag from "./EdgeFlag.svelte";

describe("EdgeFlag", () => {
  it("renders an anchor opening in a new tab when given an href", () => {
    render(EdgeFlag, { props: { label: "Feedback", href: "https://example.com/x" } });
    const link = screen.getByRole("link", { name: "Feedback" });
    expect(link.getAttribute("href")).toBe("https://example.com/x");
    expect(link.getAttribute("target")).toBe("_blank");
    expect(link.getAttribute("rel") ?? "").toContain("noreferrer");
  });

  it("renders a button and fires onClick when given a handler", async () => {
    const onClick = vi.fn();
    render(EdgeFlag, { props: { label: "Inspect", onClick } });
    await fireEvent.click(screen.getByRole("button", { name: "Inspect" }));
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("applies the vertical stacking offset to its inline style", () => {
    render(EdgeFlag, { props: { label: "Inspect", onClick: () => {}, offset: -52 } });
    expect(screen.getByRole("button", { name: "Inspect" }).getAttribute("style") ?? "").toContain("-52px");
  });

  it("reflects the active (pressed) state via aria-pressed", () => {
    render(EdgeFlag, { props: { label: "Inspect", onClick: () => {}, active: true } });
    expect(screen.getByRole("button", { name: "Inspect" }).getAttribute("aria-pressed")).toBe("true");
  });

  it("is keyboard-focusable", () => {
    render(EdgeFlag, { props: { label: "Inspect", onClick: () => {} } });
    const btn = screen.getByRole("button", { name: "Inspect" });
    btn.focus();
    expect(document.activeElement).toBe(btn);
  });
});
