import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { TopBar } from "./TopBar";

describe("TopBar", () => {
  it("renders a Business Goal button and fires onOpenGoal", () => {
    const onOpenGoal = vi.fn();
    render(<TopBar onOpenGoal={onOpenGoal} />);
    fireEvent.click(screen.getByRole("button", { name: /business goal/i }));
    expect(onOpenGoal).toHaveBeenCalled();
  });

  it("shows no sign-in / account controls (local-only app)", () => {
    render(<TopBar />);
    expect(screen.queryByText("Sign in")).toBeNull();
    expect(screen.queryByText("Sign out")).toBeNull();
    expect(screen.queryByRole("combobox")).toBeNull(); // no storage picker
  });
});
