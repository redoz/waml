import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import App from "./App.svelte";

test("renders the blank SvelteFlow canvas shell", () => {
  const { container } = render(App);
  expect(container.querySelector(".svelte-flow")).not.toBeNull();
});
