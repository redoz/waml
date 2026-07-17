import { test, expect, beforeAll } from "vitest";
import { initWasm } from "@waml/wasm";
import { DEFAULT_DISPLAY } from "@waml/okf";
import { createModelStore } from "@waml/core/state/model";
import { toRFNode, toGroupNode } from "./toRFNode";
import type { SolvedGroup } from "@waml/wasm";

const frame: SolvedGroup = { rect: { x: 8, y: 8, w: 232, h: 212 }, shape: "Frame", title: "Users", depth: 1 };

beforeAll(async () => {
  await initWasm();
});

test("toRFNode wraps a model node with okf type + payload flags and a cloned position", () => {
  const s = createModelStore();
  const n = s.addNode({ x: 3, y: 4 });
  const display = { ...DEFAULT_DISPLAY, showAttributes: false };
  const rf = toRFNode(s.get().nodes[0], display, "uml", true);
  expect(rf).toMatchObject({ id: n.key, type: "okf", position: { x: 3, y: 4 } });
  const d = rf.data as { _display: typeof display; _profile: string; _collapsed: boolean; key: string };
  expect(d).toMatchObject({ _display: display, _profile: "uml", _collapsed: true, key: n.key });
  expect(rf.position).not.toBe(s.get().nodes[0].position); // cloned, not shared
});

test("toGroupNode maps a Frame group to a non-interactive group-frame pseudo-node", () => {
  const n = toGroupNode(frame, 0)!;
  expect(n).toMatchObject({
    id: "__group__0",
    type: "group-frame",
    position: { x: 8, y: 8 },
    selectable: false,
    draggable: false,
    deletable: false,
  });
  expect(n.data).toMatchObject({ title: "Users", width: 232, height: 212 });
  expect(n.zIndex).toBe(1 - 1000); // depth 1 → below members (0), above outer groups
});

test("toGroupNode returns null for Box and Shrink groups (they draw nothing)", () => {
  expect(toGroupNode({ ...frame, shape: "Box" }, 1)).toBeNull();
  expect(toGroupNode({ ...frame, shape: "Shrink" }, 2)).toBeNull();
});
