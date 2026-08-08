# Granular Projection Mask Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the editor's binary `folder_projection::ViewMode` (Projected/Raw) with a
`ProjectionMask` — a session-only set of disabled middleware names, toggled per extension and
per stage from a popup on the tree panel's toolbar.

**Architecture:** The mask lives in the `waml` crate (`waml::view::mask::ProjectionMask`)
because the CLI and the vscode server run the same chain path. `Chain::build` takes it and
**silently skips** a masked stage — never unregisters it, because an unknown name collapses
the whole chain. `MiddlewareRegistry` regains extension ownership so the editor builds the
layered popup from the registry rather than a second hand-written list. `ViewMode` is deleted:
full raw becomes "every maskable name masked", so one value describes what is running.

**Tech Stack:** Rust (`waml`, `waml-editor`), makepad widgets (fork at `C:\dev\makepad`,
branch `waml`), the editor's own `IconButton` / `MenuPopup` / `PopupRoot` surfaces, lucide
SVGs vendored through `scripts/gen-icon.py`.

**Source spec:** `docs/superpowers/specs/2026-08-08-granular-projection-mask-design.md`.
Read it — every decision and rejected alternative is recorded there.

## Global Constraints

- **The spec's glyph table is STALE.** It names `SquareLibrary` / `SquareSplitHorizontal` /
  `SquareCode`. Commit `8628098a` already landed the two end states as bare `Icon::Library`
  and `Icon::Code` (the `Square*` pair moved to `UNWIRED_BUT_LISTED`). This plan therefore
  uses `Library` (empty mask) / `LibraryBig` (partial) / `Code` (fully masked). Do not
  re-vendor `square-split-horizontal`.
- **Disable is a skip, never a de-registration.** `crates/waml/src/view/chain.rs:230-244`
  returns `Chain::root_only` plus an `UnknownViewMiddleware` diagnostic for an unknown name.
  Unregistering a masked name would collapse the whole chain and fake an author error.
- **`index` is never maskable.** `RootView` is the terminal stage every chain falls to;
  masking it cannot remove the listing. It is omitted from the popup.
- **`markdown` and `member` are resolutions, not stages.** They are handled inline in
  `Chain::build` before the registry lookup and are untouched by the mask.
- **Session-only.** Nothing persisted. No `.waml/settings.json` entry, no gate, no developer
  mode. Every launch starts with an empty mask.
- **Gate for every task** (all must be green before the commit):
  ```
  cargo test --workspace
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  ```
  Plus, for any task touching `editors/vscode`: its own `npm run build && npm test`. No task
  in this plan touches `editors/vscode`.
- **Known pre-existing red:** the `waml-syntax` properties proptest reparse failure is
  unrelated to this work. Never commit `crates/waml-syntax/proptest-regressions/`.
- **`main` is a SHARED checkout.** Another session commits to it concurrently. Rebase before
  pushing; never force-push.
- **No Claude co-author trailer in commit messages.** Subject + body only.
- **Icon catalog has SIX ordered sites** that must stay in lockstep — `enum` == field == DSL
  == `get` == `ALL` == label — plus `ALL`'s length constant and the label-count assertion,
  plus `icons_overlay` groups. See Task 7.
- **Visual verification is NOT part of this plan.** An automated implementer cannot do it.
  Every visual check is listed in "Deferred visual checks" at the end and is owed to the user
  after the plan lands.

---

## File Structure

| File | Responsibility |
|---|---|
| `crates/waml/src/view/mask.rs` (**new**) | `ProjectionMask` — the disabled-name set and its predicate |
| `crates/waml/src/view/mod.rs` | export `mask` |
| `crates/waml/src/view/chain.rs` | registry ownership; mask-aware `Chain::build` |
| `crates/waml/src/okf.rs:556` | `Bundle::resolved_view` gains the mask |
| `crates/waml-editor/src/folder_projection.rs` | delete `ViewMode`; `chain_for` / `project_rows` take `&ProjectionMask` |
| `crates/waml-editor/src/{tree,documents,nav,navigation,folder_documents,folder_view,extension_editor}.rs` | mechanical `ViewMode` → `&ProjectionMask` thread-through |
| `crates/waml-editor/src/app.rs`, `app/{navigation,actions}.rs` | `App` holds the mask; toggle handling; nav-tree cache key |
| `crates/waml-editor/src/popup/base.rs` | `PopupItem::checked` |
| `crates/waml-editor/src/popup/menu.rs` | sticky (toggle-and-stay) open mode |
| `crates/waml-editor/src/icons.rs`, `resources/icons/library-big.svg` (**new**) | the partial-mask glyph |
| `crates/waml-editor/src/tree_panel.rs` | mask field, three-state glyph, toolbar buttons |
| `crates/waml-editor/src/app/menus.rs` | build the projection popup items from the registry |

---

### Task 1: `ProjectionMask` in the `waml` crate

**Files:**
- Create: `crates/waml/src/view/mask.rs`
- Modify: `crates/waml/src/view/mod.rs`
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/view/mask.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `waml::view::mask::ProjectionMask` with
  `ProjectionMask::default() -> ProjectionMask` (empty),
  `ProjectionMask::from_names(impl IntoIterator<Item = impl Into<String>>) -> ProjectionMask`,
  `is_masked(&self, name: &str) -> bool`,
  `set_masked(&mut self, name: &str, masked: bool)`,
  `is_empty(&self) -> bool`,
  `names(&self) -> impl Iterator<Item = &str>`.
  Derives `Clone, Debug, Default, PartialEq, Eq`. Backed by `BTreeSet<String>` so equality and
  iteration order are deterministic (the nav-tree cache in Task 4 compares masks).

- [ ] **Step 1: Write the failing test**

Create `crates/waml/src/view/mask.rs` with the tests only (no type yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_mask_disables_nothing() {
        let mask = ProjectionMask::default();
        assert!(mask.is_empty());
        assert!(!mask.is_masked("hide"));
        assert!(!mask.is_masked("uml"));
    }

    #[test]
    fn set_masked_adds_and_removes_one_name_without_touching_siblings() {
        let mut mask = ProjectionMask::default();
        mask.set_masked("hide", true);
        assert!(mask.is_masked("hide"));
        assert!(!mask.is_masked("uml"));

        mask.set_masked("uml", true);
        mask.set_masked("hide", false);
        assert!(!mask.is_masked("hide"));
        assert!(mask.is_masked("uml"), "unmasking one name must not clear the set");
        assert!(!mask.is_empty());
    }

    #[test]
    fn masks_built_from_the_same_names_in_any_order_are_equal() {
        let a = ProjectionMask::from_names(["uml", "hide"]);
        let b = ProjectionMask::from_names(["hide", "uml"]);
        assert_eq!(a, b, "the nav-tree cache key compares masks by value");
        assert_eq!(a.names().collect::<Vec<_>>(), vec!["hide", "uml"]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml mask::`
Expected: FAIL — `cannot find type ProjectionMask in this scope` (and the module is not
declared yet, so it may not even reach the test).

- [ ] **Step 3: Write minimal implementation**

At the TOP of `crates/waml/src/view/mask.rs`, above the test module:

```rust
//! The projection mask: which declared middleware stages are switched OFF for
//! this session.
//!
//! Lives in `waml`, not `waml-editor`, because the CLI and the vscode server
//! run the same chain path and must be able to describe the same state.
//!
//! Presentational reachability ONLY. A row a chain declined to emit is not
//! protected by anything; masking a stage asks for the listing without it. It
//! is never a permission boundary.
//!
//! Session-only by construction: nothing here serializes, and no caller writes
//! it to `.waml/settings.json`. Raw is a deliberate act, not a preference, so
//! every launch starts empty and an author's declared `view:` is what a reader
//! sees unless they ask otherwise.

use std::collections::BTreeSet;

/// A set of disabled middleware names. Empty (the default) is exactly today's
/// behaviour: every declared stage runs.
///
/// `BTreeSet` rather than `HashSet`: `PartialEq` is used as a cache key by the
/// editor's nav-tree memo, and `names()` feeds a popup whose row order must not
/// wobble between frames.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProjectionMask {
    disabled: BTreeSet<String>,
}

impl ProjectionMask {
    pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> ProjectionMask {
        ProjectionMask {
            disabled: names.into_iter().map(Into::into).collect(),
        }
    }

    /// Is `name` switched off? `Chain::build` asks this per declared entry.
    pub fn is_masked(&self, name: &str) -> bool {
        self.disabled.contains(name)
    }

    pub fn set_masked(&mut self, name: &str, masked: bool) {
        if masked {
            self.disabled.insert(name.to_string());
        } else {
            self.disabled.remove(name);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.disabled.is_empty()
    }

    /// The disabled names, sorted. Deterministic for the popup and for tests.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.disabled.iter().map(String::as_str)
    }
}
```

In `crates/waml/src/view/mod.rs`, add the module declaration alongside the existing ones
(keep the file's existing alphabetical/grouping convention):

```rust
pub mod mask;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml mask::`
Expected: PASS, 3 tests.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: green except the known pre-existing `waml-syntax` proptest red.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/view/mask.rs crates/waml/src/view/mod.rs
git commit -m "feat(view): a ProjectionMask of disabled middleware names"
```

---

### Task 2: The registry remembers which extension owns each name

**Files:**
- Modify: `crates/waml/src/view/chain.rs:51-109` (the `MiddlewareRegistry` struct,
  `register`, `from_extensions`, `build`)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/view/chain.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  `MiddlewareRegistry::register(&mut self, name, factory)` — unchanged signature, records the
  owner `""` (host-registered, ungrouped);
  `MiddlewareRegistry::register_owned(&mut self, owner: impl Into<String>, name: impl Into<String>, factory)`;
  `MiddlewareRegistry::owner(&self, name: &str) -> Option<&str>`;
  `MiddlewareRegistry::owners(&self) -> Vec<(&str, Vec<&str>)>` — one entry per owning
  extension, sorted by owner name, each with its middleware names sorted. Owners whose name is
  `""` are omitted (test/host registrations have no extension to group under).

  Task 10 builds the popup from `owners()`; nothing else may hand-write an extension list.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/src/view/chain.rs`'s existing `mod tests`:

```rust
    #[test]
    fn from_extensions_records_which_extension_owns_each_name() {
        let extensions: Vec<&dyn crate::extension::CoreExtension> =
            crate::extension::SHIPPED_EXTENSIONS
                .iter()
                .map(|ext| *ext as &dyn crate::extension::CoreExtension)
                .collect();
        let registry = MiddlewareRegistry::from_extensions(&extensions).unwrap();

        assert_eq!(registry.owner("hide"), Some("core"));
        assert_eq!(registry.owner("index"), Some("core"));
        assert_eq!(registry.owner("uml"), Some("uml"));
        assert_eq!(registry.owner("nonexistent"), None);

        let owners = registry.owners();
        let shape: Vec<(&str, Vec<&str>)> = owners
            .iter()
            .map(|(owner, names)| (*owner, names.clone()))
            .collect();
        assert_eq!(
            shape,
            vec![("core", vec!["hide", "index"]), ("uml", vec!["uml"])],
            "owners() is the ONE source the editor's popup is built from",
        );
    }

    #[test]
    fn a_host_registration_has_no_owner_and_is_not_grouped() {
        let mut registry = MiddlewareRegistry::new();
        registry.register("pass-through", || Box::new(PassThrough));
        assert_eq!(registry.owner("pass-through"), None);
        assert!(
            registry.owners().is_empty(),
            "an ungrouped name must not invent an extension group",
        );
    }
```

If the existing test module has no `PassThrough` stage, reuse whichever no-op `Projection`
test double the module already defines (the module already builds registries for the
`pass-through` / `adding` / `failing` tests around lines 780-940) — use that name verbatim
rather than adding a second double.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml chain::tests::from_extensions_records`
Expected: FAIL — `no method named owner found for struct MiddlewareRegistry`.

- [ ] **Step 3: Write minimal implementation**

Replace the struct and its impl block in `crates/waml/src/view/chain.rs`:

```rust
/// A name -> (owner, stage-factory) map. Populated by the host (Task E1's
/// `CoreExtension`, later others); the chain looks names up the same way
/// regardless of who populated it.
///
/// The owner is kept so the editor can offer an extension-level projection
/// toggle without hand-writing a second extension list beside this one -- two
/// construction sites that disagree are invisible (see
/// `folder_projection::editor_registry`'s own warning).
#[derive(Default)]
pub struct MiddlewareRegistry {
    factories: HashMap<String, Registration>,
}

#[derive(Clone)]
struct Registration {
    /// The `CoreExtension::name()` that declared this middleware. Empty for a
    /// direct `register` call (tests, hosts) -- ungrouped, never offered as an
    /// extension-level toggle.
    owner: String,
    factory: Arc<dyn Fn() -> Box<dyn Projection> + Send + Sync>,
}
```

and inside `impl MiddlewareRegistry`:

```rust
    /// Register a middleware under `name`, with no owning extension. A later
    /// registration for the same name replaces the earlier one.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        factory: impl Fn() -> Box<dyn Projection> + Send + Sync + 'static,
    ) {
        self.register_owned("", name, factory);
    }

    /// Register a middleware `name` owned by extension `owner`.
    pub fn register_owned(
        &mut self,
        owner: impl Into<String>,
        name: impl Into<String>,
        factory: impl Fn() -> Box<dyn Projection> + Send + Sync + 'static,
    ) {
        self.factories.insert(
            name.into(),
            Registration {
                owner: owner.into(),
                factory: Arc::new(factory),
            },
        );
    }

    /// Build a registry from every `CoreExtension`'s declared middleware.
    /// One flat name table: the same name declared by two extensions is a
    /// build error rather than a silent last-write-wins.
    pub fn from_extensions(
        extensions: &[&dyn crate::extension::CoreExtension],
    ) -> Result<MiddlewareRegistry, DuplicateMiddlewareName> {
        let mut registry = MiddlewareRegistry::new();
        for extension in extensions {
            for (name, factory) in extension.middleware() {
                if registry.factories.contains_key(name) {
                    return Err(DuplicateMiddlewareName(name.to_string()));
                }
                registry.factories.insert(
                    name.to_string(),
                    Registration {
                        owner: extension.name().to_string(),
                        factory,
                    },
                );
            }
        }
        Ok(registry)
    }

    /// Which extension declared `name`, if any. `None` for an unregistered
    /// name AND for an ungrouped host registration.
    pub fn owner(&self, name: &str) -> Option<&str> {
        self.factories
            .get(name)
            .map(|r| r.owner.as_str())
            .filter(|owner| !owner.is_empty())
    }

    /// Extension name -> its middleware names, both sorted. The ONE source the
    /// editor's projection popup is built from. Ungrouped registrations are
    /// omitted rather than pooled under a blank group.
    pub fn owners(&self) -> Vec<(&str, Vec<&str>)> {
        let mut grouped: std::collections::BTreeMap<&str, Vec<&str>> =
            std::collections::BTreeMap::new();
        for (name, registration) in &self.factories {
            if registration.owner.is_empty() {
                continue;
            }
            grouped
                .entry(registration.owner.as_str())
                .or_default()
                .push(name.as_str());
        }
        grouped
            .into_iter()
            .map(|(owner, mut names)| {
                names.sort_unstable();
                (owner, names)
            })
            .collect()
    }

    fn build(&self, name: &str) -> Option<Box<dyn Projection>> {
        self.factories.get(name).map(|r| (r.factory)())
    }
```

`extension.middleware()` already yields the `Arc` factory type — if the compiler disagrees
about `factory`'s type here, match whatever `CoreExtension::middleware()` returns rather than
changing that trait.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml chain::`
Expected: PASS. Every pre-existing chain test still passes — no behavioural change yet.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/view/chain.rs
git commit -m "feat(view): the middleware registry remembers its owning extension"
```

---

### Task 3: `Chain::build` skips masked stages silently

**Files:**
- Modify: `crates/waml/src/view/chain.rs:176-257` (`Chain::build`)
- Modify: `crates/waml/src/okf.rs:556-580` (`Bundle::resolved_view`)
- Modify: every in-crate caller of `Chain::build` / `resolved_view`:
  `crates/waml/src/extension.rs:126,182`, `crates/waml/src/okf.rs:1739-1823` (tests),
  `crates/waml/src/view/chain.rs:783-940` (tests)
- Test: inline `#[cfg(test)] mod tests` in `crates/waml/src/view/chain.rs`

**Interfaces:**
- Consumes: `ProjectionMask` (Task 1).
- Produces:
  `Chain::build(decl: &ViewDecl, registry: &MiddlewareRegistry, index: &okf::Index, mask: &ProjectionMask) -> (Chain, Vec<Diagnostic>)`;
  `Bundle::resolved_view(&self, directory: &str, registry: &MiddlewareRegistry, mask: &ProjectionMask) -> (Chain, Vec<Diagnostic>)`.
  The mask is the LAST parameter in both. Task 4 calls `resolved_view` with the editor's mask.

**Behaviour required:**
1. Ids are computed via `ViewId::disambiguate` over the **declared** names first, unchanged,
   so a surviving stage keeps the id it would have had unmasked — flipping the mask never
   silently renumbers owners.
2. A masked name is skipped: no stage pushed, no id pushed, **no diagnostic**.
3. `hide`'s parameter check is gated on the mask — a masked `hide` with malformed globs must
   NOT collapse the chain.
4. Unknown-name behaviour is unchanged: whole-chain fallback plus `UnknownViewMiddleware`.
   The masked check comes FIRST, so masking a name that is also unknown skips it quietly —
   consistent with (2), and the reader asked for it off either way.
5. `markdown` / `member` resolutions are untouched by the mask.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml/src/view/chain.rs`'s `mod tests`. Use the module's existing `decl(...)`,
`idx()`, and registry-building helpers verbatim; the `hide`-specific test needs a real
core registry and a real index with a malformed `hide:` entry, so build it the way
`crates/waml/src/view/hide.rs`'s own tests build theirs:

```rust
    fn core_registry_for_tests() -> MiddlewareRegistry {
        let extensions: Vec<&dyn crate::extension::CoreExtension> =
            crate::extension::SHIPPED_EXTENSIONS
                .iter()
                .map(|ext| *ext as &dyn crate::extension::CoreExtension)
                .collect();
        MiddlewareRegistry::from_extensions(&extensions).unwrap()
    }

    #[test]
    fn an_empty_mask_reproduces_the_unmasked_chain_exactly() {
        let registry = registry_with_pass_through();
        let idx = idx();
        let (unmasked, diags) = Chain::build(
            &decl(&["pass-through", "pass-through"]),
            &registry,
            &idx,
            &ProjectionMask::default(),
        );
        assert!(diags.is_empty());
        assert_eq!(unmasked.ids().len(), 2);
    }

    #[test]
    fn masking_one_stage_keeps_its_siblings_and_diagnoses_nothing() {
        let registry = core_registry_for_tests();
        let idx = idx();
        let mask = ProjectionMask::from_names(["uml"]);
        let (chain, diags) = Chain::build(&decl(&["index", "uml"]), &registry, &idx, &mask);
        assert!(
            diags.is_empty(),
            "a masked stage is a reader's choice, not an author error: {diags:?}",
        );
        assert_eq!(
            chain.ids().len(),
            1,
            "the sibling survives; only the masked stage is dropped",
        );
    }

    #[test]
    fn a_surviving_stage_keeps_the_id_it_would_have_had_unmasked() {
        let registry = core_registry_for_tests();
        let idx = idx();
        let unmasked = Chain::build(
            &decl(&["index", "uml"]),
            &registry,
            &idx,
            &ProjectionMask::default(),
        )
        .0;
        let masked = Chain::build(
            &decl(&["index", "uml"]),
            &registry,
            &idx,
            &ProjectionMask::from_names(["index"]),
        )
        .0;
        assert_eq!(
            masked.ids().first(),
            unmasked.ids().get(1),
            "ids come from the DECLARED names, so a mask flip never renumbers an owner",
        );
    }

    #[test]
    fn an_unknown_name_still_collapses_the_whole_chain() {
        let registry = core_registry_for_tests();
        let idx = idx();
        let (chain, diags) = Chain::build(
            &decl(&["nonexistent"]),
            &registry,
            &idx,
            &ProjectionMask::default(),
        );
        assert_eq!(chain.ids().len(), 0);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, DiagCode::UnknownViewMiddleware);
    }
```

And the `hide`-gating test, which needs an index whose `extra` carries a malformed `hide:`
(the shape `super::hide::parse_hide_globs` rejects — a scalar where a sequence is required):

```rust
    #[test]
    fn a_masked_hide_with_malformed_globs_does_not_collapse_the_chain() {
        let registry = core_registry_for_tests();
        let idx = index_with_malformed_hide_globs();

        let (collapsed, diags) = Chain::build(
            &decl(&["hide", "index"]),
            &registry,
            &idx,
            &ProjectionMask::default(),
        );
        assert_eq!(collapsed.ids().len(), 0, "unmasked, a bad `hide:` still collapses");
        assert_eq!(diags.len(), 1);

        let (survives, diags) = Chain::build(
            &decl(&["hide", "index"]),
            &registry,
            &idx,
            &ProjectionMask::from_names(["hide"]),
        );
        assert!(
            diags.is_empty(),
            "a switched-off stage's params are not checked: {diags:?}",
        );
        assert_eq!(
            survives.ids().len(),
            1,
            "`index` survives -- the bad params belonged to the masked stage",
        );
    }
```

Write `index_with_malformed_hide_globs()` next to the other test helpers, building an
`okf::Index` whose `extra` holds a `hide` entry of the wrong shape. Mirror however
`crates/waml/src/view/hide.rs`'s own tests construct a rejected `extra` — read that module's
tests first and copy the construction verbatim rather than guessing the frontmatter value
type.

`Chain::ids()` may not exist as a public accessor. If it does not, add one:

```rust
    /// The resolved stage ids, in order. Test + diagnostic surface.
    pub fn ids(&self) -> &[ViewId] {
        &self.ids
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml chain::tests::masking_one_stage`
Expected: FAIL — `this function takes 3 arguments but 4 arguments were supplied`.

- [ ] **Step 3: Write minimal implementation**

In `crates/waml/src/view/chain.rs`, add the import and change `build`:

```rust
use super::mask::ProjectionMask;
```

```rust
    /// Build from a `ViewDecl` against a middleware registry. An unknown
    /// name is a declaration-level failure: returns the root-view-only
    /// chain plus a diagnostic spanned on the name in `view:`.
    ///
    /// `mask` switches declared stages OFF. A masked name is SKIPPED, never
    /// looked up: removing it from the registry instead would hit the
    /// unknown-name path below and collapse the whole chain, destroying the
    /// per-stage granularity the mask exists to provide and spraying
    /// diagnostics that read as author errors.
    pub fn build(
        decl: &ViewDecl,
        registry: &MiddlewareRegistry,
        index: &okf::Index,
        mask: &ProjectionMask,
    ) -> (Chain, Vec<Diagnostic>) {
        let file = format!("{}/index.md", index.directory.as_str());
        let names: Vec<&str> = decl.entries.iter().map(|e| entry_name(&e.raw)).collect();
        // Disambiguated over the DECLARED names, before masking: a surviving
        // stage keeps the id it would have had unmasked, so flipping the mask
        // never silently renumbers a row's owner.
        let disambiguated = ViewId::disambiguate(names.iter().copied());

        let mut ids = Vec::with_capacity(decl.entries.len());
        let mut stages: Vec<Box<dyn Projection>> = Vec::with_capacity(decl.entries.len());
        let mut resolution: Option<Resolution> = None;
        let mut diagnostics = Vec::new();
        for (entry, (name, view_id)) in decl.entries.iter().zip(names.iter().zip(disambiguated)) {
            // Switched off by the reader. Silent: no stage, no id, no
            // diagnostic -- including for a name that is ALSO unknown, since
            // the reader asked for it off either way.
            if mask.is_masked(name) {
                continue;
            }
            match *name {
```

The rest of the `match` body is unchanged, including `markdown`, `member`, the `hide` params
check, and the registry lookup. The `hide` arm needs no explicit gate: the `continue` above
already skipped a masked `hide` before reaching it. Leave a comment on the `hide` arm saying
so, so nobody re-adds a redundant check:

```rust
                // A masked `hide` never reaches here -- the mask `continue`
                // above skipped it, so its params are not checked and a
                // malformed `hide:` cannot collapse a chain whose `hide` is
                // switched off.
                "hide" => {
```

In `crates/waml/src/okf.rs`, thread the mask through `resolved_view`:

```rust
    pub fn resolved_view(
        &self,
        directory: &str,
        registry: &crate::view::chain::MiddlewareRegistry,
        mask: &crate::view::mask::ProjectionMask,
    ) -> (
        crate::view::chain::Chain,
        Vec<crate::diagnostic::Diagnostic>,
    ) {
        use crate::view::chain::Chain;

        let Some(index) = self.index(directory) else {
            return (Chain::root_only(registry), Vec::new());
        };
        if let Some(decl) = &index.view {
            return Chain::build(decl, registry, index, mask);
        }
        if let Some(decl) = self
            .resolved_profile(directory)
            .and_then(crate::profile::profile)
            .and_then(|profile_def| profile_def.default_view)
        {
            return Chain::build(&decl, registry, index, mask);
        }
        (Chain::root_only(registry), Vec::new())
    }
```

Then fix every remaining call site in the `waml` crate by appending
`&ProjectionMask::default()` (tests) — `crates/waml/src/extension.rs:126,182` and the
`resolved_view` tests in `crates/waml/src/okf.rs`. Do not change what those tests assert.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p waml`
Expected: PASS, including the five new tests.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Note: `waml-editor` will now fail to compile at its `resolved_view` call site
(`folder_projection.rs:91`). Fix it minimally IN THIS TASK by passing
`&waml::view::mask::ProjectionMask::default()` there, keeping `ViewMode` intact — Task 4
deletes it properly. The gate must be green before committing.

- [ ] **Step 6: Commit**

```bash
git add crates/waml/src/view/chain.rs crates/waml/src/okf.rs crates/waml/src/extension.rs crates/waml-editor/src/folder_projection.rs
git commit -m "feat(view): Chain::build skips masked stages silently"
```

---

### Task 4: Delete `ViewMode`; thread `&ProjectionMask` through the editor

**Files:**
- Modify: `crates/waml-editor/src/folder_projection.rs:20-133` (delete `ViewMode`; `chain_for`
  and `project_rows` take `&ProjectionMask`) and its tests at `:182-275`
- Modify (mechanical, `mode: ViewMode` → `mask: &ProjectionMask`):
  `crates/waml-editor/src/tree.rs:122,145,207` (+ its tests),
  `crates/waml-editor/src/documents.rs:65,107` (+ tests),
  `crates/waml-editor/src/nav.rs:46,134` (+ tests),
  `crates/waml-editor/src/navigation.rs:571`,
  `crates/waml-editor/src/folder_documents.rs:45` (+ tests),
  `crates/waml-editor/src/folder_view.rs:245` (+ tests),
  `crates/waml-editor/src/extension_editor.rs:53,331`
- Modify: `crates/waml-editor/src/app.rs:20,719,728`,
  `crates/waml-editor/src/app/navigation.rs:670,684-696,718`,
  `crates/waml-editor/src/app/tests/navigation.rs:2035-2052`
- Test: `crates/waml-editor/src/folder_projection.rs` inline tests

**Interfaces:**
- Consumes: `ProjectionMask` (Task 1), the mask-aware `resolved_view` (Task 3).
- Produces:
  `folder_projection::chain_for(analysis, directory, mask: &ProjectionMask, registry) -> (Chain, Vec<Diagnostic>)`;
  `folder_projection::project_rows(analysis, directory, mask: &ProjectionMask, limits, registry) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)>`;
  `folder_projection::maskable_names(registry: &MiddlewareRegistry) -> Vec<(&str, Vec<&str>)>` —
  `registry.owners()` with the terminal `index` stage filtered out, and any extension left
  with no maskable names dropped entirely;
  `App::projection_mask: ProjectionMask` (private field, read via `&self.projection_mask`).
  Task 8 reads `maskable_names`; Task 10 builds popup rows from it.

**Notes on the mechanical sweep:**
- The parameter is `&ProjectionMask`, not an owned clone — the mask is a `BTreeSet` and these
  functions run per directory on every model refresh.
- `ViewMode::Raw` in a test becomes `&ProjectionMask::from_names(["hide", "uml"])` — "every
  maskable name masked" — EXCEPT where the test's point is specifically that raw bypasses a
  named stage, in which case mask only that stage and say so in the assertion message.
- `ViewMode::Projected` becomes `&ProjectionMask::default()`.
- `App::nav_tree`'s cache key is `((u64, ViewMode, usize), ProjectTree)`. `ProjectionMask` is
  `Clone + Eq` but not `Copy`, so the key becomes `(u64, ProjectionMask, usize)` and the
  comparison clones the mask when the memo is refilled. That is one small `BTreeSet` clone per
  cache miss, not per row.
- `App::toggle_view_mode` (`app/navigation.rs:684`) is replaced in Task 10. For THIS task,
  keep a working control: rename it `toggle_full_raw` and have it flip between
  `ProjectionMask::default()` and every maskable name masked, so the existing
  `ToggleViewMode` button keeps behaving exactly as it does today while the popup is built.
- `Chain::raw()` is no longer called from the editor. Leave it in `waml` — its own tests use
  it — but delete `folder_projection`'s reference to it.

- [ ] **Step 1: Rewrite the `folder_projection` tests first (they are the spec of this task)**

Replace the `ViewMode` tests in `crates/waml-editor/src/folder_projection.rs`'s `mod tests`:

```rust
    use waml::view::mask::ProjectionMask;

    fn every_maskable_name() -> ProjectionMask {
        let registry = core_registry();
        ProjectionMask::from_names(
            maskable_names(&registry)
                .into_iter()
                .flat_map(|(_owner, names)| names)
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn an_empty_mask_runs_the_declared_chain_and_a_full_mask_bypasses_it() {
        let prepared = hidden_bundle();
        let limits = ChainLimits::default();

        let (_, projected, diagnostics) = project_rows(
            prepared.okf(),
            "/",
            &ProjectionMask::default(),
            limits,
            &core_registry(),
        )
        .unwrap();
        assert!(
            diagnostics.is_empty(),
            "a correctly authored `view: hide` must not be diagnosed: {diagnostics:?}"
        );
        assert_eq!(
            projected
                .iter()
                .map(|row| row.label.as_str())
                .collect::<Vec<_>>(),
            vec!["Orders"],
        );

        let (_, raw, raw_diagnostics) = project_rows(
            prepared.okf(),
            "/",
            &every_maskable_name(),
            limits,
            &core_registry(),
        )
        .unwrap();
        assert!(
            raw_diagnostics.is_empty(),
            "masking a stage is not an author error"
        );
        assert_eq!(
            raw.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
            vec!["Orders", "References"],
            "a full mask is presentational reachability, not a permission decision",
        );
    }

    #[test]
    fn masking_only_hide_leaves_every_other_stage_running() {
        let prepared = hidden_bundle();
        let (_, rows, diagnostics) = project_rows(
            prepared.okf(),
            "/",
            &ProjectionMask::from_names(["hide"]),
            ChainLimits::default(),
            &core_registry(),
        )
        .unwrap();
        assert!(diagnostics.is_empty());
        assert_eq!(
            rows.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(),
            vec!["Orders", "References"],
            "`hide` is the only stage this folder declares, so its rows come back",
        );
    }

    #[test]
    fn an_unknown_middleware_name_still_diagnoses_under_an_empty_mask() {
        let prepared = analysis([
            (
                "index.md",
                "---\nview: nonexistent\n---\n# Root\n\n* [Orders](orders.md)\n",
            ),
            ("orders.md", "# Orders\n"),
        ]);
        let (_, _, declared) = project_rows(
            prepared.okf(),
            "/",
            &ProjectionMask::default(),
            ChainLimits::default(),
            &core_registry(),
        )
        .unwrap();
        assert!(!declared.is_empty(), "an unknown middleware name diagnoses");
    }

    #[test]
    fn a_missing_directory_yields_none_rather_than_panicking() {
        let prepared = hidden_bundle();
        assert!(project_rows(
            prepared.okf(),
            "/missing",
            &ProjectionMask::default(),
            ChainLimits::default(),
            &core_registry(),
        )
        .is_none());
    }

    #[test]
    fn a_full_mask_leaves_every_row_owned_by_the_root_view() {
        let prepared = hidden_bundle();
        let (_, rows, _) = project_rows(
            prepared.okf(),
            "/",
            &every_maskable_name(),
            ChainLimits::default(),
            &core_registry(),
        )
        .unwrap();
        assert!(
            rows.iter()
                .all(|row| row.id.owner.as_str() == waml::view::ROOT_VIEW_OWNER),
            "with every declared stage masked the chain is empty, so RootView owns every row",
        );
    }

    #[test]
    fn index_is_never_offered_as_maskable() {
        let registry = core_registry();
        let offered: Vec<&str> = maskable_names(&registry)
            .into_iter()
            .flat_map(|(_owner, names)| names)
            .collect();
        assert!(
            !offered.contains(&"index"),
            "`index` is the terminal stage; masking it cannot remove the listing",
        );
        assert!(offered.contains(&"hide"));
        assert!(offered.contains(&"uml"));
    }

    #[test]
    fn an_extension_toggle_masks_exactly_that_extensions_names() {
        let registry = core_registry();
        let core_names: Vec<&str> = maskable_names(&registry)
            .into_iter()
            .find(|(owner, _)| *owner == "core")
            .expect("core owns at least one maskable stage")
            .1;
        let mask = ProjectionMask::from_names(core_names.iter().map(|n| n.to_string()));
        assert!(mask.is_masked("hide"));
        assert!(
            !mask.is_masked("uml"),
            "an extension toggle must not reach another extension's stages",
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p waml-editor folder_projection::`
Expected: FAIL to compile — `cannot find function maskable_names`, and `project_rows` arity.

- [ ] **Step 3: Implement `folder_projection`**

Delete the `ViewMode` enum entirely. Replace `chain_for` / `project_rows` signatures and add
`maskable_names`:

```rust
use waml::view::mask::ProjectionMask;

/// The stages a reader may switch off, grouped by owning extension.
///
/// `registry.owners()` minus the terminal `index` stage: `RootView` is where a
/// chain lands whenever it runs out of declared stages, so masking `index`
/// cannot remove the listing and offering it would be a lie. An extension left
/// with nothing maskable is dropped rather than shown as an empty group.
///
/// Driven off the registry -- NOT a second hand-written extension list. Two
/// construction sites that disagree are invisible (see `editor_registry`).
pub fn maskable_names(registry: &MiddlewareRegistry) -> Vec<(&str, Vec<&str>)> {
    registry
        .owners()
        .into_iter()
        .filter_map(|(owner, names)| {
            let names: Vec<&str> = names
                .into_iter()
                .filter(|name| *name != waml::view::ROOT_VIEW_NAME)
                .collect();
            (!names.is_empty()).then_some((owner, names))
        })
        .collect()
}

/// The chain `directory` runs under `mask`, plus any build-level diagnostics
/// (unknown middleware name, bad params) the declared chain produced.
///
/// An empty mask is exactly today's projected behaviour. A mask naming every
/// maskable stage yields the identity listing -- the chain builds, every
/// declared stage is skipped, and `RootView` owns every row.
pub fn chain_for(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mask: &ProjectionMask,
    registry: &MiddlewareRegistry,
) -> (Chain, Vec<Diagnostic>) {
    analysis.bundle.resolved_view(directory, registry, mask)
}

pub fn project_rows(
    analysis: &waml::analysis::OkfAnalysis,
    directory: &str,
    mask: &ProjectionMask,
    limits: ChainLimits,
    registry: &MiddlewareRegistry,
) -> Option<(Chain, Vec<Row>, Vec<Diagnostic>)> {
    // ... body unchanged except the chain_for call:
    let (chain, mut diagnostics) = chain_for(analysis, directory, mask, registry);
    // ...
}
```

`waml::view::ROOT_VIEW_NAME` may not exist — `ROOT_VIEW_OWNER` does (used at
`folder_projection.rs:272`). If there is no separate name constant, filter on the literal
`"index"` and add a comment naming `crates/waml/src/view/root.rs` as the reason, or add a
`pub const ROOT_VIEW_NAME: &str = "index";` next to `ROOT_VIEW_OWNER` and use it. Prefer the
constant.

Also update the module doc comment at the top of the file: it says "in the same mode" — say
"under the same mask".

- [ ] **Step 4: Sweep every remaining call site**

Run `cargo check -p waml-editor` and fix each error mechanically per the notes above. Work
until `cargo check -p waml-editor` is clean. Do not change any test's assertion semantics
during the sweep — only its mode argument.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p waml-editor`
Expected: PASS.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 7: Commit**

```bash
git add -A crates/waml-editor crates/waml
git commit -m "refactor(editor): the projection mask replaces ViewMode

One value describes what is running, instead of a mode and a mask that
can disagree. Full raw is now every maskable name masked."
```

---

### Task 5: `PopupItem` carries an optional checked state

**Files:**
- Modify: `crates/waml-editor/src/popup/base.rs:11-22`
- Modify every construction site: `crates/waml-editor/src/app/menus.rs:10,17,24,31,38,45,67,75,83,92,109`,
  `crates/waml-editor/src/class_diagram_view.rs:801`,
  `crates/waml-editor/src/popup/node_menu.rs:25,32,90`,
  `crates/waml-editor/src/popup/select.rs:267`,
  `crates/waml-editor/src/popup/marking.rs:177` (test helper),
  `crates/waml-editor/src/popup/radial.rs:865` (test helper)
- Test: inline tests in `crates/waml-editor/src/popup/base.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `PopupItem.checked: Option<bool>` — `None` means a plain item (today's behaviour
  exactly), `Some(true)`/`Some(false)` a checkable row. Task 6 draws it; Task 10 sets it.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-editor/src/popup/base.rs`'s `mod tests`:

```rust
    #[test]
    fn a_plain_item_has_no_checked_state() {
        let item = PopupItem {
            id: live_id!(plain),
            label: "Plain".to_string(),
            icon: None,
            danger: false,
            enabled: true,
            checked: None,
        };
        assert_eq!(
            item.checked, None,
            "None keeps every pre-existing popup row behaving identically",
        );
    }

    #[test]
    fn a_checkable_item_reports_both_states() {
        let on = PopupItem {
            id: live_id!(hide),
            label: "hide".to_string(),
            icon: None,
            danger: false,
            enabled: true,
            checked: Some(true),
        };
        let off = PopupItem {
            checked: Some(false),
            ..on.clone()
        };
        assert_eq!(on.checked, Some(true));
        assert_eq!(off.checked, Some(false));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor popup::base::`
Expected: FAIL — `struct PopupItem has no field named checked`.

- [ ] **Step 3: Add the field**

In `crates/waml-editor/src/popup/base.rs`:

```rust
    /// `false` = greyed, holds its slot, cannot arm or commit.
    pub enabled: bool,
    /// `None` = a plain command row: invoking it commits and closes, which is
    /// every pre-existing popup row. `Some(_)` = a checkable row in a sticky
    /// popup: it reports its state and invoking it toggles without closing
    /// (see `MenuPopup::open_sticky`).
    pub checked: Option<bool>,
```

- [ ] **Step 4: Fix every construction site**

Run `cargo check -p waml-editor --all-targets` and add `checked: None,` to each of the listed
`PopupItem { .. }` literals. Every one is a plain command row today — none becomes checkable
in this task.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p waml-editor popup::`
Expected: PASS.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 7: Commit**

```bash
git add -A crates/waml-editor/src
git commit -m "feat(popup): PopupItem carries an optional checked state"
```

---

### Task 6: `MenuPopup` gains a sticky (toggle-and-stay) open mode

**Files:**
- Modify: `crates/waml-editor/src/popup/menu.rs:420-500` (`MenuPopup` fields, `open_popup`,
  a new `open_sticky`, `draw`) and its `handle`/verdict path
- Modify: `crates/waml-editor/src/popup/root.rs:290-310` (the open dispatch) if a new
  `PopupOpen` variant is needed to reach `open_sticky`
- Test: inline tests in `crates/waml-editor/src/popup/menu.rs`

**Interfaces:**
- Consumes: `PopupItem::checked` (Task 5).
- Produces:
  `MenuPopup::open_sticky(&mut self, cx: &mut Cx, anchor: DVec2, items: Vec<PopupItem>, max_height: Option<f64>)`;
  `MenuPopup::set_items(&mut self, cx: &mut Cx, items: Vec<PopupItem>)` — re-seeds the open
  surface's rows in place, so a toggle can repaint its own checkmark;
  `MenuPopup::is_sticky(&self) -> bool`.
  In sticky mode, committing an item emits `PopupVerdict::Consumed` plus a widget action
  carrying the invoked id, and the surface STAYS OPEN. Light-dismiss (Esc / outside click /
  focus loss) closes it exactly as before, reporting `PopupResult::Dismissed`.

  Add `PopupRootAction::Toggled { tag, id }` next to the existing `Armed` / `Closed`, with the
  matching reader helper (mirror `PopupRoot::closed` / `armed` at `root.rs:561,573`). Task 10
  reads it.

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-editor/src/popup/menu.rs`'s test module (or create one, mirroring
`popup/marking.rs`'s test-double style):

```rust
    fn checkable(id: LiveId, checked: bool) -> PopupItem {
        PopupItem {
            id,
            label: format!("{id:?}"),
            icon: None,
            danger: false,
            enabled: true,
            checked: Some(checked),
        }
    }

    #[test]
    fn a_sticky_popup_stays_open_after_a_commit() {
        let mut cx = Cx::new_test();
        let mut menu = MenuPopup::new_for_test(&mut cx);
        menu.open_sticky(
            &mut cx,
            DVec2 { x: 100.0, y: 100.0 },
            vec![checkable(live_id!(hide), false), checkable(live_id!(uml), true)],
            None,
        );
        assert!(menu.is_open());
        assert!(menu.is_sticky());

        let verdict = menu.commit_for_test(&mut cx, live_id!(hide));
        assert_eq!(
            verdict,
            PopupVerdict::Consumed,
            "a sticky commit is consumed, NOT Closed -- a checklist toggles and stays",
        );
        assert!(menu.is_open(), "the surface must survive its own commit");
    }

    #[test]
    fn a_non_sticky_popup_still_closes_on_commit() {
        let mut cx = Cx::new_test();
        let mut menu = MenuPopup::new_for_test(&mut cx);
        menu.open_popup(
            &mut cx,
            DVec2 { x: 100.0, y: 100.0 },
            vec![PopupItem {
                id: live_id!(plain),
                label: "Plain".to_string(),
                icon: None,
                danger: false,
                enabled: true,
                checked: None,
            }],
            None,
            None,
        );
        let verdict = menu.commit_for_test(&mut cx, live_id!(plain));
        assert_eq!(
            verdict,
            PopupVerdict::Closed(PopupResult::Invoked(live_id!(plain))),
            "every pre-existing popup keeps pick-one-and-close",
        );
        assert!(!menu.is_open());
    }

    #[test]
    fn set_items_repaints_a_toggled_row_without_reopening() {
        let mut cx = Cx::new_test();
        let mut menu = MenuPopup::new_for_test(&mut cx);
        menu.open_sticky(
            &mut cx,
            DVec2 { x: 0.0, y: 0.0 },
            vec![checkable(live_id!(hide), false)],
            None,
        );
        menu.set_items(&mut cx, vec![checkable(live_id!(hide), true)]);
        assert!(menu.is_open());
        assert_eq!(menu.items_for_test()[0].checked, Some(true));
    }
```

`Cx::new_test()`, `new_for_test`, `commit_for_test`, and `items_for_test` are placeholders for
whatever harness this crate's existing widget tests use. Read
`crates/waml-editor/src/tree_panel.rs:1355-1380` and `crates/waml-editor/src/popup/marking.rs`
first and copy their harness construction verbatim. If `MenuPopup` has no test constructor,
drive `MarkingCore` directly the way `marking.rs` does and assert on the core's state rather
than inventing a widget harness — the behaviour under test (commit does not close in sticky
mode) lives in the verdict path, not in the draw.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor popup::menu::`
Expected: FAIL — `no method named open_sticky`.

- [ ] **Step 3: Implement sticky mode**

Add to `MenuPopup`'s `#[rust]` fields:

```rust
    /// Sticky: committing a row reports the toggle and leaves the surface open
    /// (a checklist), instead of the default pick-one-and-close. Light-dismiss
    /// is unchanged -- Esc, an outside click, and focus loss all still close.
    #[rust]
    sticky: bool,
```

```rust
    /// Checklist open: rows toggle and the card stays up until light-dismiss.
    pub fn open_sticky(
        &mut self,
        cx: &mut Cx,
        anchor: DVec2,
        items: Vec<PopupItem>,
        max_height: Option<f64>,
    ) {
        self.open_popup(cx, anchor, items, None, max_height);
        self.sticky = true;
    }

    pub fn is_sticky(&self) -> bool {
        self.sticky
    }

    /// Re-seed an OPEN surface's rows, keeping its geometry and armed row. A
    /// toggle repaints its own checkmark through this rather than reopening,
    /// which would reset the anchor and drop the hover.
    pub fn set_items(&mut self, cx: &mut Cx, items: Vec<PopupItem>) {
        if !self.is_open() {
            return;
        }
        self.mark.set_items(items);
        self.draw_frame.redraw(cx);
    }
```

Set `self.sticky = false;` in `open_marking` and `open_popup` (before `open_sticky` re-sets
it) and in `reset`, so a sticky open never leaks into the next surface.

In the commit path (wherever `MenuPopup::handle` currently returns
`PopupVerdict::Closed(PopupResult::Invoked(id))`), branch:

```rust
        if self.sticky {
            // A checklist reports the toggle and stays up. The opener repaints
            // the rows via `set_items`; only a light-dismiss closes the card.
            cx.widget_action(self.widget_uid(), MenuPopupAction::Toggled(id));
            return PopupVerdict::Consumed;
        }
        PopupVerdict::Closed(PopupResult::Invoked(id))
```

If `MarkingCore` has no `set_items`, add one next to its existing item accessor in
`crates/waml-editor/src/popup/marking.rs`, preserving the armed index when the new list is the
same length.

Add the action enum + the `PopupRoot` relay. In `popup/root.rs`, extend `PopupRootAction`:

```rust
    /// A sticky surface's row was toggled. The surface is STILL OPEN; the
    /// opener updates its own state and pushes fresh rows back via
    /// `MenuPopup::set_items`.
    Toggled { tag: LiveId, id: LiveId },
```

and mirror the existing `armed(...)` reader at `root.rs:573`:

```rust
    pub fn toggled(actions: &Actions, uid: WidgetUid) -> Option<(LiveId, LiveId)> {
        // same shape as `armed`
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p waml-editor popup::`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 6: Commit**

```bash
git add -A crates/waml-editor/src/popup
git commit -m "feat(popup): a sticky MenuPopup mode that toggles and stays open"
```

---

### Task 7: Draw a checkmark on a checkable popup row

**Files:**
- Modify: `crates/waml-editor/src/popup/menu.rs` (`draw`, the row loop; the label gutter
  constants `LABEL_X` / `LABEL_PAD_R` near the top of the file)
- Test: inline test in `crates/waml-editor/src/popup/menu.rs`

**Interfaces:**
- Consumes: `PopupItem::checked` (Task 5), sticky mode (Task 6).
- Produces: no new public API. `Some(true)` draws `Icon::Check` in the row's glyph slot,
  `Some(false)` draws nothing there, `None` draws whatever `item.icon` says — today's
  behaviour.

**Note:** `Icon::Check` may not exist in the catalog. Check `crates/waml-editor/src/icons.rs`
for an existing check/tick glyph FIRST. If none exists, do not vendor one in this task — use
the existing `Icon::Library`-style accent tint on the row label plus the `draw_icon_accent`
holder on the row's existing icon slot, and note it in the commit body as a follow-up. A
missing glyph must not block the mechanism.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn a_checked_row_resolves_a_different_glyph_than_an_unchecked_one() {
        assert_ne!(
            MenuPopup::row_glyph(&checkable(live_id!(hide), true)),
            MenuPopup::row_glyph(&checkable(live_id!(hide), false)),
            "a reader must be able to tell a switched-on stage from a switched-off one",
        );
        let plain = PopupItem {
            id: live_id!(plain),
            label: "Plain".to_string(),
            icon: Some(Icon::Library),
            danger: false,
            enabled: true,
            checked: None,
        };
        assert_eq!(
            MenuPopup::row_glyph(&plain),
            Some(Icon::Library),
            "a plain row still draws its own icon",
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor popup::menu::a_checked_row`
Expected: FAIL — `no function or associated item named row_glyph`.

- [ ] **Step 3: Implement**

```rust
    /// Which glyph a row draws in its icon slot. A checkable row's state wins
    /// over any icon it carries: in a checklist the slot IS the checkbox.
    pub(crate) fn row_glyph(item: &PopupItem) -> Option<Icon> {
        match item.checked {
            Some(true) => Some(Icon::Check),
            Some(false) => None,
            None => item.icon,
        }
    }
```

and call it from the row loop in `draw` in place of the current direct `item.icon` read.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p waml-editor popup::menu::`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 6: Commit**

```bash
git add -A crates/waml-editor/src/popup
git commit -m "feat(popup): a checkable row draws its state in the glyph slot"
```

---

### Task 8: Vendor the `library-big` glyph for the partial-mask state

**Files:**
- Create: `resources/icons/library-big.svg`
- Modify: `crates/waml-editor/src/icons.rs` (SIX ordered sites + the `ALL` length + the label
  count assertion)
- Modify: `crates/waml-editor/src/icons_overlay.rs` (add to the same group `Library` /
  `Code` sit in)
- Test: the existing catalog-consistency tests in `crates/waml-editor/src/icons.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Icon::LibraryBig`, DSL name `"library-big"`. Task 9 uses it as the partial-mask
  glyph.

**The trap:** `scripts/gen-icon.py` parses only `d` attributes. Lucide's `library-big` is
`<rect width="8" height="18" x="3" y="3" rx="1"/>` plus two paths — the `rect` is INVISIBLE to
the generator, so a plain run produces a glyph missing its whole left block. The rounded rect
must be hand-authored as a path.

- [ ] **Step 1: Vendor the SVG with the rect converted to a path**

Create `resources/icons/library-big.svg`. Take the lucide `library-big` source and replace the
`<rect ... rx="1"/>` with an equivalent rounded-rect `<path d="...">`, matching the
formatting of the existing entries (`resources/icons/square-library.svg` is the closest
neighbour — read it first and match its header, `viewBox`, stroke attributes, and indentation
exactly).

The rounded rect `x=3 y=3 w=8 h=18 rx=1` as a path:

```
M4 3 H10 A1 1 0 0 1 11 4 V20 A1 1 0 0 1 10 21 H4 A1 1 0 0 1 3 20 V4 A1 1 0 0 1 4 3 Z
```

- [ ] **Step 2: Generate the SDF**

Run: `python scripts/gen-icon.py library-big`
(Match the invocation the other glyphs used — read `scripts/gen-icon.py`'s own usage line
first. Do NOT run `gen-all-icons.py`; it is stale.)

Expected: a `mod.draw.IconLibraryBig` shader block to paste into `icons.rs`.

- [ ] **Step 3: Add the SIX ordered catalog sites**

In `crates/waml-editor/src/icons.rs`, in this order, placing `LibraryBig` immediately AFTER
`Library` at every site (the existing `Library` sites are at roughly `:4144` shader,
`:4299` field, `:4700` get, `:4849` enum, `:4983` ALL, `:5117` label):

1. the `mod.draw.IconLibraryBig = mod.draw.DrawColor{...}` shader block, with the same
   `// LibraryBig: the tree panel's partial-mask glyph. Faithful port of ...` header comment
   style as its neighbours;
2. the DSL field: `library_big: mod.draw.IconLibraryBig{ color: atlas.accent }`;
3. the `get` arm: `Icon::LibraryBig => &mut self.library_big,`;
4. the enum variant: `LibraryBig,`;
5. the `ALL` entry: `Icon::LibraryBig,` — and bump `pub const ALL: [Icon; 128]` to `129`;
6. the label arm: `Icon::LibraryBig => "library-big",`.

Then add `Icon::LibraryBig` to `icons_overlay.rs`'s group that already lists `Icon::Library`.

- [ ] **Step 4: Run the catalog tests**

Run: `cargo test -p waml-editor icons`
Expected: PASS — including the `ALL`-length and label-count assertions. If a count assertion
fails, you missed one of the six sites; fix the site, do not relax the assertion.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 6: Commit**

```bash
git add resources/icons/library-big.svg crates/waml-editor/src/icons.rs crates/waml-editor/src/icons_overlay.rs
git commit -m "feat(icons): a library-big glyph for the partial-mask state

gen-icon.py reads only `d` attributes, so lucide's <rect rx> is
invisible to it -- the left block is hand-authored as a rounded-rect
path in the vendored svg."
```

---

### Task 9: The tree panel's projection glyph reports three states

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:22` (import), `:371` (field), `:660` (action),
  `:715` (draw seeding), `:906-928` (`view_mode_icon` / `set_view_mode`), `:230-245` (the
  action enum)
- Modify: `crates/waml-editor/src/app/navigation.rs:670` (the push), `app/actions.rs:101,1217`
- Test: `crates/waml-editor/src/tree_panel.rs` inline tests at `:1355-1380`

**Interfaces:**
- Consumes: `ProjectionMask` (Task 1), `maskable_names` (Task 4), `Icon::LibraryBig` (Task 8).
- Produces:
  `ProjectTree::projection_icon(&self) -> Icon`;
  `ProjectTree::set_projection(&mut self, cx: &mut Cx, mask: ProjectionMask, maskable: Vec<String>)` —
  the panel stores both the mask and the maskable-name universe, because "fully masked" can
  only be judged against that universe;
  `ProjectTreeAction::OpenProjectionMenu { anchor: DVec2 }` replaces `ToggleViewMode`.

  The glyph reports the CURRENT state, not the action the button performs:
  | mask | glyph |
  |---|---|
  | empty | `Icon::Library` |
  | some but not all maskable names | `Icon::LibraryBig` |
  | every maskable name | `Icon::Code` |

- [ ] **Step 1: Rewrite the glyph test**

Replace the existing three-line `set_view_mode` test at `tree_panel.rs:1355-1380` with:

```rust
    #[test]
    fn the_projection_glyph_reports_all_three_mask_states() {
        let mut cx = /* the same harness the neighbouring tests use */;
        let mut panel = /* ditto */;
        let maskable = vec!["hide".to_string(), "uml".to_string()];

        panel.set_projection(&mut cx, ProjectionMask::default(), maskable.clone());
        assert_eq!(
            panel.projection_icon(),
            Icon::Library,
            "an empty mask means the declared chain is running",
        );

        panel.set_projection(
            &mut cx,
            ProjectionMask::from_names(["hide"]),
            maskable.clone(),
        );
        assert_eq!(
            panel.projection_icon(),
            Icon::LibraryBig,
            "some stages off, some on",
        );

        panel.set_projection(
            &mut cx,
            ProjectionMask::from_names(["hide", "uml"]),
            maskable.clone(),
        );
        assert_eq!(
            panel.projection_icon(),
            Icon::Code,
            "every maskable stage off is the old Raw",
        );
    }

    #[test]
    fn a_mask_naming_an_unmaskable_stage_does_not_read_as_fully_masked() {
        let mut cx = /* harness */;
        let mut panel = /* harness */;
        panel.set_projection(
            &mut cx,
            ProjectionMask::from_names(["index"]),
            vec!["hide".to_string(), "uml".to_string()],
        );
        assert_eq!(
            panel.projection_icon(),
            Icon::Library,
            "`index` is not maskable, so masking it changes nothing the glyph reports",
        );
    }
```

Copy the `cx` / `panel` harness construction verbatim from the neighbouring test at
`tree_panel.rs:1355` — do not invent one.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor tree_panel::tests::the_projection_glyph`
Expected: FAIL — `no method named set_projection`.

- [ ] **Step 3: Implement**

Replace the `view_mode: ViewMode` field at `:371` with:

```rust
    /// What this panel is DISPLAYING. `App` owns the session-wide mask and
    /// pushes it here; the panel never flips its own.
    #[rust]
    projection_mask: ProjectionMask,
    /// Every stage a reader may switch off, from
    /// `folder_projection::maskable_names`. Stored because "fully masked" is
    /// only meaningful against this universe -- a mask naming `index` (which
    /// is not maskable) must not read as raw.
    #[rust]
    maskable: Vec<String>,
```

```rust
    /// The glyph for the CURRENT state -- `Library` when every declared stage
    /// is running, `LibraryBig` when some are switched off, `Code` when they
    /// all are. Not the action the button would perform: a reader must be able
    /// to read the panel and know what they are looking at.
    ///
    /// `Code` is also the document header's source toggle. Deliberate: both
    /// say "you are seeing the underlying thing", and they sit in different
    /// panels.
    pub fn projection_icon(&self) -> Icon {
        let masked = self
            .maskable
            .iter()
            .filter(|name| self.projection_mask.is_masked(name))
            .count();
        if masked == 0 {
            Icon::Library
        } else if masked == self.maskable.len() {
            Icon::Code
        } else {
            Icon::LibraryBig
        }
    }

    pub fn set_projection(&mut self, cx: &mut Cx, mask: ProjectionMask, maskable: Vec<String>) {
        self.projection_mask = mask;
        self.maskable = maskable;
        let icon = self.projection_icon();
        let button = self.view.icon_button(cx, ids!(view_mode_btn));
        button.set_icon(cx, icon);
        // Anything other than "everything running" is the deliberate,
        // non-default state, so it reads lit.
        button.set_active(cx, !matches!(icon, Icon::Library));
    }
```

Update the draw-time seeding at `:711-716` to call `self.projection_icon()` and the same
`set_active` rule. Update the `ToggleViewMode` action to
`OpenProjectionMenu { anchor: DVec2 }`, emitted at `:660` with the button's own rect origin so
the popup drops from the button:

```rust
            if self
                .view
                .icon_button(cx, ids!(view_mode_btn))
                .clicked(actions)
            {
                let rect = self.view.icon_button(cx, ids!(view_mode_btn)).area().rect(cx);
                cx.widget_action(
                    uid,
                    ProjectTreeAction::OpenProjectionMenu {
                        anchor: DVec2 {
                            x: rect.pos.x,
                            y: rect.pos.y + rect.size.y,
                        },
                    },
                );
            }
```

Update `app/navigation.rs:670`'s push to `panel.set_projection(cx, self.projection_mask.clone(), maskable)`
where `maskable` comes from `folder_projection::maskable_names(&registry)` flattened to owned
`String`s. Update `app/actions.rs`'s `ExclusiveHandler::TreeViewModeToggle` name to
`TreeProjectionMenu` and its arm to match the new action; Task 10 fills in what it does.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p waml-editor tree_panel::`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 6: Commit**

```bash
git add -A crates/waml-editor/src
git commit -m "feat(tree): the projection glyph reports three mask states"
```

---

### Task 10: The projection popup toggles the mask

**Files:**
- Modify: `crates/waml-editor/src/app/menus.rs` (a new `projection_menu_items` builder)
- Modify: `crates/waml-editor/src/app/actions.rs` (open the sticky popup on
  `OpenProjectionMenu`; handle `PopupRootAction::Toggled`)
- Modify: `crates/waml-editor/src/app/navigation.rs` (replace `toggle_full_raw` from Task 4
  with `set_projection_mask`)
- Test: inline tests in `crates/waml-editor/src/app/menus.rs`

**Interfaces:**
- Consumes: `maskable_names` (Task 4), `PopupItem::checked` (Task 5), `open_sticky` /
  `set_items` / `PopupRootAction::Toggled` (Task 6), `OpenProjectionMenu` (Task 9).
- Produces:
  `app::menus::projection_menu_items(maskable: &[(&str, Vec<&str>)], mask: &ProjectionMask) -> Vec<PopupItem>`;
  `app::menus::projection_toggle_target(id: LiveId, maskable: &[(&str, Vec<&str>)]) -> Option<ProjectionToggle>`
  where
  ```rust
  pub enum ProjectionToggle {
      /// Every one of this extension's maskable names.
      Extension(Vec<String>),
      /// One stage.
      Stage(String),
  }
  ```
  `App::set_projection_mask(&mut self, cx: &mut Cx, mask: ProjectionMask)` — stores it, then
  `refresh_nav(cx, false)`, `refresh_folder_tabs(cx)`, `cx.redraw_all()` (the body
  `toggle_view_mode` has today).

**Row shape:**
- One row per extension owning maskable stages, `checked: Some(!every_name_masked)` — checked
  means "running".
- Its stage rows beneath, labelled with two leading spaces so the nesting reads without a new
  indent mechanism (`"  hide"`), `checked: Some(!mask.is_masked(name))`.
- Row ids are `LiveId::from_str`-derived from a stable string: `"ext:core"` /
  `"stage:hide"`. `projection_toggle_target` maps an id back by re-deriving those same ids
  from `maskable` — do NOT parse the label.
- `index` never appears (`maskable_names` already filtered it).

- [ ] **Step 1: Write the failing test**

Append to `crates/waml-editor/src/app/menus.rs`:

```rust
#[cfg(test)]
mod projection_tests {
    use super::*;
    use waml::view::mask::ProjectionMask;

    fn maskable() -> Vec<(&'static str, Vec<&'static str>)> {
        vec![("core", vec!["hide"]), ("uml", vec!["uml"])]
    }

    #[test]
    fn an_empty_mask_shows_every_row_checked() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::default());
        assert_eq!(
            items.len(),
            4,
            "two extension rows and their two stage rows",
        );
        assert!(
            items.iter().all(|item| item.checked == Some(true)),
            "checked means running, and an empty mask runs everything",
        );
    }

    #[test]
    fn masking_a_stage_unchecks_it_and_its_extension() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::from_names(["hide"]));
        let core_ext = items
            .iter()
            .find(|item| item.id == LiveId::from_str("ext:core"))
            .unwrap();
        let hide = items
            .iter()
            .find(|item| item.id == LiveId::from_str("stage:hide"))
            .unwrap();
        let uml = items
            .iter()
            .find(|item| item.id == LiveId::from_str("stage:uml"))
            .unwrap();
        assert_eq!(hide.checked, Some(false));
        assert_eq!(
            core_ext.checked,
            Some(false),
            "core's only maskable stage is off, so core reads off",
        );
        assert_eq!(uml.checked, Some(true), "another extension is untouched");
    }

    #[test]
    fn index_never_appears_as_a_row() {
        let items = projection_menu_items(&maskable(), &ProjectionMask::default());
        assert!(
            !items.iter().any(|item| item.label.trim() == "index"),
            "the terminal stage is not maskable, so offering it would be a lie",
        );
    }

    #[test]
    fn an_extension_row_resolves_to_all_of_its_names() {
        let target = projection_toggle_target(LiveId::from_str("ext:core"), &maskable());
        assert_eq!(
            target,
            Some(ProjectionToggle::Extension(vec!["hide".to_string()])),
        );
        let stage = projection_toggle_target(LiveId::from_str("stage:uml"), &maskable());
        assert_eq!(stage, Some(ProjectionToggle::Stage("uml".to_string())));
        assert_eq!(
            projection_toggle_target(LiveId::from_str("ext:nope"), &maskable()),
            None,
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor projection_tests`
Expected: FAIL — `cannot find function projection_menu_items`.

- [ ] **Step 3: Implement the builder**

In `crates/waml-editor/src/app/menus.rs`:

```rust
/// What a projection popup row toggles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectionToggle {
    /// Every one of this extension's maskable names, moved together.
    Extension(Vec<String>),
    /// One stage.
    Stage(String),
}

fn extension_row_id(owner: &str) -> LiveId {
    LiveId::from_str(&format!("ext:{owner}"))
}

fn stage_row_id(name: &str) -> LiveId {
    LiveId::from_str(&format!("stage:{name}"))
}

/// The projection checklist: one row per extension owning maskable stages,
/// its stage rows nested beneath.
///
/// CHECKED MEANS RUNNING, not masked -- the popup answers "what is on", the
/// same question the toolbar glyph answers.
///
/// Built from `folder_projection::maskable_names`, which is built from the
/// registry. Never hand-write an extension list here: two construction sites
/// that disagree are invisible.
pub fn projection_menu_items(
    maskable: &[(&str, Vec<&str>)],
    mask: &ProjectionMask,
) -> Vec<PopupItem> {
    let mut items = Vec::new();
    for (owner, names) in maskable {
        let all_masked = names.iter().all(|name| mask.is_masked(name));
        items.push(PopupItem {
            id: extension_row_id(owner),
            label: (*owner).to_string(),
            icon: None,
            danger: false,
            enabled: true,
            checked: Some(!all_masked),
        });
        for name in names {
            items.push(PopupItem {
                id: stage_row_id(name),
                // Two leading spaces read as nesting without a new indent
                // mechanism in the menu's row layout.
                label: format!("  {name}"),
                icon: None,
                danger: false,
                enabled: true,
                checked: Some(!mask.is_masked(name)),
            });
        }
    }
    items
}

/// Map a committed row id back to what it toggles, by re-deriving the same ids
/// `projection_menu_items` minted. Never parses a label.
pub fn projection_toggle_target(
    id: LiveId,
    maskable: &[(&str, Vec<&str>)],
) -> Option<ProjectionToggle> {
    for (owner, names) in maskable {
        if id == extension_row_id(owner) {
            return Some(ProjectionToggle::Extension(
                names.iter().map(|n| (*n).to_string()).collect(),
            ));
        }
        for name in names {
            if id == stage_row_id(name) {
                return Some(ProjectionToggle::Stage((*name).to_string()));
            }
        }
    }
    None
}
```

`LiveId::from_str` is the makepad hashing constructor — confirm the exact name against an
existing call in this crate before using it, and match whatever the codebase already does to
mint a runtime `LiveId` from a string.

- [ ] **Step 4: Wire it in `app/actions.rs`**

On `ProjectTreeAction::OpenProjectionMenu { anchor }`, build
`folder_projection::maskable_names(&registry)`, call `projection_menu_items`, and open the
sticky popup through `popup_root` — copy the surrounding open-dispatch idiom at
`actions.rs:394-410` verbatim, substituting the sticky open.

On `PopupRootAction::Toggled { tag, id }` for the projection tag:

```rust
    let maskable = crate::folder_projection::maskable_names(&registry);
    let Some(target) = crate::app::menus::projection_toggle_target(id, &maskable) else {
        return;
    };
    let mut mask = self.projection_mask.clone();
    match target {
        // An extension row moves all of its names together: if any is still
        // running, the row switches the whole extension off; otherwise it
        // switches the whole extension back on.
        crate::app::menus::ProjectionToggle::Extension(names) => {
            let any_running = names.iter().any(|name| !mask.is_masked(name));
            for name in &names {
                mask.set_masked(name, any_running);
            }
        }
        crate::app::menus::ProjectionToggle::Stage(name) => {
            let masked = mask.is_masked(&name);
            mask.set_masked(&name, !masked);
        }
    }
    self.set_projection_mask(cx, mask);
    // Repaint the open card's checkmarks in place -- reopening would reset the
    // anchor and drop the hover.
    let items = crate::app::menus::projection_menu_items(
        &maskable,
        &self.projection_mask,
    );
    /* menu.set_items(cx, items) through popup_root */
```

In `app/navigation.rs`, replace Task 4's `toggle_full_raw` with:

```rust
    /// Install a new session-wide projection mask.
    ///
    /// Both surfaces read the same mask, so there is no state in which the
    /// tree and a folder view disagree about what a directory contains. It
    /// lives in memory only: raw is a deliberate act, not a preference, so it
    /// is never written to `.waml/settings.json` and every launch starts with
    /// an empty mask.
    ///
    /// This is presentational. A row a masked stage would have removed is not
    /// protected by anything; masking simply asks for the listing without it.
    pub(super) fn set_projection_mask(&mut self, cx: &mut Cx, mask: ProjectionMask) {
        if self.projection_mask == mask {
            return;
        }
        self.projection_mask = mask;
        self.refresh_nav(cx, false);
        self.refresh_folder_tabs(cx);
        cx.redraw_all();
    }
```

Update `app/tests/navigation.rs:2035-2052` to drive `set_projection_mask` and assert on
`app.projection_mask` instead of `app.view_mode`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p waml-editor`
Expected: PASS.

- [ ] **Step 6: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 7: Commit**

```bash
git add -A crates/waml-editor/src
git commit -m "feat(tree): a sticky popup toggles the projection mask per extension and stage"
```

---

### Task 11: Collapse-all and expand-all toolbar buttons

**Files:**
- Modify: `crates/waml-editor/src/tree_panel.rs:200-208` (the `control_strip` DSL),
  `:651-662` (the click handling), `:718-723` (the draw seeding), the action enum at `:230`
- Test: `crates/waml-editor/src/tree_panel.rs` inline tests

**Interfaces:**
- Consumes: `ProjectTree::directory_keys: HashSet<String>` (already present, `:366`),
  `TreeLayout::set_folder_open(&mut self, key: &str, open: bool, animate: bool)` (already
  present).
- Produces:
  `ProjectTree::collapse_all(&mut self, cx: &mut Cx)`,
  `ProjectTree::expand_all(&mut self, cx: &mut Cx)`.
  No `App` involvement — folds are panel-local state, unlike the mask.

**Two buttons, not one toggle.** A partially-expanded tree has no honest value for one
toggling glyph to report — the same rule the projection glyph follows.

The strip's final left-to-right order is: projection button, `ListCollapse`, `ListExpand`,
then the existing NOOP `tidy_btn`. The strip currently sets `align: Align{x: 1.0}` (right);
the spec calls for a left-aligned cluster, so change it to `Align{x: 0.0}` and reorder the
children accordingly.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn collapse_all_closes_every_known_directory_and_expand_all_opens_them() {
        let mut cx = /* the harness the neighbouring tests use */;
        let mut panel = /* ditto */;
        panel.directory_keys.insert(k("/sales"));
        panel.directory_keys.insert(k("/ops"));
        panel.layout.set_folder_open(&k("/sales"), true, false);
        panel.layout.set_folder_open(&k("/ops"), true, false);

        panel.collapse_all(&mut cx);
        assert!(!panel.layout.is_folder_open(&k("/sales")));
        assert!(!panel.layout.is_folder_open(&k("/ops")));

        panel.expand_all(&mut cx);
        assert!(panel.layout.is_folder_open(&k("/sales")));
        assert!(
            panel.layout.is_folder_open(&k("/ops")),
            "expand-all reaches directories that were not visible while collapsed",
        );
    }
```

Copy the `cx` / `panel` / `k(...)` helpers verbatim from the neighbouring test at
`tree_panel.rs:1757`, which already inserts into `panel.directory_keys`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p waml-editor tree_panel::tests::collapse_all`
Expected: FAIL — `no method named collapse_all`.

- [ ] **Step 3: Implement**

```rust
    /// Close every directory the panel knows about.
    ///
    /// Driven off `directory_keys`, not off the visible rows: a subtree
    /// beneath a collapsed parent has no visible row, and expand-all must
    /// reach it.
    pub fn collapse_all(&mut self, cx: &mut Cx) {
        self.set_all_folds(cx, false);
    }

    pub fn expand_all(&mut self, cx: &mut Cx) {
        self.set_all_folds(cx, true);
    }

    fn set_all_folds(&mut self, cx: &mut Cx, open: bool) {
        let keys: Vec<String> = self.directory_keys.iter().cloned().collect();
        for key in keys {
            self.layout.set_folder_open(&key, open, true);
        }
        self.fold_next_frame = cx.new_next_frame();
        self.view.redraw(cx);
    }
```

DSL, replacing `control_strip`'s body:

```rust
        // The panel's controls: the projection state/menu button, the two fold
        // buttons, and the (inert) tidy button. A LEFT-aligned icon-only
        // cluster -- deliberately not the Visual Studio look, so no split
        // buttons and no caret affordances.
        control_strip := View {
            width: Fill
            height: Fit
            flow: Right
            align: Align{x: 0.0}
            padding: Inset{left: 6.0, right: 6.0, top: 6.0, bottom: 2.0}
            view_mode_btn := IconButton{ width: 28.0 height: 28.0 icon_size: 16.0 }
            collapse_all_btn := IconButton{ width: 28.0 height: 28.0 icon_size: 16.0 }
            expand_all_btn := IconButton{ width: 28.0 height: 28.0 icon_size: 16.0 }
            tidy_btn := IconButton{ width: 28.0 height: 28.0 icon_size: 16.0 }
        }
```

Seed the two new glyphs in `draw_walk` beside the existing `tidy_btn` seeding — `IconButton::icon`
is `#[rust]` and the DSL cannot supply it, so an unseeded button is a blank 28px hole:

```rust
        self.view
            .icon_button(cx, ids!(collapse_all_btn))
            .set_icon(cx, Icon::ListCollapse);
        self.view
            .icon_button(cx, ids!(expand_all_btn))
            .set_icon(cx, Icon::ListExpand);
```

Confirm `Icon::ListCollapse` / `Icon::ListExpand` exist in `icons.rs` before using them (the
spec says both are already catalogued). If either is missing, catalogue it following Task 8's
six-site procedure in THIS task, and say so in the commit body.

Handle the clicks alongside the projection button:

```rust
            if self
                .view
                .icon_button(cx, ids!(collapse_all_btn))
                .clicked(actions)
            {
                self.collapse_all(cx);
            }
            if self
                .view
                .icon_button(cx, ids!(expand_all_btn))
                .clicked(actions)
            {
                self.expand_all(cx);
            }
```

Update the `control_strip` doc comment at `:197-199` — the panel now owns four controls, not
one — and the module doc at `:15`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p waml-editor tree_panel::`
Expected: PASS.

- [ ] **Step 5: Run the gate**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

- [ ] **Step 6: Commit**

```bash
git add -A crates/waml-editor/src
git commit -m "feat(tree): collapse-all and expand-all toolbar buttons

Two buttons, not one toggle: a partially-expanded tree has no honest
state for one glyph to report."
```

---

## Deferred visual checks (owed to the user after the plan lands)

None of these can be automated. Run the editor (`scripts/run-native.ps1`, or the `run` skill)
and check by eye:

1. **Toolbar row layout.** The four-button left cluster sits under the tree header without
   crowding it, and the buttons are evenly spaced.
2. **FileTree row labels still draw.** Adding a fixed-height row above the tree list is the
   exact geometry that has previously blanked FileTree row labels (a lone fixed child filling
   a fixed parent). The strip already existed before this work, but its content changed —
   confirm rows still render their text.
3. **Popup legibility.** The extension rows and their two-space-indented stage rows read as a
   group and a nesting, not as a flat list.
4. **Checkmark glyph.** A checked row's mark is legible and does not shove the label.
5. **The three glyphs read as one control's states.** `Library` → `LibraryBig` → `Code` at
   16px, in sequence, in the toolbar.
6. **`library-big` SDF fit.** If the stroke clips or the glyph reads too small beside
   `Library` / `Code`, nudge `A`/`B` for THIS glyph in the icon harness — never the shared fit.
7. **Collapse-all / expand-all** actually fold and unfold the whole tree, including subtrees
   that were not visible when collapsed.
8. **A masked `hide`** makes hidden rows appear in both the tree AND every open folder tab,
   together.
