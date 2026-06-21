import { describe, it, expect } from "vitest";
import type { ModelNode, ModelEdge } from "@mc/okf";
import { joinFieldType } from "./joinFieldType";

const nodes: ModelNode[] = [
  { key: "badges", title: "Badges", inputSource: "TABLE", status: "pending", owoxId: null, position: { x: 0, y: 0 },
    schema: [{ name: "id", type: "INTEGER", pk: true }, { name: "user_id", type: "INTEGER", pk: false }] },
  { key: "newobj", title: "New object", inputSource: "SQL", status: "pending", owoxId: null, position: { x: 0, y: 0 },
    schema: [] },
];
const edges: ModelEdge[] = [
  { id: "e1", from: "newobj", to: "badges", keys: [{ left: "id", right: "id" }], bidirectional: false },
];

describe("joinFieldType", () => {
  it("infers a missing field's type from the counterpart (INTEGER, not STRING)", () => {
    // newobj.id is missing; the join pairs it with badges.id (INTEGER).
    expect(joinFieldType(nodes, edges, "newobj", "id")).toBe("INTEGER");
  });

  it("returns the counterpart type for the other side too", () => {
    expect(joinFieldType(nodes, edges, "badges", "id")).toBe("STRING"); // newobj.id missing → unknown → STRING
  });

  it("falls back to STRING when neither side resolves", () => {
    expect(joinFieldType(nodes, edges, "newobj", "ghost")).toBe("STRING");
  });

  it("returns STRING for an empty field name", () => {
    expect(joinFieldType(nodes, edges, "newobj", "")).toBe("STRING");
  });
});
