# Use Case Feature Cut

**Goal:** A use case diagram in WAML shows the users of the system and the
boundary of the system.

**Done when:** Every row below is `done` or `horizon`, and no `planned` row is
`MVP: yes`.

**Status:** partial — unverified
**MVP:** no

Every status in this table is a first reading of the code. `Evidence` shows
`unverified` until an audit replaces it with a `file:line` reference or the
name of a test.

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

- The model part of this kind exists. `uml.Actor` and `uml.UseCase` are
  metaclasses. `includes`, `extends`, `associates`, and `specializes` are
  relationship kinds. The view does not exist. At this time these elements draw
  through the structural view as boxes.
- Thus this kind is separate from [Class](../class/feature-cut.md). The
  remaining work is presentation and layout only. A reader must see that work
  as one task.
- Each row has the flag `MVP: no`. `docs/waml` describes its actors in text at
  this time. The bar does not ask for more.
