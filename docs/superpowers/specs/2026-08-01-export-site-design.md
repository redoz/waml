# `waml export site` — design

**Date:** 2026-08-01
**Status:** approved (design), plan pending
**Depends on:** `2026-07-31-bundle-envelope-v1-design.md` (bundle file format)
**Feeds:** `2026-07-25-waml-serve-design.md` (artifact embedding, assembler seam)

## Problem

The GitHub Pages deploy publishes the wasm editor and nothing else. A visitor
lands on an empty start screen. The WAML documentation lives in `docs/waml/` as
a real WAML bundle, and the editor is the natural way to read it, but there is
no way to put the two together: the web build's only model source is the URL
fragment written by `waml share`.

More generally there is no way to hand someone a bundle as a browsable
artifact. `waml bundle` emits JSON or TypeScript for a program to consume, and
`waml share` emits a link that carries the whole model in its fragment. Neither
produces something you can upload to a static host and read.

`waml export site` produces that artifact: a directory holding the wasm editor
and a bundle, servable by any static host with no server logic.

## Decisions

Settled in brainstorming, recorded here so the plan does not relitigate them.

### The command is `waml export site`

`export` is a subcommand group. `site` is its first member; `svg` and `json`
are anticipated. `package` was rejected because `Package` is a WAML classifier
(`docs/waml/architecture/concepts/model/package.md`) and naming a CLI verb after
a domain type invites confusion in every later conversation.

The existing `waml bundle` (directory to JSON/TS) is arguably already
`waml export json`. Folding it in is a separate deprecation and is out of scope.

### One assembler, three sinks

A single function produces a virtual file map; the sinks differ in how they
deliver it and where the bundle comes from.

```text
assemble(artifact, source) -> Map<path, bytes>

source = Static(path)  -> map contains bundle.waml;
                          index.html boots the fetch arm at it
source = Api           -> no bundle in the map;
                          index.html boots the Http backend at /api
```

- `waml export site` writes the map to a directory.
- `waml serve` serves the same map from memory and mounts `/api/*` alongside.
- `.github/workflows/pages.yml` stops assembling the site by hand and runs
  `waml export site docs/waml` instead, deploying the result.

Making the Pages deploy a plain consumer of the command means the deploy is the
standing test of the export path. The alternative — a workflow that copies files
itself — leaves the real command untested in CI, which is exactly how the
git-dep glue bug shipped a blank page (see `web-deploy-was-dead` and the comment
block in `pages.yml`).

The sinks diverge in exactly one place, and legitimately. `serve` controls its
own response headers, so it holds the artifact brotli-compressed and streams it
with `Content-Encoding: br`. `export site` writes files a static host will serve
verbatim, so it decompresses on write.

That divergence is forced, not chosen. Measured against the live deploy:

```text
$ curl -sSI -H "Accept-Encoding: br, gzip" https://redoz.github.io/waml/
HTTP/1.1 200 OK
Content-Encoding: gzip
Vary: Accept-Encoding
```

GitHub Pages answers `gzip` even when `br` is offered, and it cannot be told to
set `Content-Encoding` on a `.br` file. Shipping pre-compressed bytes would
serve garbage.

### The artifact is embedded behind a Cargo feature

The built wasm editor is compiled into the `waml` binary, brotli-precompressed
(3.88 MB stored against 16.35 MB raw, measured in the serve design). An
`--artifact <path>` flag is rejected there and stays rejected here: it is a
footgun that silently exports a stale editor.

Embedding cannot be unconditional. Producing the artifact requires nightly Rust,
`-Z build-std`, `rust-src`, `cargo-makepad` and `wasm-opt`; making that a
prerequisite of `cargo build` would break the workspace gate and every
contributor's checkout.

So: Cargo feature `embed-web`, **default off**.

- Off — `export site` and `serve` compile, and exit with an error naming how to
  produce a binary that can run them. `cargo test --workspace` is unaffected.
- On — `include_bytes!` over a prebuilt artifact directory. Release CI builds
  the wasm first, then the CLI with the feature.

The export path additionally links a brotli decoder (`brotli-decompressor`),
used only to decompress on write.

### Font pruning is fixed here

`scripts/prune-web-fonts.mjs` reports "kept 8" while 17 font files survive,
because it misses `makepad_widgets`' own resource tree. Those files are 4.33 MB
raw / 1.84 MB brotli — **47% of the compressed payload** — and include
`fa-solid-900`, `NewCMMath`, `LiberationMono`, `NotoSans`, and
`IBMPlexSans-Italic`/`-SemiBold` duplicated across crate resource trees.

The serve design flagged this and deliberately excluded it. It is in scope here
because export multiplies the cost: every exported directory pays it, and the
difference is roughly 10 MB against 6 MB per package. Fixing it also shrinks the
Pages deploy and the embedded artifact.

### Editor boot precedence

The web build gains one new model source. Existing sources are untouched.

1. `#w1.` fragment — decode it. Share links behave exactly as today.
2. `?api=<base>` — Http backend, read/write. This is `waml serve`.
3. `?bundle=<url>` — fetch via `cx.http_request`, read-only. This is an
   exported site.
4. Otherwise — start screen.

The fetch arm takes any URL, because a relative-only restriction would be extra
code for less capability. `?bundle=https://raw.githubusercontent.com/...` works,
since that host sends `Access-Control-Allow-Origin: *`. A host that refuses
cross-origin reads must surface an explicit error naming the refusal — never a
blank canvas.

`export site` itself only ever emits a relative path.

Pointing the editor at a repository rather than a bundle file — `?repo=owner/name@ref:path`,
discovering `.md` files through the GitHub tree API — is deliberately deferred.
It is a vendor integration, not a generic feature, and unauthenticated GitHub API
access is 60 requests per hour per IP, which a docs tree of a few dozen files
exhausts in one or two loads. It needs its own design.

### Edits are preserved in a self-contained share URL

The exported site is read-only at the transport level: there is nothing behind
`?bundle=` to write to. But the editor is a full editor, and silently discarding
edits is not acceptable.

So the fragment backend becomes the save *sink* even when it was not the boot
*source*. The first edit writes the current state into `#w1.` and the URL becomes
a self-contained share URL. The exported bundle remains unchanged; the URL holds
the user's editable copy.

This costs almost nothing — the fragment arm already exists — and preserves the
"open the docs, tweak something, send someone the link" flow that Pages has
today. A read-only mode that hides every mutating affordance was rejected: it is
real UI work across every edit path, and it would make the docs site strictly
less capable than the current deploy.

### The editor exports the current WAML bundle

The editor's existing burger menu gains **Export WAML bundle…** beside Create,
Open model, and Close model. It serializes the current model, including edits
preserved in the share URL, and downloads one `.waml` bundle-envelope v1 file.
This is a complete editable artifact, not an op-log or patch.

The same action is available in the native editor, `waml serve`, exported sites,
and the GitHub Pages deployment. The editor owns this product capability.

Native builds use the platform save dialog. Web builds use one supported
Makepad primitive added to the pinned fork:

```rust
cx.download_file(name, bytes, mime_type)
```

The primitive transfers owned bytes to the browser bridge, creates a Blob,
clicks a temporary download anchor, and then revokes the Blob URL. It is generic
platform capability, not WAML-specific behavior. The generated site shell does
not add a private URL scheme or patch a Makepad prototype.

## Command surface

```text
waml export site DIR [--out OUT] [--force]
```

- `DIR` — bundle directory to export. Required.
- `--out` — output directory. Defaults to `./site`.
- `--force` — permit writing into a non-empty existing directory. Without it, a
  non-empty `OUT` is an error, so a mistyped path cannot quietly consume a
  directory.

`file://` is not supported. A `fetch` of a sibling file is blocked at a `file://`
origin, so double-clicking an exported `index.html` cannot load its bundle. The
package requires an HTTP origin; `python -m http.server` suffices.

## Output layout

```text
out/
  index.html          boot with ?bundle=bundle.waml
  waml_editor.wasm
  *.js                makepad JS glue
  resources/...       fonts and assets, post-pruning
  bundle.waml         envelope v1
```

## Browser-local persistence

The first version adds no IndexedDB or service-worker persistence. Edits are
preserved in the `#w1.` share URL as soon as the model changes. The user can also
download the current state through **Export WAML bundle…**. Browser-local
session storage remains a separate future design.

## Testing

- **Unit.** Assembler output map for both `Static` and `Api` sources: correct
  paths present, bundle present or absent, `index.html` boot parameters correct.
  Output-directory guard (empty, non-empty, non-empty with `--force`).
- **Compression.** Bytes written by `export site` decompress to the same bytes
  the embedded blob holds.
- **Browser.** Playwright against a served export: the wasm boots without a
  console panic, `?bundle=` loads the bundle, an edit writes the current model
  into `#w1.`, and **Export WAML bundle…** downloads a bundle that opens
  with the edited state.
- **CI.** The Pages deploy runs `waml export site` and the existing
  `scripts/verify-web-artifact.mjs` guard still passes.

## Out of scope

- `?repo=owner/name` and GitHub tree discovery.
- `export svg`, `export json`, and folding in `waml bundle`.
- `file://` support.
- Browser-local persistence beyond the share URL.
- A `--base-url` for hosting under a subpath; relative paths cover the known
  cases.
- Anything behind `waml serve`: the HTTP API, tokens, the security matrix.
- Op-log generation, download, or `Op -> OpDto` reverse mapping.
