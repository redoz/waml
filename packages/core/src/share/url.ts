import { gzipSync, gunzipSync, strToU8, strFromU8 } from "fflate";
import { split_bundle } from "@waml/wasm";
import type { Bundle } from "../state/model";

// Shareable model links. The whole bundle is gzip-compressed and packed into the
// URL hash (#m=…) — no backend, fully anonymous, and the hash never leaves the
// browser for the server. Opening the link reopens the exact model. Every
// shared/forked model is a backlink and an impression: the growth loop for a
// free tool.
//
// The payload is the bundle joined into the multi-document string that the Rust
// core's `split_bundle` reads back: each doc is preceded by an
// `<!-- path/slug.md -->` marker line (see `crates/waml/src/parse.rs::split_bundle`).

const HASH_KEY = "m";
const NAME_KEY = "n";

// The compressed Orders-Domain payload must fit a comfortable URL-hash ceiling.
// Browsers/CDNs tolerate multi-KB URLs; we keep well under 8 KB of hash so the
// link stays paste-safe everywhere (see url.test.ts, which asserts the headroom).
export const SHARE_URL_HASH_CEILING = 8000;

/** Join a bundle into the `split_bundle` multi-document string. Each doc is
 *  emitted behind an HTML-comment path marker; a doc is normalized to end in a
 *  newline so the following marker starts at column 0 (the marker regex is
 *  line-anchored). */
function joinBundle(bundle: Bundle): string {
  return bundle.map(([path, md]) => `<!-- ${path} -->\n${md.endsWith("\n") ? md : md + "\n"}`).join("");
}

function bytesToB64url(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function b64urlToBytes(s: string): Uint8Array {
  const b64 = s.replace(/-/g, "+").replace(/_/g, "/") + "===".slice((s.length + 3) % 4);
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Compress a bundle into a compact, URL-safe payload string. */
export function encodeModel(bundle: Bundle): string {
  return bytesToB64url(gzipSync(strToU8(joinBundle(bundle)), { level: 9 }));
}

/** Reverse of encodeModel: gunzip → split into `[path, markdown][]`. Returns null
 *  on any malformed/corrupt payload. */
export function decodeModel(payload: string): Bundle | null {
  try {
    const text = strFromU8(gunzipSync(b64urlToBytes(payload)));
    if (!text) return null;
    return split_bundle(text) as Bundle;
  } catch {
    return null;
  }
}

/** Full shareable URL for the current page that reopens `bundle`. */
export function buildShareUrl(bundle: Bundle, name?: string): string {
  // The model name isn't part of the bundle, so carry it alongside the payload as
  // a separate hash param — the recipient opens the model under the sender's name.
  const namePart = name && name.trim() ? `&${NAME_KEY}=${encodeURIComponent(name.trim())}` : "";
  return `${location.origin}${location.pathname}#${HASH_KEY}=${encodeModel(bundle)}${namePart}`;
}

/** If the current URL carries a shared model, decode it into a bundle; else null. */
export function readSharedModel(): Bundle | null {
  const match = new RegExp(`[#&]${HASH_KEY}=([^&]+)`).exec(location.hash);
  return match ? decodeModel(match[1]) : null;
}

/** The model name carried in a shared link, if any. */
export function readSharedName(): string | null {
  const match = new RegExp(`[#&]${NAME_KEY}=([^&]+)`).exec(location.hash);
  if (!match) return null;
  try {
    return decodeURIComponent(match[1]);
  } catch {
    return null;
  }
}

/** Strip the shared-model payload from the address bar (after we've loaded it),
 *  so a refresh doesn't re-clobber the canvas and the URL stays clean. */
export function clearSharedModelFromUrl(): void {
  if (new RegExp(`[#&]${HASH_KEY}=`).test(location.hash)) {
    history.replaceState(null, "", location.pathname + location.search);
  }
}
