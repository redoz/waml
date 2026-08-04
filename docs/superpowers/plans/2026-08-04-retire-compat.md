# Retire `compat` — implementation plan

Implements the approved spec `docs/superpowers/specs/2026-08-04-retire-compat-design.md`.

## Context / Goal

`crates/waml/src/compat.rs` (726 lines) holds the *current* mixed-domain edit engine
(`Step`, `Batch`, `apply`, `MixedLoweringCursor`, the `CandidateInvalidation`
propagation machinery, the `EditBatch` impl for `Batch`) mislabeled as a deprecated
adapter, plus a genuine legacy bridge (`TryFrom<crate::ops::Op> for Step`,
`step_from_legacy`) whose only callers are the `waml::ops::apply`/`apply_source`
shims and their ~1470 lines of tests.

Goal: the mixed batch moves to `edit` as its permanent, documented home with the
propagation seam formalized (`edit::Invalidation` + `edit::InvalidationSink`);
`NameSpec`/`FieldEdit`/`DiagramDisplaySet` move into `uml`; `OpError` moves into
`edit` renamed `EditError`; `waml::ops` and `compat.rs` are deleted;
`OpDto::to_compat_step`/`from_compat_step` become `to_step`/`from_step`; all legacy
tests are ported 1:1 to `Step`/`Batch` + `edit::apply`. **Zero behaviour change;
DTO wire format untouched.**

Strategy: introduce new homes with re-exports from the old paths first, migrate
callers, port tests, delete old paths last. `waml::ops` items are all `pub` in a
lib crate, so they never trip `dead_code` while awaiting deletion; `step_from_legacy`
(`pub(crate)`) stays referenced by `pub fn ops::apply_source` until both die together
in the final task. The gate after every task: `cargo test --workspace`,
clippy `-D warnings`, `cargo fmt --all --check`, vscode extension checks.

Verified call-site inventory (all paths absolute from repo root `C:\dev\waml`):

- `crates/waml/src/compat.rs` — everything listed above; `directory()` helper is used
  only by the legacy `TryFrom` bridge; `validate_context` emits op string
  `"compat.context"` (asserted nowhere — safe to rename to `"edit.context"`).
- `crates/waml/src/edit.rs` — `pub type EditError = crate::ops::OpError;` (L11) plus
  `Display`/`Error` impls for `crate::ops::OpError`; `EditBatch`, `EditContext`,
  `sealed`, `AppliedEdit`, `PendingEdit`, `SequenceBatch`, `ExactSourceEdit`,
  `mod reversible`.
- `crates/waml/src/ops/mod.rs` — `Bundle`, `OpError` (+ `at`/`with_sel`), `NameSpec`,
  `FieldEdit` (+ serde impls), `DiagramDisplaySet`, legacy `Op` enum,
  `apply`/`apply_source`, `referrers` wrapper (delegates to
  `crate::uml::lower::referrers`), `pub mod selector` re-export +
  `pub use selector::{parse_selector, render_selector, RelBy, Selector}`, tests
  L247–1716.
- `crates/waml/src/uml.rs` L13: `pub use crate::ops::{DiagramDisplaySet, FieldEdit, NameSpec};`
- In-crate legacy-test users of `crate::ops::{apply, Op}`: `crates/waml/src/okf/lower.rs`
  tests (L926+, also `crate::ops::Bundle` at L1161/1198/1217/1242/1272/1295) and
  `crates/waml/src/uml/rename.rs` tests (L289+).
- `crates/waml/tests/ops_golden.rs` — `use waml::ops::{apply, Op};`
- `crates/waml/tests/compat_lowering_order.rs` — `use waml::{compat::{Batch, Step}, …}`.
- `crates/waml/tests/uml_lowering_authority.rs` L10 — `include_str!("../src/compat.rs")`
  source-scan guard; must follow the mixed-lowering code to its new file.
- `crates/waml-ops-dto/src/lib.rs` — L2 `use waml::compat::{Batch, Step};`, L9
  `use waml::ops::{DiagramDisplaySet, FieldEdit, NameSpec, RelBy};`, `to_batch` L387,
  `to_compat_step` L399, `from_compat_step` L676; tests use `waml::compat::apply`
  (L1117/1154/1189/1211/1454) and `waml::ops::{FieldEdit, Op}` / `{NameSpec, Op, RelBy, Selector}`
  (L954, L1226).
- `crates/waml-cli/src/main.rs` — L8 `use waml::ops::FieldEdit;`, `run_batch` L699
  takes `waml::compat::Batch`. No other `waml-cli` file touches `ops`/`compat`.
- `crates/waml-editor/src/editor_session.rs` tests build `waml::compat::Batch`/`Step`
  at L2849–2861, L2903–2911, L3127–3135; `class_diagram_view.rs` L1043 and
  `diagram_properties.rs` L10/L709 use `waml::ops::DiagramDisplaySet`.
- Domain hook methods for the sink impls (already exist):
  `crates/waml/src/okf/lower.rs` `OkfLoweringState::{invalidate_text L39, inserted L43, removed L54, renamed L59}`;
  `crates/waml/src/uml/lower.rs` `UmlLoweringState::{invalidate_text L61, inserted_concept L65, removed_concept L86, renamed_concept L92}`.

### Legacy `Op` → `Step` porting table (from the verified `TryFrom` impl, compat.rs L48–240)

| Legacy `Op` | `Step` |
|---|---|
| `AttrAdd{node,name,ty_token,multiplicity,visibility}` | `Step::Uml(uml::Op::AttributeAdd{…same fields…})` |
| `AttrSet{…,rename}` | `Step::Uml(uml::Op::AttributeSet{…same fields…})` |
| `AttrRm{node,name}` | `Step::Uml(uml::Op::AttributeRemove{node,name})` |
| `ValueAdd`/`ValueRm{node,literal}` | `Step::Uml(uml::Op::ValueAdd/ValueRemove{node,literal})` |
| `RelAdd{source,kind,target,name,ends}` | `Step::Uml(uml::Op::RelationshipAdd{…})` |
| `RelSet{selector,ends,name}` | `Step::Uml(uml::Op::RelationshipSet{selector: RelationshipSelector::try_from(selector).unwrap(), ends, name})` |
| `RelRm{selector}` | `Step::Uml(uml::Op::RelationshipRemove{selector: …try_from…})` |
| `NodeNew{slug,dir,ty,title,stereotype,description,abstract_}` | `Step::Uml(uml::Op::ClassifierNew{slug, directory: DirectoryAddress::parse("/<dir>")…, ty, title, stereotype, description, abstract_})` |
| `NodeSet{slug,…}` | `Step::Uml(uml::Op::ClassifierSet{id: slug, …})` |
| `NodeRm{slug,cascade}` | `Step::Uml(uml::Op::ClassifierRemove{id: slug, cascade})` |
| `NodeRename{from,to}` | `Step::Uml(uml::Op::ClassifierRename{from,to})` |
| `PkgMove{slug,to_dir}` | `Step::Okf(okf::Op::ConceptMove{id: slug, to_directory: dir(to_dir)})` |
| `PkgRename{from,to}` (parent differs) | `Step::Okf(okf::Op::DirectoryMove{directory: dir(from), to_parent: dir(parent-of-to), name: Some(last-segment-of-to)})` |
| `PkgRename{from,to}` (same parent) | `Step::Okf(okf::Op::DirectoryRename{directory: dir(from), name: last-segment-of-to})` |
| `PkgDelete{path,cascade}` | `Step::Okf(okf::Op::DirectoryDelete{directory,cascade})` |
| `PkgReorder{path,order}` | `Step::Okf(okf::Op::IndexReorder{directory,order})` |
| `PkgSort{path}` | `Step::Okf(okf::Op::IndexSort{directory})` |
| `PkgRetitle{path,title}` | `Step::Okf(okf::Op::IndexRetitle{directory,title})` |
| `PkgInsert{parent_path,name,docs}` | `Step::Okf(okf::Op::BundleImport{parent, name, bundle: SourceBundle::try_from_pairs(docs)…})` |
| `DiagramSet{key,title,description,clear_description,display}` | `Step::Uml(uml::Op::DiagramSet{…same fields…})` |
| `PlaceSet{diagram,subject_title,subject_slug,reference_title,reference_slug,directions}` | `Step::Uml(uml::Op::PlacementSet{…same fields…})` |
| `PlaceRm{diagram,subject_slug,reference_slug}` | `Step::Uml(uml::Op::PlacementRemove{…same fields…})` |

where `dir(p)` = `okf::DirectoryAddress::parse(if trimmed.is_empty() { "/" } else { format!("/{p}") })` (trim `/`).

---

### Task 1: Move the mixed batch into `edit` and formalize the invalidation seam

Files: `crates/waml/src/edit.rs` → `crates/waml/src/edit/mod.rs` (git mv),
new `crates/waml/src/edit/batch.rs`, `crates/waml/src/compat.rs`,
`crates/waml/src/okf/lower.rs`, `crates/waml/src/uml/lower.rs`,
`crates/waml/tests/uml_lowering_authority.rs`.

Steps:
1. `git mv crates/waml/src/edit.rs crates/waml/src/edit/mod.rs` and
   `git mv crates/waml/src/edit/reversible.rs` is NOT needed — `reversible.rs` already
   lives at `crates/waml/src/edit/reversible.rs`; verify with
   `ls crates/waml/src/edit/` after the move and leave `mod reversible;` as is.
2. Create `crates/waml/src/edit/batch.rs`. Move from `compat.rs`, verbatim except
   as noted: `Step`, `Batch` (+ `new`/`steps`), `apply`, `MixedLoweringCursor`
   (+ `StepFamily`), `snapshot`, `claimed_id`, `invalidations`,
   `impl crate::edit::sealed::Sealed for Batch`, `impl crate::edit::EditBatch for Batch`,
   and the two mixed-batch tests (`mixed_okf_uml_batch_round_trips_as_one_transaction`,
   `late_mixed_batch_failure_publishes_no_source_or_inverse`) into a
   `#[cfg(test)] mod tests` in `batch.rs`. Do NOT move: `directory()`,
   `TryFrom<crate::ops::Op> for Step`, `step_from_legacy`, or the four legacy tests —
   those stay in `compat.rs` for now.
3. In `edit/mod.rs` add `mod batch;` and
   `pub use batch::{apply, Batch, Step};` (also `pub use batch::{Invalidation};` per
   step 4). Drop `#[doc(hidden)]` on all moved items and write real doc comments
   naming `waml::edit::{Step, Batch, apply}` the public edit surface.
4. Formalize the seam (behaviour identical):
   - Rename `CandidateInvalidation` → `pub enum Invalidation` with the same four
     variants and payloads (`TextChanged(BundlePath)`, `Inserted{id, path}`,
     `Removed{id, path}`, `Renamed{id_from, id_to, from, to}`); make the payload
     fields `pub`. Define it in `edit/batch.rs` (or `edit/mod.rs`) and re-export as
     `waml::edit::Invalidation`.
   - Add `pub trait InvalidationSink { fn absorb(&mut self, event: &Invalidation) -> Result<(), EditError>; }`
     in `edit` (exported as `waml::edit::InvalidationSink`).
   - In `crates/waml/src/okf/lower.rs`: `impl crate::edit::InvalidationSink for OkfLoweringState`
     whose `absorb` body is exactly today's `propagate_to_okf` match
     (`invalidate_text` / `inserted` / `removed` / `renamed`, compat.rs L363–373).
   - In `crates/waml/src/uml/lower.rs`: `impl crate::edit::InvalidationSink for UmlLoweringState`
     whose `absorb` body is exactly today's `propagate_to_uml` match
     (compat.rs L375–412: `invalidate_text` / `inserted_concept` / `removed_concept` /
     `renamed_concept` with the four-way `(id_from, id_to)` match).
   - In `MixedLoweringCursor`, delete `propagate`, `propagate_from`,
     `propagate_to_okf`, `propagate_to_uml`; replace with one routing method that,
     for each event, visits both `(StepFamily::Okf, &mut self.okf)` and
     `(StepFamily::Uml, &mut self.uml)` as `&mut dyn InvalidationSink` and calls
     `sink.absorb(&event)` when the sink's family differs from the originating
     step's family OR the event is `Invalidation::TextChanged(_)`. This is exactly
     today's `propagate_from` rule expressed once. Index-stamping of errors in
     `MixedLoweringCursor::apply` is unchanged.
5. Rename the `validate_context` op string `"compat.context"` → `"edit.context"`
   (grep-verified: no test or caller asserts the old string).
6. Shrink `compat.rs` to the legacy bridge only: keep `directory()`,
   `TryFrom<crate::ops::Op> for Step`, `step_from_legacy`, the four legacy tests,
   and add `pub use crate::edit::{apply, Batch, Step};` so every `waml::compat::*`
   caller (dto, cli, editor, `tests/compat_lowering_order.rs`) stays green. Trim
   now-unused imports (unused imports are denied warnings).
7. In `crates/waml/tests/uml_lowering_authority.rs` replace the
   `("compat.rs", include_str!("../src/compat.rs"))` entry with
   `("edit/batch.rs", include_str!("../src/edit/batch.rs"))` — the guard follows the
   mixed lowering path; keep the compat entry too only if compat.rs still contains
   lowering-adjacent code (it does not after this task; replacing is correct).

Verification: `cargo test --workspace` && `cargo clippy --workspace --all-targets -- -D warnings` && `cargo fmt --all --check`.

### Task 2: Move value types into `uml`; move `OpError` into `edit` as `EditError`

Files: `crates/waml/src/ops/mod.rs`, `crates/waml/src/uml/ops.rs`,
`crates/waml/src/uml.rs`, `crates/waml/src/edit/mod.rs`.

Steps:
1. Move `NameSpec`, `FieldEdit` (with its inherent `is_unchanged` and both
   `#[cfg(feature = "serde")]` Serialize/Deserialize impls, ops/mod.rs L32–78) and
   `DiagramDisplaySet` (L80–96) into `crates/waml/src/uml/ops.rs` (they parameterize
   `uml::Op`), keeping doc comments verbatim.
2. In `crates/waml/src/uml.rs` replace L13
   `pub use crate::ops::{DiagramDisplaySet, FieldEdit, NameSpec};` with
   `pub use ops::{DiagramDisplaySet, FieldEdit, NameSpec};` (merge into the existing
   L15 `pub use ops::{Batch, Op};` if clippy prefers). Public path
   `waml::uml::NameSpec` etc. is unchanged.
3. In `crates/waml/src/ops/mod.rs` replace the moved definitions with
   `pub use crate::uml::{DiagramDisplaySet, FieldEdit, NameSpec};` so
   `waml::ops::…` importers stay green until Task 8.
4. Move the `OpError` struct (fields `index`, `op`, `selector`, `reason` unchanged)
   plus its `at`/`with_sel` inherent impl from `ops/mod.rs` into
   `crates/waml/src/edit/mod.rs`, renamed `EditError`. Delete the old
   `pub type EditError = crate::ops::OpError;` (edit/mod.rs L11) and retarget the
   existing `Display`/`std::error::Error` impls (edit/mod.rs L13–23) at the struct.
   In `ops/mod.rs` add the flipped alias `pub type OpError = crate::edit::EditError;`
   so `ops` signatures and any `waml::ops::OpError` importers stay green. All in-crate
   uses already go through `EditError`/`EditError::at`, which keep compiling unchanged.

Verification: full gate as in Task 1 (`cargo test --workspace`, clippy `-D warnings`, fmt check).

### Task 3: Re-point `waml-ops-dto` and rename the DTO seam to `to_step`/`from_step`

Files: `crates/waml-ops-dto/src/lib.rs`, `crates/waml-cli/src/main.rs`.

Steps:
1. In `waml-ops-dto/src/lib.rs`: change L2 to `use waml::edit::{Batch, Step};` and
   L9 to `use waml::uml::{DiagramDisplaySet, FieldEdit, NameSpec};` plus
   `use waml::uml::selector::RelBy;` (`RelBy` is not re-exported from `uml.rs`; the
   `uml::selector` module is `pub` — verified at `crates/waml/src/uml/selector.rs:26`).
2. Rename `OpDto::to_compat_step` (L399) → `to_step` and `OpDto::from_compat_step`
   (L676) → `from_step` (return/parameter types are already the `Step` now imported
   from `waml::edit`). Update all internal callers: `to_batch` L391 and every test
   call site (L958, L1029, L1046, L1057, L1079/1082, L1375/1378, L1396/1400, L1416,
   L1435/1438, L1483, L1516/1520); rename test
   `from_compat_step_round_trips_through_wire` → `from_step_round_trips_through_wire`.
3. In the dto tests replace `waml::compat::apply` (L1117, L1154, L1189, L1211, L1454)
   with `waml::edit::apply`. Leave the two legacy-`Op` dto tests that import
   `waml::ops::{FieldEdit, Op}` (L954) and `waml::ops::{NameSpec, Op, RelBy, Selector}`
   (L1226) importing `waml::ops` for now ONLY if they construct legacy `Op` values;
   inspect them — if they merely use the value types, re-point those imports to
   `waml::uml` now; if they construct legacy `Op` literals, port them to
   `Step` literals per the table above in this task (they are dto-local, small, and
   the wire assertions are untouched).
4. `waml-cli/src/main.rs`: L8 `use waml::ops::FieldEdit;` → `use waml::uml::FieldEdit;`
   and `run_batch` (L699) signature `waml::compat::Batch` → `waml::edit::Batch`.
   `to_batch` callers (L689, L805) are untouched — `to_batch`'s return type already
   resolves to the same `Batch`.
5. Grep the workspace for `to_compat_step|from_compat_step` — zero hits must remain
   (verified today: only dto-internal callers exist; `waml-cli`/LSP go through
   `to_batch`).

Verification: full gate; additionally `rg "to_compat_step|from_compat_step|waml::compat" crates/waml-ops-dto crates/waml-cli` returns nothing.

### Task 4: Re-point `waml-editor` and `tests/compat_lowering_order.rs`

Files: `crates/waml-editor/src/editor_session.rs`,
`crates/waml-editor/src/class_diagram_view.rs`,
`crates/waml-editor/src/diagram_properties.rs`,
`crates/waml/tests/compat_lowering_order.rs` (rename to
`crates/waml/tests/edit_lowering_order.rs`).

Steps:
1. `editor_session.rs`: replace every `waml::compat::Batch` / `waml::compat::Step`
   (L2849–2861, L2903–2911, L3127–3135) with `waml::edit::Batch` / `waml::edit::Step`.
2. `class_diagram_view.rs` L1043 and `diagram_properties.rs` L10 + L709:
   `use waml::ops::DiagramDisplaySet;` → `use waml::uml::DiagramDisplaySet;`.
3. `git mv crates/waml/tests/compat_lowering_order.rs crates/waml/tests/edit_lowering_order.rs`
   and change its import block from `compat::{Batch, Step}` to
   `edit::{Batch, EditBatch, EditContext, Step}` (it already imports
   `edit::{EditBatch, EditContext}`; merge into one line).
4. Grep: `rg "waml::compat" crates/` must now hit nothing outside `crates/waml` itself.

Verification: full gate (workspace tests + clippy `-D warnings` + fmt + vscode extension checks).

### Task 5: Port legacy op tests, part 1 — attr/value/rel (ops/mod.rs L247–L821)

Files: new `crates/waml/src/edit/port_tests.rs` (declared
`#[cfg(test)] mod port_tests;` in `edit/mod.rs`), `crates/waml/src/ops/mod.rs`.

Steps:
1. Create `crates/waml/src/edit/port_tests.rs` with a local helper mirroring the old
   `ops::apply` pair-based signature so assertions port verbatim:
   ```rust
   fn apply(bundle: &[(String, String)], steps: Vec<Step>) -> Result<Vec<(String, String)>, EditError> {
       let source = SourceBundle::try_from_pairs(bundle.iter().cloned())
           .map_err(|error| EditError::at("bundle", error.to_string()))?;
       crate::edit::apply(&source, &Batch::new(steps)).map(|bundle| bundle.to_pairs())
   }
   ```
   Also port the shared helpers `projection`, `layout_statement_count`, and
   `attr_add` (retyped to return `Step`) from ops/mod.rs L255–280. Keep the old
   tests' use of `crate::uml::lower::slug_of` — it is `pub(crate)`, which is why
   this module must live in-crate rather than under `tests/`.
2. Move and mechanically port every test from ops/mod.rs L282–L821 (from
   `retitle_changes_index_content_without_changing_child_paths` through
   `rel_matches_ref_named_selector`): each legacy `Op` literal becomes the `Step`
   from the porting table; `Selector`/`RelBy` imports come from
   `crate::uml::selector`; assertions unchanged. `RelSet`/`RelRm` selectors become
   `uml::RelationshipSelector::try_from(selector).unwrap()` — for error-path tests
   that relied on non-relationship selectors being rejected at conversion, assert on
   the `try_from` error instead, preserving coverage 1:1.
3. Delete the ported tests (and only those) from `ops/mod.rs`; leave the remaining
   test ranges and shared helpers they still need in place.

Verification: full gate; test count must not drop (compare
`cargo test -p waml -- --list | wc -l` before/after, minus exact renames).

### Task 6: Port legacy op tests, part 2 — node/pkg/referrers/full-path-id, plus in-crate okf/uml legacy tests

Files: `crates/waml/src/edit/port_tests.rs`, `crates/waml/src/ops/mod.rs`,
`crates/waml/src/okf/lower.rs`, `crates/waml/src/uml/rename.rs`.

Steps:
1. Port ops/mod.rs tests L822–L1153 (from
   `node_new_writes_frontmatter_and_title_and_refuses_dup` through
   `rel_rm_resolves_endpoint_target_addressed_by_full_path_id`) into
   `edit/port_tests.rs` per the table. The two `referrers_*` tests call the legacy
   `ops::referrers` wrapper — port them to call `crate::uml::lower::referrers`
   directly (identical body, verified at uml/lower.rs L1635). `crate::ops::Bundle`
   annotations become `Vec<(String, String)>`.
2. `okf/lower.rs` tests module (L926+): replace `use crate::ops::{apply, Op};` with
   `use crate::edit::{Batch, Step};` plus a tiny local `apply` helper identical to
   Task 5's, and port each `Op::Pkg*` literal to its `Step::Okf(okf::Op::…)` per the
   table (`PkgMove`→`ConceptMove`, `PkgRename`→`DirectoryRename`/`DirectoryMove`,
   `PkgDelete`→`DirectoryDelete`, `PkgReorder`/`PkgSort`/`PkgRetitle`→`Index*`,
   `PkgInsert`→`BundleImport`); `crate::ops::Bundle` → `Vec<(String, String)>`.
3. `uml/rename.rs` tests module (L289+): same treatment; `Op::NodeRename` →
   `Step::Uml(uml::Op::ClassifierRename{from, to})`.
4. Delete the ported ranges from `ops/mod.rs`.

Verification: full gate; `rg "crate::ops" crates/waml/src/okf crates/waml/src/uml` returns only the `uml.rs`-era re-export if any remains (it must not — Task 2 removed it).

### Task 7: Port legacy op tests, part 3 — diagram/placement, golden test, compat's legacy tests

Files: `crates/waml/src/edit/port_tests.rs`, `crates/waml/src/ops/mod.rs`,
`crates/waml/tests/ops_golden.rs` (rename to `crates/waml/tests/edit_golden.rs`),
`crates/waml/src/compat.rs`, `crates/waml/src/edit/batch.rs`.

Steps:
1. Port ops/mod.rs tests L1154–end (helpers `diagram_doc`, `full_display`,
   `layout_diagram`, `diagram_no_layout`, `placeset`, `placerm`, and all
   `diagram_set_*` / `place_set_*` / `place_rm_*` tests) into `edit/port_tests.rs`
   per the table (`DiagramSet`/`PlaceSet`/`PlaceRm` map field-for-field to
   `uml::Op::DiagramSet`/`PlacementSet`/`PlacementRemove`). After this the
   `ops/mod.rs` `#[cfg(test)] mod tests` is empty — delete the module.
2. `git mv crates/waml/tests/ops_golden.rs crates/waml/tests/edit_golden.rs`; replace
   `use waml::ops::{apply, Op};` with `waml::edit::{apply, Batch, Step}` + a local
   pairs helper (public API only: `SourceBundle::try_from_pairs` + `to_pairs`), and
   port its three tests (`NodeRename` → `ClassifierRename`, `PkgRetitle` →
   `IndexRetitle`). Golden fixture `tests/fixtures/orders-domain.md` untouched.
3. Port compat.rs's four remaining legacy tests into `edit/batch.rs`'s test module,
   preserving coverage 1:1 without the bridge:
   - `malformed_legacy_directories_return_errors_instead_of_panicking` → assert
     `okf::DirectoryAddress::parse("/../escape")` errors (that is what the bridge's
     `directory()` surfaced); keep an `edit::apply` variant with a valid
     `IndexRetitle` only if it adds coverage — the essential assertion is the
     malformed-directory rejection.
   - `malformed_legacy_import_bundle_returns_error` → build
     `Step::Okf(okf::Op::BundleImport{…})` whose bundle comes from
     `SourceBundle::try_from_pairs` with duplicate paths and assert the error
     (assert `try_from_pairs` errors, mirroring the bridge's `pkg.insert` mapping).
   - `legacy_directory_rename_preserves_combined_move_and_rename` and
     `combined_rename_ignores_occupied_intermediate_destination` → apply
     `Step::Okf(okf::Op::DirectoryMove{directory: dir("/domains/sales"), to_parent: dir("/archive"), name: Some("commerce")})`
     via `edit::apply` with the same fixtures and assertions — this is exactly what
     `PkgRename` lowered to for a cross-parent rename.
   After this, compat.rs's test module is gone.
4. Do NOT delete the bridge itself yet — `ops::apply_source` (pub) still calls
   `step_from_legacy`, keeping it referenced; both die in Task 8.

Verification: full gate; `crates/waml/src/ops/mod.rs` now contains no `#[cfg(test)]` code; ported diagram/place tests all pass.

### Task 8: Delete `waml::ops`, the legacy bridge, and `compat.rs`

Files: `crates/waml/src/ops/mod.rs` (delete directory `crates/waml/src/ops/`),
`crates/waml/src/compat.rs` (delete), `crates/waml/src/lib.rs`.

Steps:
1. Verify no remaining references first:
   `rg "waml::ops|crate::ops|waml::compat|crate::compat" crates/` must hit only
   `ops/mod.rs`, `compat.rs`, `lib.rs`, and the stale comment at
   `crates/waml-editor/src/app/actions.rs:1268` — reword that comment
   (`waml::ops` → `waml::edit`).
2. Delete `crates/waml/src/ops/` entirely: legacy `Op` enum, `apply`, `apply_source`,
   `Bundle` alias, the `referrers` wrapper, the `pub mod selector` re-export and its
   `pub use`, the `pub use crate::uml::{…}` and `pub type OpError` shims from Task 2.
3. Delete `crates/waml/src/compat.rs` (only the bridge — `directory()`,
   `TryFrom<crate::ops::Op> for Step`, `step_from_legacy` — and the
   `pub use crate::edit::…` shim remain in it by now).
4. In `crates/waml/src/lib.rs` remove `pub mod ops;` (L16) and `pub mod compat;`
   (L27, including its preceding comment if any).
5. Final doc sweep: confirm no `#[doc(hidden)]` survives on `edit::{Step, Batch, apply,
   Invalidation, InvalidationSink}` and module docs for `edit` describe the layering
   (okf::ops::Batch / uml::ops::Batch under the edit composition layer).

Verification: full gate green; `rg "OpError|step_from_legacy|apply_source\b|waml::ops|waml::compat" crates/` returns nothing (except unrelated words); vscode extension checks pass.
