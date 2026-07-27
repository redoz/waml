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

## License

[Apache License 2.0](LICENSE) — © 2026 OWOX, Inc.; modifications © 2026 Patrik Husfloen (redoz). See [NOTICE](NOTICE).
