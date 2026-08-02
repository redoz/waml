# Keep the Map Stable

**Goal:** An edit perturbs only the neighbourhood it touches. The rest of the
diagram holds its position.

**Why:** This is the goal that decides whether editing a laid-out diagram feels
possible at all. An author builds a mental map of where things are; a solver
that reflows the whole drawing because one node was added destroys that map on
every edit, and no amount of good click targets compensates.

**Done when:** Adding, removing, renaming, or reconnecting one element leaves
every unaffected node within a small bounded distance of where it was, and the
change animates or is otherwise legible rather than appearing as a new drawing.

**Status:** planned — unverified
**MVP:** yes

## Notes

- The current solvers lay out from scratch. Nothing carries the previous
  solution forward as a bias, so an edit is free to produce a wholly different
  arrangement.
- The usual remedy is to seed the solve with the previous positions and
  penalise displacement, which is a smaller change than it sounds and does not
  require a different algorithm.
- This interacts with [Arrange a
  Diagram](../../author-in-the-editor/arrange-a-diagram.md): explicit
  constraints are the author's hard override, stability is the solver's soft
  courtesy. Both are needed; neither replaces the other.
- `MVP: yes` — the dogfood bar requires authoring `docs/waml` in the editor,
  and its diagrams are large enough that whole-diagram reflow on each edit
  would make that impractical.
