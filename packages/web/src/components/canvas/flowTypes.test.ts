import { test, expect } from "vitest";
import { nodeTypes } from "./flowTypes";
import GroupFrame from "./nodes/GroupFrame.svelte";
import OkfNode from "./nodes/OkfNode.svelte";

test("registers the okf and group-frame node types", () => {
  expect(nodeTypes.okf).toBe(OkfNode);
  expect(nodeTypes["group-frame"]).toBe(GroupFrame);
});
