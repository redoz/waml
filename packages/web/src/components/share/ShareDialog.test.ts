import { test, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import ShareDialog from "./ShareDialog.svelte";

const PNG = new Blob([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], { type: "image/png" });

beforeEach(() => {
  // jsdom has no object-URL support; the dialog builds a preview URL from the blob.
  (URL as unknown as { createObjectURL: () => string }).createObjectURL = vi.fn(() => "blob:preview");
  (URL as unknown as { revokeObjectURL: () => void }).revokeObjectURL = vi.fn();
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

function baseProps(over: Record<string, unknown> = {}) {
  return {
    shareUrl: "https://x/#m=1",
    imageName: "my-model",
    canShareImage: true,
    generatePng: async () => PNG,
    onClose: vi.fn(),
    ...over,
  };
}

test("shows the share URL and Copy writes it to the clipboard", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });

  render(ShareDialog, { props: baseProps() });
  expect((screen.getByLabelText("Share URL") as HTMLInputElement).value).toBe("https://x/#m=1");

  await fireEvent.click(screen.getByRole("button", { name: /^Copy$/ }));
  expect(writeText).toHaveBeenCalledWith("https://x/#m=1");
});

test("Share as image renders a preview from generatePng", async () => {
  render(ShareDialog, { props: baseProps() });
  await fireEvent.click(screen.getByRole("button", { name: /Share as image/ }));
  await waitFor(() => expect(screen.getByAltText(/preview/i)).toBeTruthy());
});

test("Copy image is disabled when the Clipboard image API is absent", async () => {
  // navigator.clipboard.write / ClipboardItem unavailable (e.g. Firefox).
  Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn() }, configurable: true });
  vi.stubGlobal("ClipboardItem", undefined);

  render(ShareDialog, { props: baseProps() });
  await fireEvent.click(screen.getByRole("button", { name: /Share as image/ }));
  await waitFor(() => expect(screen.getByAltText(/preview/i)).toBeTruthy());

  const copyImg = screen.getByRole("button", { name: /Copy image/ }) as HTMLButtonElement;
  expect(copyImg.disabled).toBe(true);
  // Save is the fallback and stays available.
  expect((screen.getByRole("button", { name: /Save/ }) as HTMLButtonElement).disabled).toBe(false);
});

test("Save triggers a download named after the diagram", async () => {
  let downloaded: string | null = null;
  vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
    downloaded = this.download;
  });

  render(ShareDialog, { props: baseProps() });
  await fireEvent.click(screen.getByRole("button", { name: /Share as image/ }));
  await waitFor(() => expect(screen.getByAltText(/preview/i)).toBeTruthy());

  await fireEvent.click(screen.getByRole("button", { name: /Save/ }));
  expect(downloaded).toBe("my-model.png");
});

test("Share as image is disabled for an empty diagram", () => {
  render(ShareDialog, { props: baseProps({ canShareImage: false }) });
  expect((screen.getByRole("button", { name: /Share as image/ }) as HTMLButtonElement).disabled).toBe(true);
});

test("Close fires onClose", async () => {
  const onClose = vi.fn();
  render(ShareDialog, { props: baseProps({ onClose }) });
  await fireEvent.click(screen.getByRole("button", { name: "Close" }));
  expect(onClose).toHaveBeenCalledTimes(1);
});
