import { describe, it, expect } from "vitest";
import type { ModelNode } from "@mc/okf";
import { erdAwareNodeSize } from "./layoutSize";

const mk = (fields: number): ModelNode => ({
  key: "n", title: "n", inputSource: "VIEW",
  schema: Array.from({ length: fields }, (_, i) => ({ name: `f${i}`, type: "STRING", pk: false })),
  position: { x: 0, y: 0 }, status: "created", owoxId: null,
});

describe("erdAwareNodeSize", () => {
  it("uses the fixed compact size regardless of field count", () => {
    expect(erdAwareNodeSize(mk(8), "compact")).toEqual({ width: 200, height: 90 });
  });

  it("grows ERD height with the number of fields", () => {
    const few = erdAwareNodeSize(mk(2), "erd");
    const many = erdAwareNodeSize(mk(8), "erd");
    expect(many.height).toBeGreaterThan(few.height);
    expect(few.width).toBe(250);
  });

  it("gives a field-less ERD node a non-zero height", () => {
    expect(erdAwareNodeSize(mk(0), "erd").height).toBeGreaterThan(0);
  });
});
