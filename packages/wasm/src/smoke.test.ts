// Proves build_bundle is callable end-to-end through wasm: resolve a bundle
// into a lossless OKF Concept, all through Rust core.
import { test, expect } from "vitest";
import { initWasm, build_bundle } from "./index";

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
  const out = build_bundle(bundle);
  const c = out.concepts.find((c: any) => c.id === "playbooks/dataplex");
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
