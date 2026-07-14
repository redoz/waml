import { describe, it, expect } from "vitest";
import { DEFAULT_DISPLAY, resolveDisplay } from "../src/types";

describe("resolveDisplay", () => {
  it("returns the full DEFAULT_DISPLAY when display is absent", () => {
    expect(resolveDisplay(undefined)).toEqual(DEFAULT_DISPLAY);
  });

  it("returns the documented default values", () => {
    expect(DEFAULT_DISPLAY).toEqual({
      showAttributes: true,
      attributeDetail: "name-type",
      associationLabels: "all",
      emphasizeMultiplicity: false,
      showStereotype: true,
    });
  });

  it("overlays a partial display onto the defaults", () => {
    expect(resolveDisplay({ showAttributes: false, attributeDetail: "name-only" })).toEqual({
      ...DEFAULT_DISPLAY,
      showAttributes: false,
      attributeDetail: "name-only",
    });
  });

  it("does not mutate DEFAULT_DISPLAY", () => {
    resolveDisplay({ showStereotype: false });
    expect(DEFAULT_DISPLAY.showStereotype).toBe(true);
  });
});
