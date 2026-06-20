# OWOX Model Canvas

A Miro-like canvas where drawing a data model creates draft OWOX Data Marts and joinable relationships via the OWOX API, with OKF import/export. A free, **anonymous-first** lead-gen tool: the whole canvas works without signing in — an OWOX API key is needed only to **Push** the model into OWOX.

See `docs/superpowers/specs/` for the design and `docs/superpowers/plans/` for the implementation plan. The approved interaction/visual reference is `docs/superpowers/prototype/canvas.html`.

## Layout (pnpm monorepo)

- `packages/okf` — pure shared lib: `ModelGraph` ⇄ OKF markdown bundle (parse/serialize). No I/O.
- `packages/server` — Fastify BFF: holds the OWOX access token in a cookie-keyed in-memory session, proxies all OWOX HTTP, serves the built SPA. Hardened with CSP/security headers, per-IP rate limiting, an `apiOrigin` allowlist, and session TTL/cap (see **Security**).
- `packages/web` — React + Vite + React Flow SPA: canvas, ERD view toggle, inspector, OKF import/export, template library, optional OWOX sign-in (only gated on Push).

## Develop

```bash
pnpm install
pnpm --filter @mc/okf build      # web/server consume okf's built dist
pnpm dev:web                     # Vite dev server (SPA) on :5173
pnpm dev                         # BFF (tsx watch) on :3000 — proxy/serve API
```

For a quick integrated check: `pnpm build` then `PORT=3111 pnpm --filter @mc/server start`, open http://localhost:3111 — the canvas opens immediately (anonymous). Pressing **Push to OWOX** opens the sign-in modal asking for your OWOX API key.

## Test

```bash
pnpm -r test     # okf + server + web (Vitest)
```

## Deploy (Render)

One Web Service, Node runtime, `starter` plan (`render.yaml`, `autoDeploy` from `main`).

- **Build:** `corepack enable && pnpm install --frozen-lockfile && pnpm build` (builds `okf` + `web`, typechecks `server`).
- **Start:** `pnpm --filter @mc/server start` (runs `src/server.ts` via `tsx` — chosen over compiled JS to avoid Node-ESM extension resolution friction; `tsx` is a runtime dependency).
- **Health check:** `/` (serves the SPA).
- **Env:** `PORT` (provided by Render). No secrets at rest — the OWOX API key is supplied by the user at runtime and held only in the BFF session. Tunable knobs: `RATE_LIMIT_MAX`, `RATE_LIMIT_WINDOW`, `CONNECT_RATE_LIMIT_MAX`, `OWOX_ALLOWED_ORIGIN_SUFFIXES`, `SESSION_TTL_MS`, `MAX_SESSIONS`.

## Auth

Auth is **optional** — only **Push** needs it. To connect, create an OWOX API key (in OWOX: project menu → **Project settings** → **My API Keys** → **Create API Key**) and paste it into the sign-in modal. The BFF exchanges it for a short-lived access token kept in a cookie-keyed session; the key itself is stored in the browser's `localStorage` for convenience. **Sign out** (top bar) clears it.

## Security

The BFF is a thin proxy in front of the OWOX API. Hardening:

- **CSP + security headers** (`@fastify/helmet`): `script-src 'self'` (no inline scripts in the build), HSTS, `X-Content-Type-Options`, `frame-ancestors 'none'`.
- **Per-IP rate limiting** (`@fastify/rate-limit`): a generous global cap plus a tight cap on `/api/auth/connect` (the only endpoint that triggers an outbound OWOX token exchange).
- **`apiOrigin` allowlist**: the origin embedded in a user-supplied key must be an `https` `owox.com` host, blocking SSRF to internal/metadata addresses.
- **Session TTL + hard cap**: in-memory sessions expire and are bounded so abuse can't exhaust memory.

Per-session tenant isolation: every OWOX call uses the caller's own token, so users never see each other's data. Known tradeoff: the OWOX key lives in `localStorage`, so an XSS would expose it — CSP is the primary mitigation.
