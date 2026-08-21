// Copy the built `waml` binary into the extension so the packaged .vsix can
// run a language server without one being on PATH.
//
// `resolveServerCommand` has always had a "bundled" branch looking for
// <extensionPath>/server/waml[.exe]; nothing ever put a binary there, so the
// branch documented itself as dead. This is the missing half.
//
// Run before `pnpm run package` when you want a self-contained extension:
//   cargo build --release -p waml-cli && pnpm run bundle:server && pnpm run package
import { copyFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, "..", "..", "..");
const exeName = process.platform === "win32" ? "waml.exe" : "waml";
const built = join(repoRoot, "target", "release", exeName);

if (!existsSync(built)) {
  console.error(`no release binary at ${built}\nbuild it first: cargo build --release -p waml-cli`);
  process.exit(1);
}

const destDir = join(here, "..", "server");
mkdirSync(destDir, { recursive: true });
const dest = join(destDir, exeName);
copyFileSync(built, dest);
console.log(`bundled ${built} -> ${dest}`);
