import { describe, it, expect } from "vitest";
import { reconcileStorageId } from "./storageReconcile";

const list = [{ id: "a" }, { id: "b" }];

describe("reconcileStorageId", () => {
  it("keeps the current selection when it still exists", () =>
    expect(reconcileStorageId("b", list)).toBe("b"));
  it("falls back to the first storage when the current one is gone (project switch)", () =>
    expect(reconcileStorageId("stale", list)).toBe("a"));
  it("picks the first storage when nothing is selected yet", () =>
    expect(reconcileStorageId(null, list)).toBe("a"));
  it("returns null when there are no storages", () =>
    expect(reconcileStorageId("a", [])).toBeNull());
});
