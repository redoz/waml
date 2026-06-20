import { describe, it, expect, vi, beforeEach } from "vitest";
import { buildApp } from "../src/app";
import * as client from "../src/owox/client";

const KEY = "owox_key_" + Buffer.from(JSON.stringify({ apiOrigin: "https://app.owox.com", apiKeyId: "k", apiKeySecret: "s" })).toString("base64url");

beforeEach(() => {
  vi.spyOn(client, "exchangeToken").mockResolvedValue("tok");
  vi.spyOn(client, "decodeProjectFromToken").mockReturnValue({ projectTitle: "Demo", fullName: "Vlad" });
});

describe("auth", () => {
  it("connect sets a session cookie and /me returns identity", async () => {
    const app = buildApp();
    const connect = await app.inject({ method: "POST", url: "/api/auth/connect", payload: { apiKey: KEY } });
    expect(connect.statusCode).toBe(200);
    const cookie = connect.cookies[0];
    expect(cookie.name).toBe("mc_sid");
    const me = await app.inject({ method: "GET", url: "/api/me", cookies: { mc_sid: cookie.value } });
    expect(me.json()).toMatchObject({ projectTitle: "Demo", fullName: "Vlad" });
  });
  it("/me without session is 401", async () => {
    const app = buildApp();
    expect((await app.inject({ method: "GET", url: "/api/me" })).statusCode).toBe(401);
  });
  it("rate-limits /api/auth/connect after 10 requests/min from one IP", async () => {
    const app = buildApp();
    // A malformed key fails fast at parseApiKey (400, no network), but each
    // request still counts toward the limiter, so the 11th must be 429.
    const codes: number[] = [];
    for (let i = 0; i < 11; i++) {
      codes.push((await app.inject({ method: "POST", url: "/api/auth/connect", payload: { apiKey: "bad" } })).statusCode);
    }
    expect(codes.slice(0, 10).every((c) => c === 400)).toBe(true);
    expect(codes[10]).toBe(429);
  });
});
