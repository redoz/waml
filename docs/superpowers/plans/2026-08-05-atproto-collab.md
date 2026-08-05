# Collaborative Editing over atproto — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two or more people edit the same waml workspace in near-real-time with no waml-operated server. Each collaborator's changes live in their own atproto repository. A shared workspace gets a permanent `at://` URI that anyone can open read-only.

**Architecture:** A new headless crate `waml-collab` holds an automerge document per shared *space*, mapping `BundlePath → Text` of raw `.waml` source. Local edits reach it as `SourceDelta::Text(Vec<TextSplice>)` — a type that already exists. Remote changes leave it through `EditorSession::replace_external`, a door that already exists. Records are published to the editor's PDS over `makepad_network`, which already abstracts HTTP and WebSocket across native and wasm, so no new JavaScript is required. Convergence rides on opaque automerge `change` records; a parallel, non-load-bearing `op` record publishes the semantic `OpDto` so other AppViews can read what changed.

**Tech Stack:** Rust, automerge 0.10, `makepad_network` (HTTP + WebSocket), atproto XRPC, Jetstream, existing `waml`/`waml-editor` crates.

## Global Constraints

- Worktree: run all commands from `C:/dev/waml/.claude/worktrees/wobbly-wishing-steele`.
- Spec: `docs/superpowers/specs/2026-08-05-atproto-collab-design.md`. Read it before Task 1.
- No Co-Authored-By Claude trailer on commits.
- Gate every task with `cargo test --workspace`. Tasks touching the editor also run the vscode extension's test/lint/build.
- `waml-collab` must not depend on `waml-editor`, on makepad widgets, or on a window. Everything in it is testable headless. This is not negotiable — it is the rule that keeps native and web from drifting.
- No `unwrap`/`expect` on any path reachable from a remote peer's bytes. Remote input is untrusted input.
- automerge on wasm requires `--cfg getrandom_backend="custom"` plus a Rust-supplied `__getrandom_v03_custom`. Task 3 establishes this; do not skip it and discover it on web later.
- Auth in this plan is app passwords only (`com.atproto.server.createSession`). OAuth is a deliberate follow-up, listed at the bottom.

---

### Task 1: Publish the splice types from `waml`, scaffold `waml-collab`

`SourceDelta` and `TextSplice` in `crates/waml/src/edit/reversible.rs` are `pub(crate)`. They are exactly the CRDT splice input, so they become public API.

**Files:**
- Modify: `crates/waml/src/edit/reversible.rs` — make `SourceDelta`, `TextSplice` and its accessors public; add doc comments explaining that `TextSplice` is a minimal prefix/suffix-trimmed splice, one per document.
- Modify: `crates/waml/src/edit/mod.rs` — re-export from `waml::edit`.
- Create: `crates/waml-collab/Cargo.toml`, `crates/waml-collab/src/lib.rs`.
- Modify: root `Cargo.toml` — add the crate to the workspace, add `automerge` to workspace dependencies pinned to an exact version.

**Interfaces:**
- `pub struct TextSplice { .. }` with `path() -> &BundlePath`, `start() -> usize`, `removed() -> &str`, `inserted() -> &str`.
- `pub enum SourceDelta { Text(Vec<TextSplice>), Structural { before, after } }`.

- [ ] Make the types public with accessors rather than public fields, so the internal representation stays free to change.
- [ ] Scaffold `waml-collab` with a `lib.rs` that compiles and a `README`-level module doc stating the no-editor-dependency rule.
- [ ] Gate: `cargo test --workspace`.

---

### Task 2: Bundle ↔ automerge mapping

The core of the feature, and entirely headless. No network, no editor, no window.

**Files:**
- Create: `crates/waml-collab/src/doc.rs` — the automerge document wrapper.
- Create: `crates/waml-collab/tests/convergence.rs`.

**Interfaces:**
- `pub struct SpaceDoc` wrapping an `automerge::AutoCommit`.
- `SpaceDoc::from_bundle(&SourceBundle) -> Self`
- `SpaceDoc::apply_delta(&mut self, &SourceDelta) -> Result<(), CollabError>` — `Text` splices become `splice_text`; `Structural` reconciles the key set (documents added, removed, reordered).
- `SpaceDoc::to_bundle(&self) -> Result<SourceBundle, CollabError>`
- `SpaceDoc::save_incremental(&mut self) -> Vec<u8>`
- `SpaceDoc::merge_change(&mut self, &[u8]) -> Result<ChangedPaths, CollabError>`

- [ ] Write the round-trip test first: `SourceBundle → SpaceDoc → SourceBundle` is the identity, over the existing fixture bundles.
- [ ] Write the convergence property test: N simulated peers, randomised op and typing sequences, delivered in randomised orders, every peer's resulting text byte-identical. This is the test that makes the whole design trustworthy — write it before the implementation, and make it a proptest.
- [ ] Implement `SpaceDoc`. Commit timestamps must be injected, never read from a clock — `SystemTime::now()` panics on wasm.
- [ ] Reject `BundlePath` keys arriving from a merge that contain `..`, are absolute, or name a Windows device. A remote peer controls these strings.
- [ ] Gate: `cargo test --workspace`.

---

### Task 3: wasm buildability guard

This class of failure is invisible to a native gate and blanks the canvas on web. Establish it before any more code depends on automerge.

**Files:**
- Modify: `.cargo/config.toml` — `--cfg getrandom_backend="custom"` for `wasm32-unknown-unknown`.
- Create: `crates/waml-collab/src/rng.rs` — `__getrandom_v03_custom`, seeded explicitly by the caller.
- Create: `scripts/verify-wasm-imports.mjs` — compile the built artifact and assert its import list contains nothing unexpected.

**Interfaces:**
- `pub fn seed_rng(seed: u64)` — called once at startup from a platform-supplied entropy source.

- [ ] Implement the custom getrandom backend. Document *why* at the definition: makepad's wasm build is not wasm-bindgen based, so the browser backend's `__wbindgen_*` imports would fail to instantiate.
- [ ] Write `scripts/verify-wasm-imports.mjs` and wire it into the web build path, so a future dependency bump cannot silently reintroduce wasm-bindgen.
- [ ] Verify by building the wasm artifact and instantiating it against an empty import object.
- [ ] Gate: `cargo test --workspace`, plus a successful web build with the import check passing.

---

### Task 4: Lexicon record types

**Files:**
- Create: `crates/waml-collab/src/lexicon.rs`.
- Create: `lexicons/dev/waml/*.json` — the lexicon definitions themselves, checked in.

**Interfaces:** serde types for `dev.waml.space`, `dev.waml.change`, `dev.waml.op`, `dev.waml.snapshot`, `dev.waml.presence`, per the spec's Records section.

- [ ] Reuse `waml-ops-dto::OpDto` for the `op` record body rather than defining a second vocabulary.
- [ ] The `op` lexicon description must state that records are *intent*, not resulting state — after merging, the document may not reflect what an op asked for. Consumers will misread them otherwise.
- [ ] Confirm the NSID namespace with the user before finalising; NSIDs are domain-backed and these records outlive the application. See the spec's open question.
- [ ] Gate: `cargo test --workspace`.

---

### Task 5: Transport trait, XRPC client, session

**Files:**
- Create: `crates/waml-collab/src/transport.rs` — the trait and the in-memory fake.
- Create: `crates/waml-collab/src/xrpc.rs` — `createSession`, `createRecord`, `putRecord`, `getRecord`, `listRecords`, `uploadBlob`.

**Interfaces:**
- `pub trait Transport { fn request(..); fn open_socket(..); .. }` — implemented once over `makepad_network`, and once in-memory for tests.

- [ ] Build the in-memory fake first; every later task tests against it.
- [ ] Auth is app passwords via `com.atproto.server.createSession`. Store the token in memory only for now; persistence is a follow-up alongside OAuth.
- [ ] Every failure path returns a `Result` with a diagnostic naming the operation and the space. No swallowed errors — the observability rules apply.
- [ ] Gate: `cargo test --workspace`.

---

### Task 6: Flush scheduler and rate-limit budget

**Files:**
- Create: `crates/waml-collab/src/flush.rs`.

**Interfaces:**
- `pub struct FlushScheduler` — coalesces changes on a five-second timer, merges buffered automerge changes into one, and writes via the rolling-tail strategy.
- `pub struct Budget` — 5,000 points/hour, 35,000/day; CREATE 3, UPDATE 2, DELETE 1.

- [ ] Rolling tail: `putRecord` into the current `change` record (2 points) until it exceeds a size threshold, then start a new one. Cheaper per flush than create-per-update.
- [ ] Budget accounting is arithmetic — unit-test it directly, including the degrade-the-interval path when the budget runs low.
- [ ] Exceeding budget lengthens the flush interval and surfaces that state. It must never drop a change.
- [ ] Time is injected, not read from a clock.
- [ ] Gate: `cargo test --workspace`.

---

### Task 7: Jetstream subscription and CID verification

**Files:**
- Create: `crates/waml-collab/src/jetstream.rs`.

- [ ] Subscribe filtered by `wantedCollections` and `wantedDids`.
- [ ] Verify every event: fetch the record via `com.atproto.repo.getRecord` and compare `cid` against `event.commit.cid` before applying. Jetstream strips signatures, so this is the only authorship check available.
- [ ] Drop events from DIDs not on the space's editor list *before* verification, to bound request volume.
- [ ] Socket loss degrades to polling, reconnects with backoff, and is visible in the UI rather than silent.
- [ ] Test the failure paths against the fake transport: CID mismatch, unknown DID, dropped socket, malformed payload.
- [ ] Gate: `cargo test --workspace`.

---

### Task 8: Snapshots and compaction

**Files:**
- Create: `crates/waml-collab/src/snapshot.rs`.

- [ ] Full `save()` goes to a blob via `com.atproto.repo.uploadBlob`; the `snapshot` record holds the blob ref and covered heads. Records stay small; snapshots will not.
- [ ] Loading a space reads the newest snapshot plus changes after it.
- [ ] Snapshot on a change-count or size threshold, not a timer.
- [ ] Gate: `cargo test --workspace`.

---

### Task 9: Editor wiring — outbound

**Files:**
- Modify: `crates/waml-editor/src/editor_session.rs` or its caller in `app/actions.rs` — hand each `SessionChange` to the collaboration session.
- Create: `crates/waml-editor/src/collab_session.rs` — the thin editor-side holder.

- [ ] After each local edit, `SourceDelta::between(before, after)` gives the splices; apply to `SpaceDoc`, queue `save_incremental()`.
- [ ] When the edit originated from a `Step` batch, also queue its `OpDto`.
- [ ] Local editing must never block on the network. A rejected write retains the change and retries.
- [ ] Gate: `cargo test --workspace` and the vscode extension test/lint/build.

---

### Task 10: Editor wiring — inbound, and undo isolation

**Files:**
- Modify: `crates/waml-editor/src/collab_session.rs`, `crates/waml-editor/src/editor_history.rs` if required.

- [ ] Verified remote change → `merge_change` → for each changed path, `EditorSession::replace_external(document, base_revision, text)`.
- [ ] **Verify, do not assume,** that `replace_external` keeps remote edits out of the local undo stack. A collaborator's change is not something the user can undo. If it currently enters history, fix that here and add a test.
- [ ] A space that fails to load must not take down the workspace or the other open documents.
- [ ] Add a test that a malformed remote change produces a diagnostic rather than a panic.
- [ ] Gate: `cargo test --workspace` and the vscode extension test/lint/build.

---

### Task 11: Share and join UI

**Files:**
- Create: a collaboration panel widget in `crates/waml-editor/src/`.
- Modify: `crates/waml-editor/src/app.rs` — register the widget's module. An unregistered widget is silently dropped with a green gate; register child widgets before their consumers.

- [ ] Sign in (handle + app password), create a space, copy its `at://` URI, join a space, manage the editor list.
- [ ] **The public-data warning is the most important element in this feature.** Everything published is world-readable and effectively permanent. Say so at the moment of sharing, not in a settings page.
- [ ] Show connection state — connected, degraded to polling, rate-limited, signed out.
- [ ] Widget behaviour needs visual verification; a green gate is not evidence for a drawing change. State the verification performed.
- [ ] Gate: `cargo test --workspace` and the vscode extension test/lint/build.

---

### Task 12: Presence

**Files:**
- Create: `crates/waml-collab/src/presence.rs`; presence rendering in the editor.

- [ ] One `presence` record per editor per space, **overwritten** not appended — 2 points, no accumulation.
- [ ] Own five-second timer, independent of the change flush.
- [ ] Stale presence expires; a collaborator who closes the editor must not leave a permanent ghost cursor.
- [ ] Gate: `cargo test --workspace` and the vscode extension test/lint/build.

---

### Task 13: Measure the web boot cost

- [ ] Run `scripts/measure-web-boot.mjs` before and after, interleaved A/B. automerge adds roughly 1.1 MB to the artifact and boot cost is measured, not guessed.
- [ ] Record the numbers in the spec. If the regression is material, that is a finding to raise, not to absorb silently.
- [ ] Gate: `cargo test --workspace`.

---

## Deliberate follow-ups

Not in this plan, and each has a reason.

- **OAuth.** Roughly 40% of the total work and blocks nothing. App passwords ship the feature; OAuth polishes it. Web uses a `client_metadata.json` on the existing Pages deployment; native uses the localhost client_id exception or a custom scheme. PKCE and DPoP in Rust so both platforms share them.
- **Per-op splices.** `SourceDelta::between` emits one prefix/suffix-trimmed splice per document, so an op touching two distant places produces one splice spanning both, clobbering more CRDT range than necessary. Recording exact ranges during lowering — where `replace_range` already knows them — would improve concurrent-merge quality.
- **Placement CRDT.** `place.set` writes coordinates into text, so two people dragging the same node conflict at the text level instead of resolving last-writer-wins.
- **Semantic conflict diagnostics.** Use the existing post-merge OKF analysis to flag "this merge changed a name neither of you asked for". Cheap, because we re-analyse on every change already.
- **Private spaces.** Blocked on atproto shipping private data. Not fixable at our layer.
