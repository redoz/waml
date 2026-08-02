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
- Label placement is a solver stage in world space, not a renderer concern. A
  renderer that places labels cannot know what else is on the canvas, which is
  why labels overlap each other and vanish under node cards.
- Placement never fails silently. A label that cannot be placed cleanly gets a
  leader line to its owner instead. Drawing a label on top of something else is
  not an acceptable outcome of any input.
- A lifeline head shows the authored title and is measured on that title. The
  resolved reference key is what the model uses to correlate messages, and it
  is never what the reader sees.
- Text measurement is shared with [Solve the
  Layout](./solve-the-layout.md) — a label the solver measured differently from
  the renderer is the classic source of clipping.
- Font sizes in text styles are points, and the canvas has previously
  double-applied the 96/72 conversion. Any measurement bug should check that
  first.
