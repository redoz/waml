import { test, expect, beforeAll, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import { initWasm } from "@uaml/okf";
import TemplateApplyDialog from "./TemplateApplyDialog.svelte";

beforeAll(async () => {
  await initWasm();
});

// A two-class, one-association bundle → 2 nodes, 1 relationship (derived by the
// WASM core for the dialog's preview counts).
const bundle: [string, string][] = [
  [
    "m/a.md",
    "---\ntype: uml.Class\ntitle: A\n---\n\n# A\n\n## Relationships\n- associates [B](./b.md): 1 to 1\n",
  ],
  ["m/b.md", "---\ntype: uml.Class\ntitle: B\n---\n\n# B\n"],
];

test("Apply confirms with the default replace mode", async () => {
  const onConfirm = vi.fn();
  render(TemplateApplyDialog, { props: { bundle, name: "Retail", onConfirm, onClose: vi.fn() } });
  expect(document.body.textContent).toContain("Will import 2 nodes, 1 relationships.");
  await fireEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(onConfirm).toHaveBeenCalledWith("replace");
});
