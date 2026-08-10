# Use-Case Edge Routing Design

## Goal

Make shipped use-case diagrams easy to trace without changing their notation,
node rendering, authored constraints, semantic containment, actor order, or band
order.

## Diagnosis

Use-case defaults add placement constraints for every actor rail and band. The
constrained stress path therefore does not run its generic crossing-reduction
pass. The default member order uses only the smallest adjacent actor rank, which
loses the full actor-to-use-case adjacency pattern. The generic Manhattan router
avoids node and group hulls, but it does not know about the visible heading
strips. Finally, the editor clips rectangular route endpoints to measured actor
figures and ellipses after routing. Moving only the endpoint can turn an
orthogonal terminal stub into a diagonal.

## Design

Use three bounded phases at their existing ownership layers:

1. `solve/use_case.rs` computes a stable member order from the complete ordered
   actor-adjacency vector. Authored actor order and band order stay fixed. Only
   the generated defaults for members inside a band can change. Ties retain
   authored member order and key order.
2. `solve/route.rs` receives explicit hard routing obstacles and provides a
   deterministic terminal clipping helper. A hard obstacle is always active,
   unlike a containing group hull. The helper clips to measured actor or ellipse
   geometry and inserts the minimum orthogonal elbow needed to preserve a clean
   terminal stub. Generic route length, bend, obstacle, hub-spread, and lane
   separation rules remain reusable here.
3. `waml-editor/src/scene.rs` projects use-case heading strips into hard routing
   obstacles because this layer owns measured headings. It passes measured port
   geometry to terminal clipping after initial routing and after label rerouting.
   It does not choose semantic order or draw different notation.

The routing search remains deterministic A* over a finite orthogonal visibility
graph. Candidate and obstacle order use ordered collections and total floating
point comparison. No random or unbounded optimization is added.

## Invariants

- Authored placements and alignments take precedence over generated defaults.
- Actor and band order do not change.
- Group membership is semantic, not inferred from rectangle overlap.
- Node bounds and every heading strip are hard obstacles for unrelated routes.
- Actor and ellipse endpoints use measured geometry.
- Include, extend, specialization, association direction, markers, and labels
  continue to use the existing scene edge order.
- Repeated solves produce equal route and label geometry.

## Tests

Add focused regressions for the real Editor Workflows crossing count, one actor
with several relationships, two actors with interleaved targets, nested heading
strips, disconnected and direct boundary members, label rerouting, endpoint
orthogonality, and repeated byte-stable solves. Keep the existing marker tests.

Validate all three shipped use-case documents with native screenshots. Review
the updated images independently before the full workspace gates.
