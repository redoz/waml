import { describe, it, expect, beforeAll } from "vitest";
import { initWasm } from "@waml/wasm";
import { encodeModel, decodeModel, buildShareUrl, readSharedName, SHARE_URL_HASH_CEILING } from "./url";
import { ordersDomainBundle } from "../templates/orders-domain.bundle";
import type { Bundle } from "../state/model";

beforeAll(async () => {
  await initWasm();
});

const bundle: Bundle = [
  [
    "m/order.md",
    "---\ntype: uml.Class\ntitle: Order\n---\n\n# Order\n\n## Attributes\n- id: OrderId\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n",
  ],
  ["m/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n\n# Customer\n"],
];

describe("share url", () => {
  it("round-trips a bundle through encode/decode (URL-safe, identity)", () => {
    const payload = encodeModel(bundle);
    expect(payload).toMatch(/^[A-Za-z0-9_-]+$/); // url-safe, no +/=
    const back = decodeModel(payload)!;
    expect(back).toEqual(bundle);
  });

  it("returns null for a corrupt payload", () => {
    expect(decodeModel("not-a-real-payload")).toBeNull();
    expect(decodeModel("")).toBeNull();
  });

  it("carries the model name in the link and reads it back", () => {
    const url = buildShareUrl(bundle, "My SaaS / Subscription model");
    expect(url).toContain("&n=");
    history.replaceState(null, "", url.slice(url.indexOf("#")));
    expect(readSharedName()).toBe("My SaaS / Subscription model");
  });

  it("omits the name param when no name is given, and reads null", () => {
    const url = buildShareUrl(bundle);
    expect(url).not.toContain("&n=");
    history.replaceState(null, "", url.slice(url.indexOf("#")));
    expect(readSharedName()).toBeNull();
  });

  it("the Orders Domain payload fits the URL-hash ceiling", () => {
    // A comfortable headroom bound so shared links stay paste-safe everywhere.
    const len = encodeModel(ordersDomainBundle).length;
    expect(len).toBeLessThan(SHARE_URL_HASH_CEILING);
  });
});
