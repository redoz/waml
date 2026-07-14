// Rasterize an SVG string to a PNG Blob for the "Share as image" flow.
//
// Pipeline (the spec's recommended, no-new-dependency approach): the diagram SVG
// — produced by @waml/core buildCanvasSvg (html-to-image, styles inlined) — is
// loaded into an <img> from a data URL, drawn onto an <canvas>, and read back via
// canvas.toBlob(…, "image/png").
//
// NOTE (runtime caveat): the flow renders nodes as HTML wrapped in an SVG
// <foreignObject>. In Chromium, drawing a <foreignObject>-bearing SVG onto a
// canvas can mark it origin-unclean and make toBlob throw — which is exactly why
// the plain SVG export exists. This rasterizer therefore surfaces failures
// (rejects) so the UI can fall back to the SVG download / manual right-click.
//
// The browser primitives are injected (createImage/createCanvas) so the control
// flow is unit-testable under jsdom, which implements neither SVG decoding nor
// canvas rasterization.

/** Hard cap on the longest raster edge — keeps huge diagrams from allocating a
 *  multi-hundred-megapixel canvas (memory / browser limits). */
export const MAX_RASTER_DIM = 4096;

export type RasterizeOptions = {
  /** Natural width of the diagram bounds (px). */
  width: number;
  /** Natural height of the diagram bounds (px). */
  height: number;
  /** Override the max raster edge (defaults to MAX_RASTER_DIM). */
  maxDimension?: number;
  /** Injectable image factory (defaults to a real HTMLImageElement). */
  createImage?: () => HTMLImageElement;
  /** Injectable canvas factory (defaults to a real <canvas>). */
  createCanvas?: () => HTMLCanvasElement;
};

/** Encode an SVG string as a UTF-8 data URL an <img> can load. */
export function svgToDataUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** Draw an SVG string onto a canvas and return an `image/png` Blob. */
export async function svgToPngBlob(svg: string, opts: RasterizeOptions): Promise<Blob> {
  const maxDim = opts.maxDimension ?? MAX_RASTER_DIM;
  // Scale down (never up) so the longest edge fits under the cap; keep aspect.
  const longest = Math.max(opts.width, opts.height) || 1;
  const scale = Math.min(1, maxDim / longest);
  const w = Math.max(1, Math.round(opts.width * scale));
  const h = Math.max(1, Math.round(opts.height * scale));

  const img = (opts.createImage ?? (() => new Image()))();
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve();
    img.onerror = () => reject(new Error("Failed to load SVG for rasterization"));
    img.src = svgToDataUrl(svg);
  });

  const canvas = (opts.createCanvas ?? (() => document.createElement("canvas")))();
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("2D canvas context unavailable");
  ctx.drawImage(img, 0, 0, w, h);

  return await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (blob) => (blob ? resolve(blob) : reject(new Error("Canvas produced no image data"))),
      "image/png",
    );
  });
}
