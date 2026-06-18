import { describe, it, expect, vi } from "vitest";
import { pushModel } from "../src/sync/push";
import { createModelStore } from "../src/state/model";

describe("pushModel", () => {
  it("creates pending nodes, stores owoxId, sets created", async () => {
    const s = createModelStore({ storageId: "stor_1" });
    s.addNode({ x: 0, y: 0 });
    const calls: string[] = [];
    const apiMock = vi.fn(async (path: string) => { calls.push(path); return { id: "owox_a" }; });
    const res = await pushModel(s, apiMock as any);
    expect(calls).toContain("/api/data-marts");
    expect(s.get().nodes[0].status).toBe("created");
    expect(s.get().nodes[0].owoxId).toBe("owox_a");
    expect(res.created).toBe(1); expect(res.failed).toBe(0);
  });
  it("marks a node error on failure and counts it", async () => {
    const s = createModelStore({ storageId: "stor_1" }); s.addNode({ x: 0, y: 0 });
    const apiMock = vi.fn(async () => { throw new Error("boom"); });
    const res = await pushModel(s, apiMock as any);
    expect(s.get().nodes[0].status).toBe("error");
    expect(res.failed).toBe(1);
  });
});
