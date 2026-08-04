# OKF v0.2 reader — design

**Date:** 2026-08-05
**Spec under implementation:** `docs/specs/OKF_SPEC.md` (Open Knowledge Format v0.2)

## Problem

`docs/specs/OKF_SPEC.md` has been replaced with OKF v0.2. Our reader
(`crates/waml/src/okf.rs`, `crates/waml/src/okf/shell.rs`) implements v0.1.
Two v0.2 changes are breaking (§13.1), so a v0.2 bundle currently loses
meaning without erroring:

- `timestamp` is superseded by `generated: { by, at }`. We read `timestamp`
  only, so `Concept::timestamp` is `None` for every v0.2 document.
- The body `# Citations` list is superseded by frontmatter `sources`. We
  parse the body heading only, so `Concept::citations` is empty for every
  v0.2 document.

Nothing crashes: unknown frontmatter keys are collected into `Concept::extra`
(`shell.rs:326`), and `type` is an open string (`uml.rs:50`), so a v0.2
document parses and lands untyped. The work is *promotion*, not parsing —
`FmValue::Map`/`List` already represent v0.2's nested YAML, depth-capped at
32.

## Scope

In scope: the two breaking families, plus the lifecycle keys `status` and
`stale_after` (§5.4, §5.5) and the trust keys `generated`/`verified` (§5.2)
with their derived trust tiers (§5.3).

Out of scope, left untyped in `Concept::extra`: the `Attested Computation`
concept type and its `runtime`/`parameters`/`computation`/`executor`/
`attester` keys (§10), the `# Computation` body heading (§4.2), and per-claim
footnote attribution (§5.1). None has a consumer today. Each is additive
under §13.2, so a later change adds it without disturbing this one.

Files touched: `crates/waml/src/okf.rs` (types), `crates/waml/src/okf/shell.rs`
(promotion), and `docs/waml/architecture/concepts/model/okf-bundle.md` (a
version note). No other workspace crate and no TypeScript reads these
fields — the only consumers of `Concept::citations` are `shell.rs:350` and
unit tests in `okf.rs`.

## Design

### Sources replace citations

`Citation` is deleted. One list carries provenance regardless of which
syntax produced it:

```rust
pub struct UsageWindow {
    pub from: Option<String>,
    pub to: Option<String>,
}

pub struct Source {
    /// Stable key used to attribute individual claims (OKF §5.1).
    pub id: Option<String>,
    /// REQUIRED within an entry: a followable artifact or a scope descriptor.
    pub resource: String,
    pub title: Option<String>,
    /// Authority signal, in the actor convention (OKF §7).
    pub author: Option<Actor>,
    /// Adoption signal, framed by `usage_window`.
    pub usage_count: Option<f64>,
    /// When the source itself last changed, distinct from `generated.at`.
    pub last_modified: Option<String>,
    /// Entry-level override of the `sources` sibling `usage_window`.
    pub usage_window: Option<UsageWindow>,
}
```

`Concept::citations: Vec<Citation>` becomes `Concept::sources: Vec<Source>`.

A legacy `# Citations` body link maps in as
`Source { resource: href, title: Some(text), .. }` with every other field
`None`. A v0.1 entry is therefore distinguishable by its absent credibility
signals; no origin tag is stored.

The sibling `usage_window` (written once alongside `sources`) is resolved at
read time into each entry that lacks its own, so a consumer never has to
look outside the `Source` it holds.

**Precedence.** Frontmatter `sources` wins outright. The body `# Citations`
heading is parsed only when `sources` is absent — §13.1 says a consumer
SHOULD read `sources` and MAY still parse the legacy list, and merging the
two would manufacture phantom entries for a v0.2 document that kept a stale
heading. The heading text remains in `Concept::body` verbatim either way, so
nothing is lost.

### Trust and lifecycle

```rust
/// An actor in the OKF §7 convention: `human:ahormati`, `process:finance-nightly`,
/// or a bare id when no prefix was written.
pub struct Actor {
    pub kind: Option<String>,
    pub id: String,
}

pub struct Generated {
    pub by: Actor,
    pub at: Option<String>,
}

pub struct Verification {
    pub by: Actor,
    pub at: Option<String>,
}

pub enum Status {
    Draft,
    Stable,
    Deprecated,
}
```

New `Concept` fields:

```rust
pub generated: Option<Generated>,
pub verified: Vec<Verification>,
pub status: Status,
pub stale_after: Option<String>,
pub timestamp: Option<String>,   // retained; v0.1 only
```

- `generated.at` falls back to `timestamp` when `generated` is absent
  (§13.1). `timestamp` itself is retained on `Concept` so a v0.1 document
  round-trips unchanged.
- `verified` written as a bare `{ by, at }` mapping normalizes to a
  one-element list — §5.2 makes this a MUST.
- Absent `status` is `Stable` (§5.4), so `Status` needs no `Option`.

### Trust tiers are derived, never stored

```rust
pub enum TrustTier {
    Unverified,
    MachineConfirmed,
    HumanReviewed,
}

impl Concept {
    pub fn trust_tier(&self) -> TrustTier { /* from `verified` per §5.3 */ }
}
```

§5.3 defines tiers as inferred from `verified`, and §5.1 argues explicitly
against storing a credibility score because it is subjective and goes stale.
Storing a tier on `Concept` would be the same mistake one level up.

`stale_after` stays a raw date with no `is_stale()` helper: `waml` is
headless and has no clock, and `SystemTime::now()` panics on wasm. A caller
that wants the comparison passes today's date.

### Malformed input degrades, never rejects

§11 requires a consumer not to reject a document over these fields. Every
promotion failure is local:

| Input | Behaviour |
|---|---|
| `sources` entry without `resource` | Entry skipped; other entries still promote |
| `sources` not a list, or an entry not a map | No `Source` promoted |
| `generated` without `by` | `generated` stays `None` |
| `verified` entry without `by` | That entry skipped |
| `status` not one of the three tokens | Falls back to `Stable` |
| `usage_count` not a number | Field stays `None` |

In every one of these cases the raw value is **left in `Concept::extra`**.
`Concept`'s doc comment promises the projection is lossless — "nothing a
producer wrote is dropped" — and a value we declined to promote is exactly a
value we must not silently discard. Concretely: `KNOWN_KEYS` grows the new
keys, but a key is filtered out of `extra` only when its promotion actually
succeeded, not merely because the name is known.

No new `BundleError` variant. None of these is a bundle-level failure.

## Testing

Unit tests in `crates/waml/src/okf.rs`, beside the existing ones:

1. **v0.2 full promotion** — a document carrying `sources` (with all
   credibility signals and a sibling `usage_window`), `generated`,
   multi-entry `verified`, `status`, and `stale_after` promotes every field,
   and `extra` is empty.
2. **v0.1 still reads** — `timestamp` populates both `timestamp` and the
   `generated.at` fallback; a `# Citations` body list arrives as `Source`
   entries with `None` signals.
3. **Precedence** — a document with both `sources` and a `# Citations`
   heading yields only the frontmatter entries, and the heading text is
   still present in `body`.
4. **Bare-mapping `verified`** — normalizes to one element.
5. **Sibling `usage_window`** — resolved into entries that lack their own;
   an entry-level window overrides it.
6. **Malformed cases** — one test per row of the table above, each asserting
   both the degraded field *and* the surviving `extra` value.
7. **Trust tiers** — no `verified`, machine-only, and human-present.

Existing v0.1 fixtures and tests are left unmodified and must stay green;
that is the dual-read proof. Full gate: `cargo test --workspace`, plus the
`editors/vscode` test/lint/build.

## Risks

- **`Concept`'s serde surface has no production consumer.** Renaming
  `citations` to `sources` changes the derived JSON, but every serialize and
  deserialize of `okf::Bundle` in the workspace is a test: `okf.rs:674` and
  `serde_shape.rs:33-77`. `waml-cli` holds `OkfAnalysis` as Rust types
  (`bundle.rs:42`), `waml-editor` reads `&okf::Bundle` fields directly
  (`editor_session.rs:687`), and `waml-ops-dto` converts `DirectoryAddress`
  by hand (`lib.rs:686`) rather than through its `Deserialize`. `GET /api/bundle`
  returns raw `(path, markdown)` pairs (`serve/state.rs:56`), not a parsed
  bundle.

  The surface is a leftover read side from the retired Svelte frontend
  (`ef618e76`, "retire legacy web and WASM stack"); the write half survived as
  `waml-ops-dto`, the read half did not. Decision for this change: **leave it
  alone.** New types carry the same `cfg_attr` derives as their neighbours for
  consistency. Whether an OKF read route should exist — and whether it should
  serialize the domain type or a purpose-built DTO the way the write side
  does — is a separate question with its own spec.

  Practical consequence: the OKF assertions in `serde_shape.rs` never mention
  `citations`, so they pass untouched. They pin that the semantic collections
  stay separate and that `body` serializes as a string; `Source` holds only
  plain scalars, so both invariants survive.
- **`Source::resource` is `String`, not `Option<String>`.** An entry that
  cannot supply it is not a `Source` at all. This is what forces the
  skip-and-leave-in-`extra` rule rather than a half-built entry.
