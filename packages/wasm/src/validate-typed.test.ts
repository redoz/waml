import { test, expect } from "vitest";
import { initWasm, validate } from "./index";
import type { Diagnostic } from "./index";

test("validate returns a typed Diagnostic[]", async () => {
  await initWasm();
  const bundle: [string, string][] = [["a.md", "---\ntype: uml.Class\n---\n# A\n\n## Relationships\n- composes [Gone](./gone.md)\n"]];
  const diags: Diagnostic[] = validate(bundle);
  // If `validate` were still `any`, `.severity` access would be untyped; this pins the shape.
  expect(diags.every((d) => d.severity === "error" || d.severity === "warning")).toBe(true);
});
