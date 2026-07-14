import { test, expect, beforeAll } from "vitest";
import { initWasm } from "@waml/wasm";
import { DEFAULT_DISPLAY } from "@waml/okf";
import { createModelStore } from "@waml/core/state/model";
import { toRFNode } from "./toRFNode";

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
