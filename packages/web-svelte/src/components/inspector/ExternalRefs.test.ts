import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ExternalRefs from "./ExternalRefs.svelte";

const nodes = [
  { key: "a", title: "A" }, { key: "b", title: "B" },
] as any;
const edges = [{ id: "e1", from: "a", to: "b", kind: "associates" }] as any;
const diagrams = [{ key: "d2", members: ["b"] }] as any;

test("renders a chip for an off-diagram ref and navigates on click", async () => {
  const onNavigate = vi.fn();
  render(ExternalRefs, {
    props: { nodeKey: "a", nodes, edges, members: ["a"], diagrams, onNavigate },
  });
  await fireEvent.click(screen.getByRole("button", { name: /associates → B/ }));
  expect(onNavigate).toHaveBeenCalledWith("d2", "b");
});

test("renders nothing when there are no off-diagram refs", () => {
  render(ExternalRefs, {
    props: { nodeKey: "a", nodes, edges, members: ["a", "b"], diagrams, onNavigate: vi.fn() },
  });
  expect(screen.queryByRole("button")).toBeNull();
  expect(screen.queryByText("External references")).toBeNull();
});
