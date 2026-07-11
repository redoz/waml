import { test, expect, vi } from "vitest";
import { render } from "@testing-library/svelte";
import ShareToast from "./ShareToast.svelte";

test("renders the message and auto-dismisses after 3500ms", () => {
  vi.useFakeTimers();
  const onClose = vi.fn();
  render(ShareToast, { props: { message: "Link copied", onClose } });

  expect(document.body.textContent).toContain("Link copied");
  expect(onClose).not.toHaveBeenCalled();

  vi.advanceTimersByTime(3499);
  expect(onClose).not.toHaveBeenCalled();
  vi.advanceTimersByTime(1);
  expect(onClose).toHaveBeenCalledTimes(1);

  vi.useRealTimers();
});
