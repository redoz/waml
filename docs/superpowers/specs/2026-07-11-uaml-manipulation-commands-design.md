# UAML manipulation commands — design

Date: 2026-07-11
Status: Approved (design). Implementation not started.

## Goal

Extend the `uaml` CLI beyond `check` + `fmt` with a logical, composable command set
for manipulating UAML/OKF documents (create / inspect / edit / rename nodes, attributes,
enum values, relationships). Must serve both a human at a terminal and an LLM agent driving
it programmatically. Diagram mutation (members, render hints) is explicitly out of scope for
this cut — diagram shape is still undecided.

## Decisions (settled during brainstorming)

1. **Mutation architecture: op-engine + noun-verb sugar.** A pure core mutation engine keyed
   on a typed `Op` list, with thin noun-verb CLI subcommands that each build exactly one `Op`,
   plus an `apply` subcommand that takes a raw `Op[]` JSON batch. One mutation source of truth
   for three producers: human at a terminal, LLM agent, and the web canvas (see below).
1b. **The `Op` JSON is a durable cross-tool contract, not just an agent batch.** The OKF Canvas
   web UI, running in a static `file://` context with no backend to persist edits, can record
   every user action (add attribute, rename node, draw relationship…) as an `Op`, export the log
   as JSON, and hand it to `uaml apply` to replay onto the on-disk bundle. So the `Op` schema
   must be language-neutral and stable: op names and field shapes are shared between the Rust
   engine and the TS web side (`packages/okf`). The web side is **not** built in this cut, but
   the schema is designed now to serve it. The op-log is **NDJSON / JSONL — one `Op` object per
   line** — so the web UI appends a single line per user action with no array-bracket
   bookkeeping, `apply` streams it line-by-line from a file or stdin, and it diffs cleanly in git.
   An **optional leading header line** `{ "uamlOps": 1 }` carries the schema version: if the first
   line is an object with a `uamlOps` key it is consumed as the header (and replay rejects an
   unknown version); otherwise version 1 is assumed and every line is an op. Blank lines are
   ignored.
2. **Addressing: a shared `Selector` type built on the `NoteAnchor` vocabulary.** `NoteAnchor`
   is already a pointer-to-model-element vocabulary (a classifier by slug; a relationship by
   source+name or source+verb+target). `Selector` adopts those forms for nodes and
   relationships and extends them with sub-selectors for attributes and enum values. The anchor
   grammar is written once; the still-deferred `annotates`/notes feature reuses it later.
3. **Engine operates on the whole bundle.** `apply(bundle, &[Op]) -> Result<Bundle, OpError>`,
   `Bundle = Vec<(path, text)>`. Single-doc ops touch one entry; `node rename` sweeps all
   entries. Pure text-map → text-map; all I/O stays in the CLI.
4. **Atomic, in-place default, with `--dry-run` and `--stdout`.** A command or batch is
   all-or-nothing: any op fails → nothing written, exit 1, the error names the failing op.
   Success writes touched files in place (like `fmt`). `--dry-run` prints a unified diff and
   writes nothing. `--stdout` emits the resulting bundle for piping.
5. **Refuse corruption, allow forward references.** The engine refuses structurally-invalid
   ops (duplicate attribute name, duplicate enum literal, ends on a forbidden verb / missing on
   a required verb, slug collision on rename). It allows forward references (a relationship or
   type-ref to a not-yet-created node) — `check` flags those as warnings, unchanged. Unknown
   `##` sections, note `## Body`, and unknown frontmatter keys are never dropped.

## Scope

**In this cut:** node (new / rename / set / rm / show), attribute (add / set / rm),
enum value (add / rm), relationship (add / set / rm), query (`show`, `refs`), and the `apply`
JSON batch.

**Deferred:** diagram member and render-hint mutation commands (`member *`, `hint *`) and their
`Selector::Member` / `Selector::Hint` variants; full `annotates`/notes resolution and rendering
(only the anchor *grammar* is built now, as the selector spine). `node rename` still rewrites
diagram `## Members` and `## Render hints`, and `refs` still reports diagram members as
referrers — diagrams are read-and-rewritten-as-referrers even though no command edits them
directly.

## Architecture

### Core (`uaml` crate, pure — no std::fs / threads / OS)

New `ops` module (plus small additions to `grammar`):

- **`Selector`** — the addressing type:
  ```rust
  enum Selector {
      Node(String),                                   // slug
      Rel { source: String, by: RelBy },              // = NoteAnchor Named / Endpoint, minus note
      Attr { node: String, name: String },            // extension (notes never reach an attr)
      Value { node: String, literal: String },        // extension
  }
  enum RelBy {
      Named(String),                                  // as "name" / as [Ref]
      Endpoint { kind: RelationshipKind, target: String }, // verb + target slug
  }
  ```
  `parse_selector` / `render_selector` implement the anchor line grammar
  (`[Src](./src.md) associates [Tgt](./tgt.md)`, `[Src](./src.md) as "name"`,
  `[Classifier](./slug.md)`). This is the grammar `annotates` will reuse when notes land.
  The node/rel variants correspond one-to-one with `NoteAnchor::{Classifier, NamedAssoc,
  EndpointAssoc}`; keep them convertible so note resolution can share the parser.

- **`Op`** — one variant per sugar command, `serde`-(de)serializable (each line of the NDJSON
  op-log is one of these):
  ```
  NodeNew { slug, ty, title, stereotype?, description?, abstract? }
  NodeRename { from, to }              // to = new slug; new title inferred/settable
  NodeSet { slug, title?, description?, stereotype?, abstract?, ty? }
  NodeRm { slug, cascade }
  AttrAdd { node, name, ty, multiplicity?, visibility? }
  AttrSet { node, name, ty?, multiplicity?, visibility?, rename? }
  AttrRm { node, name }
  ValueAdd { node, literal }
  ValueRm { node, literal }
  RelAdd { source, kind, target, name?, ends? }
  RelSet { selector, ends?, name? }
  RelRm { selector }
  ```
  `ends` is the exact `<near> to <far>` clause string (near = source side), parsed by the
  existing end grammar. `ty` for an attribute is a bare token or a slug ref (rendered
  `[Title](./slug.md)`, title taken from the target doc when present, else the slug title-cased).

- **`apply(bundle: &Bundle, ops: &[Op]) -> Result<Bundle, OpError>`** — folds ops over a
  working copy of the bundle. Single-doc ops parse the target file to a `Document`, mutate the
  relevant `Section`, and re-serialize canonically. `NodeRename` and `NodeRm { cascade }` sweep
  every entry rewriting referrers (see below). Returns the full new bundle on success, or the
  first `OpError` (nothing partially applied — the working copy is discarded on error).

- **`OpError`** — `{ index: usize, op: String, selector: Option<String>, reason: String }`.
  Reasons are specific: `attribute 'id' already exists in order`, `verb 'depends' forbids ends`,
  `rename target slug 'line-item' already exists`.

### Cross-file rename (the riskiest op)

`NodeRename { from, to }` rewrites, across the whole bundle:
- the file key (`from.md` → `to.md`);
- the renamed doc's `title` frontmatter (and thus its `# Title`) when the rename implies a new
  title;
- every `## Relationships` line whose `target-slug` is `from` (in every other doc);
- every `## Attributes` type-ref `[Title](./from.md)`;
- every relationship `as [Ref](./from.md)` name link;
- every diagram `## Members` and `## Render hints` line referencing `from`.
It refuses if `to` already exists. Titles on referrers are preserved; only the slug/path in the
link changes. Covered by a golden test on the orders-domain fixture.

### CLI (`uaml-cli`)

- New subcommands under the existing clap enum, one arm per sugar command; each constructs a
  single `Op` and calls `apply` on the bundle read from the target paths.
- `apply` subcommand: read the NDJSON op-log from a file or stdin (`-`), parse the optional
  header line + one `Op` per line, call `apply`.
- Shared flags on mutating commands: `--dry-run`, `--stdout`, `--format human|json`.
- New `io` helpers: write-back of only the changed entries, unified-diff rendering for
  `--dry-run`, bundle emission for `--stdout`.
- `show` / `refs` are read-only: build the `Model`, print the resolved node/edges (`show`) or
  every referrer of a slug (`refs`).

### Data flow

```
flags / JSON  ->  Op[]  ->  apply(bundle)  ->  Ok(new bundle)  ->  { in-place write | --stdout | --dry-run diff }
                                           ->  Err(OpError)     ->  print error, write nothing, exit 1
```

Every touched file is re-emitted via `serialize_document`, so each edit also canonicalizes the
files it touches (same normalization as `fmt`).

## Command surface

```
# Nodes
uaml node new  <slug> --type uml.Class --title "Order" [--stereotype a,b] [--desc ...] [--abstract]
uaml node rename <old-slug> <new-slug>
uaml node set  <slug> [--title|--desc|--stereotype|--abstract|--type ...]
uaml node rm   <slug> [--cascade]
uaml node show <slug> [--json]
uaml list [--type uml.Enum]

# Attributes   <node> <name> [<type>]
uaml attr add  <node> <name> <Type> [--mult 0..1] [--vis +|-|#|~]
uaml attr set  <node> <name> [--type|--mult|--vis|--rename <newname>]
uaml attr rm   <node> <name>

# Enum values  <node> <LITERAL>
uaml value add <node> <LITERAL>
uaml value rm  <node> <LITERAL>

# Relationships   <source> <verb> <target>  (reads as the document bullet)
uaml rel add order composes order-line --ends "1 to 1..* lines"
uaml rel add order associates customer  --ends "1 order to 1 customer" [--as "places" | --as-ref places]
uaml rel add order specializes base                  # forbidden verb: no --ends
uaml rel set order composes order-line --ends "1 to *"
uaml rel rm  order composes order-line
uaml rel rm  order --as "places"

# Query
uaml show  <slug> [--json]
uaml refs  <slug>
uaml apply <ops.ndjson | ->  [--dry-run|--stdout] [--format human|json]   # NDJSON: one Op/line, optional {uamlOps:1} header line

# Global on mutating commands
--dry-run   # print diff, write nothing
--stdout    # emit resulting bundle to stdout
```

## Error handling

- Any op refused → nothing written, exit 1, `uaml: op <i>: <reason>` (human) or a JSON array of
  `{ index, op, error }` under `--format json`.
- I/O failure (unreadable path, unwritable file) → exit 2, `uaml: <e>`.
- `--dry-run` is informational: print the diff, write nothing, exit 0.
- Success → exit 0.

## Testing

- Pure `apply` unit tests, one per op plus its refusal cases (dup attr, dup literal, bad/missing
  ends, slug collision), no I/O.
- `Selector` parse/render round-trip tests over all three anchor forms.
- `apply` parses NDJSON: one `Op` per line, blank lines ignored, optional leading `{uamlOps:1}`
  header; an unknown `uamlOps` version is rejected with a clear error (replay-compat guard for
  the web op-log). A malformed line names its line number in the error.
- Apply-then-serialize is a canonical fixpoint (an edit leaves the file in canonical form).
- Cross-file rename golden on `tests/fixtures/orders-domain.md`: every referrer (rel target,
  attribute type-ref, `as [Ref]`, diagram member, hint) rewritten; unrelated content untouched.
- Gate (unchanged): `cargo test` (workspace) + `cargo clippy --all-targets` clean + golden
  fixture green + `cargo build --release`.

## Invariants carried from the tooling build

- Core crate stays pure/WASM-friendly: deps `regex` + `pulldown-cmark` only; no std::fs /
  threads / OS. Mutation API is text→text (bundle map in, bundle map out); the CLI does all
  reading/writing.
- "Never lose data" extends to every mutating command: never drop Unknown sections, note
  `## Body`, unknown frontmatter keys, or unknown classifier types — refuse rather than corrupt.
- Canonical form matches the TS serializer (default `[1]` omitted; links render
  `[Title](./slug.md)`; ended rels render `: <from> to <to>`); round-trip is a semantic fixpoint,
  not byte-identity.
- Node key = filename slug. The two-tier AST is deliberate: `Document` (per-file fidelity) is the
  tier single-file edits target; `Model` (resolved bundle graph) backs cross-ref queries (`refs`)
  and rename's referrer sweep.
