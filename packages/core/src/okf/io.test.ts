import { describe, it, expect } from "vitest";
import { bundleToZip, zipToFiles, bundleToDownloadFiles } from "./io";

describe("zip round-trip", () => {
  it("zips and unzips bundle files losslessly", () => {
    const files = { "demo/index.md": "# Demo\n", "demo/orders.md": "# Orders\n" };
    const buf = bundleToZip(files);
    expect(buf).toBeInstanceOf(Uint8Array);
    expect(zipToFiles(buf)).toEqual(files);
  });
});

describe("bundleToDownloadFiles", () => {
  it("appends a WAML attribution footer to the index only", () => {
    const bundle: [string, string][] = [
      ["demo/index.md", "# Demo\n"],
      ["demo/orders.md", "# Orders\n"],
    ];
    const files = bundleToDownloadFiles(bundle, "Demo");
    expect(files["demo/index.md"]).toContain("Generated with");
    expect(files["demo/index.md"]).toContain("WAML");
    expect(files["demo/index.md"]).toContain("github.com/redoz/waml");
    expect(files["demo/orders.md"]).not.toContain("Generated with"); // per-doc stays clean
  });

  it("synthesizes an index doc when the bundle has none", () => {
    const bundle: [string, string][] = [["orders-domain-uml/order.md", "# Order\n"]];
    const files = bundleToDownloadFiles(bundle, "Orders Domain");
    expect(files["index.md"]).toContain("# Orders Domain");
    expect(files["index.md"]).toContain("Generated with");
    expect(files["orders-domain-uml/order.md"]).toBe("# Order\n"); // original docs untouched
  });
});
