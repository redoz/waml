import { toSvg } from "html-to-image";

// Export the whole model (not just the visible viewport) as an SVG, with a minimal
// WAML watermark in the bottom-right corner. Capturing the flow viewport
// element with an overridden transform renders every node at 1:1 regardless of
// the user's current pan/zoom.
//
// SVG (not PNG): the flow renderer renders nodes as HTML, so html-to-image wraps
// them in an SVG <foreignObject>. Rasterizing that to PNG taints the canvas in
// Chromium (a security rule) and toDataURL/toPng then hangs/throws. SVG sidesteps
// that and is the better format for a diagram anyway — vector, crisp at any size.

const PAD = 60; // px of breathing room around the model

export type BoundsNode = {
  position: { x: number; y: number };
  measured?: { width?: number | null; height?: number | null };
  width?: number | null;
  height?: number | null;
};

// Bounding box over the node array — framework-free replacement for
// @xyflow/react's getNodesBounds. Nodes carry a measured size once rendered
// (`measured`), falling back to any static width/height, else the layout default.
const DEF_W = 200, DEF_H = 90; // matches runDagreLayout NODE_W/NODE_H
function nodesBounds(nodes: BoundsNode[]): { x: number; y: number; width: number; height: number } {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const n of nodes) {
    const w = n.measured?.width ?? n.width ?? DEF_W;
    const h = n.measured?.height ?? n.height ?? DEF_H;
    minX = Math.min(minX, n.position.x);
    minY = Math.min(minY, n.position.y);
    maxX = Math.max(maxX, n.position.x + w);
    maxY = Math.max(maxY, n.position.y + h);
  }
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

// WAML wordmark glyph paths (100-unit-tall grid; laid out left→right with the
// same translate offsets as the TopBar wordmark). Rendered flat brand-blue.
const GLYPH_U = "M 0,0 H 25 V 75 H 55 V 0 H 80 V 85 L 65,100 H 15 L 0,85 Z";
const GLYPH_A = "M 0,100 V 15 L 15,0 H 65 L 80,15 V 100 H 55 V 65 H 25 V 100 Z M 25,25 H 55 V 40 H 25 Z";
const GLYPH_M = "M 0,100 V 0 H 25 L 50,40 L 75,0 H 100 V 100 H 75 V 45 L 50,75 L 25,45 V 100 Z";
const GLYPH_L = "M 0,0 H 25 V 75 H 80 V 85 L 65,100 H 15 L 0,85 Z";

const WM_H = 18;
const WM_W = 72; // wordmark spans ~400 glyph units wide × 100 tall → 72×18 at this height
// Watermark as an SVG <g> WM_W×WM_H: the WAML wordmark in flat brand blue.
function watermarkGroup(x: number, y: number): string {
  const scale = WM_H / 100; // glyphs are 100 units tall
  return (
    `<g transform="translate(${x},${y})" opacity="0.9" fill="#0046F9">` +
    `<g transform="scale(${scale})">` +
    `<path d="${GLYPH_U}" transform="translate(0,0)"/>` +
    `<path fill-rule="evenodd" d="${GLYPH_A}" transform="translate(100,0)"/>` +
    `<path d="${GLYPH_M}" transform="translate(200,0)"/>` +
    `<path d="${GLYPH_L}" transform="translate(320,0)"/>` +
    `</g></g>`
  );
}

function captureOptions(rfNodes: BoundsNode[]) {
  const bounds = nodesBounds(rfNodes);
  const width = Math.ceil(bounds.width) + PAD * 2;
  const height = Math.ceil(bounds.height) + PAD * 2;
  // Translate so the model's top-left lands at (PAD, PAD); no scaling (1:1).
  const transform = `translate(${PAD - bounds.x}px, ${PAD - bounds.y}px) scale(1)`;
  return { width, height, style: { width: `${width}px`, height: `${height}px`, transform } };
}

export type CanvasSvg = { svg: string; width: number; height: number };

/**
 * Build the model's SVG markup (whole model, transparent background, WAML
 * watermark bottom-right) without touching the DOM to download it. Returns null
 * when there's nothing to export (no viewport element or empty diagram).
 *
 * html-to-image's `toSvg` clones the flow viewport and inlines every element's
 * *computed* CSS into the SVG's <foreignObject>, so colours/borders/layout are
 * baked in (the raster is not unstyled). `skipFonts: true` only skips embedding
 * @font-face web fonts — the app renders in the system font stack, so text still
 * rasterizes styled. This is the SVG string a PNG rasterizer can draw onto a
 * canvas (see @waml/web src/share/rasterize.ts).
 */
export async function buildCanvasSvg(rfNodes: BoundsNode[], viewportSelector: string): Promise<CanvasSvg | null> {
  const el = document.querySelector<HTMLElement>(viewportSelector);
  if (!el || rfNodes.length === 0) return null;
  const { width, height, style } = captureOptions(rfNodes);
  // Don't pass `backgroundColor` here: we want a transparent background so the
  // exported SVG drops cleanly onto any canvas. (html-to-image would otherwise
  // paint the fill on the translated viewport <div>, offsetting it anyway.)
  const dataUrl = await toSvg(el, { width, height, style, skipFonts: true });
  // toSvg returns a data: URI — decode, then inject the watermark before </svg>.
  const raw = decodeURIComponent(dataUrl.replace(/^data:image\/svg\+xml;charset=utf-8,/, ""));
  const wm = watermarkGroup(width - WM_W - 14, height - WM_H - 14);
  const svg = raw.replace(/<\/svg>\s*$/, `${wm}</svg>`);
  return { svg, width, height };
}

/** Export the model as an SVG with the WAML watermark embedded bottom-right. */
export async function exportCanvasSvg(rfNodes: BoundsNode[], filename = "model", viewportSelector: string): Promise<void> {
  const built = await buildCanvasSvg(rfNodes, viewportSelector);
  if (!built) return;
  const blob = new Blob([built.svg], { type: "image/svg+xml" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${filename}.svg`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
