# Folder View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A folder declares what it is (`profile`) and how it is shown (`view`) in its `index.md` frontmatter, and opening a folder in the editor shows a real, eventually-editable outline of its members.

**Architecture:** Model first, UI second. `okf::Index` grows three fields (`profile`, `view`, `extra`); `render_index` learns to emit them so no write path erases them; two pure `Bundle` queries (`resolved_profile`, `resolved_view`) compute what is *in effect* from what is *on disk*. Only then does the editor gain a folder document provider, a `FolderIndex` widget, and a tree row-versus-chevron split. `Index` and `Outline` are ONE widget in TWO modes (`editable: bool`), never two widgets.

**Tech Stack:** Rust (`crates/waml` — no editor dependency), makepad-based `crates/waml-editor`, TypeScript VS Code extension in `editors/vscode` (untouched by this work but part of the gate).

## Global Constraints

- **Gate for every task (all of it, every time):**
  - `cargo test --workspace`
  - `cd editors/vscode && npm run build && npm run test && npm run lint` (build FIRST — a stale `dist/` produces phantom typecheck errors).
- **`docs/specs/OKF_SPEC.md` stays byte-identical.** Do not amend, extend, or reference-edit it. Deviations are recorded in `docs/specs/waml-okf-extensions.md` (new, created in Task 2).
- **Exactly one new deviation family:** frontmatter keys in a non-root `index.md`. Only `profile`, `view`, and pass-through unknown keys. No new reserved filenames, no sidecar files, no nested index lists, no plain-text (non-link) index bullets, no profile FILE format.
- **`view:` is always a plain YAML scalar**, never a nested mapping: `index`, `outline`, `markdown`, `member:./orders` (no space after the `member:` prefix). An unrecognized value is treated as ABSENT and falls through to the next resolution step — never an error.
- **Tasks 1–5 are pure `crates/waml` model work.** They must not add any dependency on `waml-editor`, any UI type, or any file I/O. Keep them that way.
- **makepad widget rules (Tasks 7–12):**
  - Every NEW widget must be imported BY NAME in `crates/waml-editor/src/app.rs` `script_mod!` (lines 33–63) — there is no glob. An unregistered widget is silently dropped: no draw, no hit-test, and the gate stays GREEN. It must also get a `crate::<module>::script_mod(vm);` line in the boot list at `crates/waml-editor/src/app.rs:704-787`, and a child widget must be registered BEFORE its consumer.
  - Never reuse an existing makepad widget name. This plan introduces exactly one: `FolderIndex` (verified absent from `crates/` today).
  - The `script_mod` namespace must be assigned as ONE object literal, not field-by-field.
  - Inline `font_size` / `FontMember` is gate-banned. Fonts come from `crates/waml-editor/src/fonts.rs` (`fonts.text_label` and friends).
  - New modules must be declared in `crates/waml-editor/src/main.rs` (the `mod` list, lines 5–80).

## Verified touch points

Confirmed by reading the files in this worktree at plan time:

| Path | Line | What is there today |
|---|---|---|
| `crates/waml/src/okf.rs` | 235 | `pub struct Index` — 6 fields, no `profile`/`view`/`extra` |
| `crates/waml/src/okf.rs` | 319 | `fn frontmatter_is_empty(fm: &Frontmatter) -> bool` (already used by `Concept.extra`) |
| `crates/waml/src/okf.rs` | 283 / 295 / 305 / 313 | `Bundle::index` / `directory` / `indexes` / `directories` |
| `crates/waml/src/okf/shell.rs` | 25 | `const KNOWN_KEYS` (concept keys) |
| `crates/waml/src/okf/shell.rs` | 287 | synthetic (unauthored) `Index { .. }` construction |
| `crates/waml/src/okf/shell.rs` | 433 | `fn parse_authored_index` — already reads `shell.frontmatter.get_str("title")` |
| `crates/waml/src/okf/shell.rs` | 515 | authored `Index { .. }` construction |
| `crates/waml/src/index_md.rs` | 42 | `pub fn render_index(dir, title, description, members)` — emits NO frontmatter |
| `crates/waml/src/index_md.rs` | 74 | `pub fn reindex_source` — the write path that would erase frontmatter |
| `crates/waml/src/index_md.rs` | 199 | existing test asserting `!out.contains("---")` — must be updated |
| `crates/waml/src/frontmatter.rs` | 257 | `pub fn render_frontmatter(fm: &Frontmatter) -> String` |
| `crates/waml/src/model.rs` | 950 | `pub profile: String` — profile as *concept doc* frontmatter |
| `crates/waml/src/seed.rs` | 9-29 | `kind_frontmatter` / `new_diagram_doc` emit `type: Diagram` + `profile: uml-domain` |
| `crates/waml/src/ops/mod.rs` | 143 / 153 / 169 / 181 | `Op::NodeNew`, `Op::NodeSet`, `Op::PkgMove`, `Op::PkgReorder` |
| `crates/waml-editor/src/tree_panel.rs` | 320-349 | `fn row_navigation` — a directory row always yields `NavigationTarget::Directory` |
| `crates/waml-editor/src/tree_panel.rs` | 808-824 | `folder_clicked` → `ProjectTreeAction::Navigate` |
| `crates/waml-editor/src/tree_panel.rs` | 1565 | `folder_clicked_emits_intent_without_mutation_then_one_command_closes_it` |
| `crates/waml-editor/src/app/navigation.rs` | 290-300 | `NavigationTarget::Directory` handler — **folds only**, opens nothing |
| `crates/waml-editor/src/document.rs` | 20-31 | `NavCategory::Directory` already exists |
| `crates/waml-editor/src/view_history.rs` | 19-22 | `enum DocumentKind { Primary, Source }` |
| `crates/waml-editor/src/okf_documents.rs` | 14-71 | provider shape to copy: `*_tab_id`, `describe`, `open_with_asset_host` |
| `crates/waml-editor/src/doc_view.rs` | 25-42 | `BodyWidgets` (holds `ui`, surface handles) |
| `crates/waml-editor/src/doc_view.rs` | 356-363 | `enum DocViewIdentity` |
| `crates/waml-editor/src/doc_view.rs` | 372-486 | `trait DocView` |
| `crates/waml-editor/src/documents.rs` | 13-53 | provider chain `open_with_asset_host` / `open_locator_with_asset_host` |
| `crates/waml-editor/src/app.rs` | 33-63 | `script_mod!` widget imports (one by one) |
| `crates/waml-editor/src/app.rs` | 342-350 | `diagram_properties_wrap` — the body-surface sibling pattern to copy |

---

### Task 1: `ViewSpec` scalar type

**Files:**
- Create: `crates/waml/src/okf/view_spec.rs`
- Modify: `crates/waml/src/okf.rs:16` (add `pub mod view_spec;` next to `pub mod ops;`, and re-export)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/okf/view_spec.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub enum ViewSpec { Index, Outline, Member(String), Markdown }`
  - `pub fn parse_view_spec(value: &str) -> Option<ViewSpec>`
  - `impl std::fmt::Display for ViewSpec` (round-trip scalar form)
  - Re-exported from `crates/waml/src/okf.rs` as `pub use view_spec::ViewSpec;`

- [ ] **Step 1: Write the failing test**

Create `crates/waml/src/okf/view_spec.rs` containing ONLY the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_scalars_parse_and_round_trip() {
        for (text, spec) in [
            ("index", ViewSpec::Index),
            ("outline", ViewSpec::Outline),
            ("markdown", ViewSpec::Markdown),
            ("member:./orders", ViewSpec::Member("./orders".into())),
        ] {
            assert_eq!(parse_view_spec(text), Some(spec.clone()), "parse {text}");
            assert_eq!(spec.to_string(), text, "render {text}");
        }
    }

    #[test]
    fn unrecognized_values_are_absent_not_errors() {
        assert_eq!(parse_view_spec("kanban"), None);
        assert_eq!(parse_view_spec(""), None);
        assert_eq!(parse_view_spec("member:"), None);
        assert_eq!(parse_view_spec("member: ./orders"), None);
    }

    #[test]
    fn parsing_is_case_and_whitespace_tolerant_on_the_keyword_only() {
        assert_eq!(parse_view_spec("  Outline "), Some(ViewSpec::Outline));
        // The href after `member:` is taken verbatim, only outer-trimmed.
        assert_eq!(
            parse_view_spec(" member:./Orders "),
            Some(ViewSpec::Member("./Orders".into()))
        );
    }
}
```

Add `pub mod view_spec;` to `crates/waml/src/okf.rs` immediately after line 16 (`pub mod ops;`), and `pub use view_spec::ViewSpec;` beneath the module list.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml view_spec`
Expected: FAIL — `cannot find type ViewSpec` / `cannot find function parse_view_spec`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/waml/src/okf/view_spec.rs`:

```rust
//! `ViewSpec` — how a directory is shown. Serialized in index frontmatter as a
//! single plain YAML scalar (never a nested mapping) so a strict OKF consumer
//! sees one unremarkable string.

/// How a directory renders. Stored locally on `Index::view`; what is actually
/// used is computed by `Bundle::resolved_view`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ViewSpec {
    /// Rendered member listing. The fallback.
    Index,
    /// The listing plus editing.
    Outline,
    /// Delegate to one member's own view; the payload is the member href.
    Member(String),
    /// Raw `index.md` in the markdown editor.
    Markdown,
}

/// Parse the frontmatter scalar. An unrecognized value yields `None`, which
/// callers treat as "not declared" and fall through — never an error.
pub fn parse_view_spec(value: &str) -> Option<ViewSpec> {
    let value = value.trim();
    if let Some(href) = value.strip_prefix("member:") {
        // No space is permitted after the prefix: `member: x` is a YAML
        // mapping, which this format deliberately does not accept.
        if href.is_empty() || href.starts_with(char::is_whitespace) {
            return None;
        }
        return Some(ViewSpec::Member(href.trim_end().to_owned()));
    }
    match value.to_ascii_lowercase().as_str() {
        "index" => Some(ViewSpec::Index),
        "outline" => Some(ViewSpec::Outline),
        "markdown" => Some(ViewSpec::Markdown),
        _ => None,
    }
}

impl std::fmt::Display for ViewSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ViewSpec::Index => f.write_str("index"),
            ViewSpec::Outline => f.write_str("outline"),
            ViewSpec::Markdown => f.write_str("markdown"),
            ViewSpec::Member(href) => write!(f, "member:{href}"),
        }
    }
}
```

Note the third test trims the leading space before `member:./Orders`, so `parse_view_spec` must trim the whole value first (it does).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml view_spec`
Expected: PASS (3 tests).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/okf/view_spec.rs crates/waml/src/okf.rs
git commit -m "feat(okf): ViewSpec scalar for directory view declarations"
```

---

### Task 2: `Index` parses `profile`, `view`, and unknown keys

**Files:**
- Modify: `crates/waml/src/okf.rs:233-242` (the `Index` struct)
- Modify: `crates/waml/src/okf/shell.rs:287-295` (synthetic index) and `:433-525` (`parse_authored_index`)
- Create: `docs/specs/waml-okf-extensions.md`
- Test: inline tests in `crates/waml/src/okf/shell.rs`

**Interfaces:**
- Consumes: `ViewSpec`, `parse_view_spec` (Task 1).
- Produces: `Index { .., pub profile: Option<String>, pub view: Option<ViewSpec>, pub extra: Frontmatter }`. `members`, `title`, `description`, `body`, `authored` are UNCHANGED. `Index::extra` holds every frontmatter key that is not `title`, `profile`, or `view`.

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` at the bottom of `crates/waml/src/okf/shell.rs` (if that file has no test module, create one at the end with `use crate::{okf::{Bundle, ViewSpec}, source::SourceBundle};`):

```rust
#[test]
fn index_frontmatter_promotes_profile_and_view_and_keeps_unknown_keys() {
    let source = SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        (
            "sales/index.md",
            "---\ntitle: Sales\nprofile: uml-domain\nview: outline\ngenerator: acme\n---\n# Sales\n",
        ),
    ])
    .unwrap();

    let bundle = Bundle::parse(&source).unwrap();
    let index = bundle.index("/sales").unwrap();

    assert_eq!(index.title.as_deref(), Some("Sales"));
    assert_eq!(index.profile.as_deref(), Some("uml-domain"));
    assert_eq!(index.view, Some(ViewSpec::Outline));
    assert_eq!(index.extra.get_str("generator"), Some("acme"));
    assert_eq!(index.extra.get_str("profile"), None, "promoted key must not double up");
    assert_eq!(index.extra.get_str("title"), None);
    assert_eq!(index.extra.get_str("view"), None);
}

#[test]
fn an_index_without_frontmatter_parses_exactly_as_before() {
    let source = SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "# Sales\n\nA context.\n"),
    ])
    .unwrap();

    let bundle = Bundle::parse(&source).unwrap();
    let index = bundle.index("/sales").unwrap();

    assert_eq!(index.title.as_deref(), Some("Sales"));
    assert_eq!(index.profile, None);
    assert_eq!(index.view, None);
    assert!(index.extra.entries.is_empty());
    assert!(index.authored);
}

#[test]
fn an_unrecognized_view_value_reads_as_absent() {
    let source = SourceBundle::try_from_pairs([(
        "index.md",
        "---\nview: kanban\n---\n# Root\n",
    )])
    .unwrap();

    let index = Bundle::parse(&source).unwrap().index("/").unwrap().clone();

    assert_eq!(index.view, None);
    // An unparsed value is not silently dropped from the document either: it
    // survives in `extra` so a re-render preserves what the author wrote.
    assert_eq!(index.extra.get_str("view"), Some("kanban"));
}

#[test]
fn a_synthesized_index_for_a_directory_with_no_index_md_declares_nothing() {
    let source = SourceBundle::try_from_pairs([(
        "sales/order.md",
        "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
    )])
    .unwrap();

    let index = Bundle::parse(&source).unwrap().index("/sales").unwrap().clone();

    assert!(!index.authored);
    assert_eq!(index.profile, None);
    assert_eq!(index.view, None);
    assert!(index.extra.entries.is_empty());
}
```

Note the directory address form: `Bundle::index` takes the address string as `DirectoryAddress::as_str()` produces it (`"/"` for root, `"/sales"` for a child) — confirm against the existing test at `crates/waml-editor/src/tree_panel.rs:1544` which uses `"/sales"`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml index_frontmatter`
Expected: FAIL — `no field 'profile' on type '&Index'`.

- [ ] **Step 3: Add the three fields to `Index`**

`crates/waml/src/okf.rs`, replacing the struct body at line 235:

```rust
pub struct Index {
    pub directory: DirectoryAddress,
    pub title: Option<String>,
    pub description: Option<String>,
    pub members: Vec<String>,
    pub body: Option<SourceSlice>,
    pub authored: bool,
    /// Locally declared profile. What is in EFFECT is `Bundle::resolved_profile`.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub profile: Option<String>,
    /// Locally declared view. What is in EFFECT is `Bundle::resolved_view`.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub view: Option<ViewSpec>,
    /// Producer-specific index frontmatter keys with no dedicated field above.
    /// Preserved verbatim so a re-render never erases them.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "frontmatter_is_empty")
    )]
    pub extra: Frontmatter,
}
```

`frontmatter_is_empty` already exists at `crates/waml/src/okf.rs:319`; `Frontmatter` is already imported there (used by `Concept::extra`).

- [ ] **Step 4: Populate the fields in the parser**

In `crates/waml/src/okf/shell.rs`, add near `KNOWN_KEYS` (line 25):

```rust
/// Index frontmatter keys with a dedicated `Index` field. Everything else
/// lands in `Index::extra` and round-trips untouched.
const INDEX_KNOWN_KEYS: &[&str] = &["title", "profile", "view"];
```

In `parse_authored_index` (line 433), just after `title_from_frontmatter`:

```rust
    let profile = shell
        .frontmatter
        .get_str("profile")
        .map(str::trim)
        .filter(|profile| !profile.is_empty())
        .map(str::to_owned);
    let view = shell
        .frontmatter
        .get_str("view")
        .and_then(crate::okf::view_spec::parse_view_spec);
    // An unrecognized `view` stays in `extra` (it was not consumed), so the
    // author's text survives a re-render.
    let consumed_view = view.is_some();
    let extra = Frontmatter {
        entries: shell
            .frontmatter
            .entries
            .iter()
            .filter(|(key, _)| {
                if key == "view" {
                    return !consumed_view;
                }
                !INDEX_KNOWN_KEYS.contains(&key.as_str())
            })
            .cloned()
            .collect(),
    };
```

and extend the `Index { .. }` literal at line 515 with `profile,`, `view,`, `extra,`.

In the synthetic branch at line 287, add `profile: None, view: None, extra: Frontmatter::default(),`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p waml`
Expected: PASS. Any other `Index { .. }` literal that now fails to compile must be given the three defaults — search with `rg "Index \{" crates/`.

- [ ] **Step 6: Write the deviations document**

Create `docs/specs/waml-okf-extensions.md`:

```markdown
# WAML deviations from OKF

`docs/specs/OKF_SPEC.md` is an external standard and stays byte-identical.
This file records every place WAML's authored bundles deviate from it, and
what a strict OKF consumer sees.

One entry per deviation.

### 1. Frontmatter in a non-root `index.md`

OKF §6 says index files carry no frontmatter; §11 permits it only in the
bundle-root index, for `okf_version`.

WAML reads and writes a frontmatter block in ANY `index.md`, with these keys:

| Key | Meaning |
|---|---|
| `title` | The directory's title (pre-existing behavior). |
| `profile` | What the directory IS. A bare name; resolution is nearest-declaring-ancestor. |
| `view` | How the directory is SHOWN. Always a plain scalar: `index`, `outline`, `markdown`, or `member:<href>`. |

Any other key is preserved verbatim on round-trip and otherwise ignored.

**Degradation for a strict consumer:** the block renders as a leading YAML
block or is skipped. No member, link, or body content is affected. Members
stay flat and link-only; no new reserved filenames or sidecar files are
introduced. A bundle authored by WAML remains readable by any OKF consumer.
```

- [ ] **Step 7: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add crates/waml/src/okf.rs crates/waml/src/okf/shell.rs docs/specs/waml-okf-extensions.md
git commit -m "feat(okf): parse profile, view, and unknown keys from index frontmatter"
```

---

### Task 3: `render_index` emits frontmatter and round-trips

**Files:**
- Modify: `crates/waml/src/index_md.rs:42-70` (`render_index`), `:74-134` (`reindex_source`), `:199` (the `!out.contains("---")` assertion)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/index_md.rs:143`

**Interfaces:**
- Consumes: `Index::{profile, view, extra}` (Task 2), `ViewSpec::to_string` (Task 1), `crate::frontmatter::render_frontmatter` (`crates/waml/src/frontmatter.rs:257`).
- Produces: new signature

  ```rust
  pub fn render_index(
      dir: &str,
      title: Option<&str>,
      description: Option<&str>,
      members: &[IndexEntry],
      frontmatter: &IndexFrontmatter<'_>,
  ) -> String
  ```

  with

  ```rust
  #[derive(Default)]
  pub struct IndexFrontmatter<'a> {
      pub profile: Option<&'a str>,
      pub view: Option<&'a crate::okf::ViewSpec>,
      pub extra: Option<&'a crate::frontmatter::Frontmatter>,
  }
  ```

  `IndexFrontmatter::default()` emits no block at all, so a caller with nothing to declare renders exactly today's bytes.

  Also produced by this task (see Steps 7–9): `update_authored_index` (`crates/waml/src/okf/lower.rs:669`) updates a frontmatter `title:` key when one is present, not just the H1.

  **Why this task is needed — and what it is NOT.** `render_index` runs on the op write path in exactly one place: `crates/waml/src/okf/lower.rs:845`, the guarded `else` branch of `write_package_index` where the index file **does not yet exist**. A newly created index must be able to carry declarations, and `reindex_source` (used by `crates/waml/tests/golden.rs:815-851` and any future full-rebuild path) must not drop them.

  It is **not** true that Tasks 9–12 depend on this for correctness. For an index that already exists, the op write path is `update_authored_index` (`crates/waml/src/okf/lower.rs:669`), which edits `index.md` surgically: it rewrites the member ranges and the H1 and leaves the rest of the file — including any frontmatter block — untouched. `reindex_source` has no callers outside `crates/waml/src/index_md.rs`'s own tests, `crates/waml/tests/golden.rs`, and the `#[deprecated]` shim at `crates/waml/src/index_md.rs:136`. So do **not** blanket-append `&IndexFrontmatter::default()` to call sites as a safety measure: the `lower.rs:845` site takes `default()` because that branch has no parsed index to read declarations from, and that is the correct value there.

  The genuine ordering constraint is Steps 7–9 of this task: `Op::PkgRetitle` must move a frontmatter `title:` before Task 10 ships directory-row retitling.

- [ ] **Step 1: Write the failing tests**

Add to `crates/waml/src/index_md.rs`'s test module:

```rust
#[test]
fn render_index_emits_declared_frontmatter_before_the_heading() {
    let extra = crate::frontmatter::Frontmatter {
        entries: vec![(
            "generator".into(),
            crate::frontmatter::FmValue::Str("acme".into()),
        )],
    };
    let out = render_index(
        "sales",
        Some("Sales"),
        None,
        &[],
        &IndexFrontmatter {
            profile: Some("uml-domain"),
            view: Some(&crate::okf::ViewSpec::Outline),
            extra: Some(&extra),
        },
    );

    assert!(out.starts_with("---\n"), "frontmatter leads the file: {out}");
    assert!(out.contains("\ntitle: Sales\n"));
    assert!(out.contains("\nprofile: uml-domain\n"));
    assert!(out.contains("\nview: outline\n"));
    assert!(out.contains("\ngenerator: acme\n"));
    assert!(out.contains("\n---\n# Sales\n"));
}

#[test]
fn render_index_without_declarations_is_byte_identical_to_today() {
    let out = render_index("sales", None, Some("Sales bounded context."), &[], &IndexFrontmatter::default());
    assert_eq!(out, "# sales\n\nSales bounded context.\n");
}

#[test]
fn index_frontmatter_survives_parse_render_reparse() {
    let original = "---\ntitle: Sales\nprofile: uml-domain\nview: member:./orders\ngenerator: acme\n---\n# Sales\n\n* [Order](./order.md)\n";
    let source = crate::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", original),
        (
            "sales/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
    ])
    .unwrap();

    let rendered = reindex_source(&source);
    let reparsed = crate::okf::Bundle::parse(&rendered).unwrap();
    let before = crate::okf::Bundle::parse(&source).unwrap();

    let a = before.index("/sales").unwrap();
    let b = reparsed.index("/sales").unwrap();
    assert_eq!(a.title, b.title);
    assert_eq!(a.profile, b.profile);
    assert_eq!(a.view, b.view);
    assert_eq!(a.extra, b.extra);
    assert_eq!(a.members, b.members);

    // And it is stable: a second pass changes nothing.
    let again = reindex_source(&rendered);
    assert_eq!(again.to_pairs(), rendered.to_pairs());
}
```

Also update the existing test at `crates/waml/src/index_md.rs:199`: `assert!(!out.contains("---"))` still holds for that call site because it passes `&IndexFrontmatter::default()` — change its comment to `// no declarations => frontmatter-less`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml index_md`
Expected: FAIL — `cannot find struct IndexFrontmatter`, plus arity errors at the existing `render_index` call sites.

- [ ] **Step 3: Implement**

In `crates/waml/src/index_md.rs`, add above `render_index`:

```rust
/// The frontmatter half of an index document. `Default` declares nothing and
/// renders no block, so a caller with nothing to say emits today's bytes.
#[derive(Default)]
pub struct IndexFrontmatter<'a> {
    pub profile: Option<&'a str>,
    pub view: Option<&'a crate::okf::ViewSpec>,
    pub extra: Option<&'a crate::frontmatter::Frontmatter>,
}
```

Then, at the top of `render_index` (after `heading` is computed), build the block. `title` is emitted into frontmatter ONLY when some other key is present — a bare title stays in the H1, exactly as today, so no existing bundle grows a frontmatter block:

```rust
    let mut entries: Vec<(String, crate::frontmatter::FmValue)> = Vec::new();
    if let Some(profile) = frontmatter.profile.map(str::trim).filter(|p| !p.is_empty()) {
        entries.push((
            "profile".into(),
            crate::frontmatter::FmValue::Str(profile.to_owned()),
        ));
    }
    if let Some(view) = frontmatter.view {
        entries.push((
            "view".into(),
            crate::frontmatter::FmValue::Str(view.to_string()),
        ));
    }
    if let Some(extra) = frontmatter.extra {
        entries.extend(extra.entries.iter().cloned());
    }
    let mut out = String::new();
    if !entries.is_empty() {
        // The title leads the block, matching how `parse_authored_index` reads
        // it; the H1 below stays the human-facing heading either way.
        if let Some(title) = title.map(str::trim).filter(|t| !t.is_empty()) {
            entries.insert(
                0,
                ("title".into(), crate::frontmatter::FmValue::Str(title.to_owned())),
            );
        }
        let block = crate::frontmatter::render_frontmatter(&crate::frontmatter::Frontmatter {
            entries,
        });
        out.push_str("---\n");
        out.push_str(&block);
        out.push_str("\n---\n");
    }
    out.push_str(&format!("# {heading}\n"));
```

(replacing the existing `let mut out = format!("# {heading}\n");`).

In `reindex_source` (line 114), pass the parsed index's declarations through:

```rust
            render_index(
                directory,
                index.title.as_deref(),
                index.description.as_deref(),
                &entries,
                &IndexFrontmatter {
                    profile: index.profile.as_deref(),
                    view: index.view.as_ref(),
                    extra: Some(&index.extra),
                },
            ),
```

There is exactly one other non-test call site: `crates/waml/src/okf/lower.rs:845`, inside `write_package_index`'s `else` branch (the index file does not exist yet). Pass `&IndexFrontmatter::default()` there — that branch has no parsed index to read declarations from, so declaring nothing is correct. Confirm with `rg "render_index\(" crates/` that no third non-test site exists; if one appears, read it before choosing a value rather than defaulting reflexively.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml index_md`
Expected: PASS.

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green. If a snapshot/fixture test elsewhere now sees a frontmatter block, that bundle declared `profile`/`view` — verify the new bytes are correct rather than reverting.

- [ ] **Step 6: Write the failing test for the frontmatter-title retitle**

Task 2 makes `parse_authored_index` resolve the title as `title_from_frontmatter.or(<h1>)` (`crates/waml/src/okf/shell.rs:517`) — **frontmatter wins**. But `Op::PkgRetitle` lowers to `write_package_index` → `update_authored_index` (`crates/waml/src/okf/lower.rs:669-706`), which rewrites the **H1 only**. So on a folder whose `index.md` declares `title:`, a retitle would change the H1 and the parsed title would not move: Task 10's directory-row retitle would appear to do nothing.

Add to the test module in `crates/waml/src/okf/lower.rs` (next to the existing `PkgMove` tests at lines 937/954):

```rust
#[test]
fn pkg_retitle_moves_a_frontmatter_title_not_just_the_h1() {
    let source = crate::source::SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        (
            "sales/index.md",
            "---\ntitle: Sales\nprofile: uml-domain\n---\n# Sales\n\n* [Order](./order.md)\n",
        ),
        (
            "sales/order.md",
            "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
        ),
    ])
    .unwrap();

    let applied = crate::ops::apply_source(
        &source,
        &[crate::ops::Op::PkgRetitle {
            path: "sales".into(),
            title: "Sales Domain".into(),
        }],
    )
    .unwrap();

    let text = applied
        .to_pairs()
        .into_iter()
        .find(|(path, _)| path == "sales/index.md")
        .unwrap()
        .1;
    assert!(text.contains("title: Sales Domain"), "frontmatter title moves: {text}");
    assert!(text.contains("# Sales Domain"), "H1 moves too: {text}");
    assert!(text.contains("profile: uml-domain"), "other keys survive: {text}");

    // The parsed title — which is what the outline shows — actually changed.
    let bundle = crate::okf::Bundle::parse(&applied).unwrap();
    assert_eq!(
        bundle.index("/sales").unwrap().title.as_deref(),
        Some("Sales Domain")
    );
}
```

Run: `cargo test -p waml pkg_retitle_moves_a_frontmatter_title`
Expected: FAIL — the frontmatter still reads `title: Sales`, and the parsed title is unchanged.

- [ ] **Step 7: Make `update_authored_index` update the frontmatter title**

In `update_authored_index` (`crates/waml/src/okf/lower.rs:669`), the `title_override` branch currently edits only the H1 range. Extend it: when the shell's frontmatter already carries a `title` key, also push an edit replacing that key's value range with the new title. The shell (`state.shell(work, index_path, "pkg.index")?`) is already in hand at line 691; use the same frontmatter range information `parse_closed_syntax` produces (`crates/waml/src/frontmatter.rs`) to locate the value span, and render the replacement through the same scalar-quoting helper `render_frontmatter` uses so a title needing quotes stays valid YAML.

Leave the H1 edit exactly as it is — both move together, so a strict OKF consumer reading the H1 and WAML reading the frontmatter never disagree. Do **not** add a `title:` key to an index that does not already have one; a bare-H1 index keeps its shape.

- [ ] **Step 8: Run the test to verify it passes**

Run: `cargo test -p waml pkg_retitle`
Expected: PASS.

- [ ] **Step 9: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 10: Commit**

```bash
git add crates/waml/src/index_md.rs crates/waml/src/okf/lower.rs
git commit -m "feat(okf): emit index frontmatter and retitle through it"
```

---

### Task 4: `ProfileDef` static table

**Files:**
- Create: `crates/waml/src/profile.rs`
- Modify: `crates/waml/src/lib.rs:16` (add `pub mod profile;` between `pub mod ops;` and `pub mod seed;`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/profile.rs`

**Interfaces:**
- Consumes: `ViewSpec` (Task 1).
- Produces:
  - `pub struct ProfileDef { pub name: &'static str, pub default_view: Option<ViewSpec> }`
  - `pub fn profile(name: &str) -> Option<&'static ProfileDef>`

  A static table, no trait. Ships `uml-domain` and `okf`, both `default_view: None` — today's behavior is preserved and Outline is opt-in per folder. The full profile system (legal element types, child templates, validation) stays deferred; when it lands, `profile()` grows a bundle argument and this table becomes the fallback, and call sites do not change. **No profile file format is specified here.**

- [ ] **Step 1: Write the failing test**

Create `crates/waml/src/profile.rs` with only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_profiles_resolve_by_name_and_default_to_no_view() {
        for name in ["uml-domain", "okf"] {
            let def = profile(name).unwrap_or_else(|| panic!("{name} is a shipped profile"));
            assert_eq!(def.name, name);
            assert_eq!(
                def.default_view, None,
                "shipped profiles must not assume outline yet"
            );
        }
    }

    #[test]
    fn unknown_profiles_resolve_to_none() {
        assert!(profile("not-a-profile").is_none());
        assert!(profile("").is_none());
        assert!(profile("UML-Domain").is_none(), "names are exact, not folded");
    }
}
```

Add `pub mod profile;` to `crates/waml/src/lib.rs` after line 15 (`pub mod ops;`).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml profile::tests`
Expected: FAIL — `cannot find function 'profile'`.

- [ ] **Step 3: Write minimal implementation**

Prepend to `crates/waml/src/profile.rs`:

```rust
//! What a directory IS. A profile is a Rust data type, not a file format:
//! a static table with no trait. The full profile system (legal element types,
//! child templates, validation) is deferred; when it lands, `profile()` grows a
//! bundle argument and this table becomes the fallback — call sites do not
//! change.

use crate::okf::ViewSpec;

/// A profile definition. `default_view` is consulted by
/// `Bundle::resolved_view` only when the directory declares no `view:` of
/// its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: &'static str,
    pub default_view: Option<ViewSpec>,
}

/// The shipped profiles. Both default to `None` on purpose: today's behavior
/// is preserved and Outline stays opt-in per folder until real use justifies a
/// profile that assumes it.
static PROFILES: &[ProfileDef] = &[
    ProfileDef {
        name: "uml-domain",
        default_view: None,
    },
    ProfileDef {
        name: "okf",
        default_view: None,
    },
];

/// Look a profile up by its exact declared name.
pub fn profile(name: &str) -> Option<&'static ProfileDef> {
    PROFILES.iter().find(|def| def.name == name)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml profile::tests`
Expected: PASS (2 tests).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/profile.rs crates/waml/src/lib.rs
git commit -m "feat(waml): ProfileDef static table for uml-domain and okf"
```

---

### Task 5: `resolved_profile` and `resolved_view` queries

**Files:**
- Modify: `crates/waml/src/okf.rs` — add two methods to `impl Bundle` after `directories()` (line 313)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/okf.rs:433`

**Interfaces:**
- Consumes: `Index::{profile, view}` (Task 2), `crate::profile::profile` (Task 4), `ViewSpec` (Task 1), `Bundle::index` (`crates/waml/src/okf.rs:283`), `DirectoryAddress::parent`.
- Produces:
  - `pub fn resolved_profile(&self, directory: &str) -> Option<&str>` — nearest declaring ancestor, self first. Walking stops at the first index that declares a `profile`, so an explicit local declaration always beats an inherited one.
  - `pub fn resolved_view(&self, directory: &str) -> ViewSpec` — (1) the index's own `view:`, else (2) `resolved_profile`'s `default_view`, else (3) `ViewSpec::Index`. Step 2 uses the INHERITED profile.

  Both are pure: no editor, no I/O.

- [ ] **Step 1: Write the failing tests**

Add to the test module at `crates/waml/src/okf.rs:433`:

```rust
fn resolution_bundle(pairs: &[(&str, &str)]) -> Bundle {
    let source = crate::source::SourceBundle::try_from_pairs(pairs.iter().copied()).unwrap();
    Bundle::parse(&source).unwrap()
}

#[test]
fn resolved_profile_prefers_self_then_nearest_ancestor_then_none() {
    let bundle = resolution_bundle(&[
        ("index.md", "---\nprofile: okf\n---\n# Root\n\n* [Sales](sales/)\n"),
        (
            "sales/index.md",
            "---\nprofile: uml-domain\n---\n# Sales\n\n* [Orders](orders/)\n",
        ),
        ("sales/orders/index.md", "# Orders\n\n* [Line](./line.md)\n"),
        (
            "sales/orders/line.md",
            "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n",
        ),
    ]);

    assert_eq!(bundle.resolved_profile("/sales"), Some("uml-domain"), "self wins");
    assert_eq!(
        bundle.resolved_profile("/sales/orders"),
        Some("uml-domain"),
        "nearest ancestor wins over the further root"
    );
    assert_eq!(bundle.resolved_profile("/"), Some("okf"));
}

#[test]
fn resolved_profile_is_none_when_nothing_declares() {
    let bundle = resolution_bundle(&[
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "# Sales\n"),
    ]);

    assert_eq!(bundle.resolved_profile("/sales"), None);
    assert_eq!(bundle.resolved_profile("/"), None);
}

#[test]
fn resolved_view_walks_local_then_profile_default_then_index() {
    // Step 1: a local `view:` is used verbatim.
    let local = resolution_bundle(&[("index.md", "---\nview: outline\n---\n# Root\n")]);
    assert_eq!(local.resolved_view("/"), ViewSpec::Outline);

    // Step 3: nothing declared anywhere.
    let bare = resolution_bundle(&[("index.md", "# Root\n")]);
    assert_eq!(bare.resolved_view("/"), ViewSpec::Index);

    // Step 3 again: a shipped profile whose `default_view` is None.
    let profiled = resolution_bundle(&[("index.md", "---\nprofile: uml-domain\n---\n# Root\n")]);
    assert_eq!(profiled.resolved_view("/"), ViewSpec::Index);
}

#[test]
fn resolved_view_step_two_uses_the_inherited_profile_default() {
    // Step 2 has no shipped profile with a non-None `default_view` today, so
    // drive it through the table directly: marking /sales gives /sales/orders
    // the same profile without restating it, which is what step 2 consumes.
    let bundle = resolution_bundle(&[
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "---\nprofile: uml-domain\n---\n# Sales\n\n* [Orders](orders/)\n"),
        ("sales/orders/index.md", "# Orders\n"),
    ]);

    let inherited = bundle.resolved_profile("/sales/orders").unwrap();
    assert_eq!(inherited, "uml-domain");
    assert_eq!(
        crate::profile::profile(inherited).unwrap().default_view,
        None
    );
    assert_eq!(bundle.resolved_view("/sales/orders"), ViewSpec::Index);
}

#[test]
fn an_explicit_local_view_beats_an_inherited_profile_default() {
    let bundle = resolution_bundle(&[
        ("index.md", "---\nprofile: uml-domain\n---\n# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "---\nview: markdown\n---\n# Sales\n"),
    ]);

    assert_eq!(bundle.resolved_profile("/sales"), Some("uml-domain"));
    assert_eq!(bundle.resolved_view("/sales"), ViewSpec::Markdown);
}

#[test]
fn an_unknown_directory_resolves_to_the_index_fallback() {
    let bundle = resolution_bundle(&[("index.md", "# Root\n")]);
    assert_eq!(bundle.resolved_view("/nope"), ViewSpec::Index);
    assert_eq!(bundle.resolved_profile("/nope"), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml resolved_`
Expected: FAIL — `no method named 'resolved_profile' found for struct 'Bundle'`.

- [ ] **Step 3: Implement**

Add to `impl Bundle` in `crates/waml/src/okf.rs`, after `directories()` (line 313):

```rust
    /// The profile in EFFECT for `directory`: the nearest declaring ancestor,
    /// self first. Walking stops at the first index that declares a `profile`,
    /// so a child opts out of a parent's profile by declaring its own.
    pub fn resolved_profile(&self, directory: &str) -> Option<&str> {
        let mut address = DirectoryAddress::parse(directory).ok()?;
        loop {
            if let Some(profile) = self
                .index(address.as_str())
                .and_then(|index| index.profile.as_deref())
            {
                return Some(profile);
            }
            address = address.parent()?;
        }
    }

    /// The view in EFFECT for `directory`. Three steps, no special cases:
    /// the index's own `view:`, else the (inherited) profile's `default_view`,
    /// else `ViewSpec::Index`. There is no auto-detection — a folder holding
    /// exactly one diagram does not silently resolve to `Member`.
    pub fn resolved_view(&self, directory: &str) -> ViewSpec {
        if let Some(view) = self.index(directory).and_then(|index| index.view.clone()) {
            return view;
        }
        if let Some(default) = self
            .resolved_profile(directory)
            .and_then(crate::profile::profile)
            .and_then(|def| def.default_view.clone())
        {
            return default;
        }
        ViewSpec::Index
    }
```

If `DirectoryAddress` exposes no `parse`, use whatever constructor the type already offers (check `crates/waml/src/okf.rs` and `crates/waml/src/okf/*`); the walk only needs `as_str()` and `parent()`. If no constructor is public, walk the string instead: strip the trailing `/<segment>` and stop after `"/"`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml resolved_`
Expected: PASS (6 tests).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/okf.rs
git commit -m "feat(okf): resolved_profile and resolved_view queries on Bundle"
```

---

### Task 6: Folder rows — the pure model behind the view

**Files:**
- Create: `crates/waml-editor/src/folder_rows.rs`
- Modify: `crates/waml-editor/src/main.rs` (add `mod folder_rows;` alphabetically, near `mod fonts;` at line 32)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml-editor/src/folder_rows.rs`

**Interfaces:**
- Consumes: `waml::okf::Bundle::{index, directory, concept}`, `Index::members` (unchanged: flat, link-backed ids).
- Produces:

  ```rust
  pub enum FolderRowTarget {
      Directory { address: String },
      Concept { concept_id: String },
  }

  pub struct FolderRow {
      pub title: String,
      pub blurb: Option<String>,
      pub target: FolderRowTarget,
  }

  pub fn folder_rows(bundle: &waml::okf::Bundle, directory: &str) -> Vec<FolderRow>;
  ```

  Rows come from `okf::Directory` (`child_directories` + `concepts`), ordered by the index's authored member order, with unlisted items appended. Each row carries a title and an optional blurb taken from the concept's frontmatter `description`. Pure Rust, no widget: this is the piece Tasks 7 and 9 both draw from.

- [ ] **Step 1: Write the failing tests**

Create `crates/waml-editor/src/folder_rows.rs` with only the tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use waml::source::SourceBundle;

    fn bundle(pairs: &[(&str, &str)]) -> waml::okf::Bundle {
        let source = SourceBundle::try_from_pairs(pairs.iter().copied()).unwrap();
        waml::okf::Bundle::parse(&source).unwrap()
    }

    #[test]
    fn rows_follow_authored_member_order_and_carry_blurbs() {
        let bundle = bundle(&[
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "# Sales\n\n* [Order](./order.md)\n* [Orders](orders/)\n",
            ),
            (
                "sales/order.md",
                "---\ntype: uml.Class\ntitle: Order\ndescription: A placed order.\n---\n# Order\n",
            ),
            (
                "sales/orders/line.md",
                "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n",
            ),
        ]);

        let rows = folder_rows(&bundle, "/sales");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "Order");
        assert_eq!(rows[0].blurb.as_deref(), Some("A placed order."));
        assert_eq!(
            rows[0].target,
            FolderRowTarget::Concept {
                concept_id: "sales/order".into()
            }
        );
        assert_eq!(rows[1].title, "orders");
        assert_eq!(rows[1].blurb, None);
        assert_eq!(
            rows[1].target,
            FolderRowTarget::Directory {
                address: "/sales/orders".into()
            }
        );
    }

    #[test]
    fn unlisted_members_are_appended_not_dropped() {
        let bundle = bundle(&[
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "# Sales\n\n* [Zebra](./zebra.md)\n"),
            (
                "sales/zebra.md",
                "---\ntype: uml.Class\ntitle: Zebra\n---\n# Zebra\n",
            ),
            (
                "sales/apple.md",
                "---\ntype: uml.Class\ntitle: Apple\n---\n# Apple\n",
            ),
        ]);

        let titles: Vec<_> = folder_rows(&bundle, "/sales")
            .into_iter()
            .map(|row| row.title)
            .collect();

        assert_eq!(titles, vec!["Zebra".to_string(), "Apple".to_string()]);
    }

    #[test]
    fn an_unknown_directory_has_no_rows() {
        let bundle = bundle(&[("index.md", "# Root\n")]);
        assert!(folder_rows(&bundle, "/nope").is_empty());
    }
}
```

Add `mod folder_rows;` to `crates/waml-editor/src/main.rs` (alphabetical position, next to `mod fonts;` at line 32).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor folder_rows`
Expected: FAIL — `cannot find function 'folder_rows'`.

- [ ] **Step 3: Implement**

Prepend to `crates/waml-editor/src/folder_rows.rs`:

```rust
//! The rows a folder view shows, derived purely from the OKF model.
//!
//! `Index` and `Outline` are one widget in two modes, so both draw exactly
//! these rows; the mode only decides whether they are editable.

/// What a row opens when it is clicked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FolderRowTarget {
    Directory { address: String },
    Concept { concept_id: String },
}

/// One member row: a bullet, a title, and an optional blurb.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderRow {
    pub title: String,
    pub blurb: Option<String>,
    pub target: FolderRowTarget,
}

/// Rows for `directory`, in the index's authored member order.
///
/// `Index::members` is already the authored order with unlisted items appended
/// (see `crates/waml/src/okf/shell.rs:271-296`), so this walks it directly.
/// A member id starting with `/` is a child directory; anything else is a
/// concept id.
pub fn folder_rows(bundle: &waml::okf::Bundle, directory: &str) -> Vec<FolderRow> {
    let Some(index) = bundle.index(directory) else {
        return Vec::new();
    };
    index
        .members
        .iter()
        .filter_map(|member| {
            if member.starts_with('/') {
                let child = bundle.index(member);
                let fallback = member.rsplit('/').next().unwrap_or(member).to_string();
                Some(FolderRow {
                    title: child
                        .and_then(|child| child.title.clone())
                        .unwrap_or(fallback),
                    blurb: None,
                    target: FolderRowTarget::Directory {
                        address: member.clone(),
                    },
                })
            } else {
                let concept = bundle.concept(member)?;
                Some(FolderRow {
                    title: concept
                        .title
                        .clone()
                        .unwrap_or_else(|| member.rsplit('/').next().unwrap_or(member).to_string()),
                    blurb: concept
                        .description
                        .as_ref()
                        .map(|d| d.lines().next().unwrap_or("").trim().to_string())
                        .filter(|d| !d.is_empty()),
                    target: FolderRowTarget::Concept {
                        concept_id: member.clone(),
                    },
                })
            }
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-editor folder_rows`
Expected: PASS (3 tests).

- [ ] **Step 5: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/folder_rows.rs crates/waml-editor/src/main.rs
git commit -m "feat(editor): folder_rows model for the folder index view"
```

---

### Task 7: `FolderIndex` widget and the folder document provider

**Files:**
- Create: `crates/waml-editor/src/folder_index.rs` (the `FolderIndex` widget)
- Create: `crates/waml-editor/src/folder_documents.rs` (the provider) and `crates/waml-editor/src/folder_view.rs` (the `DocView`)
- Modify: `crates/waml-editor/src/main.rs` (three `mod` lines)
- Modify: `crates/waml-editor/src/app.rs:33-63` (`script_mod!` import), `:704-787` (boot registration), `:342-350` region (new `folder_surface` sibling)
- Modify: `crates/waml-editor/src/doc_view.rs:25-42` + `356-363` (`BodyWidgets` handle, `show_folder`, `DocViewIdentity::Folder`)
- Test: inline tests in `crates/waml-editor/src/folder_documents.rs`, plus a script-gate test in `crates/waml-editor/src/folder_index.rs`

**Interfaces:**
- Consumes: `folder_rows` / `FolderRow` / `FolderRowTarget` (Task 6), `Bundle::resolved_view` (Task 5), `ViewSpec` (Task 1), the provider shape at `crates/waml-editor/src/okf_documents.rs:14-71`, `trait DocView` (`crates/waml-editor/src/doc_view.rs:372`).
- Produces:
  - widget `FolderIndex` (makepad) with `pub fn set_rows(&mut self, cx: &mut Cx, rows: &[FolderRow])` and `pub fn set_editable(&mut self, cx: &mut Cx, editable: bool)` — **one widget, two modes**; `editable` is wired but inert until Task 9.
  - `pub enum FolderIndexAction { RowClicked(FolderRowTarget) }`
  - `pub fn folder_tab_id(address: &str) -> LiveId` (`__doc_tab_folder__{address}`)
  - `pub fn open_folder(okf: &waml::analysis::OkfAnalysis, address: &str) -> Option<OpenDocument>`
  - `pub struct FolderView` implementing `DocView` with `DocViewIdentity::Folder`

  A folder tab's `OpenDocument.concept_id` is the **directory address** (`/sales`). Directory addresses always start with `/` and concept ids never do, so the two identifier spaces cannot collide in `DocumentLocator`. `kind` is `DocumentKind::Primary`; `presentation.category` is the already-existing `NavCategory::Directory` (`crates/waml-editor/src/document.rs:21`), with a folder icon distinct from file tabs (`Icon::Folder` if present in `crates/waml-editor/src/icons.rs`, otherwise the nearest folder glyph the catalog already ships — do not add an icon).

  **Registration is mandatory and silent when missed:** `FolderIndex` must appear BY NAME in `script_mod!` at `crates/waml-editor/src/app.rs` (after `use mod.widgets.MarkdownEditor` on line 63) and get `crate::folder_index::script_mod(vm);` in the boot list, placed BEFORE any consumer's registration. An unregistered widget draws nothing and the gate still passes — hence the script-gate test in Step 1.

- [ ] **Step 1: Write the failing tests**

In `crates/waml-editor/src/folder_index.rs`, add a script-gate test proving the widget is really registered and reachable (mirroring `crates/waml-editor/src/tree_panel.rs:1511-1529`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_index_is_registered_and_instantiable_from_the_dsl() {
        let mut vm = crate::script_gate::boot_test_vm();
        crate::theme_atlas::script_mod(&mut vm);
        crate::fonts::script_mod(&mut vm);
        crate::icons::script_mod(&mut vm);
        crate::folder_index::script_mod(&mut vm);

        let widget = script_eval!(vm, { mod.widgets.FolderIndex {} });
        assert!(
            widget.as_object().is_some(),
            "FolderIndex must instantiate from the DSL; an unregistered widget is silently dropped"
        );
    }
}
```

In `crates/waml-editor/src/folder_documents.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::NavCategory;
    use waml::source::SourceBundle;

    fn prepared(pairs: &[(&str, &str)]) -> waml::analysis::PreparedCandidate {
        let source = SourceBundle::try_from_pairs(pairs.iter().copied()).unwrap();
        waml::analysis::prepare_candidate(source, None, 1).unwrap()
    }

    #[test]
    fn a_directory_opens_as_a_folder_tab_titled_by_its_index() {
        let prepared = prepared(&[
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "---\ntitle: Sales\n---\n# Sales\n"),
            (
                "sales/order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
        ]);

        let document = open_folder(prepared.okf(), "/sales").unwrap();

        assert_eq!(document.tab_id, folder_tab_id("/sales"));
        assert_eq!(document.concept_id, "/sales");
        assert_eq!(document.title, "Sales");
        assert_eq!(document.presentation.category, NavCategory::Directory);
    }

    #[test]
    fn folder_tab_identity_is_stable_and_distinct_from_a_same_named_concept() {
        assert_eq!(folder_tab_id("/sales"), folder_tab_id("/sales"));
        assert_ne!(
            folder_tab_id("/sales"),
            crate::okf_documents::okf_document_tab_id("sales")
        );
    }

    #[test]
    fn an_unknown_directory_does_not_open() {
        let prepared = prepared(&[("index.md", "# Root\n")]);
        assert!(open_folder(prepared.okf(), "/nope").is_none());
    }

    #[test]
    fn a_markdown_view_folder_opens_its_index_md_instead_of_the_folder_surface() {
        let prepared = prepared(&[
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            ("sales/index.md", "---\nview: markdown\n---\n# Sales\n"),
        ]);

        assert_eq!(
            prepared.okf().bundle.resolved_view("/sales"),
            waml::okf::ViewSpec::Markdown
        );
        assert!(
            open_folder(prepared.okf(), "/sales").is_none(),
            "Markdown folders are routed to the markdown editor, not the folder surface"
        );
    }
}
```

(If `waml::analysis::PreparedCandidate` is spelled differently, copy the exact idiom from `crates/waml-editor/src/okf_documents.rs:136-154`.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor folder_`
Expected: FAIL — unresolved modules `folder_index` / `folder_documents`.

- [ ] **Step 3: Build the widget**

Create `crates/waml-editor/src/folder_index.rs`. Follow `crates/waml-editor/src/diagram_properties.rs:12-60` exactly for the `script_mod!` shape — ONE object literal for the namespace, no field-by-field assignment, no inline `font_size` (use `fonts.text_label` / `fonts.text_body` from `crates/waml-editor/src/fonts.rs`):

```rust
use makepad_widgets::*;

use crate::folder_rows::{FolderRow, FolderRowTarget};

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.atlas
    use mod.widgets.*
    use mod.text.*
    use mod.fonts

    mod.widgets.FolderIndexBase = #(FolderIndex::register_widget(vm))
    mod.widgets.FolderIndex = set_type_default() do mod.widgets.FolderIndexBase{
        width: Fill
        height: Fill
        flow: Down
        show_bg: true
        draw_bg +: {
            color: atlas.surface
            pixel: fn() {
                return vec4(self.color.rgb * self.color.a, self.color.a)
            }
        }
        heading := Label {
            text: ""
            draw_text +: {
                color: atlas.text
                text_style: fonts.text_label
            }
        }
        rows := View { width: Fill height: Fill flow: Down }
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum FolderIndexAction {
    RowClicked(FolderRowTarget),
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct FolderIndex {
    #[deref]
    view: View,
    #[rust]
    rows: Vec<FolderRow>,
    #[rust]
    editable: bool,
}

impl FolderIndex {
    /// Replace the member rows. Both modes draw exactly these.
    pub fn set_rows(&mut self, cx: &mut Cx, rows: &[FolderRow]) {
        self.rows = rows.to_vec();
        self.redraw(cx);
    }

    /// `false` = `ViewSpec::Index` (read-only), `true` = `ViewSpec::Outline`.
    /// Wired now, inert until Task 9: building the two modes as two widgets
    /// would produce two layouts that drift.
    pub fn set_editable(&mut self, cx: &mut Cx, editable: bool) {
        self.editable = editable;
        self.redraw(cx);
    }
}
```

Draw the rows in `draw_walk` by hand — a bullet, the title, and the blurb — following `draw_nodes` in `crates/waml-editor/src/tree_panel.rs:684-780` for the hand-drawn-row idiom (`draw_abs`, pixel rounding, `cx.turtle().pos()`). Emit `FolderIndexAction::RowClicked(target)` from `handle_event` on a `Hit::FingerDown` inside a row rect. **Drive `View::draw_walk` to `done` in a `while ... .step()` loop** — a one-shot call leaves the turtle begun-never-ended and silently blanks the surrounding chrome.

- [ ] **Step 4: Register the widget**

1. `crates/waml-editor/src/main.rs`: add `mod folder_documents;`, `mod folder_index;`, `mod folder_view;` in alphabetical position (after `mod fonts;`… keep the file's existing ordering convention).
2. `crates/waml-editor/src/app.rs`, `script_mod!` block: add `use mod.widgets.FolderIndex` after line 63 (`use mod.widgets.MarkdownEditor`).
3. `crates/waml-editor/src/app.rs` boot list (lines 704-787): add `crate::folder_index::script_mod(vm);` alongside the other body widgets, BEFORE `crate::doc_tabs::script_mod(vm);` (line 762) so no consumer registers first.
4. `crates/waml-editor/src/app.rs`, next to `diagram_properties_wrap` (line 342): add the surface sibling —

```
                                folder_surface := View {
                                    width: Fill
                                    height: Fill
                                    visible: false
                                    folder_index := FolderIndex {
                                        width: Fill
                                        height: Fill
                                    }
                                }
```

5. `crates/waml-editor/src/doc_view.rs`: add `folder_index: WidgetRef` to `BodyWidgets` (line 25), populate it in `BodyWidgets::new` with `ui.widget(_cx, ids!(folder_surface.folder_index))`, and add

```rust
    /// Swap the shared center surface to the folder view, hiding the canvas,
    /// behavior canvas, diagram properties, and markdown surfaces.
    pub fn show_folder(&self, cx: &mut Cx) {
        self.ui.widget(cx, ids!(canvas_wrap)).set_visible(cx, false);
        self.set_canvas_interaction_enabled(cx, false);
        self.ui
            .widget(cx, ids!(behavior_canvas_wrap))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(diagram_properties_wrap))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(markdown_surface))
            .set_visible(cx, false);
        self.ui
            .widget(cx, ids!(folder_surface))
            .set_visible(cx, true);
    }

    pub fn folder_index(&self) -> &WidgetRef {
        &self.folder_index
    }
```

  Every OTHER surface-showing method (`show_markdown_editor`, `set_diagram_properties_visible`, `set_behavior_canvas_visible`) must also hide `folder_surface`, or a folder tab's rows will bleed over the next tab.

6. `crates/waml-editor/src/doc_view.rs:356`: add `Folder` to `enum DocViewIdentity`.

- [ ] **Step 5: Write the provider and the `DocView`**

`crates/waml-editor/src/folder_documents.rs`:

```rust
use crate::document::{DocumentPresentation, NavCategory, OpenDocument};
use crate::icons::Icon;
use crate::view_history::DocumentKind;
use makepad_widgets::LiveId;

/// Stable tab identity for a folder. Directory addresses lead with `/` and
/// concept ids never do, so this can never collide with a document tab.
pub fn folder_tab_id(address: &str) -> LiveId {
    LiveId::from_str(&format!("__doc_tab_folder__{address}"))
}

/// Open `address` as a folder tab, when its resolved view is one this surface
/// renders (`Index` or `Outline`). `Markdown` and `Member` are routed by the
/// caller to the markdown editor and to the member's own view respectively.
pub fn open_folder(
    analysis: &waml::analysis::OkfAnalysis,
    address: &str,
) -> Option<OpenDocument> {
    let index = analysis.bundle.index(address)?;
    let view = analysis.bundle.resolved_view(address);
    let editable = match view {
        waml::okf::ViewSpec::Index => false,
        waml::okf::ViewSpec::Outline => true,
        waml::okf::ViewSpec::Markdown | waml::okf::ViewSpec::Member(_) => return None,
    };
    let fallback = address.rsplit('/').next().unwrap_or(address);
    let fallback = if fallback.is_empty() { "/" } else { fallback };
    Some(OpenDocument {
        tab_id: folder_tab_id(address),
        concept_id: address.to_string(),
        kind: DocumentKind::Primary,
        title: index.title.clone().unwrap_or_else(|| fallback.to_string()),
        presentation: DocumentPresentation {
            icon: Icon::Folder,
            accent: None,
            category: NavCategory::Directory,
        },
        view: Box::new(crate::folder_view::FolderView::new(
            address.to_string(),
            editable,
        )),
    })
}
```

If `Icon::Folder` is not in `crates/waml-editor/src/icons.rs`, use the nearest existing folder glyph the catalog already ships — do not add a new icon in this task.

`crates/waml-editor/src/folder_view.rs`: a `FolderView { address: String, editable: bool }` implementing `DocView`:
- `identity()` → `DocViewIdentity::Folder`
- `sync()` → `body.show_folder(cx)`, then borrow `body.folder_index()` as `FolderIndex` and call `set_editable(cx, self.editable)` and `set_rows(cx, &crate::folder_rows::folder_rows(bundle, &self.address))` (the bundle comes off `ViewData`; copy how `crates/waml-editor/src/class_diagram_view.rs` reads it)
- `handle()` → map `FolderIndexAction::RowClicked(target)` to a `ViewOutcome` navigation: `FolderRowTarget::Concept { concept_id }` → `NavigationTarget::Document { concept_id, fragment: None }`; `FolderRowTarget::Directory { address }` → `NavigationTarget::Directory { address }` (which Task 8 makes open the child's own resolved view)
- `chrome()` → `BodyChrome { tool_dock: false, view_bar: false, canvas_overlays: false, document_header: DocumentHeaderChrome { breadcrumb: true, right_dock: None } }`

Register `open_folder` in the provider chain: in `crates/waml-editor/src/documents.rs`, `open_locator_with_asset_host` (line 41) must try `crate::folder_documents::open_folder(okf, &locator.concept_id)` FIRST when `locator.concept_id.starts_with('/')`, so a reopened folder tab resolves.

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p waml-editor folder_`
Expected: PASS.

- [ ] **Step 7: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 8: Visual verification (mandatory — the gate cannot see a dropped widget)**

Run the editor on a fixture with a `sales/` directory (`./run.ps1`, see `crates/waml-editor/src/bin` presets), click a folder in the tree, and confirm the folder surface draws titled rows with blurbs. A blank surface with a green gate means the widget was not registered — recheck Step 4.

- [ ] **Step 9: Commit**

```bash
git add crates/waml-editor/src/folder_index.rs crates/waml-editor/src/folder_documents.rs \
        crates/waml-editor/src/folder_view.rs crates/waml-editor/src/main.rs \
        crates/waml-editor/src/app.rs crates/waml-editor/src/doc_view.rs \
        crates/waml-editor/src/documents.rs
git commit -m "feat(editor): FolderIndex widget and folder document provider"
```

---

### Task 8: Tree row opens the folder; chevron folds it

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:320-349` (`row_navigation`), `:798-824` (hit handling), `:618-642` (`draw_row_chevron` — record the rect)
- Modify: `crates/waml-editor/src/app/navigation.rs:290-300` (the `Directory` handler)
- Modify: `crates/waml-editor/src/tree_panel.rs:1565` (the existing folder-click test)
- Test: inline tests in `crates/waml-editor/src/tree_panel.rs` and `crates/waml-editor/src/app/tests/navigation.rs`

**Interfaces:**
- Consumes: `open_folder`, `folder_tab_id` (Task 7), `Bundle::resolved_view` (Task 5).
- Produces:
  - `ProjectTreeAction::ToggleFold(String)` — emitted only from a chevron hit.
  - `NavigationTarget::Directory` now OPENS the folder's resolved view as a tab instead of folding.

  Today the whole row folds: `row_navigation` (`crates/waml-editor/src/tree_panel.rs:327`) returns a `Directory` intent for any directory row, and `crates/waml-editor/src/app/navigation.rs:290-300` handles it by calling `toggle_directory`. This task splits the two actions apart. **This is a behavior change to an existing surface and needs its own verification.**

- [ ] **Step 1: Write the failing tests**

Replace the body of `folder_clicked_emits_intent_without_mutation_then_one_command_closes_it` (`crates/waml-editor/src/tree_panel.rs:1565`) with two tests:

```rust
#[test]
fn a_chevron_hit_folds_without_emitting_a_navigation_intent() {
    let (mut cx, mut panel, file_tree) = mounted_project_tree_test_context();
    let tree = ProjectTreeData {
        roots: vec![node("/sales", "Sales", TreeKind::Directory, vec![])],
    };
    panel.set_view(&mut cx, NavView::Browse(tree));
    assert!(file_tree_folder_is_open(&mut cx, &file_tree, "/sales"));

    panel.chevron_clicked(&mut cx, "/sales");

    assert!(!panel.open_directories.contains("/sales"), "chevron folds");
    assert!(!file_tree_folder_is_open(&mut cx, &file_tree, "/sales"));
    assert_eq!(panel.navigation(&cx.new_actions), None, "and opens nothing");
}

#[test]
fn a_row_body_hit_emits_a_directory_intent_without_folding() {
    let (mut cx, mut panel, file_tree) = mounted_project_tree_test_context();
    let tree = ProjectTreeData {
        roots: vec![node("/sales", "Sales", TreeKind::Directory, vec![])],
    };
    panel.set_view(&mut cx, NavView::Browse(tree));

    let actions: ActionsBuf = vec![Box::new(WidgetAction {
        data: None,
        action: Box::new(FileTreeAction::FolderClicked(LiveId::from_str("/sales"))),
        widget_uid: file_tree.widget_uid(),
        group: None,
    })];
    panel.handle_event(&mut cx, &Event::Actions(actions), &mut Scope::empty());

    assert_eq!(
        panel.navigation(&cx.new_actions),
        Some(NavigationIntent::Resolved {
            target: NavigationTarget::Directory {
                address: "/sales".into(),
            },
            disposition: OpenDisposition::Preview,
        })
    );
    assert!(
        panel.open_directories.contains("/sales"),
        "opening a folder must not fold it"
    );
    assert!(file_tree_folder_is_open(&mut cx, &file_tree, "/sales"));
}
```

And in `crates/waml-editor/src/app/tests/navigation.rs` (near the existing `NavigationTarget::Directory` cases at lines 1028/1062/1274/1337):

```rust
#[test]
fn navigating_to_a_directory_opens_a_folder_tab_instead_of_folding() {
    // Uses whatever app-shell harness the neighbouring tests use; mirror the
    // setup of the test at crates/waml-editor/src/app/tests/navigation.rs:1274.
    let mut app = app_with_source(&[
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "---\ntitle: Sales\n---\n# Sales\n\n* [Order](./order.md)\n"),
        ("sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
    ]);

    app.navigate_to(NavigationTarget::Directory {
        address: "/sales".into(),
    });

    let tab = app.documents.active_tab().unwrap();
    assert_eq!(tab.id, crate::folder_documents::folder_tab_id("/sales"));
    assert_eq!(tab.presentation.category, crate::document::NavCategory::Directory);
    assert_eq!(tab.title, "Sales");
}

#[test]
fn a_markdown_view_directory_opens_its_index_md_in_the_markdown_editor() {
    let mut app = app_with_source(&[
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "---\nview: markdown\n---\n# Sales\n"),
    ]);

    app.navigate_to(NavigationTarget::Directory {
        address: "/sales".into(),
    });

    assert_ne!(
        app.documents.active_tab().unwrap().id,
        crate::folder_documents::folder_tab_id("/sales")
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor folder_ chevron directory`
Expected: FAIL — `no method named 'chevron_clicked'`; the navigation tests still see a fold.

- [ ] **Step 3: Split the chevron hit from the row hit**

In `crates/waml-editor/src/tree_panel.rs`:
1. In `draw_row_chevron` (line 622), the chevron rect is already computed (`x`, `y`, `size`). Have `draw_nodes` record `(node.key.clone(), rect)` into a `Vec<(String, Rect)>` on `ProjectTree` (clear it at the start of each `draw_walk`), so `handle_event` can hit-test it.
2. Add:

```rust
    /// Fold/unfold `address` without emitting any navigation. The chevron's
    /// only job; the row body opens the folder instead.
    pub fn chevron_clicked(&mut self, cx: &mut Cx, address: &str) -> bool {
        self.toggle_directory(cx, address)
    }
```

3. In `handle_event` (line 798), on `Hit::FingerDown(fe)` check the recorded chevron rects first; if the position is inside one, call `chevron_clicked` and **do not** fall through to the `folder_clicked` branch (set a `chevron_consumed` flag read at line 808).

`row_navigation` (line 320) is unchanged: a directory row still yields a `Directory` intent. What changes is who handles it.

- [ ] **Step 4: Make `Directory` navigation open a tab**

Replace the handler at `crates/waml-editor/src/app/navigation.rs:290-300`:

```rust
            crate::navigation::NavigationTarget::Directory { address } => {
                // The chevron folds; the row body opens. Resolve what this
                // folder says it is and open that.
                let snapshot = self.session.snapshot();
                match snapshot.okf().bundle.resolved_view(&address) {
                    waml::okf::ViewSpec::Markdown => {
                        // Raw index.md in the markdown editor.
                        self.open_index_source(cx, &address)
                    }
                    waml::okf::ViewSpec::Member(href) => {
                        // Delegate to the member's own view in this tab slot.
                        match crate::navigation::resolve_link(
                            &snapshot.okf().bundle,
                            &address,
                            &href,
                        ) {
                            Some(target) => self.navigate_to_target(cx, target, disposition, browser),
                            None => false,
                        }
                    }
                    _ => {
                        let Some(document) =
                            crate::folder_documents::open_folder(snapshot.okf(), &address)
                        else {
                            return false;
                        };
                        self.documents.transition(
                            cx,
                            &self.ui,
                            &self.session,
                            DocumentCommand::Open {
                                document,
                                persistent: disposition
                                    == crate::navigation::OpenDisposition::Persistent,
                            },
                        );
                        cx.redraw_all();
                        self.set_navigation_message(cx, None);
                        true
                    }
                }
            }
```

Adapt the exact session/snapshot accessors and the `resolve_link` signature to what the surrounding code uses (see `crates/waml-editor/src/navigation.rs:215` and the `Document` arm directly above at line 269). `open_index_source` is a small helper next to it: build a `DocumentLocator` for `<address>/index.md`'s concept path and open it through `crate::okf_documents::open_source_with_asset_host`. If that concept id is not addressable (indexes are excluded from the concept providers — see `crates/waml-editor/src/documents.rs:190-200`), fall back to the folder surface and record it in the plan's open questions rather than inventing a new reserved path.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p waml-editor`
Expected: PASS. Update any other test that assumed a directory click folds.

- [ ] **Step 6: Run the full gate**

Run: `cargo test --workspace`
Then: `cd editors/vscode && npm run build && npm run test && npm run lint`
Expected: all green.

- [ ] **Step 7: Visual verification (mandatory — this changes an existing surface)**

Launch the editor, then confirm by hand:
- clicking the chevron folds/unfolds and opens NOTHING;
- clicking the row body opens a folder tab and leaves the fold state alone;
- the folder tab's icon is visibly distinct from a file tab's.

- [ ] **Step 8: Commit**

```bash
git add crates/waml-editor/src/tree_panel.rs crates/waml-editor/src/app/navigation.rs \
        crates/waml-editor/src/app/tests/navigation.rs
git commit -m "feat(editor): folder row opens its resolved view, chevron only folds"
```

---

### Task 9: Outline mode — Enter creates a concept

**Files:**
- Modify: `crates/waml-editor/src/folder_index.rs` (row focus + `KeyCode::Return` when `editable`)
- Modify: `crates/waml-editor/src/folder_view.rs` (map the action to an op batch)
- Test: inline tests in `crates/waml-editor/src/folder_view.rs`

**Interfaces:**
- Consumes: `FolderIndex::set_editable` (Task 7), `waml::ops::Op::{NodeNew, PkgReorder}` (`crates/waml/src/ops/mod.rs:143,181`).
- Produces: `FolderIndexAction::CreateAfter { index: usize }` and, in `FolderView::handle`, a `ViewOutcome` carrying an `EditIntent` whose op batch is `[Op::NodeNew { .. }, Op::PkgReorder { path, order }]`.

  **Every edit maps to an existing OKF op or a small composite of them. Nothing bypasses the model to write files directly.** `PkgReorder` is an OKF-substrate op (`crates/waml/src/compat.rs` → `crates/waml/src/okf/ops.rs`), so it edits `index.md` in place via `update_authored_index` and leaves the folder's `profile:`/`view:` alone.

> **BLOCKED ON OPEN QUESTION 3 — do not start this task until it is answered.**
> `Op::NodeNew` carries a UML `ElementType` (`crates/waml/src/ops/mod.rs:143`) and its lowering **hard-refuses a non-UML type**: `crates/waml/src/uml/ops.rs:216-218` returns `EditError::at("node.new", "type is not claimed by UML")` unless `crate::uml::recognizes_type(ty)` (`crates/waml/src/uml.rs:37-44`) accepts it, which it does only for `Uml(_)`, `Behavior(_)`, and `Diagram`. `ElementType::Unknown(_)` is rejected. There is today **no op that creates a plain OKF concept**, and Outline is meant to work in a folder with `profile: okf` or no profile at all. See Open question 3 for the options found. Do not resolve it by defaulting to `uml.Class` — that would silently make every outline row a UML class.

- [ ] **Step 1: Write the failing test**

In `crates/waml-editor/src/folder_view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use waml::ops::Op;
    use waml::source::SourceBundle;

    fn source() -> SourceBundle {
        SourceBundle::try_from_pairs([
            ("index.md", "# Root\n\n* [Sales](sales/)\n"),
            (
                "sales/index.md",
                "---\nprofile: uml-domain\nview: outline\n---\n# Sales\n\n* [Order](./order.md)\n",
            ),
            (
                "sales/order.md",
                "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n",
            ),
        ])
        .unwrap()
    }

    #[test]
    fn enter_after_a_row_creates_a_concept_and_places_it_at_that_position() {
        let ops = crate::folder_view::create_after_ops(
            &waml::okf::Bundle::parse(&source()).unwrap(),
            "/sales",
            0,
        )
        .unwrap();

        assert!(
            matches!(&ops[0], Op::NodeNew { dir, .. } if dir == "sales"),
            "new concept lands in the folder: {ops:?}"
        );
        let Op::NodeNew { slug, .. } = &ops[0] else {
            panic!("first op is NodeNew")
        };
        let Op::PkgReorder { path, order } = &ops[1] else {
            panic!("second op is PkgReorder: {ops:?}")
        };
        assert_eq!(path, "sales");
        assert_eq!(order[0], "sales/order");
        assert_eq!(order[1], format!("sales/{slug}"));
    }

    #[test]
    fn creating_a_concept_leaves_the_folders_own_declarations_intact() {
        let bundle = waml::okf::Bundle::parse(&source()).unwrap();
        let ops = crate::folder_view::create_after_ops(&bundle, "/sales", 0).unwrap();

        // `apply_source` (crates/waml/src/ops/mod.rs:225) is SourceBundle in,
        // SourceBundle out. The sibling `apply` (line 219) takes and returns
        // plain `Vec<(String, String)>` pairs — `ops::Bundle` is a type alias
        // for that (line 6), NOT a SourceBundle. Do not call `.to_pairs()` on
        // an `apply` result.
        let applied = waml::ops::apply_source(&source(), &ops).unwrap();
        let reparsed = waml::okf::Bundle::parse(&applied).unwrap();
        let index = reparsed.index("/sales").unwrap();

        assert_eq!(index.profile.as_deref(), Some("uml-domain"));
        assert_eq!(index.view, Some(waml::okf::ViewSpec::Outline));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor folder_view`
Expected: FAIL — `cannot find function 'create_after_ops'`.

- [ ] **Step 3: Implement the op composite**

In `crates/waml-editor/src/folder_view.rs`:

```rust
/// The op batch for "Enter after row `position`": create an untitled concept in
/// this directory, then place it directly after that row in the index's member
/// order. Two existing ops, no direct file writes.
pub fn create_after_ops(
    bundle: &waml::okf::Bundle,
    address: &str,
    position: usize,
) -> Option<Vec<waml::ops::Op>> {
    let index = bundle.index(address)?;
    let dir = address.trim_start_matches('/').to_string();
    let slug = unique_slug(index, &dir);
    let new_id = if dir.is_empty() {
        slug.clone()
    } else {
        format!("{dir}/{slug}")
    };
    let mut order = index.members.clone();
    let at = (position + 1).min(order.len());
    order.insert(at, new_id);
    Some(vec![
        // The creating op — SEE OPEN QUESTION 3. `Op::NodeNew`'s `ty` is a UML
        // `ElementType` and its lowering refuses anything `recognizes_type`
        // does not claim, so this line cannot be written until the seam
        // question is answered. Whatever the answer, it is ONE op here.
        creating_op,
        waml::ops::Op::PkgReorder {
            path: address.trim_start_matches('/').to_string(),
            order,
        },
    ])
}
```

`unique_slug` is a small local helper: `untitled`, `untitled-2`, … until no member id collides.

Note there is **no** existing `Op::NodeNew` call site to copy from in the editor: `rg "Op::NodeNew" crates/` finds it only in `crates/waml/src/ops/mod.rs`'s own tests (lines 826, 844, 862, 1072, 1087). `creating_op` comes from the resolution of Open question 3.

- [ ] **Step 4: Wire the key**

In `crates/waml-editor/src/folder_index.rs`, track a focused row index and, when `self.editable` and `KeyCode::ReturnKey` arrives, emit `FolderIndexAction::CreateAfter { index }`. In `FolderView::handle`, turn that into `create_after_ops` → an `EditIntent` on the `ViewOutcome` (copy the `EditIntent` construction from `crates/waml-editor/src/class_diagram_view.rs`).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p waml-editor folder_view`
Expected: PASS.

- [ ] **Step 6: Run the full gate + visual verification**

Run: `cargo test --workspace`; `cd editors/vscode && npm run build && npm run test && npm run lint`.
Then launch the editor on a folder whose `index.md` declares `view: outline`, press Enter on a row, and confirm a new row appears and `sales/index.md` keeps its frontmatter.

- [ ] **Step 7: Commit**

```bash
git add crates/waml-editor/src/folder_index.rs crates/waml-editor/src/folder_view.rs
git commit -m "feat(editor): outline mode creates a concept on Enter"
```

---

### Task 10: Outline mode — typing a row retitles the concept

**Files:**
- Modify: `crates/waml-editor/src/folder_index.rs` (inline text entry on the focused row when `editable`)
- Modify: `crates/waml-editor/src/folder_view.rs`
- Test: inline tests in `crates/waml-editor/src/folder_view.rs`

**Interfaces:**
- Consumes: `waml::ops::Op::NodeSet` (`crates/waml/src/ops/mod.rs:153`), `Op::PkgRetitle` (`:188`).
- Produces: `FolderIndexAction::Retitle { index: usize, title: String }`, and `pub fn retitle_ops(bundle, address, position, title) -> Option<Vec<Op>>`.

  Retitle sets the H1 and the frontmatter `title`. **It is not a file rename** — the slug and path are untouched.

  Two different ops, because the two row kinds sit on opposite sides of the OKF/UML seam:
  - A **directory** row uses `Op::PkgRetitle`, an OKF-substrate op with no claim gate. It works for any folder — and it moves a frontmatter `title:` only because Task 3 Steps 6–9 made it. **Task 3 must land before this task.**
  - A **concept** row uses `Op::NodeSet`, which lowers through `Op::ClassifierSet` and calls `require_claimed(state, work, id, "node.set")` (`crates/waml/src/uml/ops.rs:238`). A concept whose `type:` is not claimed by UML — the ordinary case in a `profile: okf` folder — **cannot be retitled by this op**. This is the same seam as Open question 3; the concept-row half of this task inherits that answer. The directory-row half does not and can ship regardless.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn retitling_a_concept_row_sets_the_title_and_not_the_filename() {
    let bundle = waml::okf::Bundle::parse(&source()).unwrap();
    let ops = crate::folder_view::retitle_ops(&bundle, "/sales", 0, "Purchase Order").unwrap();

    assert_eq!(ops.len(), 1);
    match &ops[0] {
        waml::ops::Op::NodeSet { slug, title, .. } => {
            assert_eq!(slug, "sales/order", "the slug is untouched");
            assert_eq!(title.as_deref(), Some("Purchase Order"));
        }
        other => panic!("expected NodeSet, got {other:?}"),
    }
}

#[test]
fn retitling_a_child_directory_row_uses_the_package_retitle_op() {
    let source = SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        ("sales/index.md", "---\nview: outline\n---\n# Sales\n\n* [Orders](orders/)\n"),
        ("sales/orders/line.md", "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n"),
    ])
    .unwrap();
    let bundle = waml::okf::Bundle::parse(&source).unwrap();

    let ops = crate::folder_view::retitle_ops(&bundle, "/sales", 0, "Order Lines").unwrap();

    assert!(
        matches!(&ops[0], waml::ops::Op::PkgRetitle { path, title }
            if path == "sales/orders" && title == "Order Lines"),
        "got {ops:?}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor retitl`
Expected: FAIL — `cannot find function 'retitle_ops'`.

- [ ] **Step 3: Implement**

```rust
/// Retitle the row at `position`. A concept row sets the doc's H1 +
/// frontmatter title (`NodeSet`); a directory row sets the child index's title
/// (`PkgRetitle`). Neither renames a file.
pub fn retitle_ops(
    bundle: &waml::okf::Bundle,
    address: &str,
    position: usize,
    title: &str,
) -> Option<Vec<waml::ops::Op>> {
    let row = crate::folder_rows::folder_rows(bundle, address).into_iter().nth(position)?;
    Some(match row.target {
        crate::folder_rows::FolderRowTarget::Concept { concept_id } => {
            vec![waml::ops::Op::NodeSet {
                slug: concept_id,
                title: Some(title.to_string()),
                description: None,
                stereotype: None,
                abstract_: None,
                ty: None,
            }]
        }
        crate::folder_rows::FolderRowTarget::Directory { address } => {
            vec![waml::ops::Op::PkgRetitle {
                path: address.trim_start_matches('/').to_string(),
                title: title.to_string(),
            }]
        }
    })
}
```

Then wire the inline editing in `folder_index.rs`: when `self.editable` and a row is focused, accept `Event::TextInput` into a per-row text buffer and emit `FolderIndexAction::Retitle { index, title }` when the row loses focus or Enter/Escape ends the edit.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-editor retitl`
Expected: PASS.

- [ ] **Step 5: Run the full gate + visual verification**

Run: `cargo test --workspace`; `cd editors/vscode && npm run build && npm run test && npm run lint`. Then type into an outline row in the running editor and confirm the concept's H1 changes and its filename does not.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/folder_index.rs crates/waml-editor/src/folder_view.rs
git commit -m "feat(editor): outline row typing retitles the concept"
```

---

### Task 11: Outline mode — drag a row to reorder

**Files:**
- Modify: `crates/waml-editor/src/folder_index.rs` (drag state on the recorded row rects)
- Modify: `crates/waml-editor/src/folder_view.rs`
- Test: inline tests in `crates/waml-editor/src/folder_view.rs`

**Interfaces:**
- Consumes: `waml::ops::Op::PkgReorder` (`crates/waml/src/ops/mod.rs:181`).
- Produces: `FolderIndexAction::Reorder { from: usize, to: usize }` and `pub fn reorder_ops(bundle, address, from, to) -> Option<Vec<Op>>`.

  Reordering rewrites the index's member order only. **No file is touched.**

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn dragging_a_row_reorders_members_and_moves_no_file() {
    let source = SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        (
            "sales/index.md",
            "---\nview: outline\n---\n# Sales\n\n* [Order](./order.md)\n* [Customer](./customer.md)\n",
        ),
        ("sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
        ("sales/customer.md", "---\ntype: uml.Class\ntitle: Customer\n---\n# Customer\n"),
    ])
    .unwrap();
    let bundle = waml::okf::Bundle::parse(&source).unwrap();

    let ops = crate::folder_view::reorder_ops(&bundle, "/sales", 1, 0).unwrap();

    assert!(
        matches!(&ops[0], waml::ops::Op::PkgReorder { path, order }
            if path == "sales" && order == &vec!["sales/customer".to_string(), "sales/order".to_string()]),
        "got {ops:?}"
    );

    // `apply_source` in, `apply_source` out; `waml::ops::apply` would return
    // plain pairs (`ops::Bundle`), which has no `to_pairs`.
    let applied = waml::ops::apply_source(&source, &ops).unwrap();
    let paths: Vec<_> = applied.to_pairs().into_iter().map(|(p, _)| p).collect();
    assert!(paths.contains(&"sales/order.md".to_string()), "no file moved");
    assert!(paths.contains(&"sales/customer.md".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor reorder`
Expected: FAIL — `cannot find function 'reorder_ops'`.

- [ ] **Step 3: Implement**

```rust
/// Move the row at `from` to `to` within this index's member order. One op,
/// no file touched.
pub fn reorder_ops(
    bundle: &waml::okf::Bundle,
    address: &str,
    from: usize,
    to: usize,
) -> Option<Vec<waml::ops::Op>> {
    let mut order = bundle.index(address)?.members.clone();
    if from >= order.len() || to > order.len() {
        return None;
    }
    let member = order.remove(from);
    order.insert(to.min(order.len()), member);
    Some(vec![waml::ops::Op::PkgReorder {
        path: address.trim_start_matches('/').to_string(),
        order,
    }])
}
```

In `folder_index.rs`, on `Hit::FingerDown` inside a row when `self.editable`, record the row index; on `FingerMove` track the pointer against the recorded row rects; on `FingerUp` emit `FolderIndexAction::Reorder { from, to }` when the target differs.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p waml-editor reorder`
Expected: PASS.

- [ ] **Step 5: Run the full gate + visual verification**

Run: `cargo test --workspace`; `cd editors/vscode && npm run build && npm run test && npm run lint`. Then drag an outline row in the running editor and confirm the order changes and no file path changes.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/folder_index.rs crates/waml-editor/src/folder_view.rs
git commit -m "feat(editor): outline row drag reorders index members"
```

---

### Task 12: Outline mode — Tab / Shift-Tab move a concept between directories

**Files:**
- Modify: `crates/waml-editor/src/folder_index.rs` (`KeyCode::Tab`, with and without shift, when `editable`)
- Modify: `crates/waml-editor/src/folder_view.rs`
- Test: inline tests in `crates/waml-editor/src/folder_view.rs`

**Interfaces:**
- Consumes: `waml::ops::Op::{PkgMove, PkgReorder}` (`crates/waml/src/ops/mod.rs:169,181`).
- Produces: `FolderIndexAction::{Indent, Outdent} { index: usize }` and `pub fn indent_ops` / `pub fn outdent_ops`, each returning `Option<Vec<Op>>`. Both rewrite BOTH affected `index.md` files (the source directory loses the member, the destination gains it) — `PkgMove` plus the destination's `PkgReorder`.

> **OPEN QUESTION — carried forward from the spec, deliberately unresolved.**
> **Tab on a concept with no preceding sibling directory.** Workflowy indents
> under the bullet above; here that would mean *promoting* a concept to a
> directory (`orders.md` → `orders/index.md`). Legal in OKF and reversible, but
> it turns a keystroke into a structural change. The alternative is for Tab to
> refuse unless a real directory precedes.
>
> **This task implements the REFUSE behavior** (`indent_ops` returns `None`)
> because it is the reversible, non-structural option and it does not foreclose
> the other. Do not resolve the question in code comments or docs; leave it open
> and flag it at review. If the decision later goes the other way, only
> `indent_ops` changes.

- [ ] **Step 1: Write the failing tests**

```rust
fn nested_source() -> SourceBundle {
    SourceBundle::try_from_pairs([
        ("index.md", "# Root\n\n* [Sales](sales/)\n"),
        (
            "sales/index.md",
            "---\nview: outline\n---\n# Sales\n\n* [Orders](orders/)\n* [Order](./order.md)\n",
        ),
        ("sales/orders/line.md", "---\ntype: uml.Class\ntitle: Line\n---\n# Line\n"),
        ("sales/order.md", "---\ntype: uml.Class\ntitle: Order\n---\n# Order\n"),
    ])
    .unwrap()
}

#[test]
fn tab_moves_a_concept_into_the_preceding_sibling_directory() {
    let bundle = waml::okf::Bundle::parse(&nested_source()).unwrap();

    // Row 1 is `Order`; row 0 is the `orders/` directory that precedes it.
    let ops = crate::folder_view::indent_ops(&bundle, "/sales", 1).unwrap();

    assert!(
        matches!(&ops[0], waml::ops::Op::PkgMove { slug, to_dir }
            if slug == "sales/order" && to_dir == "sales/orders"),
        "got {ops:?}"
    );

    let applied = waml::ops::apply_source(&nested_source(), &ops).unwrap();
    let reparsed = waml::okf::Bundle::parse(&applied).unwrap();
    // Both index files are consistent afterwards.
    assert!(!reparsed
        .index("/sales")
        .unwrap()
        .members
        .contains(&"sales/order".to_string()));
    assert!(reparsed
        .index("/sales/orders")
        .unwrap()
        .members
        .contains(&"sales/orders/order".to_string()));
}

#[test]
fn tab_refuses_when_no_real_directory_precedes_the_row() {
    // OPEN QUESTION: promoting a concept to a directory is the alternative
    // behavior; this asserts the refuse option that ships today.
    let bundle = waml::okf::Bundle::parse(&nested_source()).unwrap();
    assert!(crate::folder_view::indent_ops(&bundle, "/sales", 0).is_none());
}

#[test]
fn shift_tab_moves_a_concept_out_to_the_parent_directory() {
    let bundle = waml::okf::Bundle::parse(&nested_source()).unwrap();

    let ops = crate::folder_view::outdent_ops(&bundle, "/sales/orders", 0).unwrap();

    assert!(
        matches!(&ops[0], waml::ops::Op::PkgMove { slug, to_dir }
            if slug == "sales/orders/line" && to_dir == "sales"),
        "got {ops:?}"
    );
}

#[test]
fn shift_tab_refuses_at_the_root() {
    let bundle = waml::okf::Bundle::parse(&nested_source()).unwrap();
    assert!(crate::folder_view::outdent_ops(&bundle, "/", 0).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p waml-editor indent`
Expected: FAIL — `cannot find function 'indent_ops'`.

- [ ] **Step 3: Implement**

```rust
/// Tab: move the concept at `position` into the directory row that immediately
/// precedes it. Rewrites both index files (`PkgMove` moves the file and
/// updates both listings; the trailing `PkgReorder` pins the destination
/// position).
///
/// Returns `None` when the row is not a concept, or when no real directory
/// precedes it. Promoting a concept to a directory in that case is an OPEN
/// QUESTION and is deliberately not implemented.
pub fn indent_ops(
    bundle: &waml::okf::Bundle,
    address: &str,
    position: usize,
) -> Option<Vec<waml::ops::Op>> {
    let rows = crate::folder_rows::folder_rows(bundle, address);
    let crate::folder_rows::FolderRowTarget::Concept { concept_id } =
        rows.get(position)?.target.clone()
    else {
        return None;
    };
    let crate::folder_rows::FolderRowTarget::Directory { address: into } =
        rows.get(position.checked_sub(1)?)?.target.clone()
    else {
        return None;
    };
    Some(vec![waml::ops::Op::PkgMove {
        slug: concept_id,
        to_dir: into.trim_start_matches('/').to_string(),
    }])
}

/// Shift-Tab: move the concept at `position` out to the parent directory.
/// `None` at the bundle root, or when the row is not a concept.
pub fn outdent_ops(
    bundle: &waml::okf::Bundle,
    address: &str,
    position: usize,
) -> Option<Vec<waml::ops::Op>> {
    if address == "/" {
        return None;
    }
    let rows = crate::folder_rows::folder_rows(bundle, address);
    let crate::folder_rows::FolderRowTarget::Concept { concept_id } =
        rows.get(position)?.target.clone()
    else {
        return None;
    };
    let parent = address.rsplit_once('/').map(|(head, _)| head).unwrap_or("");
    Some(vec![waml::ops::Op::PkgMove {
        slug: concept_id,
        to_dir: parent.trim_start_matches('/').to_string(),
    }])
}
```

If `Op::PkgMove` alone does not leave the destination index listing the member (check its implementation in `crates/waml/src/ops/mod.rs`), append a `PkgReorder` for the destination directory built the same way as in Task 9.

Wire the keys in `folder_index.rs`: when `self.editable` and `KeyCode::Tab` arrives, emit `Indent` or `Outdent` depending on `modifiers.shift`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml-editor indent`
Expected: PASS (5 tests).

- [ ] **Step 5: Run the full gate + visual verification**

Run: `cargo test --workspace`; `cd editors/vscode && npm run build && npm run test && npm run lint`. Then, in the running editor, Tab a concept into a preceding folder and Shift-Tab it back out; confirm both `index.md` files are consistent and neither loses its frontmatter.

- [ ] **Step 6: Commit**

```bash
git add crates/waml-editor/src/folder_index.rs crates/waml-editor/src/folder_view.rs
git commit -m "feat(editor): outline Tab and Shift-Tab move a concept between directories"
```

---

## Open questions

1. **Tab on a concept with no preceding sibling directory** (Task 12 only). Promote
   `orders.md` to `orders/index.md`, or refuse? Task 12 ships REFUSE as the
   reversible option; the spec leaves this undecided and this plan does not
   resolve it. It affects step 4 only and blocks nothing in Tasks 1–11.
2. **`ViewSpec::Markdown` routing** (surfaced in Task 8, Step 4). An `index.md`
   is not addressable as a concept (`crates/waml-editor/src/documents.rs:190-200`
   excludes it), so opening it in the markdown editor may need a locator form
   that does not exist yet. If it does, fall back to the folder surface and
   raise it rather than adding a reserved path or a new locator kind unasked.
3. **Creating and retitling a plain OKF concept — the OKF-substrate / UML-profile
   seam.** Blocks Task 9 outright, and the concept-row half of Task 10. This is a
   design question, not an implementation detail; it must be answered before
   those tasks start.

   **What is actually there.** `Op::NodeNew` (`crates/waml/src/ops/mod.rs:143`)
   takes `ty: ElementType`. `ElementType` (`crates/waml/src/model.rs:772-777`) is
   `Uml(UmlMetaclass) | Behavior(BehaviorKind) | Diagram | Unknown(String)`, and
   `Unknown` **does** round-trip cleanly: `ElementType::parse` returns it for any
   unrecognized string and `as_str` returns that string verbatim
   (`crates/waml/src/model.rs:794-816`). OKF's own `Concept.ty`
   (`crates/waml/src/okf.rs:185`) is explicitly "the free-text `type` frontmatter
   field (NOT the UML `ElementType`)", so the substrate has no objection to an
   arbitrary type.

   **But the op refuses it.** `Op::NodeNew` lowers through `Op::ClassifierNew`,
   which returns `EditError::at("node.new", "type is not claimed by UML")` unless
   `crate::uml::recognizes_type(ty)` accepts it
   (`crates/waml/src/uml/ops.rs:216-218`), and that function accepts only
   `Uml(_)`, `Behavior(_)`, and `Diagram` — never `Unknown`
   (`crates/waml/src/uml.rs:37-44`). `Op::NodeSet` is gated the same way via
   `require_claimed` (`crates/waml/src/uml/ops.rs:238`). So today there is **no
   op that creates or retitles a plain OKF concept**; every concept-creating path
   in the codebase goes through the UML profile.

   Options found, none chosen:
   - **(a)** Relax `recognizes_type` / the `ClassifierNew` guard to admit
     `ElementType::Unknown`. Smallest diff, but it widens what the UML profile
     claims, which is the opposite of what the guard exists for.
   - **(b)** Add an OKF-substrate `concept.new` / `concept.set` op pair beside
     the existing `PkgMove`/`PkgReorder`/`PkgRetitle` OKF ops
     (`crates/waml/src/okf/ops.rs`, `crates/waml/src/okf/lower.rs`), taking a
     free-text `type` string. Honest to the layering — an OKF concept is a
     substrate object — but it is new op surface and a new lowering path.
   - **(c)** Make Outline UML-only for now: a folder whose resolved profile is
     not `uml-domain` gets the read-only `Index` view and Enter/typing are inert.
     Ships Tasks 9–12 for the `uml-domain` case with no model change, at the cost
     of the `profile: okf` case the spec's outliner goal implies.

   The plan does not pick one. Note that Tasks 11 and 12 are unaffected either
   way: `PkgReorder` and `PkgMove` are OKF-substrate ops with no claim gate.

## Deliberately out of scope

Named by the spec as excluded, and not to be added by any task: nested index
lists; plain-text (non-link) index bullets; a profile FILE format; the full
profile system (legal element types, child templates, validation); new reserved
filenames or sidecar files; any edit to `docs/specs/OKF_SPEC.md`;
auto-detection of `ViewSpec::Member` from directory contents.
