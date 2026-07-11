// Owned conceptually by Canvas (Task 7 / Plan 3b wires the real selection state
// machine there); lives here so both Canvas and Inspector's stub can import the
// same type without a cross-import cycle between them.
export type Selection = { type: "node"; id: string } | { type: "edge"; id: string } | null;
