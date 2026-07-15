import { test, expect } from "vitest";
import { diagramCandidateStereotypes, isDiagramEditable } from "./diagramProps";
import { ALL_DIAGRAM_KEY } from "@waml/core/state/diagrams";
import type { ModelNode } from "@waml/okf";

const node = (key: string, stereotypes: string[]): ModelNode =>
  ({ key, type: "uml.Class", concept: { id: key, type: "uml.Class", title: key, description: "" }, stereotypes, attributes: [], position: { x: 0, y: 0 } }) as unknown as ModelNode;

test("candidate stereotypes are unique, sorted, and scoped to members", () => {
  const nodes = [node("a", ["entity", "root"]), node("b", ["entity"]), node("c", ["service"])];
  expect(diagramCandidateStereotypes(nodes, ["a", "b"])).toEqual(["entity", "root"]);
});

test("no members ⇒ empty candidate list", () => {
  expect(diagramCandidateStereotypes([node("a", ["entity"])], [])).toEqual([]);
});

test("editable is false only for the implicit All diagram", () => {
  expect(isDiagramEditable(ALL_DIAGRAM_KEY)).toBe(false);
  expect(isDiagramEditable("orders")).toBe(true);
});
