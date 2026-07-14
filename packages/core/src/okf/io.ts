import { zipSync, unzipSync, strToU8, strFromU8 } from "fflate";

// Branded footer appended to the bundle index — every exported model carries an
// attribution + a link back to the source.
const OKF_FOOTER =
  "\n\n---\n\n" +
  "_Generated with [WAML](https://github.com/redoz/waml)_\n";

/** Turn the WASM store's `[path, markdown][]` bundle into the flat file map the
 *  zip download expects, tacking the WAML attribution footer onto the index
 *  (creating one if the bundle has no index doc). Per-doc markdown stays clean. */
export function bundleToDownloadFiles(bundle: [string, string][], projectTitle: string): Record<string, string> {
  const files: Record<string, string> = {};
  for (const [path, md] of bundle) files[path] = md;
  const indexKey = Object.keys(files).find(k => k.endsWith("index.md"));
  if (indexKey) files[indexKey] = files[indexKey].replace(/\s*$/, "") + OKF_FOOTER;
  else files["index.md"] = `# ${projectTitle}\n${OKF_FOOTER}`;
  return files;
}

export function bundleToZip(files: Record<string, string>): Uint8Array {
  const entries: Record<string, Uint8Array> = {};
  for (const [path, content] of Object.entries(files)) entries[path] = strToU8(content);
  return zipSync(entries, { level: 6 });
}

export function zipToFiles(buf: Uint8Array): Record<string, string> {
  const out: Record<string, string> = {};
  const unzipped = unzipSync(buf);
  for (const [path, bytes] of Object.entries(unzipped)) {
    if (path.endsWith("/")) continue;
    out[path] = strFromU8(bytes);
  }
  return out;
}

export function downloadBundle(files: Record<string, string>, name = "model-okf") {
  const blob = new Blob([bundleToZip(files).slice()], { type: "application/zip" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `${name}.zip`;
  a.click();
}

export function parsePastedMarkdown(text: string): Record<string, string> {
  const parts = text.split(/<!--\s*(.+?)\s*-->\n/).slice(1);
  if (parts.length === 0) return { "pasted/doc.md": text };
  const files: Record<string, string> = {};
  for (let i = 0; i < parts.length; i += 2) files[parts[i]] = parts[i + 1] || "";
  return files;
}
