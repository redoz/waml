import { describe, it, expect } from "vitest";
import { DEFAULT_DISPLAY, resolveDisplay, type DiagramDisplay } from "@uaml/okf";
import { createModelStore } from "./model";

describe("model store — per-diagram display", () => {
  it("updateDiagram sets the active diagram's display", () => {
    const s = createModelStore();
    const d = s.addDiagram("Core");
    // No display until one is set → resolves to defaults.
    expect(s.get().diagrams[0].display).toBeUndefined();
    expect(resolveDisplay(s.get().diagrams[0].display)).toEqual(DEFAULT_DISPLAY);

    const display: DiagramDisplay = { ...DEFAULT_DISPLAY, showAttributes: false, associationLabels: "hidden" };
    s.updateDiagram(d.key, { display });
    expect(s.get().diagrams[0].display).toEqual(display);
  });

  it("only touches the targeted diagram", () => {
    const s = createModelStore();
    const a = s.addDiagram("A");
    const b = s.addDiagram("B");
    s.updateDiagram(b.key, { display: { ...DEFAULT_DISPLAY, showStereotype: false } });
    expect(s.get().diagrams.find(d => d.key === a.key)!.display).toBeUndefined();
    expect(s.get().diagrams.find(d => d.key === b.key)!.display).toEqual({ ...DEFAULT_DISPLAY, showStereotype: false });
  });

  it("merging a fresh display replaces the prior one (partial merge is the caller's job)", () => {
    const s = createModelStore();
    const d = s.addDiagram("Core");
    s.updateDiagram(d.key, { display: { ...DEFAULT_DISPLAY, showAttributes: false } });
    // Caller resolves + spreads the previous display before patching a single field.
    const prev = resolveDisplay(s.get().diagrams[0].display);
    s.updateDiagram(d.key, { display: { ...prev, attributeDetail: "name-only" } });
    expect(s.get().diagrams[0].display).toEqual({ ...DEFAULT_DISPLAY, showAttributes: false, attributeDetail: "name-only" });
  });
});
