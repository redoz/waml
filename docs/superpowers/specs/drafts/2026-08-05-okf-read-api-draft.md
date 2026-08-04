# OKF read API — draft

**Status:** DRAFT. Not designed, not approved, nothing scheduled. Captured so
the finding is not lost.
**Raised during:** the OKF v0.2 reader work
(`docs/superpowers/specs/2026-08-05-okf-v0-2-reader-design.md`), where the
`okf::Bundle` serde surface turned out to have no consumer. The v0.2 change
deliberately leaves that surface alone; this draft holds the question it
raised.

## The finding

`waml serve` has a write side and a read side, and they are not symmetric.

Writes:

- `POST /api/ops` — `OpDto`, from `waml-ops-dto`, a crate that exists purely
  to be the wire contract. Versioned, tested, deliberately decoupled from the
  domain types.
- `POST /api/documents` — raw text writes.

Reads (`crates/waml-cli/src/serve/routes.rs:34-38`):

- `GET /api/bundle` — raw `(path, markdown)` pairs. `serve/state.rs:56` is
  `encode_bundle_envelope(&self.prepared.source().to_pairs())`. Source text,
  not a parsed bundle.
- `GET /api/model` — `uml::Projection`. The UML view of the bundle.
- `GET /api/diagnostics` — validation output.

So a consumer can read the **text**, and can read the **UML** interpretation.
It cannot read the **OKF semantic layer**: concepts with their type, title,
resource, tags, links, and (after v0.2) sources, trust, and lifecycle. To get
that today a client must fetch the markdown and re-implement the reader.

`okf::Bundle` and `Concept` do carry serde derives, plus two hand-written
impls with real logic — `Bundle::deserialize` (`okf.rs:273`) keeps every field
required so a truncated wire form is rejected, and `DirectoryAddress::deserialize`
(`okf.rs:114`) validates on the way in. Nothing calls either. Every
serialize/deserialize site in the workspace is a test.

The explanation is historical: `ef618e76` (2026-07-28, "refactor: retire
legacy web and WASM stack") deleted `packages/web/src/**/*.svelte`. That
frontend was the read side's consumer. The write half was later rebuilt as
`waml-ops-dto`; the read half was not. The derives are its residue.

## The question

Should `waml serve` expose the OKF semantic layer, and if so, through what?

This matters more after v0.2 than it did before. v0.2 adds exactly the fields
a remote consumer would want to query rather than recompute:

- Which concepts are unverified, machine-confirmed, or human-reviewed (§5.3)?
- Which are `deprecated`, or past their `stale_after` date (§5.4, §5.5)?
- What sources back a concept, and what are their credibility signals (§5.1)?

Those are questions about the bundle, and the current answer is "fetch all the
markdown and parse it yourself."

## Open questions

Genuinely open — none of this is decided.

1. **Is there a consumer?** The Svelte frontend is gone and the native editor
   holds the Rust types directly (`editor_session.rs:687`). A read API with no
   client is how we got here. Plausible clients: an agent querying a knowledge
   bundle over HTTP, the VS Code extension, a static site generator, an
   external catalog importer. None exists today.
2. **Domain type or DTO?** Serializing `okf::Bundle` directly makes every
   `Concept` field change a wire break. `waml-ops-dto` exists because the write
   side hit that problem. A read side would probably want the same, which would
   make the current derives the wrong mechanism even if the feature is right.
3. **Whole bundle or queries?** `GET /api/okf` returning everything is simple
   and matches `GET /api/model`. But the interesting v0.2 questions are
   filters ("what's stale", "what's unverified"), and serving those as query
   parameters versus making the client filter a full payload is a real choice.
4. **Derived or raw?** Trust tier is *derived* from `verified` (§5.3), never
   stored — the spec is explicit that a stored score goes stale. Does the wire
   carry the derived tier for convenience, or only the raw `verified` list,
   leaving derivation to the client? Same question for staleness, which needs
   a clock the headless crate does not have.
5. **What happens to the existing derives?** If the answer is a DTO, the
   derives and both hand-written impls on the domain types become removable —
   along with the OKF half of `serde_shape.rs:33-77`. If the answer is
   "serialize the domain type", they are the contract and `serde_shape.rs`
   is correctly pinning it. Either way the current state — a defended wire
   boundary no code crosses — is not the answer.

## Not a decision

Nothing here is approved. If this gets picked up it goes through brainstorming
first, starting with question 1: name the consumer. If no consumer can be
named, the honest outcome is deleting the serde surface rather than building
a route to justify it.
