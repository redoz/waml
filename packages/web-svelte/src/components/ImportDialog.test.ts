import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ImportDialog from "./ImportDialog.svelte";

test("Import is disabled until there is a preview; Cancel closes", async () => {
  const onClose = vi.fn();
  render(ImportDialog, { props: { onConfirm: vi.fn(), onClose } });
  // @testing-library/jest-dom (toBeDisabled) isn't a dependency anywhere in
  // this monorepo; assert via the native `disabled` DOM property instead.
  expect(
    (screen.getByRole("button", { name: "Import" }) as HTMLButtonElement).disabled
  ).toBe(true);
  await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(onClose).toHaveBeenCalledTimes(1);
});
