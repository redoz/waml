# OKF Canvas

> Fork of [OWOX Model Canvas](https://github.com/OWOX/owox-model-canvas) (© OWOX, Inc.). Not affiliated with or endorsed by OWOX. See [NOTICE](NOTICE) for attribution and changes.

In-browser canvas for sketching data models, reading/writing [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog) Markdown.

## Develop

```bash
pnpm install
pnpm --filter @waml/okf build   # web consumes okf's built dist — build okf first
pnpm --filter @waml/web dev     # Vite dev server on :5173
```

pnpm monorepo: `packages/okf`, `packages/core`, `packages/web` (the SvelteFlow canvas), `packages/wasm`, `packages/vscode`, plus a Rust workspace under `crates/` for the WAML language core/CLI/LSP.

## License

[Apache License 2.0](LICENSE) — © 2026 OWOX, Inc.; modifications © 2026 Patrik Husfloen (redoz). See [NOTICE](NOTICE).
