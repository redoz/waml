import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import Harness from "./RowsCompartmentHarness.svelte";

test("max caps visible rows and shows a static '+K more' with no button", () => {
  const { container } = render(Harness, { props: { rows: 8, max: 3 } });
  expect(container.querySelectorAll("[data-row]")).toHaveLength(3);
  expect(container.textContent).toContain("+5 more");
  expect(container.querySelector("button")).toBeNull();
});

test("max larger than the row count shows all rows and no footer", () => {
  const { container } = render(Harness, { props: { rows: 2, max: 10 } });
  expect(container.querySelectorAll("[data-row]")).toHaveLength(2);
  expect(container.textContent).not.toContain("more");
});

test("without max, the interactive expand button is still present when rows overflow", () => {
  const { container } = render(Harness, { props: { rows: 20 } });
  expect(container.querySelector("button")).not.toBeNull();
});
