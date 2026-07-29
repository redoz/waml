# WAML

> Fork of [OWOX Model Canvas](https://github.com/OWOX/owox-model-canvas) (© OWOX, Inc.). Not affiliated with or endorsed by OWOX. See [NOTICE](NOTICE) for attribution and changes.

Native editor and language tooling for [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog) Markdown and WAML/UML projections.

## Develop

```bash
cargo build --workspace
cargo test --workspace
pnpm install --frozen-lockfile
pnpm build
pnpm test
```

Run the native editor with `scripts/run-native.ps1` on Windows or
`scripts/run-native.sh` on Unix-like systems.

The Rust workspace contains `waml`, `waml-cli`, `waml-ops-dto`, and
`waml-editor`. The pnpm workspace contains only `packages/vscode`, which
launches the Rust `waml lsp --stdio` server through `vscode-languageclient`.

## Web delivery

The same `waml-editor` also builds to WebAssembly and is published to GitHub
Pages on every push to `main` by `.github/workflows/pages.yml`:

```bash
cargo makepad wasm build -p waml-editor --release --no-threads
```

`--no-threads` is required, not an optimisation: the threaded build needs
`Cross-Origin-Opener-Policy`/`Cross-Origin-Embedder-Policy` response headers for
`SharedArrayBuffer`, and Pages cannot set headers. The workflow then runs
`scripts/prune-web-fonts.mjs`, `scripts/brand-web-artifact.mjs`, and
`scripts/inject-runtime-shell.mjs` over the generated artifact. `cargo-makepad`
must be installed at the same makepad rev as `crates/waml-editor/Cargo.toml`.

## License

[Apache License 2.0](LICENSE) — © 2026 OWOX, Inc.; modifications © 2026 Patrik Husfloen (redoz). See [NOTICE](NOTICE).
