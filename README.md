# WAML

Native editor and language tooling for [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog) Markdown and WAML/UML projections.

## Develop

```bash
cargo build --workspace
cargo test --workspace
```

Run the native editor with `run.ps1` on Windows or `run.sh` on Unix-like
systems.

The Rust workspace contains `waml`, `waml-cli`, `waml-ops-dto`, `waml-syntax`,
and `waml-editor`. The VS Code extension in `editors/vscode` is a standalone
Node project that launches the Rust `waml lsp --stdio` server through
`vscode-languageclient`; see `editors/vscode/README.md` for its build steps.

"Open Knowledge Format (OKF)" is an open specification published by Google
(GoogleCloudPlatform/knowledge-catalog). WAML reads and writes that format but
is an independent project — it is not affiliated with or endorsed by Google.

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

## Export a site

A `waml` binary built with `--features embed-web` carries the whole web editor
and can write a self-contained site for any model — the editor plus that
model's `bundle.waml`, with no server behind it:

```bash
waml export site docs/waml --out site
python -m http.server --directory site
```

Open the served page and the editor boots that bundle. The embedded
`bundle.waml` is immutable — nothing writes back to it — but edits are not
lost: the first one moves the whole model into the page's `#w1.` share URL, so
a refresh or a copied link reopens the edited version, and **Export WAML
bundle…** in the burger menu downloads the edited source as one `.waml` file.
`--out` defaults to `./site` and refuses a non-empty directory unless you pass
`--force`.

## License

[Mozilla Public License 2.0](LICENSE) — © 2026 Patrik Husfloen (redoz).
