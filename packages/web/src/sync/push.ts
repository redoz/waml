import type { ModelStore } from "../state/model";
import { api as defaultApi } from "../lib/api";

type Api = typeof defaultApi;

export interface PushResult {
  created: number;
  updated: number;
  failed: number;
}

export async function pushModel(store: ModelStore, api: Api = defaultApi): Promise<PushResult> {
  const res: PushResult = { created: 0, updated: 0, failed: 0 };
  const g = store.get();

  for (const n of g.nodes) {
    if (n.status === "created") continue;

    store.updateNode(n.key, { status: "creating", error: null });

    try {
      const out = await api<{ id: string }>("/api/data-marts", {
        method: "POST",
        body: JSON.stringify({
          title: n.title,
          storageId: g.storageId,
          description: n.description,
          schema: {
            fields: n.schema.map(f => ({ name: f.name, type: f.type, isPrimaryKey: f.pk })),
          },
        }),
      });

      store.updateNode(n.key, {
        status: "created",
        owoxId: out.id,
        createdAt: new Date().toISOString(),
      });
      res.created++;
    } catch (e) {
      store.updateNode(n.key, { status: "error", error: (e as Error).message });
      res.failed++;
    }
  }

  // Relationships are created only once the write endpoint is confirmed (R-OPEN-1);
  // until then they round-trip via OKF only.
  return res;
}
