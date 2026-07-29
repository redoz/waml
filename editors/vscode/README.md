# WAML for VS Code

Live WAML diagnostics for Markdown documents. Launches the Rust
`waml lsp --stdio` language server through `vscode-languageclient`.

This is a standalone Node project — it is not part of the Rust cargo workspace
and has no pnpm workspace above it. Run everything from this directory.

```bash
pnpm install --frozen-lockfile
pnpm build        # tsc
pnpm test         # vitest
pnpm lint         # eslint
pnpm format:check # prettier
```

The extension resolves the server binary from the `waml.serverPath` setting
(default `waml`, i.e. found on `PATH`). Build it with `cargo build -p waml-cli`
from the repository root.

## License

[Mozilla Public License 2.0](../../LICENSE) — © 2026 Patrik Husfloen (redoz).
