import type { Bundle } from "../state/model";
import { TEMPLATES } from "@waml/core/templates";

// Deep-link: `?template=<id>` opens a named built-in template straight onto the
// canvas. It's the CTA target for the blog template gallery, launch emails and
// social posts — one click from "here's a model" to "you're editing it". An
// unknown/missing id falls through to the normal first-run flow.

const PARAM = "template";

/** If the URL carries `?template=<id>` and it matches a known template, return a
 *  copy of that template's bundle. The caller derives + Dagre-lays-out the model,
 *  exactly like any freshly loaded bundle. Missing or unknown id → null (so the
 *  bootstrap falls back to shared link / localStorage / welcome). */
export function readTemplateModel(): Bundle | null {
  const id = new URLSearchParams(location.search).get(PARAM);
  if (!id) return null;
  const t = TEMPLATES.find((tpl) => tpl.id === id);
  return t ? t.bundle.map(([p, m]) => [p, m] as [string, string]) : null;
}

/** Strip the `template` param from the address bar after loading, so a refresh
 *  doesn't re-clobber the canvas. UTM params and the hash are preserved — the
 *  UTMs are captured by analytics at page load, and the hash may carry a share
 *  link. Safe to call unconditionally: no-ops when the param isn't present. */
export function clearTemplateFromUrl(): void {
  const params = new URLSearchParams(location.search);
  if (!params.has(PARAM)) return;
  params.delete(PARAM);
  const qs = params.toString();
  history.replaceState(null, "", location.pathname + (qs ? `?${qs}` : "") + location.hash);
}
