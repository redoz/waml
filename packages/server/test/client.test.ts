import { describe, it, expect, vi } from "vitest";
import { parseApiKey, exchangeToken, OwoxClient } from "../src/owox/client";
import list from "./fixtures/owox-list.json";
import detail from "./fixtures/owox-detail.json";
import graph from "./fixtures/owox-relationships-graph.json";

const KEY = "owox_key_" + Buffer.from(JSON.stringify({
  apiOrigin: "https://app.owox.com", apiKeyId: "kid_1", apiKeySecret: "sec_1",
})).toString("base64url");

const keyFor = (apiOrigin: string) =>
  "owox_key_" + Buffer.from(JSON.stringify({ apiOrigin, apiKeyId: "kid_1", apiKeySecret: "sec_1" })).toString("base64url");

describe("parseApiKey", () => {
  it("decodes origin/id/secret", () =>
    expect(parseApiKey(KEY)).toEqual({ apiOrigin: "https://app.owox.com", apiKeyId: "kid_1", apiKeySecret: "sec_1" }));
  it("rejects malformed keys", () => expect(() => parseApiKey("nope")).toThrow());
  // SSRF guard: apiOrigin must be an https owox.com host.
  it("rejects a non-owox apiOrigin", () => expect(() => parseApiKey(keyFor("https://evil.com"))).toThrow(/allowed OWOX host/));
  it("rejects an owox look-alike host", () => expect(() => parseApiKey(keyFor("https://evilowox.com"))).toThrow(/allowed OWOX host/));
  it("rejects the cloud metadata IP", () => expect(() => parseApiKey(keyFor("http://169.254.169.254"))).toThrow());
  it("rejects http (non-tls) origins", () => expect(() => parseApiKey(keyFor("http://app.owox.com"))).toThrow(/https/));
  it("accepts the apex and subdomains of owox.com", () => {
    expect(parseApiKey(keyFor("https://owox.com")).apiOrigin).toBe("https://owox.com");
    expect(parseApiKey(keyFor("https://app.owox.com")).apiOrigin).toBe("https://app.owox.com");
  });
});

describe("exchangeToken", () => {
  it("posts secret and returns the access token", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ accessToken: "tok_1" }), { status: 200 }));
    const tok = await exchangeToken(parseApiKey(KEY), fetchMock as any);
    expect(tok).toBe("tok_1");
    expect(fetchMock).toHaveBeenCalledWith("https://app.owox.com/api/auth/api-keys/exchange",
      expect.objectContaining({ method: "POST" }));
  });
});

describe("OwoxClient.listDataMarts", () => {
  it("pages until nextOffset is null", async () => {
    const pages = [
      new Response(JSON.stringify({ items: [{ id: "a" }], nextOffset: 1 }), { status: 200 }),
      new Response(JSON.stringify({ items: [{ id: "b" }], nextOffset: null }), { status: 200 }),
    ];
    const fetchMock = vi.fn(async () => pages.shift()!);
    const c = new OwoxClient("https://app.owox.com", "tok_1", "kid_1", fetchMock as any);
    expect((await c.listDataMarts()).map(m => m.id)).toEqual(["a", "b"]);
  });

  it("sends both x-owox-authorization and X-OWOX-Api-Key-Id headers", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ items: [], nextOffset: null }), { status: 200 }));
    const c = new OwoxClient("https://app.owox.com", "tok_1", "kid_1", fetchMock as any);
    await c.listDataMarts();
    const headers = (fetchMock.mock.calls[0][1] as any).headers;
    expect(headers["x-owox-authorization"]).toBe("Bearer tok_1");
    expect(headers["X-OWOX-Api-Key-Id"]).toBe("kid_1");
  });
});

describe("OwoxClient read methods", () => {
  const clientWith = (body: unknown) => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify(body), { status: 200 }));
    return { c: new OwoxClient("https://app.owox.com", "tok", "kid", fetchMock as any), fetchMock };
  };

  it("getImportMart normalizes schema (incl. pk/alias/description) + SQL definition", async () => {
    const { c } = clientWith(detail);
    const m = await c.getImportMart(detail.id);
    expect(m).toMatchObject({ id: detail.id, title: detail.title, inputSource: "SQL" });
    expect(m.description).toContain("Demo guide"); // mart-level description must be imported
    expect(m.definition).toContain("SELECT");
    // session_id is the primary key in the fixture
    expect(m.schema.find(f => f.name === "session_id")).toMatchObject({ type: "STRING", pk: true });
    expect(m.schema.find(f => f.name === "date")).toMatchObject({ pk: false });
  });

  it("getRelationshipGraph maps nodes to source/target ids, skips cycle stubs, dedupes by id", async () => {
    const { c } = clientWith(graph);
    const out = await c.getRelationshipGraph(graph.rootDataMartId);
    // fixture: 2 direct (depth 1) edges + 1 isCycleStub:true node → stub dropped
    expect(out).toHaveLength(2);
    expect(out[0]).toEqual({
      sourceId: "b4f59656-d52e-4ae3-847e-c34c025956bf",
      targetId: "61b3c045-b334-440d-9913-bf52bc622af4",
      joinConditions: [{ sourceFieldName: "traffic_source_id", targetFieldName: "traffic_source_id" }],
    });
  });

  it("listDataMartsForStorage matches on storage title + type", async () => {
    const { c } = clientWith(list);
    const out = await c.listDataMartsForStorage("BigQuery [Common]", "GOOGLE_BIGQUERY");
    expect(out.map(m => m.id)).toEqual([
      "d57170ef-8de5-4475-bbfb-61b20a72b051",
      "b4f59656-d52e-4ae3-847e-c34c025956bf",
    ]);
  });
});
