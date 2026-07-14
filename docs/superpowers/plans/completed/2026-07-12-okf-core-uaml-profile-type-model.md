# Two-tier type-model: OKF core + UAML profile (Rust)

## Context

`crates/uaml` (Rust) is now **the** core type-model (TS `packages/okf` mirrors it). But the model collapses two conceptual layers into one:

- **OKF** (`docs/specs/OKF_SPEC.md`) — Open Knowledge Format, domain-agnostic substrate. A Bundle of Concepts, one markdown doc each: free-text `type`, `title`/`description`/`resource`/`tags`/`timestamp`, body, untyped Links, Citations, plus reserved `index.md`/`log.md`.
- **UAML** (`docs/uaml-spec.md`) — a UML *profile* over OKF: refines `type` to `family.Metaclass`, adds stereotypes/attributes/relationship-verbs/diagrams.

Today `model::Model{nodes,edges,diagrams}` **is** the UML profile directly — no typed OKF tier beneath it. `build_node` (`parse.rs:387`) reads only `title/stereotype/abstract/description` + Attributes/Values/Body and **drops** `tags`/`resource`/`timestamp`/citations/links/unknown-sections. A non-UML doc (`Playbook`, future `bpmn.Task`) flattens to `Node{ty:Unknown(..)}` with everything stripped.

**Goal:** split into a genuine two-tier model — OKF foundation where every doc is a first-class typed `Concept` that round-trips losslessly, and a UAML tier where `Node` **wraps** a `Concept` plus UML-only fields. Rename family token `uml.*` → `uaml.*`. North star is the OKF standard, NOT the legacy front-end (which is being replaced on a separate track — no back-compat pressure).

## Three layers (structural key)

The lossless **syntactic** layer already exists — `syntax::Document` (frontmatter + typed `Section`s + `Line::Error`). Serialize/ops/round-trip all ride on it and keep doing so. Both semantic tiers **project from** `Document`; neither replaces it. The markdown bundle stays the round-trip truth; `Bundle`/`Model` are derived read-models. The whole refactor is therefore **additive read-only projection** — cannot regress round-trip.

```
raw bundle: Vec<(path, markdown)>
  │ parse::parse   (UNCHANGED, per file, lossless)
  ▼
syntax::Document   ← round-trip source truth
  │ okf::project   (per file)
  ▼
okf::Concept → okf::Bundle   ← agnostic foundation (ALL docs land here)
  │ uaml projects over uaml.* / Diagram concepts
  ▼
model::Model {nodes,edges,diagrams}   ← UML overlay; Node WRAPS its Concept
```

## Target type-model

### New module `crate::okf` (`crates/uaml/src/okf.rs`)

Hard rule: `okf` must NOT import any UML type (`ClassifierType`/`RelationshipKind`/`UmlMetaclass`). One-way dep: UML tier depends on `okf`, never the reverse — keeps a later `okf-core` crate split mechanical.

```rust
pub struct Concept {
    pub id: String,             // concept ID = full path minus ".md" (OKF §2)
    pub ty: String,             // serde "type" — FREE-TEXT, not ClassifierType
    pub title: Option<String>,
    pub description: Option<String>,
    pub resource: Option<String>,
    pub tags: Vec<String>,
    pub timestamp: Option<String>,
    pub body: String,           // full markdown body (section-agnostic, OKF)
    pub links: Vec<Link>,       // untyped (OKF §5.3)
    pub citations: Vec<Citation>,
    pub role: ConceptRole,      // reserved docs carried here, Bundle stays flat
    pub extra: Frontmatter,     // producer-specific frontmatter (reuse frontmatter.rs)
}
pub struct Link { pub text: String, pub href: String }
pub struct Citation { /* per OKF §8 */ }
pub enum ConceptRole { Concept, Index, Log }
pub struct Bundle { pub concepts: Vec<Concept> }   // EVERY doc lands here
```

### UAML tier — `crate::model`, `Node` wraps `Concept`

```rust
pub struct Node {
    pub concept: okf::Concept,   // foundation (id/title/description/tags/... live here)
    pub ty: ClassifierType,      // uaml.Metaclass refinement of concept.ty
    // UML-only extension fields (no home in Concept):
    pub stereotypes: Vec<String>,
    pub abstract_: bool,
    pub attributes: Vec<Attribute>,
    pub values: Vec<String>,           // uaml.Enum literals
    pub note_body: Option<String>,     // uaml.Note ## Body
    pub annotates: Vec<NoteAnchor>,
}
```

`Node`'s key is `concept.id`. `Edge`/`Diagram`/`ClassifierType`/`UmlMetaclass`/`Attribute`/`RelEnd`/`AssocName`/`NoteAnchor` keep current shapes. Non-UML concepts stay in `bundle.concepts` and get NO `Node`. Diagrams stay UML-tier-only. `RelationshipKind` stays **closed** (no `Unknown` arm) — agnostic relationships are `okf::Link`.

New / refactored functions:
- `build_bundle(bundle) -> okf::Bundle` — new; every doc → `Concept` via `okf::project`.
- `build_model(bundle) -> Model` — refactored to source generic fields from the projected `Concept`, re-reading only UML `Section`s for extension fields. Behavior-preserving (guarded by `golden.rs`).

### Full-path node keying (DECIDED: unify now)

Node key = `concept.id` (full path, e.g. `tables/orders`), replacing today's slug keying. This is the larger-blast-radius option, chosen deliberately so no legacy slug keying is left behind. Refactor to path-based keys:
- `parse.rs` `build_node`/`build_model` keyset + `keyset: &HashSet<&str>` become path-based.
- `validate.rs` `DuplicateSlug` detection keys on full path (`validate.rs:144`, tests ~317). Diagnostic code name stays `DuplicateSlug` unless a rename is trivially clean.
- Relationship + diagram-member target resolution (`solve/resolve.rs`, `validate.rs` `UnresolvedTarget`) resolve against full-path IDs. Confirm link-href → id normalization is consistent (`./order.md` → `order` vs nested `tables/order`).
- Expect golden churn (`golden.rs`, `ops_golden.rs`, `solver_golden.rs`) — regenerate/verify, not blindly accept.

### `uml.` → `uaml.` rename

- `model.rs`: `ClassifierType::parse` family check `"uml"` → `"uaml"`; `UmlMetaclass::as_str`/parse emit/accept `uaml.{name}` (`model.rs:270`).
- `Diagram.profile` default `"uml-domain"` → `"uaml-domain"` (`parse.rs:524` and every fixture string above). Separate axis from the family token, but rename together for consistency (DECIDED: yes).
- Fixtures/goldens/templates: every `type: uml.X` → `uaml.X` and `profile: uml-domain` → `uaml-domain` across `crates/uaml/tests/*`, `crates/uaml/src/**` inline fixtures, `packages/core/src/templates/*`.

### Unchanged
- `syntax.rs` / `parse::parse` — lossless per-file AST, the round-trip truth. Do not disturb.
- `serialize` / `ops` — operate on `Document`, below both tiers.
- `solve::solve_diagram` — consumes `model::Diagram`.

## Wire / TS (downstream, not a constraint)

Front-end is tainted legacy being replaced — no back-compat. Wire = whatever the clean Rust model serializes to.
- `crates/uaml-wasm/src/lib.rs`: add `build_bundle` (+ native `build_bundle_json` mirroring `build_model_json`). `build_model`/`validate`/`apply_ops`/`fmt`/`split_bundle` keep roles; `build_model` output shape changes to `Node`-wraps-`Concept` (fine — consumers rewritten).
- `packages/okf/src/types.ts`: regenerate fresh — `Concept`/`Link`/`Citation`/`ConceptRole`/`Bundle` + nested-`concept` `Node`. Do NOT carry legacy `position`/`members`/`hints`/`display`.
- `crates/uaml/tests/serde_shape.rs`: regenerate to pin the **new** model field names. Not a design constraint.

## Rollout (small green units on `main`)

1. `crate::okf` types + serde + unit tests. Additive, no wiring.
2. `okf::project` + `build_bundle` + tests: `Playbook`/`bpmn.Task` doc round-trips as first-class `Concept` (headline); `index.md`/`log.md` get roles; citations/links extracted.
3. `uml.`→`uaml.` + `uml-domain`→`uaml-domain` rename as its own green unit (family check, metaclass strings, all fixtures/templates).
4. Refactor `build_model` so `Node` wraps `Concept`; UML fields re-read from Sections. Behavior-preserving vs `golden.rs`.
5. Full-path node keying: switch keyset + resolution + `DuplicateSlug` to path IDs; regenerate goldens.
6. Wire: WASM `build_bundle`/`build_bundle_json`, regenerate `types.ts`, regenerate `serde_shape.rs`.

Each unit: green gate (below) → commit with `Plan-Tasks` trailer → push.

## Execution (implement-plan skill in a worktree)

Per prior instruction: promote this plan into the repo, then run `implement-plan` in a git worktree.

1. Copy this file into the repo: `docs/superpowers/plans/2026-07-12-okf-core-uaml-profile-type-model.md`.
2. Invoke `implement-plan` with `{plan:"docs/superpowers/plans/2026-07-12-okf-core-uaml-profile-type-model.md"}`.

**Skill patches required BEFORE running** (known wrong-repo template — see user memory `implement-plan-wrong-repo-template`):
- `REPO_DIR` → `C:\dev\uaml` (template hardcodes `C:/dev/vendor/owox`).
- `GREEN_GATE` → this repo's gate (below), NOT pnpm-only.
- `COMMIT_FOOTER` → empty. NO `Co-Authored-By: Claude` / Claude banner on any commit or PR (user memory `no-claude-commit-banner`).

## Green gate (run per unit)

```
cargo test -p uaml
cargo test -p uaml-wasm
pnpm -r test && pnpm lint && pnpm build
```

## Verification

Core-centric (front-end is out of scope, being replaced):
- `cargo test -p uaml` (unit + `golden.rs` + `serde_shape.rs` + `ops_golden` + `solver_golden`) green per unit.
- **Headline test**: author an OKF `Playbook` (non-`uaml.*` type with `tags`/`resource`/`timestamp`/citations/links) → `build_bundle` → assert every field survives on the `Concept`. (Impossible before this change.)
- Round-trip test: `fmt`/`apply_ops` on a mixed bundle (uaml + non-uaml docs) stays byte-lossless — proves tiers are read-only over `Document`.
- `cargo test -p uaml-wasm` covers `build_bundle_json`.
- `pnpm build` compiles regenerated `types.ts`. Old front-end app is NOT a gate.

## Open questions (defaults chosen; confirm only if blocked)

- **Concept ID = full path** — DECIDED, unify node keying now (this plan, unit 5).
- **`Concept.body` = full markdown** — default yes (section-agnostic OKF); accept that UML-tier `attributes`/`values` re-parse overlapping content.
- **Reserved docs via `role` field** — default yes; `Bundle` stays flat.
