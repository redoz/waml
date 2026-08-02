# Resolve References

**Goal:** A link to a document or to an element resolves, or the tool reports
it.

**Why:** The value of a bundle is its graph. A broken edge that the tool does
not report gives incorrect information to the reader.

**Done when:** Each relationship target, each `describes` target, each slot
reference, and each link in text in this bundle resolves. Each reference that
does not resolve causes a diagnostic at its position.

**Status:** partial — unverified
**MVP:** yes

## Notes

- The model layer resolves relationship targets and `describes` targets.
  Verify whether the tool examines links in text.
- A link to a heading in another document is a different condition from a link
  to the document. The tool probably does not examine it.
- [Report Every Problem](./report-every-problem.md) delivers the report. This
  goal finds the problem.
