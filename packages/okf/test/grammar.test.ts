import { describe, it, expect } from "vitest";
import {
  isValidMultiplicity, parseAttributeLine, parseValueLine, parseRelationshipLine,
  renderAttributeLine, renderRelationshipLine,
} from "../src/grammar";

describe("isValidMultiplicity", () => {
  it.each(["1", "5", "*", "0..1", "1..*", "0..*", "2..5"])("accepts %s", s =>
    expect(isValidMultiplicity(s)).toBe(true));
  it.each(["0", "", "N", "1..", "..1", "5..2", "*..1", "01"])("rejects %s", s =>
    expect(isValidMultiplicity(s)).toBe(false));
});

describe("parseAttributeLine", () => {
  const resolve = (slug: string) => (slug === "money" ? "money" : undefined);
  it("bare token with default multiplicity", () => {
    expect(parseAttributeLine("- placedAt: Timestamp", resolve))
      .toEqual({ name: "placedAt", type: { name: "Timestamp" }, multiplicity: "1" });
  });
  it("linked type with multiplicity", () => {
    expect(parseAttributeLine("- total: [Money](./money.md) {1}", resolve))
      .toEqual({ name: "total", type: { name: "Money", ref: "money" }, multiplicity: "1" });
  });
  it("unresolvable link keeps the display name as a token", () => {
    expect(parseAttributeLine("- addr: [Address](./address.md) {0..1}", resolve))
      .toEqual({ name: "addr", type: { name: "Address" }, multiplicity: "0..1" });
  });
  it("leading visibility", () => {
    expect(parseAttributeLine("- + id: OrderId {1}", resolve))
      .toEqual({ name: "id", type: { name: "OrderId" }, multiplicity: "1", visibility: "+" });
  });
  it("tolerates CRLF", () => {
    expect(parseAttributeLine("- a: B\r", resolve)).toEqual({ name: "a", type: { name: "B" }, multiplicity: "1" });
  });
  it("rejects non-attribute lines", () => {
    expect(parseAttributeLine("- just prose", resolve)).toBeNull();
  });
});

describe("attribute multiplicity delimiter is {…} (not [ … ])", () => {
  const resolve = (slug: string) => (slug === "money" ? "money" : undefined);
  it("parses a bare-token type with brace multiplicity", () => {
    expect(parseAttributeLine("- tags: String {0..*}", resolve))
      .toEqual({ name: "tags", type: { name: "String" }, multiplicity: "0..*" });
  });
  it("parses a linked type with brace multiplicity, keeping the markdown link", () => {
    expect(parseAttributeLine("- status: [OrderStatus](./order-status.md) {0..1}", resolve))
      .toEqual({ name: "status", type: { name: "OrderStatus" }, multiplicity: "0..1" });
  });
  it("no longer treats a trailing [mult] as multiplicity (bracket makes the type malformed)", () => {
    expect(parseAttributeLine("- id: OrderId [1]", resolve)).toBeNull();
    expect(parseAttributeLine("- tags: String [0..*]", resolve)).toBeNull();
  });
  it("rejects a brace token whose contents are not a valid multiplicity", () => {
    expect(parseAttributeLine("- x: Foo {bogus}", resolve)).toBeNull();
    expect(parseAttributeLine("- x: Foo {0}", resolve)).toBeNull();
  });
  it("type-guard rejects a stray { or } in type position", () => {
    expect(parseAttributeLine("- x: Foo{", resolve)).toBeNull();
    expect(parseAttributeLine("- x: Foo}", resolve)).toBeNull();
  });
  it("renders multiplicity with braces; omits the default 1", () => {
    const slugFor = (key: string) => (key === "money" ? "money" : undefined);
    expect(renderAttributeLine({ name: "tags", type: { name: "String" }, multiplicity: "0..*" }, slugFor))
      .toBe("- tags: String {0..*}");
    expect(renderAttributeLine({ name: "total", type: { name: "Money", ref: "money" }, multiplicity: "1" }, slugFor))
      .toBe("- total: [Money](./money.md)");
  });
});

describe("parseValueLine", () => {
  it("reads a literal", () => expect(parseValueLine("- DRAFT")).toBe("DRAFT"));
  it("rejects blanks", () => expect(parseValueLine("-  ")).toBeNull());
});

describe("parseRelationshipLine", () => {
  it("associates with ends and roles", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md): 1 order to 1 buyer"))
      .toEqual({ kind: "associates", targetSlug: "customer",
        fromEnd: { multiplicity: "1", role: "order" }, toEnd: { multiplicity: "1", role: "buyer" } });
  });
  it("composes with range multiplicity", () => {
    expect(parseRelationshipLine("- composes [OrderLine](./order-line.md): 1 to 1..*"))
      .toEqual({ kind: "composes", targetSlug: "order-line",
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1..*" } });
  });
  it("specializes takes no ends", () => {
    expect(parseRelationshipLine("- specializes [Party](./party.md)"))
      .toEqual({ kind: "specializes", targetSlug: "party", fromEnd: {}, toEnd: {} });
  });
  it("captures an `as \"string\"` association name before the ends", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md) as \"places\": 1 order to 1 customer"))
      .toEqual({ kind: "associates", targetSlug: "customer", name: "places",
        fromEnd: { multiplicity: "1", role: "order" }, toEnd: { multiplicity: "1", role: "customer" } });
  });
  it("captures an `as [link]` association-class name as { ref: slug }", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md) as [Places](./places.md): 1 to 1"))
      .toEqual({ kind: "associates", targetSlug: "customer", name: { ref: "places" },
        fromEnd: { multiplicity: "1" }, toEnd: { multiplicity: "1" } });
  });
  it("allows a name on a no-ends verb too", () => {
    expect(parseRelationshipLine("- depends [PricingService](./pricing-service.md) as \"prices\""))
      .toEqual({ kind: "depends", targetSlug: "pricing-service", name: "prices", fromEnd: {}, toEnd: {} });
  });
  it("ends are REQUIRED for associates", () => {
    expect(parseRelationshipLine("- associates [Customer](./customer.md)")).toBeNull();
  });
  it("ends are FORBIDDEN for depends", () => {
    expect(parseRelationshipLine("- depends [PricingService](./pricing-service.md): 1 to 1")).toBeNull();
  });
  it("rejects invalid multiplicities and unknown verbs", () => {
    expect(parseRelationshipLine("- associates [C](./c.md): N to 1")).toBeNull();
    expect(parseRelationshipLine("- likes [C](./c.md)")).toBeNull();
  });
});

describe("render round-trip", () => {
  it("attribute line", () => {
    const slugFor = (key: string) => (key === "money" ? "money" : undefined);
    expect(renderAttributeLine({ name: "total", type: { name: "Money", ref: "money" }, multiplicity: "1" }, slugFor))
      .toBe("- total: [Money](./money.md)");
    expect(renderAttributeLine({ name: "addr", type: { name: "Address" }, multiplicity: "0..1", visibility: "-" }, slugFor))
      .toBe("- - addr: Address {0..1}");
  });
  it("relationship line", () => {
    expect(renderRelationshipLine("composes", "OrderLine", "order-line", { multiplicity: "1" }, { multiplicity: "1..*", role: "lines" }))
      .toBe("- composes [OrderLine](./order-line.md): 1 to 1..* lines");
    expect(renderRelationshipLine("specializes", "Party", "party", {}, {}))
      .toBe("- specializes [Party](./party.md)");
  });
  it("relationship line with a string name", () => {
    expect(renderRelationshipLine("associates", "Customer", "customer", { multiplicity: "1" }, { multiplicity: "1" }, "places"))
      .toBe("- associates [Customer](./customer.md) as \"places\": 1 to 1");
  });
  it("relationship line with an association-class link name", () => {
    expect(renderRelationshipLine("associates", "Customer", "customer", { multiplicity: "1" }, { multiplicity: "1" }, { title: "Places", slug: "places" }))
      .toBe("- associates [Customer](./customer.md) as [Places](./places.md): 1 to 1");
  });
});
