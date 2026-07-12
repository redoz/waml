import { describe, it, expect, beforeEach } from "vitest";
import { readTemplateModel, clearTemplateFromUrl } from "./templateLink";

const setUrl = (url: string) => history.replaceState(null, "", url);

beforeEach(() => setUrl("/"));

describe("readTemplateModel", () => {
  it("loads a known template by id (as a bundle)", () => {
    setUrl("/?template=uml_orders_domain");
    const b = readTemplateModel();
    expect(b).not.toBeNull();
    expect(b!.some(([p]) => p.endsWith("order.md"))).toBe(true);
    expect(b!.length).toBeGreaterThan(0);
  });

  it("returns a fresh copy each call (the source template is never mutated)", () => {
    setUrl("/?template=uml_orders_domain");
    const a = readTemplateModel()!;
    a[0][1] = "MUTATED";
    const b = readTemplateModel()!;
    expect(b[0][1]).not.toBe("MUTATED");
  });

  it("returns null for an unknown id", () => {
    setUrl("/?template=does-not-exist");
    expect(readTemplateModel()).toBeNull();
  });

  it("returns null when no template param is present", () => {
    setUrl("/?utm_source=newsletter");
    expect(readTemplateModel()).toBeNull();
  });
});

describe("clearTemplateFromUrl", () => {
  it("removes only the template param, preserving UTM params and the hash", () => {
    setUrl("/?template=uml_orders_domain&utm_source=newsletter#m=abc");
    clearTemplateFromUrl();
    expect(location.search).toBe("?utm_source=newsletter");
    expect(location.hash).toBe("#m=abc");
  });

  it("is a no-op when there is no template param", () => {
    setUrl("/?utm_source=x");
    clearTemplateFromUrl();
    expect(location.search).toBe("?utm_source=x");
  });
});
