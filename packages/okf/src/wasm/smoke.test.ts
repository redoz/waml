// Proves the inlined wasm is callable end-to-end from JS: apply an op, then
// resolve the edited bundle to a Model, all through the Rust core.
import { test, expect } from "vitest";
import { initWasm, apply_ops, build_model, build_bundle } from "./index";
import type { Bundle } from "../types";

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

test("build_bundle projects every doc to a lossless OKF Concept through wasm", async () => {
  await initWasm();
  const bundle = [
    [
      "playbooks/dataplex.md",
      "---\n" +
        "type: Playbook\n" +
        "title: Dataplex Playbook\n" +
        "description: How to onboard Dataplex.\n" +
        "resource: /playbooks/dataplex\n" +
        "tags: [data, governance]\n" +
        "timestamp: 2026-05-22\n" +
        "owner: data-team\n" +
        "---\n" +
        "# Dataplex Playbook\n\n" +
        "See the [customers table](/tables/customers.md) for the join key.\n\n" +
        "# Citations\n\n" +
        "[1] [BigQuery announcement](https://cloud.google.com/blog/x)\n",
    ],
  ];
  const out = build_bundle(bundle) as Bundle;
  const c = out.concepts.find((c) => c.id === "playbooks/dataplex");
  expect(c).toBeDefined();
  expect(c!.type).toBe("Playbook");
  expect(c!.title).toBe("Dataplex Playbook");
  expect(c!.description).toBe("How to onboard Dataplex.");
  expect(c!.resource).toBe("/playbooks/dataplex");
  expect(c!.tags).toEqual(["data", "governance"]);
  expect(c!.timestamp).toBe("2026-05-22");
  expect(c!.body).toContain("# Dataplex Playbook");
  expect(c!.links?.[0].href).toBe("/tables/customers.md");
  expect(c!.citations?.[0].href).toBe("https://cloud.google.com/blog/x");
  expect(c!.extra?.owner).toBe("data-team");
});
