import { randomUUID } from "node:crypto";
import { OwoxClient } from "../owox/client";
export interface Session { origin: string; token: string; keyId: string; projectTitle?: string; fullName?: string; }

// Sessions live in memory only (single Render instance). Bound them so abuse of
// /api/auth/connect can't grow the map until the 512 MB instance OOMs, and so
// stale entries don't accumulate forever.
const TTL_MS = Number(process.env.SESSION_TTL_MS) || 12 * 60 * 60 * 1000; // 12h
const MAX_SESSIONS = Number(process.env.MAX_SESSIONS) || 5000;

interface Entry { session: Session; expiresAt: number; }
const store = new Map<string, Entry>();

function sweepExpired(now: number) {
  for (const [id, e] of store) if (e.expiresAt <= now) store.delete(id);
}

export function createSession(s: Session): string {
  const now = Date.now();
  sweepExpired(now);
  // Hard cap: Map preserves insertion order, so evict the oldest entries first.
  while (store.size >= MAX_SESSIONS) {
    const oldest = store.keys().next().value;
    if (oldest === undefined) break;
    store.delete(oldest);
  }
  const id = randomUUID();
  store.set(id, { session: s, expiresAt: now + TTL_MS });
  return id;
}

export function getSession(id?: string): Session | undefined {
  if (!id) return undefined;
  const e = store.get(id);
  if (!e) return undefined;
  if (e.expiresAt <= Date.now()) { store.delete(id); return undefined; } // lazy expiry
  return e.session;
}

export function dropSession(id?: string) { if (id) store.delete(id); }
export function clientFor(s: Session) { return new OwoxClient(s.origin, s.token, s.keyId); }
