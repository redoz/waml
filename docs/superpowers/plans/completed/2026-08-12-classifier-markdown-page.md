# Classifier Markdown Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The classifier preview surface stops drawing a single class-diagram card and renders a generated documentation page — prose identity, a definition list of properties, and associations written as directional sentences — on the markdown reading view, with clickable links to the classifiers it names.

**Architecture:** A pure `classifier_page(model, key) -> Option<String>` in the `waml` crate turns one classifier node plus every edge that touches it into markdown; it has no editor dependency, so a CLI subcommand can emit the identical page later. `ClassifierPreviewView` compiles that string through the existing `parse_markdown -> compile_presentation -> build_reading_document` path and installs it on the shared `MarkdownViewer`, replacing `body.show_canvas` with `body.show_markdown_viewer`. Reading-view links become clickable for the first time: `ReadingDocument` starts carrying the presentation plan's `PresentedLink`s, the viewer widget hit-tests a `FingerUp` through its existing flow-to-source map, and `ClassifierPreviewView::handle` resolves the destination through `navigation::resolve_link`.

**Tech Stack:** Rust (workspace crates `waml`, `waml-markdown-editor`, `waml-editor`), makepad-widgets (pinned fork), `cargo test`, `vitest`/`eslint`/`tsc` for `editors/vscode`.

**Spec:** `docs/superpowers/specs/2026-08-12-classifier-markdown-page-design.md`

## Global Constraints

- **Task headings are H3 (`### Task N: ...`).** Do not promote them.
- **Every task must leave the repo green under the full gate**, in this order:
  ```bash
  cargo test --workspace
  cd editors/vscode && pnpm build && pnpm test && pnpm lint
  ```
  `editors/vscode`'s packaging test asserts `dist/` exists, so `pnpm build` MUST precede `pnpm test`.
- CI also runs `cargo clippy --workspace --all-targets --all-features -- -D warnings`. `dead_code` is therefore a hard error: never leave a private item without a caller.
- CI also runs `cargo fmt`. Run `cargo fmt --all` before every commit.
- **Commit messages in this repo carry no Claude co-author trailer.** Subject + body only.
- `Annotates` is never a relationship: it anchors a `uml.Note`. Skip it everywhere, matching `build_classifier_view` (`crates/waml-editor/src/inspector.rs:568`).
- Class names are never inflected. `one or more Wheel` is deliberate and correct.
- Properties keep UML multiplicity notation (`1..*`) and suppress a bare `1`. Association sentences spell multiplicity out in words and always state it when the far end declares one. The two differ deliberately.
- Out of scope, do not add: operations, editing from the page, any diagram or graphic, package/directory surfaces.

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/waml/src/classifier_page.rs` (new) | The whole pure generator: multiplicity speller, far-end namer, sentence table, page assembly. Private helpers + `#[cfg(test)] mod tests`. | 1–4 |
| `crates/waml/src/lib.rs` | Register `pub mod classifier_page;`. | 1 |
| `crates/waml/tests/classifier_page.rs` (new) | Fixture-driven string snapshots of whole pages. | 4 |
| `crates/waml-markdown-editor/src/reading/model.rs` | `ReadingLink` + `ReadingDocument::links` + `link_at`. | 5 |
| `crates/waml-markdown-editor/src/reading/mod.rs` | Re-export `ReadingLink`. | 5 |
| `crates/waml-markdown-editor/tests/reading_model.rs` | Link-carrying tests. | 5 |
| `crates/waml-markdown-editor/src/reading/widget.rs` | `FingerUp` hit-test, `MarkdownViewerAction::LinkClicked`, two `test_` accessors. | 6 |
| `crates/waml-markdown-editor/tests/reading_widget_draw.rs` | Drawn-geometry link hit test. | 6 |
| `crates/waml-editor/src/classifier_preview_view.rs` | Surface swap + page install (Task 7), link routing (Task 8). | 7–8 |
| `crates/waml-editor/src/scene.rs` | Delete `build_focus_scene` and its two tests. | 9 |

---

### Task 1: Multiplicity spelled out in words

**Why:** Spec §"Multiplicity in words". A pure, table-driven speller is the smallest testable unit of the generator and everything else builds on it.

**Files:**
- Create: `crates/waml/src/classifier_page.rs`
- Modify: `crates/waml/src/lib.rs` (module list, alphabetical — between `bundle_envelope` and `diagnostic`)

**Interfaces:**
- Consumes: `waml::multiplicity::Multiplicity` (`parse(&str) -> Option<Multiplicity>`, `as_str(&self) -> &str`).
- Produces, for Tasks 2–4 (private to this module):
  - `fn spell_multiplicity(raw: &str) -> Option<String>` — `None` when `raw` is not a multiplicity `Multiplicity::parse` accepts, so a caller omits the count entirely.
  - `fn number_word(n: u64) -> String` — `"zero"`..`"ten"`, decimal digits above ten.

**Design decisions locked here (the spec's table, filled in):**

| Multiplicity | Prose |
| --- | --- |
| `1` | one |
| `0..1` | zero or one |
| `1..*` | one or more |
| `*` | zero or more |
| `0..*` | zero or more |
| `n` (exact, n >= 2) | exactly {word(n)} |
| `lo..*` (lo >= 2) | {word(lo)} or more |
| `a..b` (any other range) | {word(a)} to {word(b)} |

- [ ] **Step 1: Write the failing test**

Create `crates/waml/src/classifier_page.rs` containing ONLY the test module for now:

```rust
//! A pure markdown page for one classifier: prose identity, a definition list
//! of properties, and every relationship written as a directional sentence.
//!
//! Model in, markdown out. No editor dependency — a CLI subcommand can emit
//! the identical page.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_of_the_prose_table_spells_out() {
        let cases = [
            ("1", "one"),
            ("0..1", "zero or one"),
            ("1..*", "one or more"),
            ("0..*", "zero or more"),
            ("*", "zero or more"),
            ("3", "exactly three"),
            ("2..5", "two to five"),
            ("2..*", "two or more"),
        ];
        for (raw, expected) in cases {
            assert_eq!(
                spell_multiplicity(raw).as_deref(),
                Some(expected),
                "multiplicity {raw}"
            );
        }
    }

    #[test]
    fn numbers_spell_out_through_ten_and_show_digits_above_it() {
        assert_eq!(number_word(0), "zero");
        assert_eq!(number_word(10), "ten");
        assert_eq!(number_word(11), "11");
        // Both boundaries, as read through the speller itself.
        assert_eq!(spell_multiplicity("10").as_deref(), Some("exactly ten"));
        assert_eq!(spell_multiplicity("11").as_deref(), Some("exactly 11"));
    }

    #[test]
    fn an_unparseable_multiplicity_spells_nothing() {
        // `Multiplicity::parse` rejects each of these, so the sentence must
        // omit the count rather than invent one.
        for raw in ["", "0", "many", "1..", "5..2", "-1"] {
            assert_eq!(spell_multiplicity(raw), None, "multiplicity {raw:?}");
        }
    }
}
```

Add to `crates/waml/src/lib.rs`, keeping the list alphabetical:

```rust
pub mod classifier_page;
```

(insert it directly after `pub mod bundle_envelope;`)

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml classifier_page`
Expected: FAIL — `cannot find function 'spell_multiplicity' in this scope` and the same for `number_word`.

- [ ] **Step 3: Write minimal implementation**

Insert above the `#[cfg(test)] mod tests` block in `crates/waml/src/classifier_page.rs`:

```rust
use crate::multiplicity::Multiplicity;

/// Cardinal numbers spelled out through ten; above ten prose reads worse than
/// digits, so digits win.
fn number_word(n: u64) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    match WORDS.get(n as usize) {
        Some(word) => (*word).to_string(),
        None => n.to_string(),
    }
}

/// A UML multiplicity as English. `None` when `raw` is not a multiplicity this
/// crate can parse — the caller then omits the count entirely rather than
/// printing notation into a sentence.
fn spell_multiplicity(raw: &str) -> Option<String> {
    let parsed = Multiplicity::parse(raw)?;
    let raw = parsed.as_str();
    if raw == "*" {
        return Some("zero or more".to_string());
    }
    let Some((lo, hi)) = raw.split_once("..") else {
        // An exact count. `1` is the ordinary case and reads as a plain
        // article; anything else is worth calling out.
        let n: u64 = raw.parse().ok()?;
        return Some(if n == 1 {
            "one".to_string()
        } else {
            format!("exactly {}", number_word(n))
        });
    };
    let lo: u64 = lo.parse().ok()?;
    if hi == "*" {
        return Some(match lo {
            0 => "zero or more".to_string(),
            1 => "one or more".to_string(),
            lo => format!("{} or more", number_word(lo)),
        });
    }
    let hi: u64 = hi.parse().ok()?;
    if lo == 0 && hi == 1 {
        return Some("zero or one".to_string());
    }
    Some(format!("{} to {}", number_word(lo), number_word(hi)))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml classifier_page`
Expected: PASS — 3 tests.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/classifier_page.rs crates/waml/src/lib.rs
git commit -m "feat(waml): spell UML multiplicity in words"
```

---

### Task 2: Far-end naming and the relationship sentence table

**Why:** Spec §"Associations" and §"Naming the far end". Direction is carried by word order, never by a glyph; each `RelationshipKind` gets one verb form for the subject-elided column and one for the named-subject column.

**Files:**
- Modify: `crates/waml/src/classifier_page.rs`

**Interfaces:**
- Consumes: `spell_multiplicity` (Task 1); `waml::model::RelationshipKind`.
- Produces, for Tasks 3–4 (private to this module):
  - `fn far_end_phrase(role: Option<&str>, classifier: &str, key: &str) -> String` — the linked noun phrase for the far end. The classifier name is always the link text; a declared role leads with the classifier following in parentheses.
  - `fn document_href(key: &str) -> String` — `"/{key}.md"`. Absolute so a page for a nested classifier still resolves from its own directory.
  - `fn outgoing_verb(kind: RelationshipKind) -> &'static str`
  - `fn incoming_verb(kind: RelationshipKind) -> &'static str`

**Locked verb table** (`Associates` is the one kind that shifts register: it is not a transitive verb in ordinary English, so the elided form is participial):

| `RelationshipKind` | `outgoing_verb` | `incoming_verb` |
| --- | --- | --- |
| `Associates` | `Associated with` | `is associated with` |
| `Aggregates` | `Aggregates` | `aggregates` |
| `Composes` | `Composes` | `composes` |
| `Specializes` | `Specializes` | `specializes` |
| `Implements` | `Implements` | `implements` |
| `Depends` | `Depends on` | `depends on` |
| `Includes` | `Includes` | `includes` |
| `Extends` | `Extends` | `extends` |
| `InstanceOf` | `Instance of` | `is an instance of` |
| `Links` | `Links to` | `links to` |

`Annotates` has no row: it is skipped before either verb function is reached. Both functions must still be total over the enum (no `_ =>` arm) so a new kind forces a decision here at compile time; give `Annotates` an `unreachable!` arm with a message naming the skip.

- [ ] **Step 1: Write the failing test**

Append inside `mod tests` in `crates/waml/src/classifier_page.rs`:

```rust
    #[test]
    fn a_far_end_without_a_role_is_just_the_linked_classifier() {
        assert_eq!(
            far_end_phrase(None, "Wheel", "wheel"),
            "[Wheel](/wheel.md)"
        );
    }

    #[test]
    fn a_declared_role_leads_and_the_classifier_follows_in_parentheses() {
        assert_eq!(
            far_end_phrase(Some("lines"), "OrderLine", "sales/order-line"),
            "lines ([OrderLine](/sales/order-line.md))"
        );
    }

    #[test]
    fn a_role_identical_to_the_classifier_is_not_repeated() {
        // "Customer (Customer)" says nothing twice.
        assert_eq!(
            far_end_phrase(Some("Customer"), "Customer", "customer"),
            "[Customer](/customer.md)"
        );
    }

    #[test]
    fn every_kind_has_both_a_subject_elided_and_a_named_subject_verb() {
        use crate::model::RelationshipKind as RK;
        let cases = [
            (RK::Associates, "Associated with", "is associated with"),
            (RK::Aggregates, "Aggregates", "aggregates"),
            (RK::Composes, "Composes", "composes"),
            (RK::Specializes, "Specializes", "specializes"),
            (RK::Implements, "Implements", "implements"),
            (RK::Depends, "Depends on", "depends on"),
            (RK::Includes, "Includes", "includes"),
            (RK::Extends, "Extends", "extends"),
            (RK::InstanceOf, "Instance of", "is an instance of"),
            (RK::Links, "Links to", "links to"),
        ];
        for (kind, out, incoming) in cases {
            assert_eq!(outgoing_verb(kind), out, "{kind:?} outgoing");
            assert_eq!(incoming_verb(kind), incoming, "{kind:?} incoming");
        }
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml classifier_page`
Expected: FAIL — `cannot find function 'far_end_phrase'` (and `outgoing_verb`, `incoming_verb`).

- [ ] **Step 3: Write minimal implementation**

Add to `crates/waml/src/classifier_page.rs`, above `mod tests`:

```rust
use crate::model::RelationshipKind;

/// The link a classifier's page is written to. Absolute from the bundle root:
/// a node key IS its concept id, so `/{key}.md` resolves the same from any
/// referring directory (`waml-editor`'s `navigation::resolve_link` normalises
/// a leading `/` against the bundle root).
fn document_href(key: &str) -> String {
    format!("/{key}.md")
}

/// The far end's noun phrase. The classifier name is the link text either way;
/// a declared role leads, with the classifier in parentheses behind it. A role
/// spelled exactly like the classifier adds nothing, so it collapses.
///
/// Class names are never inflected: a plural count beside a singular name
/// ("one or more Wheel") is deliberate. The name is an identifier and must
/// match the model exactly; an author who wants a plural noun declares a role,
/// which is their own text.
fn far_end_phrase(role: Option<&str>, classifier: &str, key: &str) -> String {
    let link = format!("[{classifier}]({})", document_href(key));
    match role {
        Some(role) if !role.is_empty() && role != classifier => format!("{role} ({link})"),
        _ => link,
    }
}

/// The verb under `## Associations`, where this classifier is the elided
/// subject. `Associates` is the one kind that shifts register: it is not a
/// transitive verb in ordinary English ("Associates one Customer" reads as a
/// typo), so its elided form is the participial "Associated with".
fn outgoing_verb(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Associates => "Associated with",
        RelationshipKind::Aggregates => "Aggregates",
        RelationshipKind::Composes => "Composes",
        RelationshipKind::Specializes => "Specializes",
        RelationshipKind::Implements => "Implements",
        RelationshipKind::Depends => "Depends on",
        RelationshipKind::Includes => "Includes",
        RelationshipKind::Extends => "Extends",
        RelationshipKind::InstanceOf => "Instance of",
        RelationshipKind::Links => "Links to",
        RelationshipKind::Annotates => {
            unreachable!("Annotates anchors a uml.Note and is skipped before any verb lookup")
        }
    }
}

/// The verb under `## Referenced by`, where the FAR classifier is the named
/// subject and this one is the object.
fn incoming_verb(kind: RelationshipKind) -> &'static str {
    match kind {
        RelationshipKind::Associates => "is associated with",
        RelationshipKind::Aggregates => "aggregates",
        RelationshipKind::Composes => "composes",
        RelationshipKind::Specializes => "specializes",
        RelationshipKind::Implements => "implements",
        RelationshipKind::Depends => "depends on",
        RelationshipKind::Includes => "includes",
        RelationshipKind::Extends => "extends",
        RelationshipKind::InstanceOf => "is an instance of",
        RelationshipKind::Links => "links to",
        RelationshipKind::Annotates => {
            unreachable!("Annotates anchors a uml.Note and is skipped before any verb lookup")
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml classifier_page`
Expected: PASS — 7 tests.

`spell_multiplicity` is not yet called by non-test code. If clippy flags it as dead, add `#[allow(dead_code)]` on it with the comment `// Wired into the association sentences in Task 4.` and REMOVE that attribute in Task 4.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/classifier_page.rs
git commit -m "feat(waml): name a relationship's far end and its verb"
```

---

### Task 3: The page head, properties and values

**Why:** Spec §"Page generator" sections 1–4. This lands `classifier_page` itself with every section that reads only the node, so the association sections in Task 4 slot into a working page.

**Files:**
- Modify: `crates/waml/src/classifier_page.rs`

**Interfaces:**
- Consumes: `waml::model::{Model, Node, Attribute, ElementType, UmlMetaclass, Visibility}`; `Model::node(key) -> Option<&Node>`; `node.concept.title: Option<String>`, `node.concept.description: Option<String>`. `node.concept.body` is deliberately not read.
- Produces:
  - `pub fn classifier_page(model: &Model, key: &str) -> Option<String>` — the module's only public item.

**Section order (fixed; each section omitted when it would be empty):**
1. `# {title}` — `concept.title`, falling back to `key`.
2. Dek line — kind label, then each stereotype as `«name»`, then `abstract` when `abstract_`; joined by ` · `.
3. Description paragraph — `concept.description`.
4. `## Properties` (or `## Values` for `uml.Enum`).
5. `## Associations` — Task 4.
6. `## Referenced by` — Task 4.
`concept.body` is deliberately NOT emitted, and the spec now says so
explicitly. It holds "The full markdown body (everything after the
frontmatter), verbatim" (`crates/waml/src/okf.rs:304`) — for `sixkind/car.md`
that is the whole of `# Car`, `## Attributes` and `## Relationships` — so
appending it would paste the entire source document under every generated
page. No "body minus the structured UML sections" field or helper exists.
Surfacing author prose the structured sections do not already carry needs a
body splitter, which is its own design.

**Property bullet shape** — name and type always, multiplicity only when it is not a bare `1`, visibility only when declared, description as an indented continuation line:

```markdown
- `id` · `OrderId`
- `total` · `Decimal` — private
- `lines` · `OrderLine` `1..*`
  The line items on the order.
```

Note: `Attribute::description` is never populated by lowering today (`crates/waml/src/uml/declared.rs:275` hard-codes `description: None`), so its test builds the `Attribute` by hand.

- [ ] **Step 1: Write the failing test**

Append inside `mod tests` in `crates/waml/src/classifier_page.rs`:

```rust
    use crate::model::{Attribute, Model, Node, TypeRef};
    use crate::multiplicity::Multiplicity;
    use crate::source::SourceBundle;

    /// The projection of a small in-test bundle — the same path the editor
    /// installs (`prepare_candidate` -> `uml().projection`).
    fn projection(pairs: &[(&str, &str)]) -> Model {
        let source = SourceBundle::try_from_pairs(
            pairs
                .iter()
                .map(|(path, text)| ((*path).to_string(), (*text).to_string())),
        )
        .expect("fixture bundle parses");
        crate::analysis::prepare_candidate(source, None, 0)
            .expect("fixture analyses")
            .uml()
            .projection
            .clone()
    }

    #[test]
    fn a_missing_key_has_no_page() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )]);
        assert_eq!(classifier_page(&model, "nope"), None);
    }

    #[test]
    fn the_head_carries_title_kind_stereotypes_abstract_and_description() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Purchase Order\ndescription: One customer order.\nstereotype: [aggregateRoot, entity]\nabstract: true\n---\n# Order\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert_eq!(
            page,
            "# Purchase Order\n\nClass · «aggregateRoot» · «entity» · abstract\n\nOne customer order.\n"
        );
    }

    #[test]
    fn a_title_less_node_falls_back_to_its_key() {
        let model = projection(&[("order.md", "---\ntype: uml.Class\n---\n")]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(page.starts_with("# order\n"), "page was:\n{page}");
    }

    #[test]
    fn properties_show_type_always_and_multiplicity_only_when_it_is_not_one() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n- -total: Decimal\n- lines: OrderLine {1..*}\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(page.contains("## Properties\n\n"), "page was:\n{page}");
        assert!(page.contains("- `id` · `OrderId`\n"), "page was:\n{page}");
        assert!(
            page.contains("- `total` · `Decimal` — private\n"),
            "page was:\n{page}"
        );
        assert!(
            page.contains("- `lines` · `OrderLine` `1..*`\n"),
            "page was:\n{page}"
        );
    }

    #[test]
    fn an_attribute_description_is_an_indented_continuation_line() {
        // Lowering never sets `Attribute::description` today, so build one.
        let mut model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        )]);
        let node: &mut Node = model
            .nodes
            .iter_mut()
            .find(|node| node.key == "order")
            .expect("the fixture node");
        node.attributes.push(Attribute {
            name: "lines".into(),
            ty: TypeRef {
                name: "OrderLine".into(),
                ref_: None,
            },
            multiplicity: Multiplicity::parse("1..*"),
            visibility: None,
            description: Some("The line items on the order.".into()),
        });
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            page.contains("- `lines` · `OrderLine` `1..*`\n  The line items on the order.\n"),
            "page was:\n{page}"
        );
    }

    #[test]
    fn every_visibility_marker_has_a_word() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- +a: A\n- -b: B\n- #c: C\n- ~d: D\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        for expected in [
            "- `a` · `A` — public\n",
            "- `b` · `B` — private\n",
            "- `c` · `C` — protected\n",
            "- `d` · `D` — package\n",
        ] {
            assert!(page.contains(expected), "missing {expected:?} in:\n{page}");
        }
    }

    #[test]
    fn an_enum_renders_values_in_place_of_properties() {
        let model = projection(&[(
            "state.md",
            "---\ntype: uml.Enum\ntitle: State\n---\n# State\n\n## Values\n- OPEN\n- Ready for use\n",
        )]);
        let page = classifier_page(&model, "state").expect("the node has a page");
        assert!(page.contains("## Values\n\n"), "page was:\n{page}");
        assert!(page.contains("- `OPEN`\n"), "page was:\n{page}");
        assert!(page.contains("- `Ready for use`\n"), "page was:\n{page}");
        assert!(
            !page.contains("## Properties"),
            "an enum must not also emit Properties:\n{page}"
        );
    }

    /// `concept.body` is the WHOLE markdown body, so echoing it would
    /// duplicate everything the page just rendered in prose. This guards that
    /// omission — it is deliberate, not an oversight.
    #[test]
    fn the_page_does_not_echo_the_source_document_back() {
        let model = projection(&[(
            "order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n",
        )]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            !page.contains("## Attributes"),
            "the authored UML section must not be pasted back in:\n{page}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml classifier_page`
Expected: FAIL — `cannot find function 'classifier_page' in this scope`.

- [ ] **Step 3: Write minimal implementation**

Extend the `use` line at the top of `crates/waml/src/classifier_page.rs`:

```rust
use crate::model::{Attribute, ElementType, Model, Node, RelationshipKind, UmlMetaclass, Visibility};
```

and add above `mod tests`:

```rust
/// The classifier's own page, as markdown. `None` when `key` names no node.
///
/// Sections emit in a fixed order and each is omitted when it would be empty:
/// title, dek, description, properties (or values), associations, then
/// referenced by.
pub fn classifier_page(model: &Model, key: &str) -> Option<String> {
    let node = model.node(key)?;
    let mut sections: Vec<String> = Vec::new();

    let title = node.concept.title.clone().unwrap_or_else(|| key.to_string());
    sections.push(format!("# {title}"));

    if let Some(dek) = dek_line(node) {
        sections.push(dek);
    }
    if let Some(description) = node
        .concept
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        sections.push(description.to_string());
    }
    if let Some(members) = member_section(node) {
        sections.push(members);
    }
    // `concept.body` is deliberately not emitted: it is the whole source
    // document, so echoing it would repeat every section above.

    Some(format!("{}\n", sections.join("\n\n")))
}

/// Kind label, stereotypes as guillemet names, then `abstract`. `None` when a
/// node somehow carries none of the three.
fn dek_line(node: &Node) -> Option<String> {
    let mut parts = vec![kind_label(&node.ty)];
    parts.extend(
        node.stereotypes
            .iter()
            .map(|stereotype| format!("«{stereotype}»")),
    );
    if node.abstract_ {
        parts.push("abstract".to_string());
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The metaclass' own name (`Class`, `Interface`, `Enum`, `DataType`), without
/// the `uml.` family prefix. Mirrors the inspector's `kind_label`.
fn kind_label(ty: &ElementType) -> String {
    match ty {
        ElementType::Uml(metaclass) => metaclass.name().to_string(),
        other => {
            let text = other.as_str();
            text.strip_prefix("uml.").unwrap_or(&text).to_string()
        }
    }
}

/// `## Values` for an enum, `## Properties` for every other classifier.
/// `None` when the node declares neither.
fn member_section(node: &Node) -> Option<String> {
    if node.ty == ElementType::Uml(UmlMetaclass::Enum) {
        if node.values.is_empty() {
            return None;
        }
        let bullets: Vec<String> = node
            .values
            .iter()
            .map(|value| format!("- `{value}`"))
            .collect();
        return Some(format!("## Values\n\n{}", bullets.join("\n")));
    }
    if node.attributes.is_empty() {
        return None;
    }
    let bullets: Vec<String> = node.attributes.iter().map(property_bullet).collect();
    Some(format!("## Properties\n\n{}", bullets.join("\n")))
}

/// One property. Name and type always; multiplicity only when it is not a bare
/// `1` (a definition list is scanned down a column, where a repeated `1` on
/// every row is noise); visibility only when declared; a description as an
/// indented continuation line under the bullet.
fn property_bullet(attribute: &Attribute) -> String {
    let mut line = format!("- `{}` · `{}`", attribute.name, attribute.ty.name);
    if let Some(multiplicity) = attribute
        .multiplicity
        .as_ref()
        .map(|multiplicity| multiplicity.as_str())
        .filter(|multiplicity| *multiplicity != "1")
    {
        line.push_str(&format!(" `{multiplicity}`"));
    }
    if let Some(visibility) = attribute.visibility {
        line.push_str(&format!(" — {}", visibility_word(visibility)));
    }
    if let Some(description) = attribute
        .description
        .as_deref()
        .map(str::trim)
        .filter(|description| !description.is_empty())
    {
        line.push_str(&format!("\n  {description}"));
    }
    line
}

fn visibility_word(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Public => "public",
        Visibility::Private => "private",
        Visibility::Protected => "protected",
        Visibility::Package => "package",
    }
}
```

This EXTENDS the `use crate::model::RelationshipKind;` line Task 2 added — do not add a second `use crate::model::...` line, that is a duplicate-import error. Drop any name from the list that clippy reports as unused.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml classifier_page`
Expected: PASS — 15 tests.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/classifier_page.rs
git commit -m "feat(waml): render a classifier's identity and properties as markdown"
```

---

### Task 4: Association sentences, both directions

**Why:** Spec §"Associations". This is the section the whole design exists for: `SpecialOrder specializes Car.` in words, with incoming relationships derived across documents — something the card could never show.

**Files:**
- Modify: `crates/waml/src/classifier_page.rs`
- Create: `crates/waml/tests/classifier_page.rs`

**Interfaces:**
- Consumes: `spell_multiplicity`, `far_end_phrase`, `outgoing_verb`, `incoming_verb`, `classifier_page` (Tasks 1–3); `waml::model::{Edge, RelEnd}` (`edge.source`, `edge.target`, `edge.kind`, `edge.from_end`, `edge.to_end`, `edge.bidirectional`; `RelEnd { multiplicity, role, navigable }`).
- Produces: no new public API. `classifier_page` gains its sections 5 and 6.

**Rules locked here:**
- Skip `RelationshipKind::Annotates` entirely — it anchors a `uml.Note`, matching `build_classifier_view` and the web renderer.
- An edge is bidirectional when `edge.bidirectional` or both ends are `navigable == Some(true)`. It renders ONCE, under `## Associations`, with ` (both ways)` before the full stop, and must NOT also appear under `## Referenced by` — even when this classifier is the edge's target.
- Multiplicity is read from the FAR end and stated whenever that end declares one. Only the ended kinds (`Associates`/`Aggregates`/`Composes`) carry ends, so the structural kinds omit the count automatically — no special-casing needed.
- Under `## Referenced by` the far classifier is the named subject and takes no count and no role: `ShippingLabel depends on Order.` The object is this classifier's own title, in plain text — the reader is already on its page, so it is not a link.
- Both sections are omitted when empty; never emitted with no bullets under them.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` in `crates/waml/src/classifier_page.rs`:

```rust
    #[test]
    fn outgoing_relationships_elide_the_subject_and_spell_the_count() {
        let model = projection(&[
            (
                "car.md",
                "---\ntype: uml.Class\ntitle: Car\n---\n# Car\n\n## Relationships\n- specializes [Vehicle](./vehicle.md)\n- depends [Fuel](./fuel.md)\n- aggregates [Wheel](./wheel.md): 1 to *\n- composes [Engine](./engine.md): 1 to 1 engine\n",
            ),
            ("vehicle.md", "---\ntype: uml.Class\ntitle: Vehicle\n---\n"),
            ("fuel.md", "---\ntype: uml.Class\ntitle: Fuel\n---\n"),
            ("wheel.md", "---\ntype: uml.Class\ntitle: Wheel\n---\n"),
            ("engine.md", "---\ntype: uml.Class\ntitle: Engine\n---\n"),
        ]);
        let page = classifier_page(&model, "car").expect("the node has a page");
        assert!(page.contains("## Associations\n\n"), "page was:\n{page}");
        for expected in [
            "- Specializes [Vehicle](/vehicle.md).\n",
            "- Depends on [Fuel](/fuel.md).\n",
            "- Aggregates zero or more [Wheel](/wheel.md).\n",
            "- Composes one engine ([Engine](/engine.md)).\n",
        ] {
            assert!(page.contains(expected), "missing {expected:?} in:\n{page}");
        }
    }

    #[test]
    fn incoming_relationships_name_the_far_classifier_as_the_subject() {
        let model = projection(&[
            ("car.md", "---\ntype: uml.Class\ntitle: Car\n---\n# Car\n"),
            (
                "special-order.md",
                "---\ntype: uml.Class\ntitle: SpecialOrder\n---\n# SpecialOrder\n\n## Relationships\n- specializes [Car](./car.md)\n",
            ),
            (
                "shipping-label.md",
                "---\ntype: uml.Class\ntitle: ShippingLabel\n---\n# ShippingLabel\n\n## Relationships\n- depends [Car](./car.md)\n",
            ),
        ]);
        let page = classifier_page(&model, "car").expect("the node has a page");
        assert!(page.contains("## Referenced by\n\n"), "page was:\n{page}");
        for expected in [
            "- [SpecialOrder](/special-order.md) specializes Car.\n",
            "- [ShippingLabel](/shipping-label.md) depends on Car.\n",
        ] {
            assert!(page.contains(expected), "missing {expected:?} in:\n{page}");
        }
        assert!(
            !page.contains("## Associations"),
            "car declares no relationships of its own:\n{page}"
        );
    }

    #[test]
    fn a_note_anchor_is_not_a_relationship() {
        let model = projection(&[
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- annotates [Aside](./aside.md)\n",
            ),
            ("aside.md", "---\ntype: uml.Note\ntitle: Aside\n---\n"),
        ]);
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            !page.contains("## Associations") && !page.contains("## Referenced by"),
            "an Annotates anchor must produce no association section:\n{page}"
        );
    }

    #[test]
    fn a_bidirectional_edge_renders_once_under_associations() {
        use crate::model::{Edge, RelEnd};
        let mut model = projection(&[
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
            (
                "customer.md",
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
            ),
        ]);
        // Lowering never sets `bidirectional` today, so build the edge.
        model.edges.push(Edge {
            source: "customer".into(),
            target: "order".into(),
            kind: RelationshipKind::Associates,
            name: None,
            from_end: RelEnd {
                multiplicity: Multiplicity::parse("1"),
                role: None,
                navigable: Some(true),
            },
            to_end: RelEnd {
                multiplicity: Multiplicity::parse("1..*"),
                role: None,
                navigable: Some(true),
            },
            bidirectional: true,
        });
        let page = classifier_page(&model, "order").expect("the node has a page");
        assert!(
            page.contains("- Associated with one [Customer](/customer.md) (both ways).\n"),
            "page was:\n{page}"
        );
        assert!(
            !page.contains("## Referenced by"),
            "a bidirectional edge must not also appear as incoming:\n{page}"
        );
    }
```

Create `crates/waml/tests/classifier_page.rs` — the whole-page string snapshots the spec asks for, read from the existing fixture bundles:

```rust
//! Whole-page string snapshots of `classifier_page`. The output is reviewable
//! text, so the assertion is the text.

use std::path::{Path, PathBuf};

use waml::classifier_page::classifier_page;
use waml::model::Model;
use waml::source::SourceBundle;

/// The shared fixture bundles live with the editor's tests; `waml-cli`'s own
/// tests reach them the same way (`crates/waml-cli/src/io.rs`).
fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../waml-editor/tests/fixtures")
        .join(name)
}

/// Every `*.md` in `dir`, keyed by its bundle-relative path — the shape
/// `SourceBundle` wants, without pulling the editor's ingest in.
fn projection(name: &str) -> Model {
    let dir = fixture_dir(name);
    let mut pairs: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("fixture {name} is readable: {error}"))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                return None;
            }
            let key = path.file_name()?.to_str()?.to_string();
            Some((key, std::fs::read_to_string(&path).ok()?))
        })
        .collect();
    pairs.sort();
    let source = SourceBundle::try_from_pairs(pairs).expect("fixture bundle parses");
    waml::analysis::prepare_candidate(source, None, 0)
        .expect("fixture analyses")
        .uml()
        .projection
        .clone()
}

#[test]
fn sixkind_car_writes_every_relationship_kind_as_a_sentence() {
    let page = classifier_page(&projection("sixkind"), "car").expect("car has a page");
    assert_eq!(
        page,
        concat!(
            "# Car\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `vin` · `String`\n",
            "\n",
            "## Associations\n",
            "\n",
            "- Specializes [Vehicle](/vehicle.md).\n",
            "- Implements [Drivable](/drivable.md).\n",
            "- Depends on [Fuel](/fuel.md).\n",
            "- Associated with one [Driver](/driver.md).\n",
            "- Aggregates zero or more [Wheel](/wheel.md).\n",
            "- Composes one [Engine](/engine.md).\n",
        )
    );
}

#[test]
fn groups_linked_order_states_the_far_end_role_and_count() {
    let page = classifier_page(&projection("groups-linked"), "order").expect("order has a page");
    assert_eq!(
        page,
        concat!(
            "# Order\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `id` · `OrderId`\n",
            "- `total` · `Decimal`\n",
            "\n",
            "## Associations\n",
            "\n",
            "- Associated with one customer ([Customer](/customer.md)).\n",
            "- Associated with one invoice ([Invoice](/invoice.md)).\n",
        )
    );
}

/// The section a card could never draw: `Customer` declares no relationship of
/// its own, so everything here is derived from ANOTHER document's edges.
#[test]
fn groups_linked_customer_derives_referenced_by_across_documents() {
    let page = classifier_page(&projection("groups-linked"), "customer").expect("customer has a page");
    assert_eq!(
        page,
        concat!(
            "# Customer\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `id` · `CustomerId`\n",
            "- `name` · `String`\n",
            "\n",
            "## Referenced by\n",
            "\n",
            "- [Order](/order.md) is associated with Customer.\n",
        )
    );
}

/// Both association sections must be OMITTED, not emitted empty.
#[test]
fn mini_customer_has_no_association_sections_at_all() {
    let page = classifier_page(&projection("mini"), "customer").expect("customer has a page");
    assert_eq!(
        page,
        concat!(
            "# Customer\n",
            "\n",
            "Class\n",
            "\n",
            "## Properties\n",
            "\n",
            "- `id` · `CustomerId`\n",
            "- `name` · `String`\n",
        )
    );
}

#[test]
fn a_missing_key_has_no_page() {
    assert_eq!(classifier_page(&projection("mini"), "nope"), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml classifier_page`
Expected: FAIL — the in-module tests fail on the missing `## Associations` / `## Referenced by` sections; the integration snapshots fail on the same.

- [ ] **Step 3: Write minimal implementation**

In `crates/waml/src/classifier_page.rs`, insert the two sections into `classifier_page` between the `member_section` push and the body push:

```rust
    if let Some(associations) = association_section(model, node) {
        sections.push(associations);
    }
    if let Some(referenced_by) = referenced_by_section(model, node) {
        sections.push(referenced_by);
    }
```

and add these helpers above `mod tests`:

```rust
/// Both ends navigable — declared reciprocally, or flagged during resolution.
/// Such an edge renders ONCE, under `## Associations`, whichever side of it
/// this classifier sits on.
fn is_bidirectional(edge: &crate::model::Edge) -> bool {
    edge.bidirectional
        || (edge.from_end.navigable == Some(true) && edge.to_end.navigable == Some(true))
}

/// The classifier's title for a key, or the key when it names no node.
fn node_title(model: &Model, key: &str) -> String {
    model
        .node(key)
        .and_then(|node| node.concept.title.clone())
        .unwrap_or_else(|| key.to_string())
}

/// Outgoing relationships, plus every bidirectional edge that touches this
/// classifier from either side. The subject is this classifier and is elided.
fn association_section(model: &Model, node: &Node) -> Option<String> {
    let mut bullets: Vec<String> = Vec::new();
    for edge in &model.edges {
        if edge.kind == RelationshipKind::Annotates {
            continue;
        }
        let outgoing = edge.source == node.key;
        let incoming = edge.target == node.key;
        if !outgoing && !(incoming && is_bidirectional(edge)) {
            continue;
        }
        let (far_end, far_key) = if outgoing {
            (&edge.to_end, &edge.target)
        } else {
            (&edge.from_end, &edge.source)
        };
        let phrase = far_end_phrase(
            far_end.role.as_deref(),
            &node_title(model, far_key),
            far_key,
        );
        let count = far_end
            .multiplicity
            .as_ref()
            .and_then(|multiplicity| spell_multiplicity(multiplicity.as_str()));
        let subject = match count {
            Some(count) => format!("{count} {phrase}"),
            None => phrase,
        };
        let tail = if is_bidirectional(edge) {
            " (both ways)"
        } else {
            ""
        };
        bullets.push(format!("- {} {subject}{tail}.", outgoing_verb(edge.kind)));
    }
    (!bullets.is_empty()).then(|| format!("## Associations\n\n{}", bullets.join("\n")))
}

/// Incoming relationships, with the FAR classifier as the named subject and
/// this one as the object. A bidirectional edge already rendered under
/// `## Associations` and must not repeat here.
fn referenced_by_section(model: &Model, node: &Node) -> Option<String> {
    let title = node.concept.title.clone().unwrap_or_else(|| node.key.clone());
    let mut bullets: Vec<String> = Vec::new();
    for edge in &model.edges {
        if edge.kind == RelationshipKind::Annotates || is_bidirectional(edge) {
            continue;
        }
        if edge.target != node.key || edge.source == node.key {
            continue;
        }
        let subject = far_end_phrase(None, &node_title(model, &edge.source), &edge.source);
        bullets.push(format!("- {subject} {} {title}.", incoming_verb(edge.kind)));
    }
    (!bullets.is_empty()).then(|| format!("## Referenced by\n\n{}", bullets.join("\n")))
}
```

Remove any `#[allow(dead_code)]` added to `spell_multiplicity` in Task 2; it now has a real caller.

`is_bidirectional` is checked twice per outgoing edge above (once to decide inclusion, once for the tail). That is deliberate: hoisting it into a `let` before the `continue` reads worse than the two calls, and the function is three field reads.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p waml classifier_page`
Expected: PASS — the in-module tests and all 5 integration snapshots.

If a snapshot's expected string is wrong (a fixture attribute order, an extra section), fix the EXPECTED string only after reading the fixture and confirming the generator is right; do not soften the assertion to `contains`.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/classifier_page.rs crates/waml/tests/classifier_page.rs
git commit -m "feat(waml): write a classifier's relationships as sentences"
```

---

### Task 5: The reading model carries its links

**Why:** Spec §"Link navigation" step 1. `PresentationPlan` already carries `PresentedLink { owner, source_range, destination, title }`; `build_reading_document` drops every one of them, so nothing downstream can know a link exists. This is a pure-model change with no widget in it.

**Files:**
- Modify: `crates/waml-markdown-editor/src/reading/model.rs`
- Modify: `crates/waml-markdown-editor/src/reading/mod.rs` (re-export)
- Modify: `crates/waml-markdown-editor/tests/reading_model.rs`

**Interfaces:**
- Consumes: `crate::presentation::PresentationPlan` (`plan.links: Arc<[PresentedLink]>`; each has `source_range: TextRange` and `destination: Arc<str>`).
- Produces, for Task 6:
  - `pub struct ReadingLink { pub source_range: TextRange, pub destination: Arc<str> }` (`Clone, Debug, PartialEq`).
  - `ReadingDocument.links: Vec<ReadingLink>` — a new public field, in source order.
  - `pub fn ReadingDocument::link_at(&self, offset: TextSize) -> Option<&ReadingLink>` — the link whose `source_range` contains `offset` (half-open: `start <= offset < end`).

`ReadingDocument` is only ever constructed inside `model.rs` (one literal, at `build_reading_document`), so adding a field breaks no caller.

**Note on the spec's wording:** the spec says "keyed by flow range … through `ReadingDocument::source_span`". `source_span` is in fact `SourceMap::source_span` on the WIDGET (`crates/waml-markdown-editor/src/reading/widget.rs:164`), and the plan's links are keyed by SOURCE range. Key them by source range here; Task 6 maps flow to source at the widget, which is the direction the existing map already runs.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-markdown-editor/tests/reading_model.rs`:

```rust
#[test]
fn the_reading_model_carries_the_plans_links() {
    let doc = document("See [Customer](./customer.md) for more.\n");
    assert_eq!(doc.links.len(), 1, "one inline link");
    assert_eq!(&*doc.links[0].destination, "./customer.md");
}

#[test]
fn a_link_is_found_by_any_offset_inside_its_source_range() {
    use waml_markdown_editor::syntax::TextSize;

    let doc = document("See [Customer](./customer.md) for more.\n");
    let link = doc.links[0].clone();
    let start = link.source_range.start().to_usize();
    let end = link.source_range.end().to_usize();

    for offset in [start, start + 1, end - 1] {
        let found = doc
            .link_at(TextSize::try_from_usize(offset).unwrap())
            .unwrap_or_else(|| panic!("offset {offset} is inside the link"));
        assert_eq!(found.destination, link.destination);
    }
    // The end boundary is exclusive, and the leading "See " is outside.
    assert!(doc
        .link_at(TextSize::try_from_usize(end).unwrap())
        .is_none());
    assert!(doc.link_at(TextSize::try_from_usize(0).unwrap()).is_none());
}

#[test]
fn a_document_without_links_carries_none() {
    let doc = document("# Title\n\nJust prose.\n");
    assert!(doc.links.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-markdown-editor --test reading_model`
Expected: FAIL — `no field 'links' on type 'ReadingDocument'`.

- [ ] **Step 3: Write minimal implementation**

In `crates/waml-markdown-editor/src/reading/model.rs`:

Add `use std::sync::Arc;` to the imports, then, next to `ReadingDocument`:

```rust
/// A link the reading view can navigate to, keyed by the SOURCE range its
/// text occupies. The widget maps a click to a source offset through its own
/// flow-to-source map, so nothing here needs to know about pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadingLink {
    pub source_range: TextRange,
    pub destination: Arc<str>,
}
```

Give `ReadingDocument` the field:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ReadingDocument {
    pub roots: Vec<ReadingBlock>,
    pub source_len: TextSize,
    /// Every navigable link in the plan, in source order. Images are already
    /// excluded upstream (`compile.rs` skips a link whose owner is an image).
    pub links: Vec<ReadingLink>,
}
```

Add the lookup to `impl ReadingDocument`:

```rust
    /// The link covering `offset`, if any. Half-open: the end boundary belongs
    /// to whatever follows.
    pub fn link_at(&self, offset: TextSize) -> Option<&ReadingLink> {
        self.links
            .iter()
            .find(|link| link.source_range.start() <= offset && offset < link.source_range.end())
    }
```

In `build_reading_document`, populate it where the document literal is built:

```rust
    let document = ReadingDocument {
        roots: assembled,
        source_len: plan.source_len,
        links: plan
            .links
            .iter()
            .map(|link| ReadingLink {
                source_range: link.source_range,
                destination: link.destination.clone(),
            })
            .collect(),
    };
```

In `crates/waml-markdown-editor/src/reading/mod.rs`, add `ReadingLink` to the `model` re-export list (the line already naming `build_reading_document, ReadingBlock, ReadingBlockKind, ReadingDocument, ReadingError`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml-markdown-editor --test reading_model`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-markdown-editor/src/reading/model.rs crates/waml-markdown-editor/src/reading/mod.rs crates/waml-markdown-editor/tests/reading_model.rs
git commit -m "feat(markdown): carry presentation links into the reading model"
```

---

### Task 6: A click on a reading-view link posts an action

**Why:** Spec §"Link navigation" step 2. No markdown link is clickable in the reading view today — including the `[Customer](./customer.md)` links already in every fixture document — so this fixes reading-view links everywhere, not only on the generated page.

**Files:**
- Modify: `crates/waml-markdown-editor/src/reading/widget.rs`
- Modify: `crates/waml-markdown-editor/src/reading/mod.rs` (add `MarkdownViewerAction` to the `pub use widget::{...}` list — it is not re-exported today, and Task 8 constructs it from `waml-editor`)
- Modify: `crates/waml-markdown-editor/tests/reading_widget_draw.rs`

**Interfaces:**
- Consumes: `ReadingDocument::link_at` (Task 5); `SourceMap::source_span(Range<usize>) -> Option<TextRange>` and `SourceMap::area_slots_for_source` (both already on this widget); `WidgetNode::selection_point_to_char_index(&self, &Cx, DVec2) -> Option<usize>` (makepad, implemented by `TextFlow` over its selection tracker).
- Produces, for Task 8:
  - `MarkdownViewerAction::LinkClicked { destination: Arc<str> }` — a new variant on the existing enum.
  - `MarkdownViewerRef::link_clicked(&self, actions: &Actions) -> Option<Arc<str>>`.
  - `MarkdownViewerRef::test_link_at_point(&self, cx: &Cx, point: DVec2) -> Option<Arc<str>>`.
  - `MarkdownViewerRef::test_source_rects(&self, cx: &Cx, source: TextRange) -> Vec<Rect>`.

**Two makepad facts this task turns on — do not fight them:**
1. `Hit::FingerUp` is delivered only to the area that CAPTURED the `FingerDown`. The inner `TextFlow` captures it (its own `handle_event` hit-tests its area for selection), so the hit-test here must be against the FLOW's area, not `self.view.area()`. `MarkdownViewer::flow(cx)` already returns that `TextFlowRef`.
2. `TextFlow`'s selection buffer indices are exactly the flow indices `SourceMap::push_run` records, which is why `selected_source_span` already round-trips a selection into a source range. A click maps the same way, with a one-byte span.

A drag that ends over the text is a SELECTION, not a click: gate on `FingerUpEvent::was_tap()` and `is_over`.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-markdown-editor/tests/reading_widget_draw.rs`:

```rust
/// A drawn link is clickable: probing the centre of the pixels its text
/// actually occupied resolves to its destination, and a point well outside
/// resolves to nothing.
#[test]
fn a_point_inside_a_drawn_link_resolves_to_its_destination() {
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.init_cx_os();
    let ui = mounted_body(&mut cx);
    let viewer = ui
        .widget(&cx, ids!(markdown_viewer_surface.viewer))
        .as_markdown_viewer();

    let source = "See [Customer](./customer.md) for more.\n";
    let text = SourceText::new(source.to_owned()).expect("valid source text");
    let syntax = parse_markdown(
        DocumentRevision::INITIAL,
        text,
        MarkdownDialect::WAML_DEFAULT,
    )
    .expect("markdown parses");
    let styles = Arc::new(PresentationStyles::balanced());
    let plan = compile_presentation(&syntax, &styles, &HighlighterRegistry::default())
        .expect("presentation compiles");
    let document = Arc::new(build_reading_document(&plan).expect("reading model builds"));
    let link = document.links[0].clone();
    viewer.install_document(&mut cx, document, Arc::from(source));

    // Draw once, so the widget has real geometry to hit-test against.
    let pass = Pass::new(&mut cx);
    pass.set_size(&mut cx, dvec2(800.0, 600.0));
    let mut draw_list = DrawList::new(&mut cx);
    let mut draw_cx = Cx2d::new(&mut cx, &pass);
    draw_list.begin_always(&mut draw_cx);
    {
        let mut cx_2d = Cx2d::new(&mut draw_cx);
        cx_2d.begin_root_turtle(dvec2(800.0, 600.0), Layout::default());
        ui.draw_walk_all(&mut cx_2d, &mut Scope::empty(), Walk::fill());
        cx_2d.end_turtle();
        draw_list.end(&mut cx_2d);
    }
    draw_cx.end_pass(&pass);
    drop(draw_cx);

    let rects = viewer.test_source_rects(&cx, link.source_range);
    assert!(
        !rects.is_empty(),
        "the link's text must have drawn somewhere"
    );
    let centre = rects[0].pos + rects[0].size * 0.5;
    assert_eq!(
        viewer
            .test_link_at_point(&cx, centre)
            .as_deref(),
        Some("./customer.md"),
        "the centre of the drawn link resolves to its destination"
    );
    assert!(
        viewer
            .test_link_at_point(&cx, dvec2(-100.0, -100.0))
            .is_none(),
        "a point outside the document resolves to nothing"
    );
}
```

This test's draw block mirrors `a_mounted_viewer_paints_the_installed_document` in the same file — copy that test's exact draw scaffolding if the snippet above drifts from it; the imports it needs (`Pass`, `DrawList`, `Cx2d`, `dvec2`) come from `makepad_widgets::*`, already imported at the top of the file.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-markdown-editor --test reading_widget_draw`
Expected: FAIL — `no method named 'test_source_rects' found`.

- [ ] **Step 3: Write minimal implementation**

In `crates/waml-markdown-editor/src/reading/widget.rs`:

(a) Refactor `highlight_rects` so the geometry lookup is reusable, replacing its body's inner loop. Add next to it:

```rust
    /// Window-space rects of every drawn run overlapping `source`, from the
    /// LAST draw. Empty before the first draw of a document.
    fn source_rects(&self, cx: &Cx, source: TextRange) -> Vec<Rect> {
        let flow_ref = self.flow(cx);
        let Some(flow) = flow_ref.borrow() else {
            return Vec::new();
        };
        let mut rects = Vec::new();
        for slots in self.source_map.area_slots_for_source(source) {
            for slot in slots {
                let Some(area) = flow.areas_tracker.areas.get(slot) else {
                    continue;
                };
                let rect = area.rect(cx);
                if rect.size.x > 0.0 && rect.size.y > 0.0 {
                    rects.push(rect);
                }
            }
        }
        rects
    }
```

and rewrite `highlight_rects` to fold over it:

```rust
    fn highlight_rects(&self, cx: &Cx) -> Vec<Rect> {
        self.search_highlights
            .iter()
            .flat_map(|highlight| self.source_rects(cx, *highlight))
            .collect()
    }
```

(b) Add the point-to-destination lookup:

```rust
    /// The link destination under `point`, if any. Window-space point in,
    /// destination out: the flow maps the point to its own char index, the
    /// source map maps that back to a source offset, and the document
    /// answers which link covers it.
    fn link_at_point(&self, cx: &Cx, point: DVec2) -> Option<Arc<str>> {
        let document = self.document.as_ref()?;
        let flow_ref = self.flow(cx);
        let index = {
            let flow = flow_ref.borrow()?;
            flow.selection_point_to_char_index(cx, point)?
        };
        let span = self.source_map.source_span(index..index + 1)?;
        Some(document.link_at(span.start())?.destination.clone())
    }
```

(c) Post the action from `handle_event`, BEFORE the delegation to `self.view` (a `MouseUp` mutates no capture, so reading it first is safe, and it keeps the zoom-wheel early return above it untouched):

```rust
        // A tap that lands on a link navigates. Hit-tested against the FLOW's
        // area, not this widget's: the inner `TextFlow` captures the
        // `FingerDown` for its own selection, and makepad delivers `FingerUp`
        // only to the area that captured. A DRAG that ends over the text is a
        // selection, not a click, which is what `was_tap` screens out.
        let flow_area = self.flow(cx).area();
        if let Hit::FingerUp(fu) = event.hits(cx, flow_area) {
            if fu.is_over && fu.was_tap() {
                if let Some(destination) = self.link_at_point(cx, fu.abs) {
                    cx.widget_action(
                        self.widget_uid(),
                        MarkdownViewerAction::LinkClicked { destination },
                    );
                }
            }
        }
```

(d) Extend the action enum:

```rust
    /// A tap on a rendered markdown link. `destination` is the raw href, NOT
    /// resolved: resolution needs a bundle this widget has no business
    /// knowing about.
    LinkClicked { destination: Arc<str> },
```

(e) Widen `MarkdownViewerRef::zoom_wheel`'s `match` — it currently matches the single variant exhaustively and will stop compiling. Rewrite both accessors:

```rust
    pub fn zoom_wheel(&self, actions: &Actions) -> Option<f64> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.action.downcast_ref::<MarkdownViewerAction>()? {
            MarkdownViewerAction::ZoomWheel { delta } => Some(*delta),
            MarkdownViewerAction::LinkClicked { .. } => None,
        }
    }

    /// The raw href of a `LinkClicked` action posted by this widget, if
    /// `actions` carries one.
    pub fn link_clicked(&self, actions: &Actions) -> Option<Arc<str>> {
        let item = actions.find_widget_action(self.widget_uid())?;
        match item.action.downcast_ref::<MarkdownViewerAction>()? {
            MarkdownViewerAction::LinkClicked { destination } => Some(destination.clone()),
            MarkdownViewerAction::ZoomWheel { .. } => None,
        }
    }

    /// Test seam: the link destination under a window-space point, from the
    /// last draw. The same lookup `handle_event` runs on a tap.
    pub fn test_link_at_point(&self, cx: &Cx, point: DVec2) -> Option<Arc<str>> {
        self.borrow()?.link_at_point(cx, point)
    }

    /// Test seam: where a source range actually drew, from the last draw.
    pub fn test_source_rects(&self, cx: &Cx, source: TextRange) -> Vec<Rect> {
        self.borrow()
            .map(|inner| inner.source_rects(cx, source))
            .unwrap_or_default()
    }
```

If `TextFlowRef::area()` is not directly available, borrow the inner flow and call `flow.area()` — the same call `draw_walk` already makes to read `content_height`.

(f) In `crates/waml-markdown-editor/src/reading/mod.rs`, add `MarkdownViewerAction` to the `pub use widget::{...}` list, keeping it alphabetical:

```rust
pub use widget::{
    caret_for_span, MarkdownViewer, MarkdownViewerAction, MarkdownViewerRef,
    MarkdownViewerWidgetRefExt, SourceMap,
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml-markdown-editor --test reading_widget_draw`
Expected: PASS.

Then `cargo test -p waml-editor` — `app/actions.rs` and `book_surface.rs` consume `zoom_wheel`; the widened match must not have changed its behaviour.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-markdown-editor/src/reading/widget.rs crates/waml-markdown-editor/src/reading/mod.rs crates/waml-markdown-editor/tests/reading_widget_draw.rs
git commit -m "feat(markdown): make reading-view links clickable"
```

---

### Task 7: The classifier preview renders its page

**Why:** Spec §"Surface". This is the swap the whole design is for: `ClassifierPreviewView::sync` stops showing a focus canvas and shows the generated page instead. The inspector keeps its subject and its no-picker rule, so field editing is unchanged.

**Files:**
- Modify: `crates/waml-editor/src/classifier_preview_view.rs`

**Interfaces:**
- Consumes: `waml::classifier_page::classifier_page` (Task 4); `BodyWidgets::show_markdown_viewer` / `markdown_viewer()`; `waml_markdown_editor::syntax::{parse_markdown, DocumentRevision, MarkdownDialect, SourceText}`; `waml_markdown_editor::presentation::{compile_presentation, HighlighterRegistry, PresentationStyles}`; `waml_markdown_editor::reading::build_reading_document`; `ViewData { uml_analysis, revision, .. }`.
- Produces, for Task 8: `ClassifierPreviewView.page: Option<Arc<str>>` — the installed markdown, kept so `handle` can be tested and so a re-sync at the same revision is a no-op.

**What changes in `sync`:**
- `body.show_canvas(cx)` becomes `body.show_markdown_viewer(cx)`.
- The `build_focus_scene` + `canvas.set_focus` block goes away entirely (with the `use crate::scene::build_focus_scene;` import).
- A compiled page is installed on `body.markdown_viewer()`.
- The inspector block, the selection-toolbar block and the view-bar block are UNCHANGED. Chrome is unchanged: `tool_dock: false`, `view_bar: false`, breadcrumb plus right dock.
- The `ClassDiagramSurfaceAction::NodeSelect` / `NodeDeselect` arms in `handle` go away: this view no longer shows a canvas, so those actions can no longer reach it.

Recompiling on every `sync` would reparse the page on every event pass, so gate on `ViewData::revision`.

- [ ] **Step 1: Write the failing test**

Append a `#[cfg(test)] mod tests` block to `crates/waml-editor/src/classifier_preview_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_session::EditorSession;

    /// The two surfaces this view arbitrates between, plus the inspector whose
    /// subject must survive the swap.
    fn mounted_body(cx: &mut Cx) -> (WidgetRef, BodyWidgets) {
        waml_markdown_editor::live_design(cx);
        let viewer = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(waml_markdown_editor::reading::MarkdownViewer::script_new_with_default),
        ));
        let mut viewer_body = cx.with_vm(View::script_new_with_default);
        viewer_body.children.push((live_id!(viewer), viewer));
        let viewer_body = WidgetRef::new_with_inner(Box::new(viewer_body));
        let mut viewer_surface = cx.with_vm(View::script_new_with_default);
        viewer_surface
            .children
            .push((live_id!(viewer_body), viewer_body));
        let viewer_surface = WidgetRef::new_with_inner(Box::new(viewer_surface));
        let inspector = WidgetRef::new_with_inner(Box::new(
            cx.with_vm(crate::inspector_panel::Inspector::script_new_with_default),
        ));
        let mut root = cx.with_vm(View::script_new_with_default);
        root.children
            .push((live_id!(markdown_viewer_surface), viewer_surface));
        root.children.push((live_id!(inspector), inspector));
        let ui = WidgetRef::new_with_inner(Box::new(root));
        let body = BodyWidgets::new(cx, &ui);
        (ui, body)
    }

    fn session_with_order() -> EditorSession {
        let source = waml::source::SourceBundle::try_from_pairs([
            (
                "order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Attributes\n- id: OrderId {1}\n\n## Relationships\n- associates [Customer](./customer.md): 1 to 1\n",
            ),
            (
                "customer.md",
                "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n",
            ),
        ])
        .unwrap();
        let mut session = EditorSession::default();
        session.replace(source).unwrap();
        session
    }

    #[test]
    fn sync_installs_the_generated_page_and_keeps_the_inspector_subject() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view =
            ClassifierPreviewView::new("order".into(), NavCategory::Class);

        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let page = view.test_page().expect("sync must generate a page");
        assert!(page.starts_with("# Order\n"), "page was:\n{page}");
        assert!(
            page.contains("- Associated with one [Customer](/customer.md)."),
            "page was:\n{page}"
        );
        let inspector = ui.widget(&cx, ids!(inspector));
        let inspector = inspector
            .borrow::<crate::inspector_panel::Inspector>()
            .expect("the inspector is mounted");
        assert_eq!(inspector.subject_key_for_test().as_deref(), Some("order"));
    }

    #[test]
    fn a_key_that_names_no_classifier_installs_nothing() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("nope".into(), NavCategory::Class);

        view.sync(&mut cx, &body, snapshot.borrowed().into());

        assert!(view.test_page().is_none());
    }

    #[test]
    fn chrome_is_unchanged_by_the_surface_swap() {
        let view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        let chrome = view.chrome();
        assert!(!chrome.tool_dock);
        assert!(!chrome.view_bar);
        assert!(chrome.document_header.breadcrumb);
        assert_eq!(chrome.document_header.right_dock, Some(Icon::PanelRight));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor classifier_preview_view`
Expected: FAIL — `no method named 'test_page' found`.

- [ ] **Step 3: Write minimal implementation**

Rewrite the head of `crates/waml-editor/src/classifier_preview_view.rs`:

```rust
//! `ClassifierPreviewView` — the single-element preview: the classifier's own
//! generated documentation page on the reading surface, plus the
//! inspector-without-picker. No canvas, no tool dock.

use std::sync::Arc;

use crate::doc_view::{
    BodyChrome, BodyWidgets, DocView, DocViewIdentity, DocumentHeaderChrome, RevealTarget,
    ViewData, ViewOutcome,
};
use crate::document::NavCategory;
use crate::icons::Icon;
use crate::inspector::Subject;
use makepad_widgets::*;
use waml_markdown_editor::presentation::{
    compile_presentation, HighlighterRegistry, PresentationStyles,
};
use waml_markdown_editor::reading::build_reading_document;
use waml_markdown_editor::syntax::{
    parse_markdown, DocumentRevision, MarkdownDialect, SourceText,
};

pub struct ClassifierPreviewView {
    key: String,
    category: NavCategory,
    /// The markdown installed on the viewer, kept so `handle` can resolve a
    /// clicked link and so a re-sync at the same revision is a no-op.
    page: Option<Arc<str>>,
    /// The session revision `page` was generated from.
    installed_revision: Option<u64>,
}

impl ClassifierPreviewView {
    pub fn new(key: String, category: NavCategory) -> ClassifierPreviewView {
        ClassifierPreviewView {
            key,
            category,
            page: None,
            installed_revision: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_page(&self) -> Option<&str> {
        self.page.as_deref()
    }

    /// Generate the page and install it on the reading surface. A failure to
    /// parse or compile leaves the previous page up and says so: a stale
    /// surface is otherwise indistinguishable from a current one.
    fn install_page(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        if self.installed_revision == Some(data.revision) {
            return;
        }
        self.installed_revision = Some(data.revision);
        let Some(markdown) = waml::classifier_page::classifier_page(
            &data.uml_analysis.projection,
            &self.key,
        ) else {
            self.page = None;
            return;
        };
        let Ok(text) = SourceText::new(markdown.clone()) else {
            log!("classifier preview {}: generated page is not valid source text", self.key);
            return;
        };
        let syntax = match parse_markdown(
            DocumentRevision::INITIAL,
            text,
            MarkdownDialect::WAML_DEFAULT,
        ) {
            Ok(syntax) => syntax,
            Err(error) => {
                log!("classifier preview {}: generated page did not parse: {error:?}", self.key);
                return;
            }
        };
        let styles = Arc::new(PresentationStyles::balanced());
        let plan = match compile_presentation(&syntax, &styles, &HighlighterRegistry::default()) {
            Ok(plan) => plan,
            Err(error) => {
                log!("classifier preview {}: presentation compile failed: {error:?}", self.key);
                return;
            }
        };
        let document = match build_reading_document(&plan) {
            Ok(document) => document,
            Err(error) => {
                log!("classifier preview {}: reading model build failed: {error:?}", self.key);
                return;
            }
        };
        let source: Arc<str> = Arc::from(markdown.as_str());
        self.page = Some(source.clone());
        body.markdown_viewer()
            .install_document(cx, Arc::new(document), source);
    }
}
```

Replace the first block of `sync` (the `show_canvas` line through the `canvas.set_focus` block) with:

```rust
    fn sync(&mut self, cx: &mut Cx, body: &BodyWidgets, data: ViewData<'_>) {
        body.show_markdown_viewer(cx);
        self.install_page(cx, body, data);
```

leaving the inspector, selection-toolbar and view-bar blocks that follow exactly as they are.

In `handle`, delete the whole `let canvas_action = ...; match canvas_action { ... }` block (lines currently handling `ClassDiagramSurfaceAction::NodeSelect` / `NodeDeselect`) — this view no longer shows a canvas. Keep the inline-edit promotion block above it and the selection-toolbar block below it. The `let model = &data.uml_analysis.projection;` binding at the top of `handle` becomes unused once the canvas arms are gone; delete it too (clippy will insist).

If `PresentationStyles::balanced()` returns an owned value rather than needing an `Arc`, match `reading_view.rs`'s call shape exactly.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml-editor classifier_preview_view`
Expected: PASS — 3 tests.

Then run `cargo test -p waml-editor` in full. `crates/waml-editor/src/app/tests/find_strip.rs` opens this view (`production_app_with_order_classifier`); its `next_on_a_canvas_tab_sets_a_spotlight_and_calls_reveal` asserts against the canvas surface's spotlight, which the app sets independently of which surface is visible, so it should still pass. If it does not, fix the TEST's expectation (the classifier preview is no longer a canvas tab) rather than restoring the canvas. Update the now-inaccurate comments at `find_strip.rs:117` and `:207` that describe this tab as a focus scene.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/classifier_preview_view.rs crates/waml-editor/src/app/tests/find_strip.rs
git commit -m "feat(editor): render a classifier preview as a markdown page"
```

---

### Task 8: A link on the page navigates

**Why:** Spec §"Link navigation" step 3. Classifier names in the page are links to that classifier's page; without this the page is a dead end.

**Files:**
- Modify: `crates/waml-editor/src/classifier_preview_view.rs`

**Interfaces:**
- Consumes: `MarkdownViewerRef::link_clicked(actions) -> Option<Arc<str>>` (Task 6); `crate::navigation::{resolve_link, NavigationIntent, NavigationTarget, OpenDisposition}`; `ViewData::okf_analysis` (`.bundle`); `ViewOutcome::navigation: Option<NavigationIntent>`.
- Produces: no new API.

**Resolution contract:**
- `resolve_link(&data.okf_analysis.bundle, &self.key, &href)` — `self.key` IS the current concept id (a node's key is exactly its concept id, `crates/waml/src/uml/analysis.rs:2344`).
- On `Ok(target)`: `ViewOutcome { navigation: Some(NavigationIntent::Resolved { target, disposition: OpenDisposition::Preview }), .. }`.
- On `Err(_)`: emit `NavigationIntent::MarkdownLink { current_concept_id: self.key.clone(), href }` instead. The app's `handle_navigation_intent` re-resolves it and puts `NavigationError::status_message` on the status bar — the only path that reports a broken link to the reader. A silent drop would look like a dead click.

- [ ] **Step 1: Write the failing test**

Append inside the `mod tests` block of `crates/waml-editor/src/classifier_preview_view.rs`:

```rust
    use crate::navigation::{NavigationIntent, NavigationTarget, OpenDisposition};

    /// Synthesize the action the viewer posts on a tap, without a live pointer.
    fn link_click(body: &BodyWidgets, href: &str) -> ActionsBuf {
        let viewer = body.markdown_viewer();
        vec![Box::new(WidgetAction {
            data: None,
            action: Box::new(
                waml_markdown_editor::reading::MarkdownViewerAction::LinkClicked {
                    destination: Arc::from(href),
                },
            ),
            widget_uid: viewer.widget_uid(),
            group: None,
        })]
    }

    #[test]
    fn a_link_click_resolves_to_the_target_document() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let actions = link_click(&body, "/customer.md");
        let outcome = view.handle(&mut cx, &body, &actions, snapshot.borrowed().into());

        assert_eq!(
            outcome.navigation,
            Some(NavigationIntent::Resolved {
                target: NavigationTarget::Document {
                    concept_id: "customer".into(),
                    surface: None,
                    fragment: None,
                },
                disposition: OpenDisposition::Preview,
            })
        );
    }

    #[test]
    fn an_unresolvable_link_defers_to_the_app_so_the_reader_is_told() {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.init_cx_os();
        let (_ui, body) = mounted_body(&mut cx);
        let session = session_with_order();
        let snapshot = session.snapshot();
        let mut view = ClassifierPreviewView::new("order".into(), NavCategory::Class);
        view.sync(&mut cx, &body, snapshot.borrowed().into());

        let actions = link_click(&body, "/missing.md");
        let outcome = view.handle(&mut cx, &body, &actions, snapshot.borrowed().into());

        assert_eq!(
            outcome.navigation,
            Some(NavigationIntent::MarkdownLink {
                current_concept_id: "order".into(),
                href: "/missing.md".into(),
            })
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor classifier_preview_view`
Expected: FAIL — `outcome.navigation` is `None`.

- [ ] **Step 3: Write minimal implementation**

In `ClassifierPreviewView::handle`, immediately after the inline-edit promotion block and before the selection-toolbar block, add:

```rust
        // A tap on a classifier name in the page. Resolved here, against this
        // classifier's own concept id, because a node key IS its concept id.
        // An unresolvable href is handed to the app instead: only its
        // `handle_navigation_intent` puts `NavigationError::status_message`
        // on the status bar, and a silent drop reads as a dead click.
        if let Some(href) = body.markdown_viewer().link_clicked(actions) {
            let href = href.to_string();
            out.navigation = Some(
                match crate::navigation::resolve_link(
                    &data.okf_analysis.bundle,
                    &self.key,
                    &href,
                ) {
                    Ok(target) => crate::navigation::NavigationIntent::Resolved {
                        target,
                        disposition: crate::navigation::OpenDisposition::Preview,
                    },
                    Err(_) => crate::navigation::NavigationIntent::MarkdownLink {
                        current_concept_id: self.key.clone(),
                        href,
                    },
                },
            );
            return out;
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml-editor classifier_preview_view`
Expected: PASS — 5 tests.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/classifier_preview_view.rs
git commit -m "feat(editor): navigate from a classifier page's links"
```

---

### Task 9: Retire the focus scene

**Why:** Spec §"Surface": `build_focus_scene` loses its only caller. Leaving a scene builder nobody builds invites the next reader to wire it back up.

**Files:**
- Modify: `crates/waml-editor/src/scene.rs`

**Interfaces:**
- Consumes: nothing. This task is a deletion.
- Produces: nothing.

**What goes:**
- `pub fn build_focus_scene` (`crates/waml-editor/src/scene.rs:1328`) and its doc comment.
- Its two tests in the same file's `mod tests`: `focus_scene_node_carries_attribute_rows` (~:2411) and `focus_scene_node_carries_declared_stereotypes` (~:2431). Both assert attribute rows and stereotypes on a projected node — behaviour `project_scene_node_carries_concept_and_members` (~:2450) and the `attribute_rows` tests already cover through the surviving path.

**What STAYS, and why — do not widen this deletion:**
- `crate::card::card_size` and `crate::card::mono_sheet`: still called by `scene.rs:1056`, `sizing.rs:14`, `sizing.rs:85` and the card module's own tests. No card-sizing path dies with `build_focus_scene`.
- `ClassDiagramSurface::set_focus` and `SceneUpdate::Focus` (`crates/waml-editor/src/canvas/class/widget.rs`): still exercised by that widget's own reconciliation tests, and `set_focus` is public API of the `waml-editor` LIB target, so it raises no `dead_code`.
- The comment at `scene.rs:218` names `build_focus_scene` as an example. Reword it to name the surviving sizer (`build_scene`'s `card_size` call) rather than deleting the paragraph.

- [ ] **Step 1: Confirm the caller is gone**

Run: `git grep -n "build_focus_scene" -- crates/`
Expected: matches ONLY in `crates/waml-editor/src/scene.rs` (the definition, the `:218` comment, and the two tests). If `crates/waml-editor/src/classifier_preview_view.rs` still appears, Task 7 was not completed — stop and finish it first.

- [ ] **Step 2: Delete**

Remove `build_focus_scene` and the two named tests from `crates/waml-editor/src/scene.rs`, and reword the `:218` comment so it no longer names a function that does not exist.

- [ ] **Step 3: Verify the deletion is complete and nothing else died**

```bash
git grep -n "build_focus_scene" -- crates/
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: no matches in `crates/`; clippy clean (in particular, no `dead_code` on anything `build_focus_scene` was the last caller of — if clippy names one, delete that too and note it in the commit body).

- [ ] **Step 4: Run the full gate**

```bash
cargo fmt --all
cargo test --workspace
cd editors/vscode && pnpm build && pnpm test && pnpm lint
```
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/waml-editor/src/scene.rs
git commit -m "refactor(editor): drop the classifier focus scene"
```

---

## Self-Review

**Spec coverage**

| Spec section | Task |
| --- | --- |
| §Surface — `show_canvas` -> `show_markdown_viewer`, inspector keeps subject and no-picker, chrome unchanged | 7 |
| §Surface — `build_focus_scene` removed, card-sizing paths audited | 9 |
| §Surface — all four classifier kinds through `NavCategory` | 3 (kind label is the metaclass name; the page shape is kind-independent apart from Enum) |
| §Page generator — signature, `None` for a missing key, fixed section order | 3, 4 |
| §Properties — name/type always, multiplicity not-`1`, visibility when declared, description continuation | 3 |
| §Properties — `## Values` for `uml.Enum` | 3 |
| §Page generator — `concept.body` deliberately not emitted, guarded by a test | 3 |
| §Associations — sentence table, both directions, `Associates` register shift | 2, 4 |
| §Associations — `Annotates` skipped | 4 |
| §Multiplicity in words — whole prose table, spell-out-through-ten | 1 |
| §Naming the far end — role leads, classifier in parentheses, name is the link, no inflection | 2 |
| §Naming the far end — bidirectional renders once with `(both ways)` | 4 |
| §Link navigation 1 — plan links into `ReadingDocument` | 5 |
| §Link navigation 2 — `FingerUp` hit-test, link-clicked action | 6 |
| §Link navigation 3 — `resolve_link` through `ViewOutcome` | 8 |
| §Testing — generator snapshots (sixkind/car, groups-linked/order, groups-linked/customer, mini/customer, enum, missing key) | 3, 4 |
| §Testing — multiplicity table + both ten-boundaries + unparseable | 1 |
| §Testing — far-end naming: role present, absent, identical to class name | 2 |
| §Testing — widget: a link click resolves to the expected target | 6, 8 |
| §Testing — view: page installed, inspector subject intact | 7 |
| §Out of scope — operations, editing, diagrams, package/directory surfaces | none (nothing added) |

**Type consistency** — `classifier_page(&Model, &str) -> Option<String>` is used identically in Tasks 3, 4 and 7. `ReadingLink { source_range, destination }` is defined in Task 5 and consumed in Task 6. `MarkdownViewerAction::LinkClicked { destination: Arc<str> }` is defined in Task 6 and constructed in Task 8's test with the same field name. `far_end_phrase(role, classifier, key)` keeps the same argument order in Tasks 2 and 4.

**Nothing is blocked.** The one open question — whether to emit `concept.body` verbatim — was taken back to the spec, which now states the omission and its reason. Task 3 guards it with a test.

**Known open questions the spec does not answer** (each resolved above with the reasoning stated inline; revisit only with the author):
- `2..*` and `2..5` share no example in the spec's prose table; generalised as "{word} or more" / "{word} to {word}".
- The fallback for an unparseable multiplicity is not specified; the count is omitted, matching the structural kinds.
- A far-end role spelled exactly like the classifier is not specified; it collapses to the bare linked name.
- `(both ways)` is specified as "trailing the sentence"; placed before the full stop so the line reads as English.
- `## Values` bullets have no worked example; code-spanned for symmetry with `## Properties`.
- The `## Referenced by` sentence carries neither a count nor a role, following the spec's own table column exactly.
- What happens when `resolve_link` fails is not specified; deferred to the app so the reader gets a status message.
