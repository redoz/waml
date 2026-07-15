// Regenerates the checked-in `*.bundle.ts` template files from their source
// `.md` folders via `waml bundle`. Run manually with `pnpm gen:templates`
// after editing any packages/core/src/templates/*-uml/*.md source.
import { execFileSync } from "node:child_process";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const templatesDir = join(root, "packages", "core", "src", "templates");

// `dir` is resolved relative to `templatesDir` (the cwd for the cargo
// invocation below) so emitted bundle paths match the `<dir>/<file>.md`
// shape the hand-authored bundles used, e.g. "orders-domain-uml/order.md".
const bundles = [
  { dir: "orders-domain-uml", exportName: "ordersDomainBundle", out: join(templatesDir, "orders-domain.bundle.ts") },
];

for (const { dir, exportName, out } of bundles) {
  execFileSync(
    "cargo",
    ["run", "--quiet", "-p", "waml-cli", "--", "bundle", dir, "--format", "ts", "--export-name", exportName, "--out", out],
    { cwd: templatesDir, stdio: "inherit" },
  );
  console.log(`wrote ${out}`);
}
