import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import TemplateApplyDialog from "./TemplateApplyDialog.svelte";

const graph = { nodes: [{}, {}], edges: [{}] } as any;

test("Apply confirms with the default replace mode", async () => {
  const onConfirm = vi.fn();
  render(TemplateApplyDialog, { props: { graph, name: "Retail", onConfirm, onClose: vi.fn() } });
  expect(document.body.textContent).toContain("Will import 2 marts, 1 relationships.");
  await fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(onConfirm).toHaveBeenCalledWith("replace");
});
