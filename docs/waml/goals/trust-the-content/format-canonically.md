# Format Canonically

**Goal:** The formatter writes the same bytes for the same model each time.

**Why:** Stable bytes keep a review focused on the semantic change.

**Done when:** A first format pass produces canonical source and a second pass
produces no change.

**Status:** done
**MVP:** yes

## Notes

- `crates/waml-cli/tests/cli_e2e.rs::fmt_canonical_output_is_idempotent`
  checks canonical CLI output and an unchanged second pass.
- `crates/waml/tests/formatter_actions.rs::noncanonical_claimed_families_match_exact_canonical_bytes`
  checks exact canonical output for claimed UML families.
- `crates/waml/tests/sequence_formatter.rs::canonical_sequence_format_is_idempotent_and_semantic`
  checks idempotent canonical sequence formatting and retained semantics.
