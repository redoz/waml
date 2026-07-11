import { test, expect } from "vitest";
import { createModelStore } from "@mc/core/state/model";
import { toRFNode } from "./toRFNode";

test("toRFNode wraps a model node with okf type + payload flags and a cloned position", () => {
  const s = createModelStore();
  const n = s.addNode({ x: 3, y: 4 });
  const rf = toRFNode(s.get().nodes[0], "erd", "uml", true);
  expect(rf).toMatchObject({ id: n.key, type: "okf", position: { x: 3, y: 4 } });
  const d = rf.data as { _viewMode: string; _profile: string; _collapsed: boolean; key: string };
  expect(d).toMatchObject({ _viewMode: "erd", _profile: "uml", _collapsed: true, key: n.key });
  expect(rf.position).not.toBe(s.get().nodes[0].position); // cloned, not shared
});
