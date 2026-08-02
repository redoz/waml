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
- Two plans own the remaining work.
  `docs/superpowers/plans/2026-08-02-edge-label-placement.md` moves placement
  out of the renderer into the solver as a world-space stage, so labels stop
  overlapping each other and stop vanishing under node cards.
  `2026-08-03-edge-label-route-pressure.md` adds a leader line for any label
  that still cannot be placed, so no label is ever drawn on top of anything.
- `2026-07-31-sequence-lifeline-title-label.md` fixes lifeline heads to display
  and measure the authored title while keeping the resolved key for behavior.
- Text measurement is shared with [Solve the
  Layout](./solve-the-layout.md) — a label the solver measured differently from
  the renderer is the classic source of clipping.
- Font sizes in text styles are points, and the canvas has previously
  double-applied the 96/72 conversion. Any measurement bug should check that
  first.
