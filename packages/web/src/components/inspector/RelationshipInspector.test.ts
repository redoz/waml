import { test, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import type { ModelEdge, ModelNode } from "@waml/okf";
import RelationshipInspector from "./RelationshipInspector.svelte";

const node = (key: string, title: string): ModelNode =>
  ({ concept: { id: key, type: "uml.Class", title, body: "" }, key, type: "uml.Class", stereotypes: [], attributes: [], position: { x: 0, y: 0 } });

const edge: ModelEdge = {
  id: "e1", kind: "associates", from: "a", to: "b",
  fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "*", navigable: true }, bidirectional: false,
};

test("changing Kind patches through onUpdate", async () => {
  const onUpdate = vi.fn();
  render(RelationshipInspector, { props: { edge, fromNode: node("a", "Order"), toNode: node("b", "OrderLine"), onUpdate } });
  await fireEvent.change(screen.getByLabelText("Kind"), { target: { value: "composes" } });
  expect(onUpdate).toHaveBeenCalledWith({ kind: "composes" });
});

test("editing the near-end multiplicity", async () => {
  const onUpdate = vi.fn();
  render(RelationshipInspector, { props: { edge, fromNode: node("a", "Order"), toNode: node("b", "OrderLine"), onUpdate } });
  await fireEvent.input(screen.getByLabelText("Order multiplicity"), { target: { value: "0..1" } });
  expect(onUpdate).toHaveBeenCalledWith({ fromEnd: { multiplicity: "0..1" } });
});

test("hides end editors for specializes", () => {
  render(RelationshipInspector, {
    props: {
      edge: { ...edge, kind: "specializes", fromEnd: {}, toEnd: {} },
      fromNode: node("a", "A"),
      toNode: node("b", "B"),
      onUpdate: () => {},
    },
  });
  expect(screen.queryByLabelText("A multiplicity")).toBeNull();
});
