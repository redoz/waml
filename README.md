# OKF Canvas

> **OKF Canvas is a community fork of [OWOX Model Canvas](https://github.com/OWOX/owox-model-canvas)** (© OWOX, Inc., licensed Apache-2.0). It is an independent derivative maintained by Patrik Husfloen (redoz) and is not affiliated with or endorsed by OWOX. See [NOTICE](NOTICE) for attribution and a summary of changes.

<img width="1920" height="1200" alt="1782428766581" src="https://github.com/user-attachments/assets/0cdc74a8-c28a-4e77-a637-6b9c2bef65fd" />

> Sketch a data model on a free, anonymous, in-browser canvas — then share it as a link or export it as portable Markdown.

OKF Canvas is a static, single-page canvas for data people. Sketch your **data marts** (tables, views, SQL or connector outputs) as nodes and **joinable relationships** as edges, start from ready-made templates, flip to a field-level ERD view, and share the whole thing as a link. It's 100% client-side: there's no backend, no account, and no sign-in — your model lives in your browser and travels inside the URL you share.

> This fork removes the upstream server-side pieces (the OWOX API proxy, sign-in/Push, and the AI questions feature). What remains is a purely static SPA.

## Why use it

- **Free & anonymous** — design, template and share without signing in, an account, or any server. Nothing you draw is sent anywhere.
- **Templates for real domains** — E-commerce, SaaS, Marketplace, Marketing / Lead-gen, Mobile / Gaming, Finance, Healthcare, plus public datasets (Bitcoin, Stack Overflow).
- **Field-level ERD view** — see columns, primary keys and join keys, not just boxes.
- **Business Goal** — record a niche and an objective for the model; it's saved locally in your browser as context for the design (no AI, no network calls).
- **Build it with AI** — copy a short brief into Claude, ChatGPT or another LLM and it generates an importable OKF model (authored in your own AI tool, outside this app). In-app guide: [`/ai-instructions.html`](packages/web/public/ai-instructions.html).
- **Shareable links** — the whole model is packed (gzip + JSON) into the URL hash (`#m=…`); anyone with the link opens your exact diagram, entirely client-side. The packed model never leaves the browser except as text in a link you choose to share.
- **Local persistence** — your current model, its name, the view mode, relationship labels and the business goal are saved to `localStorage`, so a reload keeps your work.

## Open Knowledge Format (OKF) — your model stays portable

The canvas reads and writes **[Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog)** — a vendor-neutral way to describe a data model as a **folder of Markdown files** with YAML frontmatter: one document per data mart, a schema table, and a `## Joins` section for relationships. It's **human-readable and agent-friendly** — people can review it in a pull request and LLMs can author it.

- **Author + export, as a pair.** Design visually here and **Export OKF** to get a bundle that round-trips — re-importing restores the model. OWOX publishes real bundles at **[OWOX/models](https://github.com/OWOX/models)**; this canvas is a visual authoring/export front-end for that same format.
- **No lock-in.** Because the format is open and text-based, your model isn't trapped in a proprietary file — it lives in Markdown you own, in any git repo.
- **Generate it with AI.** The exact format an assistant should follow is documented at [`/okf-format.md`](packages/web/public/okf-format.md); the in-app guide has a one-click **Copy AI instructions** button.

That's what sets the canvas apart from generic ERD tools: the diagram is just a view over a portable, open model.

---

# Development

A static React Flow canvas for sketching data models, with OKF Markdown import/export and URL-based sharing. Everything runs in the browser — there is no server, no auth, and no external API.

## Layout (pnpm monorepo)

The repo has exactly two packages:

- `packages/okf` — pure shared lib: `ModelGraph` ⇄ OKF Markdown bundle (parse/serialize). No I/O.
- `packages/web` — React + Vite + React Flow SPA: the canvas, ERD view toggle, inspector, template library, OKF import/export, URL sharing, and `localStorage` persistence. Consumes `okf`'s built `dist/`.

## Develop

```bash
corepack pnpm install
corepack pnpm --filter @mc/okf build   # web consumes okf's built dist — build okf first
corepack pnpm --filter @mc/web dev      # Vite dev server on :5173
```

Open http://localhost:5173 — the canvas loads immediately, no sign-in.

To build and serve the production static bundle locally:

```bash
corepack pnpm --filter @mc/web build     # emits packages/web/dist
corepack pnpm --filter @mc/web preview    # serves the built dist/
```

The root `corepack pnpm build` runs the two in order (okf, then web); the per-package commands above are the ones to reach for during local work.

## Test

```bash
corepack pnpm -r test    # okf + web (Vitest)
corepack pnpm lint       # eslint
```

## Deploy

The app is a **static site** — the build emits `packages/web/dist`, a folder of HTML/CSS/JS that can be served by any static host or CDN. CI produces that `dist/`.

`render.yaml` is a static-site stub (`runtime: static`, `staticPublishPath: packages/web/dist`, with an SPA rewrite so every route serves `index.html`). It documents how a static host would serve the build; **the choice of where to host is deferred** and there is no maintained live deployment to point at.

## License

[Apache License 2.0](LICENSE) — © 2026 OWOX, Inc. See [NOTICE](NOTICE).

> "Open Knowledge Format (OKF)" is an open specification published by Google. OKF Canvas reads and writes that format but is an independent, community project — not affiliated with or endorsed by Google.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) for setup, tests, and the PR flow. By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Reporting a vulnerability

Please report security issues privately — see [SECURITY.md](SECURITY.md). Don't open a public issue for a vulnerability.
