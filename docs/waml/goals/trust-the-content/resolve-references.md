# Resolve References

**Goal:** A link to a document or an element either resolves or is reported.

**Why:** The bundle's value is its graph. A broken edge that nobody reports is
a lie the reader believes.

**Done when:** Every relationship target, `describes` target, slot reference,
and prose link in this bundle resolves, and every unresolvable one produces a
diagnostic at its exact position.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Relationship and `describes` targets are resolved through the model layer.
  Whether plain prose links are checked at all is the unverified part.
- A link to a heading inside another document is a distinct case from a link to
  the document, and probably unhandled.
- Reporting is delivered through [Report Every
  Problem](./report-every-problem.md); this goal owns the detection.
