# Use Case Feature Cut

**Goal:** A use case diagram in WAML expresses who the system serves and where
its boundary lies.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** no

Every status in this table is a first-pass guess. `Evidence` reads
`unverified` until an audit replaces it with a `file:line` or a test name.

## Elements

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Actor | done | no | unverified |
| Actor drawn as a stick figure rather than a box | planned | no | unverified |
| Use case | done | no | unverified |
| Use case drawn as an ellipse | planned | no | unverified |
| System boundary box | planned | no | unverified |
| Note anchored to an element | partial | no | unverified |

## Relationships

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| Actor associates use case | done | no | unverified |
| `includes` between use cases | done | no | unverified |
| `extends` between use cases | done | no | unverified |
| Extension point on an extend | horizon | no | unverified |
| Actor generalization via `specializes` | done | no | unverified |
| Use case generalization via `specializes` | done | no | unverified |

## Presentation

| Feature | Status | MVP | Evidence |
| --- | --- | --- | --- |
| A document type that selects the use case view | planned | no | unverified |
| Layout that puts actors outside the boundary | planned | no | unverified |

## Notes

- The model side of this kind already exists: `uml.Actor` and `uml.UseCase` are
  metaclasses, and `includes`, `extends`, `associates`, and `specializes` are
  relationship kinds. What is missing is the *view* — today these render
  through the structural view as boxes.
- That is why this is its own kind rather than rows under
  [Class](../class/feature-cut.md): the remaining work is entirely presentation
  and layout, and it belongs somewhere a reader can see it as one job.
- `MVP: no` throughout. `docs/waml` documents its actors as prose today and the
  dogfood bar does not ask for more.
