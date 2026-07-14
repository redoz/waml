import { test, expect } from "vitest";
import { resolveNodeRenderer } from "./registry";
import GenericNode from "./GenericNode.svelte";
import UmlNoteNode from "./UmlNoteNode.svelte";
import UmlActorNode from "./UmlActorNode.svelte";
import UmlUseCaseNode from "./UmlUseCaseNode.svelte";

test("resolves a known uml metaclass to its renderer", () => {
  expect(resolveNodeRenderer("uml.Note")).toBe(UmlNoteNode);
});
test("falls back to GenericNode for unknown types", () => {
  expect(resolveNodeRenderer("dwh.Table")).toBe(GenericNode);
  expect(resolveNodeRenderer("nonsense")).toBe(GenericNode);
});
test("resolves uml.Actor uml.UseCase dedicated renderers", () => {
  expect(resolveNodeRenderer("uml.Actor")).toBe(UmlActorNode);
  expect(resolveNodeRenderer("uml.UseCase")).toBe(UmlUseCaseNode);
});
