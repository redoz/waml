// tsc compiles src/generated/wasm-inline.ts but does NOT copy the plain-JS
// wasm-bindgen glue (waml_wasm.js) or its .d.ts. Copy them into dist/generated so
// the published package is self-contained for cross-package imports (core/web
// build + tests resolve @waml/wasm → dist and follow dist/index → dist/generated).
import { copyFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const pkg = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(pkg, "src", "generated");
const dist = join(pkg, "dist", "generated");

mkdirSync(dist, { recursive: true });
for (const f of ["waml_wasm.js", "waml_wasm.d.ts"]) {
  copyFileSync(join(src, f), join(dist, f));
}
