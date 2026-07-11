# UAML Rust Tooling — Plan 4: Validation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `uaml::validate`, an all-diagnostics (non-fail-fast) checker that returns a `Vec<Diagnostic>` with precise `file:line` locations: structural line errors, unresolved cross-references, duplicate slugs, the markdown-clean-frontmatter invariant, and an unknown-type warning.

**Architecture:** `validate(bundle: &[(String, String)]) -> Vec<Diagnostic>`. It first builds the classifier slug key-set, then scans each document's *source lines* (tracking frontmatter, `##` sections, and code fences) so every diagnostic has an exact line number. The library only produces diagnostics; rendering and exit codes are the CLI's job (Plan 5).

**Tech Stack:** Rust 2021, `regex`, `pulldown-cmark`. Builds on Plans 1–3.

## Global Constraints

- All Global Constraints from Plans 1–3 apply.
- **Non-fail-fast:** collect every diagnostic; never stop at the first.
- **Severity:** structural/cross-reference/duplicate/frontmatter problems are `Error`; unknown `type` and unresolved *diagram* members are `Warning` (they degrade gracefully).
- Attribute type-refs that don't resolve are **not** reported (they degrade to bare tokens — Plan 3).
- `annotates`/notes are unsupported in the first cut, so an `annotates` relationship line is reported as `malformed-relationship` with a "not supported yet" message.

---

### Task 1: `diagnostic` module

**Files:**
- Create: `crates/uaml/src/diagnostic.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Produces: `Severity`, `DiagCode` (with `as_str`, `severity`), `Diagnostic { severity, code, message, file, line }` (with `new` and `warn` constructors).

- [ ] **Step 1: Write the failing test**

`crates/uaml/src/diagnostic.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_has_stable_slug_and_severity() {
        assert_eq!(DiagCode::UnresolvedTarget.as_str(), "unresolved-target");
        assert_eq!(DiagCode::UnknownType.severity(), Severity::Warning);
        assert_eq!(DiagCode::MalformedAttribute.severity(), Severity::Error);
    }

    #[test]
    fn constructors_set_severity() {
        let e = Diagnostic::new(DiagCode::DuplicateSlug, "dup", "a.md", 1);
        assert_eq!(e.severity, Severity::Error);
        let w = Diagnostic::warn(DiagCode::UnresolvedTarget, "member", "a.md", 3);
        assert_eq!(w.severity, Severity::Warning);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml diagnostic`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement the types**

Prepend to `crates/uaml/src/diagnostic.rs`:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagCode {
    DuplicateSlug,
    FrontmatterNotClean,
    UnknownType,
    MalformedAttribute,
    MalformedRelationship,
    UnresolvedTarget,
}

impl DiagCode {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagCode::DuplicateSlug => "duplicate-slug",
            DiagCode::FrontmatterNotClean => "frontmatter-not-clean",
            DiagCode::UnknownType => "unknown-type",
            DiagCode::MalformedAttribute => "malformed-attribute",
            DiagCode::MalformedRelationship => "malformed-relationship",
            DiagCode::UnresolvedTarget => "unresolved-target",
        }
    }
    /// Default severity for this code (a specific site may downgrade to a warning).
    pub fn severity(self) -> Severity {
        match self {
            DiagCode::UnknownType => Severity::Warning,
            _ => Severity::Error,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub file: String,
    pub line: usize,
}

impl Diagnostic {
    pub fn new(code: DiagCode, message: impl Into<String>, file: impl Into<String>, line: usize) -> Diagnostic {
        Diagnostic { severity: code.severity(), code, message: message.into(), file: file.into(), line }
    }
    pub fn warn(code: DiagCode, message: impl Into<String>, file: impl Into<String>, line: usize) -> Diagnostic {
        Diagnostic { severity: Severity::Warning, code, message: message.into(), file: file.into(), line }
    }
}
```

- [ ] **Step 4: Wire the module in and run**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod diagnostic;
```

Run: `cargo test -p uaml diagnostic`
Expected: PASS — `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/diagnostic.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): diagnostic types"
```

---

### Task 2: `validate` — structural & cross-reference rules

**Files:**
- Create: `crates/uaml/src/validate.rs`
- Modify: `crates/uaml/src/lib.rs`

**Interfaces:**
- Consumes: `crate::diagnostic::*`, `crate::frontmatter::parse_frontmatter`, `crate::grammar::{parse_attribute_line, parse_relationship_line, parse_member_line}`, `crate::model::ClassifierType`.
- Produces: `pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic>` (this task: duplicate slugs, malformed attribute/relationship lines, unresolved relationship targets, unresolved diagram members).

- [ ] **Step 1: Write the failing tests**

`crates/uaml/src/validate.rs`:
```rust
use std::collections::{HashMap, HashSet};

use crate::diagnostic::{DiagCode, Diagnostic};
use crate::frontmatter::parse_frontmatter;
use crate::grammar::{parse_attribute_line, parse_member_line, parse_relationship_line};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::Severity;

    #[test]
    fn flags_unresolved_relationship_target() {
        let b = vec![("a/order.md".into(),
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- depends [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::UnresolvedTarget);
        assert_eq!(d[0].line, 8);
    }

    #[test]
    fn flags_missing_ends_on_composition() {
        let b = vec![
            ("a/order.md".into(),
             "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n\n## Relationships\n- composes [OrderLine](./order-line.md)\n".into()),
            ("a/order-line.md".into(),
             "---\ntype: uml.Class\ntitle: OrderLine\n---\n# OrderLine\n".into()),
        ];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::MalformedRelationship);
        assert!(d[0].message.contains("requires"));
    }

    #[test]
    fn flags_malformed_attribute() {
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- bad line without colon\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::MalformedAttribute);
    }

    #[test]
    fn flags_duplicate_slug() {
        let b = vec![
            ("a/order.md".into(), "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n".into()),
            ("b/order.md".into(), "---\ntype: uml.Class\ntitle: Order2\n---\n# Order2\n".into()),
        ];
        let d = validate(&b);
        assert_eq!(d.iter().filter(|x| x.code == DiagCode::DuplicateSlug).count(), 2);
    }

    #[test]
    fn unresolved_member_is_only_a_warning() {
        let b = vec![("d/dia.md".into(),
            "---\ntype: Diagram\ntitle: D\n---\n# D\n\n## Members\n- [Ghost](./ghost.md)\n".into())];
        let d = validate(&b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].code, DiagCode::UnresolvedTarget);
        assert_eq!(d[0].severity, Severity::Warning);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml validate`
Expected: FAIL — `validate` not found.

- [ ] **Step 3: Implement `validate` and `validate_doc`**

Prepend to `crates/uaml/src/validate.rs`:
```rust
fn slug_of(path: &str) -> String {
    let seg = path.rsplit(['/', '\\']).next().unwrap_or(path);
    seg.strip_suffix(".md").unwrap_or(seg).to_string()
}

fn doc_type(text: &str) -> String {
    parse_frontmatter(text).0.get_str("type").unwrap_or("uml.Class").to_string()
}

fn rel_error_message(line: &str) -> String {
    const ENDED: [&str; 3] = ["associates", "aggregates", "composes"];
    const OTHER: [&str; 3] = ["specializes", "implements", "depends"];
    let verb = line.trim_start_matches("- ").split_whitespace().next().unwrap_or("");
    let has_ends = line.contains(':');
    if ENDED.contains(&verb) && !has_ends {
        format!("'{verb}' requires ': <near> to <far>' multiplicity ends")
    } else if OTHER.contains(&verb) && has_ends {
        format!("'{verb}' does not take multiplicity ends")
    } else if verb == "annotates" {
        "note anchors ('annotates') are not supported yet".to_string()
    } else if !ENDED.contains(&verb) && !OTHER.contains(&verb) {
        format!("unknown relationship verb '{verb}'")
    } else {
        "malformed relationship line".to_string()
    }
}

fn validate_doc(path: &str, text: &str, keyset: &HashSet<String>, diags: &mut Vec<Diagnostic>) {
    let mut in_fm = false;
    let mut fm_done = false;
    let mut in_fence = false;
    let mut section = String::new();

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let trimmed = raw.trim_end_matches('\r').trim();

        if !fm_done && trimmed == "---" {
            if in_fm {
                fm_done = true;
            }
            in_fm = !in_fm;
            continue;
        }
        if in_fm {
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(h) = trimmed.strip_prefix("## ") {
            section = h.trim().to_lowercase();
            continue;
        }
        if !trimmed.starts_with("- ") {
            continue;
        }

        match section.as_str() {
            "attributes" => {
                if parse_attribute_line(trimmed).is_none() {
                    diags.push(Diagnostic::new(DiagCode::MalformedAttribute, "malformed attribute line", path, n));
                }
            }
            "relationships" => match parse_relationship_line(trimmed) {
                None => diags.push(Diagnostic::new(DiagCode::MalformedRelationship, rel_error_message(trimmed), path, n)),
                Some(r) => {
                    if !keyset.contains(&r.target_slug) {
                        diags.push(Diagnostic::new(
                            DiagCode::UnresolvedTarget,
                            format!("relationship target './{}.md' resolves to no document", r.target_slug),
                            path,
                            n,
                        ));
                    }
                }
            },
            "members" => {
                if let Some(m) = parse_member_line(trimmed) {
                    if !keyset.contains(&m.slug) {
                        diags.push(Diagnostic::warn(
                            DiagCode::UnresolvedTarget,
                            format!("diagram member './{}.md' resolves to no document", m.slug),
                            path,
                            n,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

pub fn validate(bundle: &[(String, String)]) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut keyset: HashSet<String> = HashSet::new();
    let mut slug_count: HashMap<String, usize> = HashMap::new();

    for (path, text) in bundle {
        let slug = slug_of(path);
        *slug_count.entry(slug.clone()).or_insert(0) += 1;
        if doc_type(text) != "Diagram" {
            keyset.insert(slug);
        }
    }

    for (path, text) in bundle {
        let slug = slug_of(path);
        if slug_count[&slug] > 1 {
            diags.push(Diagnostic::new(
                DiagCode::DuplicateSlug,
                format!("duplicate document slug '{slug}'"),
                path,
                1,
            ));
        }
        validate_doc(path, text, &keyset, &mut diags);
    }
    diags
}
```

- [ ] **Step 4: Wire the module in and run**

In `crates/uaml/src/lib.rs`, add:
```rust
pub mod validate;
```

Run: `cargo test -p uaml validate`
Expected: PASS — `5 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/validate.rs crates/uaml/src/lib.rs
git commit -m "feat(uaml): validate structural and cross-reference rules"
```

---

### Task 3: `validate` — markdown-invariant & unknown-type

**Files:**
- Modify: `crates/uaml/src/validate.rs`

**Interfaces:**
- Adds the frontmatter-clean invariant check and the unknown-`type` warning to `validate_doc`.

The markdown-invariant lint: a document whose source starts with `---` must have that block recognized by `pulldown-cmark` (with YAML metadata blocks enabled) as a `MetadataBlock`. If it isn't (malformed fence, leading blank line, etc.), CommonMark would render it as a thematic break + setext heading — a violation of the "valid, cleanly-rendering markdown" invariant.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/uaml/src/validate.rs`:
```rust
    #[test]
    fn flags_frontmatter_that_is_not_a_metadata_block() {
        // A blank line before the block breaks metadata-block recognition.
        let b = vec![("a/x.md".into(),
            "\n---\ntype: uml.Class\ntitle: X\n---\n# X\n".into())];
        let d = validate(&b);
        assert!(d.iter().any(|x| x.code == DiagCode::FrontmatterNotClean));
    }

    #[test]
    fn warns_on_unknown_type() {
        let b = vec![("a/x.md".into(),
            "---\ntype: bpmn.Task\ntitle: X\n---\n# X\n".into())];
        let d = validate(&b);
        let w = d.iter().find(|x| x.code == DiagCode::UnknownType).unwrap();
        assert_eq!(w.severity, crate::diagnostic::Severity::Warning);
        assert_eq!(w.line, 2);
    }

    #[test]
    fn clean_document_has_no_diagnostics() {
        let b = vec![("a/x.md".into(),
            "---\ntype: uml.Class\ntitle: X\n---\n# X\n\n## Attributes\n- id: XId\n".into())];
        assert!(validate(&b).is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p uaml validate`
Expected: FAIL — `flags_frontmatter_that_is_not_a_metadata_block` and `warns_on_unknown_type` fail.

- [ ] **Step 3: Implement the two checks**

Add these imports to the top of `crates/uaml/src/validate.rs`:
```rust
use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::model::ClassifierType;
```

Add this helper above `validate_doc`:
```rust
fn has_metadata_block(text: &str) -> bool {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_YAML_STYLE_METADATA_BLOCKS);
    Parser::new_ext(text, opts).any(|e| matches!(e, Event::Start(Tag::MetadataBlock(_))))
}
```

At the **start** of `validate_doc` (before the line loop), add the invariant check:
```rust
    if text.trim_start().starts_with("---") && !has_metadata_block(text) {
        diags.push(Diagnostic::new(
            DiagCode::FrontmatterNotClean,
            "frontmatter is not a clean CommonMark metadata block (would render as a thematic break + heading)",
            path,
            1,
        ));
    }
```

Inside the line loop, replace the `if in_fm { continue; }` block with a version that inspects the `type:` line:
```rust
        if in_fm {
            if let Some(rest) = trimmed.strip_prefix("type:") {
                let ty = rest.trim().trim_matches('"');
                if ty != "Diagram" && matches!(ClassifierType::parse(ty), ClassifierType::Unknown(_)) {
                    diags.push(Diagnostic::warn(
                        DiagCode::UnknownType,
                        format!("unknown type '{ty}' — rendered as a generic box"),
                        path,
                        n,
                    ));
                }
            }
            continue;
        }
```

- [ ] **Step 4: Run to verify passing**

Run: `cargo test -p uaml validate`
Expected: PASS — all validate tests green.

- [ ] **Step 5: Commit**

```bash
git add crates/uaml/src/validate.rs
git commit -m "feat(uaml): markdown-clean-frontmatter invariant and unknown-type warning"
```

---

### Task 4: golden fixture validates clean

**Files:**
- Modify: `crates/uaml/tests/golden.rs`

**Interfaces:**
- Consumes: `uaml::validate::validate`, `uaml::parse::split_bundle`.

- [ ] **Step 1: Write the failing test**

Add to `crates/uaml/tests/golden.rs`:
```rust
#[test]
fn orders_domain_has_no_diagnostics() {
    let bundle = uaml::parse::split_bundle(FIXTURE);
    let diags = uaml::validate::validate(&bundle);
    assert!(diags.is_empty(), "expected clean fixture, got: {diags:?}");
}
```

- [ ] **Step 2: Run to verify**

Run: `cargo test -p uaml --test golden`
Expected: PASS — the spec's own worked example validates with zero diagnostics.

- [ ] **Step 3: Run the full suite**

Run: `cargo test -p uaml`
Expected: PASS — all unit + integration tests.

- [ ] **Step 4: Commit**

```bash
git add crates/uaml/tests/golden.rs
git commit -m "test(uaml): golden orders-domain fixture validates clean"
```

---

## Self-Review

- **Spec coverage (this plan's slice):** all-diagnostics (non-fail-fast) validator returning `Vec<Diagnostic>` with `file:line` ✔; structural rules (malformed attribute/relationship, ends required/forbidden via the grammar + a tailored message) ✔ (Task 2); cross-reference (unresolved relationship target, duplicate slug) ✔ (Task 2); consistency warnings (unknown type; unresolved diagram member as warning) ✔ (Tasks 2–3); markdown-invariant lint (frontmatter clean) ✔ (Task 3); library returns data only, no rendering ✔. Reciprocal-multiplicity-mismatch warning is intentionally deferred (documented in the design's open items).
- **Placeholder scan:** none — every step has concrete code and commands.
- **Type consistency:** `validate` consumes the same `(path, text)` bundle shape as `build_model` (Plan 3). Grammar functions (`parse_attribute_line`/`parse_relationship_line`/`parse_member_line`) and `ClassifierType::parse` match Plans 1–2. `Diagnostic`/`DiagCode`/`Severity` are the exact types the CLI renders in Plan 5.
