import { describe, it, expect } from "vitest";
import { migrateAttributeMultiplicityDelimiter } from "../src/index";

describe("migrateAttributeMultiplicityDelimiter — one-shot [mult] → {mult} rewrite", () => {
  it("rewrites a trailing bare-token multiplicity", () => {
    expect(migrateAttributeMultiplicityDelimiter("- id: OrderId [1]")).toBe("- id: OrderId {1}");
    expect(migrateAttributeMultiplicityDelimiter("- tags: String [0..*]")).toBe("- tags: String {0..*}");
  });
  it("rewrites a trailing multiplicity after a linked type, leaving the link untouched", () => {
    expect(migrateAttributeMultiplicityDelimiter("- shippingAddress: [Address](./address.md) [0..1]"))
      .toBe("- shippingAddress: [Address](./address.md) {0..1}");
  });
  it("applies per line across a whole document (multiline)", () => {
    const before = [
      "## Attributes",
      "- id: OrderId [1]",
      "- status: [OrderStatus](./order-status.md) [1]",
      "- total: [Money](./money.md) [1]",
    ].join("\n");
    const after = [
      "## Attributes",
      "- id: OrderId {1}",
      "- status: [OrderStatus](./order-status.md) {1}",
      "- total: [Money](./money.md) {1}",
    ].join("\n");
    expect(migrateAttributeMultiplicityDelimiter(before)).toBe(after);
  });
  it("leaves a markdown type link with no trailing multiplicity unchanged", () => {
    expect(migrateAttributeMultiplicityDelimiter("- status: [OrderStatus](./order-status.md)"))
      .toBe("- status: [OrderStatus](./order-status.md)");
  });
  it("leaves relationship-end multiplicities (bare, unbracketed) unchanged", () => {
    const rel = "- composes [OrderLine](./order-line.md): 1 order to 1..* lines";
    expect(migrateAttributeMultiplicityDelimiter(rel)).toBe(rel);
  });
  it("does not touch a [1] that is not the last token on the line", () => {
    const line = "- see footnote [1] for details";
    expect(migrateAttributeMultiplicityDelimiter(line)).toBe(line);
  });
  it("does not rewrite a bracket whose contents are not a valid multiplicity", () => {
    expect(migrateAttributeMultiplicityDelimiter("- id: OrderId [0]")).toBe("- id: OrderId [0]");
    expect(migrateAttributeMultiplicityDelimiter("- id: OrderId [N]")).toBe("- id: OrderId [N]");
  });
});
