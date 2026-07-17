import { test, expect } from "vitest";
import { render } from "@testing-library/svelte";
import GroupFrame from "./GroupFrame.svelte";

test("renders a titled, sized frame hull", () => {
  const { container, getByText } = render(GroupFrame, {
    props: { data: { title: "Users", width: 232, height: 212 } },
  });
  expect(getByText("Users")).toBeTruthy();
  const root = container.querySelector("[data-group-frame]") as HTMLElement;
  expect(root).toBeTruthy();
  expect(root.getAttribute("style") ?? "").toContain("width: 232px");
  expect(root.getAttribute("style") ?? "").toContain("height: 212px");
});

test("renders no title when the group is untitled", () => {
  const { container } = render(GroupFrame, {
    props: { data: { title: undefined, width: 100, height: 100 } },
  });
  expect(container.querySelector("[data-group-frame-title]")).toBeNull();
});
