import { describe, it, expect } from "vitest";
import type { ModelNode } from "@waml/okf";
import { DEFAULT_DISPLAY } from "@waml/okf";
import { erdAwareNodeSize } from "./layoutSize";

const mk = (fields: number): ModelNode => ({
  concept: { id: "n", type: "uml.Class", body: "" },
  key: "n", type: "uml.Class", stereotypes: [],
  attributes: Array.from({ length: fields }, (_, i) => ({ name: `f${i}`, type: { name: "STRING" }, multiplicity: "1" })),
  position: { x: 0, y: 0 },
});

const hidden = { ...DEFAULT_DISPLAY, showAttributes: false };
const shown = { ...DEFAULT_DISPLAY, showAttributes: true };

describe("erdAwareNodeSize", () => {
  it("uses the fixed compact size when attributes are hidden, regardless of field count", () => {
    expect(erdAwareNodeSize(mk(8), hidden)).toEqual({ width: 200, height: 90 });
  });

  it("grows height with the number of fields when attributes are shown", () => {
    const few = erdAwareNodeSize(mk(2), shown);
    const many = erdAwareNodeSize(mk(8), shown);
    expect(many.height).toBeGreaterThan(few.height);
    expect(few.width).toBe(250);
  });

  it("gives a field-less shown node a non-zero height", () => {
    expect(erdAwareNodeSize(mk(0), shown).height).toBeGreaterThan(0);
  });
});
