# Contributing to OKF Canvas

Thanks for your interest! OKF Canvas is a free, open-source, in-browser visual editor for data models in the **Open Knowledge Format (OKF)**. Bug reports, fixes, templates, and OKF-compatibility improvements are all welcome.

By contributing, you agree that your contributions are licensed under the project's [Apache License 2.0](LICENSE), and that you follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Project layout (pnpm monorepo)

The repo has exactly two packages — there is no backend:

- `packages/okf` — pure shared lib: `ModelGraph` ⇄ OKF Markdown bundle (parse/serialize). No I/O.
- `packages/web` — React + Vite + React Flow SPA: the canvas, ERD view, inspector, template library, OKF import/export, URL sharing, and `localStorage` persistence. Consumes `okf`'s built `dist/`.

## Local setup

```bash
corepack pnpm install
corepack pnpm --filter @mc/okf build   # web consumes okf's built dist — build okf first
corepack pnpm --filter @mc/web dev      # Vite dev server on :5173
```

Open http://localhost:5173. The canvas is fully client-side — no server, no sign-in.

To check the production build:

```bash
corepack pnpm --filter @mc/web build     # emits packages/web/dist
corepack pnpm --filter @mc/web preview    # serves the built dist/
```

## Tests & checks

```bash
corepack pnpm -r test    # okf + web (Vitest)
corepack pnpm lint       # eslint
corepack pnpm build      # builds okf, then web
```

Please make sure tests, lint and the build pass before opening a PR. Add or update tests for behavior changes — the OKF parser/serializer in `packages/okf` is well covered, and new parsing rules should come with a fixture.

## Pull requests

- Branch from `main` and keep each PR focused on one change.
- `main` is protected — changes land via PR and review, not direct pushes.
- Use clear, conventional commit subjects, e.g. `feat(web): …`, `fix(okf): …`, `docs: …`, `chore: …`.
- Match the surrounding code's style and comment density. The build has a strict CSP (`script-src 'self'`), so **no inline scripts/handlers** — use external files.
- For UI changes, include before/after notes or screenshots.

## Working with OKF

The format the app reads and writes is documented at [`packages/web/public/okf-format.md`](packages/web/public/okf-format.md) (served live at `/okf-format.md`). If you're improving import compatibility (e.g. with Google's OKF v0.1 bundles), keep changes **additive** — don't change how the app exports its own bundles, and keep the export↔import round-trip stable.

## Reporting bugs & ideas

Open a GitHub issue with steps to reproduce (a shared model link or a small OKF snippet helps a lot). For **security vulnerabilities**, do **not** open a public issue — see [SECURITY.md](SECURITY.md).
