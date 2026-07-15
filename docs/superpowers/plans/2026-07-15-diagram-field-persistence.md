# Diagram Field Persistence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give diagram `title`, a new `description` note, and the full `DiagramDisplay` field set a real persistence path — Rust `Diagram` struct → flat frontmatter round-trip → `Op::DiagramSet` → DTO → wasm → ops adapter → `store.updateDiagram` — and retire the in-memory `displaySettings.svelte.ts` session store.

**Architecture:** Diagram docs are ordinary markdown with **flat** frontmatter (`FmValue = Str | Bool | Num | List`, no nested maps). Every new diagram field is a flat scalar/bool/num/list key. Rust round-trips exactly the keys the file holds (never invents defaults); TS `resolveDisplay(partial)` fills the rest from `DEFAULT_DISPLAY`, the single source of defaults. Mutations flow through the existing op log (`apply` → `edit_doc` → `parse_document`/`serialize_document`); a new `Op::DiagramSet` mirrors `Op::NodeSet` for scalars and treats the display block as a whole-block replace (so tri-state fields can clear back to absent). The generic `serialize_document`/`render_frontmatter` path already emits every frontmatter key, so there is no diagram serializer to extend.

**Tech Stack:** Rust (`crates/waml`, `crates/waml-ops-dto`, `crates/waml-wasm`), `cargo test`; wasm-bindgen glue regenerated via `pnpm build:wasm`; TypeScript pnpm workspaces (`@waml/okf`, `@waml/core`, `@waml/wasm`, `@waml/web`), Vitest; Svelte 5 runes.

## Global Constraints

- **Package/crate names are `@waml/*` and `crates/waml`.** The spec text says `@uaml/*` / `packages/okf` in places — that is pre-rename staleness. Use `@waml/okf`, `@waml/core`, `@waml/wasm`, `@waml/web` and `crates/waml`, `crates/waml-ops-dto`, `crates/waml-wasm` throughout. (The okf package **directory** is still `packages/okf`, but its package **name** is `@waml/okf`.)
- **Frontmatter is flat.** Every new diagram field is a flat key. `stereotypeColors` is a `List<Str>` of `"name:#rrggbb"` pairs (split on the FIRST `:`); Rust treats it as an opaque `Vec<String>` passthrough and never parses the hex.
- **Frontmatter keys are camelCase**, identical to the TS field names and to the Rust serde `rename_all = "camelCase"` wire keys (one frontmatter key per field).
- **Rust never invents display defaults.** A diagram with none of the new keys yields `description: None` + an empty `DiagramDisplay`, both omitted from the wire via `skip_serializing_if` — legacy diagram files stay byte-identical on read.
- **Tri-state preservation end to end:** `maxAttributes` (`undefined` = unlimited vs a number) and `stereotypeFilter` (`undefined` = show all vs `[]` = show none vs names) each carry a meaningful *absent* state. Presence of the frontmatter key is what distinguishes absent from empty.
- **No new panel UI, no rendering behaviour change** (the separate v2 spec owns that). No diagram create/delete/membership ops (`addDiagram`/`removeDiagram` stay stubs).
- **Generated files are never hand-edited.** `packages/wasm/src/generated/*` is produced only by `pnpm build:wasm`.
- **Known accepted limitation (do NOT fix):** the synthesized implicit "All" diagram (`ALL_DIAGRAM_KEY = "__all__"`, shown when a model has no authored `Diagram` docs) has no backing document. `store.updateDiagram` no-ops for it (the `!prev` guard). This is documented, out-of-scope behaviour; the v2 spec owns the disabled-panel UX.

---

## File Structure

**Rust (`crates/waml`)**
- `src/model.rs` — add `DiagramDisplay` struct; add `description` + `display` fields to `Diagram`.
- `src/parse.rs` — `build_diagrams` reads the new frontmatter keys.
- `src/ops/mod.rs` — `Op::DiagramSet` + `DiagramDisplaySet` + `op_diagram_set` handler + `apply_one` registration.
- `src/serialize.rs` — round-trip fixpoint test for a diagram doc carrying the new keys.
- `src/solve/resolve.rs` — two existing test-fixture `Diagram { .. }` literals must gain the new fields to keep compiling.

**Rust (`crates/waml-ops-dto`)**
- `src/lib.rs` — `OpDto::DiagramSet` variant + `DisplayDto` struct + `to_op`/`from_op` + round-trip test cases.

**Wasm**
- `packages/wasm/src/generated/*` — regenerated (`pnpm build:wasm`), committed as a `chore(waml)`.

**TypeScript**
- `packages/okf/src/types.ts` — extend `DiagramDisplay`, `DEFAULT_DISPLAY`; add `Diagram.description`; retype `Diagram.display` as `Partial<DiagramDisplay>`.
- `packages/okf/test/display.test.ts` — extend defaulting tests (existing "documented default values" test must be updated).
- `packages/core/src/state/overlay.ts` — `RustDiagram` gains `description`/`display`; add `RustDiagramDisplay` + `partialDisplayFromWire`; `toModelGraph` maps them.
- `packages/core/src/state/overlay.test.ts` — cover the new mapping.
- `packages/core/src/state/ops-adapter.ts` — `OpDto` union member; `DisplayDto`; `toDisplayDto`; `updateDiagramOps`.
- `packages/core/src/state/ops-adapter.test.ts` — cover `updateDiagramOps`.
- `packages/core/src/state/model.ts` — replace the `updateDiagram` no-op with `run(updateDiagramOps(...))`.
- `packages/core/src/state/model.test.ts` — store integration (real key persists; `__all__` no-ops).

**Web**
- `packages/web/src/components/canvas/CanvasInner.svelte` — repoint `activeDisplay`/`handleDisplayChange` at the store; drop the `displaySettings` import.
- `packages/web/src/state/displaySettings.svelte.ts` + `displaySettings.svelte.test.ts` — **deleted**.

---

## Task 1: Rust `Diagram` gains `description` + `DiagramDisplay` (model.rs)

**Files:**
- Modify: `crates/waml/src/model.rs:389-399` (the `Diagram` struct)
- Modify: `crates/waml/src/parse.rs:732` (the `Diagram { .. }` construction in `build_diagrams`)
- Modify: `crates/waml/src/solve/resolve.rs:270` and `:428` (two test-fixture `Diagram { .. }` literals)

**Interfaces:**
- Produces: `pub struct DiagramDisplay` (partial; all fields `Option`/`Vec`) with `pub fn is_empty(&self) -> bool`, and `Diagram.description: Option<String>` + `Diagram.display: DiagramDisplay`. Consumed by Tasks 2 (parse) and later serde on the wasm wire.

- [ ] **Step 1: Write the failing unit test**

Add to the existing `#[cfg(test)] mod tests` in `crates/waml/src/model.rs` (create the module if absent — mirror the `mod tests` in `frontmatter.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_display_default_is_empty() {
        let d = DiagramDisplay::default();
        assert!(d.is_empty(), "an all-None/empty display must report empty");
    }

    #[test]
    fn diagram_display_with_a_set_field_is_not_empty() {
        let d = DiagramDisplay { show_attributes: Some(false), ..Default::default() };
        assert!(!d.is_empty(), "any set field makes the display non-empty");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml diagram_display`
Expected: FAIL — `DiagramDisplay` does not exist / does not compile.

- [ ] **Step 3: Add the struct and fields**

In `crates/waml/src/model.rs`, replace the `Diagram` struct (currently `key/title/profile/groups/layout`) with:

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
    // `layout` carries the raw layout AST (`syntax::LayoutStatement`). Serialized
    // end to end so the frontend can read the layout relations.
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
    pub attribute_detail: Option<String>, // "name-only" | "name-type"
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attribute_visibility: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub show_attribute_multiplicity: Option<bool>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub max_attributes: Option<u32>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub association_labels: Option<String>, // "all" | "hidden"
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
    pub fn is_empty(&self) -> bool {
        *self == DiagramDisplay::default()
    }
}
```

- [ ] **Step 4: Update the three `Diagram { .. }` construction sites to compile**

In `crates/waml/src/parse.rs:732`, change:

```rust
        out.push(Diagram { key: p.id.clone(), title, profile, groups, layout });
```

to (new fields defaulted for now — Task 2 fills them from frontmatter):

```rust
        out.push(Diagram {
            key: p.id.clone(),
            title,
            profile,
            description: None,
            groups,
            layout,
            display: DiagramDisplay::default(),
        });
```

Ensure `DiagramDisplay` is in scope in `parse.rs` (it is imported via `use crate::model::...`; add `DiagramDisplay` to that import list if the wildcard is not used).

In `crates/waml/src/solve/resolve.rs:270`, change:

```rust
        Diagram { key: "orders".into(), title: "Orders".into(), profile: "uml-domain".into(), groups, layout }
```

to:

```rust
        Diagram { key: "orders".into(), title: "Orders".into(), profile: "uml-domain".into(),
            description: None, groups, layout, display: crate::model::DiagramDisplay::default() }
```

In `crates/waml/src/solve/resolve.rs:428`, the `let d = Diagram { key: "tables/dia".into(), title: "D".into(), profile: "uml-domain".into(), groups: vec![...] ... }` literal — add `description: None,` after `profile`, and `display: crate::model::DiagramDisplay::default(),` before the closing `}` (keep the existing `groups`/`layout` fields; add `layout: vec![],` if that literal did not already set it).

- [ ] **Step 5: Run the test + full crate to verify green**

Run: `cargo test -p waml`
Expected: PASS — `diagram_display_*` pass; all pre-existing `waml` tests still pass (build_diagrams reads nothing new yet, so legacy behaviour is unchanged).

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/model.rs crates/waml/src/parse.rs crates/waml/src/solve/resolve.rs
git commit -m "feat(waml): add description + DiagramDisplay to the Diagram model"
```

---

## Task 2: `build_diagrams` reads the new frontmatter keys (parse.rs)

**Files:**
- Modify: `crates/waml/src/parse.rs:708-735` (`build_diagrams`)
- Test: `crates/waml/src/parse.rs` (`#[cfg(test)] mod tests`, add cases)

**Interfaces:**
- Consumes: `Frontmatter::{get_str, get_bool, get_string_list, get}` and `FmValue::Num` (from `crate::frontmatter`), `DiagramDisplay` (Task 1).
- Produces: a populated `Diagram.description` + `Diagram.display` on every parsed diagram doc.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `crates/waml/src/parse.rs`:

```rust
    fn diagram_bundle(fm_body: &str) -> Vec<(String, String)> {
        vec![("d.md".to_string(), format!("---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n{fm_body}---\n# D\n"))]
    }

    #[test]
    fn build_diagrams_reads_all_display_keys() {
        let b = diagram_bundle(
            "description: \"Notes\"\nshowAttributes: false\nattributeDetail: name-only\n\
             showAttributeVisibility: false\nshowAttributeMultiplicity: false\nmaxAttributes: 6\n\
             associationLabels: hidden\nemphasizeMultiplicity: true\nshowStereotype: false\n\
             stereotypeFilter: [entity, valueObject]\nstereotypeColors: [\"entity:#ffedd5\"]\n",
        );
        let m = build_model(&b);
        let d = &m.diagrams[0];
        assert_eq!(d.description.as_deref(), Some("Notes"));
        let x = &d.display;
        assert_eq!(x.show_attributes, Some(false));
        assert_eq!(x.attribute_detail.as_deref(), Some("name-only"));
        assert_eq!(x.show_attribute_visibility, Some(false));
        assert_eq!(x.show_attribute_multiplicity, Some(false));
        assert_eq!(x.max_attributes, Some(6));
        assert_eq!(x.association_labels.as_deref(), Some("hidden"));
        assert_eq!(x.emphasize_multiplicity, Some(true));
        assert_eq!(x.show_stereotype, Some(false));
        assert_eq!(x.stereotype_filter, Some(vec!["entity".to_string(), "valueObject".to_string()]));
        assert_eq!(x.stereotype_colors, vec!["entity:#ffedd5".to_string()]);
    }

    #[test]
    fn build_diagrams_distinguishes_absent_vs_empty_stereotype_filter() {
        let present = build_model(&diagram_bundle("stereotypeFilter: []\n"));
        assert_eq!(present.diagrams[0].display.stereotype_filter, Some(vec![]));
        let absent = build_model(&diagram_bundle(""));
        assert_eq!(absent.diagrams[0].display.stereotype_filter, None);
    }

    #[test]
    fn build_diagrams_maps_max_attributes_floor() {
        assert_eq!(build_model(&diagram_bundle("maxAttributes: 6\n")).diagrams[0].display.max_attributes, Some(6));
        assert_eq!(build_model(&diagram_bundle("maxAttributes: 0\n")).diagrams[0].display.max_attributes, None);
        assert_eq!(build_model(&diagram_bundle("")).diagrams[0].display.max_attributes, None);
    }

    #[test]
    fn build_diagrams_legacy_doc_has_no_description_and_empty_display() {
        let m = build_model(&diagram_bundle(""));
        assert_eq!(m.diagrams[0].description, None);
        assert!(m.diagrams[0].display.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml build_diagrams_`
Expected: FAIL — `description`/`display` are still defaulted (e.g. `show_attributes` is `None`, `is_empty()` is true even with keys set).

- [ ] **Step 3: Implement the reader**

In `crates/waml/src/parse.rs` `build_diagrams`, after `layout` is computed and before `out.push(Diagram { .. })`, build the display + description from `fm`:

```rust
        use crate::frontmatter::FmValue;
        let description = fm.get_str("description").map(String::from);
        let max_attributes = match fm.get("maxAttributes") {
            Some(FmValue::Num(n)) if *n >= 1.0 => Some(*n as u32),
            _ => None,
        };
        let stereotype_filter = if fm.get("stereotypeFilter").is_some() {
            Some(fm.get_string_list("stereotypeFilter"))
        } else {
            None
        };
        let display = crate::model::DiagramDisplay {
            show_attributes: fm.get_bool("showAttributes"),
            attribute_detail: fm.get_str("attributeDetail").map(String::from),
            show_attribute_visibility: fm.get_bool("showAttributeVisibility"),
            show_attribute_multiplicity: fm.get_bool("showAttributeMultiplicity"),
            max_attributes,
            association_labels: fm.get_str("associationLabels").map(String::from),
            emphasize_multiplicity: fm.get_bool("emphasizeMultiplicity"),
            show_stereotype: fm.get_bool("showStereotype"),
            stereotype_filter,
            stereotype_colors: fm.get_string_list("stereotypeColors"),
        };
```

Then change the push to carry them:

```rust
        out.push(Diagram { key: p.id.clone(), title, profile, description, groups, layout, display });
```

(Remove the `description: None` / `display: DiagramDisplay::default()` placeholders from Task 1.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml build_diagrams_`
Expected: PASS (4 new tests). Then `cargo test -p waml` — all green.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/parse.rs
git commit -m "feat(waml): read diagram display + description from flat frontmatter"
```

---

## Task 3: Serialize round-trip fixpoint for diagram display keys (serialize.rs)

**Files:**
- Test: `crates/waml/src/serialize.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `parse::parse_document`, `serialize::serialize_document` (already used by the existing fixpoint tests in this module).

**Context:** There is no dedicated diagram serializer — diagram docs round-trip through the generic `serialize_document` → `render_frontmatter` path, which emits every `fm.entries` key. This test locks in that a diagram doc carrying all the new keys survives `serialize_document(parse_document(text))` unchanged (the semantic-fixpoint contract the module already tests for classifiers).

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/waml/src/serialize.rs` (mirror `serialize_is_a_semantic_fixpoint`, which parses once, serializes, re-parses, and asserts the two `Document`s are equal):

```rust
    #[test]
    fn serialize_is_a_fixpoint_for_diagram_display_frontmatter() {
        let text = "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\ndescription: \"Notes\"\n\
                    showAttributes: false\nattributeDetail: name-only\nmaxAttributes: 6\n\
                    stereotypeFilter: [entity, valueObject]\nstereotypeColors: [\"entity:#ffedd5\"]\n\
                    ---\n# D\n";
        let once = serialize_document(&parse_document(text));
        let twice = serialize_document(&parse_document(&once));
        assert_eq!(once, twice, "diagram display frontmatter must be a serialize fixpoint");
    }
```

- [ ] **Step 2: Run test to verify it fails-or-passes honestly**

Run: `cargo test -p waml serialize_is_a_fixpoint_for_diagram_display`
Expected: This documents the generic path. If it passes immediately (no serializer change needed), that is the intended confirmation — keep it as a regression guard. If it FAILS, the generic path is dropping a key; stop and investigate before proceeding (do not add diagram-specific serializer logic without confirming the generic path is the culprit).

- [ ] **Step 3: Implementation**

No implementation expected — the generic `serialize_document`/`render_frontmatter` path already emits every frontmatter key. This task only adds the regression guard.

- [ ] **Step 4: Run the crate to verify green**

Run: `cargo test -p waml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/serialize.rs
git commit -m "test(waml): fixpoint guard for diagram display frontmatter round-trip"
```

---

## Task 4: `Op::DiagramSet` op + handler (ops/mod.rs)

**Files:**
- Modify: `crates/waml/src/ops/mod.rs` (add variant, struct, handler, registration, `DISPLAY_KEYS`)
- Test: `crates/waml/src/ops/mod.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `edit_doc`, `fm_set`, `str_list`, `FmValue` (all already in this module).
- Produces: `pub struct DiagramDisplaySet` (a fully-specified display block — non-nullable fields present, nullable fields via their own absent state) and `Op::DiagramSet { key, title, description, display }`. `DiagramDisplaySet` is consumed by `crates/waml-ops-dto` (Task 5).

`DiagramDisplaySet` shape:

```rust
/// A fully-specified display block. The panel always holds the full resolved
/// display, so every non-nullable field is present; nullable fields use their
/// own absent state (`None` ⇒ omit the key).
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
    pub stereotype_filter: Option<Vec<String>>, // None ⇒ omit ⇒ show all; Some([]) ⇒ [] ⇒ show none
    pub stereotype_colors: Vec<String>,         // "name:#rrggbb"; empty ⇒ omit key
}
```

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` in `crates/waml/src/ops/mod.rs`:

```rust
    fn diagram_doc() -> Bundle {
        vec![("shop/dia.md".to_string(),
            "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n".to_string())]
    }
    fn full_display() -> DiagramDisplaySet {
        DiagramDisplaySet {
            show_attributes: false,
            attribute_detail: "name-only".into(),
            show_attribute_visibility: false,
            show_attribute_multiplicity: false,
            max_attributes: Some(6),
            association_labels: "hidden".into(),
            emphasize_multiplicity: true,
            show_stereotype: false,
            stereotype_filter: Some(vec!["entity".into()]),
            stereotype_colors: vec!["entity:#ffedd5".into()],
        }
    }

    #[test]
    fn diagram_set_writes_title_and_note() {
        let out = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: Some("Order lifecycle".into()),
            description: Some("Notes for reviewers".into()), display: None,
        }]).unwrap();
        assert!(out[0].1.contains("title: \"Order lifecycle\""));
        assert!(out[0].1.contains("# Order lifecycle"), "H1 kept in sync");
        assert!(out[0].1.contains("description: \"Notes for reviewers\""));
    }

    #[test]
    fn diagram_set_replaces_display_block_and_drops_stale_keys() {
        let set = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None, display: Some(full_display()),
        }]).unwrap();
        assert!(set[0].1.contains("showAttributes: false"));
        assert!(set[0].1.contains("maxAttributes: 6"));
        assert!(set[0].1.contains("stereotypeFilter: [\"entity\"]"));
        assert!(set[0].1.contains("stereotypeColors: [\"entity:#ffedd5\"]"));
        // a follow-up block with max_attributes: None / stereotype_filter: None drops them
        let cleared = apply(&set, &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None,
            display: Some(DiagramDisplaySet { max_attributes: None, stereotype_filter: None,
                stereotype_colors: vec![], ..full_display() }),
        }]).unwrap();
        assert!(!cleared[0].1.contains("maxAttributes"), "stale maxAttributes must be removed");
        assert!(!cleared[0].1.contains("stereotypeFilter"), "stale stereotypeFilter must be removed");
        assert!(!cleared[0].1.contains("stereotypeColors"), "stale stereotypeColors must be removed");
    }

    #[test]
    fn diagram_set_show_none_vs_show_all() {
        let none = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None,
            display: Some(DiagramDisplaySet { stereotype_filter: Some(vec![]), ..full_display() }),
        }]).unwrap();
        assert!(none[0].1.contains("stereotypeFilter: []"), "empty allowlist ⇒ show none");
        let all = apply(&none, &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None,
            display: Some(DiagramDisplaySet { stereotype_filter: None, ..full_display() }),
        }]).unwrap();
        assert!(!all[0].1.contains("stereotypeFilter"), "None ⇒ key removed ⇒ show all");
    }

    #[test]
    fn diagram_set_leaves_display_untouched_when_none() {
        let seeded = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "dia".into(), title: None, description: None, display: Some(full_display()),
        }]).unwrap();
        let retitled = apply(&seeded, &[Op::DiagramSet {
            key: "dia".into(), title: Some("Renamed".into()), description: None, display: None,
        }]).unwrap();
        assert!(retitled[0].1.contains("title: \"Renamed\""));
        assert!(retitled[0].1.contains("showAttributes: false"), "display keys untouched when display: None");
    }

    #[test]
    fn diagram_set_resolves_nested_doc_by_full_path_id() {
        let out = apply(&diagram_doc(), &[Op::DiagramSet {
            key: "shop/dia".into(), title: Some("D2".into()), description: None, display: None,
        }]).unwrap();
        assert_eq!(out[0].0, "shop/dia.md");
        assert!(out[0].1.contains("title: \"D2\""));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml diagram_set_`
Expected: FAIL — `Op::DiagramSet` / `DiagramDisplaySet` do not exist.

- [ ] **Step 3: Add the variant, struct, registration, and handler**

In `crates/waml/src/ops/mod.rs`, add the `DiagramDisplaySet` struct (shown in the Interfaces block above) near the other op types (e.g. after `NameSpec`).

Add the variant to `enum Op` (after `NodeRename`, alongside the other doc-editing ops):

```rust
    DiagramSet {
        key: String,                        // diagram doc id (full-path) or bare slug
        title: Option<String>,              // None = leave unchanged
        description: Option<String>,        // None = leave unchanged
        display: Option<DiagramDisplaySet>, // None = leave display untouched
    },
```

Register it in `apply_one`'s match (alongside `Op::NodeSet`):

```rust
        Op::DiagramSet { key, title, description, display } => {
            op_diagram_set(work, key, title, description, display)
        }
```

Add the display-keys constant and handler near `op_node_set`:

```rust
const DISPLAY_KEYS: &[&str] = &[
    "showAttributes", "attributeDetail", "showAttributeVisibility",
    "showAttributeMultiplicity", "maxAttributes", "associationLabels",
    "emphasizeMultiplicity", "showStereotype", "stereotypeFilter",
    "stereotypeColors",
];

fn op_diagram_set(
    work: &mut Bundle,
    key: &str,
    title: &Option<String>,
    description: &Option<String>,
    display: &Option<DiagramDisplaySet>,
) -> Result<(), OpError> {
    edit_doc(work, key, "diagram.set", |doc| {
        if let Some(t) = title {
            fm_set(&mut doc.frontmatter, "title", FmValue::Str(t.clone()));
            doc.title = t.clone();
        }
        if let Some(d) = description {
            fm_set(&mut doc.frontmatter, "description", FmValue::Str(d.clone()));
        }
        if let Some(ds) = display {
            let fm = &mut doc.frontmatter;
            fm.entries.retain(|(k, _)| !DISPLAY_KEYS.contains(&k.as_str()));
            fm_set(fm, "showAttributes", FmValue::Bool(ds.show_attributes));
            fm_set(fm, "attributeDetail", FmValue::Str(ds.attribute_detail.clone()));
            fm_set(fm, "showAttributeVisibility", FmValue::Bool(ds.show_attribute_visibility));
            fm_set(fm, "showAttributeMultiplicity", FmValue::Bool(ds.show_attribute_multiplicity));
            if let Some(n) = ds.max_attributes {
                fm_set(fm, "maxAttributes", FmValue::Num(n as f64));
            }
            fm_set(fm, "associationLabels", FmValue::Str(ds.association_labels.clone()));
            fm_set(fm, "emphasizeMultiplicity", FmValue::Bool(ds.emphasize_multiplicity));
            fm_set(fm, "showStereotype", FmValue::Bool(ds.show_stereotype));
            if let Some(filter) = &ds.stereotype_filter {
                fm_set(fm, "stereotypeFilter", str_list(filter));
            }
            if !ds.stereotype_colors.is_empty() {
                fm_set(fm, "stereotypeColors", str_list(&ds.stereotype_colors));
            }
        }
        Ok(())
    })
}
```

If `DiagramDisplaySet` needs to be reachable from the DTO crate (Task 5), export it: add `DiagramDisplaySet` to the module's public surface (it is `pub struct`, and `crate::ops::DiagramDisplaySet` is already reachable since the `ops` module is public; confirm `pub use` is not required — the enum variant already references it publicly).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml diagram_set_`
Expected: PASS (5 tests). Then `cargo test -p waml` — all green.

- [ ] **Step 5: Commit**

```bash
git add crates/waml/src/ops/mod.rs
git commit -m "feat(waml): Op::DiagramSet writes diagram title/note/display frontmatter"
```

---

## Task 5: DTO wire for `diagram.set` (waml-ops-dto)

**Files:**
- Modify: `crates/waml-ops-dto/src/lib.rs` (variant, `DisplayDto`, `to_op`, `from_op`, round-trip test)

**Interfaces:**
- Consumes: `waml::ops::{DiagramDisplaySet, Op}` (Task 4).
- Produces: `OpDto::DiagramSet` (wire tag `"diagram.set"`) + `pub struct DisplayDto` (serde `rename_all="camelCase"`), mirrored on the TS side in Task 9.

- [ ] **Step 1: Write the failing test cases**

In `crates/waml-ops-dto/src/lib.rs`, in the `every_op_survives_a_wire_round_trip` test, add two `Op::DiagramSet` cases to the `ops` vec (import `DiagramDisplaySet` in the test's `use waml::ops::{...}`):

```rust
            Op::DiagramSet {
                key: "dia".into(),
                title: Some("D".into()),
                description: None,
                display: None,
            },
            Op::DiagramSet {
                key: "dia".into(),
                title: None,
                description: Some("Notes".into()),
                display: Some(DiagramDisplaySet {
                    show_attributes: false,
                    attribute_detail: "name-only".into(),
                    show_attribute_visibility: false,
                    show_attribute_multiplicity: true,
                    max_attributes: Some(6),
                    association_labels: "hidden".into(),
                    emphasize_multiplicity: true,
                    show_stereotype: false,
                    stereotype_filter: Some(vec!["entity".into()]),
                    stereotype_colors: vec!["entity:#ffedd5".into()],
                }),
            },
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-ops-dto every_op_survives`
Expected: FAIL — `OpDto::DiagramSet` / `DisplayDto` do not exist; `from_op` is non-exhaustive.

- [ ] **Step 3: Implement the DTO**

Add the `DisplayDto` struct (after `OpDto`, near the helper fns):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayDto {
    pub show_attributes: bool,
    pub attribute_detail: String,
    pub show_attribute_visibility: bool,
    pub show_attribute_multiplicity: bool,
    #[serde(default)]
    pub max_attributes: Option<u32>,
    pub association_labels: String,
    pub emphasize_multiplicity: bool,
    pub show_stereotype: bool,
    #[serde(default)]
    pub stereotype_filter: Option<Vec<String>>,
    #[serde(default)]
    pub stereotype_colors: Vec<String>,
}
```

Add the variant to `enum OpDto`:

```rust
    #[serde(rename = "diagram.set")]
    DiagramSet {
        #[serde(default = "one")]
        v: u32,
        key: String,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        desc: Option<String>,
        #[serde(default)]
        display: Option<DisplayDto>,
    },
```

Add the import at the top: `use waml::ops::{DiagramDisplaySet, NameSpec, Op, RelBy, Selector};` (extend the existing `use waml::ops::{...}` line).

Add to `to_op`'s match:

```rust
            OpDto::DiagramSet { v, key, title, desc, display } => {
                check_v(*v, "diagram.set")?;
                Ok(Op::DiagramSet {
                    key: key.clone(),
                    title: title.clone(),
                    description: desc.clone(),
                    display: display.as_ref().map(display_dto_to_set),
                })
            }
```

Add to `from_op`'s match:

```rust
            Op::DiagramSet { key, title, description, display } => OpDto::DiagramSet {
                v: 1,
                key: key.clone(),
                title: title.clone(),
                desc: description.clone(),
                display: display.as_ref().map(display_set_to_dto),
            },
```

Add the two field-for-field converters (near `sel_parts`):

```rust
fn display_dto_to_set(d: &DisplayDto) -> DiagramDisplaySet {
    DiagramDisplaySet {
        show_attributes: d.show_attributes,
        attribute_detail: d.attribute_detail.clone(),
        show_attribute_visibility: d.show_attribute_visibility,
        show_attribute_multiplicity: d.show_attribute_multiplicity,
        max_attributes: d.max_attributes,
        association_labels: d.association_labels.clone(),
        emphasize_multiplicity: d.emphasize_multiplicity,
        show_stereotype: d.show_stereotype,
        stereotype_filter: d.stereotype_filter.clone(),
        stereotype_colors: d.stereotype_colors.clone(),
    }
}

#[allow(dead_code)]
fn display_set_to_dto(d: &DiagramDisplaySet) -> DisplayDto {
    DisplayDto {
        show_attributes: d.show_attributes,
        attribute_detail: d.attribute_detail.clone(),
        show_attribute_visibility: d.show_attribute_visibility,
        show_attribute_multiplicity: d.show_attribute_multiplicity,
        max_attributes: d.max_attributes,
        association_labels: d.association_labels.clone(),
        emphasize_multiplicity: d.emphasize_multiplicity,
        show_stereotype: d.show_stereotype,
        stereotype_filter: d.stereotype_filter.clone(),
        stereotype_colors: d.stereotype_colors.clone(),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-ops-dto`
Expected: PASS. Then `cargo test --workspace` — all green.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-ops-dto/src/lib.rs
git commit -m "feat(waml-ops-dto): diagram.set wire variant with DisplayDto"
```

---

## Task 6: Regenerate the wasm glue

**Files:**
- Modify (generated): `packages/wasm/src/generated/waml_wasm.js`, `waml_wasm.d.ts`, `wasm-inline.ts`

**Context:** `Op::DiagramSet` / `DisplayDto` change the DTO surface, and the `Diagram` model change alters the serialized wire shape. `packages/wasm/src/generated/*` is a base64-inlined build of `crates/waml-wasm`, produced only by `pnpm build:wasm` (`node scripts/build-wasm.mjs`, which shells out to `wasm-pack`). **Assumption/prereq:** `wasm-pack` must be installed in the execution environment. If it is not available, this task blocks — install `wasm-pack` (or run in the CI image that has it) rather than hand-editing generated files.

- [ ] **Step 1: Regenerate**

Run: `pnpm build:wasm`
Expected: `wrote .../wasm-inline.ts (NNN KB base64)`, no errors.

- [ ] **Step 2: Verify only generated files changed**

Run: `git status --short packages/wasm/src/generated`
Expected: only files under `packages/wasm/src/generated/` are modified (or none, if the wire bytes happen to be identical — unlikely given the DTO change, but acceptable).

- [ ] **Step 3: Sanity-check the workspace still builds against the new glue**

Run: `pnpm --filter @waml/wasm build`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add packages/wasm/src/generated
git commit -m "chore(waml): regenerate wasm glue for diagram.set"
```

---

## Task 7: TS `DiagramDisplay` + `Diagram` types (@waml/okf)

**Files:**
- Modify: `packages/okf/src/types.ts:86-126` (`DiagramDisplay`, `DEFAULT_DISPLAY`, `Diagram`)
- Test: `packages/okf/test/display.test.ts`

**Interfaces:**
- Produces: extended `DiagramDisplay` (5 new fields), extended `DEFAULT_DISPLAY` (3 new non-nullable defaults), `Diagram.description?: string`, `Diagram.display?: Partial<DiagramDisplay>`. `resolveDisplay` is unchanged (`{ ...DEFAULT_DISPLAY, ...display }` covers the new keys automatically). Consumed by Tasks 8, 9, 11.

- [ ] **Step 1: Update the failing test**

In `packages/okf/test/display.test.ts`, the existing `"returns the documented default values"` test will break once the 3 new defaults are added. Update it and add coverage for the nullable fields:

```ts
  it("returns the documented default values", () => {
    expect(DEFAULT_DISPLAY).toEqual({
      showAttributes: true,
      attributeDetail: "name-type",
      showAttributeVisibility: true,
      showAttributeMultiplicity: true,
      associationLabels: "all",
      emphasizeMultiplicity: false,
      showStereotype: true,
      stereotypeColors: {},
    });
  });

  it("leaves nullable fields (maxAttributes, stereotypeFilter) undefined by default", () => {
    const r = resolveDisplay(undefined);
    expect(r.maxAttributes).toBeUndefined();
    expect(r.stereotypeFilter).toBeUndefined();
  });

  it("overlays new fields, keeping stereotypeColors a record", () => {
    const r = resolveDisplay({ maxAttributes: 6, stereotypeFilter: ["entity"], stereotypeColors: { entity: "#fff" } });
    expect(r.maxAttributes).toBe(6);
    expect(r.stereotypeFilter).toEqual(["entity"]);
    expect(r.stereotypeColors).toEqual({ entity: "#fff" });
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/okf test display`
Expected: FAIL — `DEFAULT_DISPLAY` lacks the new keys.

- [ ] **Step 3: Extend the types**

In `packages/okf/src/types.ts`, replace the `DiagramDisplay` interface body with (add the 5 new fields):

```ts
export interface DiagramDisplay {
  /** Show attribute rows inside class boxes (vs. a collapsed attribute count). */
  showAttributes: boolean;
  /** How much of each attribute row shows: just the name, or name + type. */
  attributeDetail: "name-only" | "name-type";
  /** Diagram-level gate on the +/-/#/~ visibility marker per attribute row. */
  showAttributeVisibility: boolean;
  /** Independent gate on the {mult} suffix per attribute row. */
  showAttributeMultiplicity: boolean;
  /** Cap on attribute rows drawn per box; excess folded as "+K more". Absent ⇒ unlimited. */
  maxAttributes?: number;
  /** Whether association edges carry their multiplicity/role labels. */
  associationLabels: "all" | "hidden";
  /** Visually emphasize multiplicity on association labels. */
  emphasizeMultiplicity: boolean;
  /** Show the «stereotype» / keyword row on class boxes. */
  showStereotype: boolean;
  /** Allowlist of stereotype tag names to render. Absent ⇒ show all; [] ⇒ show none. */
  stereotypeFilter?: string[];
  /** Per-stereotype-name color override. */
  stereotypeColors: Record<string, string>;
}
```

Replace `DEFAULT_DISPLAY`:

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

Update the `Diagram` interface: add `description?: string` and retype `display`:

```ts
export interface Diagram {
  key: string;
  title: string;
  profile: string;
  members: string[];
  hints?: DiagramHints;
  /** Free-text reviewer note. */
  description?: string;
  /** The raw STORED partial (only authored keys); always fed through resolveDisplay before use. */
  display?: Partial<DiagramDisplay>;
}
```

Leave `resolveDisplay` unchanged — its spread now covers the new keys.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/okf test display`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/okf/src/types.ts packages/okf/test/display.test.ts
git commit -m "feat(okf): extend DiagramDisplay + add Diagram.description"
```

---

## Task 8: Overlay maps display/description off the wire (@waml/core)

**Files:**
- Modify: `packages/core/src/state/overlay.ts` (`RustDiagram`, add `RustDiagramDisplay` + `partialDisplayFromWire`, `toModelGraph`)
- Test: `packages/core/src/state/overlay.test.ts`

**Interfaces:**
- Consumes: `DiagramDisplay` (Task 7), `RustDiagram` wire shape.
- Produces: `Diagram.display: Partial<DiagramDisplay>` + `Diagram.description` on the fused `ModelGraph`.

- [ ] **Step 1: Write the failing test**

Add to `packages/core/src/state/overlay.test.ts` (follow the existing `toModelGraph` test style in that file — construct a minimal `RustModel` and assert on `toModelGraph(model, emptyOverlay())`):

```ts
import { describe, it, expect } from "vitest";
import { toModelGraph, emptyOverlay, type RustModel } from "./overlay";

function modelWith(diagram: RustModel["diagrams"][number]): RustModel {
  return { nodes: [], edges: [], diagrams: [diagram], path: "", packages: [] };
}

describe("toModelGraph diagram display/description", () => {
  it("parses stereotypeColors list into a record and copies scalars", () => {
    const g = toModelGraph(
      modelWith({
        key: "d", title: "D", profile: "uml-domain", groups: [],
        description: "Notes",
        display: { showAttributes: false, maxAttributes: 6, stereotypeColors: ["entity:#ffedd5"] },
      }),
      emptyOverlay(),
    );
    expect(g.diagrams[0].description).toBe("Notes");
    expect(g.diagrams[0].display).toEqual({ showAttributes: false, maxAttributes: 6, stereotypeColors: { entity: "#ffedd5" } });
  });

  it("splits stereotypeColors on the first colon (hex keeps its own colons? no — hex has none)", () => {
    const g = toModelGraph(
      modelWith({ key: "d", title: "D", profile: "uml-domain", groups: [], display: { stereotypeColors: ["entity:#ffedd5"] } }),
      emptyOverlay(),
    );
    expect(g.diagrams[0].display?.stereotypeColors).toEqual({ entity: "#ffedd5" });
  });

  it("leaves display undefined when the wire carries no display", () => {
    const g = toModelGraph(modelWith({ key: "d", title: "D", profile: "uml-domain", groups: [] }), emptyOverlay());
    expect(g.diagrams[0].display).toBeUndefined();
    expect(g.diagrams[0].description).toBeUndefined();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/core test overlay`
Expected: FAIL — `RustDiagram` has no `display`/`description`; mapping is absent.

- [ ] **Step 3: Implement**

In `packages/core/src/state/overlay.ts`, add `DiagramDisplay` to the `@waml/okf` type import. Extend `RustDiagram` and add `RustDiagramDisplay`:

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

export interface RustDiagram {
  key: string;
  title: string;
  profile: string;
  groups: RustDiagramGroup[];
  description?: string;
  display?: RustDiagramDisplay;
}
```

Add the parser above `toModelGraph`:

```ts
/** Build a Partial<DiagramDisplay> from the wire partial: copy scalars verbatim,
 *  parse the "name:#hex" list (split on the FIRST colon) into a record. */
function partialDisplayFromWire(d: RustDiagramDisplay): Partial<DiagramDisplay> {
  const out: Partial<DiagramDisplay> = {};
  if (d.showAttributes !== undefined) out.showAttributes = d.showAttributes;
  if (d.attributeDetail !== undefined) out.attributeDetail = d.attributeDetail as DiagramDisplay["attributeDetail"];
  if (d.showAttributeVisibility !== undefined) out.showAttributeVisibility = d.showAttributeVisibility;
  if (d.showAttributeMultiplicity !== undefined) out.showAttributeMultiplicity = d.showAttributeMultiplicity;
  if (d.maxAttributes !== undefined) out.maxAttributes = d.maxAttributes;
  if (d.associationLabels !== undefined) out.associationLabels = d.associationLabels as DiagramDisplay["associationLabels"];
  if (d.emphasizeMultiplicity !== undefined) out.emphasizeMultiplicity = d.emphasizeMultiplicity;
  if (d.showStereotype !== undefined) out.showStereotype = d.showStereotype;
  if (d.stereotypeFilter !== undefined) out.stereotypeFilter = d.stereotypeFilter;
  if (d.stereotypeColors && d.stereotypeColors.length) {
    out.stereotypeColors = Object.fromEntries(
      d.stereotypeColors.map((s) => {
        const i = s.indexOf(":");
        return i >= 0 ? [s.slice(0, i), s.slice(i + 1)] : [s, ""];
      }),
    );
  }
  return out;
}
```

Update the `diagrams` map in `toModelGraph`:

```ts
  const diagrams: Diagram[] = model.diagrams.map((d) => ({
    key: d.key,
    title: d.title,
    profile: d.profile,
    members: flattenGroups(d.groups),
    ...(d.description !== undefined ? { description: d.description } : {}),
    ...(d.display ? { display: partialDisplayFromWire(d.display) } : {}),
  }));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/core test overlay`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/overlay.ts packages/core/src/state/overlay.test.ts
git commit -m "feat(core): map diagram display/description off the Rust wire"
```

---

## Task 9: `updateDiagramOps` in the ops adapter (@waml/core)

**Files:**
- Modify: `packages/core/src/state/ops-adapter.ts` (`OpDto` union, `DisplayDto`, `toDisplayDto`, `updateDiagramOps`)
- Test: `packages/core/src/state/ops-adapter.test.ts`

**Interfaces:**
- Consumes: `Diagram`, `DiagramDisplay`, `resolveDisplay` from `@waml/okf`.
- Produces: `export function updateDiagramOps(prev: Diagram, patch: Partial<Diagram>): OpDto[]`, `export interface DisplayDto`, and a `{ op: "diagram.set" }` member on `OpDto`. Consumed by Task 10 (store).

**Judgment call (noted for review):** the spec has the store pass a pre-resolved full display to the adapter. To keep `toDisplayDto` type-correct against `Diagram.display: Partial<DiagramDisplay>` and idempotent regardless of caller, `toDisplayDto` re-runs `resolveDisplay` internally (`resolveDisplay(full)` is a no-op). Behaviour matches the spec.

- [ ] **Step 1: Write the failing test**

Add to `packages/core/src/state/ops-adapter.test.ts` (mirror the existing `nodeSetOps`/`updateNodeOps` test style in that file). Import `updateDiagramOps` and `DEFAULT_DISPLAY`/`resolveDisplay` from `@waml/okf`:

```ts
import { updateDiagramOps } from "./ops-adapter";
import { resolveDisplay, type Diagram } from "@waml/okf";

const baseDiagram: Diagram = { key: "d", title: "D", profile: "uml-domain", members: [] };

describe("updateDiagramOps", () => {
  it("emits a title-only diagram.set for a title change", () => {
    expect(updateDiagramOps(baseDiagram, { title: "New" })).toEqual([{ op: "diagram.set", key: "d", title: "New" }]);
  });

  it("emits a desc-only diagram.set for a description change", () => {
    expect(updateDiagramOps(baseDiagram, { description: "Notes" })).toEqual([{ op: "diagram.set", key: "d", desc: "Notes" }]);
  });

  it("emits [] when nothing changed", () => {
    expect(updateDiagramOps(baseDiagram, { title: "D" })).toEqual([]);
  });

  it("emits a full display DTO, colors as a name:#hex list, undefined nullable fields omitted", () => {
    const display = resolveDisplay({ showAttributes: false, stereotypeColors: { entity: "#ffedd5" } });
    const ops = updateDiagramOps(baseDiagram, { display });
    expect(ops).toHaveLength(1);
    const dto = (ops[0] as { display: Record<string, unknown> }).display;
    expect(dto.showAttributes).toBe(false);
    expect(dto.stereotypeColors).toEqual(["entity:#ffedd5"]);
    expect("maxAttributes" in dto).toBe(false);
    expect("stereotypeFilter" in dto).toBe(false);
  });

  it("passes stereotypeFilter [] (show none) through as an empty array", () => {
    const display = resolveDisplay({ stereotypeFilter: [] });
    const dto = (updateDiagramOps(baseDiagram, { display })[0] as { display: { stereotypeFilter?: string[] } }).display;
    expect(dto.stereotypeFilter).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/core test ops-adapter`
Expected: FAIL — `updateDiagramOps` does not exist.

- [ ] **Step 3: Implement**

In `packages/core/src/state/ops-adapter.ts`:

Extend the type import from `@waml/okf` to include `Diagram, DiagramDisplay` and add a value import `import { ENDED_KINDS, resolveDisplay } from "@waml/okf";` (extend the existing `ENDED_KINDS` import line).

Add the `DisplayDto` interface and the new `OpDto` union member (add to the union near `node.set`):

```ts
export interface DisplayDto {
  showAttributes: boolean;
  attributeDetail: string;
  showAttributeVisibility: boolean;
  showAttributeMultiplicity: boolean;
  maxAttributes?: number;
  associationLabels: string;
  emphasizeMultiplicity: boolean;
  showStereotype: boolean;
  stereotypeFilter?: string[];
  stereotypeColors: string[];
}
```

```ts
  | { op: "diagram.set"; key: string; title?: string; desc?: string; display?: DisplayDto }
```

Add the converter + composite (place near `updateNodeOps`):

```ts
/** Full resolved DiagramDisplay → wire DisplayDto. Serializes the color record to a
 *  "name:#hex" list and passes maxAttributes/stereotypeFilter through as-is (undefined
 *  ⇒ omitted key ⇒ unlimited / show-all server-side). Resolves internally so a Partial
 *  input is safe. */
function toDisplayDto(display: Partial<DiagramDisplay>): DisplayDto {
  const d = resolveDisplay(display);
  return {
    showAttributes: d.showAttributes,
    attributeDetail: d.attributeDetail,
    showAttributeVisibility: d.showAttributeVisibility,
    showAttributeMultiplicity: d.showAttributeMultiplicity,
    ...(d.maxAttributes !== undefined ? { maxAttributes: d.maxAttributes } : {}),
    associationLabels: d.associationLabels,
    emphasizeMultiplicity: d.emphasizeMultiplicity,
    showStereotype: d.showStereotype,
    ...(d.stereotypeFilter !== undefined ? { stereotypeFilter: d.stereotypeFilter } : {}),
    stereotypeColors: Object.entries(d.stereotypeColors).map(([k, v]) => `${k}:${v}`),
  };
}

/** Scalar title/description + whole-block display. Emits a single diagram.set or []. */
export function updateDiagramOps(prev: Diagram, patch: Partial<Diagram>): OpDto[] {
  const set: Omit<Extract<OpDto, { op: "diagram.set" }>, "op" | "key"> = {};
  if (patch.title !== undefined && patch.title !== prev.title) set.title = patch.title;
  if (patch.description !== undefined && patch.description !== prev.description) set.desc = patch.description;
  if (patch.display !== undefined) set.display = toDisplayDto(patch.display);
  return Object.keys(set).length ? [{ op: "diagram.set", key: prev.key, ...set }] : [];
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/core test ops-adapter`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/ops-adapter.ts packages/core/src/state/ops-adapter.test.ts
git commit -m "feat(core): updateDiagramOps emits diagram.set ops"
```

---

## Task 10: Store `updateDiagram` persists (@waml/core)

**Files:**
- Modify: `packages/core/src/state/model.ts:271-273` (the `updateDiagram` no-op) + import block
- Test: `packages/core/src/state/model.test.ts`

**Interfaces:**
- Consumes: `updateDiagramOps` (Task 9), the existing `run(...)` + `graph()` internals.
- Produces: a real `store.updateDiagram(key, patch)` that persists for real diagram keys and no-ops for `ALL_DIAGRAM_KEY`/unknown keys.

- [ ] **Step 1: Write the failing test**

Add to `packages/core/src/state/model.test.ts` (this suite `await initWasm()` in `beforeAll` — follow the existing pattern in the file). Use a bundle that contains a real `Diagram` doc:

```ts
import { ALL_DIAGRAM_KEY } from "./diagrams";

it("updateDiagram persists display on a real diagram doc", async () => {
  const bundle: [string, string][] = [
    ["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"],
    ["dia.md", "---\ntype: Diagram\ntitle: D\nprofile: uml-domain\n---\n# D\n\n## Members\n- [Order](./order.md)\n"],
  ];
  const store = createModelStore(bundle);
  const key = store.get().diagrams.find((d) => d.title === "D")!.key;
  store.updateDiagram(key, { display: resolveDisplay({ showAttributes: false }) });
  const after = store.get().diagrams.find((d) => d.key === key)!;
  expect(after.display?.showAttributes).toBe(false);
});

it("updateDiagram on the implicit All diagram is a silent no-op", async () => {
  const store = createModelStore([["order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"]]);
  const before = store.get();
  store.updateDiagram(ALL_DIAGRAM_KEY, { display: resolveDisplay({ showAttributes: false }) });
  expect(store.get()).toBe(before); // no emit, bundle unchanged (graph().diagrams has no such key)
});
```

(Import `resolveDisplay` from `@waml/okf` and `createModelStore` per the file's existing imports. If `store.get()` identity is not a stable no-op signal in this suite, assert instead that `store.get().diagrams` is empty and no error was surfaced.)

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter @waml/core test model`
Expected: FAIL — `updateDiagram` is a no-op; `after.display` is undefined.

- [ ] **Step 3: Implement**

In `packages/core/src/state/model.ts`, add `updateDiagramOps` to the ops-adapter import block:

```ts
import {
  updateNodeOps,
  updateDiagramOps,
  nodeNewOps,
  // …existing imports…
} from "./ops-adapter";
```

Replace the `updateDiagram` no-op (currently `updateDiagram(_key, _patch) { /* no-op in 1b */ }`) with:

```ts
    updateDiagram(key: string, patch: Partial<Diagram>): void {
      // graph().diagrams holds only REAL diagram docs; the implicit "All" diagram
      // is synthesized downstream (effectiveDiagrams) and never appears here, so
      // the !prev guard makes edits on it a silent no-op (documented limitation).
      const prev = graph().diagrams.find((d) => d.key === key);
      if (!prev) return;
      run(updateDiagramOps(prev, patch));
    },
```

Leave `addDiagram`/`addDiagramFromMembers`/`removeDiagram` as their existing stubs.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter @waml/core test model`
Expected: PASS. Then `pnpm --filter @waml/core test` — all core tests green.

- [ ] **Step 5: Commit**

```bash
git add packages/core/src/state/model.ts packages/core/src/state/model.test.ts
git commit -m "feat(core): store.updateDiagram persists via diagram.set ops"
```

---

## Task 11: Rewire CanvasInner and retire the session store (@waml/web)

**Files:**
- Modify: `packages/web/src/components/canvas/CanvasInner.svelte` (lines ~21, ~153, ~326-333)
- Delete: `packages/web/src/state/displaySettings.svelte.ts`
- Delete: `packages/web/src/state/displaySettings.svelte.test.ts`

**Interfaces:**
- Consumes: `resolveDisplay` (already imported at `CanvasInner.svelte:59`), `store.updateDiagram` (Task 10).
- Produces: no new exports; `activeDisplay` keeps type `DiagramDisplay` and still flows into `toRFNode`/`buildRfEdges`/`runDagreLayout`/`CentralEditPanelHost` unchanged.

**Context:** This is glue over already-tested units; verify via `pnpm build` (typecheck) + a "nothing imports displaySettings" grep + the manual smoke check. On the implicit "All" diagram, `handleDisplayChange` calls `store.updateDiagram(ALL_DIAGRAM_KEY, …)`, which is the documented no-op from Task 10.

- [ ] **Step 1: Delete the session store and its test**

```bash
git rm packages/web/src/state/displaySettings.svelte.ts packages/web/src/state/displaySettings.svelte.test.ts
```

- [ ] **Step 2: Drop the import in CanvasInner**

Remove line 21 of `packages/web/src/components/canvas/CanvasInner.svelte`:

```ts
  import { displaySettings } from "../../state/displaySettings.svelte";
```

(Leave the `resolveDisplay, slugify, type DiagramDisplay` import at line 59 as-is — `resolveDisplay` is now used directly.)

- [ ] **Step 3: Repoint `activeDisplay` at the diagram's own display**

Replace line ~153:

```ts
  const activeDisplay = $derived(displaySettings.resolve(activeDiagram.key, activeDiagram.display));
```

with:

```ts
  const activeDisplay = $derived(resolveDisplay(activeDiagram.display));
```

- [ ] **Step 4: Rewrite `handleDisplayChange` to persist through the store**

Replace the `handleDisplayChange` function (lines ~326-333) with:

```ts
  // Merge the single-field panel patch onto the current resolved display and
  // persist the full display through the store. On the implicit "All" diagram
  // (no backing doc) store.updateDiagram is a documented no-op.
  function handleDisplayChange(p: Partial<DiagramDisplay>) {
    store.updateDiagram(activeDiagram.key, {
      display: resolveDisplay({ ...activeDiagram.display, ...p }),
    });
  }
```

(The TopBar rename path at line ~508, `store.updateDiagram(key, { title })`, now persists with no further change.)

- [ ] **Step 5: Verify nothing still references the deleted module**

Run: `git grep -n "displaySettings" -- packages/web/src`
Expected: no matches.

- [ ] **Step 6: Typecheck + build the web package**

Run: `pnpm --filter @waml/web build`
Expected: PASS — no unused-import or type errors (`resolveDisplay` and `DiagramDisplay` both still referenced; `displaySettings` gone).

- [ ] **Step 7: Manual smoke verification**

Run: `pnpm dev`. In a model that has a **real** `Diagram` doc: open Dock → Diagram properties, toggle a control, and confirm the canvas updates live AND the change survives a reload (it is now persisted to the bundle). Rename a diagram via the TopBar and confirm the new title survives reload. On a fresh model (implicit "All" view), confirm toggling a control does not throw (it is a no-op).

- [ ] **Step 8: Commit**

```bash
git add packages/web/src/components/canvas/CanvasInner.svelte packages/web/src/state/displaySettings.svelte.ts packages/web/src/state/displaySettings.svelte.test.ts
git commit -m "refactor(web): persist diagram display via store, retire session displaySettings"
```

---

## Task 12: Full workspace gate

**Files:** none (verification only).

- [ ] **Step 1: Run the complete gate**

Run: `cargo test --workspace && pnpm -r test && pnpm lint && pnpm build`
Expected: all green — Rust workspace tests, every Vitest suite (okf/core/web), eslint, and the full `@waml/wasm → okf → core → web` build.

- [ ] **Step 2: Confirm no stray references remain**

Run: `git grep -n "displaySettings"` and `git grep -n "no-op in 1b"`
Expected: no matches for `displaySettings`; the `updateDiagram` "no-op in 1b" comment is gone (the `addDiagram`/`removeDiagram` stub comment may remain — those are still stubs, out of scope).

---

## Self-Review

**Spec coverage:**
- Rust model (`DiagramDisplay` + `Diagram.description`/`display`) → Task 1. ✓
- `build_diagrams` reader incl. tri-state + maxAttributes floor + legacy → Task 2. ✓
- Serialization (generic path, fixpoint) → Task 3. ✓
- `Op::DiagramSet` + `DiagramDisplaySet` + block-replace + all 5 ops tests → Task 4. ✓
- DTO wire + `DisplayDto` + round-trip test cases → Task 5. ✓
- Wasm regen → Task 6. ✓
- TS types (`DiagramDisplay` +5 fields, `DEFAULT_DISPLAY` +3, `Diagram.description`, `display` retyped Partial) → Task 7. ✓
- Overlay (`RustDiagramDisplay`, `partialDisplayFromWire`, `toModelGraph`) → Task 8. ✓
- Ops adapter (`updateDiagramOps`, `DisplayDto`, `toDisplayDto`) → Task 9. ✓
- Store `updateDiagram` + `!prev` "All" no-op → Task 10. ✓
- Web rewire + retire `displaySettings` → Task 11. ✓
- Full gate → Task 12. ✓

**Placeholder scan:** No TBD/TODO/"handle edge cases"/"similar to Task N" — every code step shows the actual code.

**Type consistency:** `DiagramDisplaySet` field names identical across Rust ops (Task 4), DTO converters (Task 5). Frontmatter/serde/TS keys all camelCase (`showAttributeVisibility` etc.). `toDisplayDto`/`partialDisplayFromWire`/`DisplayDto` field sets match the Rust `DisplayDto`. `updateDiagramOps(prev: Diagram, patch: Partial<Diagram>)` matches the store call in Task 10 and the `Diagram` type from Task 7. `stereotypeColors` is `Record<string,string>` in TS `DiagramDisplay`, `string[]` on both wires (`RustDiagramDisplay`, `DisplayDto`) — conversions live in `partialDisplayFromWire` (in) and `toDisplayDto` (out).

**Assumptions flagged inline** (also in the final report): (A) `@waml/*`/`crates/waml` naming vs the spec's `@uaml`; (B) `wasm-pack` required for Task 6; (C) two `solve/resolve.rs` test fixtures need the new `Diagram` fields; (D) `toDisplayDto` re-resolves internally for type-safety/idempotency; (E) web handler has no isolated unit test — verified via build + grep + manual smoke, per the spec's "(or a focused unit)".
