---
type: uml.Sequence
title: Broken sequence
---
# Broken sequence

## Lifelines
- [Target](./target.md) as target
- missing-link

## Messages
- alt
   - when `bad-indent`
  - when `ok`
    - target calls
    - opt
      - else
        - target returns to
- par
  - when `not-a-branch`
- ref [Target](./target.md) as target-use
  - bind target target
- target signals target

## Relationships
- depends [Target](./target.md)
