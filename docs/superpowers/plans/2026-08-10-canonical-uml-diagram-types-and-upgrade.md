# Canonical UML Diagram Types and Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the five canonical UML diagram document types the only accepted view types, and add an atomic `waml upgrade` command for old documents.

**Architecture:** Keep semantic UML node types separate from diagram view types. A migration-only reader resolves old diagram documents before the strict reader rejects them. The CLI runs an ordered migration registry in memory, validates the complete candidate with the strict pipeline, and then uses the existing atomic writer.

**Tech Stack:** Rust, `waml`, `waml-cli`, `waml-editor`, `waml-ops-dto`, `waml-syntax`, Cargo integration tests, Markdown fixtures.

## Global Constraints

- Work only in `C:/dev/waml/.worktrees/use-case-diagram-rendering`.
- Use ASD-STE100 Simplified Technical English in code messages and documentation.
- Prefix every shell command with `rtk`.
- Use test-driven development. Run each red test before implementation.
- Do not add compatibility aliases to the normal parser or analyzer.
- Accept only `uml.ClassDiagram`, `uml.UseCaseDiagram`, `uml.ActivityDiagram`, `uml.StateMachineDiagram`, and `uml.SequenceDiagram` as diagram document types.
- Keep semantic behavior types internal. Do not classify them as document views.
- Preserve all bytes outside the changed frontmatter scalar.
- Build and validate all changed files in memory before the first write.
- Use the existing journaled writer for the commit. A pre-commit failure must change no file.
- Keep each migration id stable and each transformation idempotent.
- Do not change the approved design specification.

---

### Task 1: Add the canonical diagram type vocabulary

**Files:**
- Modify: `crates/waml/src/model.rs`
- Test: `crates/waml/tests/uml_diagram_syntax.rs`
- Test: `crates/waml/tests/serde_shape.rs`

- [ ] Add a red round-trip test for all five canonical names in `uml_diagram_syntax.rs`.

```rust
#[test]
fn canonical_diagram_kinds_round_trip() {
    let cases = [
        ("uml.ClassDiagram", DiagramKind::Class),
        ("uml.UseCaseDiagram", DiagramKind::UseCase),
        ("uml.ActivityDiagram", DiagramKind::Activity),
        ("uml.StateMachineDiagram", DiagramKind::StateMachine),
        ("uml.SequenceDiagram", DiagramKind::Sequence),
    ];
    for (name, kind) in cases {
        assert_eq!(DiagramKind::parse(name), Some(kind));
        assert_eq!(kind.as_str(), name);
    }
}
```

- [ ] Run `rtk cargo test -p waml --test uml_diagram_syntax canonical_diagram_kinds_round_trip -- --exact`.
  Expected result: compilation fails because `DiagramKind` does not exist.
- [ ] Add this interface to `model.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DiagramKind {
    Class,
    UseCase,
    Activity,
    StateMachine,
    Sequence,
}

impl DiagramKind {
    pub fn parse(value: &str) -> Option<Self>;
    pub const fn as_str(self) -> &'static str;
    pub const fn behavior_kind(self) -> Option<BehaviorKind>;
}
```

- [ ] Make `parse` and `as_str` use only the five canonical strings in the goal.
- [ ] Add a serde-shape test that serializes each kind to its Rust enum name. Do not change any existing field representation while adding the new kind field in Task 2.
- [ ] Run `rtk cargo test -p waml --test uml_diagram_syntax canonical_diagram_kinds_round_trip -- --exact` and `rtk cargo test -p waml --test serde_shape`.
  Expected result: both commands pass.
- [ ] Commit with `rtk git add crates/waml/src/model.rs crates/waml/tests/uml_diagram_syntax.rs crates/waml/tests/serde_shape.rs` and `rtk git commit -m "feat(model): add canonical diagram kinds"`.

### Task 2: Separate semantic types from diagram view types

**Files:**
- Modify: `crates/waml/src/model.rs`
- Modify: `crates/waml/src/uml.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Modify: `crates/waml/src/diagnostic.rs`
- Test: `crates/waml/tests/uml_classifier_syntax.rs`
- Test: `crates/waml/tests/uml_behavior_syntax.rs`
- Test: `crates/waml/tests/uml_diagram_syntax.rs`
- Test: `crates/waml/tests/semantic_diagnostics.rs`

- [ ] Add red tests that prove these rules:
  - `ElementType::Behavior` is a semantic classifier and is not a view.
  - `ElementType::Diagram(DiagramKind)` is a view and is not a classifier.
  - The UML recognizer claims the five canonical diagram names.
  - The UML recognizer does not claim `Diagram`, `uml.Activity`, `uml.StateMachine`, or `uml.Sequence` as view types.
  - Each old document type produces an error diagnostic with the canonical replacement or the `waml upgrade` instruction.
  - An unrelated unknown OKF type stays a warning.
- [ ] Run `rtk cargo test -p waml --test uml_diagram_syntax --test uml_behavior_syntax --test semantic_diagnostics`.
  Expected result: the new assertions fail because views still use the old variants.
- [ ] Change the model interfaces to:

```rust
pub enum ElementType {
    Uml(UmlMetaclass),
    Behavior(BehaviorKind),
    Diagram(DiagramKind),
    Unknown(String),
}

pub struct Diagram {
    pub key: String,
    pub title: String,
    pub kind: DiagramKind,
    pub profile: String,
    pub description: Option<String>,
    pub groups: Vec<DiagramGroup>,
    pub layout: Vec<crate::layout::LayoutStatement>,
    pub display: DiagramDisplay,
}
```

- [ ] Add only `kind` to `Diagram`. Preserve the existing `profile`, `description`, `groups`, `layout`, and `display` types and serde behavior.

- [ ] Make `ElementType::is_view` return true only for `ElementType::Diagram(_)`. Keep `ElementType::Behavior(_)` in `is_classifier`.
- [ ] Add `DiagCode::ObsoleteDiagramType` with slug `obsolete-diagram-type`. Keep its default severity at error.
- [ ] In normal analysis, lower canonical activity and state-machine documents to `FlowDoc`, lower the canonical sequence document to `SequenceDoc`, and lower class and use-case documents to `Diagram` with their exact `DiagramKind`.
- [ ] Detect only the four old type strings before normal projection. Use these messages:

```text
obsolete diagram type 'uml.Activity'; use 'uml.ActivityDiagram' or run 'waml upgrade'
obsolete diagram type 'uml.StateMachine'; use 'uml.StateMachineDiagram' or run 'waml upgrade'
obsolete diagram type 'uml.Sequence'; use 'uml.SequenceDiagram' or run 'waml upgrade'
obsolete diagram type 'Diagram'; run 'waml upgrade' to select 'uml.ClassDiagram' or 'uml.UseCaseDiagram'
```

- [ ] Attach each diagnostic to the frontmatter `type` scalar range.
- [ ] Emit one obsolete-type error for an old document root. Do not also emit the generic unknown-type warning for the same scalar.
- [ ] Run `rtk cargo test -p waml --test uml_classifier_syntax --test uml_behavior_syntax --test uml_diagram_syntax --test semantic_diagnostics`.
  Expected result: all commands pass, and no old type is accepted as a normal diagram view.
- [ ] Commit with `rtk git add crates/waml/src/model.rs crates/waml/src/uml.rs crates/waml/src/uml/analysis.rs crates/waml/src/diagnostic.rs crates/waml/tests/uml_classifier_syntax.rs crates/waml/tests/uml_behavior_syntax.rs crates/waml/tests/uml_diagram_syntax.rs crates/waml/tests/semantic_diagnostics.rs` and `rtk git commit -m "feat(uml): require canonical diagram document types"`.

### Task 3: Use canonical kinds at all creation and dispatch points

**Files:**
- Modify: `crates/waml/src/seed.rs`
- Modify: `crates/waml-editor/src/uml_documents.rs`
- Modify: `crates/waml-editor/src/document_host.rs`
- Modify: `crates/waml-editor/src/doc_view.rs`
- Modify: `crates/waml-editor/src/class_diagram_view.rs`
- Modify: `crates/waml-editor/src/app/actions.rs`
- Modify: `crates/waml-editor/src/app/tests/navigation.rs`
- Modify: `crates/waml-editor/src/documents.rs`
- Modify: `crates/waml-editor/src/editor_session/tests.rs`
- Modify: `crates/waml-editor/src/inspector_panel.rs`
- Modify: `crates/waml-editor/src/behavior_doc_view.rs`
- Modify: `crates/waml-ops-dto/src/lib.rs`
- Test: `crates/waml/tests/uml_diagram_syntax.rs`

- [ ] Add red seed tests for `class`, `domain`, `usecase`, `activity`, `state-machine`, and `sequence`.
- [ ] Add a red editor navigation test that opens an empty `uml.UseCaseDiagram` by its declared kind. It must not inspect members to select the view.
- [ ] Run `rtk cargo test -p waml seed::tests` and `rtk cargo test -p waml-editor navigation`.
  Expected result: generated headers or editor dispatch still use old names.
- [ ] Make `new_diagram_doc` emit this exact mapping:

```text
class, domain   -> uml.ClassDiagram
usecase         -> uml.UseCaseDiagram
activity        -> uml.ActivityDiagram
state-machine   -> uml.StateMachineDiagram
sequence        -> uml.SequenceDiagram
```

- [ ] Pass the declared `DiagramKind` through document navigation and view identity. Do not infer a kind from members.
- [ ] Use `DiagramKind::Activity`, `StateMachine`, and `Sequence` to select the existing behavior views.
- [ ] Use `DiagramKind::Class` and `UseCase` to select the shared structural surface. Keep their identities distinct so an empty use-case view cannot reuse class-only state.
- [ ] Update DTO and editor test literals to use canonical names.
- [ ] Run `rtk cargo test -p waml seed::tests`, `rtk cargo test -p waml-editor navigation`, and `rtk cargo test -p waml-ops-dto`.
  Expected result: all commands pass.
- [ ] Commit with `rtk git add crates/waml/src/seed.rs crates/waml-editor/src crates/waml-ops-dto/src/lib.rs crates/waml/tests/uml_diagram_syntax.rs` and `rtk git commit -m "feat(editor): dispatch canonical diagram kinds"`.

### Task 4: Add a migration-only legacy reader and byte-safe rewrite

**Files:**
- Create: `crates/waml/src/upgrade.rs`
- Modify: `crates/waml/src/lib.rs`
- Modify: `crates/waml/src/frontmatter.rs`
- Modify: `crates/waml/src/uml/analysis.rs`
- Create: `crates/waml/tests/upgrade_inspection.rs`
- Create: `crates/waml/tests/frontmatter_rewrite.rs`

- [ ] Add red migration-reader tests for direct behavior mappings, a use-case legacy diagram, an ER-profile legacy diagram, an empty legacy diagram, and an ambiguous mixed use-case/classifier diagram.
- [ ] Add red rewrite tests with comments, quoted scalars, CRLF input, and unchanged surrounding bytes.
- [ ] Run `rtk cargo test -p waml --test upgrade_inspection --test frontmatter_rewrite`.
  Expected result: compilation fails because the upgrade API does not exist.
- [ ] Add this public migration-only interface:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacyDiagramType {
    Diagram,
    Activity,
    StateMachine,
    Sequence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyDiagramTypeUse {
    pub path: String,
    pub legacy: LegacyDiagramType,
    pub replacement: DiagramKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpgradeInspectionError {
    AmbiguousLegacyDiagram {
        path: String,
        incompatible_members: Vec<String>,
    },
    InvalidLegacyBundle(Vec<Diagnostic>),
}

pub fn inspect_legacy_diagram_types(
    source: &SourceBundle,
) -> Result<Vec<LegacyDiagramTypeUse>, UpgradeInspectionError>;

pub fn replace_frontmatter_string_scalar(
    source: &str,
    key: &str,
    expected: &str,
    replacement: &str,
) -> Result<Option<String>, FrontmatterRewriteError>;
```

- [ ] Keep this API outside the normal preparation entry point. The strict path must still emit `ObsoleteDiagramType`.
- [ ] Resolve member links in the migration reader before classification. Use these rules:
  - Map `uml.Activity`, `uml.StateMachine`, and `uml.Sequence` directly to their canonical diagram kinds.
  - If a legacy `Diagram` contains one or more use cases and no incompatible classifier, select `UseCase`.
  - If it contains a use case and any class, interface, enumeration, data type, activity, state machine, or sequence node, return `AmbiguousLegacyDiagram` and sort the incompatible member keys.
  - Treat actors, notes, packages, relationships, and empty groups as neutral.
  - Select `Class` for every other legacy `Diagram`, including ER-profile and empty diagrams.
- [ ] Replace only the frontmatter scalar token. Preserve its quote style when it is quoted. Preserve the newline style and every byte outside the scalar.
- [ ] Run `rtk cargo test -p waml --test upgrade_inspection --test frontmatter_rewrite`.
  Expected result: all direct, classified, ambiguous, and byte-preservation cases pass.
- [ ] Commit with `rtk git add crates/waml/src/upgrade.rs crates/waml/src/lib.rs crates/waml/src/frontmatter.rs crates/waml/src/uml/analysis.rs crates/waml/tests/upgrade_inspection.rs crates/waml/tests/frontmatter_rewrite.rs` and `rtk git commit -m "feat(waml): inspect legacy diagram documents"`.

### Task 5: Add the ordered and idempotent upgrade registry

**Files:**
- Create: `crates/waml-cli/src/upgrade.rs`
- Modify: `crates/waml-cli/src/main.rs`
- Create: `crates/waml-cli/tests/upgrade_plan.rs`

- [ ] Add red tests that check registry order, the stable migration id, one report per changed file, full-bundle validation, and a byte-identical second run.
- [ ] Run `rtk cargo test -p waml-cli --test upgrade_plan`.
  Expected result: compilation fails because the registry and plan types do not exist.
- [ ] Add these interfaces in `upgrade.rs`:

```rust
pub struct Migration {
    pub id: &'static str,
    pub description: &'static str,
    pub detect: fn(&SourceBundle) -> Result<bool, UpgradeError>,
    pub transform: fn(&SourceBundle) -> Result<SourceBundle, UpgradeError>,
}

pub const DIAGRAM_TYPE_MIGRATION_ID: &str = "canonical-uml-diagram-types";
pub static MIGRATIONS: &[Migration] = &[Migration {
    id: DIAGRAM_TYPE_MIGRATION_ID,
    description: "Use canonical UML diagram document types",
    detect: detect_canonical_uml_diagram_types,
    transform: transform_canonical_uml_diagram_types,
}];

pub struct AppliedMigration {
    pub path: String,
    pub id: &'static str,
    pub description: &'static str,
}

pub struct UpgradePlan {
    pub files: Vec<(String, String)>,
    pub applied: Vec<AppliedMigration>,
}

pub fn plan_upgrade(files: &[(String, String)]) -> Result<UpgradePlan, UpgradeError>;
```

- [ ] Run each detector and transformation in registry order. Pass each transformed in-memory bundle to the next migration.
- [ ] After the last migration, run the normal strict preparation and semantic analysis on the full candidate. Return an error if any error-severity diagnostic exists.
- [ ] Do not write from this module. Return all candidate files and reports to the caller.
- [ ] On a second call with upgraded bytes, return the same file bytes and an empty `applied` list.
- [ ] Run `rtk cargo test -p waml-cli --test upgrade_plan`.
  Expected result: all registry, validation, and idempotence tests pass.
- [ ] Commit with `rtk git add crates/waml-cli/src/upgrade.rs crates/waml-cli/src/main.rs crates/waml-cli/tests/upgrade_plan.rs` and `rtk git commit -m "feat(cli): plan ordered source upgrades"`.

### Task 6: Add the atomic `waml upgrade` command

**Files:**
- Modify: `crates/waml-cli/src/main.rs`
- Modify: `crates/waml-cli/src/io.rs`
- Modify: `crates/waml-cli/tests/cli_e2e.rs`

- [ ] Add red end-to-end tests for these commands:

```text
waml upgrade
waml upgrade PATH
waml upgrade PATH --check
```

- [ ] In the tests, prove these results:
  - The omitted path uses `.`.
  - Write mode changes every valid candidate and reports `path: canonical-uml-diagram-types`.
  - `--check` returns exit code 1 when a migration is required and changes no bytes.
  - `--check` returns exit code 0 after the upgrade.
  - An ambiguous bundle returns exit code 1 and changes no file.
  - A strict validation error returns exit code 1 and changes no file.
  - A journaled multi-file write failure rolls back all files.
  - A cleanup warning after the commit keeps exit code 0 and prints the warning.
- [ ] Run `rtk cargo test -p waml-cli --test cli_e2e upgrade`.
  Expected result: Clap rejects `upgrade` or the assertions fail.
- [ ] Add this command shape:

```rust
Upgrade {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    check: bool,
}
```

- [ ] Read the physical bundle once. Call `plan_upgrade`. In `--check` mode, print the planned reports and do not call the writer.
- [ ] In write mode, pass the old and new complete file lists to the existing `io::write_back` journaled transaction.
- [ ] Print each changed path with the migration id and description. Print post-commit cleanup warnings as warnings.
- [ ] Run `rtk cargo test -p waml-cli --test cli_e2e upgrade` and `rtk cargo test -p waml-cli io::tests`.
  Expected result: all command, rollback, and cleanup tests pass.
- [ ] Commit with `rtk git add crates/waml-cli/src/main.rs crates/waml-cli/src/io.rs crates/waml-cli/tests/cli_e2e.rs` and `rtk git commit -m "feat(cli): add atomic upgrade command"`.

### Task 7: Upgrade repository documents, fixtures, and generated expectations

**Files:**
- Modify: `docs/waml/use-cases/views/editor-workflows.md`
- Modify: `docs/waml/use-cases/views/browser-and-publishing-workflows.md`
- Modify: `docs/waml/use-cases/views/tooling-workflows.md`
- Modify: `docs/waml/architecture/views/authoring-and-validation.md`
- Modify: `docs/waml/architecture/views/crate-ownership.md`
- Modify: `docs/waml/architecture/views/deployment-surfaces.md`
- Modify: `docs/waml/architecture/views/domain-model.md`
- Modify: `docs/waml/architecture/views/editing-round-trip.md`
- Modify: `docs/waml/architecture/views/editor-ownership.md`
- Modify: `docs/waml/architecture/views/incremental-analysis.md`
- Modify: `docs/waml/architecture/views/layout-solving.md`
- Modify: `docs/waml/architecture/views/model-vocabulary.md`
- Modify: `docs/waml/architecture/views/preparation-pipeline.md`
- Modify: `docs/waml/architecture/views/revisioned-edit-transaction.md`
- Modify: `docs/waml/architecture/views/share-round-trip.md`
- Modify: `docs/waml/architecture/views/system-context.md`
- Modify: `docs/waml/architecture/views/web-delivery.md`
- Modify: `docs/uaml-spec.md`
- Modify: `docs/superpowers/specs/2026-07-11-diagram-layout-language-design.md`
- Modify: `docs/superpowers/specs/2026-07-11-okf-agnostic-profiles-uml-domain-design.md`
- Modify: `docs/superpowers/specs/2026-07-11-uaml-behavioral-substrates-design.md`
- Modify: `docs/superpowers/specs/2026-08-02-folder-view-design.md`
- Modify: `crates/waml-editor/tests/fixtures/groups-linked/*.md`
- Modify: `crates/waml-editor/tests/fixtures/groups/*.md`
- Modify: `crates/waml-editor/tests/fixtures/mini/*.md`
- Modify: `crates/waml-editor/tests/fixtures/mixed-okf/*.md`
- Modify: `crates/waml-editor/tests/fixtures/sixkind/*.md`
- Modify: `crates/waml/tests/fixtures/behavior/activity/flow.md`
- Modify: `crates/waml/tests/fixtures/behavior/sequence-nested/sequence.md`
- Modify: `crates/waml/tests/fixtures/behavior/state-machine/states.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/activity.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/diagram.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/sequence.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/state-machine.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/recovery/diagram.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/recovery/flow.md`
- Modify: `crates/waml/tests/fixtures/parser-platform/recovery/sequence.md`
- Modify: `crates/waml/tests/fixtures/orders-domain.md`
- Modify: `fuzz/seeds/uml_islands/activity.md`
- Modify: `fuzz/seeds/uml_islands/diagram.md`
- Modify: `fuzz/seeds/uml_islands/sequence.md`
- Modify: `crates/waml/tests/diagram_display_projection.rs`
- Modify: `crates/waml/tests/edit_lowering_order.rs`
- Modify: `crates/waml/tests/formatter_actions.rs`
- Modify: `crates/waml/tests/href_contract.rs`
- Modify: `crates/waml/tests/incremental_analysis.rs`
- Modify: `crates/waml/tests/layout_atom_api.rs`
- Modify: `crates/waml/tests/layout_serde_roundtrip.rs`
- Modify: `crates/waml/tests/prepared_referrers.rs`
- Modify: `crates/waml/tests/sequence_formatter.rs`
- Modify: `crates/waml/tests/sequence_language_syntax.rs`
- Modify: `crates/waml/tests/sequence_semantics.rs`
- Modify: `crates/waml/tests/uml_lowering_order.rs`
- Modify: `crates/waml-cli/tests/lsp_e2e.rs`

- [ ] Run `rtk cargo run -p waml-cli -- upgrade docs --check`.
  Expected result: exit code 1 and a report for every old document type under `docs`.
- [ ] Run `rtk cargo run -p waml-cli -- upgrade docs`, `rtk cargo run -p waml-cli -- upgrade crates`, and `rtk cargo run -p waml-cli -- upgrade fuzz`.
  Expected result: the command upgrades physical Markdown documents, fixtures, and seeds. It does not change the approved 2026-08-09 design specification or the new upgrade tests that must contain old input.
- [ ] Update embedded Rust Markdown literals and string expectations manually because the path-based command does not rewrite source-code strings. Use canonical output names in every generator and formatter expectation. Keep old inputs only in migration and obsolete-diagnostic tests.
- [ ] Run `rtk cargo run -p waml-cli -- upgrade . --check`.
  Expected result: exit code 0 and no changed-file report.
- [ ] Run `rtk rg -n --glob "*.md" '^type:\s*(Diagram|uml\.(Activity|StateMachine|Sequence))$' docs/waml docs/uaml-spec.md crates/waml/tests/fixtures crates/waml-editor/tests/fixtures fuzz/seeds`.
  Expected result: no output. Historical design records can keep historical examples. Old strings in source code can remain only as migration input or obsolete-diagnostic text.
- [ ] Run `rtk cargo test -p waml` and `rtk cargo test -p waml-editor`.
  Expected result: all migrated fixtures and expectations pass.
- [ ] Commit with `rtk git add docs fuzz/seeds/uml_islands crates/waml crates/waml-cli crates/waml-editor crates/waml-ops-dto` and `rtk git commit -m "docs: upgrade UML diagram document types"`.

### Task 8: Verify the complete canonical-type and upgrade change

**Files:**
- Verify: all files changed in Tasks 1-7

- [ ] Run `rtk cargo fmt --all -- --check`.
  Expected result: exit code 0.
- [ ] Run `rtk cargo test --workspace`.
  Expected result: all workspace tests pass.
- [ ] Run `rtk cargo clippy --workspace --all-targets -- -D warnings`.
  Expected result: exit code 0 with no warning.
- [ ] Run `rtk cargo run -p waml-cli -- upgrade . --check`.
  Expected result: exit code 0 and no changed-file report.
- [ ] Create `target/diagram-upgrade-verification` with one document for each old type. Run `rtk cargo run -p waml-cli -- check target/diagram-upgrade-verification`.
  Expected result: each document gets `obsolete-diagram-type`; the generic `Diagram` message tells the user to run `waml upgrade`.
- [ ] Copy the same files to `target/diagram-upgrade-idempotence`, run `rtk cargo run -p waml-cli -- upgrade target/diagram-upgrade-idempotence` twice, and compare the bytes after both runs.
  Expected result: the first run reports migrations, the second reports none, and the second run changes zero bytes.
- [ ] Run `rtk git diff --check`.
  Expected result: no whitespace error.
- [ ] Review the diff and confirm that the normal parser has no old-type alias, migration validation occurs before writing, and all five creation paths emit canonical types.

## Plan Self-Review

- The plan covers the five exact canonical type names and strict rejection of all four old names.
- The plan keeps semantic behavior types separate from document view types.
- The plan uses declared kind dispatch, including empty diagrams.
- The plan covers direct mappings, resolved legacy `Diagram` classification, neutral members, ambiguity, ER-profile diagrams, and empty diagrams.
- The plan defines a stable ordered migration registry and an idempotent transform.
- The plan validates the full in-memory candidate before the journaled write.
- The plan covers default path, write mode, `--check`, reports, rollback, and cleanup warnings.
- The plan migrates repository documents, fixtures, seeds, generators, formatter expectations, tests, and documentation.
- The plan has no compatibility alias, no parser grammar extension, and no placeholder.
