import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { EnablePanel } from "./EnablePanel";

describe("EnablePanel", () => {
  it("shows the intro copy and both legal links", () => {
    render(<EnablePanel onGoogle={()=>{}} onGitHub={()=>{}} onEmail={()=>{}} />);
    expect(screen.getByText(/we'll occasionally email you about data-modeling topics/i)).toBeTruthy();
    expect(screen.getByRole("link", { name: "Terms of Service" }).getAttribute("href"))
      .toBe("https://www.owox.com/policies/terms-of-service");
    expect(screen.getByRole("link", { name: "Privacy Policy" }).getAttribute("href"))
      .toBe("https://www.owox.com/policies/privacy");
  });
  it("does NOT list named sharing as a perk", () => {
    render(<EnablePanel onGoogle={()=>{}} onGitHub={()=>{}} onEmail={()=>{}} />);
    expect(screen.queryByText(/named sharing/i)).toBeNull();
  });
  it("submits the typed email", () => {
    const onEmail = vi.fn();
    render(<EnablePanel onGoogle={()=>{}} onGitHub={()=>{}} onEmail={onEmail} />);
    fireEvent.change(screen.getByPlaceholderText("you@company.com"), { target: { value: "a@b.co" } });
    fireEvent.click(screen.getByRole("button", { name: /send link/i }));
    expect(onEmail).toHaveBeenCalledWith("a@b.co");
  });
  it("shows a confirmation with the address once the link is sent", async () => {
    const onEmail = vi.fn().mockResolvedValue(undefined);
    render(<EnablePanel onGoogle={()=>{}} onGitHub={()=>{}} onEmail={onEmail} />);
    fireEvent.change(screen.getByPlaceholderText("you@company.com"), { target: { value: "a@b.co" } });
    fireEvent.click(screen.getByRole("button", { name: /send link/i }));
    await waitFor(() => expect(screen.getByText(/check your email/i)).toBeTruthy());
    expect(screen.getByText("a@b.co")).toBeTruthy();
    expect(screen.getByRole("button", { name: /resend the link/i })).toBeTruthy();
  });
  it("surfaces an error when sending fails", async () => {
    const onEmail = vi.fn().mockRejectedValue(new Error("rate limit exceeded"));
    render(<EnablePanel onGoogle={()=>{}} onGitHub={()=>{}} onEmail={onEmail} />);
    fireEvent.change(screen.getByPlaceholderText("you@company.com"), { target: { value: "a@b.co" } });
    fireEvent.click(screen.getByRole("button", { name: /send link/i }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toMatch(/rate limit exceeded/i));
  });
});
