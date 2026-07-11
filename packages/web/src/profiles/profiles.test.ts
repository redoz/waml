import { describe, it, expect } from "vitest";
import { getProfile } from "./index";

describe("profiles", () => {
  it("uml-domain is the default and the unknown-name fallback", () => {
    expect(getProfile().name).toBe("uml-domain");
    expect(getProfile("no-such-profile").name).toBe("uml-domain");
    expect(getProfile("uml-domain").hide).toContain("visibility");
    expect(getProfile("uml-domain").emphasize).toContain("multiplicity");
  });
  it("uml-domain styles the DDD stereotypes and offers the spec palette", () => {
    const p = getProfile("uml-domain");
    expect(p.stereotypes.aggregateRoot).toEqual({ header: "#eab308", border: "thick" });
    expect(p.stereotypes.valueObject).toEqual({ header: "#64748b" });
    expect(p.stereotypes.domainEvent).toEqual({ shape: "hexagon" });
    expect(p.palette.metaclasses).toEqual(["uml.Class", "uml.Interface", "uml.Enum", "uml.DataType"]);
    expect(p.palette.stereotypes).toEqual(["entity", "valueObject", "aggregateRoot", "service", "domainEvent"]);
  });
});
