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

// Dimming logic (ElementPreviewCanvas): nodes in `focalKeys` render at full
// opacity (`style: undefined`); every other node in the subset renders dimmed
// (`style: "opacity:0.4"`). @xyflow/svelte applies a node's `style` directly
// to its `.svelte-flow__node[data-id]` wrapper div, so we can assert on it
// through the rendered DOM (unlike edges — see note on the edge-mode test
// below, where @xyflow/svelte's viewport-overlap culling means no edge DOM
// renders at all under jsdom, regardless of dimming correctness).
test("node-edit mode: focal node renders full-opacity, connected neighbor renders dimmed", () => {
  const { container } = render(ElementPreview, {
    props: {
      mode: "node",
      focalKey: "a",
      nodes: [node("a"), node("b")],
      edges: [edge],
      display: { ...DEFAULT_DISPLAY },
      profileName: "uml-domain",
    },
  });
  const focalNode = container.querySelector('.svelte-flow__node[data-id="a"]') as HTMLElement;
  const neighborNode = container.querySelector('.svelte-flow__node[data-id="b"]') as HTMLElement;
  expect(focalNode).toBeTruthy();
  expect(neighborNode).toBeTruthy();
  expect(focalNode.style.opacity).toBe("");
  expect(neighborNode.style.opacity).toBe("0.4");
});

// Edge-edit mode: both endpoints of the focal edge are in `focalKeys`
// (`edgePreviewSubset` sets `focalKeys = { edge.from, edge.to }`), so both
// nodes render full-opacity — this exercises the same subset->focalKeys
// wiring the buggy earlier draft got wrong for the edge's own AND condition
// (`focalKeys.has(e.source) && focalKeys.has(e.target)`).
//
// NOTE: asserting directly on the *edge's* dim style was attempted but is
// not observable via rendered DOM in this test environment:
// @xyflow/svelte's EdgeRenderer filters `store.visible.edges` through
// `isEdgeVisible()`, which computes overlap against the flow container's
// live `clientWidth`/`clientHeight` — always 0 under jsdom (no real layout
// engine), so no edge ever renders regardless of the dimming logic's
// correctness. Forcing non-zero measured dimensions via a firing
// ResizeObserver + `getBoundingClientRect` stub was attempted and caused
// @xyflow/svelte's internal reactivity to loop indefinitely (the test
// process had to be killed), so that path was abandoned as impractical.
// Separately, `RelEdge.svelte` (the "rel" edge type used for all model
// edges) was found to not forward its incoming `style` prop to `<BaseEdge>`
// at all — so even outside tests, edge dimming had no visual effect in the
// running app. That gap has since been fixed (see RelEdge.svelte's `style`
// destructuring and `edgeStyle` computation); it just isn't observable via
// rendered DOM in this jsdom test environment for the reason above.
test("edge-edit mode: both endpoints of the focal edge render full-opacity", () => {
  const { container } = render(ElementPreview, {
    props: {
      mode: "edge",
      focalKey: "ab",
      nodes: [node("a"), node("b")],
      edges: [edge],
      display: { ...DEFAULT_DISPLAY },
      profileName: "uml-domain",
    },
  });
  const fromNode = container.querySelector('.svelte-flow__node[data-id="a"]') as HTMLElement;
  const toNode = container.querySelector('.svelte-flow__node[data-id="b"]') as HTMLElement;
  expect(fromNode).toBeTruthy();
  expect(toNode).toBeTruthy();
  expect(fromNode.style.opacity).toBe("");
  expect(toNode.style.opacity).toBe("");
});
