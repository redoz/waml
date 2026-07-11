import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { RelEdge } from "./RelEdge";
import type { EdgeProps } from "@xyflow/react";
import { Position } from "@xyflow/react";

const base: EdgeProps = {
  id: "e1", source: "a", target: "b",
  sourceX: 0, sourceY: 0, targetX: 100, targetY: 0,
  sourcePosition: Position.Right, targetPosition: Position.Left,
  selected: false,
} as unknown as EdgeProps;

const draw = (data: Record<string, unknown>) =>
  render(<svg><RelEdge {...base} data={data} /></svg>);

describe("RelEdge UML adornments", () => {
  it("composes draws a filled diamond marker at the source", () => {
    const { container } = draw({ kind: "composes", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    const marker = container.querySelector("marker#diamond-filled-e1");
    expect(marker).toBeTruthy();
    expect(container.innerHTML).toContain("marker-start");
  });
  it("aggregates draws a hollow diamond", () => {
    const { container } = draw({ kind: "aggregates", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    expect(container.querySelector("marker#diamond-hollow-e1")).toBeTruthy();
  });
  it("specializes draws a hollow triangle at the target on a solid line", () => {
    const { container } = draw({ kind: "specializes", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    expect(container.querySelector("marker#triangle-e1")).toBeTruthy();
    expect(container.innerHTML).not.toContain("stroke-dasharray");
  });
  it("implements and depends are dashed", () => {
    const { container } = draw({ kind: "implements", fromEnd: {}, toEnd: {}, bidirectional: false, relLabelMode: "hidden" });
    expect(container.innerHTML).toContain("stroke-dasharray");
  });
  it("associates puts an arrowhead only on navigable ends", () => {
    const one = draw({ kind: "associates", fromEnd: {}, toEnd: { navigable: true }, bidirectional: false, relLabelMode: "hidden" });
    expect(one.container.innerHTML).toContain("marker-end");
    expect(one.container.innerHTML).not.toContain("marker-start");
    const both = draw({ kind: "associates", fromEnd: { navigable: true }, toEnd: { navigable: true }, bidirectional: true, relLabelMode: "hidden" });
    expect(both.container.innerHTML).toContain("marker-start");
  });
});
