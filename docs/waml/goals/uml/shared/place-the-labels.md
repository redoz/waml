# Place the Labels

**Goal:** Every label — node title, member, edge name, multiplicity, role,
stereotype, guard — is legible, positioned, and unclipped.

**Why:** An unreadable label is a wrong diagram. Labels are where layout
failures become visible first.

**Done when:** No label in this bundle is clipped, overlapped, or placed
ambiguously between two edges, at the default zoom and at every zoom a reader
can reach.

**Status:** partial — unverified
**MVP:** yes

## Notes

- Edge labels have their own placement pass; the solver sizes connected gaps to
  hold terminal labels.
- Text measurement is shared with [Solve the
  Layout](./solve-the-layout.md) — a label the solver measured differently from
  the renderer is the classic source of clipping.
- Font sizes in text styles are points, and the canvas has previously
  double-applied the 96/72 conversion. Any measurement bug should check that
  first.
