// Proves the inlined wasm is callable end-to-end from JS: apply an op, then
// resolve the edited bundle to a Model, all through the Rust core.
import { test, expect } from "vitest";
import { initWasm, apply_ops, build_model } from "./index";

test("apply_ops → build_model round-trips through wasm", async () => {
  await initWasm();
  const bundle = [["m/a.md", "---\ntype: uml.Class\ntitle: A\n---\n# A\n"]];
  const out = apply_ops(bundle, [{ op: "attr.add", node: "a", name: "id", ty: "AId" }]);
  const model = build_model(out) as { nodes: any[] };
  const node = model.nodes.find((n) => n.key === "a");
  expect(node.type).toBe("uml.Class");
  expect(node.attributes[0].name).toBe("id");
  expect(node.attributes[0].type.name).toBe("AId");
});
