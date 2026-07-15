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
      showAttributeVisibility: true,
      showAttributeMultiplicity: true,
      showRoles: true,
      showCardinality: true,
      showLabels: true,
      showStereotype: true,
      stereotypeColors: {},
    });
  });

  it("leaves nullable fields (maxAttributes, stereotypeFilter) undefined by default", () => {
    const r = resolveDisplay(undefined);
    expect(r.maxAttributes).toBeUndefined();
    expect(r.stereotypeFilter).toBeUndefined();
  });

  it("overlays new fields, keeping stereotypeColors a record", () => {
    const r = resolveDisplay({ maxAttributes: 6, stereotypeFilter: ["entity"], stereotypeColors: { entity: "#fff" } });
    expect(r.maxAttributes).toBe(6);
    expect(r.stereotypeFilter).toEqual(["entity"]);
    expect(r.stereotypeColors).toEqual({ entity: "#fff" });
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
