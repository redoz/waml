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

### Editing an exported site forks into a share link

The exported site is read-only at the transport level: there is nothing behind
`?bundle=` to write to. But the editor is a full editor, and silently discarding
edits is not acceptable.

So the fragment backend becomes the save *sink* even when it was not the boot
*source*. The first edit writes accumulated state into `#w1.` and the URL becomes
a share link. The exported bundle is the starting point; edits fork into the URL.

This costs almost nothing — the fragment arm already exists — and preserves the
"open the docs, tweak something, send someone the link" flow that Pages has
today. A read-only mode that hides every mutating affordance was rejected: it is
real UI work across every edit path, and it would make the docs site strictly
less capable than the current deploy.

### The op-log is downloadable

A share link forks the state. It does not let you take your edits back to the
source repository. An op-log does: `waml apply` already accepts NDJSON, and ops
are semantic, so they survive an unrelated change to the target bundle in a way
a forked document does not.

The pieces are already in place, which is what makes this in scope:

- The editor already mutates through ops. Every mutation goes through
  `EditorSession::apply<B: EditBatch>` (`crates/waml-editor/src/editor_session.rs:109`),
  and callers push `waml::uml::Op::{ClassifierSet, AttributeRemove, PlacementSet,
  PlacementRemove, DiagramSet}` and `waml::okf::Op::IndexRetitle` inside
  `waml::uml::Batch` / `PendingEdit`.
- The applied edits are already retained in order. `EditorHistory`
  (`crates/waml-editor/src/editor_history.rs:91`) holds `EditHistoryStep`s each
  carrying a `PendingEdit`, with `mark_saved` / `is_saved` / `current_state`.
- `waml-ops-dto` is already an exhaustive versioned wire contract, each variant
  carrying a `v: u32`.

The gap is one direction of conversion. `waml-ops-dto` has
`to_batch(dtos) -> Batch` (`crates/waml-ops-dto/src/lib.rs:387`) — DTO inbound,
which is all the CLI and `serve` ever need. `Op -> OpDto` does not exist. Partial
precedent does: `to_compat_step` / `from_compat_step` (`:399`, `:676`) already
round-trip through a `Step` form.

So the work is: reverse-map the DTO enum (mechanical, one arm per variant),
serialize the history to NDJSON, and deliver the file to the user.

The log carries a header line — the format already permits one, per commit
`9c68d06f`, "op-log is NDJSON with optional header line, not array envelope" —
holding a hash of the bundle the ops were authored against. `waml apply` refuses
a mismatched base unless forced, so applying a stale log to a drifted bundle is
an error rather than silent corruption.

### Download is implemented in the generated shell, not the fork

makepad's web bridge exposes a fixed message set
(`platform/src/os/web/from_wasm.rs`); there is no arbitrary wasm-to-JS escape
hatch. `cx.open_url` — the editor's only current exit
(`crates/waml-editor/src/platform_browser.rs:12`) — cannot download, because
browsers block top-level navigation to `data:` URLs.

But `export site` generates `index.html`, so the shell can supply the capability
itself. `FromWasmOpenUrl` is a method on an exported class
(`export class WasmWebBrowser` — `platform/src/os/web/web.js:3`, with
`WasmWebGL extends` it), whose implementation builds an anchor and clicks it
(`web.js:266`). The generated shell patches that method on the prototype before
boot, intercepts a private scheme (`waml-download:<name>`), and performs a
Blob-anchor download. Real URLs pass through untouched.

This needs no fork change, no new pin, and no wasm rebuild — which matters,
because the fork pin is a standing trap in this repository (pin the exact sha
from `Cargo.toml`, never a branch tip).

Its limit is that the capability lives in shells *we* generate. Exported sites
and the Pages deploy get download; someone hosting the bare artifact by hand does
not. If it ever needs to be universal it graduates to a `cx.download_file`
primitive in the fork, at the same call site.

"Copy op-log" is offered alongside and is free: makepad already implements web
clipboard copy (`FromWasmTextCopyResponse`).

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
  index.html          shell: prototype patch, then boot with ?bundle=bundle.waml
  waml_editor.wasm
  *.js                makepad JS glue
  resources/...       fonts and assets, post-pruning
  bundle.waml         envelope v1
```

## Open question: browser-local persistence

Undecided, and deliberately separated from the settled design above. Nothing
else in this spec depends on the answer.

The problem: an exported site holds edits only in memory and in the URL
fragment. Closing the tab loses anything not yet forked into a link, and the
fragment is a poor place to accumulate a long editing session.

### Option A — service worker backed by IndexedDB

The exported site ships a service worker implementing the same `/api/*` routes
`waml serve` defines, against IndexedDB. The editor is unchanged: it uses the
`?api=` backend and cannot tell whether it is talking to a local server or to
the browser's own storage.

```text
serve:          editor --HTTP--> waml serve      --> disk
exported site:  editor --HTTP--> service worker  --> IndexedDB
```

Gains: edits survive refresh and tab close, the revision counter works as
designed, and one backend arm serves both worlds.

Costs: service workers require HTTPS or localhost — fine for Pages and `serve`,
but a plain `http://` intranet host would lose persistence silently unless a
degraded mode is explicit. And a service worker is a real artifact with scope,
versioning and update semantics, carrying the standard trap of a stale worker
pinning a stale wasm indefinitely.

### Option B — prototype shim over `FromWasmHTTPRequest`

The same interception trick as the download shim: the generated shell answers
private API paths from IndexedDB directly. No HTTPS requirement and no service
worker cache traps, but the response has to be handed back *into* wasm through
makepad's internal dispatch, which is unsupported surface that a fork bump can
break without a compile error.

### Option C — no persistence

Edits live in memory and in the fragment, and the op-log download is the way to
take work away. Simplest by a wide margin, and it is what Pages does today.

### Option D — explicit local save/restore

No background persistence. The user explicitly saves a session to IndexedDB and
restores it, through a shell-provided capability. Less magic than A, far less
machinery, but it does not survive an accidental tab close, which is the main
thing persistence is for.

## Testing

- **Unit.** Assembler output map for both `Static` and `Api` sources: correct
  paths present, bundle present or absent, `index.html` boot parameters correct.
  Output-directory guard (empty, non-empty, non-empty with `--force`).
- **Round trip.** `Op -> OpDto -> to_batch -> Op` over every variant, asserting
  equality. The reverse mapping is new code against an exhaustive enum, so
  exhaustiveness is the property worth testing.
- **Op-log.** An editing session's history serializes to NDJSON that `waml apply`
  accepts against the source bundle and rejects against a drifted one.
- **Compression.** Bytes written by `export site` decompress to the same bytes
  the embedded blob holds.
- **Browser.** Playwright against a served export: the wasm boots without a
  console panic, `?bundle=` loads the bundle, an edit forks into `#w1.`, and the
  download shim produces a file. This is the regression guard on the shell
  patch, which is the most fork-fragile part of the design.
- **CI.** The Pages deploy runs `waml export site` and the existing
  `scripts/verify-web-artifact.mjs` guard still passes.

## Out of scope

- `?repo=owner/name` and GitHub tree discovery.
- `export svg`, `export json`, and folding in `waml bundle`.
- `file://` support.
- Browser-local persistence (open question above).
- A `--base-url` for hosting under a subpath; relative paths cover the known
  cases.
- Anything behind `waml serve`: the HTTP API, tokens, the security matrix.
- A `cx.download_file` primitive in the makepad fork.
