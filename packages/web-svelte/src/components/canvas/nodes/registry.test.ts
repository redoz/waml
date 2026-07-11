import { test, expect } from "vitest";
import { resolveNodeRenderer } from "./registry";
import GenericNode from "./GenericNode.svelte";
import UmlNoteNode from "./UmlNoteNode.svelte";

test("resolves a known uml metaclass to its renderer", () => {
  expect(resolveNodeRenderer("uml.Note")).toBe(UmlNoteNode);
});
test("falls back to GenericNode for unknown types", () => {
  expect(resolveNodeRenderer("dwh.Table")).toBe(GenericNode);
  expect(resolveNodeRenderer("nonsense")).toBe(GenericNode);
});
