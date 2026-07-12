import { test, expect, beforeAll } from "vitest";
import { get } from "svelte/store";
import { initWasm } from "@uaml/okf";
import { createModelStore } from "@uaml/core/state/model";
import { toModelReadable } from "./model.svelte";

beforeAll(async () => {
  await initWasm();
});

test("readable yields the current graph and re-emits on store.updateNode", () => {
  const s = createModelStore();
  const n = s.addNode({ x: 0, y: 0 });

  const m = toModelReadable(s);

  const titles: string[] = [];
  const unsub = m.subscribe((g) => {
    const node = g.nodes.find((x) => x.key === n.key);
    if (node) titles.push(node.title);
  });

  // subscribe delivered the current value synchronously
  expect(get(m).nodes.find((x) => x.key === n.key)!.title).toBe("New object");

  s.updateNode(n.key, { title: "Renamed" });

  expect(get(m).nodes.find((x) => x.key === n.key)!.title).toBe("Renamed");
  expect(titles).toContain("New object");
  expect(titles).toContain("Renamed");

  unsub();
});
