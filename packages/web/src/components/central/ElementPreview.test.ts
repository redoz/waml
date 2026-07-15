import { test, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import { DEFAULT_DISPLAY, type ModelNode, type ModelEdge } from "@waml/okf";
import ElementPreview from "./ElementPreview.svelte";

const node = (key: string): ModelNode =>
  ({
    key,
    type: "uml.Class",
    concept: { id: key, type: "uml.Class", title: key, body: "" },
    stereotypes: [],
    attributes: [],
    position: { x: 0, y: 0 },
  }) as ModelNode;

const edge: ModelEdge = {
  id: "ab",
  kind: "associates",
  from: "a",
  to: "b",
  fromEnd: {},
  toEnd: {},
  bidirectional: false,
};

test("renders the fixed-height preview region for a node", () => {
  render(ElementPreview, {
    props: {
      mode: "node",
      focalKey: "a",
      nodes: [node("a"), node("b")],
      edges: [edge],
      display: { ...DEFAULT_DISPLAY },
      profileName: "uml-domain",
    },
  });
  const region = screen.getByTestId("element-preview");
  expect(region).toBeTruthy();
  expect(region.className).toContain("h-[220px]");
});

test("renders the preview region for an edge", () => {
  render(ElementPreview, {
    props: {
      mode: "edge",
      focalKey: "ab",
      nodes: [node("a"), node("b")],
      edges: [edge],
      display: { ...DEFAULT_DISPLAY },
      profileName: "uml-domain",
    },
  });
  expect(screen.getByTestId("element-preview")).toBeTruthy();
});
