# Sequence Feature Cut

**Goal:** A sequence diagram in WAML expresses the language and model that an
architecture document needs to describe an interaction over time.

**Done when:** Every MVP language and model row below is `done`, and every
other row is `done` or `horizon`.

**Status:** partial
**MVP:** yes

This document records language and model coverage. [Sequence
Language](./language.md) defines the authored form. [Interact with a Sequence
Diagram](./interact-with-a-sequence-diagram.md) owns sequence-diagram UI
behavior.

## Participants

| Feature | Status | MVP |
| --- | --- | --- |
| Lifeline | done | yes |
| Lifeline typed by a classifier | done | yes |
| Actor as a participant | done | no |
| Lifeline ordering under author control | done | no |
| Activation bar | done | yes |
| Nested activation | done | no |
| Create message and delayed lifeline start | done | no |
| Destroy message and lifeline end | done | no |

## Messages

| Feature | Status | MVP |
| --- | --- | --- |
| `calls` — synchronous message | done | yes |
| `calls ... async` — asynchronous message | done | yes |
| `returns` — reply message | done | yes |
| `signals` — signal message | done | yes |
| `creates` — create message | done | no |
| `destroys` — destroy message | done | no |
| Self message | done | yes |
| Message arguments | done | no |
| Message return value | done | no |
| Lost and found message | horizon | no |

## Structure

| Feature | Status | MVP |
| --- | --- | --- |
| `alt` fragment — one or more `when`, optional final `else` | done | yes |
| `opt` fragment — exactly one `when` | done | yes |
| `loop` fragment — exactly one `when` | done | yes |
| `break` fragment — exactly one `when` | done | no |
| `par` fragment — two or more `branch` | done | no |
| `critical` fragment — exactly one `branch` | done | no |
| `assert` fragment — exactly one `branch` | done | no |
| `neg` fragment — exactly one `branch` | done | no |
| Nested fragments | done | no |
| Guard on a fragment operand | done | yes |
| `ref` — interaction use of another interaction | done | no |
| Gate at the interaction boundary, with bindings | done | no |
| `outside` as a boundary endpoint | done | no |
| Note anchored to a message | partial | no |
| Time and duration constraint | horizon | no |
| Coregion, continuation, general ordering | horizon | no |
| `strict`, `seq`, `ignore`, `consider` fragments | horizon | no |
| Part decomposition, state invariant, execution specification | horizon | no |

## Evidence

- Canonical messages and their authored fields are covered by
  `crates/waml/tests/sequence_language_syntax.rs::canonical_messages_have_fixed_lossless_slots`.
- Return correlation and diagnostics are covered by
  `crates/waml/tests/sequence_semantics.rs::returns_follow_the_locked_candidate_algorithm`.
- Fragment kinds and operand rules are covered by
  `crates/waml/tests/sequence_semantics.rs::fragment_operand_rules_are_exact`.
- Nested fragments and authored order are covered by
  `crates/waml/tests/sequence_semantics.rs::nested_fragments_keep_order_and_branch_boundaries`.
- Create and destroy lifetimes are covered by
  `crates/waml/tests/interaction_solver_golden.rs::creates_target_stem_starts_at_its_row_and_destroys_ends_it`.
- Activation nesting is covered by
  `crates/waml/tests/interaction_solver_golden.rs::activation_nesting_is_contained_and_depth_matches_stack`.
- Interaction uses, gates, and bindings are covered by
  `crates/waml/tests/sequence_semantics.rs::interaction_use_resolves_without_flattening`.

## Notes

- The [Sequence Language](./language.md) defines the accepted authored forms
  and their stable language scenarios.
- Excluded constructs are intentional language limits, not planned work.
- [Interact with a Sequence
  Diagram](./interact-with-a-sequence-diagram.md) owns rendering, hit testing,
  selection, camera, refresh, and unavailable-document results.
