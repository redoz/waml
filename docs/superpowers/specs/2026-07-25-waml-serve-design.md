# `waml serve` — design

**Date:** 2026-07-25
**Status:** approved (design), plan pending
**Branch:** `waml-serve-spec`

## Problem

The web editor can open a bundle and edit it, but it cannot save anywhere. Today
`waml-editor`'s `save_backend` seam (`crates/waml-editor/src/app.rs`) has two
arms: the web arm writes the whole bundle into a URL fragment, and the native arm
is a deliberate empty stub. Neither writes files.

Meanwhile `waml-cli` (`crates/waml-cli/src/main.rs`) already owns every file-side
operation — `check`, `fmt`, `reindex`, `share`, `lsp` — and `waml-wasm` already
exposes the full semantic surface (`apply_ops`, `build_model`, `validate`,
`fmt`, `reindex`) to JavaScript. What is missing is a way for a *browser* to
reach the *local filesystem* with those same operations.

`waml serve` is that bridge: a `waml-cli` subcommand that serves the embedded web
editor from a loopback HTTP server and exposes an ops API over the directory it
was pointed at. It fills the native `save_backend` stub, and it makes the web
editor a real local tool rather than a share-link toy.

## Decisions

Settled in brainstorming, recorded here so the plan does not relitigate them.

### Writes are ops, not files

`POST /api/ops` carries fine-grained semantic ops (`OpDto`), not file bodies.
The server applies them via `waml::ops::apply`, so validation, canonical
formatting and index regeneration live in exactly one place. A client that PUT
raw markdown would be able to write bundles the CLI would reject or reformat.

This is not a novel shape. `crates/waml-wasm/src/lib.rs:143` is already
`apply_ops(bundle, ops) -> bundle`; the HTTP API mirrors it with the bundle held
server-side instead of passed in.

`crates/waml-ops-dto/src/lib.rs` (1077 lines) is *already* a versioned wire
contract — each variant carries a `v: u32` checked by `check_v` — shipped to
JavaScript as a Tsify union in `packages/wasm/src/generated/waml_wasm.d.ts`.
`waml serve` reuses it verbatim. It does not invent an API.

### Reads are state, and include a model projection

The first draft had writes carrying semantics and reads returning raw markdown.
That asymmetry is wrong: the server demonstrably knows how to parse the bundle
(it must, to apply an op), so making every consumer re-implement or re-link a
parser to read back what it just wrote is a false economy.

The counter-argument — "a `Model` projection adds a second wire contract to
version" — does not hold, because `Model` is *already* a wire contract:

- `crates/waml/src/model.rs:982` derives serde, and every type in the module
  derives `tsify_next::Tsify` under the `wasm` feature.
- `crates/waml-wasm/src/lib.rs:111` already exports
  `build_model(bundle) -> waml::model::Model` with generated TypeScript types.

So the read side mirrors the wasm surface:

| Endpoint | Returns | Mirrors |
| --- | --- | --- |
| `GET /api/bundle` | `{ revision, files: [path, markdown][] }` | source of truth |
| `GET /api/model` | `{ revision, model: Model }` | `build_model` |
| `GET /api/diagnostics` | `{ revision, diagnostics: Diagnostic[] }` | `validate` |

A thin client (a script, CI, another editor) reads `/api/model` and never links
a parser. `waml-editor` keeps markdown as its source of truth and keeps its own
parser, because the GitHub Pages build has no server to ask — but `waml serve`
no longer *forces* that cost on anyone else.

`GET /api/okf` (mirroring `build_bundle`) is deliberately omitted: no consumer
has asked for it. Adding it later is additive.

### Ops responses return changed files, not the whole bundle

`POST /api/ops` responds with `{ revision, changed: [path, markdown][] }` —
only the documents the ops touched, plus the new revision. Returning the whole
bundle on every keystroke-driven save is wasteful on a bundle of any size.

`revision` is a monotonically increasing counter, bumped on every successful
apply. The request carries the revision the client believes it holds; a mismatch
is answered `409 Conflict` and the client re-reads `GET /api/bundle`. Ops within
a request are all-or-nothing: on any op error nothing is written, and the
response is `422` carrying the failing op index and reason (the same
`{ index, reason }` shape `waml::ops::apply` already returns).

A client that wants a fresh `Model` after a write issues a second
`GET /api/model`. On loopback that round trip is free, and it keeps the write
response small. A `?projection=model` query on `POST /api/ops` is a plausible
later optimisation; it is out of scope until something measures the round trip
and finds it matters.

### The web artifact is embedded in the binary

The built `waml-editor` wasm artifact is compiled into the `waml` binary and
served from memory. Rejected alternatives: an `--artifact <path>` flag (the
binary stops being self-contained, and the flag is a footgun that serves stale
builds), and having `serve` build the artifact on demand (requires a full Rust +
`cargo-makepad` toolchain at run time).

Bytes are stored **brotli-precompressed** and served with
`Content-Encoding: br`, so there is no runtime compression and no runtime
decompression — the stored bytes go straight onto the socket. Measured on the
current artifact:

| | raw | brotli-11 | ratio |
| --- | --- | --- | --- |
| whole artifact | 16.35 MB | 3.88 MB | 23.7% |
| `waml-editor` wasm | 11.83 MB | 1.96 MB | 16.6% |

A client that does not advertise `br` in `Accept-Encoding` gets a `406`. Every
browser that can run the wasm supports brotli; supporting a decompress path for
a hypothetical client is not worth the code.

*Noted, not in scope:* fonts are **47% of the compressed payload** — 17 font
files totalling 4.33 MB raw / 1.84 MB brotli, including `fa-solid-900`
(FontAwesome), `NewCMMath`, `LiberationMono`, `NotoSans`, and
`IBMPlexSans-Italic`/`-SemiBold` duplicated across crate resource trees.
`scripts/prune-web-fonts.mjs` reports "kept 8" but 17 survive, because it misses
`makepad_widgets`' resource tree. Fixing that would shrink both this binary and
the Pages deploy. It is separable work and deliberately excluded here.

### Same-origin only — cross-origin was empirically disproven

An attractive alternative was to leave the UI on GitHub Pages and have it talk
to a local `waml serve` API. This does not work, and the finding is empirical
rather than theoretical.

Tested with headless Chromium 149 from the real `https://redoz.github.io/waml/`
origin:

```
blocked by CORS policy: Permission was denied for this request to access the
`loopback` address space.
```

The preflight never left the browser, which also makes
`Access-Control-Allow-Private-Network` dead weight — both header spellings were
sent and neither had any effect. Controls passed (direct navigation to the local
server returned 200; a local page fetching the local API returned 200), so this
is browser policy, not a broken test rig.

Therefore the UI is served from the same origin as the API. Untested and
recorded as unknown: whether *headed* Chrome offers a user-facing permission
prompt that would flip this. Even if it does, a prompt-gated cross-origin path is
not a design worth building on.

## Command surface

```
waml serve [DIR] [--port <PORT>] [--bind-all] [--api-only] [--no-open]
```

- `DIR` — directory to serve and edit. Defaults to the current directory.
- `--port` — default `8099`. `--port 0` binds an ephemeral port and prints it.
- `--bind-all` — bind `0.0.0.0` instead of `127.0.0.1`. Prints a warning to
  stderr naming the exposure.
- `--api-only` — suppress the embedded UI routes. This exists so a developer can
  run the API against a locally built UI on another port; it is **not** a way to
  let the hosted Pages UI edit local files (see above — that is blocked by the
  browser regardless).
- `--no-open` — do not launch a browser.

On start it prints one line to stderr:

```
waml serve  http://127.0.0.1:8099/?token=<token>   (serving C:\dev\waml\docs)
```

The token is printed exactly once, in that URL. `Ctrl-C` shuts down.

`serve` joins the existing `Command` enum in `crates/waml-cli/src/main.rs`
alongside `Check`, `Share`, `Lsp`, `Fmt`. It is the second server in the CLI;
unlike `Lsp` (stdio only, "the only supported transport in Phase 1") its
transport is HTTP and no other transport is contemplated.

## API

All routes are under `/api/`. All require authentication (see Security).
All responses are JSON; all carry `revision`.

| Method | Route | Body | Response |
| --- | --- | --- | --- |
| `GET` | `/api/bundle` | — | `{ revision, files }` |
| `GET` | `/api/model` | — | `{ revision, model }` |
| `GET` | `/api/diagnostics` | — | `{ revision, diagnostics }` |
| `POST` | `/api/ops` | `{ revision, ops: OpDto[] }` | `{ revision, changed }` |

Status codes: `200` success · `401` missing/bad token · `403` Origin, Host or
custom-header check failed · `409` revision mismatch · `422` op rejected, body
`{ index, reason }` · `406` client does not accept brotli (UI routes only) ·
`500` filesystem error, body carries the path and the OS error.

Everything not under `/api/` is the embedded UI: `/` serves the HTML shell, and
asset paths map into the embedded artifact. Unknown asset paths are `404` — the
server never falls back to serving the shell for an unmatched path, because that
turns a typo into a silent blank page.

## Security

The threat is not exotic: this is a local process that rewrites files on disk,
reachable by any other local process and by any web page open in the user's
browser.

1. **Token, always required.** 256 bits from a CSPRNG, generated fresh per
   invocation, never persisted to disk. Accepted as `Authorization: Bearer <t>`
   or as a `?token=` query parameter — the query form is required because the
   wasm editor reads its parameters from `WebParams.search`
   (makepad `platform/src/cx.rs:245`) and has no JS-to-wasm channel to receive a
   header. Compared in constant time. This is required even on loopback and even
   without `--bind-all`; loopback is not access control.
2. **Custom header on mutating routes.** `POST /api/ops` additionally requires
   `X-Waml-Client: 1`. A hostile page cannot set a custom header on a simple
   request, so the browser is forced into a preflight, which then fails the
   Origin check. This closes the `text/plain` simple-POST hole that would
   otherwise skip preflight entirely.
3. **Origin allowlist.** A request carrying an `Origin` header must match the
   server's own origin exactly. `Access-Control-Allow-Origin: *` is never sent.
   `Access-Control-Allow-Private-Network` is not sent at all — measured above to
   have no effect.
4. **Host check.** Reject unless `Host` is `127.0.0.1:<port>`, `[::1]:<port>`,
   or — under `--bind-all` — an address the socket is actually bound to. This is
   the anti-DNS-rebinding control: without it, an attacker-controlled name
   resolving to 127.0.0.1 would be same-origin to the browser.
5. **Path confinement.** Every bundle path is relative. Reject `..` segments,
   absolute paths, Windows drive prefixes and UNC paths, NUL bytes, and Windows
   reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`,
   with or without extension). Resolve symlinks and require the result to stay
   under the served `DIR`.
6. **Bind loopback by default.** `--bind-all` is opt-in and warns.
7. **No shell-out, no arbitrary-path reads.** UI bytes come only from the
   embedded blob; filesystem access is confined to `DIR`.

Items 2–4 are defence in depth. Item 1 is the actual access control.

## Editor-side changes

The guiding constraint: no web-specific concepts leak into shell code. (An
earlier `sync_share_url` was rejected for exactly that.) All backend difference
stays behind the one existing seam.

- `save_backend` in `crates/waml-editor/src/app.rs` gains a third arm,
  `Backend::Http { base, token }`, joining today's web (URL fragment) and native
  (empty stub) arms. The arm is chosen **once at startup**, not per save.
- It is transport, not platform: both the native binary and the wasm binary can
  select it. Native `waml serve` passes `base`/`token` to the editor it launches;
  the wasm build reads them from `WebParams.search`. When absent, today's
  behaviour is unchanged and the fragment/stub arms stand.
- Transport is `cx.http_request` (makepad `platform/src/cx_api.rs:1453`), which
  is cross-platform including web, so there is one code path rather than a
  `#[cfg]` fork.
- `mark_dirty` is unchanged. `save` becomes: drain accumulated `OpDto`s → `POST
  /api/ops` → apply the returned `changed` files and `revision` to the in-memory
  bundle. The editor stays optimistic; a `409` triggers a `GET /api/bundle`
  reload and a visible "reloaded from disk" notice rather than a silent
  clobber.
- The editor keeps its own parser. This is not negotiable: the Pages build has no
  server to project a `Model` for it.

## Testing

- **Unit.** Path-confinement table (`..`, `C:\`, UNC, `CON`, symlink escape,
  NUL); constant-time token compare; revision/409 logic; the Origin/Host/custom
  header rejection matrix. All pure functions, no socket.
- **Integration.** Bind an ephemeral port in-process, drive a full ops round trip
  against a temp bundle directory with an HTTP client, and assert both the
  response body and the resulting bytes on disk. Includes an assertion that a
  deliberately unformatted op result lands canonically formatted, which is what
  makes "the server owns validation and formatting" a tested claim rather than a
  stated one.
- **Contract.** Assert `GET /api/model`'s payload is identical to
  `waml::parse::build_model(&bundle)` serialized. That equivalence is the entire
  justification for the projection, so it gets a test.
- **Browser.** `playwright-core` driving the ms-playwright `chromium-1228` build
  (`chrome-win64` — note, *not* `chrome-win`): load the embedded UI from the
  server, confirm the wasm boots without a console panic, confirm `?token=`
  reaches the wasm side, and confirm a page on a foreign origin is still blocked
  from the API. That last one is a regression guard on the finding that killed
  the hosted-UI option.
- **Not tested.** Whether headed Chrome offers a loopback permission prompt.
  Recorded as an open unknown.

## Out of scope

- WebSockets, live reload, or any server-push channel.
- Multi-client collaboration or operational transform. One editor at a time; the
  revision counter detects a stale client, it does not merge.
- Any authentication beyond the single per-invocation token. No users, no
  sessions, no persistence.
- TLS. Loopback only, and `--bind-all` warns.
- Serving more than one directory, or any path outside `DIR`.
- `--api-only` as a hosted-UI enabler.
- Font pruning (measured above at 47% of the compressed payload). Separable.
- Replacing or subsuming `waml lsp`.
- Detecting external edits to `DIR`. The revision counter tracks *our own*
  writes; a file changed underneath the server is not noticed. A file watcher is
  a plausible follow-up and is not part of this.
