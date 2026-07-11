import { describe, it, expect } from "vitest";
import { Position } from "@xyflow/react";
import { getEdgeParams, portPoint } from "./floating";

// InternalNode-shaped plain objects (measured + internals.positionAbsolute).
const geom = (x: number, y: number, w = 100, h = 100) => ({
  measured: { width: w, height: h },
  internals: { positionAbsolute: { x, y } },
});

describe("getEdgeParams (floating-edge border intersection)", () => {
  it("target directly to the right → source exits its Right border at center Y", () => {
    const source = geom(0, 0);
    const target = geom(200, 0);
    const p = getEdgeParams(source, target);
    expect(p.sourcePos).toBe(Position.Right);
    expect(p.sx).toBe(100); // source right border x (0 + width)
    expect(p.sy).toBe(50); // source vertical center
    expect(p.targetPos).toBe(Position.Left);
  });

  it("target directly below → source exits its Bottom border", () => {
    const source = geom(0, 0);
    const target = geom(0, 200);
    const p = getEdgeParams(source, target);
    expect(p.sourcePos).toBe(Position.Bottom);
    expect(p.sx).toBe(50); // horizontal center
    expect(p.sy).toBe(100); // bottom border y
    expect(p.targetPos).toBe(Position.Top);
  });

  it("diagonal target → finite coords and a border-facing position", () => {
    const source = geom(0, 0);
    const target = geom(300, 300);
    const p = getEdgeParams(source, target);
    expect(Number.isFinite(p.sx)).toBe(true);
    expect(Number.isFinite(p.sy)).toBe(true);
    expect(Number.isFinite(p.tx)).toBe(true);
    expect(Number.isFinite(p.ty)).toBe(true);
    expect([Position.Right, Position.Bottom]).toContain(p.sourcePos);
    expect([Position.Left, Position.Top]).toContain(p.targetPos);
  });

  it("zero-size node → no NaN (safe center/Right-Left fallback)", () => {
    const source = { measured: { width: 0, height: 0 }, internals: { positionAbsolute: { x: 10, y: 20 } } };
    const target = geom(200, 0);
    const p = getEdgeParams(source, target);
    expect(Number.isNaN(p.sx)).toBe(false);
    expect(Number.isNaN(p.sy)).toBe(false);
    expect(Number.isNaN(p.tx)).toBe(false);
    expect(Number.isNaN(p.ty)).toBe(false);
    expect(p.sourcePos).toBe(Position.Right);
  });

  it("undefined measured → no NaN", () => {
    const source = { internals: { positionAbsolute: { x: 0, y: 0 } } };
    const target = { internals: { positionAbsolute: { x: 200, y: 0 } } };
    const p = getEdgeParams(source, target);
    expect(Number.isNaN(p.sx)).toBe(false);
    expect(Number.isNaN(p.ty)).toBe(false);
  });
});

describe("portPoint (spread edges along a border)", () => {
  const rect = { x: 0, y: 0, w: 100, h: 200 };
  it("a lone edge sits at the side midpoint", () => {
    expect(portPoint(rect, Position.Right)).toEqual({ x: 100, y: 100 });
    expect(portPoint(rect, Position.Bottom)).toEqual({ x: 50, y: 200 });
  });
  it("a group spreads in order along the side, inside the border band", () => {
    const a = portPoint(rect, Position.Right, { index: 0, count: 3 });
    const b = portPoint(rect, Position.Right, { index: 1, count: 3 });
    const c = portPoint(rect, Position.Right, { index: 2, count: 3 });
    expect(a.x).toBe(100); expect(b.x).toBe(100); expect(c.x).toBe(100); // all on the right edge
    expect(a.y).toBeLessThan(b.y); expect(b.y).toBeLessThan(c.y);        // ordered by slot
    expect(b.y).toBe(100);                                              // middle slot at center
    expect(a.y).toBeGreaterThan(0); expect(c.y).toBeLessThan(200);       // never on the corners
  });
  it("horizontal sides vary X", () => {
    const a = portPoint(rect, Position.Top, { index: 0, count: 2 });
    const b = portPoint(rect, Position.Top, { index: 1, count: 2 });
    expect(a.y).toBe(0); expect(b.y).toBe(0);
    expect(a.x).toBeLessThan(b.x);
  });
});
