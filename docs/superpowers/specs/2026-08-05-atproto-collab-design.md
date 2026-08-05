# Collaborative editing over atproto

Status: design, awaiting review
Date: 2026-08-05

## Why

waml documents are text files. Sharing one today means sharing a repository, and
collaborating on one means taking turns. The obvious fix — a sync server — is the
thing we do not want: infrastructure to own, pay for, and eventually shut down,
which would take every shared document with it.

atproto offers a way out. It is a federation of personal data servers already
maintained by a large community for reasons that have nothing to do with us. If we
store collaborative state in each editor's own repository, a shared waml document
survives us. Nobody has to trust a waml server, because there isn't one.

This design follows the shape Jake Lazaroff described in *Building more resilient
local-first software with atproto*, and departs from it in one place that matters:
we publish a legible, lexicon-shaped operation log alongside the opaque merge data,
so other applications on the network can read what changed.

The whole feature is public-only. That is a deliberate limit, discussed under
Non-goals.

## What we are building

Real-time collaborative editing of a waml workspace between two or more people,
with no waml-operated server. Convergence within roughly five seconds. Presence
(cursors, selection) between active editors. Every collaborator's contribution
lives in their own atproto repository under their own identity.

Because that mechanism stores the document durably and publicly, it also delivers
asynchronous sharing for free: a workspace gets a stable `at://` URI that anyone
can open read-only, forever, without an invitation.

## Non-goals

- **Private or encrypted documents.** atproto repository records are public. Until
  atproto ships private data, everything shared this way is world-readable. The
  share affordance must say so unmissably; see Security.
- **Open public editing.** Only DIDs on a space's editor list are merged. There is
  no "anyone can edit" mode, because there is no way to bound abuse without a
  server.
- **Replacing the filesystem.** Local `.waml` files remain the primary artifact.
  A shared space is a projection, not a migration.
- **Sub-second collaboration.** Rate limits put a floor near five seconds. Two
  people typing the same sentence simultaneously will not feel like Google Docs.

## Findings that shaped the design

These were verified against the codebase and by building probes, not assumed.

**makepad already owns the IO seam, so this needs no new JavaScript.**
`platform/network` defines a `NetworkBackend` trait — `http_start`, `http_cancel`,
`ws_open`, `ws_send`, `ws_close` — with native backends for Windows, Linux, Apple
and Android, and `WasmNetworkShimBackend` (`platform/src/os/web/web_network.rs`)
routing to `fetch` and `WebSocket` in `web.js` on wasm. XRPC calls and the Jetstream
socket are ordinary uses of an existing cross-platform Rust API.

**Text is already the single source of truth for every gesture.** Canvas drags,
inspector edits and source typing all become an `EditIntent { edit: PendingEdit, .. }`,
and `PendingEdit::lower()` produces a new `SourceBundle`. Nothing bypasses text.
A CRDT over text therefore captures every edit path without per-gesture work.

**Lowering is surgical, not a re-render.** `uml/lower.rs` resolves each op to a
`TextRange` and calls `replace_range` on existing source. Comments, formatting and
hand-written prose survive an op. `.waml` stays a hand-authored file, so the
git-compatibility argument for this feature holds.

**The splice type we need already exists.** `SourceDelta::between(before, after)`
returns `SourceDelta::Text(Vec<TextSplice>)`, where `TextSplice` is
`{ path, start, removed, inserted }` — precisely a CRDT splice. It is `pub(crate)`
in `waml::edit::reversible` and needs promoting. A `Structural` variant already
covers documents being added, removed or reordered.

**There are two kinds of edit, not one.** `PendingEdit` is either a semantic op
batch (`Step::Okf` / `Step::Uml`) or a raw `SourceDelta` from source-view typing.
This is the fact that decides the merge design, below.

**automerge is the only CRDT that works on makepad's wasm build.** makepad's wasm
is not wasm-bindgen based, so any dependency importing `__wbindgen_*` fails to
instantiate. Measured, by compiling each to `wasm32-unknown-unknown` and inspecting
module imports:

| library | imports | clock import | verdict |
|---|---|---|---|
| loro 1.13 | 29 `__wbindgen_*` | yes, `__wbg_now` | unusable without extensive shims |
| yrs 0.27 | 5 `__wbindgen_*` | no | needs a `fastrand` patch; yrs hardcodes `features = ["js"]` |
| automerge 0.10 | **0** | no | instantiates against an empty import object and runs |

automerge requires `--cfg getrandom_backend="custom"` for the wasm target plus a
Rust-supplied `__getrandom_v03_custom`. With that, the probe module instantiated
with `{}` as its import object and returned a correct result.

## Architecture

Five layers. Only the first three are new code; the last two are integration.

### 1. Space — the unit of sharing

A *space* is one record in the owner's repository:

```
dev.waml.space/<tid>  { title, editors: [did], createdAt }
```

Its `at://` URI is the shareable identifier. The owner's editor list is
authoritative: only changes authored by a listed DID are merged.

### 2. CRDT — the merge substrate

One automerge document per space, holding a map from `BundlePath` to a `Text`
containing the raw `.waml` source:

```
{ files: { "model/user.waml": Text, "diagrams/overview.waml": Text } }
```

Structure stays in the text. waml's parser remains the only thing that interprets
it, so there is no second model to keep in step.

### 3. Records — what actually goes in the repository

Four collections, and the split between the first two is the heart of the design.

```
dev.waml.change    { space, change: bytes, createdAt }
dev.waml.op        { space, ops: [OpDto], label, createdAt }
dev.waml.snapshot  { space, blob: blobref, heads: [...], createdAt }
dev.waml.presence  { space, cursor, selection, updatedAt }
```

**`change` is load-bearing.** It carries an automerge incremental change. It is
opaque, and convergence depends only on it.

**`op` is legible and is not load-bearing.** When an edit originated from a
semantic `Step` batch, we additionally publish the `OpDto` for it — `node.rename`,
`rel.add`, `place.set` and the rest. This is the deliberate improvement over the
article's design, and the answer to "shouldn't we use our Ops for this?":

> Ops are the right *wire vocabulary* and the wrong *merge algorithm*.

They are the right vocabulary because a record shaped `{op: "node.rename", from,
to}` is a lexicon. Another person's AppView can index a diagram's history, render
it, or build on it. A base64 CRDT blob makes atproto a dumb bucket and gives up the
entire point of being on the network. `OpDto` is already serde and already carries
a `v: u32` on every variant — it is close to a lexicon as written.

They are the wrong merge algorithm because ops are intent commands against a known
base state, and they do not commute. If peer A issues `node.rename{from: "user",
to: "account"}` while peer B issues `attr.add{node: "user", ..}`, the order decides
whether B's op finds its target. Replaying one op set in two orders yields two
documents, which is divergence. Making 29 op kinds converge pairwise is operational
transformation, whose transform matrix is the reason nobody ships OT. Worse, ops
address nodes *by name*, and `node.rename` mutates the addressing key, so a
concurrent edit to a renamed node cannot be reattributed. There is no stable node
identity in the model today to fix that with.

And decisively: source-view typing produces a raw `SourceDelta`, not an op at all.
An op-only wire cannot represent someone typing.

So we publish both. Convergence rides on `change`; legibility rides on `op`. An
`op` record is explicitly a record of *intent*, not of resulting state, and must be
labelled that way in the lexicon description — after merging, the document may not
reflect what the op asked for, and that is correct behaviour, not a bug.

**`snapshot`** compacts history. automerge's full `save()` grows; records should
stay small. Snapshots go in a blob via `com.atproto.repo.uploadBlob`, with the
record holding the blob reference and the covered heads. Loading a space reads the
newest snapshot plus changes after it, instead of the whole log.

**`presence`** is one record per editor per space, **overwritten** rather than
appended, so it costs 2 rate-limit points instead of 3 and does not accumulate.

### 4. Transport

`makepad_network` for both XRPC and Jetstream. A `Transport` trait in the crate
abstracts it so tests can run the whole sync loop against an in-memory fake with no
window and no network.

Jetstream (`wss://jetstream2.us-east.bsky.network/subscribe`) filtered by
`wantedCollections` and `wantedDids`. Jetstream strips signatures, so — as in the
article — every event is verified by fetching the record via
`com.atproto.repo.getRecord` and comparing `cid` against `event.commit.cid` before
applying. We only verify events for DIDs on the space's editor list, which bounds
the extra request volume.

### 5. Editor integration

Outbound, per local edit:

1. `EditorSession::apply_edit` already returns `SessionChange` with
   `affected_documents`.
2. `SourceDelta::between(before, after)` gives the splices — no diffing of our own.
3. Apply the splices to the automerge document.
4. `save_incremental()` produces the bytes to publish.
5. If the edit came from a `Step` batch, also queue its `OpDto`.

Inbound, per verified remote change:

1. `automerge::merge`.
2. For each changed path, take the new text.
3. `EditorSession::replace_external(document, base_revision, text)` — which already
   exists and already routes through incremental reparse.

`replace_external` is the correct door precisely because it means "this edit is not
the user's". Remote edits must not enter the local undo stack; a collaborator's
change is not something you can undo. The plan verifies this rather than assuming
it.

## Rate limits

The measured budget is 5,000 points per hour and 35,000 per day, with CREATE at 3
points, UPDATE at 2 and DELETE at 1 — so 1,666 creates or 2,500 updates per hour.

Two mechanisms, together comfortably inside budget:

- **Coalesce on a five-second timer.** Buffer local changes and merge them into one
  automerge change per flush. This sets the convergence floor.
- **Roll a tail record.** Rather than creating a record per flush, `putRecord` into
  the current tail `change` record (2 points) until it exceeds a size threshold,
  then start a new one. Cheaper per flush than the article's create-per-update and
  it keeps the collection small.

Presence updates on their own five-second timer, overwriting one record.

## Authentication

Staged deliberately, because OAuth is a large fraction of the total work and blocks
nothing else.

**Stage 1 — app passwords.** `com.atproto.server.createSession` with a handle and
an app password. Identical on native and web: no redirect, no DPoP, no hosted
metadata document. This ships the entire feature and is what most atproto tooling
still uses.

**Stage 2 — OAuth.** Required for a polished product and for users who should not
be pasting app passwords. Web uses the redirect flow with a `client_metadata.json`
served from the existing Pages deployment. Native uses either the `http://localhost`
client_id development exception or a custom scheme with a loopback listener. PKCE
and DPoP (ES256, per-request nonces) are mandatory and implemented in Rust so both
platforms share them.

## Crate structure

A new headless crate, `waml-collab`, depending on `waml` and `makepad_network`, and
on nothing in `waml-editor`. It owns:

- the automerge document and the bundle-to-CRDT mapping,
- lexicon record types and their serde contract,
- the XRPC client and Jetstream subscription behind `Transport`,
- the flush scheduler and rate-limit budget,
- session/auth state.

`waml-editor` gains only the wiring: a collaboration panel, share affordance,
presence rendering, and the two calls that connect `SessionChange` outbound and
`replace_external` inbound. This keeps every merge rule testable without a window,
as the testability rules require, and prevents a native/web split of the logic.

`waml` gains one small change: `TextSplice` and `SourceDelta` become public.

## Error handling and resilience

- A malformed or hostile remote change must produce a diagnostic, never a panic.
  Remote text reaches waml's parser, which makes this the untrusted-input boundary
  the security rules name; the existing `fuzz/` corpus applies directly.
- Losing the Jetstream socket degrades to polling and must be visible in the UI,
  not silent. Reconnect with backoff.
- A rejected write (rate limit, auth expiry) retains the change locally and retries;
  local editing never blocks on the network.
- Exceeding the rate-limit budget degrades the flush interval and says so, rather
  than dropping changes.
- A space that fails to load must not take down the workspace — one bad space, one
  bad document, per the resilience rules.

## Security

- **Everything published is public and permanent-ish.** The share affordance must
  state this at the moment of sharing, not in a settings page. This is the single
  most important piece of UI in the feature.
- Only DIDs on the editor list are merged; everything else is dropped before it
  reaches the parser.
- Every Jetstream event is CID-verified against the origin PDS before being applied,
  because Jetstream drops the signatures that would otherwise prove authorship.
- `BundlePath` values arriving from a remote peer are path-like and must be
  validated before they become map keys or touch the filesystem — `..`, absolute
  paths and Windows device names all rejected.
- No new dependency is added without pinning; automerge is a crates.io release, and
  the `getrandom` backend override is a build flag rather than a patched crate.

## Testing

- **Convergence property tests.** Generate op and typing sequences across N
  simulated peers, deliver them in randomised orders, assert every peer's text is
  byte-identical. This is the test that makes the design trustworthy.
- **Round-trip tests.** `SourceBundle` → CRDT → `SourceBundle` is the identity.
- **Full sync loop against the in-memory `Transport` fake**, including CID
  verification failure, dropped socket, and rate-limit rejection.
- **Rate-limit budget unit tests** — the accounting is arithmetic and should be
  tested as such.
- **wasm instantiation guard.** A check that the built artifact has no unexpected
  imports, so a future dependency bump cannot silently reintroduce wasm-bindgen and
  blank the canvas on web. This class of failure is invisible to a native gate.
- Presence and panel rendering are widget behaviour and need visual verification;
  the underlying state machines are lifted into `waml-collab` and unit-tested.

## Known weaknesses

Stated plainly, because each is a real cost.

- **Public only.** Discussed above. It is the largest limitation and it is not
  fixable at our layer.
- **Coarse splices in v1.** `SourceDelta::between` trims a common prefix and suffix
  and emits *one* splice per document. An op touching two distant places in a file
  produces a single splice spanning both, which clobbers more CRDT range than
  necessary and degrades concurrent-merge quality. The fix is to record exact
  ranges during lowering, where `replace_range` already knows them, and emit
  per-op splices. Deferred to a follow-up, and called out in the plan.
- **Concurrent layout drags merge poorly.** `place.set` writes coordinates into
  text, so two people dragging the same node produce a text-level conflict rather
  than a sensible last-writer-wins. A placement-specific CRDT container would fix
  it; not in v1.
- **Semantic conflicts are invisible.** Text merges cleanly while producing
  nonsense waml — two people renaming the same node differently, for instance. A
  follow-up should use the existing post-merge OKF analysis to flag "this merge
  changed a name neither of you asked for" as a diagnostic. Cheap, because we
  re-analyse on every change already.
- **`op` records can disagree with the merged result.** By construction: they
  record intent. Must be documented in the lexicon, or consumers will misread them.
- **Artifact size.** automerge adds roughly 1.1 MB to the wasm artifact
  (566 KB when size-optimised, but `[profile.release]` is inert for the web build
  since cargo-makepad owns those flags). Web boot cost is measured, not guessed —
  `scripts/measure-web-boot.mjs` must run before and after.

## Open question for review

Whether `dev.waml.*` is the right lexicon namespace. NSIDs are domain-backed, so
this should be a domain you control and intend to keep — the records outlive the
application, which is the whole point. `dev.waml` presumes a `waml.dev`.
