import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import KeyHint from "./KeyHint.svelte";

test("renders one kbd per key with the glyph text", () => {
  const { container } = render(KeyHint, { props: { keys: ["V"] } });
  const kbds = container.querySelectorAll("kbd");
  expect(kbds.length).toBe(1);
  expect(kbds[0].textContent).toBe("V");
});

test("wrapper carries the keyhint class (hidden by default via opacity-0)", () => {
  const { container } = render(KeyHint, { props: { keys: ["⌫"] } });
  const span = container.querySelector("span.keyhint");
  expect(span).not.toBeNull();
  expect(span!.classList.contains("opacity-0")).toBe(true);
});
