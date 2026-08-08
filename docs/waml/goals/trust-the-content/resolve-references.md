# Resolve References

**Goal:** A reference resolves, or the tool reports it at its source position.

**Why:** A silent broken edge gives the reader incorrect bundle information.

**Done when:** Relationship targets, `describes` targets, slot references, and
text links resolve or produce a positioned diagnostic.

**Status:** partial
**MVP:** yes

## Notes

- `crates/waml/src/uml.rs::relationships_and_diagram_members_resolve_only_claimed_concepts`
  checks relationship and diagram-member resolution.
- `crates/waml/tests/sequence_semantics.rs::sequence_describes_resolves_through_the_shared_link_ref_parser`
  checks a sequence `describes` reference through the shared parser.
- `crates/waml/tests/semantic_diagnostics.rs::unresolved_diagram_member_is_a_precise_warning`
  checks a positioned diagnostic for an unresolved diagram member.
- The evidence does not cover every Markdown text-link target. The goal stays
  partial.
- [Report Every Problem](./report-every-problem.md) owns delivery of a detected
  problem to a user surface.
