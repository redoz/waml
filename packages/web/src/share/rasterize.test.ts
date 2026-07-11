import { test, expect } from "vitest";
import { svgToPngBlob, MAX_RASTER_DIM } from "./rasterize";

// jsdom implements neither SVG image decoding nor canvas rasterization, so the
// browser primitives are injected as fakes. This exercises the pure control flow
// of svgToPngBlob: data-url plumbing, dimension capping, and the toBlob contract.
function fakeImage(): HTMLImageElement {
  const img = {} as HTMLImageElement & { src: string };
  Object.defineProperty(img, "src", {
    set() {
      // Emulate a successful decode on the next microtask.
      queueMicrotask(() => img.onload?.(new Event("load")));
    },
  });
  return img;
}

function fakeCanvas(track?: { w: number; h: number }): HTMLCanvasElement {
  const canvas = {
    set width(v: number) {
      if (track) track.w = v;
    },
    set height(v: number) {
      if (track) track.h = v;
    },
    getContext: () => ({ drawImage: () => {} }),
    toBlob: (cb: BlobCallback) => cb(new Blob([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], { type: "image/png" })),
  } as unknown as HTMLCanvasElement;
  return canvas;
}

test("SVG → PNG produces a non-empty image/png blob for a sample diagram", async () => {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="red"/></svg>`;
  const blob = await svgToPngBlob(svg, {
    width: 200,
    height: 100,
    createImage: fakeImage,
    createCanvas: () => fakeCanvas(),
  });
  expect(blob).toBeInstanceOf(Blob);
  expect(blob.type).toBe("image/png");
  expect(blob.size).toBeGreaterThan(0);
});

test("caps the raster dimension for very large diagrams", async () => {
  const track = { w: 0, h: 0 };
  await svgToPngBlob("<svg/>", {
    width: MAX_RASTER_DIM * 3,
    height: MAX_RASTER_DIM,
    createImage: fakeImage,
    createCanvas: () => fakeCanvas(track),
  });
  expect(Math.max(track.w, track.h)).toBeLessThanOrEqual(MAX_RASTER_DIM);
  // Aspect ratio preserved: the wider side is capped, the shorter scales down.
  expect(track.w).toBeGreaterThan(track.h);
});

test("rejects when the SVG image fails to decode", async () => {
  const failingImage = (): HTMLImageElement => {
    const img = {} as HTMLImageElement & { src: string };
    Object.defineProperty(img, "src", {
      set() {
        queueMicrotask(() => img.onerror?.(new Event("error")));
      },
    });
    return img;
  };
  await expect(
    svgToPngBlob("<svg/>", { width: 10, height: 10, createImage: failingImage, createCanvas: () => fakeCanvas() }),
  ).rejects.toThrow();
});
