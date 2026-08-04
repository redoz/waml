# `waml serve` — design

**Date:** 2026-07-25 (rewritten 2026-08-04 against the current architecture)
**Status:** implemented — `waml serve` ships the API, the embedded UI, and the editor boot/save wiring this spec describes

**Rewrite note.** The original draft was written against `waml::ops` and the
`waml-wasm` bindings crate. Both are gone: `waml::ops` was retired in favour of
the `waml::edit` Step/Batch transaction layer (see
`2026-08-04-retire-compat-design.md`), and `waml-wasm` was removed from the
workspace — the web editor is the makepad wasm build of `waml-editor` itself,
with no separate JS binding surface. Every claim below has been re-derived from
the code as it stands.

## Problem

The web editor can open a bundle three ways — a share fragment, a fetched
`?bundle=` envelope, an exported site's `waml-boot.txt` — but none of them can
write back to a local directory. The fourth boot source, `?api=<base>[&token=]`,
is already parsed and selected for (`crates/waml-editor/src/browser_boot.rs`),
and today dead-ends at the start screen with a comment saying "no live model
server exists yet" (`crates/waml-editor/src/app.rs`).

Meanwhile the pieces around the server all exist:

- `waml serve` is in the CLI's `Command` enum with its full flag surface, parse
  tests, and a `serve::run` stub that prints "not implemented" and exits 2
  (`crates/waml-cli/src/serve/mod.rs`).
- The built web editor is embedded brotli-compressed in the `waml` binary behind
  the `embed-web` feature (`crates/waml-cli/src/web_artifact.rs`), and
  `waml export site` already assembles it into a servable site via one shared
  assembler with a `SiteSource::Api` arm reserved for `serve`
  (`crates/waml-cli/src/site.rs`).
- The CLI already owns a full read-mutate-validate-write pipeline over a
  directory: `run_batch` in `crates/waml-cli/src/main.rs` reads the bundle,
  lowers a `waml::edit::Batch` under an `EditContext`, revalidates the result
  with `prepare_candidate`, and writes back only what changed.

`waml serve` is the bridge that connects these: a loopback HTTP server that
serves the embedded editor and exposes that same pipeline over the directory it
was pointed at. It gives the `?api=` boot source something to boot from, and it
makes the web editor a real local tool rather than a share-link toy.

## Decisions

Settled in brainstorming and revised where the architecture moved underneath
them; recorded here so the plan does not relitigate them.

### One validation funnel, two write surfaces

The original draft said "writes are ops, not files" on the grounds that a raw
file write could land bundles the CLI would reject or reformat. The
architecture has since made that half-true: the *native* editor already writes
raw files — `save_backend`'s native arm atomically replaces authored files from
a `SaveTicket` snapshot (`crates/waml-editor/src/native_save.rs`) — because the
editor is a full client with its own parser producing canonical text. Forcing
that client to re-encode its edits as wire ops would require a `Step → OpDto`
reverse encoding that does not exist and serves nobody.

What actually protects the bundle is not the wire shape but the funnel behind
it: every mutation, whatever its shape, must pass `prepare_candidate`
revalidation before anything touches disk — the same gate `run_batch` applies.
So the server exposes two write surfaces feeding one funnel:

- **`POST /api/ops` — semantic ops, for thin clients.** Carries `OpDto[]`, the
  versioned wire contract in `crates/waml-ops-dto` (each variant carries a
  `v: u32` checked by `check_v`; this is also the CLI's `--emit` line format).
  The server lowers via `OpDto::to_step` into a `waml::edit::Batch` and applies
  it exactly as `run_batch` does. A script or CI job gets validation, canonical
  formatting, and index regeneration without linking a parser.
- **`POST /api/documents` — document bodies, for the editor.** Carries
  `{ revision, writes: [{ path, baseline, desired }] }`, mirroring the
  baseline-guarded shape `native_save::save_ticket_atomic` already writes on
  native. The server applies the writes to its in-memory bundle, runs
  `prepare_candidate` on the result, and **rejects the whole request** if the
  candidate does not validate — nothing invalid ever lands on disk, which is
  the guarantee the original ops-only rule existed to provide.

Both routes are all-or-nothing. An ops failure answers `422` with the
`waml::edit::EditError` shape — `{ index, op, selector, reason }` — which is
richer than the draft's `{ index, reason }` because that is what the edit layer
actually returns now. A documents failure answers `422` with the validation
diagnostics.

### Reads mirror what exists, not what `waml-wasm` used to export

The draft justified a `Model` read projection by pointing at `waml-wasm`'s
`build_model` export and its Tsify-generated TypeScript types. That crate is
gone and nothing generates TS types anymore. The justification changes; the
conclusion mostly survives:

| Endpoint | Returns | Mirrors |
| --- | --- | --- |
| `GET /api/bundle` | Bundle Envelope v1 bytes, revision in `X-Waml-Revision` | `run_batch --stdout` (`encode_bundle_envelope`); decoded by the editor's existing `decode_boot_bundle` |
| `GET /api/model` | `{ revision, model }` JSON | `uml::Projection` (= `waml::model::Model`), serde-serialized under the `waml/serde` feature |
| `GET /api/diagnostics` | `{ revision, diagnostics }` JSON | the `waml check` validation pass |

`GET /api/bundle` is the envelope, not a JSON files array, because the editor
already ships a decoder for exactly that format on its `?bundle=` path — the
`?api=` boot reuses it byte-for-byte instead of growing a second bundle codec.

`GET /api/model` stays because its consumer argument still holds: a thin client
(a script, CI, another tool) reads the projection and never links a parser. The
`Model` type is serde-serializable and is the same projection the editor and
`build_scene` consume. What is *lost* with `waml-wasm` is generated TypeScript
typings for it; a JS consumer reads untyped JSON. That is acceptable for the
clients this exists for, and generating typings again is additive later.

### Revision counter, conflicts, external edits

`revision` is a monotonically increasing counter owned by the server, bumped on
every successful apply. Every mutating request carries the revision the client
believes it holds; a mismatch answers `409 Conflict` and the client re-reads
`GET /api/bundle`. This is the same optimistic shape the editor already uses
internally (`SaveTicket.revision` / `SaveCompletion.revision` in
`editor_session.rs`), so the HTTP arm slots into the existing save protocol
rather than inventing a parallel one.

The counter tracks *our own* writes. A file changed underneath the server by
another process is not noticed; the editor already has a file-watching ingress
designed for exactly this (`replace_external_document`, currently
`#[allow(dead_code)]` pending native watching), and wiring a watcher into
`serve` is a follow-up, not part of this.

### The web artifact is embedded — landed, with a feature gate

This decision has been implemented since the draft. The built editor is
compiled into the `waml` binary as brotli-compressed `EmbeddedAsset`s, behind
the `embed-web` cargo feature, packaged by `scripts/package-web-artifact.mjs`
and located at build time via `WAML_WEB_EMBED_DIR`. A binary built without the
feature (or without an artifact) explains itself
(`web_artifact::WebArtifactError::NotEmbedded`) rather than serving nothing.

`waml serve` on such a binary degrades to `--api-only` behaviour with a warning
naming the build flags — the API does not depend on the artifact, and refusing
to serve it would make the dev-loop binary useless.

The stored bytes are served with `Content-Encoding: br`, straight from the
embedded slices — no runtime compression or decompression. This is the one
consumer the compressed form was kept for; `waml export site` decompresses
because it writes raw files (`web_artifact.rs` documents this split). A client
that does not advertise `br` in `Accept-Encoding` gets `406` on UI routes;
every browser that can run the wasm supports brotli.

The site is assembled through the same `assemble_site` the exporter uses, with
`SiteSource::Api` — which writes a `waml-boot.txt` of `?api=/api` and no
bundle. Path-safety and duplicate checks come with the shared assembler for
free.

*Noted, not in scope:* fonts remain a large share of the compressed payload;
pruning them is separable work.

### Boot and token delivery

The server prints one line on start:

```
waml serve  http://127.0.0.1:8099/?api=/api&token=<token>   (serving C:\dev\waml\docs)
```

The URL carries both the boot source and the token, because that is the one
channel that reaches the wasm side: the editor reads `WebParams.search` at
startup and `select_browser_boot` already yields
`BrowserBootSource::Api { base, token }` from exactly this query. The served
`waml-boot.txt` (`?api=/api`, tokenless, from the shared assembler) is a
fallback for a hand-typed bare URL: the editor boots, calls the API, receives
`401`, and shows that — a named failure, not a blank page. The token is never
written into the boot config, so no on-disk or fetchable asset carries it.

### Same-origin only — cross-origin was empirically disproven

Unchanged from the draft, and the finding still stands. Serving the hosted
GitHub Pages UI against a local API does not work: tested with headless
Chromium 149 from the real `https://redoz.github.io/waml/` origin, the request
died in the browser —

```
blocked by CORS policy: Permission was denied for this request to access the
`loopback` address space.
```

The preflight never left the browser, making
`Access-Control-Allow-Private-Network` dead weight (both header spellings were
sent; neither had any effect). Controls passed, so this is browser policy, not
a broken rig. Therefore the UI is served from the same origin as the API.
Untested and recorded as unknown: whether *headed* Chrome offers a permission
prompt that would flip this. Even if it does, a prompt-gated cross-origin path
is not a design worth building on.

## Command surface

Implemented (parse layer and stub only) in `crates/waml-cli/src/main.rs` and
`crates/waml-cli/src/serve/mod.rs`:

```
waml serve [DIR] [--port <PORT>] [--bind-all] [--api-only] [--no-open]
```

- `DIR` — directory to serve and edit. Defaults to `.`.
- `--port` — default `8099`. `--port 0` binds an ephemeral port and prints it.
- `--bind-all` — bind `0.0.0.0` instead of `127.0.0.1`. Warns on stderr naming
  the exposure.
- `--api-only` — suppress the embedded UI routes. For running the API against a
  locally built UI on another port; **not** a hosted-UI enabler (blocked by the
  browser regardless, see above).
- `--no-open` — do not launch a browser.

The token is printed exactly once, in the startup URL. `Ctrl-C` shuts down.
Exit codes match the rest of the CLI: 0 ok, 2 I/O failure.

`serve` is the CLI's second server; like `crate::lsp` it owns transport and
process lifetime only, and delegates every semantic decision to `waml` proper —
`serve/mod.rs` states this as its layout rule.

## API

All routes are under `/api/`. All require authentication (see Security). All
JSON responses carry `revision`; the envelope route carries it as
`X-Waml-Revision`.

| Method | Route | Body | Response |
| --- | --- | --- | --- |
| `GET` | `/api/bundle` | — | Bundle Envelope v1, `X-Waml-Revision` header |
| `GET` | `/api/model` | — | `{ revision, model }` |
| `GET` | `/api/diagnostics` | — | `{ revision, diagnostics }` |
| `POST` | `/api/ops` | `{ revision, ops: OpDto[] }` | `{ revision, changed: [path, markdown][] }` |
| `POST` | `/api/documents` | `{ revision, writes: [{ path, baseline, desired }] }` | `{ revision }` |

Status codes: `200` success · `401` missing/bad token · `403` Origin, Host or
custom-header check failed · `409` revision mismatch · `422` mutation rejected
(ops: the `EditError` `{ index, op, selector, reason }`; documents: validation
diagnostics) · `406` client does not accept brotli (UI routes only) · `500`
filesystem error, body carries the path and the OS error.

`POST /api/ops` responds with only the documents the batch touched — returning
the whole bundle per save is wasteful on a bundle of any size. A client wanting
a fresh model afterwards issues `GET /api/model`; on loopback that round trip
is free. `GET /api/okf` is still deliberately omitted: no consumer has asked.

Everything not under `/api/` is the embedded UI. Unknown asset paths are `404`
— the server never falls back to the shell for an unmatched path, because that
turns a typo into a silent blank page.

## Security

The threat is unchanged: a local process that rewrites files on disk, reachable
by any other local process and by any web page open in the user's browser.

1. **Token, always required.** 256 bits from a CSPRNG, fresh per invocation,
   never persisted. Accepted as `Authorization: Bearer <t>` or `?token=` — the
   query form is required because the wasm editor reads its parameters from
   `WebParams.search` and has no JS-to-wasm channel for a header. Compared in
   constant time. Required even on loopback; loopback is not access control.
2. **Custom header on mutating routes.** `POST /api/ops` and
   `POST /api/documents` additionally require `X-Waml-Client: 1`. A hostile
   page cannot set a custom header on a simple request, so the browser is
   forced into a preflight, which then fails the Origin check. This closes the
   `text/plain` simple-POST hole.
3. **Origin allowlist.** A request carrying `Origin` must match the server's
   own origin exactly. `Access-Control-Allow-Origin: *` is never sent;
   `Access-Control-Allow-Private-Network` is not sent at all (measured to have
   no effect).
4. **Host check.** Reject unless `Host` is `127.0.0.1:<port>`, `[::1]:<port>`,
   or — under `--bind-all` — an address the socket is bound to. Anti-DNS-
   rebinding: without it an attacker-controlled name resolving to 127.0.0.1
   would be same-origin to the browser.
5. **Path confinement.** Every bundle path is relative. Reject `..` segments,
   absolute paths, Windows drive prefixes and UNC paths, NUL bytes, and Windows
   reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM1..9`, `LPT1..9`,
   with or without extension). Resolve symlinks and require the result to stay
   under the served `DIR`. The embedded-asset side of this rule already exists
   as `site::is_safe_relative_path`, checked where writing happens; the API
   side extends it with the device-name and symlink rules a live filesystem
   needs.
6. **Bind loopback by default.** `--bind-all` is opt-in and warns.
7. **No shell-out, no arbitrary-path reads.** UI bytes come only from the
   embedded blob; filesystem access is confined to `DIR`.

Items 2–4 are defence in depth. Item 1 is the actual access control.

## Editor-side changes

The guiding constraint stands: no web-specific concepts leak into shell code.
All backend difference stays behind the seams that already exist.

- **Boot.** `BrowserBootSource::Api { base, token }` stops falling through to
  the start screen: the editor issues `GET {base}/bundle`, decodes the envelope
  with the existing `decode_boot_bundle`, and opens it — the same path a
  `?bundle=` fetch takes today, plus the `Authorization`/`?token=` credential
  and the captured revision.
- **Save.** `save_backend` (in `crates/waml-editor/src/app/workspace.rs`) gains
  an API arm alongside today's wasm arm (URL fragment via
  `browser_save_fragment`) and native arm (`native_save::save_ticket_atomic`).
  The arm is chosen once at startup from the boot source, and it is transport,
  not platform: it consumes the same `SaveTicket` and produces the same
  `SaveCompletion` as the other two, POSTing `/api/documents` with the
  baseline-guarded writes the native arm already computes. `mark_dirty`, the
  save debounce, `SaveFeedback`, and the quit-flush protocol are untouched —
  they sit above the seam.
- Transport is `cx.http_request`, which is cross-platform including web, so
  there is one code path rather than a `#[cfg]` fork.
- A `409` triggers a `GET /api/bundle` reload and a visible "reloaded from
  disk" notice rather than a silent clobber, entering through the same
  `replace_external_document` ingress built for native file watching.
- The editor keeps its own parser. Not negotiable: the Pages build and every
  static export have no server to project a model for them.

## Testing

- **Unit.** Path-confinement table (`..`, `C:\`, UNC, `CON`, symlink escape,
  NUL); constant-time token compare; revision/409 logic; the Origin/Host/
  custom-header rejection matrix. All pure functions, no socket.
- **Integration.** Bind an ephemeral port in-process, drive a full round trip
  against a temp bundle directory over both write routes, and assert response
  bodies and resulting bytes on disk. Includes: a deliberately unformatted op
  result lands canonically formatted, and a `/api/documents` request whose
  candidate fails validation writes nothing — the two claims that make "one
  validation funnel" tested rather than stated.
- **Contract.** Assert `GET /api/model` equals the serde serialization of the
  projection `prepare_candidate` produces for the same bundle, and
  `GET /api/bundle` round-trips through `decode_boot_bundle` to the same
  `SourceBundle`. Those equivalences are the entire justification for the read
  surface, so they get tests.
- **Browser.** `playwright-core` driving the ms-playwright `chromium-1228`
  build (`chrome-win64`, *not* `chrome-win`): load the embedded UI from the
  server, confirm the wasm boots without a console panic, confirm the printed
  `?api=&token=` URL opens the served directory, and confirm a page on a
  foreign origin is still blocked from the API — a regression guard on the
  finding that killed the hosted-UI option.
- **Not tested.** Whether headed Chrome offers a loopback permission prompt.
  Recorded as an open unknown.

## Out of scope

- WebSockets, live reload, or any server-push channel.
- Multi-client collaboration or operational transform. One editor at a time;
  the revision counter detects a stale client, it does not merge.
- Any authentication beyond the single per-invocation token.
- TLS. Loopback only, and `--bind-all` warns.
- Serving more than one directory, or any path outside `DIR`.
- `--api-only` as a hosted-UI enabler.
- Font pruning. Separable.
- Replacing or subsuming `waml lsp`.
- Detecting external edits to `DIR`. The ingress exists on the editor side
  (`replace_external_document`); wiring a watcher into `serve` is a follow-up.
- Regenerating TypeScript typings for the JSON API (`waml-wasm`'s Tsify output
  is gone). Additive later if a typed JS consumer appears.
