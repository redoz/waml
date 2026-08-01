---
type: uml.Class
title: Profile
description: A presentation lens a Diagram selects to decide what its members emphasize.
---

# Profile

## Relationships
- associates [Diagram](./diagram.md): 1 profile to 0..* diagrams
- depends [Stereotype](./stereotype.md)

## Notes
- A profile selects the details that a view shows. It also gives the appearance of the stereotyped elements.
- A profile does not change the meaning of the model. The same elements under a different profile have a different appearance and the same meaning.
- The emphasis is a property of the view that selects the profile. It is not a property of the element.
