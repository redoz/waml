# Diagram Field Persistence

**Status:** Proposed (not yet planned)
**Date:** 2026-07-15
**Product:** WAML Model Canvas (`packages/web` Svelte 5) + `crates/waml` (Rust core, wasm)
**Scope:** Prerequisite plumbing for the *Diagram properties panel v2* spec
(`2026-07-15-diagram-properties-panel-v2-design.md`). This spec ships **no new
panel UI** — only the persistence path that the v2 panel builds on.

## Context

Per-diagram render settings and the diagram's own title currently do **not**
persist. Three facts combine into the gap:

1. **`store.updateDiagram` is a hard no-op.** `packages/core/src/state/model.ts`
   (the `updateDiagram(_key, _patch)` stub, ~L271-273) discards every diagram
   edit. `removeDiagram`/`addDiagram`/`addDiagramFromMembers` are likewise
   stubs; those stay out of scope here (diagram *creation*/membership is a
   separate deferred workstream — not "Stage 1c" as named in
   `2026-07-12-uaml-wasm-frontend-1b-design.md`, which there means the
   layout-from-rules engine).

2. **An interim in-memory session store masks the no-op.**
   `packages/web/src/state/displaySettings.svelte.ts` holds per-diagram
   `Partial<DiagramDisplay>` overrides keyed by diagram key. They reset on
   reload. `CanvasInner.svelte` reads them via `displaySettings.resolve(...)`
   (L153) and writes via `displaySettings.patch(...)` (`handleDisplayChange`,
   L331-333). Diagram title edits (TopBar rename → `store.updateDiagram(key,
   {title})`, `CanvasInner.svelte` L508) silently vanish.

3. **The model layer carries no diagram display/description at all.** The Rust
   `Diagram` struct (`crates/waml/src/model.rs` L389-399) is `key, title,
   profile, groups, layout` — no `description`, no display block. `DiagramDisplay`
   / `DEFAULT_DISPLAY` / `resolveDisplay` / `Diagram.display?` exist only TS-side
   as aspirational types (`packages/okf/src/types.ts` L90-126), never wired to
   the wire (`overlay.ts` `RustDiagram` has no `display`, L57-62; `toModelGraph`
   never sets `Diagram.display`, L151-156).

Diagrams are ordinary markdown docs with flat frontmatter. `build_diagrams`
(`crates/waml/src/parse.rs` L708-735) reads `title`/`profile` scalars.
Frontmatter (`crates/waml/src/frontmatter.rs`) is **deliberately flat**:
`FmValue` is `Str | Bool | Num | List<FmValue>`, no nested maps (see the comment
at L135). Any new diagram field must be a flat scalar/bool/num/list key, never a
nested `display:` block in the file.

Mutations flow through an op log: `crates/waml/src/ops/mod.rs` `enum Op`
(`NodeSet` at L77-84 is the shape to mirror), realized by `apply` →
`edit_doc` → `parse_document`/`serialize_document`. There is **no**
`DiagramSet` op yet. The DTO wire is `crates/waml-ops-dto/src/lib.rs` (`OpDto`,
`#[serde(tag="op")]`); TS emits `OpDto[]` via
`packages/core/src/state/ops-adapter.ts` (`updateNodeOps` is the pattern to
mirror). The wasm glue (`packages/wasm/src/generated/wasm-inline.ts`) is
regenerated from the Rust crate after changes.

## Goal

Give diagram `title`, a new `description` (free-text note), and the full
`DiagramDisplay` field set a **real persistence path** through the model: Rust
struct → flat frontmatter round-trip → `Op::DiagramSet` → DTO → wasm → ops
adapter → `store.updateDiagram`. Retire `displaySettings.svelte.ts` entirely.
Keep every existing consumer of `DiagramDisplay` compiling against the same
resolved shape it reads today. No new panel controls (that is the v2 spec).

## Non-goals

- No new panel UI, no textarea, no color picker, no rendering behaviour changes
  on the canvas (v2 spec owns all of that).
- No diagram create / delete / membership ops (`addDiagram` etc. stay stubs).
- No default-omission cleverness in the serializer (see "Serialization" below).

## The `DiagramDisplay` field set (final)

The field set the model must carry and round-trip. Names are the **TS field
names**; the Rust wire uses the identical camelCase keys (serde
`rename_all = "camelCase"`), and the flat frontmatter keys are identical again
(one frontmatter key per field). Existing fields keep today's semantics.

| Field (TS / frontmatter key) | Type | Absent ⇒ | Meaning |
|---|---|---|---|
| `showAttributes` | `boolean` | `true` | show attribute rows vs. a collapsed count |
| `attributeDetail` | `"name-only" \| "name-type"` | `"name-type"` | whether the attribute type token shows |
| `showAttributeVisibility` *(new)* | `boolean` | `true` | diagram-level gate on the `+/-/#/~` marker per attribute row |
| `showAttributeMultiplicity` *(new)* | `boolean` | `true` | independent gate on the `{mult}` suffix per attribute row |
| `maxAttributes` *(new)* | `number \| undefined` | `undefined` (unlimited) | cap on attribute rows drawn per box; excess folded as "+K more" |
| `associationLabels` | `"all" \| "hidden"` | `"all"` | association edge labels |
| `emphasizeMultiplicity` | `boolean` | `false` | emphasize multiplicity on labels |
| `showStereotype` | `boolean` | `true` | master on/off for the stereotype/keyword row |
| `stereotypeFilter` *(new)* | `string[] \| undefined` | `undefined` (show all) | allowlist of stereotype tag names to render; `[]` ⇒ show none |
| `stereotypeColors` *(new)* | `Record<string,string>` | `{}` | per-stereotype-name color override |

**Tri-state fields.** `maxAttributes` (`undefined` = unlimited vs a number) and
`stereotypeFilter` (`undefined` = all vs `[]` = none vs names) each carry a
meaningful *absent* state distinct from any present value. The persistence path
below preserves that three-way distinction end to end.

`DEFAULT_DISPLAY` stays the **single source of defaults** and lives only TS-side.
Rust never invents defaults; it round-trips exactly the frontmatter keys the file
holds. TS `resolveDisplay(partial)` fills the rest. `DEFAULT_DISPLAY` gains the
three new non-nullable defaults; the two nullable fields default by absence:

```ts
export const DEFAULT_DISPLAY: DiagramDisplay = {
  showAttributes: true,
  attributeDetail: "name-type",
  showAttributeVisibility: true,
  showAttributeMultiplicity: true,
  // maxAttributes omitted ⇒ undefined ⇒ unlimited
  associationLabels: "all",
  emphasizeMultiplicity: false,
  showStereotype: true,
  // stereotypeFilter omitted ⇒ undefined ⇒ show all
  stereotypeColors: {},
};
```

`DiagramDisplay` (TS, `types.ts`) gains `showAttributeVisibility: boolean`,
`showAttributeMultiplicity: boolean`, `maxAttributes?: number`,
`stereotypeFilter?: string[]`, `stereotypeColors: Record<string,string>`.
`resolveDisplay` is unchanged (`{ ...DEFAULT_DISPLAY, ...display }`) — the
spread now covers the new keys automatically. `Diagram.description?: string` is
added.

## Flat frontmatter encoding

The diagram doc frontmatter gains these flat keys (all optional; a key is
written only once its value has been authored via the panel):

```
title: "Order lifecycle"          # existing scalar (H1 kept in sync)
profile: "uml-domain"             # existing scalar
description: "Notes for reviewers…"   # NEW scalar — the diagram note
showAttributes: true
attributeDetail: "name-type"
showAttributeVisibility: true
showAttributeMultiplicity: false
maxAttributes: 6
associationLabels: "all"
emphasizeMultiplicity: false
showStereotype: true
stereotypeFilter: ["entity", "valueObject"]
stereotypeColors: ["entity:#ffedd5", "valueObject:#dbeafe"]
```

- **`stereotypeColors`** cannot be a nested map (frontmatter is flat), so it is a
  `List<Str>` of `"name:#rrggbb"` pairs. Split on the **first** `:` — name before,
  hex (which contains no `:`) after. Rust treats the whole list as an **opaque
  `Vec<String>` passthrough**: it does not parse or validate the hex. TS
  (`overlay.ts`) parses the list into `Record<string,string>` on the way in and
  serializes back to the list on the way out. A stereotype name is a WAML
  identifier (no `,` or `:`), so the `,`-delimited-list + first-`:`-split encoding
  is unambiguous; this constraint is documented, not enforced.
- **`stereotypeFilter`** is a `List<Str>`. The parser already round-trips `[]` to
  an empty list and back (verified: `parse_value("[]")` ⇒ `List([])`,
  `render_value(List([]))` ⇒ `"[]"`). The **presence** of the key is what carries
  the absent-vs-empty distinction, so the reader must probe `fm.get(key).is_some()`
  rather than `get_string_list` (which collapses absent and empty to the same
  empty `Vec`).
- **`maxAttributes`** is a `Num`. `render_value` already prints an integer-valued
  `f64` without a decimal point (verified). The reader maps a value `>= 1` to
  `Some(n as u32)` and anything else (or absent) to `None` (unlimited).

## Rust model changes (`crates/waml/src/model.rs`)

`Diagram` gains `description` and a `display` partial. The `display` field on the
**wire** is a partial (only authored keys present); it is NOT resolved against
defaults Rust-side.

```rust
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Diagram {
    pub key: String,
    pub title: String,
    pub profile: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub description: Option<String>,
    pub groups: Vec<DiagramGroup>,
    pub layout: Vec<crate::syntax::LayoutStatement>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "DiagramDisplay::is_empty"))]
    pub display: DiagramDisplay,
}

/// A diagram's authored render settings — a PARTIAL. Only keys present in the
/// file are `Some`/non-empty; TS `resolveDisplay` fills the rest from
/// `DEFAULT_DISPLAY`. Serde `rename_all="camelCase"` matches the TS keys.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", default))]
pub struct DiagramDisplay {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attributes: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub attribute_detail: Option<String>,          // "name-only" | "name-type"
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attribute_visibility: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attribute_multiplicity: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_attributes: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub association_labels: Option<String>,        // "all" | "hidden"
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub emphasize_multiplicity: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_stereotype: Option<bool>,
    /// `None` ⇒ key absent ⇒ show all; `Some(vec)` ⇒ allowlist (empty ⇒ show none).
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub stereotype_filter: Option<Vec<String>>,
    /// Opaque `"name:#rrggbb"` pairs; empty ⇒ key absent.
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Vec::is_empty"))]
    pub stereotype_colors: Vec<String>,
}

impl DiagramDisplay {
    pub fn is_empty(&self) -> bool { *self == DiagramDisplay::default() }
}
```

## Parsing (`build_diagrams`, `crates/waml/src/parse.rs`)

`build_diagrams` reads the new keys off `p.doc.frontmatter` into
`Diagram.description` and `Diagram.display`:

- `description` ← `fm.get_str("description")`.
- each `Option<bool>` field ← `fm.get_bool(key)` (already returns `Option`).
- `attribute_detail` / `association_labels` ← `fm.get_str(key).map(str::to_string)`
  (no validation — an unknown string round-trips and TS falls back through
  `resolveDisplay`/the segmented control).
- `max_attributes` ← `fm.get(key)` matched as `Num`, `Some(n) if n >= 1.0`
  ⇒ `Some(n as u32)`, else `None`.
- `stereotype_filter` ← **presence-preserving**:
  `if fm.get("stereotypeFilter").is_some() { Some(fm.get_string_list("stereotypeFilter")) } else { None }`.
- `stereotype_colors` ← `fm.get_string_list("stereotypeColors")` (empty when absent).

A diagram with none of these keys yields `description: None` and a default
(all-`None`/empty) `DiagramDisplay`, which `skip_serializing_if` omits from the
wire — so legacy diagram files stay byte-identical on read and their wire shape
gains nothing.

## Serialization

There is **no dedicated diagram serializer** to extend. Diagram docs round-trip
through the generic `serialize_document` → `render_frontmatter` path
(`crates/waml/src/serialize.rs`, `crates/waml/src/frontmatter.rs`), which emits
every `fm.entries` key. The `Op::DiagramSet` handler therefore only has to write
the right frontmatter entries (via the existing `fm_set` helper in `ops/mod.rs`);
`render_frontmatter` handles the rest. Display keys are written **explicitly and
in full** whenever the display block is set — no default-omission — so the file
after an edit deterministically holds exactly the keys the panel specified. Keys
are only ever materialized once the user changes a display setting; an untouched
diagram doc is never rewritten by this feature.

## New op: `Op::DiagramSet` (`crates/waml/src/ops/mod.rs`)

Mirrors `NodeSet` for the scalar fields (`title`/`description` are per-field
`Option`; `None` = leave unchanged). The display block is handled as a **whole-block
replace** rather than per-field-optional, because its tri-state fields
(`maxAttributes`, `stereotypeFilter`) cannot express "clear back to absent" with
a single `Option` per field. `display: Some(_)` replaces all display frontmatter
keys atomically; `display: None` leaves them untouched.

```rust
Op::DiagramSet {
    key: String,                        // diagram doc id (full-path) or bare slug
    title: Option<String>,              // None = leave unchanged
    description: Option<String>,        // None = leave unchanged
    display: Option<DiagramDisplaySet>, // None = leave display untouched
},

/// A fully-specified display block (the panel always holds the full resolved
/// display, so every non-nullable field is present; nullable fields use their
/// own absent state).
#[derive(Debug, Clone, PartialEq)]
pub struct DiagramDisplaySet {
    pub show_attributes: bool,
    pub attribute_detail: String,
    pub show_attribute_visibility: bool,
    pub show_attribute_multiplicity: bool,
    pub max_attributes: Option<u32>,          // None ⇒ omit key ⇒ unlimited
    pub association_labels: String,
    pub emphasize_multiplicity: bool,
    pub show_stereotype: bool,
    pub stereotype_filter: Option<Vec<String>>, // None ⇒ omit key ⇒ show all; Some([]) ⇒ [] ⇒ show none
    pub stereotype_colors: Vec<String>,         // "name:#rrggbb"; empty ⇒ omit key
}
```

### Handler `op_diagram_set`

```rust
const DISPLAY_KEYS: &[&str] = &[
    "showAttributes", "attributeDetail", "showAttributeVisibility",
    "showAttributeMultiplicity", "maxAttributes", "associationLabels",
    "emphasizeMultiplicity", "showStereotype", "stereotypeFilter",
    "stereotypeColors",
];
```

Registered in `apply_one`. Uses `edit_doc(work, key, "diagram.set", |doc| …)`
(which resolves the diagram doc by full-path id or bare slug like every other op):

1. If `title` is `Some(t)`: `fm_set(fm, "title", Str(t))` **and** `doc.title = t`
   (keep the H1 in sync, exactly like `op_node_set`).
2. If `description` is `Some(d)`: `fm_set(fm, "description", Str(d))`.
3. If `display` is `Some(ds)`:
   - Remove every `DISPLAY_KEYS` entry from `fm.entries` first (so a field
     returning to its absent state is actually dropped).
   - Write the non-nullable fields unconditionally: booleans via `FmValue::Bool`,
     `attribute_detail`/`association_labels` via `FmValue::Str`.
   - `max_attributes`: write `FmValue::Num(n as f64)` only when `Some`.
   - `stereotype_filter`: write `str_list(&v)` only when `Some` (empty `v` ⇒ `[]`).
   - `stereotype_colors`: write `str_list(&v)` only when non-empty.

The atomic-on-failure guarantee (`apply` aborts on any error, caller writes
nothing) is inherited unchanged. Applying `DiagramSet` to a key that resolves to
no doc yields the standard `OpError` ("no document '<key>'"); the TS store guards
against sending that (see below), so it is a defence-in-depth path, not the happy
path.

## DTO wire (`crates/waml-ops-dto/src/lib.rs`)

Add a `#[serde(rename = "diagram.set")]` variant, following the existing `v`/
`#[serde(default)]` conventions, plus a nested display DTO. `to_op` converts to
`Op::DiagramSet`; `from_op` is the inverse (kept for the round-trip test that
covers every op).

```rust
#[serde(rename = "diagram.set")]
DiagramSet {
    #[serde(default = "one")] v: u32,
    key: String,
    #[serde(default)] title: Option<String>,
    #[serde(default)] desc: Option<String>,
    #[serde(default)] display: Option<DisplayDto>,
},

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayDto {
    show_attributes: bool,
    attribute_detail: String,
    show_attribute_visibility: bool,
    show_attribute_multiplicity: bool,
    #[serde(default)] max_attributes: Option<u32>,
    association_labels: String,
    emphasize_multiplicity: bool,
    show_stereotype: bool,
    #[serde(default)] stereotype_filter: Option<Vec<String>>,
    #[serde(default)] stereotype_colors: Vec<String>,
}
```

`to_op` maps `DisplayDto` → `DiagramDisplaySet` field-for-field. The existing
`every_op_survives_a_wire_round_trip` test gains a `DiagramSet` case (both a
`display: None` title-only variant and a full-display variant).

## Wasm bridge

Regenerate `packages/wasm/src/generated/wasm-inline.ts` from the crate after the
Rust changes land (repo convention: a `chore(waml): regenerate wasm glue` commit).
No hand-editing of the generated file.

## TS wire + overlay (`packages/core/src/state/overlay.ts`)

`RustDiagram` gains `description?: string` and `display?: RustDiagramDisplay`,
where `RustDiagramDisplay` mirrors the Rust partial with `stereotypeColors` as a
`string[]`. `toModelGraph` maps them onto the derived `Diagram`, converting the
color list to a record:

```ts
export interface RustDiagramDisplay {
  showAttributes?: boolean;
  attributeDetail?: string;
  showAttributeVisibility?: boolean;
  showAttributeMultiplicity?: boolean;
  maxAttributes?: number;
  associationLabels?: string;
  emphasizeMultiplicity?: boolean;
  showStereotype?: boolean;
  stereotypeFilter?: string[];
  stereotypeColors?: string[]; // "name:#rrggbb"
}

// in toModelGraph, per diagram d:
display: d.display ? partialDisplayFromWire(d.display) : undefined,
description: d.description,
```

`partialDisplayFromWire` builds a `Partial<DiagramDisplay>`: copies scalars
verbatim and parses `stereotypeColors` (`"name:#hex"` split on first `:`) into a
`Record<string,string>`. The inverse (`Record` → `string[]`) lives in the ops
adapter. `Diagram.display?` in `types.ts` is typed as `Partial<DiagramDisplay>`
(it is the raw stored partial and is always fed through `resolveDisplay` before
use); `Diagram.description?: string` is added.

## Ops adapter (`packages/core/src/state/ops-adapter.ts`)

Add `updateDiagramOps(prev, patch)` mirroring `updateNodeOps`, plus the
`"diagram.set"` member on the `OpDto` union:

```ts
| { op: "diagram.set"; key: string; title?: string; desc?: string;
    display?: DisplayDto }
```

`updateDiagramOps(prev: Diagram, patch: Partial<Diagram>): OpDto[]`:

- `title` change (present & differs) ⇒ `set.title`.
- `description` change (present & differs) ⇒ `set.desc`.
- `display` present ⇒ `set.display = toDisplayDto(patch.display)`, where the
  input is the **full resolved** `DiagramDisplay` (the store passes
  `resolveDisplay(merged)` — see below), and `toDisplayDto` maps it field-for-field
  and serializes `stereotypeColors` (record) → `string[]` of `"name:#hex"`,
  passing `maxAttributes`/`stereotypeFilter` through as-is (`undefined` stays
  `undefined`, so a "show all" / "unlimited" choice omits the key server-side).
- Emit a single `{ op: "diagram.set", key: prev.key, ...set }` or `[]`.

`DisplayDto` is exported for the adapter's use.

## Store (`packages/core/src/state/model.ts`)

Replace the `updateDiagram` no-op with the `run()` treatment (identical shape to
`updateNode`):

```ts
updateDiagram(key: string, patch: Partial<Diagram>): void {
  const prev = graph().diagrams.find((d) => d.key === key);
  if (!prev) return; // implicit "All" or unknown key ⇒ no persistable doc
  run(updateDiagramOps(prev, patch));
},
```

`graph().diagrams` contains only **real** diagram docs (the implicit "All"
diagram is synthesized downstream by `effectiveDiagrams` and never appears here),
so the `!prev` guard naturally makes display/title edits on the implicit view a
silent no-op — there is no doc to write to. This is the one acknowledged
behavioural limitation (see "Known limitation"). `import { updateDiagramOps }`
is added to the ops-adapter import block.

## Web wiring (`packages/web/src/components/canvas/CanvasInner.svelte`) + retiring the session store

- **Delete** `packages/web/src/state/displaySettings.svelte.ts` and its test
  `displaySettings.svelte.test.ts`; drop the import at `CanvasInner.svelte` L21.
- `activeDisplay` (L153) switches from
  `displaySettings.resolve(activeDiagram.key, activeDiagram.display)` to
  `resolveDisplay(activeDiagram.display)` (`resolveDisplay` already imported, L59).
- `handleDisplayChange` (L331-333) switches from `displaySettings.patch(...)` to a
  real persist that merges the single-field panel patch onto the current resolved
  display and writes the **full** display through the store:

  ```ts
  function handleDisplayChange(p: Partial<DiagramDisplay>) {
    store.updateDiagram(activeDiagram.key, {
      display: resolveDisplay({ ...activeDiagram.display, ...p }),
    });
  }
  ```

  On the implicit "All" diagram this is a no-op via the store guard above.
- The TopBar diagram rename path (L508, `store.updateDiagram(key, { title })`)
  now actually persists — no code change beyond the store no longer being a no-op.

`DiagramPropertiesBody.svelte` and `CentralEditPanelHost.svelte` are **not
changed** in this spec beyond staying compilable against `DiagramDisplay`: the
new fields are additive and optional at the resolved level, and the panel keeps
rendering only the controls it already renders. The v2 spec adds the new
controls.

## Known limitation (flagged for product review)

The implicit "All" diagram (shown when a model has no authored `Diagram` docs —
the common case for fresh/imported models) has **no backing document**, so
display and title edits made while it is active cannot persist and become
no-ops. Previously `displaySettings` let those edits survive for the browser
session. Since this spec retires the session store and diagram *creation* is out
of scope, the "All" view loses in-session display tweaking. The v2 spec is
responsible for the UX of this state (e.g. disabling the controls with a hint).
If preserving "All"-view editing is required, wiring `store.addDiagram` to
materialize a real diagram doc is the follow-up — tracked separately, not in
either of these two specs.

## Testing

**Rust — `crates/waml/src/frontmatter.rs` / `parse.rs`**
- `build_diagrams` reads each new key: a fixture diagram doc with all keys set
  yields the expected `description` + `DiagramDisplay` partial.
- Presence distinction: `stereotypeFilter: []` ⇒ `Some(vec![])`; absent ⇒ `None`.
- `maxAttributes: 6` ⇒ `Some(6)`; `maxAttributes: 0` and absent ⇒ `None`.
- Legacy diagram doc (no new keys) ⇒ `description: None`, `display.is_empty()`,
  and `serde` omits both from the wire.

**Rust — `crates/waml/src/ops/mod.rs`**
- `diagram_set_writes_title_and_note`: title change updates fm `title` + H1;
  `description` written as a scalar.
- `diagram_set_replaces_display_block`: setting `display` writes every non-nullable
  key; a follow-up `DiagramSet` with `stereotype_filter: None` / `max_attributes:
  None` drops those keys (block-replace removes stale ones).
- `diagram_set_show_none_vs_show_all`: `Some(vec![])` writes `stereotypeFilter: []`;
  a later block with `None` removes the key entirely.
- `diagram_set_leaves_display_untouched_when_none`: `display: None` + `title:
  Some` changes only the title, display keys unchanged.
- `diagram_set_resolves_nested_doc_by_full_path_id` (mirror the NodeSet test).
- Serialize fixpoint: a diagram doc carrying all new keys is a
  `serialize_document ∘ parse_document` fixpoint (extend `serialize.rs` tests).

**Rust — `crates/waml-ops-dto/src/lib.rs`**
- `every_op_survives_a_wire_round_trip` gains a title-only `DiagramSet` and a
  full-display `DiagramSet`.

**TS — `packages/okf`**
- `resolveDisplay` fills the three new non-nullable defaults; nullable fields stay
  `undefined`; `stereotypeColors` defaults to `{}`.
- `overlay.ts`: `partialDisplayFromWire` parses `stereotypeColors` list → record
  and copies scalars; a wire diagram with no `display` ⇒ `Diagram.display`
  undefined.

**TS — `packages/core`**
- `updateDiagramOps`: title-only patch ⇒ one `diagram.set` with just `title`;
  display patch ⇒ `diagram.set` with a full `display` DTO whose
  `stereotypeColors` is a `"name:#hex"` list and whose `stereotypeFilter`/
  `maxAttributes` pass `undefined` through as omitted; no-change patch ⇒ `[]`.
- Store integration: `store.updateDiagram(realKey, {display})` mutates the bundle
  and re-derives with the new display; `store.updateDiagram(ALL_DIAGRAM_KEY, …)`
  is a no-op that leaves the bundle unchanged.

**Web — `packages/web`**
- `CanvasInner` (or a focused unit): `handleDisplayChange` on a real diagram
  persists (survives a re-derive); the old `displaySettings` module and test are
  gone and nothing imports them.
